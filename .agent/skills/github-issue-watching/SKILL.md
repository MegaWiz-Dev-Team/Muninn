---
name: github-issue-watching
description: How to watch GitHub repositories for security issues, analyze them with LLM, and propose automated fixes via pull requests
---

# GitHub Issue Watching & Auto-Fix

## When to Use This Skill

- Implementing the GitHub issue watcher in Muninn
- Analyzing security vulnerabilities from scan reports
- Generating code fixes using LLMs
- Creating pull requests with proposed fixes

## GitHub API via octocrab

### Setup
```rust
use octocrab::Octocrab;

let octocrab = Octocrab::builder()
    .personal_token(std::env::var("GITHUB_TOKEN")?)
    .build()?;
```

### Watch Issues
```rust
// List issues with security labels
let issues = octocrab.issues("MegaWiz-Dev-Team", "Mimir")
    .list()
    .labels(&["security", "vulnerability", "huginn-finding"])
    .state(octocrab::params::State::Open)
    .per_page(50)
    .send().await?;
```

### Get Issue Details
```rust
let issue = octocrab.issues("owner", "repo")
    .get(issue_number)
    .await?;
```

### Create Fix Branch + PR
```rust
// 1. Create branch
let base_sha = octocrab.repos("owner", "repo")
    .get_ref(&Reference::Branch("main".into()))
    .await?.object.sha;

octocrab.repos("owner", "repo")
    .create_ref(&Reference::Branch("fix/issue-123".into()), base_sha)
    .await?;

// 2. Push fix
octocrab.repos("owner", "repo")
    .create_file("src/fix.rs", "fix: patch vulnerability #123", fixed_content)
    .branch("fix/issue-123")
    .send().await?;

// 3. Create PR
octocrab.pulls("owner", "repo")
    .create("fix: patch vulnerability #123", "fix/issue-123", "main")
    .body("Auto-fix by Muninn 🐦\n\nFixes #123\n\n## Changes\n- ...")
    .send().await?;
```

## Analysis Pipeline

```
1. POLL:    Check repos every 5 min for new issues with security labels
2. FILTER:  Skip issues already analyzed (check SQLite)
3. FETCH:   Get issue body + referenced files/code
4. ANALYZE: Send to LLM (Gemini for complex, Qwen for simple)
5. FIX:     Generate code patch
6. TEST:    Verify patch compiles (cargo check) if Rust
7. PR:      Create branch + commit + PR
8. TRACK:   Store in SQLite, update issue with comment
```

## Issue Labels to Watch

| Label | Action |
|:--|:--|
| `huginn-finding` | Huginn created this issue — auto-analyze |
| `security` | General security issue — analyze + propose fix |
| `vulnerability` | Known CVE — check remediation DB |
| `auto-fix` | Explicitly requested auto-fix |

## LLM Prompt for Fix Generation

```
You are a security expert. Given the following vulnerability:

**Title:** {title}
**Description:** {description}
**File:** {file_path}
**Code:**
```{language}
{code_snippet}
```

Generate a minimal, secure fix. Only modify what's necessary.
Return the fixed code and explain the change.
```

## Safety Checks

- Never merge PRs automatically — always create as draft
- Add `[Muninn Auto-Fix]` prefix to PR titles
- Include original finding reference in PR body
- Run `cargo check` / `cargo clippy` before pushing
- Max 3 files changed per PR — if more, create issue instead
