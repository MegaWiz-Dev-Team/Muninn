use std::time::Duration;
use uuid::Uuid;

use super::{CodeAgentConfig, CodeAgentResult, build_fix_prompt};
use super::runner;

/// OpenCode CLI agent wrapper
///
/// OpenCode (https://github.com/opencode-ai/opencode) is a Go-based CLI
/// AI coding agent that supports multiple LLM providers. Muninn invokes it
/// in non-interactive mode to fix code issues.
pub struct OpenCodeAgent {
    config: CodeAgentConfig,
}

impl OpenCodeAgent {
    pub fn new(config: CodeAgentConfig) -> Self {
        Self { config }
    }

    /// Fix an issue using OpenCode CLI
    pub async fn fix_issue(
        &self,
        repo_url: &str,
        issue_title: &str,
        issue_body: &str,
        affected_files: &[String],
    ) -> Result<CodeAgentResult, String> {
        let session_id = Uuid::new_v4().to_string();
        let work_dir = format!("{}/opencode-{}", self.config.work_dir, &session_id[..8]);

        // Ensure workspace exists
        std::fs::create_dir_all(&self.config.work_dir)
            .map_err(|e| format!("Failed to create work dir: {}", e))?;

        // Step 1: Clone repository
        tracing::info!("📦 [OpenCode] Cloning {} for session {}", repo_url, &session_id[..8]);
        runner::git_clone(
            repo_url,
            &work_dir,
            &self.config.github_token,
            Duration::from_secs(120),
        )
        .await?;

        // Step 2: Create prompt file
        let prompt = build_fix_prompt(issue_title, issue_body, affected_files);
        let prompt_file = format!("{}/muninn_prompt.md", work_dir);
        std::fs::write(&prompt_file, &prompt)
            .map_err(|e| format!("Failed to write prompt file: {}", e))?;

        // Step 3: Run OpenCode CLI
        tracing::info!("🤖 [OpenCode] Running agent for: {}", issue_title);
        let timeout = Duration::from_secs(self.config.timeout_secs);

        let mut env_vars: Vec<(&str, &str)> = vec![];
        // Pass Gemini API key if available (OpenCode supports Gemini as provider)
        if !self.config.gemini_api_key.is_empty() {
            env_vars.push(("GEMINI_API_KEY", &self.config.gemini_api_key));
        }

        let output = runner::run_agent_command(
            &self.config.opencode_path,
            &[
                "--prompt",
                &prompt,
                "--non-interactive",
            ],
            &work_dir,
            &env_vars,
            timeout,
        )
        .await;

        // Step 4: Collect results
        let cmd_output = match output {
            Ok(o) => o,
            Err(e) => {
                runner::cleanup_workspace(&work_dir);
                return Err(format!("[OpenCode] Agent execution failed: {}", e));
            }
        };

        // Step 5: Check for changed files
        let files_changed = runner::git_changed_files(&work_dir)
            .await
            .unwrap_or_default();

        let result = CodeAgentResult {
            success: cmd_output.exit_code == 0 && !files_changed.is_empty(),
            files_changed,
            stdout: cmd_output.stdout,
            stderr: cmd_output.stderr,
            exit_code: cmd_output.exit_code,
        };

        // Clean up prompt file (keep workspace for git operations)
        let _ = std::fs::remove_file(&prompt_file);

        if result.success {
            tracing::info!(
                "✅ [OpenCode] Fix applied: {} files changed",
                result.files_changed.len()
            );
        } else {
            tracing::warn!("⚠️ [OpenCode] No files changed or agent failed");
            runner::cleanup_workspace(&work_dir);
        }

        Ok(result)
    }

    /// Get the workspace directory for a running session
    pub fn get_work_dir(&self, session_id: &str) -> String {
        format!("{}/opencode-{}", self.config.work_dir, &session_id[..8.min(session_id.len())])
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_agent::CodeAgentProvider;

    fn test_config() -> CodeAgentConfig {
        CodeAgentConfig {
            provider: CodeAgentProvider::OpenCode,
            work_dir: "/tmp/muninn-test".to_string(),
            timeout_secs: 300,
            gemini_api_key: String::new(),
            opencode_path: "opencode".to_string(),
            gemini_cli_path: "gemini".to_string(),
            github_token: String::new(),
        }
    }

    #[test]
    fn test_opencode_agent_creation() {
        let config = test_config();
        let agent = OpenCodeAgent::new(config);
        assert_eq!(agent.config.provider, CodeAgentProvider::OpenCode);
        assert_eq!(agent.config.opencode_path, "opencode");
    }

    #[test]
    fn test_prompt_generation_for_opencode() {
        let prompt = build_fix_prompt(
            "SQL Injection in /api/login",
            "Found parameterized query bypass in login handler",
            &["src/handlers/login.rs".to_string()],
        );
        assert!(prompt.contains("SQL Injection"));
        assert!(prompt.contains("src/handlers/login.rs"));
        assert!(prompt.contains("MINIMAL changes"));
    }

    #[test]
    fn test_work_dir_generation() {
        let config = test_config();
        let agent = OpenCodeAgent::new(config);
        let work_dir = agent.get_work_dir("abcdef12-3456-7890-abcd-ef1234567890");
        assert!(work_dir.starts_with("/tmp/muninn-test/opencode-"));
        assert!(work_dir.contains("abcdef12"));
    }

    #[test]
    fn test_prompt_file_content() {
        let prompt = build_fix_prompt(
            "XSS in search",
            "User input not escaped in search results page",
            &["src/web/search.rs".to_string(), "templates/search.html".to_string()],
        );
        // Verify prompt structure
        assert!(prompt.contains("## Issue"));
        assert!(prompt.contains("## Affected Files"));
        assert!(prompt.contains("## Rules"));
        assert!(prompt.contains("- src/web/search.rs"));
        assert!(prompt.contains("- templates/search.html"));
    }
}
