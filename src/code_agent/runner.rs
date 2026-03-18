use std::time::Duration;
use tokio::process::Command;

/// Output from a subprocess command
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run a command as a subprocess with timeout and output capture
pub async fn run_agent_command(
    cmd: &str,
    args: &[&str],
    work_dir: &str,
    env_vars: &[(&str, &str)],
    timeout: Duration,
) -> Result<CommandOutput, String> {
    tracing::info!("🔧 Running: {} {} (in {})", cmd, args.join(" "), work_dir);

    let mut command = Command::new(cmd);
    command
        .args(args)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (key, value) in env_vars {
        command.env(key, value);
    }

    let child = command
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {} — is it installed and in PATH?", cmd, e))?;

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("Command '{}' timed out after {}s", cmd, timeout.as_secs()))?
        .map_err(|e| format!("Command '{}' failed: {}", cmd, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    if exit_code != 0 {
        tracing::warn!(
            "⚠️ Command '{}' exit code {}: stderr={}",
            cmd, exit_code, &stderr[..stderr.len().min(500)]
        );
    }

    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code,
    })
}

/// Clone a git repository into a target directory
pub async fn git_clone(
    repo_url: &str,
    target_dir: &str,
    github_token: &str,
    timeout: Duration,
) -> Result<(), String> {
    // Build authenticated URL for private repos
    let auth_url = if !github_token.is_empty() && repo_url.starts_with("https://") {
        repo_url.replacen(
            "https://",
            &format!("https://x-access-token:{}@", github_token),
            1,
        )
    } else {
        repo_url.to_string()
    };

    let output = run_agent_command(
        "git",
        &["clone", "--depth", "1", &auth_url, target_dir],
        "/tmp",
        &[],
        timeout,
    )
    .await?;

    if output.exit_code != 0 {
        return Err(format!("git clone failed: {}", output.stderr));
    }

    tracing::info!("📦 Cloned {} → {}", repo_url, target_dir);
    Ok(())
}

/// Get list of changed files via git diff
pub async fn git_changed_files(work_dir: &str) -> Result<Vec<String>, String> {
    let output = run_agent_command(
        "git",
        &["diff", "--name-only", "HEAD"],
        work_dir,
        &[],
        Duration::from_secs(30),
    )
    .await?;

    // Also include untracked files
    let untracked = run_agent_command(
        "git",
        &["ls-files", "--others", "--exclude-standard"],
        work_dir,
        &[],
        Duration::from_secs(30),
    )
    .await?;

    let mut files: Vec<String> = output
        .stdout
        .lines()
        .chain(untracked.stdout.lines())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    files.sort();
    files.dedup();
    Ok(files)
}

/// Commit and push changes
pub async fn git_commit_and_push(
    work_dir: &str,
    branch_name: &str,
    commit_message: &str,
) -> Result<(), String> {
    // Create branch
    run_agent_command(
        "git",
        &["checkout", "-b", branch_name],
        work_dir,
        &[],
        Duration::from_secs(30),
    )
    .await?;

    // Stage all changes
    run_agent_command(
        "git",
        &["add", "-A"],
        work_dir,
        &[],
        Duration::from_secs(30),
    )
    .await?;

    // Configure git user
    run_agent_command(
        "git",
        &["config", "user.email", "muninn@asgard.ai"],
        work_dir,
        &[],
        Duration::from_secs(10),
    )
    .await?;

    run_agent_command(
        "git",
        &["config", "user.name", "Muninn Bot"],
        work_dir,
        &[],
        Duration::from_secs(10),
    )
    .await?;

    // Commit
    let commit_output = run_agent_command(
        "git",
        &["commit", "-m", commit_message],
        work_dir,
        &[],
        Duration::from_secs(30),
    )
    .await?;

    if commit_output.exit_code != 0 {
        return Err(format!("git commit failed: {}", commit_output.stderr));
    }

    // Push
    let push_output = run_agent_command(
        "git",
        &["push", "origin", branch_name],
        work_dir,
        &[],
        Duration::from_secs(120),
    )
    .await?;

    if push_output.exit_code != 0 {
        return Err(format!("git push failed: {}", push_output.stderr));
    }

    tracing::info!("📤 Pushed branch: {}", branch_name);
    Ok(())
}

/// Clean up workspace directory
pub fn cleanup_workspace(work_dir: &str) {
    if let Err(e) = std::fs::remove_dir_all(work_dir) {
        tracing::warn!("⚠️ Failed to cleanup workspace {}: {}", work_dir, e);
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_output_fields() {
        let output = CommandOutput {
            stdout: "hello world".to_string(),
            stderr: String::new(),
            exit_code: 0,
        };
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("hello"));
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn test_git_clone_url_auth_injection() {
        let token = "ghp_test123";
        let url = "https://github.com/owner/repo.git";
        let auth_url = url.replacen(
            "https://",
            &format!("https://x-access-token:{}@", token),
            1,
        );
        assert_eq!(
            auth_url,
            "https://x-access-token:ghp_test123@github.com/owner/repo.git"
        );
    }

    #[test]
    fn test_git_clone_url_no_token() {
        let url = "https://github.com/owner/repo.git";
        let token = "";
        let auth_url = if !token.is_empty() && url.starts_with("https://") {
            url.replacen("https://", &format!("https://x-access-token:{}@", token), 1)
        } else {
            url.to_string()
        };
        assert_eq!(auth_url, url);
    }

    #[test]
    fn test_git_clone_ssh_url_unchanged() {
        let url = "git@github.com:owner/repo.git";
        let token = "ghp_test123";
        let auth_url = if !token.is_empty() && url.starts_with("https://") {
            url.replacen("https://", &format!("https://x-access-token:{}@", token), 1)
        } else {
            url.to_string()
        };
        assert_eq!(auth_url, url);
    }

    #[tokio::test]
    async fn test_run_echo_command() {
        let result = run_agent_command(
            "echo",
            &["hello", "muninn"],
            "/tmp",
            &[],
            Duration::from_secs(5),
        )
        .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("hello muninn"));
    }

    #[tokio::test]
    async fn test_run_command_with_env() {
        let result = run_agent_command(
            "env",
            &[],
            "/tmp",
            &[("MUNINN_TEST_VAR", "test_value_123")],
            Duration::from_secs(5),
        )
        .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.contains("MUNINN_TEST_VAR=test_value_123"));
    }

    #[tokio::test]
    async fn test_run_nonexistent_command() {
        let result = run_agent_command(
            "nonexistent_command_xyz",
            &[],
            "/tmp",
            &[],
            Duration::from_secs(5),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to spawn"));
    }
}
