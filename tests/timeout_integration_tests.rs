//! Integration tests for hook timeout functionality
//!
//! Tests that hooks:
//! - Complete successfully within timeout
//! - Are killed when exceeding timeout
//! - Respect custom timeout values
//! - Include partial output in timeout errors

use std::{fs, process::Command};
use tempfile::TempDir;

/// Helper to create a git repository with configuration
fn setup_test_repo_with_config(config_content: &str) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Configure git
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Disable GPG signing
    Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Write hooks.toml
    fs::write(repo_path.join("hooks.toml"), config_content).unwrap();

    temp_dir
}

/// Get the peter-hook binary path
fn peter_hook_bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_hook_completes_within_timeout() {
    let config = r#"
[hooks.fast-hook]
command = "echo 'Fast hook completed'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["fast-hook"]
description = "Fast hooks"
"#;

    let temp_dir = setup_test_repo_with_config(config);
    let repo_path = temp_dir.path();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run the hook - should complete successfully
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("Fast hook completed"),
        "Hook should complete successfully within timeout.\nOutput: {combined}"
    );
    assert!(output.status.success(), "Command should succeed");
}

#[test]
fn test_hook_exceeds_timeout() {
    let config = r#"
[hooks.slow-hook]
command = "sleep 10 && echo 'This should not appear'"
modifies_repository = false
timeout_seconds = 1

[groups.pre-commit]
includes = ["slow-hook"]
description = "Slow hooks"
"#;

    let temp_dir = setup_test_repo_with_config(config);
    let repo_path = temp_dir.path();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run the hook - should timeout
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should contain timeout error message
    assert!(
        combined.contains("exceeded timeout") || combined.contains("killed"),
        "Hook should be killed after timeout.\nOutput: {combined}"
    );

    // Should NOT contain the message that was supposed to print after sleep
    assert!(
        !combined.contains("This should not appear"),
        "Hook output after timeout should not appear.\nOutput: {combined}"
    );

    // Should fail (non-zero exit code)
    assert!(!output.status.success(), "Command should fail on timeout");
}

#[test]
fn test_timeout_respects_custom_value() {
    let config = r#"
[hooks.medium-hook]
command = "sleep 2 && echo 'Completed after 2 seconds'"
modifies_repository = false
timeout_seconds = 3

[groups.pre-commit]
includes = ["medium-hook"]
description = "Medium duration hooks"
"#;

    let temp_dir = setup_test_repo_with_config(config);
    let repo_path = temp_dir.path();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run the hook - should complete because timeout is 3 seconds and hook takes 2
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("Completed after 2 seconds"),
        "Hook should complete within custom timeout.\nOutput: {combined}"
    );
    assert!(output.status.success(), "Command should succeed");
}

#[test]
fn test_timeout_with_partial_output() {
    let config = r#"
[hooks.partial-output-hook]
command = "echo 'Starting...'; echo 'Working...'; sleep 10; echo 'Never reached'"
modifies_repository = false
timeout_seconds = 1

[groups.pre-commit]
includes = ["partial-output-hook"]
description = "Hook with partial output"
"#;

    let temp_dir = setup_test_repo_with_config(config);
    let repo_path = temp_dir.path();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run the hook - should timeout but capture partial output
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should contain timeout error
    assert!(
        combined.contains("exceeded timeout") || combined.contains("killed"),
        "Should show timeout error.\nOutput: {combined}"
    );

    // Should contain partial output (the early echo statements)
    assert!(
        combined.contains("Starting") || combined.contains("Working"),
        "Should include partial output before timeout.\nOutput: {combined}"
    );

    // Should NOT contain the final echo after sleep
    assert!(
        !combined.contains("Never reached"),
        "Should not contain output after timeout.\nOutput: {combined}"
    );

    assert!(!output.status.success(), "Command should fail on timeout");
}

#[test]
fn test_timeout_with_other_execution_type() {
    let config = r#"
[hooks.template-hook]
command = "echo 'Files: {CHANGED_FILES}'; sleep 10"
modifies_repository = false
execution_type = "other"
timeout_seconds = 1

[groups.pre-commit]
includes = ["template-hook"]
description = "Template-based hooks"
"#;

    let temp_dir = setup_test_repo_with_config(config);
    let repo_path = temp_dir.path();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run the hook - should timeout
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should contain timeout error
    assert!(
        combined.contains("exceeded timeout") || combined.contains("killed"),
        "Template hook should timeout.\nOutput: {combined}"
    );

    assert!(!output.status.success(), "Command should fail on timeout");
}

#[test]
fn test_default_timeout_allows_long_running_hooks() {
    let config = r#"
[hooks.default-timeout-hook]
command = "echo 'Starting'; sleep 2; echo 'Finished'"
modifies_repository = false
# No timeout_seconds specified - should use default of 300 seconds

[groups.pre-commit]
includes = ["default-timeout-hook"]
description = "Hook with default timeout"
"#;

    let temp_dir = setup_test_repo_with_config(config);
    let repo_path = temp_dir.path();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run the hook - should complete (default timeout is 300 seconds)
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("Finished"),
        "Hook should complete with default timeout.\nOutput: {combined}"
    );
    assert!(output.status.success(), "Command should succeed");
}
