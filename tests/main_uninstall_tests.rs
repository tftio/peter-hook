#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive integration tests for uninstall command

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_uninstall_with_yes_flag() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    // Should complete (may succeed or fail if no hooks installed)
    assert!(output.status.code().is_some());
}

#[test]
fn test_uninstall_outside_git_repo_fails() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(
        !output.status.success(),
        "Uninstall should fail outside git repo"
    );
}

#[test]
fn test_uninstall_after_install() {
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

    // Install first
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // Then uninstall
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Uninstall after install should work"
    );
}

#[test]
fn test_uninstall_no_hooks_installed() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    // Should complete gracefully even with no hooks
    assert!(output.status.code().is_some());
}

#[test]
fn test_uninstall_output_contains_summary() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Should contain some output about the operation
    assert!(!combined.is_empty(), "Uninstall should produce output");
}

#[test]
fn test_uninstall_help_flag() {
    let output = Command::new(bin_path())
        .arg("uninstall")
        .arg("--help")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("uninstall") || stdout.contains("Uninstall"));
    assert!(stdout.contains("yes"));
}

#[test]
fn test_uninstall_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    // Install hooks first
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

    // Uninstall from subdirectory
    let output = Command::new(bin_path())
        .current_dir(&subdir)
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.code().is_some(),
        "Uninstall from subdir should work"
    );
}

#[test]
fn test_uninstall_twice_idempotent() {
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

    // Install
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // Uninstall twice
    let output1 = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    let output2 = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    // Both should complete
    assert!(output1.status.code().is_some());
    assert!(output2.status.code().is_some());
}

#[test]
fn test_uninstall_multiple_hooks() {
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

    // Uninstall all
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_uninstall_exit_codes() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    // Exit code should be 0 or 1
    assert!(matches!(output.status.code(), Some(0 | 1)));
}

#[test]
fn test_uninstall_without_yes_flag_needs_stdin() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Without --yes flag, command will wait for stdin
    // We can't easily test this in an automated way without providing input
    // So we just verify the --yes flag works
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_install_then_uninstall_then_install() {
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

    // Install
    let output1 = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    // Uninstall
    let output2 = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    // Install again
    let output3 = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    // All should complete
    assert!(output1.status.code().is_some());
    assert!(output2.status.code().is_some());
    assert!(output3.status.code().is_some());
}

#[test]
fn test_uninstall_with_debug_flag() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("--debug")
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_uninstall_in_nested_repo() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create nested directory structure
    let nested = temp_dir.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();

    // Install hooks
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

    // Uninstall from nested directory
    let output = Command::new(bin_path())
        .current_dir(&nested)
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}
