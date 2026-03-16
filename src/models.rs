use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Issue priority derived from labels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IssuePriority {
    Critical,
    High,
    Medium,
    Low,
}

impl fmt::Display for IssuePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssuePriority::Critical => write!(f, "critical"),
            IssuePriority::High => write!(f, "high"),
            IssuePriority::Medium => write!(f, "medium"),
            IssuePriority::Low => write!(f, "low"),
        }
    }
}

/// Issue analysis status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisStatus {
    Pending,
    Analyzing,
    Analyzed,
    Fixing,
    Fixed,
    Skipped,
    Failed,
}

impl fmt::Display for AnalysisStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisStatus::Pending => write!(f, "pending"),
            AnalysisStatus::Analyzing => write!(f, "analyzing"),
            AnalysisStatus::Analyzed => write!(f, "analyzed"),
            AnalysisStatus::Fixing => write!(f, "fixing"),
            AnalysisStatus::Fixed => write!(f, "fixed"),
            AnalysisStatus::Skipped => write!(f, "skipped"),
            AnalysisStatus::Failed => write!(f, "failed"),
        }
    }
}

impl AnalysisStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "analyzing" => Self::Analyzing,
            "analyzed" => Self::Analyzed,
            "fixing" => Self::Fixing,
            "fixed" => Self::Fixed,
            "skipped" => Self::Skipped,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

/// Labels that Muninn watches for
pub const WATCH_LABELS: &[&str] = &[
    "huginn-finding",
    "security",
    "vulnerability",
    "auto-fix",
];

/// Labels that tell Muninn to skip
pub const SKIP_LABELS: &[&str] = &["muninn-skip"];

/// A tracked GitHub issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedIssue {
    pub id: String,
    pub repo: String,
    pub issue_number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub priority: IssuePriority,
    pub status: AnalysisStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_pr_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// GitHub issue from API (simplified)
#[derive(Debug, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub labels: Vec<GitHubLabel>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
}

/// Check if an issue should be watched based on labels
pub fn should_watch(labels: &[String]) -> bool {
    let has_watch = labels.iter().any(|l| WATCH_LABELS.contains(&l.as_str()));
    let has_skip = labels.iter().any(|l| SKIP_LABELS.contains(&l.as_str()));
    has_watch && !has_skip
}

/// Derive priority from labels
pub fn derive_priority(labels: &[String]) -> IssuePriority {
    for label in labels {
        let l = label.to_lowercase();
        if l.contains("critical") { return IssuePriority::Critical; }
        if l.contains("high") { return IssuePriority::High; }
        if l.contains("medium") { return IssuePriority::Medium; }
    }
    IssuePriority::Low
}

// ── Tests (TDD) ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Label filter tests ──

    #[test]
    fn test_should_watch_huginn_finding() {
        let labels = vec!["huginn-finding".to_string(), "bug".to_string()];
        assert!(should_watch(&labels));
    }

    #[test]
    fn test_should_watch_security() {
        let labels = vec!["security".to_string()];
        assert!(should_watch(&labels));
    }

    #[test]
    fn test_should_watch_auto_fix() {
        let labels = vec!["auto-fix".to_string()];
        assert!(should_watch(&labels));
    }

    #[test]
    fn test_should_not_watch_random_labels() {
        let labels = vec!["enhancement".to_string(), "docs".to_string()];
        assert!(!should_watch(&labels));
    }

    #[test]
    fn test_should_not_watch_empty_labels() {
        let labels: Vec<String> = vec![];
        assert!(!should_watch(&labels));
    }

    #[test]
    fn test_skip_overrides_watch() {
        let labels = vec!["security".to_string(), "muninn-skip".to_string()];
        assert!(!should_watch(&labels));
    }

    // ── Priority derivation tests ──

    #[test]
    fn test_priority_critical() {
        let labels = vec!["security".to_string(), "severity:critical".to_string()];
        assert_eq!(derive_priority(&labels), IssuePriority::Critical);
    }

    #[test]
    fn test_priority_high() {
        let labels = vec!["huginn-finding".to_string(), "high".to_string()];
        assert_eq!(derive_priority(&labels), IssuePriority::High);
    }

    #[test]
    fn test_priority_default_low() {
        let labels = vec!["security".to_string()];
        assert_eq!(derive_priority(&labels), IssuePriority::Low);
    }

    // ── Status tests ──

    #[test]
    fn test_analysis_status_roundtrip() {
        let statuses = ["pending", "analyzing", "analyzed", "fixing", "fixed", "skipped", "failed"];
        for s in statuses {
            let status = AnalysisStatus::from_str(s);
            assert_eq!(status.to_string(), s);
        }
    }

    // ── Serialization tests ──

    #[test]
    fn test_tracked_issue_serialization() {
        let issue = TrackedIssue {
            id: "test-1".to_string(),
            repo: "MegaWiz-Dev-Team/Mimir".to_string(),
            issue_number: 42,
            title: "SQL Injection in login".to_string(),
            body: "Found by Huginn scan".to_string(),
            labels: vec!["huginn-finding".to_string()],
            priority: IssuePriority::High,
            status: AnalysisStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            fix_branch: None,
            fix_pr_url: None,
            error: None,
        };
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("SQL Injection"));
        assert!(json.contains("\"priority\":\"high\""));
        // None fields should be omitted
        assert!(!json.contains("fix_branch"));
    }
}
