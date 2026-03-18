use std::time::Duration;
use uuid::Uuid;

use super::{CodeAgentConfig, CodeAgentResult, build_fix_prompt};
use super::runner;

/// Gemini CLI agent wrapper
///
/// Google's Gemini CLI (https://github.com/google-gemini/gemini-cli) is an
/// AI coding agent that runs in the terminal. Muninn invokes it in
/// non-interactive mode to fix code issues using Gemini models.
pub struct GeminiCliAgent {
    config: CodeAgentConfig,
}

impl GeminiCliAgent {
    pub fn new(config: CodeAgentConfig) -> Self {
        Self { config }
    }

    /// Fix an issue using Gemini CLI
    pub async fn fix_issue(
        &self,
        repo_url: &str,
        issue_title: &str,
        issue_body: &str,
        affected_files: &[String],
    ) -> Result<CodeAgentResult, String> {
        let session_id = Uuid::new_v4().to_string();
        let work_dir = format!("{}/gemini-{}", self.config.work_dir, &session_id[..8]);

        // Ensure workspace exists
        std::fs::create_dir_all(&self.config.work_dir)
            .map_err(|e| format!("Failed to create work dir: {}", e))?;

        // Step 1: Clone repository
        tracing::info!("📦 [GeminiCLI] Cloning {} for session {}", repo_url, &session_id[..8]);
        runner::git_clone(
            repo_url,
            &work_dir,
            &self.config.github_token,
            Duration::from_secs(120),
        )
        .await?;

        // Step 2: Build prompt
        let prompt = build_fix_prompt(issue_title, issue_body, affected_files);

        // Step 3: Run Gemini CLI in non-interactive mode
        tracing::info!("🤖 [GeminiCLI] Running agent for: {}", issue_title);
        let timeout = Duration::from_secs(self.config.timeout_secs);

        let mut env_vars: Vec<(&str, &str)> = vec![];
        if !self.config.gemini_api_key.is_empty() {
            env_vars.push(("GEMINI_API_KEY", &self.config.gemini_api_key));
        }

        // Gemini CLI uses -p flag for prompt in non-interactive mode
        let output = runner::run_agent_command(
            &self.config.gemini_cli_path,
            &[
                "-p",
                &prompt,
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
                return Err(format!("[GeminiCLI] Agent execution failed: {}", e));
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

        if result.success {
            tracing::info!(
                "✅ [GeminiCLI] Fix applied: {} files changed",
                result.files_changed.len()
            );
        } else {
            tracing::warn!("⚠️ [GeminiCLI] No files changed or agent failed");
            runner::cleanup_workspace(&work_dir);
        }

        Ok(result)
    }

    /// Get the workspace directory for a running session
    pub fn get_work_dir(&self, session_id: &str) -> String {
        format!("{}/gemini-{}", self.config.work_dir, &session_id[..8.min(session_id.len())])
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_agent::CodeAgentProvider;

    fn test_config() -> CodeAgentConfig {
        CodeAgentConfig {
            provider: CodeAgentProvider::GeminiCli,
            work_dir: "/tmp/muninn-test".to_string(),
            timeout_secs: 300,
            gemini_api_key: "test-api-key".to_string(),
            opencode_path: "opencode".to_string(),
            gemini_cli_path: "gemini".to_string(),
            github_token: String::new(),
        }
    }

    #[test]
    fn test_gemini_agent_creation() {
        let config = test_config();
        let agent = GeminiCliAgent::new(config);
        assert_eq!(agent.config.provider, CodeAgentProvider::GeminiCli);
        assert_eq!(agent.config.gemini_cli_path, "gemini");
        assert_eq!(agent.config.gemini_api_key, "test-api-key");
    }

    #[test]
    fn test_prompt_generation_for_gemini() {
        let prompt = build_fix_prompt(
            "CSRF vulnerability",
            "Missing CSRF token validation in form submission",
            &["src/middleware/csrf.rs".to_string()],
        );
        assert!(prompt.contains("CSRF vulnerability"));
        assert!(prompt.contains("src/middleware/csrf.rs"));
        assert!(prompt.contains("MINIMAL changes"));
    }

    #[test]
    fn test_work_dir_generation() {
        let config = test_config();
        let agent = GeminiCliAgent::new(config);
        let work_dir = agent.get_work_dir("12345678-abcd-efgh-ijkl-mnopqrstuvwx");
        assert!(work_dir.starts_with("/tmp/muninn-test/gemini-"));
        assert!(work_dir.contains("12345678"));
    }

    #[test]
    fn test_gemini_api_key_passthrough() {
        let config = test_config();
        // Verify the API key is available in config for env injection
        assert_eq!(config.gemini_api_key, "test-api-key");
    }
}
