# PM-01: Project Plan — Muninn
**Project Name:** 🐦 Muninn (Issue Watcher + Multi-Agent Auto-Fixer)
**Document Version:** 1.0
**Date:** 2026-03-16
**Standard:** ISO/IEC 29110 — PM Process
**Parent:** [Asgard PM-01](../../../Asgard/docs/iso_29110/pm/PM_01_Project_Plan.md)

---

## 1. Project Scope & Objectives

### เป้าหมาย
พัฒนา Automated Issue Watcher + AI Auto-Fix Pipeline ที่สามารถ:
- monitor GitHub Issues (poll + label filter)
- วิเคราะห์ root cause ด้วย AI (LLM)
- สร้าง code fix + PR อัตโนมัติ
- Multi-agent review pipeline (Analyzer → Coder → Reviewer → Tester)

### ขอบเขต

| Feature | Sprint | Priority |
|:--|:--|:--|
| Scaffold + Health + GitHub Poller | S1 | P0 |
| AI Analyzer + Auto-Fix + PR Creator | S2 | P0 |
| Multi-Agent Fix Pipeline (4 agents) | S3 | P1 |
| Continuous Learning + Trend Analysis | S4 | P2 |

---

## 2. Project Organization

| Role | Person/Team | Responsibility |
|:--|:--|:--|
| **Product Owner** | Paripol (MegaWiz) | Architecture, Rust backend |
| **Developer** | AI-assisted (Antigravity) | Implementation, testing |

---

## 3. Technical Architecture

| Layer | Technology |
|:--|:--|
| Language | Rust 2024 edition |
| Web Framework | Axum 0.8 |
| Database | rusqlite (SQLite) |
| HTTP Client | reqwest 0.12 |
| GitHub API | octocrab 0.41 |
| Port | `:8500` |
| Container | `asgard_muninn` |

### Source Structure

```
src/
├── main.rs          — Axum server + routes
├── config.rs        — Configuration management
├── db.rs            — SQLite schema + queries
├── health.rs        — Health + readiness endpoints
├── models.rs        — Data models (Issue, FixResult, AnalysisReport)
├── llm.rs           — LLM client (Heimdall/Gemini)
├── watcher/
│   ├── mod.rs       — Watcher module
│   ├── poller.rs    — GitHub issue polling (configurable interval)
│   └── api.rs       — REST API watcher endpoints
└── fixer/
    ├── mod.rs       — Fixer module
    ├── analyzer.rs  — AI issue analysis (root cause, CWE)
    └── pr_creator.rs — Branch + PR creation
```

---

- **Sprint 31: Mimir Hybrid Search & MCP Server Foundation** [Planned]
  - True Vector Integration, Parallel Tree Search, Neo4j Graph, Ensemble Retrieval, and Rust MCP Server.
- **Sprint 32: Asgard/Bifrost MCP Adapter & Dynamic Tenants** [Planned]
  - Auto-discover tools from MCP servers, Dynamic Context Isolation (X-Tenant-ID), Agent-to-Agent via JSON-RPC.
- **Sprint 33: Ecosystem Gateway Sidecars** [Planned]
  - Yggdrasil & Eir Universal Go Sidecars to expose auth and medical tools to Asgard.
- **Sprint 34: Platform Automation (Testing, Browsing & Security)** [Planned]
  - Deploy MCP across Fenrir, Forseti, Ratatoskr, Huginn, Muninn, and Heimdall.

## 4. Sprint Schedule

| Sprint | Duration | Deliverable | Tests | Status |
|:--|:--|:--|:--|:--|
| **S1** | 2 wk | Foundation: scaffold, health, GitHub poller, label filter | 19 | ✅ Done (2026-03-15) |
| **S2** | 2 wk | AI Analyzer + Auto-Fix + PR creator + LLM client | 37 | ✅ Done (2026-03-15) |
| **S3** | 2 wk | Multi-Agent Pipeline: Analyzer→Coder→Reviewer→Tester | — | 📋 Planned |
| **S4** | 2 wk | Continuous Learning + Trend Analysis | — | 📋 Planned |

---

## 5. Test Summary (Current)

| Metric | Value |
|:--|:--|
| Total tests | **37** |
| Tests passed | 37 |
| Tests failed | 0 |
| Test time | 0.01s |
| Framework | Rust `cargo test` |

---

## 6. Safety Rules

- All PRs created as **draft** — never auto-merge
- PR title prefix: `[Muninn Auto-Fix]`
- Max 3 files per PR — if more, create issue instead
- Must pass `cargo check` / `npm test` before push
- Max 3 review iterations before escalating to human

---

## 7. Risk Assessment

| Risk | Impact | Mitigation |
|:--|:--|:--|
| Auto-fix introduces new bug | High | Draft PR only, reviewer agent, human approval |
| LLM generates incorrect fix | Medium | Multi-agent review + test agent |
| GitHub API rate limiting | Medium | Configurable poll interval, exponential backoff |
| Fix spans too many files | Medium | Max 3 files rule, escalate to issue |

---

## 8. References

- [BRD](../../../Asgard/docs/business/odins-ravens-brd.md)
- [TRD](../../../Asgard/docs/business/odins-ravens-trd.md)
- [SI-01 Implementation Report](../SI_01_Implementation_Report.md)

---

*บันทึกโดย: AI Assistant (ISO/IEC 29110 PM-01)*
*Created: 2026-03-16 by Antigravity*
