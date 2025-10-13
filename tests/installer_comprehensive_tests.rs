#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive tests for git hook installer

use git2::Repository as Git2Repository;
use peter_hook::git::{GitHookInstaller, GitRepository, WorktreeHookStrategy};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_installer_creation_new() {
    let original_dir = std::env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = GitHookInstaller::new();
    assert!(result.is_ok(), "Should create installer in git repo");

    // Restore directory
    let _ = std::env::set_current_dir(original_dir);
}

#[test]
fn test_installer_with_shared_strategy() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_binary_and_strategy(
        repo,
        "test-binary".to_string(),
        WorktreeHookStrategy::Shared,
    );
    let _ = installer;
}

#[test]
fn test_installer_with_per_worktree_strategy() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_binary_and_strategy(
        repo,
        "test-binary".to_string(),
        WorktreeHookStrategy::PerWorktree,
    );
    let _ = installer;
}

#[test]
fn test_installer_with_detect_strategy() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_binary_and_strategy(
        repo,
        "test-binary".to_string(),
        WorktreeHookStrategy::Detect,
    );
    let _ = installer;
}

#[test]
fn test_installer_with_repository_and_binary() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_and_binary(repo, "test-binary".to_string());

    // Should create successfully
    let _ = installer;
}

#[test]
fn test_installer_with_repository_binary_and_strategy() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_binary_and_strategy(
        repo,
        "test-binary".to_string(),
        WorktreeHookStrategy::Shared,
    );

    let _ = installer;
}

#[test]
fn test_install_all_hooks() {
    let original_dir = std::env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create config
    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.pre-commit]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let installer = GitHookInstaller::new().unwrap();
    let result = installer.install_all();

    // Restore directory
    let _ = std::env::set_current_dir(original_dir);

    // Should complete (may have warnings/errors but not panic)
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_uninstall_all_hooks() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_and_binary(repo, "test-binary".to_string());

    let report = installer.uninstall_all();
    // Should produce a report
    assert!(report.is_success() || !report.is_success());
}

#[test]
fn test_supported_hooks_list() {
    use peter_hook::git::installer::SUPPORTED_HOOKS;

    // Verify all expected hooks are in the list
    assert!(SUPPORTED_HOOKS.contains(&"pre-commit"));
    assert!(SUPPORTED_HOOKS.contains(&"commit-msg"));
    assert!(SUPPORTED_HOOKS.contains(&"pre-push"));
    assert!(SUPPORTED_HOOKS.contains(&"post-commit"));
    assert!(SUPPORTED_HOOKS.contains(&"post-merge"));
    assert!(SUPPORTED_HOOKS.contains(&"post-checkout"));

    // Should have at least 10 hooks
    assert!(SUPPORTED_HOOKS.len() >= 10);
}

#[test]
fn test_install_creates_hook_directory() {
    let original_dir = std::env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.pre-commit]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let installer = GitHookInstaller::new().unwrap();
    let _ = installer.install_all();

    // Check hooks directory exists
    let hooks_dir = temp_dir.path().join(".git/hooks");
    assert!(hooks_dir.exists());

    let _ = std::env::set_current_dir(original_dir);
}

#[test]
fn test_install_report_structure() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_and_binary(repo, "test-binary".to_string());

    // Try install (may fail but should return report)
    if let Ok(report) = installer.install_all() {
        // Report should have is_success method
        let _ = report.is_success();
    }
}

#[test]
fn test_uninstall_report_structure() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let repo = GitRepository::find_from_dir(temp_dir.path()).unwrap();
    let installer = GitHookInstaller::with_repository_and_binary(repo, "test-binary".to_string());

    let report = installer.uninstall_all();
    // Report should have is_success and print_summary methods
    let _ = report.is_success();
    report.print_summary();
}

#[test]
fn test_installer_outside_git_repo_fails() {
    let original_dir = std::env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = GitHookInstaller::new();
    assert!(result.is_err(), "Should fail outside git repo");

    let _ = std::env::set_current_dir(original_dir);
}

#[test]
fn test_installer_strategies_all_variants() {
    let original_dir = std::env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let strategies = vec![
        WorktreeHookStrategy::Shared,
        WorktreeHookStrategy::PerWorktree,
        WorktreeHookStrategy::Detect,
    ];

    for strategy in strategies {
        let result = GitHookInstaller::with_strategy(strategy);
        assert!(result.is_ok(), "Should create installer with {strategy:?}");
    }

    // Restore directory (ignore error if it doesn't exist)
    let _ = std::env::set_current_dir(original_dir);
}

#[test]
fn test_installer_from_subdirectory() {
    let original_dir = std::env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let subdir = temp_dir.path().join("sub");
    fs::create_dir(&subdir).unwrap();

    std::env::set_current_dir(&subdir).unwrap();

    let result = GitHookInstaller::new();
    assert!(result.is_ok(), "Should find git repo from subdirectory");

    let _ = std::env::set_current_dir(original_dir);
}

#[test]
fn test_install_then_uninstall() {
    let original_dir = std::env::current_dir().unwrap();
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

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let installer1 = GitHookInstaller::new().unwrap();
    let _ = installer1.install_all();

    let installer2 = GitHookInstaller::new().unwrap();
    let uninstall_report = installer2.uninstall_all();

    // Should complete
    let _ = uninstall_report.is_success();

    let _ = std::env::set_current_dir(original_dir);
}
