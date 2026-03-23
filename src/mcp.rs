//! MCP (Model Context Protocol) Adapter for Muninn
//! Uses JSON-RPC 2.0 to expose Auto-Fixer operations natively.

use std::sync::Arc;
use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

// ─── Error Codes (JSON-RPC 2.0) ─────────────────────────────────────────

pub const CODE_PARSE_ERROR: i32 = -32700;
pub const CODE_INVALID_REQUEST: i32 = -32600;
pub const CODE_METHOD_NOT_FOUND: i32 = -32601;
pub const CODE_INVALID_PARAMS: i32 = -32602;
pub const CODE_INTERNAL_ERROR: i32 = -32603;

// ─── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub id: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    pub id: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Map<String, serde_json::Value>,
}

impl Response {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
            id,
        }
    }
}

// ─── MCP Handlers ────────────────────────────────────────────────────────

pub async fn rpc_handler(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
    body: String,
) -> Json<Response> {
    let req: Request = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(_) => return Json(Response::error(serde_json::Value::Null, CODE_PARSE_ERROR, "Parse error: invalid JSON")),
    };

    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => Json(Response::success(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "muninn-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": { "tools": {} }
            }),
        )),

        "tools/list" => Json(Response::success(
            id,
            serde_json::json!({
                "tools": [
                    {
                        "name": "list_open_issues",
                        "description": "List all tracked GitHub issues waiting for auto-fix or review",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "status": { "type": "string", "description": "Filter by status (e.g. open, review_pending, fixed, failed)" }
                            }
                        }
                    },
                    {
                        "name": "get_issue_details",
                        "description": "Get detailed tracking information, analysis, and fix patch for an issue",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "issue_id": { "type": "string", "description": "Muninn internal issue ID" }
                            },
                            "required": ["issue_id"]
                        }
                    },
                    {
                        "name": "trigger_auto_fix",
                        "description": "Trigger the AI code agent to generate an automated fix for an issue",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "issue_id": { "type": "string", "description": "Muninn internal issue ID" }
                            },
                            "required": ["issue_id"]
                        }
                    },
                    {
                        "name": "approve_fix",
                        "description": "Approve a pending auto-fix patch to be committed",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "issue_id": { "type": "string", "description": "Muninn internal issue ID" }
                            },
                            "required": ["issue_id"]
                        }
                    },
                    {
                        "name": "reject_fix",
                        "description": "Reject a pending auto-fix patch",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "issue_id": { "type": "string", "description": "Muninn internal issue ID" }
                            },
                            "required": ["issue_id"]
                        }
                    }
                ]
            }),
        )),

        "tools/call" => {
            let params: ToolCallParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Json(Response::error(id, CODE_INVALID_PARAMS, format!("Invalid params: {}", e))),
            };

            let port = state.config.port;
            let base_url = format!("http://127.0.0.1:{}", port);
            let client = reqwest::Client::new();
            
            // Loopback dispatch helper
            let dispatch = |method: reqwest::Method, url: String| async move {
                let handle = tokio::spawn(async move {
                    let req = client.request(method, &url).send().await;
                    match req {
                        Ok(r) => {
                            let status = r.status();
                            let body = r.text().await.unwrap_or_default();
                            if status.is_success() {
                                Ok(body)
                            } else {
                                Err(format!("HTTP {} - {}", status, body))
                            }
                        },
                        Err(e) => Err(format!("Failed to dispatch: {}", e)),
                    }
                });

                match handle.await {
                    Ok(Ok(output)) => output,
                    Ok(Err(e)) => format!("Error: {}", e),
                    Err(e) => format!("Task Error: {}", e)
                }
            };

            let text = match params.name.as_str() {
                "list_open_issues" => {
                    let mut url = format!("{}/api/issues", base_url);
                    if let Some(status) = params.arguments.get("status").and_then(|v| v.as_str()) {
                        url = format!("{}?status={}", url, status);
                    }
                    dispatch(reqwest::Method::GET, url).await
                }
                "get_issue_details" => {
                    let issue_id = match params.arguments.get("issue_id").and_then(|v| v.as_str()) {
                        Some(s) => s,
                        None => return Json(Response::error(id, CODE_INVALID_PARAMS, "Missing issue_id")),
                    };
                    let url = format!("{}/api/issues/{}", base_url, issue_id);
                    dispatch(reqwest::Method::GET, url).await
                }
                "trigger_auto_fix" => {
                    let issue_id = match params.arguments.get("issue_id").and_then(|v| v.as_str()) {
                        Some(s) => s,
                        None => return Json(Response::error(id, CODE_INVALID_PARAMS, "Missing issue_id")),
                    };
                    let url = format!("{}/api/issues/{}/fix", base_url, issue_id);
                    dispatch(reqwest::Method::POST, url).await
                }
                "approve_fix" => {
                    let issue_id = match params.arguments.get("issue_id").and_then(|v| v.as_str()) {
                        Some(s) => s,
                        None => return Json(Response::error(id, CODE_INVALID_PARAMS, "Missing issue_id")),
                    };
                    let url = format!("{}/api/issues/{}/approve", base_url, issue_id);
                    dispatch(reqwest::Method::POST, url).await
                }
                "reject_fix" => {
                    let issue_id = match params.arguments.get("issue_id").and_then(|v| v.as_str()) {
                        Some(s) => s,
                        None => return Json(Response::error(id, CODE_INVALID_PARAMS, "Missing issue_id")),
                    };
                    let url = format!("{}/api/issues/{}/reject", base_url, issue_id);
                    dispatch(reqwest::Method::POST, url).await
                }
                _ => return Json(Response::error(id, CODE_METHOD_NOT_FOUND, format!("tool not found: {}", params.name))),
            };

            Json(Response::success(
                id,
                serde_json::json!({
                    "content": [{ "type": "text", "text": text }]
                }),
            ))
        }

        _ => Json(Response::error(id, CODE_METHOD_NOT_FOUND, format!("method not found: {}", req.method))),
    }
}
