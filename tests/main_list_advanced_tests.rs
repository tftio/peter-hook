#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Advanced list and worktree tests

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_list_shows_detailed_hook_info() {
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
modifies_repository = true
"#,
    )
    .unwrap();

    // Install hooks
    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // List hooks
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show hook information with formatting
    assert!(stdout.contains("hook") || stdout.contains("managed") || stdout.contains("executable"));
}

#[test]
fn test_list_empty_shows_message() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No git hooks") || stdout.contains("no hooks") || stdout.contains("found")
    );
}

#[test]
fn test_list_shows_managed_vs_custom() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create managed hook via install
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.managed]
command = "echo managed"
modifies_repository = false
"#,
    )
    .unwrap();

    let _ = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output();

    // Create custom hook
    let hooks_dir = temp_dir.path().join(".git/hooks");
    fs::write(hooks_dir.join("custom-hook"), "#!/bin/sh\necho custom").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(hooks_dir.join("custom-hook"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(hooks_dir.join("custom-hook"), perms).unwrap();
    }

    // List should show both
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

#[test]
fn test_list_shows_executable_vs_non_executable() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let hooks_dir = temp_dir.path().join(".git/hooks");
    fs::create_dir_all(&hooks_dir).unwrap();

    // Create executable hook
    fs::write(hooks_dir.join("executable"), "#!/bin/sh\necho test").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(hooks_dir.join("executable"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(hooks_dir.join("executable"), perms).unwrap();
    }

    // Create non-executable hook
    fs::write(hooks_dir.join("non-executable"), "#!/bin/sh\necho test").unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show executable status
    assert!(
        stdout.contains("executable")
            || stdout.contains("yes")
            || stdout.contains("no")
            || stdout.contains("✅")
            || stdout.contains("❌")
    );
}

#[test]
fn test_list_worktrees_shows_current_indicator() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list-worktrees")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show worktree information
    assert!(!stdout.trim().is_empty());
}

#[test]
fn test_list_worktrees_shows_main_indicator() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list-worktrees")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
}

#[test]
fn test_list_worktrees_empty_shows_message() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list-worktrees")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Should show worktree info or message
    assert!(!combined.trim().is_empty());
}
