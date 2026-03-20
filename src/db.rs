use rusqlite::{Connection, params};
use std::sync::Mutex;

use crate::models::{AnalysisStatus, IssuePriority, TrackedIssue};

/// Thread-safe SQLite database
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tracked_issues (
                id TEXT PRIMARY KEY,
                repo TEXT NOT NULL,
                issue_number INTEGER NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '',
                labels TEXT NOT NULL DEFAULT '[]',
                priority TEXT NOT NULL DEFAULT 'low',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                fix_branch TEXT,
                fix_pr_url TEXT,
                error TEXT,
                fix_diff TEXT,
                fix_analysis TEXT,
                UNIQUE(repo, issue_number)
            );

            CREATE TABLE IF NOT EXISTS watched_repos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo TEXT NOT NULL UNIQUE,
                last_checked TEXT,
                enabled INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS fixes (
                id TEXT PRIMARY KEY,
                issue_id TEXT NOT NULL,
                branch_name TEXT NOT NULL,
                pr_url TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                FOREIGN KEY (issue_id) REFERENCES tracked_issues(id)
            );

            CREATE INDEX IF NOT EXISTS idx_issues_status ON tracked_issues(status);
            CREATE INDEX IF NOT EXISTS idx_issues_repo ON tracked_issues(repo);
            CREATE INDEX IF NOT EXISTS idx_fixes_issue ON fixes(issue_id);"
        )?;

        // Forward-compatible migration for existing DBs
        let _ = conn.execute("ALTER TABLE tracked_issues ADD COLUMN fix_diff TEXT", []);
        let _ = conn.execute("ALTER TABLE tracked_issues ADD COLUMN fix_analysis TEXT", []);

        Ok(())
    }

    /// Insert or update a tracked issue
    pub fn upsert_issue(&self, issue: &TrackedIssue) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let labels_json = serde_json::to_string(&issue.labels).unwrap_or_default();
        conn.execute(
            "INSERT INTO tracked_issues (id, repo, issue_number, title, body, labels, priority, status, created_at, updated_at, fix_branch, fix_pr_url, error, fix_diff, fix_analysis)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(repo, issue_number) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                labels = excluded.labels,
                priority = excluded.priority,
                updated_at = excluded.updated_at",
            params![
                issue.id, issue.repo, issue.issue_number,
                issue.title, issue.body, labels_json,
                issue.priority.to_string(), issue.status.to_string(),
                issue.created_at.to_rfc3339(), issue.updated_at.to_rfc3339(),
                issue.fix_branch, issue.fix_pr_url, issue.error,
                issue.fix_diff, issue.fix_analysis,
            ],
        )?;
        Ok(())
    }

    /// Update issue status
    pub fn update_issue_status(&self, id: &str, status: &AnalysisStatus) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tracked_issues SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.to_string(), chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Save fix diff and analysis for review
    pub fn update_issue_fix_diff(&self, id: &str, diff: &str, analysis: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tracked_issues SET fix_diff = ?1, fix_analysis = ?2, status = 'review_pending', updated_at = ?3 WHERE id = ?4",
            params![diff, analysis, chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Get issue by ID
    pub fn get_issue(&self, id: &str) -> Result<Option<TrackedIssue>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, repo, issue_number, title, body, labels, priority, status, created_at, updated_at, fix_branch, fix_pr_url, error, fix_diff, fix_analysis
             FROM tracked_issues WHERE id = ?1"
        )?;
        stmt.query_row(params![id], |row| Self::row_to_issue(row)).optional()
    }

    /// Check if an issue is already tracked
    pub fn is_tracked(&self, repo: &str, issue_number: u64) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM tracked_issues WHERE repo = ?1 AND issue_number = ?2",
            params![repo, issue_number],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    /// List issues (optionally filter by status)
    pub fn list_issues(&self, status_filter: Option<&str>) -> Result<Vec<TrackedIssue>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let (query, param): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match status_filter {
            Some(s) => (
                "SELECT id, repo, issue_number, title, body, labels, priority, status, created_at, updated_at, fix_branch, fix_pr_url, error, fix_diff, fix_analysis FROM tracked_issues WHERE status = ?1 ORDER BY updated_at DESC",
                vec![Box::new(s.to_string())],
            ),
            None => (
                "SELECT id, repo, issue_number, title, body, labels, priority, status, created_at, updated_at, fix_branch, fix_pr_url, error, fix_diff, fix_analysis FROM tracked_issues ORDER BY updated_at DESC",
                vec![],
            ),
        };
        let mut stmt = conn.prepare(query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(param.iter()), |row| {
            Self::row_to_issue(row)
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Stats: (total_issues, total_fixes)
    pub fn get_stats(&self) -> Result<(u64, u64), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let issues: u64 = conn.query_row("SELECT COUNT(*) FROM tracked_issues", [], |r| r.get(0))?;
        let fixes: u64 = conn.query_row(
            "SELECT COUNT(*) FROM tracked_issues WHERE status = 'fixed'",
            [], |r| r.get(0),
        )?;
        Ok((issues, fixes))
    }

    fn row_to_issue(row: &rusqlite::Row) -> Result<TrackedIssue, rusqlite::Error> {
        let labels_str: String = row.get(5)?;
        let priority_str: String = row.get(6)?;
        let status_str: String = row.get(7)?;
        let created_str: String = row.get(8)?;
        let updated_str: String = row.get(9)?;

        Ok(TrackedIssue {
            id: row.get(0)?,
            repo: row.get(1)?,
            issue_number: row.get(2)?,
            title: row.get(3)?,
            body: row.get(4)?,
            labels: serde_json::from_str(&labels_str).unwrap_or_default(),
            priority: match priority_str.as_str() {
                "critical" => IssuePriority::Critical,
                "high" => IssuePriority::High,
                "medium" => IssuePriority::Medium,
                _ => IssuePriority::Low,
            },
            status: AnalysisStatus::from_str(&status_str),
            created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            updated_at: chrono::DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            fix_branch: row.get(10)?,
            fix_pr_url: row.get(11)?,
            error: row.get(12)?,
            fix_diff: row.get(13)?,
            fix_analysis: row.get(14)?,
        })
    }
}

use rusqlite::OptionalExtension;

// ── Tests (TDD) ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use chrono::Utc;

    fn test_db() -> Database {
        let db = Database::new(":memory:").unwrap();
        db.migrate().unwrap();
        db
    }

    fn sample_issue(id: &str, repo: &str, num: u64) -> TrackedIssue {
        TrackedIssue {
            id: id.to_string(),
            repo: repo.to_string(),
            issue_number: num,
            title: format!("Issue #{}", num),
            body: "Test body".to_string(),
            labels: vec!["huginn-finding".to_string()],
            priority: IssuePriority::High,
            status: AnalysisStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            fix_branch: None,
            fix_pr_url: None,
            error: None,
            fix_diff: None,
            fix_analysis: None,
        }
    }

    #[test]
    fn test_migrate_creates_tables() {
        let db = test_db();
        let (issues, fixes) = db.get_stats().unwrap();
        assert_eq!(issues, 0);
        assert_eq!(fixes, 0);
    }

    #[test]
    fn test_upsert_and_get_issue() {
        let db = test_db();
        let issue = sample_issue("iss-1", "MegaWiz-Dev-Team/Mimir", 42);
        db.upsert_issue(&issue).unwrap();

        let result = db.get_issue("iss-1").unwrap();
        assert!(result.is_some());
        let i = result.unwrap();
        assert_eq!(i.issue_number, 42);
        assert_eq!(i.repo, "MegaWiz-Dev-Team/Mimir");
        assert_eq!(i.priority, IssuePriority::High);
    }

    #[test]
    fn test_upsert_updates_existing() {
        let db = test_db();
        let mut issue = sample_issue("iss-2", "MegaWiz-Dev-Team/Bifrost", 10);
        issue.title = "Original".to_string();
        db.upsert_issue(&issue).unwrap();

        issue.id = "iss-2b".to_string(); // different ID but same repo+number
        issue.title = "Updated".to_string();
        db.upsert_issue(&issue).unwrap();

        // Should still be 1 issue (upsert)
        let (count, _) = db.get_stats().unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_is_tracked() {
        let db = test_db();
        assert!(!db.is_tracked("repo", 1).unwrap());

        db.upsert_issue(&sample_issue("iss-3", "repo", 1)).unwrap();
        assert!(db.is_tracked("repo", 1).unwrap());
        assert!(!db.is_tracked("repo", 2).unwrap());
    }

    #[test]
    fn test_update_status() {
        let db = test_db();
        let issue = sample_issue("iss-4", "repo", 5);
        db.upsert_issue(&issue).unwrap();

        db.update_issue_status("iss-4", &AnalysisStatus::Analyzing).unwrap();
        let updated = db.get_issue("iss-4").unwrap().unwrap();
        assert_eq!(updated.status, AnalysisStatus::Analyzing);
    }

    #[test]
    fn test_list_issues_all() {
        let db = test_db();
        for i in 1..=5 {
            db.upsert_issue(&sample_issue(&format!("iss-{}", i), "repo", i)).unwrap();
        }
        let all = db.list_issues(None).unwrap();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_list_issues_filter_status() {
        let db = test_db();
        db.upsert_issue(&sample_issue("a", "repo", 1)).unwrap();
        db.upsert_issue(&sample_issue("b", "repo", 2)).unwrap();
        db.update_issue_status("a", &AnalysisStatus::Fixed).unwrap();

        let pending = db.list_issues(Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        let fixed = db.list_issues(Some("fixed")).unwrap();
        assert_eq!(fixed.len(), 1);
    }

    #[test]
    fn test_stats_counts_fixed() {
        let db = test_db();
        db.upsert_issue(&sample_issue("x", "repo", 1)).unwrap();
        db.upsert_issue(&sample_issue("y", "repo", 2)).unwrap();
        db.update_issue_status("x", &AnalysisStatus::Fixed).unwrap();

        let (total, fixed) = db.get_stats().unwrap();
        assert_eq!(total, 2);
        assert_eq!(fixed, 1);
    }

    #[test]
    fn test_labels_preserved() {
        let db = test_db();
        let mut issue = sample_issue("lab-1", "repo", 99);
        issue.labels = vec!["huginn-finding".to_string(), "severity:critical".to_string()];
        db.upsert_issue(&issue).unwrap();

        let result = db.get_issue("lab-1").unwrap().unwrap();
        assert_eq!(result.labels.len(), 2);
        assert!(result.labels.contains(&"severity:critical".to_string()));
    }

    // ── TDD: Review mode DB operations ──

    #[test]
    fn test_update_issue_fix_diff() {
        let db = test_db();
        let issue = sample_issue("review-1", "repo", 100);
        db.upsert_issue(&issue).unwrap();

        let diff = "-allow_origins(Any)\n+allow_origins([origin])";
        let analysis = "Replace wildcard CORS with specific origins";
        db.update_issue_fix_diff("review-1", diff, analysis).unwrap();

        let updated = db.get_issue("review-1").unwrap().unwrap();
        assert_eq!(updated.status, AnalysisStatus::ReviewPending);
        assert_eq!(updated.fix_diff.as_deref(), Some(diff));
        assert_eq!(updated.fix_analysis.as_deref(), Some(analysis));
    }

    #[test]
    fn test_fix_diff_none_by_default() {
        let db = test_db();
        let issue = sample_issue("nodiff-1", "repo", 101);
        db.upsert_issue(&issue).unwrap();

        let result = db.get_issue("nodiff-1").unwrap().unwrap();
        assert!(result.fix_diff.is_none());
        assert!(result.fix_analysis.is_none());
    }

    #[test]
    fn test_review_pending_to_fixing() {
        let db = test_db();
        let issue = sample_issue("flow-1", "repo", 102);
        db.upsert_issue(&issue).unwrap();

        // Simulate review flow: pending → review_pending → fixing → fixed
        db.update_issue_fix_diff("flow-1", "diff", "analysis").unwrap();
        let i = db.get_issue("flow-1").unwrap().unwrap();
        assert_eq!(i.status, AnalysisStatus::ReviewPending);

        db.update_issue_status("flow-1", &AnalysisStatus::Fixing).unwrap();
        let i = db.get_issue("flow-1").unwrap().unwrap();
        assert_eq!(i.status, AnalysisStatus::Fixing);

        db.update_issue_status("flow-1", &AnalysisStatus::Fixed).unwrap();
        let i = db.get_issue("flow-1").unwrap().unwrap();
        assert_eq!(i.status, AnalysisStatus::Fixed);
    }

    #[test]
    fn test_review_pending_to_skipped_on_reject() {
        let db = test_db();
        let issue = sample_issue("reject-1", "repo", 103);
        db.upsert_issue(&issue).unwrap();

        db.update_issue_fix_diff("reject-1", "diff", "analysis").unwrap();
        db.update_issue_status("reject-1", &AnalysisStatus::Skipped).unwrap();

        let i = db.get_issue("reject-1").unwrap().unwrap();
        assert_eq!(i.status, AnalysisStatus::Skipped);
        // fix_diff should still be preserved even after rejection
        assert!(i.fix_diff.is_some());
    }

    #[test]
    fn test_list_review_pending_issues() {
        let db = test_db();
        db.upsert_issue(&sample_issue("rp-1", "repo", 200)).unwrap();
        db.upsert_issue(&sample_issue("rp-2", "repo", 201)).unwrap();
        db.update_issue_fix_diff("rp-1", "diff1", "analysis1").unwrap();

        let review = db.list_issues(Some("review_pending")).unwrap();
        assert_eq!(review.len(), 1);
        assert_eq!(review[0].id, "rp-1");

        let pending = db.list_issues(Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "rp-2");
    }
}
