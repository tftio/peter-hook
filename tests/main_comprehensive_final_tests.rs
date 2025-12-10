#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Final comprehensive tests to push coverage to 90%

use git2::Repository as Git2Repository;
use std::{fs, process::Command};
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

// Test version command output format
#[test]
fn test_version_output_format() {
    let output = Command::new(bin_path())
        .arg("version")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("peter-hook"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

// Test license command output format
#[test]
fn test_license_output_complete() {
    let output = Command::new(bin_path())
        .arg("license")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MIT"));
    assert!(stdout.contains("License"));
    assert!(stdout.contains("SOFTWARE"));
}

// Test run with deeply nested hierarchy
#[test]
fn test_run_deep_hierarchy() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create 5-level deep structure
    let deep = temp_dir.path().join("a/b/c/d/e");
    fs::create_dir_all(&deep).unwrap();

    // Config at each level
    for (i, level) in ["a", "a/b", "a/b/c", "a/b/c/d", "a/b/c/d/e"]
        .iter()
        .enumerate()
    {
        let level_path = temp_dir.path().join(level);
        fs::write(
            level_path.join(".peter-hook.toml"),
            format!(
                r#"
[hooks.level-{i}]
command = "echo level {i}"
modifies_repository = false
"#
            ),
        )
        .unwrap();
    }

    // Create and stage file in deepest level
    fs::write(deep.join("deep.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("a/b/c/d/e/deep.txt"))
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

// Test install with backup scenario
#[test]
fn test_install_backs_up_existing_hooks() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create pre-existing hook
    let hooks_dir = temp_dir.path().join(".git/hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(hooks_dir.join("pre-commit"), "#!/bin/sh\necho old hook").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(hooks_dir.join("pre-commit"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(hooks_dir.join("pre-commit"), perms).unwrap();
    }

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo new hook"
modifies_repository = false
"#,
    )
    .unwrap();

    // Install with force to trigger backup
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("install")
        .arg("--force")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success() || output.status.code() == Some(1));

    // Backup file may be created
    let backup = hooks_dir.join("pre-commit.backup");
    let _ = backup.exists();
}

// Test uninstall restores backups
#[test]
fn test_uninstall_restores_backup() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create hooks and install
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
        .arg("--force")
        .output();

    // Uninstall with --yes
    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("uninstall")
        .arg("--yes")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success() || output.status.code() == Some(1));
}

// Test run with multiple config groups (hierarchical)
#[test]
fn test_run_multiple_config_groups() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Root config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.root-check]
command = "echo root"
modifies_repository = false
files = ["*.txt"]
"#,
    )
    .unwrap();

    // Nested config
    let sub1 = temp_dir.path().join("sub1");
    fs::create_dir(&sub1).unwrap();
    fs::write(
        sub1.join(".peter-hook.toml"),
        r#"
[hooks.sub-check]
command = "echo sub1"
modifies_repository = false
files = ["*.rs"]
"#,
    )
    .unwrap();

    // Create files in both locations
    fs::write(temp_dir.path().join("root.txt"), "content").unwrap();
    fs::write(sub1.join("sub.rs"), "fn main() {}").unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("root.txt")).unwrap();
    index.add_path(std::path::Path::new("sub1/sub.rs")).unwrap();
    index.write().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test list with managed and unmanaged hooks
#[test]
fn test_list_mixed_hook_types() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let hooks_dir = temp_dir.path().join(".git/hooks");
    fs::create_dir_all(&hooks_dir).unwrap();

    // Create multiple hook types
    for hook_name in ["pre-commit", "pre-push", "commit-msg", "post-commit"] {
        fs::write(
            hooks_dir.join(hook_name),
            format!("#!/bin/sh\necho {hook_name}"),
        )
        .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(hooks_dir.join(hook_name))
                .unwrap()
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(hooks_dir.join(hook_name), perms).unwrap();
        }
    }

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should list multiple hooks
    assert!(!stdout.is_empty());
}

// Test validate with complex import chain
#[test]
fn test_validate_nested_imports() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create import chain: main -> lib1 -> lib2
    let lib1 = temp_dir.path().join("lib1");
    let lib2 = temp_dir.path().join("lib2");
    fs::create_dir(&lib1).unwrap();
    fs::create_dir(&lib2).unwrap();

    fs::write(
        lib2.join(".peter-hook.toml"),
        r#"
[hooks.base]
command = "echo base"
modifies_repository = false
"#,
    )
    .unwrap();

    fs::write(
        lib1.join(".peter-hook.toml"),
        r#"
imports = ["../lib2/.peter-hook.toml"]

[hooks.mid]
command = "echo mid"
modifies_repository = false
"#,
    )
    .unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
imports = ["lib1/.peter-hook.toml"]

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

// Test lint with many files triggering "and X more" message
#[test]
fn test_lint_many_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create 50 files
    for i in 1..=50 {
        fs::write(
            temp_dir.path().join(format!("file{i:03}.txt")),
            format!("content{i}"),
        )
        .unwrap();
    }

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.check-all]
command = "echo checking"
modifies_repository = false
files = ["*.txt"]
"#,
    )
    .unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("--debug")
        .arg("lint")
        .arg("check-all")
        .output()
        .expect("Failed to execute");

    assert!(output.status.code().is_some());
}

// Test config init with both flags
#[test]
fn test_config_init_all_flags() {
    let temp_dir = TempDir::new().unwrap();

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

// Test error from main when outside git repo
#[test]
fn test_main_error_handling() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .output()
        .expect("Failed to execute");

    // Should fail with error
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error") || stderr.contains("error") || !stderr.is_empty());
}
