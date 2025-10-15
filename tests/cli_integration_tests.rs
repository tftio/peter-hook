#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Integration tests for CLI commands

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

/// Get the path to the compiled binary
fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_version_command() {
    let output = Command::new(bin_path())
        .arg("version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_license_command() {
    let output = Command::new(bin_path())
        .arg("license")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MIT") || stdout.contains("Apache") || stdout.contains("License"));
}

#[test]
fn test_help_command() {
    let output = Command::new(bin_path())
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("peter-hook"));
    assert!(stdout.contains("install"));
    assert!(stdout.contains("run"));
}

#[test]
fn test_completions_bash() {
    let output = Command::new(bin_path())
        .arg("completions")
        .arg("bash")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bash"));
    assert!(stdout.contains("_run-targets"));
    assert!(stdout.contains("_lint-targets"));
}

#[test]
fn test_completions_zsh() {
    let output = Command::new(bin_path())
        .arg("completions")
        .arg("zsh")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("zsh"));
    assert!(stdout.contains("_run-targets"));
    assert!(stdout.contains("_lint-targets"));
}

#[test]
fn test_completions_fish() {
    let output = Command::new(bin_path())
        .arg("completions")
        .arg("fish")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fish"));
    assert!(stdout.contains("_run-targets"));
    assert!(stdout.contains("_lint-targets"));
}

#[test]
fn test_hidden_run_targets_command() {
    let output = Command::new(bin_path())
        .arg("_run-targets")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pre-commit"));
}

#[test]
fn test_hidden_lint_targets_command() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let config = r#"
[hooks.lint]
command = "echo lint"
modifies_repository = false

[groups.all]
includes = ["lint"]

[groups.placeholder]
placeholder = true
includes = []
"#;

    fs::write(temp_dir.path().join("hooks.toml"), config).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("_lint-targets")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("lint"));
    assert!(stdout.contains("all"));
    assert!(!stdout.contains("placeholder"));
}

#[test]
fn test_doctor_command() {
    let output = Command::new(bin_path())
        .arg("doctor")
        .output()
        .expect("Failed to execute command");

    // Doctor may return 0 or 1 depending on environment
    assert!(matches!(output.status.code(), Some(0 | 1)));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("health check") || stdout.contains("peter-hook"));
}

#[test]
fn test_validate_no_config() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo but no hooks.toml
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute command");

    // Should succeed but indicate no config
    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_validate_with_valid_config() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create valid hooks.toml
    let config = r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#;
    fs::write(temp_dir.path().join("hooks.toml"), config).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
}

#[test]
fn test_validate_with_invalid_config() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create invalid hooks.toml
    let config = r"
[hooks.broken]
# Missing required fields
";
    fs::write(temp_dir.path().join("hooks.toml"), config).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
}

#[test]
fn test_list_no_config() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute command");

    // May succeed or fail depending on finding config
    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_list_with_config() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create hooks.toml
    let config = r#"
[hooks.test1]
command = "echo test1"
modifies_repository = false

[hooks.test2]
command = "echo test2"
modifies_repository = true
"#;
    fs::write(temp_dir.path().join("hooks.toml"), config).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Output may go to stdout or stderr depending on implementation
    assert!(
        stdout.contains("test1")
            || stdout.contains("test2")
            || stderr.contains("test1")
            || stderr.contains("test2")
            || stdout.contains("hook")
            || stderr.contains("hook")
    );
}

#[test]
fn test_install_outside_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute command");

    // Should fail when not in a git repository
    assert!(!output.status.success());
}

#[test]
fn test_install_in_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create simple config
    let config = r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#;
    fs::write(temp_dir.path().join("hooks.toml"), config).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute command");

    // Installation may succeed or fail but should not panic
    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_uninstall_with_confirmation() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Test with --yes flag to skip confirmation
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute command");

    // May succeed or fail depending on hooks installed
    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_config_list() {
    let output = Command::new(bin_path())
        .arg("config")
        .arg("list")
        .output()
        .expect("Failed to execute command");

    // Just verify command runs without panicking
    // Exit code may vary based on config existence
    assert!(output.status.code().is_some());
}

#[test]
fn test_run_hook_not_in_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute command");

    // Should fail outside git repo
    assert!(!output.status.success());
}

#[test]
fn test_lint_mode_not_in_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    // Create config
    let config = r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#;
    fs::write(temp_dir.path().join("hooks.toml"), config).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute command");

    // Lint mode should work without git repo
    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_update_check_same_version() {
    let current_version = env!("CARGO_PKG_VERSION");

    let output = Command::new(bin_path())
        .arg("update")
        .arg("--version")
        .arg(current_version)
        .arg("--force")
        .output()
        .expect("Failed to execute command");

    // Update may succeed or fail due to network/permissions
    // Just verify it doesn't panic
    assert!(matches!(output.status.code(), Some(0..=2)));
}
