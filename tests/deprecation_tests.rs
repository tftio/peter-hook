#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Tests for deprecated hooks.toml configuration file detection

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

/// Helper to create a test repository with a hooks.toml file
fn create_repo_with_deprecated_config() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create deprecated hooks.toml
    let config_path = temp_dir.path().join("hooks.toml");
    fs::write(
        &config_path,
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n",
    )
    .unwrap();

    (temp_dir, config_path)
}

#[test]
fn test_deprecation_error_on_single_file() {
    let (temp_dir, _) = create_repo_with_deprecated_config();

    // Try to run any command (except version/license)
    let output = Command::new(bin_path())
        .arg("validate")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should exit with error
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check error message contains key information
    assert!(stderr.contains("hooks.toml is no longer supported"));
    assert!(stderr.contains(".peter-hook.toml"));
    assert!(stderr.contains("hooks.toml")); // Should list the file
    assert!(stderr.contains("mv hooks.toml .peter-hook.toml")); // Should show fix
}

#[test]
fn test_deprecation_error_lists_multiple_files() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create multiple deprecated hooks.toml files
    fs::write(
        temp_dir.path().join("hooks.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n",
    )
    .unwrap();

    fs::create_dir_all(temp_dir.path().join("backend")).unwrap();
    fs::write(
        temp_dir.path().join("backend/hooks.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n",
    )
    .unwrap();

    fs::create_dir_all(temp_dir.path().join("frontend")).unwrap();
    fs::write(
        temp_dir.path().join("frontend/hooks.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n",
    )
    .unwrap();

    // Try to run validate
    let output = Command::new(bin_path())
        .arg("validate")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should list all three files
    assert!(stderr.contains("hooks.toml"));
    assert!(stderr.contains("backend/hooks.toml") || stderr.contains("backend\\hooks.toml"));
    assert!(stderr.contains("frontend/hooks.toml") || stderr.contains("frontend\\hooks.toml"));
}

#[test]
fn test_version_command_bypasses_deprecation_check() {
    let (temp_dir, _) = create_repo_with_deprecated_config();

    // Version command should work even with deprecated config
    let output = Command::new(bin_path())
        .arg("version")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should succeed
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("peter-hook")); // Should show version
}

#[test]
fn test_license_command_bypasses_deprecation_check() {
    let (temp_dir, _) = create_repo_with_deprecated_config();

    // License command should work even with deprecated config
    let output = Command::new(bin_path())
        .arg("license")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should succeed
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MIT") || stdout.contains("Apache")); // Should show license
}

#[test]
fn test_new_config_name_works() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create NEW config with correct name
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n",
    )
    .unwrap();

    // Validate should work
    let output = Command::new(bin_path())
        .arg("validate")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should succeed or show "Configuration is valid"
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should NOT contain deprecation error
    assert!(!stderr.contains("hooks.toml is no longer supported"));
    assert!(!stdout.contains("hooks.toml is no longer supported"));
}
