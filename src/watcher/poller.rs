use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

use crate::AppState;
use crate::models::*;

/// Poll a single repo for new/updated issues matching watch labels
pub async fn poll_repo(state: &Arc<AppState>, repo: &str) -> Result<(), String> {
    tracing::info!("🔍 Polling {}", repo);

    let issues = fetch_issues(state, repo).await?;
    let mut new_count = 0;

    for gh_issue in issues {
        let labels: Vec<String> = gh_issue.labels.iter().map(|l| l.name.clone()).collect();

        // Apply label filter
        if !should_watch(&labels) {
            continue;
        }

        // Check if already tracked
        if state.db.is_tracked(repo, gh_issue.number).unwrap_or(true) {
            continue;
        }

        let priority = derive_priority(&labels);
        let tracked = TrackedIssue {
            id: Uuid::new_v4().to_string(),
            repo: repo.to_string(),
            issue_number: gh_issue.number,
            title: gh_issue.title,
            body: gh_issue.body.unwrap_or_default(),
            labels,
            priority,
            status: AnalysisStatus::Pending,
            created_at: chrono::DateTime::parse_from_rfc3339(&gh_issue.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: Utc::now(),
            fix_branch: None,
            fix_pr_url: None,
            error: None,
        };

        if let Err(e) = state.db.upsert_issue(&tracked) {
            tracing::error!("Failed to save issue #{}: {}", tracked.issue_number, e);
            continue;
        }

        tracing::info!("📥 New issue: {}#{} — {}", repo, tracked.issue_number, tracked.title);
        new_count += 1;
    }

    if new_count > 0 {
        tracing::info!("✅ {} — {} new issues tracked", repo, new_count);
    }
    Ok(())
}

/// Fetch open issues from GitHub API
async fn fetch_issues(state: &Arc<AppState>, repo: &str) -> Result<Vec<GitHubIssue>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/issues?state=open&per_page=30&sort=updated",
        repo
    );

    let response = state.http
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {}", state.config.github_token))
        .header("User-Agent", "Muninn/0.1.0")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API returned {}", response.status()));
    }

    let issues: Vec<GitHubIssue> = response
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    Ok(issues)
}

// ── Tests (TDD) ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_filter_integration() {
        // Simulates the filter logic that poll_repo uses
        let cases = vec![
            (vec!["huginn-finding", "severity:high"], true),
            (vec!["security", "sql-injection"], true),
            (vec!["auto-fix"], true),
            (vec!["vulnerability"], true),
            (vec!["security", "muninn-skip"], false),
            (vec!["enhancement"], false),
            (vec![], false),
        ];

        for (labels, expected) in cases {
            let labels: Vec<String> = labels.into_iter().map(String::from).collect();
            assert_eq!(
                should_watch(&labels), expected,
                "Labels {:?} should_watch = {} but got {}",
                labels, expected, !expected
            );
        }
    }

    #[test]
    fn test_github_issue_deserialization() {
        let json = r#"[
            {
                "number": 42,
                "title": "SQL Injection in /api/login",
                "body": "Found by Huginn scan",
                "labels": [
                    {"name": "huginn-finding"},
                    {"name": "severity:critical"}
                ],
                "created_at": "2026-03-16T00:00:00Z",
                "updated_at": "2026-03-16T01:00:00Z"
            }
        ]"#;

        let issues: Vec<GitHubIssue> = serde_json::from_str(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].number, 42);
        assert_eq!(issues[0].labels.len(), 2);
        assert_eq!(issues[0].labels[0].name, "huginn-finding");
    }

    #[test]
    fn test_priority_from_labels_integration() {
        let labels = vec!["huginn-finding".to_string(), "severity:critical".to_string()];
        assert_eq!(derive_priority(&labels), IssuePriority::Critical);

        let labels = vec!["security".to_string()];
        assert_eq!(derive_priority(&labels), IssuePriority::Low);
    }
}
