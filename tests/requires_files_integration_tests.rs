//! Integration tests for the `requires_files` feature
//!
//! Tests that `requires_files=true` hooks:
//! - Skip in incompatible contexts (commit-msg, prepare-commit-msg)
//! - Run in compatible contexts (pre-commit, pre-push)
//! - Show warnings during validate
//! - Work correctly with hierarchical configs

use std::fs;
use std::process::Command;
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
fn test_requires_files_skips_in_commit_msg_context() {
    let config = r#"
[hooks.test-hook]
command = "echo 'Running test hook'"
requires_files = true
modifies_repository = false

[groups.commit-msg]
includes = ["test-hook"]
description = "Commit message hooks"
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

    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create a commit message file
    fs::write(repo_path.join("COMMIT_MSG"), "Test commit").unwrap();

    // Run commit-msg hook - should skip because commit-msg can't provide files
    let output = Command::new(peter_hook_bin())
        .args(["run", "commit-msg", "COMMIT_MSG"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Hook should be skipped (not executed)
    // The output should NOT contain "Running test hook"
    assert!(
        !stdout.contains("Running test hook") && !stderr.contains("Running test hook"),
        "Hook with requires_files=true should be skipped in commit-msg context.\nStdout: {stdout}\nStderr: {stderr}"
    );

    // Should succeed (exit 0) because hook was skipped
    assert!(output.status.success(), "Command should succeed when hook is skipped");
}

#[test]
fn test_requires_files_runs_in_pre_commit_context() {
    let config = r#"
[hooks.test-hook]
command = "echo 'Hook executed successfully'"
requires_files = true
modifies_repository = false
files = ["*.txt"]

[groups.pre-commit]
includes = ["test-hook"]
description = "Pre-commit hooks"
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

    // Run pre-commit hook - should execute because pre-commit can provide files
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Hook should execute
    assert!(
        combined.contains("Hook executed successfully"),
        "Hook with requires_files=true should run in pre-commit context.\nOutput: {combined}"
    );

    assert!(output.status.success(), "Hook should succeed");
}

#[test]
fn test_requires_files_with_all_files_flag() {
    let config = r#"
[hooks.test-hook]
command = "echo 'Should not run'"
requires_files = true
modifies_repository = false

[groups.pre-commit]
includes = ["test-hook"]
description = "Pre-commit hooks"
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

    // Run with --all-files flag - should skip because no file list is provided
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit", "--all-files"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Hook should be skipped when --all-files is used
    assert!(
        !stdout.contains("Should not run") && !stderr.contains("Should not run"),
        "Hook with requires_files=true should be skipped with --all-files flag.\nStdout: {stdout}\nStderr: {stderr}"
    );
}

#[test]
fn test_validate_warns_about_incompatible_requires_files() {
    let config = r#"
[hooks.test-hook]
command = "echo 'test'"
requires_files = true
modifies_repository = false

[groups.commit-msg]
includes = ["test-hook"]
description = "Commit message hooks"
"#;

    let temp_dir = setup_test_repo_with_config(config);
    let repo_path = temp_dir.path();

    // Run validate command
    let output = Command::new(peter_hook_bin())
        .args(["validate"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should contain a warning about incompatibility
    assert!(
        combined.contains("requires files") || combined.contains("cannot provide file lists"),
        "Validate should warn about requires_files in incompatible context.\nOutput: {combined}"
    );
}

#[test]
fn test_requires_files_hierarchical_override() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

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

    Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Root config: hook WITHOUT requires_files
    let root_config = r#"
[hooks.test-hook]
command = "echo 'Root hook'"
requires_files = false
modifies_repository = false

[groups.pre-commit]
includes = ["test-hook"]
"#;
    fs::write(repo_path.join("hooks.toml"), root_config).unwrap();

    // Create subdirectory with its own config
    let subdir = repo_path.join("subdir");
    fs::create_dir(&subdir).unwrap();

    // Child config: hook WITH requires_files (overrides parent)
    let child_config = r#"
[hooks.test-hook]
command = "echo 'Child hook'"
requires_files = true
modifies_repository = false
files = ["*.txt"]

[groups.pre-commit]
includes = ["test-hook"]
"#;
    fs::write(subdir.join("hooks.toml"), child_config).unwrap();

    // Create and stage a file in subdirectory
    fs::write(subdir.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "subdir/test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run from subdirectory - should use child config (requires_files=true)
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(&subdir)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should use child's definition (requires_files=true, with file pattern)
    assert!(
        combined.contains("Child hook"),
        "Should use child config with requires_files=true.\nOutput: {combined}"
    );

    assert!(output.status.success(), "Hook should succeed");
}
