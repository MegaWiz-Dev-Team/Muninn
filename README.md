# 🐦 Muninn — Issue Watcher & Multi-Agent Auto-Fixer

> **Muninn** (มูนิน) — One of Odin's ravens, representing memory and wisdom.
> An AI-powered service that watches GitHub issues, analyzes vulnerabilities, and auto-generates fix PRs.

[![Tests](https://img.shields.io/badge/tests-60%2F60-brightgreen)]()
[![Rust](https://img.shields.io/badge/rust-2021-orange)]()
[![ISO](https://img.shields.io/badge/ISO-29110-blue)]()
[![License](https://img.shields.io/badge/license-MIT-green)]()

---

## 🏗️ Architecture

```
                    ┌─────────────────────────┐
                    │   GitHub Issues          │
                    │  ├─ huginn-finding       │
                    │  ├─ security             │
                    │  └─ auto-fix             │
                    └───────────┬──────────────┘
                                │
                    ┌───────────▼──────────────┐
                    │  Muninn Watcher (5 min)   │
                    │  1. Poll GitHub Issues    │
                    │  2. Filter by labels      │
                    │  3. Track in SQLite       │
                    └───────────┬──────────────┘
                                │
                    ┌───────────▼──────────────┐
                    │  Fix Pipeline             │
                    │  1. LLM Analyze Issue     │
                    │  2. Code Agent Fix        │
                    │     ├── OpenCode CLI      │
                    │     └── Gemini CLI        │
                    │  3. LLM Fix (fallback)    │
                    │  4. Create Draft PR       │
                    └───────────┬──────────────┘
                                │
                    ┌───────────▼──────────────┐
                    │  ⚖️ Forseti E2E Test      │
                    │  Validate fix PR          │
                    └──────────────────────────┘
```

---

## 🚀 Quick Start

### 1. Build & Run

```bash
cargo build --release
cargo run
# 🐦 Muninn starting on 0.0.0.0:8500
```

### 2. Configure

```bash
export GITHUB_TOKEN="ghp_xxx"
export WATCHED_REPOS="MegaWiz-Dev-Team/Mimir,MegaWiz-Dev-Team/Bifrost"
export GEMINI_API_KEY="xxx"

# Optional: Code Agent CLI
export CODE_AGENT_PROVIDER="gemini_cli"  # or "opencode"
export GEMINI_CLI_PATH="gemini"
```

### 3. Run Tests

```bash
cargo test
# ✅ 60/60 passed
```

---

## ✨ Key Features

### Issue Watching
- **GitHub polling** — Auto-polls repos every 5 min (configurable)
- **Label filtering** — Watches `huginn-finding`, `security`, `vulnerability`, `auto-fix`
- **Priority derivation** — Critical > High > Medium > Low from labels
- **Skip control** — `muninn-skip` label to exclude issues

### AI Auto-Fix
- **LLM Analysis** — Heimdall (local) + Gemini API (fallback) for root cause analysis
- **Code Agent CLI** — OpenCode CLI or Gemini CLI for interactive code fixing
- **LLM Fix Fallback** — Direct code generation if CLI agent fails
- **Draft PRs** — All fixes as draft PRs, never auto-merge

### Code Agent Integration (New)
- **OpenCode CLI** — Go-based AI coding agent (`opencode-ai/opencode`)
- **Gemini CLI** — Google's AI code agent (`gemini`)
- **Non-interactive mode** — Runs as subprocess with timeout
- **Auto-fallback** — Falls back to LLM API if agent fails

### Safety
- ⛔ Max **3 files** per PR — larger fixes create issues instead
- 📝 All PRs prefixed `[Muninn Auto-Fix]`
- ⏱️ Agent timeout: 300s (configurable)
- 🗂️ Isolated workspace: `/tmp/muninn-workspace/`

---

## 📡 API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/healthz` | Health check (k8s) |
| `GET` | `/api/issues` | List issues (`?status=pending`) |
| `GET` | `/api/issues/{id}` | Get issue details |
| `POST` | `/api/issues/{id}/fix` | Trigger code agent fix |
| `GET` | `/api/stats` | Summary statistics |

---

## 🏰 Asgard Ecosystem

Muninn works with its Asgard siblings:

| Flow | Description |
|------|-------------|
| 🦅 Huginn → 🐦 Muninn | Huginn scans → creates `huginn-finding` issue → Muninn auto-fixes |
| ⚖️ Forseti → 🐦 Muninn | Forseti test failure → creates `auto-fix` issue → Muninn fixes |
| 🐦 Muninn → ⚖️ Forseti | Muninn creates fix PR → Forseti runs E2E validation |

---

## 📁 Project Structure

```
muninn/
├── src/
│   ├── main.rs              # Server setup + routing
│   ├── config.rs             # Env config (12 variables)
│   ├── db.rs                 # SQLite (rusqlite)
│   ├── models.rs             # TrackedIssue, labels, priority
│   ├── health.rs             # Health check endpoint
│   ├── llm.rs                # LLM client (Heimdall + Gemini)
│   ├── code_agent/           # 🆕 Code Agent CLI
│   │   ├── mod.rs            # Provider enum, dispatch, config
│   │   ├── runner.rs         # Subprocess runner, git helpers
│   │   ├── opencode.rs       # OpenCode CLI wrapper
│   │   └── gemini_cli.rs     # Gemini CLI wrapper
│   ├── fixer/
│   │   ├── mod.rs            # Fix pipeline (agent + LLM)
│   │   ├── analyzer.rs       # LLM issue analysis
│   │   └── pr_creator.rs     # GitHub PR creation
│   └── watcher/
│       ├── mod.rs            # Background poller
│       ├── poller.rs         # GitHub issue fetcher
│       └── api.rs            # REST API handlers
├── docs/iso_29110/           # ISO 29110 documentation
├── Cargo.toml
├── Dockerfile
└── README.md
```

---

## 📊 Test Results

### Unit Tests: 60/60 PASS ✅

| Module | Tests |
|--------|:-----:|
| code_agent (mod, runner, opencode, gemini_cli) | 20 |
| config | 2 |
| fixer (analyzer, pr_creator) | 7 |
| llm | 4 |
| models | 11 |
| watcher (poller) | 3 |
| db | 8 |
| health | 1 |
| **Total** | **60** |

---

## ⚙️ Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8500` | Server port |
| `DATABASE_PATH` | `muninn.db` | SQLite path |
| `GITHUB_TOKEN` | — | GitHub API token |
| `WATCHED_REPOS` | — | Comma-separated repos |
| `POLL_INTERVAL_SECS` | `300` | Poll interval |
| `HEIMDALL_URL` | `http://host.docker.internal:8080` | Local LLM |
| `GEMINI_API_KEY` | — | Gemini API key |
| `CODE_AGENT_PROVIDER` | `none` | `opencode` / `gemini_cli` / `none` |
| `CODE_AGENT_WORK_DIR` | `/tmp/muninn-workspace` | Agent workspace |
| `CODE_AGENT_TIMEOUT` | `300` | Agent timeout (secs) |
| `OPENCODE_PATH` | `opencode` | OpenCode CLI binary path |
| `GEMINI_CLI_PATH` | `gemini` | Gemini CLI binary path |

---

## 📋 ISO 29110 Documentation

| Document | Description |
|----------|-------------|
| `SI-01_Implementation_Report.md` | Architecture, FR traceability, sprint history |

---

*🐦 Built by MegaWiz Dev Team — Part of the Asgard AI Platform*
