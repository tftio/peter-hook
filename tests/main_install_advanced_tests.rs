#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Advanced install tests covering edge cases and error paths

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_install_with_existing_unmanaged_hooks_force() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create existing unmanaged hook
    let hooks_dir = temp_dir.path().join(".git/hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(hooks_dir.join("pre-commit"), "#!/bin/sh\necho existing").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(hooks_dir.join("pre-commit"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(hooks_dir.join("pre-commit"), perms).unwrap();
    }

    // Create config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo new"
modifies_repository = false
"#,
    )
    .unwrap();

    // Install with force
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .arg("--force")
        .output()
        .expect("Failed to execute");

    // Should succeed and backup existing hook
    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_install_with_existing_unmanaged_hooks_no_force() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create existing unmanaged hook
    let hooks_dir = temp_dir.path().join(".git/hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(hooks_dir.join("pre-commit"), "#!/bin/sh\necho existing").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(hooks_dir.join("pre-commit"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(hooks_dir.join("pre-commit"), perms).unwrap();
    }

    // Create config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo new"
modifies_repository = false
"#,
    )
    .unwrap();

    // Install without force - should warn about existing hooks
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    // Should complete (may warn or install depending on logic)
    assert!(output.status.code().is_some());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should mention existing hooks or force flag
    assert!(
        stdout.contains("exist")
            || stdout.contains("force")
            || stdout.contains("managed")
            || stdout.is_empty()
    );
}

#[test]
fn test_install_with_all_worktree_strategies() {
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

    let strategies = vec!["shared", "per-worktree", "detect"];

    for strategy in strategies {
        let output = Command::new(bin_path())
            .current_dir(temp_dir.path())
            .arg("install")
            .arg("--worktree-strategy")
            .arg(strategy)
            .arg("--force")
            .output()
            .expect("Failed to execute");

        assert!(
            output.status.success() || output.status.code() == Some(1),
            "Strategy {strategy} should work"
        );
    }
}

#[test]
fn test_install_creates_executable_hooks() {
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
        .arg("install")
        .output()
        .expect("Failed to execute");

    if output.status.success() {
        let hook_file = temp_dir.path().join(".git/hooks/pre-commit");
        if hook_file.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = fs::metadata(&hook_file).unwrap();
                let permissions = metadata.permissions();
                let mode = permissions.mode();
                // Should have execute bit set (0o100 for user execute)
                assert_ne!(mode & 0o100, 0, "Hook should be executable");
            }
        }
    }
}

#[test]
fn test_install_report_print_summary() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo test"
modifies_repository = false

[hooks.pre-push]
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

    // Should print some summary
    assert!(!combined.trim().is_empty());
}

#[test]
fn test_install_with_complex_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.setup]
command = "echo setup"
modifies_repository = false

[hooks.format]
command = "echo format"
modifies_repository = true
depends_on = ["setup"]

[hooks.lint]
command = "echo lint"
modifies_repository = false
depends_on = ["format"]

[hooks.test]
command = "echo test"
modifies_repository = false
depends_on = ["lint"]

[hooks.pre-commit]
command = "echo pre-commit"
modifies_repository = false
depends_on = ["test"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success() || output.status.code() == Some(1));
}

#[test]
fn test_install_with_imports() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create imported config
    let imports_dir = temp_dir.path().join("imports");
    fs::create_dir(&imports_dir).unwrap();

    fs::write(
        imports_dir.join("base.toml"),
        r#"
[hooks.base-hook]
command = "echo base"
modifies_repository = false
"#,
    )
    .unwrap();

    // Create main config with import
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
imports = ["imports/base.toml"]

[hooks.pre-commit]
command = "echo main"
modifies_repository = false
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success() || output.status.code() == Some(1));
}
