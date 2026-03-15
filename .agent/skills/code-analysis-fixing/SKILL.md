---
name: code-analysis-fixing
description: How to analyze code for vulnerabilities using LLMs and generate secure patches
---

# Code Analysis & Auto-Fixing

## When to Use This Skill

- Analyzing code snippets for security vulnerabilities
- Generating secure code patches using LLMs
- Root cause analysis of Huginn scan findings
- Creating remediation recommendations

## Analysis Workflow

```
Finding (from Huginn)
    ↓
Fetch affected code (GitHub API)
    ↓
Build analysis context (code + finding + deps)
    ↓
LLM Analysis (Gemini for complex, Qwen for simple)
    ↓
Generate fix + explanation
    ↓
Verify fix (cargo check / syntax check)
    ↓
Create PR (draft)
```

## LLM Selection for Code Analysis

| Task | Token Count | Model |
|:--|:--|:--|
| Single file fix | < 4K tokens | Qwen3.5 (local) |
| Multi-file analysis | > 4K tokens | Gemini API |
| Root cause analysis | Any | Gemini API |
| Fix validation | < 2K tokens | Qwen3.5 (local) |

## Structured Analysis Prompt

```
<system>
You are a security code reviewer. Analyze the vulnerability and provide:
1. Root cause explanation
2. Impact assessment (CVSS-style)
3. Minimal code fix
4. Prevention recommendation
</system>

<user>
## Vulnerability
- **CWE:** {cwe_id} - {cwe_name}
- **Severity:** {severity}
- **Tool:** {scanner_tool}
- **Finding:** {finding_description}

## Affected Code
File: {file_path}
```{language}
{code}
```

## Context
- Framework: {framework}
- Dependencies: {relevant_deps}
</user>
```

## Common CWE Fix Patterns

### CWE-89: SQL Injection
```rust
// BAD
let query = format!("SELECT * FROM users WHERE id = {}", user_input);

// GOOD
let mut stmt = conn.prepare("SELECT * FROM users WHERE id = ?1")?;
stmt.query_map(params![user_input], |row| { ... })?;
```

### CWE-79: XSS
```rust
// BAD — unescaped HTML output
format!("<div>{}</div>", user_input)

// GOOD — escape HTML
html_escape::encode_text(&user_input)
```

### CWE-798: Hardcoded Credentials
```rust
// BAD
let api_key = "sk-1234567890";

// GOOD
let api_key = std::env::var("API_KEY")?;
```

### CWE-22: Path Traversal
```rust
// BAD
let path = format!("/data/{}", user_filename);

// GOOD
let filename = Path::new(user_filename).file_name()
    .ok_or("invalid filename")?;
let path = Path::new("/data").join(filename);
```

## Fix Verification

Before creating a PR, verify the fix:

### Rust Projects
```bash
cargo check          # Type check
cargo clippy         # Lint check
cargo test           # Run tests
```

### Python Projects
```bash
python -m py_compile file.py  # Syntax check
ruff check file.py            # Lint
pytest tests/                 # Run tests
```

### JavaScript/TypeScript
```bash
npx tsc --noEmit     # Type check
npx eslint file.ts   # Lint
npm test             # Run tests
```

## Fix Quality Rules

1. **Minimal changes** — only modify what's needed to fix the vulnerability
2. **No feature additions** — fix ONLY the security issue
3. **Preserve style** — match existing code conventions
4. **Add comments** — explain WHY the fix is needed
5. **Test coverage** — if modifying a tested function, ensure tests still pass
6. **Max 3 files** — if fix requires > 3 files, create an issue describing the fix instead
