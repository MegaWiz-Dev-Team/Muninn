/// Muninn configuration from environment variables
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub database_path: String,
    pub github_token: String,
    pub watched_repos: Vec<String>,
    pub poll_interval_secs: u64,
    pub heimdall_url: String,
    pub gemini_api_key: String,
    // Code Agent settings
    pub code_agent_provider: String,
    pub code_agent_work_dir: String,
    pub code_agent_timeout_secs: u64,
    pub opencode_path: String,
    pub gemini_cli_path: String,
    // Fix mode: "review" or "auto"
    pub fix_mode: String,
    // LLM settings
    pub llm_provider: String,
    pub llm_model: String,
    pub gemini_model: String,
    pub llm_temperature: f32,
    pub llm_max_tokens: u32,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let repos_str = std::env::var("WATCHED_REPOS").unwrap_or_default();
        let watched_repos: Vec<String> = if repos_str.is_empty() {
            vec![]
        } else {
            repos_str.split(',').map(|s| s.trim().to_string()).collect()
        };

        Self {
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8500".to_string())
                .parse()
                .unwrap_or(8500),
            database_path: std::env::var("DATABASE_PATH")
                .unwrap_or_else(|_| "muninn.db".to_string()),
            github_token: std::env::var("GITHUB_TOKEN")
                .unwrap_or_default(),
            watched_repos,
            poll_interval_secs: std::env::var("POLL_INTERVAL_SECS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300),
            heimdall_url: std::env::var("HEIMDALL_URL")
                .unwrap_or_else(|_| "http://host.docker.internal:8080".to_string()),
            gemini_api_key: std::env::var("GEMINI_API_KEY")
                .unwrap_or_default(),
            code_agent_provider: std::env::var("CODE_AGENT_PROVIDER")
                .unwrap_or_else(|_| "none".to_string()),
            code_agent_work_dir: std::env::var("CODE_AGENT_WORK_DIR")
                .unwrap_or_else(|_| "/tmp/muninn-workspace".to_string()),
            code_agent_timeout_secs: std::env::var("CODE_AGENT_TIMEOUT")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300),
            opencode_path: std::env::var("OPENCODE_PATH")
                .unwrap_or_else(|_| "opencode".to_string()),
            gemini_cli_path: std::env::var("GEMINI_CLI_PATH")
                .unwrap_or_else(|_| "gemini".to_string()),
            fix_mode: std::env::var("FIX_MODE")
                .unwrap_or_else(|_| "review".to_string()),
            llm_provider: std::env::var("LLM_PROVIDER")
                .unwrap_or_else(|_| "both".to_string()),
            llm_model: std::env::var("LLM_MODEL")
                .unwrap_or_else(|_| "default".to_string()),
            gemini_model: std::env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-2.5-flash".to_string()),
            llm_temperature: std::env::var("LLM_TEMPERATURE")
                .unwrap_or_else(|_| "0.1".to_string())
                .parse()
                .unwrap_or(0.1),
            llm_max_tokens: std::env::var("LLM_MAX_TOKENS")
                .unwrap_or_else(|_| "8192".to_string())
                .parse()
                .unwrap_or(8192),
        }
    }

    pub fn is_review_mode(&self) -> bool {
        self.fix_mode.to_lowercase() == "review"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::from_env();
        assert_eq!(config.port, 8500);
        assert_eq!(config.poll_interval_secs, 300);
        assert!(config.watched_repos.is_empty());
    }

    #[test]
    fn test_code_agent_config_defaults() {
        let config = AppConfig::from_env();
        assert_eq!(config.code_agent_provider, "none");
        assert_eq!(config.code_agent_work_dir, "/tmp/muninn-workspace");
        assert_eq!(config.code_agent_timeout_secs, 300);
        assert_eq!(config.opencode_path, "opencode");
        assert_eq!(config.gemini_cli_path, "gemini");
    }

    // ── TDD: Review Mode config ──

    #[test]
    fn test_fix_mode_default_is_review() {
        let config = AppConfig::from_env();
        assert_eq!(config.fix_mode, "review");
    }

    #[test]
    fn test_is_review_mode_true() {
        let mut config = AppConfig::from_env();
        config.fix_mode = "review".to_string();
        assert!(config.is_review_mode());
    }

    #[test]
    fn test_is_review_mode_false() {
        let mut config = AppConfig::from_env();
        config.fix_mode = "auto".to_string();
        assert!(!config.is_review_mode());
    }

    #[test]
    fn test_is_review_mode_case_insensitive() {
        let mut config = AppConfig::from_env();
        config.fix_mode = "REVIEW".to_string();
        assert!(config.is_review_mode());
    }

    // ── TDD: LLM config ──

    #[test]
    fn test_llm_provider_default_is_both() {
        let config = AppConfig::from_env();
        assert_eq!(config.llm_provider, "both");
    }

    #[test]
    fn test_llm_model_default() {
        let config = AppConfig::from_env();
        assert_eq!(config.llm_model, "default");
    }

    #[test]
    fn test_gemini_model_default_is_2_5_flash() {
        let config = AppConfig::from_env();
        assert_eq!(config.gemini_model, "gemini-2.5-flash");
    }

    #[test]
    fn test_llm_temperature_default_optimized() {
        let config = AppConfig::from_env();
        assert!((config.llm_temperature - 0.1).abs() < f32::EPSILON,
            "Temperature should be 0.1 for deterministic security fixes");
    }

    #[test]
    fn test_llm_max_tokens_default_optimized() {
        let config = AppConfig::from_env();
        assert_eq!(config.llm_max_tokens, 8192,
            "Max tokens should be 8192 for security analysis context");
    }
}
