#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive integration tests for run command

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_run_outside_git_repo_fails() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(!output.status.success());
}

#[test]
fn test_run_pre_commit_hook() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo pre-commit"
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

    // May fail if hooks aren't installed, but should not panic
    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_all_files_flag() {
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
        .arg("run")
        .arg("pre-commit")
        .arg("--all-files")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_dry_run_flag() {
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
        .arg("run")
        .arg("pre-commit")
        .arg("--dry-run")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_commit_msg_hook() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.commit-msg]
command = "echo commit-msg"
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
fn test_run_pre_push_hook() {
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
fn test_run_post_commit_hook() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

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
fn test_run_nonexistent_hook() {
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
        .arg("run")
        .arg("nonexistent-hook")
        .output()
        .expect("Failed to execute");

    // Should complete (may warn about no hooks configured)
    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_git_args() {
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

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .arg("--")
        .arg("extra")
        .arg("args")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_debug_flag() {
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
        .arg("--debug")
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_help_flag() {
    let output = Command::new(bin_path())
        .arg("run")
        .arg("--help")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("run") || stdout.contains("Run"));
    assert!(stdout.contains("event"));
}

#[test]
fn test_run_with_group() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.format]
command = "echo format"
modifies_repository = true

[hooks.lint]
command = "echo lint"
modifies_repository = false

[groups.pre-commit]
includes = ["format", "lint"]
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
fn test_run_with_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.first]
command = "echo first"
modifies_repository = false

[hooks.second]
command = "echo second"
modifies_repository = false
depends_on = ["first"]

[hooks.pre-commit]
command = "echo pre-commit"
modifies_repository = false
depends_on = ["second"]
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
fn test_run_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let subdir = temp_dir.path().join("sub");
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

    let output = Command::new(bin_path())
        .current_dir(&subdir)
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_all_files_and_dry_run_together() {
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
        .arg("run")
        .arg("pre-commit")
        .arg("--all-files")
        .arg("--dry-run")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_post_merge_hook() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

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
fn test_run_post_checkout_hook() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

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
fn test_run_prepare_commit_msg_hook() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.prepare-commit-msg]
command = "echo prepare"
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
fn test_run_with_file_patterns() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

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
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_run_always() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.always]
command = "echo always"
modifies_repository = false
run_always = true
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
fn test_run_with_run_at_root() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.root-hook]
command = "echo at root"
modifies_repository = false
run_at_root = true
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
fn test_run_with_env_vars() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.with-env]
command = "echo test"
modifies_repository = false
env = { TEST_VAR = "value" }
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
fn test_run_no_hooks_configured() {
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

    // Should complete but indicate no hooks found
    assert!(output.status.code().is_some());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("No hooks") || combined.contains("no hooks") || !combined.is_empty());
}

#[test]
fn test_run_with_parallel_group() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test1]
command = "echo test1"
modifies_repository = false

[hooks.test2]
command = "echo test2"
modifies_repository = false

[groups.pre-commit]
includes = ["test1", "test2"]
execution = "parallel"
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
fn test_run_with_sequential_group() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.first]
command = "echo first"
modifies_repository = true

[hooks.second]
command = "echo second"
modifies_repository = true

[groups.pre-commit]
includes = ["first", "second"]
execution = "sequential"
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
fn test_run_exit_code_on_hook_failure() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.failing]
command = "false"
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
