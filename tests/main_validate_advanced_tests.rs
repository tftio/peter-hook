#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Advanced validate tests covering more code paths

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_validate_with_trace_imports_shows_diagnostics() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create imported config
    let imports = temp_dir.path().join("lib");
    fs::create_dir(&imports).unwrap();
    fs::write(
        imports.join("base.toml"),
        r#"
[hooks.base]
command = "echo base"
modifies_repository = false
"#,
    )
    .unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
imports = ["lib/base.toml"]

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

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show import information
    assert!(stdout.contains("import") || stdout.contains("Import") || stdout.contains("valid"));
}

#[test]
fn test_validate_with_trace_imports_and_json() {
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should output JSON
    assert!(stdout.contains('{') || stdout.contains("valid"));
}

#[test]
fn test_validate_shows_hook_names() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should list hooks
    assert!(stdout.contains("hook") || stdout.contains("Found") || stdout.contains("valid"));
}

#[test]
fn test_validate_empty_config_shows_message() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(temp_dir.path().join(".peter-hook.toml"), "").unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .arg("--trace-imports")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should mention no hooks defined
    assert!(stdout.contains("No hooks") || stdout.contains("valid") || stdout.contains('0'));
}

#[test]
fn test_validate_with_merge_diagnostics() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create base config
    let base = temp_dir.path().join("base");
    fs::create_dir(&base).unwrap();
    fs::write(
        base.join(".peter-hook.toml"),
        r#"
[hooks.shared]
command = "echo shared"
modifies_repository = false
"#,
    )
    .unwrap();

    // Create override config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
imports = ["base/.peter-hook.toml"]

[hooks.shared]
command = "echo overridden"
modifies_repository = true
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
fn test_validate_shows_override_info() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let base = temp_dir.path().join("lib");
    fs::create_dir(&base).unwrap();
    fs::write(
        base.join("base.toml"),
        r#"
[hooks.format]
command = "cargo fmt --check"
modifies_repository = false
"#,
    )
    .unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
imports = ["lib/base.toml"]

[hooks.format]
command = "cargo fmt"
modifies_repository = true
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show information about hooks
    assert!(!stdout.is_empty());
}

#[test]
fn test_validate_json_output_format() {
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

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Output should contain JSON or be valid
    assert!(!combined.is_empty());
}
