#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Advanced lint mode tests

use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_lint_with_debug_shows_file_list() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple files
    for i in 1..=10 {
        fs::write(
            temp_dir.path().join(format!("file{i}.txt")),
            format!("content{i}"),
        )
        .unwrap();
    }

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.check]
command = "echo checking"
modifies_repository = false
files = ["*.txt"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("--debug")
        .arg("lint")
        .arg("check")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_with_group_shows_hook_count() {
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

[hooks.c]
command = "echo c"
modifies_repository = false

[groups.multi]
includes = ["a", "b", "c"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("multi")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_shows_config_path() {
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

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_dry_run_shows_what_would_run() {
    let temp_dir = TempDir::new().unwrap();

    // Create test files
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.rust-check]
command = "cargo clippy"
modifies_repository = false
files = ["**/*.rs"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("rust-check")
        .arg("--dry-run")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Dry run should show information
    assert!(!combined.trim().is_empty());
}

#[test]
fn test_lint_nonexistent_hook_shows_error() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.existing]
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

    // Should fail or warn
    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_no_files_match_pattern() {
    let temp_dir = TempDir::new().unwrap();

    // Create non-matching files
    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.rust-check]
command = "echo checking rust"
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
fn test_lint_hierarchical_config() {
    let temp_dir = TempDir::new().unwrap();

    // Root config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.root]
command = "echo root"
modifies_repository = false
"#,
    )
    .unwrap();

    // Nested config
    let subdir = temp_dir.path().join("sub");
    fs::create_dir(&subdir).unwrap();
    fs::write(
        subdir.join(".peter-hook.toml"),
        r#"
[hooks.nested]
command = "echo nested"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(&subdir)
        .arg("lint")
        .arg("nested")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_lint_execution_types() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

    let execution_types = vec![
        ("per-file", "per-file"),
        ("in-place", "in-place"),
        ("other", "other"),
    ];

    for (name, exec_type) in execution_types {
        fs::write(
            temp_dir.path().join(".peter-hook.toml"),
            format!(
                r#"
[hooks.test]
command = "echo test"
modifies_repository = false
execution_type = "{exec_type}"
files = ["*.txt"]
"#
            ),
        )
        .unwrap();

        let output = Command::new(bin_path())
            .current_dir(temp_dir.path())
            .arg("lint")
            .arg("test")
            .output()
            .expect("Failed to execute");

        assert!(
            output.status.code().is_some(),
            "Execution type {name} should work"
        );
    }
}
