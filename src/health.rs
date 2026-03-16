use std::sync::Arc;
use axum::{Json, extract::State};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub watched_repos: usize,
    pub issues_tracked: u64,
    pub fixes_created: u64,
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn uptime() -> u64 {
    START_TIME
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs()
}

pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Json<HealthResponse> {
    let (issues, fixes) = state.db.get_stats().unwrap_or((0, 0));

    Json(HealthResponse {
        status: "ok",
        service: "muninn",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime(),
        watched_repos: state.config.watched_repos.len(),
        issues_tracked: issues,
        fixes_created: fixes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uptime() {
        let t1 = uptime();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = uptime();
        assert!(t2 >= t1);
    }
}
