#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive tests for hierarchical hook resolution

use git2::Repository as Git2Repository;
use peter_hook::{
    git::ChangeDetectionMode,
    hooks::{WorktreeContext, resolve_hooks_hierarchically},
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_hierarchical_simple_config() {
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

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "pre-commit",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_nested_configs() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Root config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo root"
modifies_repository = false
"#,
    )
    .unwrap();

    // Nested config
    let subdir = temp_dir.path().join("sub");
    fs::create_dir(&subdir).unwrap();
    fs::write(
        subdir.join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo nested"
modifies_repository = false
"#,
    )
    .unwrap();

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "pre-commit",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_no_config() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "pre-commit",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    // Should return Ok but empty groups
    assert!(result.is_ok());
    if let Ok(groups) = result {
        assert!(groups.is_empty());
    }
}

#[test]
fn test_hierarchical_with_working_directory_mode() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
files = ["*.txt"]
"#,
    )
    .unwrap();

    // Create test file
    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "test",
        Some(ChangeDetectionMode::WorkingDirectory),
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_with_staged_mode() {
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

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "pre-commit",
        Some(ChangeDetectionMode::Staged),
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_three_level_nesting() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Level 1 (root)
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo root"
modifies_repository = false
"#,
    )
    .unwrap();

    // Level 2
    let l2 = temp_dir.path().join("level2");
    fs::create_dir(&l2).unwrap();
    fs::write(
        l2.join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo level2"
modifies_repository = false
"#,
    )
    .unwrap();

    // Level 3
    let l3 = l2.join("level3");
    fs::create_dir(&l3).unwrap();
    fs::write(
        l3.join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo level3"
modifies_repository = false
"#,
    )
    .unwrap();

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "test",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_with_groups() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.a]
command = "echo a"
modifies_repository = false

[hooks.b]
command = "echo b"
modifies_repository = false

[groups.pre-commit]
includes = ["a", "b"]
"#,
    )
    .unwrap();

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "pre-commit",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_nonexistent_event() {
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

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "nonexistent",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    // Should return Ok with empty groups
    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_in_worktree() {
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

    let worktree_context = WorktreeContext {
        is_worktree: true,
        worktree_name: Some("feature-branch".to_string()),
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "test",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}

#[test]
fn test_hierarchical_multiple_configs_same_level() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create two subdirectories with configs
    let sub1 = temp_dir.path().join("sub1");
    let sub2 = temp_dir.path().join("sub2");
    fs::create_dir(&sub1).unwrap();
    fs::create_dir(&sub2).unwrap();

    fs::write(
        sub1.join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo sub1"
modifies_repository = false
"#,
    )
    .unwrap();

    fs::write(
        sub2.join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo sub2"
modifies_repository = false
"#,
    )
    .unwrap();

    let worktree_context = WorktreeContext {
        is_worktree: false,
        worktree_name: None,
        repo_root: temp_dir.path().to_path_buf(),
        common_dir: temp_dir.path().join(".git"),
        working_dir: temp_dir.path().to_path_buf(),
    };

    let result = resolve_hooks_hierarchically(
        "test",
        None,
        temp_dir.path(),
        temp_dir.path(),
        &worktree_context,
    );

    assert!(result.is_ok());
}
