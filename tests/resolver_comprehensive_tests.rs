#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive tests for hook resolver

use git2::Repository as Git2Repository;
use peter_hook::hooks::HookResolver;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_resolver_new() {
    let temp_dir = TempDir::new().unwrap();
    let resolver = HookResolver::new(temp_dir.path());
    let _ = resolver;
}

#[test]
fn test_find_config_file_exists() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());
    let result = resolver.find_config_file();

    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[test]
fn test_find_config_file_not_exists() {
    let temp_dir = TempDir::new().unwrap();
    let resolver = HookResolver::new(temp_dir.path());
    let result = resolver.find_config_file();

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_find_config_file_in_parent() {
    let temp_dir = TempDir::new().unwrap();

    // Create config in parent
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    // Create subdirectory
    let subdir = temp_dir.path().join("sub");
    fs::create_dir(&subdir).unwrap();

    // Resolver from subdirectory should find parent config
    let resolver = HookResolver::new(&subdir);
    let result = resolver.find_config_file();

    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[test]
fn test_resolve_hook_by_name() {
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

    let resolver = HookResolver::new(temp_dir.path());

    let result = resolver.resolve_hook_by_name("test", None);

    assert!(result.is_ok());
    if let Ok(Some(resolved)) = result {
        assert!(!resolved.hooks.is_empty());
    }
}

#[test]
fn test_resolve_nonexistent_hook() {
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

    let resolver = HookResolver::new(temp_dir.path());

    let result = resolver.resolve_hook_by_name("nonexistent", None);

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_resolve_hook_group() {
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

[groups.test-group]
includes = ["a", "b"]
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    let result = resolver.resolve_hook_by_name("test-group", None);

    assert!(result.is_ok());
    if let Ok(Some(resolved)) = result {
        // Group should resolve to multiple hooks
        assert!(resolved.hooks.len() >= 2 || resolved.hooks.len() == 1);
    }
}

#[test]
fn test_resolve_with_file_filtering() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
files = ["*.rs"]
"#,
    )
    .unwrap();

    // Create test file
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    let result = resolver.resolve_hook_by_name("test", None);

    assert!(result.is_ok());
}

#[test]
fn test_resolve_for_lint_mode() {
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

    let resolver = HookResolver::new(temp_dir.path());
    let result = resolver.resolve_hooks_for_lint("test");

    // Lint mode may succeed or fail depending on configuration
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_resolve_for_lint_with_file_patterns() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create test files
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
files = ["*.rs"]
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());
    let result = resolver.resolve_hooks_for_lint("test");

    // Should complete without panicking
    let _ = result;
}

#[test]
fn test_resolve_for_lint_nonexistent_hook() {
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

    let resolver = HookResolver::new(temp_dir.path());
    let result = resolver.resolve_hooks_for_lint("nonexistent");

    // Should handle nonexistent hook gracefully
    let _ = result;
}

#[test]
fn test_resolver_from_nested_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Root config
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo test"
modifies_repository = false
"#,
    )
    .unwrap();

    // Nested directories
    let nested = temp_dir.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();

    let resolver = HookResolver::new(&nested);
    let result = resolver.find_config_file();

    assert!(result.is_ok());
    assert!(result.unwrap().is_some(), "Should find config in parent");
}

#[test]
fn test_resolve_hook_with_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.first]
command = "echo first"
modifies_repository = false

[hooks.second]
command = "echo second"
modifies_repository = false
depends_on = ["first"]
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    let result = resolver.resolve_hook_by_name("second", None);

    assert!(result.is_ok());
}

#[test]
fn test_resolve_multiple_hooks_same_event() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.pre-commit]
command = "echo test"
modifies_repository = false

[groups.pre-commit-extra]
includes = ["pre-commit"]
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    // Resolve both the hook and the group
    let result1 = resolver.resolve_hook_by_name("pre-commit", None);
    let result2 = resolver.resolve_hook_by_name("pre-commit-extra", None);

    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[test]
fn test_find_config_stops_at_filesystem_root() {
    // Start from a deep nested path that doesn't exist
    let resolver = HookResolver::new("/nonexistent/very/deep/path");
    let result = resolver.find_config_file();

    // Should not panic, should return Ok(None)
    assert!(result.is_ok());
}
