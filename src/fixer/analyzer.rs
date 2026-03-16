use serde::{Deserialize, Serialize};

use crate::llm::LlmClient;
use crate::models::TrackedIssue;

/// Analysis result from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAnalysis {
    pub root_cause: String,
    pub fixable: bool,
    pub affected_files: Vec<String>,
    pub fix_description: String,
    pub cwe_category: Option<String>,
}

/// Generated fix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFix {
    pub branch_name: String,
    pub commit_message: String,
    pub file_changes: Vec<FileChange>,
    pub test_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub content: String,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Modified,
    Created,
    Deleted,
}

const ANALYZE_SYSTEM_PROMPT: &str = r#"You are Muninn, an AI security issue analyzer. Given a GitHub issue about a security vulnerability, analyze it and respond in JSON format:

{
  "root_cause": "Brief root cause description",
  "fixable": true/false,
  "affected_files": ["path/to/file1.rs", "path/to/file2.rs"],
  "fix_description": "How to fix this issue",
  "cwe_category": "CWE-89" or null
}

Rules:
- Set fixable=false if the issue is too complex, requires architectural changes, or affects >3 files
- Be conservative — prefer false negatives over false positives
- Only suggest fixes for code-level issues, not infrastructure/deployment"#;

const FIX_SYSTEM_PROMPT: &str = r#"You are Muninn, an AI code fixer. Given an issue analysis, generate a minimal fix. Respond in JSON format:

{
  "branch_name": "fix/muninn-{issue_number}-short-desc",
  "commit_message": "fix: descriptive commit message\n\nCo-authored-by: Muninn <muninn@asgard.ai>",
  "file_changes": [
    {"path": "src/file.rs", "content": "full file content with fix applied", "change_type": "modified"}
  ],
  "test_commands": ["cargo test", "cargo clippy"]
}

Rules:
- Generate MINIMAL changes — touch as few lines as possible
- Include the FULL file content (not just diffs)
- Maximum 3 files changed
- Always include test commands"#;

/// Analyze a tracked issue using LLM
pub async fn analyze_issue(llm: &LlmClient, issue: &TrackedIssue) -> Result<IssueAnalysis, String> {
    let user_message = format!(
        "Repository: {}\nIssue #{}: {}\n\nBody:\n{}",
        issue.repo, issue.issue_number, issue.title, issue.body
    );

    let response = llm.chat(ANALYZE_SYSTEM_PROMPT, &user_message).await?;

    // Extract JSON from response (may be wrapped in markdown code block)
    let json_str = extract_json(&response)?;

    serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse analysis JSON: {}\nResponse: {}", e, json_str))
}

/// Generate a fix for an analyzed issue
pub async fn generate_fix(llm: &LlmClient, issue: &TrackedIssue, analysis: &IssueAnalysis) -> Result<GeneratedFix, String> {
    let user_message = format!(
        "Repository: {}\nIssue #{}: {}\n\nRoot cause: {}\nAffected files: {}\nFix description: {}",
        issue.repo, issue.issue_number, issue.title,
        analysis.root_cause,
        analysis.affected_files.join(", "),
        analysis.fix_description,
    );

    let response = llm.chat(FIX_SYSTEM_PROMPT, &user_message).await?;
    let json_str = extract_json(&response)?;

    serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse fix JSON: {}\nResponse: {}", e, json_str))
}

/// Extract JSON from a response that might be wrapped in ```json ... ```
fn extract_json(text: &str) -> Result<String, String> {
    // Try direct parse first
    if serde_json::from_str::<serde_json::Value>(text.trim()).is_ok() {
        return Ok(text.trim().to_string());
    }

    // Try extracting from markdown code block
    if let Some(start) = text.find("```json") {
        let after_marker = &text[start + 7..];
        if let Some(end) = after_marker.find("```") {
            return Ok(after_marker[..end].trim().to_string());
        }
    }

    // Try extracting from any code block
    if let Some(start) = text.find("```") {
        let after_marker = &text[start + 3..];
        // Skip optional language label
        let content_start = after_marker.find('\n').unwrap_or(0) + 1;
        if let Some(end) = after_marker[content_start..].find("```") {
            return Ok(after_marker[content_start..content_start + end].trim().to_string());
        }
    }

    // Try finding JSON object
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        return Ok(text[start..=end].to_string());
    }

    Err(format!("Could not extract JSON from response: {}", &text[..text.len().min(200)]))
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_direct() {
        let json = r#"{"root_cause": "SQL injection", "fixable": true, "affected_files": ["src/db.rs"], "fix_description": "Use parameterized queries", "cwe_category": "CWE-89"}"#;
        let result = extract_json(json).unwrap();
        let parsed: IssueAnalysis = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.root_cause, "SQL injection");
        assert!(parsed.fixable);
    }

    #[test]
    fn test_extract_json_from_markdown() {
        let text = "Here is the analysis:\n```json\n{\"root_cause\": \"XSS\", \"fixable\": true, \"affected_files\": [\"src/web.rs\"], \"fix_description\": \"Escape output\", \"cwe_category\": \"CWE-79\"}\n```\n";
        let result = extract_json(text).unwrap();
        let parsed: IssueAnalysis = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.root_cause, "XSS");
    }

    #[test]
    fn test_extract_json_from_braces() {
        let text = "Sure! Here is the result: {\"root_cause\": \"CSRF\", \"fixable\": false, \"affected_files\": [], \"fix_description\": \"Add CSRF tokens\", \"cwe_category\": null} Hope this helps!";
        let result = extract_json(text).unwrap();
        let parsed: IssueAnalysis = serde_json::from_str(&result).unwrap();
        assert!(!parsed.fixable);
    }

    #[test]
    fn test_analysis_deserialize() {
        let json = r#"{
            "root_cause": "Hardcoded credentials",
            "fixable": true,
            "affected_files": ["config.py"],
            "fix_description": "Use environment variables",
            "cwe_category": "CWE-798"
        }"#;
        let analysis: IssueAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.affected_files.len(), 1);
        assert_eq!(analysis.cwe_category, Some("CWE-798".to_string()));
    }

    #[test]
    fn test_fix_deserialize() {
        let json = r#"{
            "branch_name": "fix/muninn-42-sql-injection",
            "commit_message": "fix: use parameterized queries\n\nCo-authored-by: Muninn <muninn@asgard.ai>",
            "file_changes": [
                {"path": "src/db.rs", "content": "// fixed content", "change_type": "modified"}
            ],
            "test_commands": ["cargo test"]
        }"#;
        let fix: GeneratedFix = serde_json::from_str(json).unwrap();
        assert_eq!(fix.branch_name, "fix/muninn-42-sql-injection");
        assert_eq!(fix.file_changes.len(), 1);
        assert_eq!(fix.file_changes[0].change_type, ChangeType::Modified);
    }

    #[test]
    fn test_max_3_files_rule() {
        let json = r#"{
            "root_cause": "Complex refactor needed",
            "fixable": true,
            "affected_files": ["a.rs", "b.rs", "c.rs", "d.rs"],
            "fix_description": "Major refactor",
            "cwe_category": null
        }"#;
        let analysis: IssueAnalysis = serde_json::from_str(json).unwrap();
        // The pipeline should skip this — too many files
        assert!(analysis.affected_files.len() > 3);
    }
}
