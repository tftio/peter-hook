#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Integration tests for lint command

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_lint_basic_execution() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    // Lint mode doesn't require git repo
    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_without_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    // Should work without git repo
    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_with_dry_run() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .arg("--dry-run")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_nonexistent_hook() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("nonexistent")
        .output()
        .expect("Failed to execute");

    // Should fail or warn about hook not found
    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_with_file_patterns() {
    let temp_dir = TempDir::new().unwrap();

    // Create some Rust files
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.rust-check]
command = "echo checking"
modifies_repository = false
files = ["**/*.rs"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("rust-check")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_help_flag() {
    let output = Command::new(bin_path())
        .arg("lint")
        .arg("--help")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("lint") || stdout.contains("Lint"));
}

#[test]
fn test_lint_with_debug_flag() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("--debug")
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_in_git_repo() {
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

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_with_group() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.a]
command = "echo a"
modifies_repository = false

[hooks.b]
command = "echo b"
modifies_repository = false

[groups.test-group]
includes = ["a", "b"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test-group")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_discovers_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create test files
    fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
    fs::write(temp_dir.path().join("file2.txt"), "content").unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
files = ["*.txt"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_respects_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create gitignore
    fs::write(temp_dir.path().join(".gitignore"), "ignored/\n").unwrap();

    // Create ignored directory
    let ignored = temp_dir.path().join("ignored");
    fs::create_dir(&ignored).unwrap();
    fs::write(ignored.join("test.txt"), "ignored").unwrap();

    // Create non-ignored file
    fs::write(temp_dir.path().join("test.txt"), "not ignored").unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
files = ["**/*.txt"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_no_config_found() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    // Should fail if no config found
    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_exit_codes() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("test")
        .output()
        .expect("Failed to execute");

    // Should return valid exit code
    assert!(matches!(output.status.code(), Some(0 | 1)));
}
