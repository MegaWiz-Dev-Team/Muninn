pub mod analyzer;
pub mod pr_creator;

use std::sync::Arc;
use crate::AppState;
use crate::models::AnalysisStatus;

/// Process pending issues: analyze → generate fix → create PR
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

    let llm = crate::llm::LlmClient::new(
        state.http.clone(),
        &state.config.heimdall_url,
        &state.config.gemini_api_key,
    );

    for issue in issues {
        tracing::info!("🔍 Analyzing: {}#{} — {}", issue.repo, issue.issue_number, issue.title);

        // Update status
        let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Analyzing);

        // Step 1: Analyze issue
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

        let fix = match analyzer::generate_fix(&llm, &issue, &analysis).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Fix generation failed: {}", e);
                let _ = state.db.update_issue_status(&issue.id, &AnalysisStatus::Failed);
                continue;
            }
        };

        tracing::info!("✅ Fix generated: {} file changes", fix.file_changes.len());

        // Step 3: Create PR (if GitHub token available)
        if !state.config.github_token.is_empty() {
            match pr_creator::create_fix_pr(&state, &issue, &fix).await {
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
}
