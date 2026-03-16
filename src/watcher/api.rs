use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::AppState;

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
        })).into_response(),
        Err(e) => {
            tracing::error!("DB error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"}))).into_response()
        }
    }
}
