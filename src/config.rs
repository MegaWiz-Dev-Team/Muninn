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
        }
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
}
