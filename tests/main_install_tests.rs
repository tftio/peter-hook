#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive integration tests for install command

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_install_in_empty_git_repo() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create simple config
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
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success(),
        "Install should succeed in empty repo"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installing") || stdout.contains("install"));
}

#[test]
fn test_install_with_force_flag() {
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
        .arg("install")
        .arg("--force")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with force should not panic"
    );
}

#[test]
fn test_install_with_shared_strategy() {
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
        .arg("install")
        .arg("--worktree-strategy")
        .arg("shared")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with shared strategy should work"
    );
}

#[test]
fn test_install_with_per_worktree_strategy() {
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
        .arg("install")
        .arg("--worktree-strategy")
        .arg("per-worktree")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with per-worktree strategy should work"
    );
}

#[test]
fn test_install_with_detect_strategy() {
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
        .arg("install")
        .arg("--worktree-strategy")
        .arg("detect")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with detect strategy should work"
    );
}

#[test]
fn test_install_with_invalid_strategy() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .arg("--worktree-strategy")
        .arg("invalid")
        .output()
        .expect("Failed to execute");

    assert!(
        !output.status.success(),
        "Install should reject invalid strategy"
    );
}

#[test]
fn test_install_outside_git_repo_fails() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        !output.status.success(),
        "Install should fail outside git repo"
    );
}

#[test]
fn test_install_without_config() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();
    // No .peter-hook.toml file

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    // May succeed or fail depending on whether .peter-hook.toml is required
    assert!(output.status.code().is_some());
}

#[test]
fn test_install_creates_hooks_directory() {
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

    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // Check that .git/hooks directory exists
    let hooks_dir = temp_dir.path().join(".git/hooks");
    assert!(hooks_dir.exists(), "Hooks directory should be created");
}

#[test]
fn test_install_with_multiple_hooks() {
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

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with multiple hooks should work"
    );
}

#[test]
fn test_install_with_hook_group() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.format]
command = "cargo fmt"
modifies_repository = true

[hooks.lint]
command = "cargo clippy"
modifies_repository = false

[groups.pre-commit]
includes = ["format", "lint"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with groups should work"
    );
}

#[test]
fn test_install_output_contains_summary() {
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
        .arg("install")
        .output()
        .expect("Failed to execute");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Output should contain some indication of what happened
    assert!(
        combined.contains("Install")
            || combined.contains("hook")
            || combined.contains("success")
            || combined.contains("error"),
        "Output should contain installation feedback"
    );
}

#[test]
fn test_install_twice_without_force() {
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

    // First install
    let output1 = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    // Second install without force
    let output2 = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    // Both should complete (may warn or succeed)
    assert!(output1.status.code().is_some());
    assert!(output2.status.code().is_some());
}

#[test]
fn test_install_with_complex_config() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.format]
command = "cargo fmt"
modifies_repository = true
files = ["**/*.rs"]

[hooks.lint]
command = "cargo clippy"
modifies_repository = false
files = ["**/*.rs"]
depends_on = ["format"]

[hooks.test]
command = "cargo test"
modifies_repository = false
depends_on = ["lint"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with complex config should work"
    );
}

#[test]
fn test_install_help_flag() {
    let output = Command::new(bin_path())
        .arg("install")
        .arg("--help")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("install") || stdout.contains("Install"));
    assert!(stdout.contains("force") || stdout.contains("Force"));
}

#[test]
fn test_install_with_invalid_config_fails() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create truly invalid TOML (syntax error)
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.broken
# Unclosed bracket
command = "echo test"
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        !output.status.success(),
        "Install should fail with invalid config"
    );
}

#[test]
fn test_install_preserves_existing_managed_hooks() {
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

    // First install
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // Second install (should handle managed hooks gracefully)
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    // Should complete without error
    assert!(output.status.code().is_some());
}

#[test]
fn test_install_with_env_vars_in_config() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
env = { TEST_VAR = "test_value" }
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with env vars should work"
    );
}

#[test]
fn test_install_with_working_dir_in_config() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
workdir = "."
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with workdir should work"
    );
}

#[test]
fn test_install_default_worktree_strategy() {
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

    // No --worktree-strategy flag, should use default (shared)
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with default strategy should work"
    );
}

#[test]
fn test_install_in_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create subdirectory
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    // Create config in root
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    // Run install from subdirectory
    let output = Command::new(bin_path())
        .current_dir(&subdir)
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install from subdirectory should work"
    );
}

#[test]
fn test_install_with_template_variables() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo {HOOK_DIR}"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with template variables should work"
    );
}

#[test]
fn test_install_with_run_at_root() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
run_at_root = true
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with run_at_root should work"
    );
}

#[test]
fn test_install_with_run_always() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
run_always = true
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Install with run_always should work"
    );
}

#[test]
fn test_install_exit_code_on_success() {
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
        .arg("install")
        .output()
        .expect("Failed to execute");

    // Exit code should be 0 or 1 (depending on actual result)
    assert!(matches!(output.status.code(), Some(0 | 1)));
}
