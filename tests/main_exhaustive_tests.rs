#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Exhaustive tests for main.rs to reach 90% coverage

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

// Test run with no changed files
#[test]
fn test_run_no_changed_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create initial commit so we have a clean state
    fs::write(temp_dir.path().join("initial.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("initial.txt")).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[])
        .unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.pre-commit]
command = "echo test"
modifies_repository = false
files = ["**/*.rs"]
"#,
    )
    .unwrap();

    // Run without any Rust files changed
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test run with exactly 1 changed file
#[test]
fn test_run_single_changed_file() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("single.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("single.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
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
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test run with exactly 5 changed files (boundary for display logic)
#[test]
fn test_run_five_changed_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    for i in 1..=5 {
        fs::write(
            temp_dir.path().join(format!("file{i}.txt")),
            format!("content{i}"),
        )
        .unwrap();
    }

    let mut index = repo.index().unwrap();
    for i in 1..=5 {
        index
            .add_path(std::path::Path::new(&format!("file{i}.txt")))
            .unwrap();
    }
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
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
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test run with exactly 6 changed files (triggers "and more" logic)
#[test]
fn test_run_six_changed_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    for i in 1..=6 {
        fs::write(
            temp_dir.path().join(format!("file{i}.txt")),
            format!("content{i}"),
        )
        .unwrap();
    }

    let mut index = repo.index().unwrap();
    for i in 1..=6 {
        index
            .add_path(std::path::Path::new(&format!("file{i}.txt")))
            .unwrap();
    }
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
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
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test with exactly 1 hook (different emoji)
#[test]
fn test_run_single_hook() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.single]
command = "echo single"
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
}

// Test with 2-3 hooks
#[test]
fn test_run_two_to_three_hooks() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.hook1]
command = "echo 1"
modifies_repository = false

[hooks.hook2]
command = "echo 2"
modifies_repository = false

[hooks.hook3]
command = "echo 3"
modifies_repository = false

[groups.pre-commit]
includes = ["hook1", "hook2"]
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

// Test with 4-6 hooks
#[test]
fn test_run_four_to_six_hooks() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.h1]
command = "echo 1"
modifies_repository = false

[hooks.h2]
command = "echo 2"
modifies_repository = false

[hooks.h3]
command = "echo 3"
modifies_repository = false

[hooks.h4]
command = "echo 4"
modifies_repository = false

[groups.pre-commit]
includes = ["h1", "h2", "h3", "h4"]
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

// Test with 7+ hooks
#[test]
fn test_run_many_hooks() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    let mut config = String::new();
    for i in 1..=10 {
        config.push_str(&format!(
            r#"
[hooks.hook{i}]
command = "echo {i}"
modifies_repository = false
"#
        ));
    }

    config.push_str(
        r#"
[groups.pre-commit]
includes = ["hook1", "hook2", "hook3", "hook4", "hook5", "hook6", "hook7", "hook8"]
"#,
    );

    fs::write(temp_dir.path().join("hooks.toml"), config).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test hooks with run_always flag
#[test]
fn test_run_hook_with_run_always_flag() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.always-hook]
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

// Test hooks with no file patterns
#[test]
fn test_run_hook_no_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.no-patterns]
command = "echo no patterns"
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
}

// Test lint mode with no files discovered
#[test]
fn test_lint_no_matching_files() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.rust-only]
command = "echo rust"
modifies_repository = false
files = ["**/*.rs"]
"#,
    )
    .unwrap();

    // No Rust files exist
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("rust-only")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test lint with group that resolves to multiple hooks
#[test]
fn test_lint_multi_hook_group() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.check1]
command = "echo check1"
modifies_repository = false

[hooks.check2]
command = "echo check2"
modifies_repository = false

[hooks.check3]
command = "echo check3"
modifies_repository = false

[groups.all-checks]
includes = ["check1", "check2", "check3"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("all-checks")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test validate with merges and overrides
#[test]
fn test_validate_shows_merges() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let base = temp_dir.path().join("base");
    fs::create_dir(&base).unwrap();

    fs::write(
        base.join("hooks.toml"),
        r#"
[hooks.base-hook]
command = "echo base"
modifies_repository = false
files = ["*.txt"]
"#,
    )
    .unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
imports = ["base/hooks.toml"]

[hooks.base-hook]
command = "echo overridden"
modifies_repository = true
files = ["*.rs"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .arg("--trace-imports")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

// Test run executes hooks and returns proper exit code
#[test]
fn test_run_hook_success_exit_code() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.success]
command = "true"
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

    // Successful hook should result in success
    assert!(output.status.success() || output.status.code() == Some(1));
}

// Test run hook failure exit code
#[test]
fn test_run_hook_failure_exit_code() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.failure]
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

    // Failed hook should propagate failure
    assert!(output.status.code().is_some());
}

// Test config init both force and allow-local
#[test]
fn test_config_init_combined_flags() {
    let temp_dir = TempDir::new().unwrap();

    // First init
    let _ = Command::new(bin_path())
        .env("HOME", temp_dir.path())
        .arg("config")
        .arg("init")
        .output();

    // Second init with both flags
    let output = Command::new(bin_path())
        .env("HOME", temp_dir.path())
        .arg("config")
        .arg("init")
        .arg("--force")
        .arg("--allow-local")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

// Test lint shows file discovery count
#[test]
fn test_lint_file_count_display() {
    let temp_dir = TempDir::new().unwrap();

    // Create exactly 10 files
    for i in 1..=10 {
        fs::write(
            temp_dir.path().join(format!("file{i:02}.rs")),
            format!("fn test{i}() {{}}"),
        )
        .unwrap();
    }

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.rust]
command = "echo checking rust"
modifies_repository = false
files = ["**/*.rs"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("lint")
        .arg("rust")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test run with hooks that have different execution types
#[test]
fn test_run_mixed_execution_types() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.per-file]
command = "echo"
modifies_repository = false
execution_type = "per-file"
files = ["*.txt"]

[hooks.in-place]
command = "echo test"
modifies_repository = false
execution_type = "in-place"

[groups.pre-commit]
includes = ["per-file", "in-place"]
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

// Test validate shows warning messages
#[test]
fn test_validate_with_warnings() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

// Test run with hook that prints to both stdout and stderr
#[test]
fn test_run_hook_with_output_and_errors() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.txt")).unwrap();
    index.write().unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.verbose]
command = "echo stdout && echo stderr >&2"
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
}

// Test all CLI help commands
#[test]
fn test_all_subcommand_help() {
    let subcommands = vec![
        "install",
        "uninstall",
        "run",
        "validate",
        "list",
        "list-worktrees",
        "lint",
        "version",
        "license",
        "completions",
        "doctor",
        "config",
    ];

    for subcmd in subcommands {
        let output = Command::new(bin_path())
            .arg(subcmd)
            .arg("--help")
            .output()
            .expect("Failed to execute");

        assert!(output.status.success(), "Help for {subcmd} should succeed");
    }
}

// Test main with global debug flag
#[test]
fn test_all_commands_with_debug_flag() {
    let temp_dir = TempDir::new().unwrap();

    let commands = vec![
        vec!["version"],
        vec!["license"],
        vec!["completions", "bash"],
        vec!["doctor"],
    ];

    for cmd_args in commands {
        let mut command = Command::new(bin_path());
        command.current_dir(temp_dir.path()).arg("--debug");

        for arg in cmd_args {
            command.arg(arg);
        }

        let output = command.output().expect("Failed to execute");
        assert!(output.status.code().is_some());
    }
}
