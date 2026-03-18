pub mod gemini_cli;
pub mod opencode;
pub mod runner;

use serde::{Deserialize, Serialize};

/// Supported code agent CLI providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CodeAgentProvider {
    OpenCode,
    GeminiCli,
    None,
}

impl CodeAgentProvider {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "opencode" | "open_code" => Self::OpenCode,
            "gemini_cli" | "gemini-cli" | "gemini" => Self::GeminiCli,
            _ => Self::None,
        }
    }
}

impl std::fmt::Display for CodeAgentProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenCode => write!(f, "opencode"),
            Self::GeminiCli => write!(f, "gemini_cli"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Configuration for code agent execution
#[derive(Debug, Clone)]
pub struct CodeAgentConfig {
    pub provider: CodeAgentProvider,
    pub work_dir: String,
    pub timeout_secs: u64,
    pub gemini_api_key: String,
    pub opencode_path: String,
    pub gemini_cli_path: String,
    pub github_token: String,
}

/// Result of a code agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAgentResult {
    pub success: bool,
    pub files_changed: Vec<String>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run the configured code agent to fix an issue
pub async fn run_code_agent_fix(
    config: &CodeAgentConfig,
    repo_url: &str,
    issue_title: &str,
    issue_body: &str,
    affected_files: &[String],
) -> Result<CodeAgentResult, String> {
    match config.provider {
        CodeAgentProvider::OpenCode => {
            let agent = opencode::OpenCodeAgent::new(config.clone());
            agent.fix_issue(repo_url, issue_title, issue_body, affected_files).await
        }
        CodeAgentProvider::GeminiCli => {
            let agent = gemini_cli::GeminiCliAgent::new(config.clone());
            agent.fix_issue(repo_url, issue_title, issue_body, affected_files).await
        }
        CodeAgentProvider::None => {
            Err("No code agent provider configured".to_string())
        }
    }
}

/// Build an agent prompt for the issue
pub fn build_fix_prompt(
    issue_title: &str,
    issue_body: &str,
    affected_files: &[String],
) -> String {
    let files_list = if affected_files.is_empty() {
        "No specific files identified.".to_string()
    } else {
        affected_files.iter()
            .map(|f| format!("- {}", f))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"You are Muninn, an automated code fixer. Fix the following issue with MINIMAL changes.

## Issue
**Title:** {issue_title}

**Description:**
{issue_body}

## Affected Files
{files_list}

## Rules
1. Make MINIMAL changes — touch as few lines as possible
2. Do NOT break existing tests
3. Run tests after fixing to verify
4. Maximum 3 files changed
5. Follow existing code style and conventions
6. Add inline comments explaining critical fixes
"#
    )
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_string() {
        assert_eq!(CodeAgentProvider::from_str("opencode"), CodeAgentProvider::OpenCode);
        assert_eq!(CodeAgentProvider::from_str("open_code"), CodeAgentProvider::OpenCode);
        assert_eq!(CodeAgentProvider::from_str("gemini_cli"), CodeAgentProvider::GeminiCli);
        assert_eq!(CodeAgentProvider::from_str("gemini-cli"), CodeAgentProvider::GeminiCli);
        assert_eq!(CodeAgentProvider::from_str("gemini"), CodeAgentProvider::GeminiCli);
        assert_eq!(CodeAgentProvider::from_str("none"), CodeAgentProvider::None);
        assert_eq!(CodeAgentProvider::from_str(""), CodeAgentProvider::None);
        assert_eq!(CodeAgentProvider::from_str("unknown"), CodeAgentProvider::None);
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(CodeAgentProvider::OpenCode.to_string(), "opencode");
        assert_eq!(CodeAgentProvider::GeminiCli.to_string(), "gemini_cli");
        assert_eq!(CodeAgentProvider::None.to_string(), "none");
    }

    #[test]
    fn test_provider_roundtrip() {
        let providers = ["opencode", "gemini_cli", "none"];
        for p in providers {
            let parsed = CodeAgentProvider::from_str(p);
            assert_eq!(parsed.to_string(), p);
        }
    }

    #[test]
    fn test_build_fix_prompt_with_files() {
        let prompt = build_fix_prompt(
            "SQL Injection in /api/login",
            "Found SQL injection vulnerability",
            &["src/db.rs".to_string(), "src/auth.rs".to_string()],
        );
        assert!(prompt.contains("SQL Injection in /api/login"));
        assert!(prompt.contains("SQL injection vulnerability"));
        assert!(prompt.contains("- src/db.rs"));
        assert!(prompt.contains("- src/auth.rs"));
        assert!(prompt.contains("MINIMAL changes"));
    }

    #[test]
    fn test_build_fix_prompt_no_files() {
        let prompt = build_fix_prompt(
            "Test issue",
            "Test body",
            &[],
        );
        assert!(prompt.contains("No specific files identified"));
    }

    #[test]
    fn test_agent_config_defaults() {
        let config = CodeAgentConfig {
            provider: CodeAgentProvider::None,
            work_dir: "/tmp/muninn-workspace".to_string(),
            timeout_secs: 300,
            gemini_api_key: String::new(),
            opencode_path: "opencode".to_string(),
            gemini_cli_path: "gemini".to_string(),
            github_token: String::new(),
        };
        assert_eq!(config.provider, CodeAgentProvider::None);
        assert_eq!(config.timeout_secs, 300);
        assert_eq!(config.opencode_path, "opencode");
        assert_eq!(config.gemini_cli_path, "gemini");
    }

    #[test]
    fn test_code_agent_result_serialization() {
        let result = CodeAgentResult {
            success: true,
            files_changed: vec!["src/db.rs".to_string()],
            stdout: "Fix applied".to_string(),
            stderr: String::new(),
            exit_code: 0,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("src/db.rs"));

        let parsed: CodeAgentResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.files_changed.len(), 1);
    }
}
