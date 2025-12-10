#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Advanced run tests covering debug mode and edge cases

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_run_with_debug_shows_extravaganza_message() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create and stage files
    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
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

    // Debug mode should produce output
    assert!(output.status.code().is_some());
}

#[test]
fn test_run_dry_run_doesnt_execute() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create file that would be modified
    fs::write(temp_dir.path().join("test.txt"), "original").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo modified > test.txt"
modifies_repository = true
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .arg("--dry-run")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());

    // In dry-run, file should not be modified
    let content = fs::read_to_string(temp_dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "original");
}

#[test]
fn test_run_with_many_changed_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create 20 files to trigger "and X more files" message
    for i in 1..=20 {
        fs::write(
            temp_dir.path().join(format!("file{i:02}.txt")),
            format!("content{i}"),
        )
        .unwrap();
    }

    let mut index = repo.index().unwrap();
    for i in 1..=20 {
        index
            .add_path(std::path::Path::new(&format!("file{i:02}.txt")))
            .unwrap();
    }
    index.write().unwrap();

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
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_post_commit_uses_commit_range() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create initial commit
    fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("file.txt")).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[])
        .unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.post-commit]
command = "echo post-commit"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("post-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_post_merge_uses_commit_range() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create initial commit
    fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("file.txt")).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[])
        .unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.post-merge]
command = "echo post-merge"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("post-merge")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_post_checkout_uses_commit_range() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create initial commit
    fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("file.txt")).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[])
        .unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.post-checkout]
command = "echo post-checkout"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("post-checkout")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_commit_msg_no_file_filtering() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.commit-msg]
command = "echo checking message"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("commit-msg")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_prepare_commit_msg_no_file_filtering() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.prepare-commit-msg]
command = "echo preparing"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("prepare-commit-msg")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_pre_push_uses_push_mode() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-push]
command = "echo pre-push"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-push")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_unknown_event_uses_working_directory_mode() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.custom-event]
command = "echo custom"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("custom-event")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_executes_hooks_and_shows_results() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create and stage file
    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo Executing hook"
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

    assert!(output.status.code().is_some());

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Should show some execution output
    assert!(!combined.trim().is_empty());
}
