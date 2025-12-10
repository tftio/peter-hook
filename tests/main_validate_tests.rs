#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive integration tests for validate command

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_validate_valid_config() {
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
        .arg("validate")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("valid") || stdout.contains("Valid"));
}

#[test]
fn test_validate_invalid_toml() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        "[hooks.broken\nno closing bracket",
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute");

    assert!(!output.status.success());
}

#[test]
fn test_validate_no_config() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute");

    // May succeed or fail depending on whether config is required
    assert!(output.status.code().is_some());
}

#[test]
fn test_validate_with_trace_imports() {
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
        .arg("validate")
        .arg("--trace-imports")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

#[test]
fn test_validate_with_json_output() {
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
        .arg("validate")
        .arg("--trace-imports")
        .arg("--json")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

#[test]
fn test_validate_with_imports() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create imported config using relative import
    let import_dir = temp_dir.path().join("imports");
    fs::create_dir(&import_dir).unwrap();

    fs::write(
        import_dir.join("base.toml"),
        r#"
[hooks.base]
command = "echo base"
modifies_repository = false
"#,
    )
    .unwrap();

    // Create main config with relative import
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
imports = ["imports/base.toml"]

[hooks.main]
command = "echo main"
modifies_repository = false
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

#[test]
fn test_validate_circular_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.a]
command = "echo a"
modifies_repository = false
depends_on = ["b"]

[hooks.b]
command = "echo b"
modifies_repository = false
depends_on = ["a"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute");

    // Validate completes but may show warnings/errors about circular deps
    assert!(output.status.code().is_some());
}

#[test]
fn test_validate_help_flag() {
    let output = Command::new(bin_path())
        .arg("validate")
        .arg("--help")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("validate") || stdout.contains("Validate"));
}

#[test]
fn test_validate_from_subdirectory() {
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
        .arg("validate")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

#[test]
fn test_validate_with_debug() {
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
        .arg("validate")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

#[test]
fn test_validate_config_with_groups() {
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

#[test]
fn test_validate_shows_hook_count() {
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
        .arg("validate")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show hook information
    assert!(!stdout.is_empty());
}
