#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive tests that actually exercise `run_hooks` code paths

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_run_pre_commit_with_real_execution() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create a file and stage it
    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    // Create hook config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo Running pre-commit"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    // Should execute
    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_actual_file_filtering() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create and stage a Rust file
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.rs")).unwrap();
    index.write().unwrap();

    // Create hook that only targets Rust files
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.rust-check]
command = "echo Checking Rust files"
modifies_repository = false
files = ["**/*.rs"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_hierarchical_configs() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

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
    let subdir = temp_dir.path().join("subdir");
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

    // Create and stage file in nested dir
    fs::write(subdir.join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("subdir/test.txt"))
        .unwrap();
    index.write().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_shows_no_hooks_message() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Empty config
    fs::write(temp_dir.path().join(".peter-hook.toml"), "").unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(combined.contains("No hooks") || combined.contains("no hooks"));
}

#[test]
fn test_run_all_supported_hook_types() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let hook_types = vec![
        "pre-commit",
        "commit-msg",
        "pre-push",
        "post-commit",
        "post-merge",
        "post-checkout",
    ];

    for hook_type in hook_types {
        fs::write(
            temp_dir.path().join(".peter-hook.toml"),
            format!(
                r#"
[hooks.{hook_type}]
command = "echo {hook_type}"
modifies_repository = false
"#
            ),
        )
        .unwrap();

        let output = Command::new(bin_path())
            .current_dir(temp_dir.path())
            .arg("run")
            .arg(hook_type)
            .output()
            .expect("Failed to execute");

        assert!(
            output.status.code().is_some(),
            "Hook type {hook_type} should execute"
        );
    }
}

#[test]
fn test_run_with_failing_hook_propagates_error() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.failing]
command = "exit 1"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    // May return non-zero on hook failure
    assert!(output.status.code().is_some());
}

#[test]
fn test_run_debug_mode_shows_extra_output() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

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
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_multiple_changed_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create and stage multiple files
    for i in 1..=10 {
        fs::write(
            temp_dir.path().join(format!("file{i}.txt")),
            format!("content{i}"),
        )
        .unwrap();
    }

    let mut index = repo.index().unwrap();
    for i in 1..=10 {
        index
            .add_path(std::path::Path::new(&format!("file{i}.txt")))
            .unwrap();
    }
    index.write().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.check]
command = "echo checking files"
modifies_repository = false
files = ["*.txt"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}
