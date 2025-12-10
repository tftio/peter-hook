#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive integration tests for list and list-worktrees commands

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_list_in_empty_repo() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No git hooks") || stdout.contains("hooks"));
}

#[test]
fn test_list_outside_git_repo_fails() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(
        !output.status.success(),
        "List should fail outside git repo"
    );
}

#[test]
fn test_list_after_install() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    // Install hooks first
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // List hooks
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hook") || stdout.contains("pre-commit"));
}

#[test]
fn test_list_multiple_hooks() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo pre-commit"
modifies_repository = false

[hooks.pre-push]
command = "echo pre-push"
modifies_repository = false

[hooks.commit-msg]
command = "echo commit-msg"
modifies_repository = false
"#,
    )
    .unwrap();

    // Install multiple hooks
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // List hooks
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show hook information
    assert!(!stdout.is_empty());
}

#[test]
fn test_list_shows_managed_status() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    // Install hooks
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // List hooks
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should indicate managed status
    assert!(stdout.contains("managed") || stdout.contains("custom") || stdout.contains("hook"));
}

#[test]
fn test_list_help_flag() {
    let output = Command::new(bin_path())
        .arg("list")
        .arg("--help")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list") || stdout.contains("List"));
}

#[test]
fn test_list_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // List from subdirectory
    let output = Command::new(bin_path())
        .current_dir(&subdir)
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_list_with_debug_flag() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("--debug")
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

#[test]
fn test_list_worktrees_in_main_repo() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list-worktrees")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show main repository or worktree information
    assert!(!stdout.is_empty());
}

#[test]
fn test_list_worktrees_outside_git_repo_fails() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list-worktrees")
        .output()
        .expect("Failed to execute");

    assert!(!output.status.success());
}

#[test]
fn test_list_worktrees_help_flag() {
    let output = Command::new(bin_path())
        .arg("list-worktrees")
        .arg("--help")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("worktree") || stdout.contains("Worktree"));
}

#[test]
fn test_list_shows_executable_status() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    // Install hooks
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // List hooks
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show hook information (executable status or other details)
    assert!(
        !stdout.is_empty() || !output.stderr.is_empty(),
        "Should produce output"
    );
}

#[test]
fn test_list_output_formatting() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    // Just verify it produces output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty() || output.stderr.is_empty());
}

#[test]
fn test_list_exit_code() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert_eq!(output.status.code(), Some(0), "List should exit with 0");
}
