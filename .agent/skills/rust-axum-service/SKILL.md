---
name: rust-axum-service
description: How to build Rust/Axum services consistent with the Asgard ecosystem patterns (Mimir, Eir, Várðr, Heimdall Gateway)
---

# Rust/Axum Service Development

## When to Use This Skill

- Building or modifying Muninn (Issue Watcher / Auto-Fixer)
- Creating new Axum routes, handlers, or middleware
- Adding database access (rusqlite) or HTTP clients (reqwest)

## Asgard Conventions

### Project Structure
```
src/
├── main.rs              # Axum server setup + routes
├── config.rs            # Environment config (from_env pattern)
├── db.rs                # SQLite via rusqlite
├── health.rs            # GET /health endpoint
└── routes/              # Feature-specific route modules
```

### Standard Crates
- `axum = "0.8"` — Web framework
- `tokio = { version = "1", features = ["full"] }` — Async runtime
- `serde = { version = "1", features = ["derive"] }` — Serialization
- `reqwest = { version = "0.12", features = ["json", "stream"] }` — HTTP client
- `rusqlite = { version = "0.32", features = ["bundled"] }` — SQLite
- `octocrab = "0.41"` — GitHub API
- `tower-http = { version = "0.6", features = ["cors", "trace"] }` — Middleware
- `tracing` + `tracing-subscriber` — Logging

### Health Endpoint Pattern
Every Asgard service MUST expose `GET /health` returning:
```json
{
  "status": "ok",
  "service": "muninn",
  "version": "0.1.0",
  "repos_watched": 10,
  "issues_analyzed": 45,
  "fixes_proposed": 8,
  "fixes_merged": 3
}
```

### Config Pattern
```rust
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub huginn_url: String,
    pub github_token: String,
    // ...
}

impl AppConfig {
    pub fn from_env() -> Self { /* load from env vars */ }
}
```

### Docker
- Multi-stage build: `rust:1.85-slim` builder + `debian:bookworm-slim` runtime
- Memory limit: `64M` (reserve: `16M`)
- Container name: `asgard_muninn`
- Port: `8500`
- Network: `asgard`

### Testing
- Unit tests: `cargo test`
- Lint: `cargo clippy`
- Format: `cargo fmt`
