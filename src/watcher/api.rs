use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::AppState;
use crate::code_agent::{self, CodeAgentConfig, CodeAgentProvider};
use crate::models::AnalysisStatus;

#[derive(Deserialize)]
pub struct ListParams {
    pub status: Option<String>,
}

/// GET /api/issues — list tracked issues
pub async fn list_issues(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    match state.db.list_issues(params.status.as_deref()) {
        Ok(issues) => (StatusCode::OK, Json(serde_json::to_value(issues).unwrap())).into_response(),
        Err(e) => {
            tracing::error!("DB error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"}))).into_response()
        }
    }
}

/// GET /api/issues/:id — get issue details
pub async fn get_issue(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_issue(&id) {
        Ok(Some(issue)) => (StatusCode::OK, Json(serde_json::to_value(issue).unwrap())).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Issue not found"}))).into_response(),
        Err(e) => {
            tracing::error!("DB error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"}))).into_response()
        }
    }
}

/// GET /api/stats — summary stats
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_stats() {
        Ok((issues, fixes)) => Json(serde_json::json!({
            "total_issues": issues,
            "total_fixes": fixes,
            "watched_repos": state.config.watched_repos.len(),
            "code_agent_provider": state.config.code_agent_provider,
        })).into_response(),
        Err(e) => {
            tracing::error!("DB error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"}))).into_response()
        }
    }
}

/// POST /api/issues/:id/fix — trigger code agent fix manually
pub async fn trigger_fix(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Get the issue
    let issue = match state.db.get_issue(&id) {
        Ok(Some(issue)) => issue,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Issue not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Database error: {}", e)})),
            )
                .into_response()
        }
    };

    // Check code agent provider
    let provider = CodeAgentProvider::from_str(&state.config.code_agent_provider);
    if provider == CodeAgentProvider::None {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No code agent configured. Set CODE_AGENT_PROVIDER env variable."
            })),
        )
            .into_response();
    }

    // Build agent config
    let agent_config = CodeAgentConfig {
        provider: provider.clone(),
        work_dir: state.config.code_agent_work_dir.clone(),
        timeout_secs: state.config.code_agent_timeout_secs,
        gemini_api_key: state.config.gemini_api_key.clone(),
        opencode_path: state.config.opencode_path.clone(),
        gemini_cli_path: state.config.gemini_cli_path.clone(),
        github_token: state.config.github_token.clone(),
    };

    let repo_url = format!("https://github.com/{}", issue.repo);

    // Update status
    let _ = state.db.update_issue_status(&id, &AnalysisStatus::Fixing);

    // Run code agent (spawn as background task)
    let state_clone = state.clone();
    let issue_id = id.clone();
    tokio::spawn(async move {
        let result = code_agent::run_code_agent_fix(
            &agent_config,
            &repo_url,
            &issue.title,
            &issue.body,
            &issue.labels, // use labels as hint for affected areas
        )
        .await;

        match result {
            Ok(r) if r.success => {
                tracing::info!("🤖 Manual fix completed: {} files changed", r.files_changed.len());
                let _ = state_clone
                    .db
                    .update_issue_status(&issue_id, &AnalysisStatus::Fixed);
            }
            Ok(_) => {
                tracing::warn!("⚠️ Code agent ran but no changes produced");
                let _ = state_clone
                    .db
                    .update_issue_status(&issue_id, &AnalysisStatus::Analyzed);
            }
            Err(e) => {
                tracing::error!("❌ Code agent error: {}", e);
                let _ = state_clone
                    .db
                    .update_issue_status(&issue_id, &AnalysisStatus::Failed);
            }
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Fix triggered",
            "issue_id": id,
            "provider": provider.to_string(),
        })),
    )
        .into_response()
}

/// POST /api/issues/:id/approve — approve a review_pending fix
pub async fn approve_fix(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let issue = match state.db.get_issue(&id) {
        Ok(Some(issue)) => issue,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Issue not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("{}", e)}))).into_response(),
    };

    if issue.status != AnalysisStatus::ReviewPending {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Issue is not in review_pending status",
            "current_status": issue.status.to_string(),
        }))).into_response();
    }

    let _ = state.db.update_issue_status(&id, &AnalysisStatus::Fixing);
    tracing::info!("✅ Fix approved for {}#{}: {}", issue.repo, issue.issue_number, issue.title);

    (StatusCode::OK, Json(serde_json::json!({
        "message": "Fix approved",
        "issue_id": id,
        "status": "fixing",
    }))).into_response()
}

/// POST /api/issues/:id/reject — reject a proposed fix
pub async fn reject_fix(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let issue = match state.db.get_issue(&id) {
        Ok(Some(issue)) => issue,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Issue not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("{}", e)}))).into_response(),
    };

    if issue.status != AnalysisStatus::ReviewPending {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Issue is not in review_pending status",
            "current_status": issue.status.to_string(),
        }))).into_response();
    }

    let _ = state.db.update_issue_status(&id, &AnalysisStatus::Skipped);
    tracing::info!("❌ Fix rejected for {}#{}: {}", issue.repo, issue.issue_number, issue.title);

    (StatusCode::OK, Json(serde_json::json!({
        "message": "Fix rejected",
        "issue_id": id,
        "status": "skipped",
    }))).into_response()
}

/// GET /api/config — return current config (non-sensitive)
pub async fn get_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "fix_mode": state.config.fix_mode,
        "llm_provider": state.config.llm_provider,
        "llm_model": state.config.llm_model,
        "gemini_model": state.config.gemini_model,
        "llm_temperature": state.config.llm_temperature,
        "llm_max_tokens": state.config.llm_max_tokens,
        "code_agent_provider": state.config.code_agent_provider,
        "poll_interval_secs": state.config.poll_interval_secs,
        "watched_repos": state.config.watched_repos,
    }))
}
