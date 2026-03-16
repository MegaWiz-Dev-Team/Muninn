pub mod api;
pub mod poller;

use std::sync::Arc;
use crate::AppState;

/// Start the background GitHub issue watcher
pub async fn start_watcher(state: Arc<AppState>) {
    let interval = state.config.poll_interval_secs;
    let repos = state.config.watched_repos.clone();

    if repos.is_empty() {
        tracing::warn!("🐦 No repos configured (WATCHED_REPOS is empty). Watcher idle.");
        return;
    }

    if state.config.github_token.is_empty() {
        tracing::warn!("🐦 No GITHUB_TOKEN set. Watcher disabled.");
        return;
    }

    tracing::info!("🐦 Watcher started — polling {} repos every {}s", repos.len(), interval);

    loop {
        for repo in &repos {
            if let Err(e) = poller::poll_repo(&state, repo).await {
                tracing::error!("❌ Failed to poll {}: {}", repo, e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
    }
}
