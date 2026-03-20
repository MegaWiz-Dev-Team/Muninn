pub mod analyzer;
pub mod pr_creator;

use std::sync::Arc;
use crate::AppState;
use crate::models::AnalysisStatus;
use crate::code_agent::{self, CodeAgentConfig, CodeAgentProvider};

/// Process pending issues: analyze → generate fix → create PR (or review)
pub async fn process_pending_issues(state: Arc<AppState>) {
    let issues = match state.db.list_issues(Some("pending")) {
        Ok(issues) => issues,
        Err(e) => {
            tracing::error!("Failed to list pending issues: {}", e);
            return;
        }
    };

    if issues.is_empty() {
        return;
    }

    tracing::info!("🔧 Processing {} pending issues", issues.len());

    let llm = crate::llm::LlmClient::with_config(
        state.http.clone(),
        &state.config.heimdall_url,
        &state.config.gemini_api_key,
        &state.config.llm_provider,
        &state.config.llm_model,
        &state.config.gemini_model,
        state.config.llm_temperature,
        state.config.llm_max_tokens,
    );

    let review_mode = state.config.is_review_mode();
    if review_mode {
        tracing::info!("👁️ Review mode: fixes will be saved for approval");
    }

    // Determine code agent provider
    let agent_provider = CodeAgentProvider::from_str(&state.config.code_agent_provider);
    let use_code_agent = agent_provider != CodeAgentProvider::None;

    if use_code_agent {
        tracing::info!("🤖 Code agent mode: {}", agent_provider);
    }

    for issue in issues {
        tracing::info!("🔍 Analyzing: {}#{} — {}", issue.repo, issue.issue_number, issue.title);

        // Update status
        let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Analyzing);

        // Step 1: Analyze issue (always uses LLM)
        let analysis = match analyzer::analyze_issue(&llm, &issue).await {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Analysis failed for {}: {}", issue.id, e);
                let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Failed);
                continue;
            }
        };

        tracing::info!("📋 Analysis: root_cause={}, fixable={}, files={}", analysis.root_cause, analysis.fixable, analysis.affected_files.len());

        if !analysis.fixable {
            tracing::info!("⏭️ Issue not auto-fixable, skipping");
            let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Skipped);
            continue;
        }

        // Safety: max 3 files rule (M2.7)
        if analysis.affected_files.len() > 3 {
            tracing::warn!("⚠️ Fix requires {} files (max 3). Creating issue instead.", analysis.affected_files.len());
            let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Skipped);
            continue;
        }

        // Step 2: Generate fix
        let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Fixing);

        if use_code_agent {
            // ── Code Agent CLI path ──
            let agent_config = build_agent_config(&state, &agent_provider);
            let repo_url = format!("https://github.com/{}", issue.repo);

            match code_agent::run_code_agent_fix(
                &agent_config,
                &repo_url,
                &issue.title,
                &issue.body,
                &analysis.affected_files,
            )
            .await
            {
                Ok(result) if result.success => {
                    tracing::info!(
                        "🤖 Code agent fix: {} files changed",
                        result.files_changed.len()
                    );

                    // Commit, push and create PR from agent workspace
                    let session_work_dir = format!(
                        "{}/{}-{}",
                        state.config.code_agent_work_dir,
                        if agent_provider == CodeAgentProvider::OpenCode { "opencode" } else { "gemini" },
                        &issue.id[..8.min(issue.id.len())]
                    );
                    let branch_name = format!("fix/muninn-{}-agent", issue.issue_number);
                    let commit_msg = format!(
                        "fix: [Muninn Agent] {}\n\nCo-authored-by: Muninn <muninn@asgard.ai>",
                        issue.title
                    );

                    match code_agent::runner::git_commit_and_push(
                        &session_work_dir,
                        &branch_name,
                        &commit_msg,
                    )
                    .await
                    {
                        Ok(()) => {
                            // Create PR via GitHub API
                            if !state.config.github_token.is_empty() {
                                let fix = analyzer::GeneratedFix {
                                    branch_name: branch_name.clone(),
                                    commit_message: commit_msg,
                                    file_changes: vec![],
                                    test_commands: vec!["cargo test".to_string()],
                                };
                                match pr_creator::create_fix_pr(&state, &issue, &fix).await {
                                    Ok(pr_url) => {
                                        tracing::info!("🎉 PR created: {}", pr_url);
                                        let _ = state.db.update_issue_status(
                                            &issue.id,
                                            &AnalysisStatus::Fixed,
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!("PR creation failed: {}", e);
                                        let _ = state.db.update_issue_status(
                                            &issue.id,
                                            &AnalysisStatus::Failed,
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Git push failed: {}", e);
                            let _ = state.db.update_issue_status(
                                &issue.id,
                                &AnalysisStatus::Failed,
                            );
                        }
                    }

                    // Cleanup workspace
                    code_agent::runner::cleanup_workspace(&session_work_dir);
                }
                Ok(_) => {
                    tracing::warn!("⚠️ Code agent produced no changes, falling back to LLM");
                    // Fallback to LLM-based fix
                    run_llm_fix(&state, &llm, &issue, &analysis).await;
                }
                Err(e) => {
                    tracing::error!("Code agent failed: {}, falling back to LLM", e);
                    // Fallback to LLM-based fix
                    run_llm_fix(&state, &llm, &issue, &analysis).await;
                }
            }
        } else {
            // ── LLM API path (original) ──
            run_llm_fix(&state, &llm, &issue, &analysis).await;
        }
    }
}

/// Original LLM-based fix generation path
async fn run_llm_fix(
    state: &Arc<AppState>,
    llm: &crate::llm::LlmClient,
    issue: &crate::models::TrackedIssue,
    analysis: &analyzer::IssueAnalysis,
) {
    let fix = match analyzer::generate_fix(llm, issue, analysis).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Fix generation failed: {}", e);
            let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Failed);
            return;
        }
    };

    tracing::info!("✅ Fix generated: {} file changes", fix.file_changes.len());

    // Create PR (if GitHub token available)
    if !state.config.github_token.is_empty() {
        match pr_creator::create_fix_pr(state, issue, &fix).await {
            Ok(pr_url) => {
                tracing::info!("🎉 PR created: {}", pr_url);
                let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Fixed);
            }
            Err(e) => {
                tracing::error!("PR creation failed: {}", e);
                let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Failed);
            }
        }
    } else {
        tracing::warn!("No GITHUB_TOKEN — fix generated but PR not created");
        let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Analyzed);
    }
}

/// Build code agent config from app state
fn build_agent_config(state: &Arc<AppState>, provider: &CodeAgentProvider) -> CodeAgentConfig {
    CodeAgentConfig {
        provider: provider.clone(),
        work_dir: state.config.code_agent_work_dir.clone(),
        timeout_secs: state.config.code_agent_timeout_secs,
        gemini_api_key: state.config.gemini_api_key.clone(),
        opencode_path: state.config.opencode_path.clone(),
        gemini_cli_path: state.config.gemini_cli_path.clone(),
        github_token: state.config.github_token.clone(),
    }
}
