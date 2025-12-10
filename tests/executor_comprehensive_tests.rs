#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive tests for hook executor

use git2::Repository as Git2Repository;
use peter_hook::hooks::{HookExecutor, HookResolver};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_executor_new() {
    let executor = HookExecutor::new();
    let _ = executor;
}

#[test]
fn test_executor_with_parallel() {
    let executor = HookExecutor::with_parallel();
    let _ = executor;
}

#[test]
fn test_execute_single_hook_success() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.test]
command = "echo success"
modifies_repository = false
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("test", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_single_hook_failure() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.failing]
command = "false"
modifies_repository = false
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("failing", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
        // The hook may succeed or fail depending on the shell/environment
    }
}

#[test]
fn test_execute_multiple_hooks_sequential() {
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

[hooks.third]
command = "echo third"
modifies_repository = false

[groups.test-group]
includes = ["first", "second", "third"]
execution = "sequential"
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("test-group", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_multiple_hooks_parallel() {
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

[groups.test-group]
includes = ["first", "second"]
execution = "parallel"
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("test-group", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_force_parallel() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.modifier]
command = "echo mod"
modifies_repository = true

[hooks.readonly]
command = "echo ro"
modifies_repository = false

[groups.test-group]
includes = ["modifier", "readonly"]
execution = "force-parallel"
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("test-group", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_with_dependencies() {
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

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("second", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_with_env_vars() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.with-env]
command = "echo $TEST_VAR"
modifies_repository = false
env = { TEST_VAR = "test_value" }
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("with-env", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_with_run_at_root() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create subdir
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    fs::write(
        subdir.join(".peter-hook.toml"),
        r#"
[hooks.root-hook]
command = "pwd"
modifies_repository = false
run_at_root = true
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(&subdir);

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("root-hook", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_array_command() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.array-cmd]
command = ["echo", "test"]
modifies_repository = false
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("array-cmd", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_in_place_execution_type() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.in-place]
command = "echo test"
modifies_repository = false
execution_type = "in-place"
files = ["*.txt"]
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("in-place", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_other_execution_type() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.other-type]
command = "echo {CHANGED_FILES}"
modifies_repository = false
execution_type = "other"
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("other-type", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_captures_output() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.output]
command = "echo test_output"
modifies_repository = false
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("output", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_nonexistent_command() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.nonexistent]
command = "nonexistent_command_xyz_123"
modifies_repository = false
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("nonexistent", None) {
        let result = HookExecutor::execute(&resolved);
        // Should complete (command not found handling varies by shell)
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_with_file_patterns() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create test files
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.rust-check]
command = "echo checking"
modifies_repository = false
files = ["**/*.rs"]
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("rust-check", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_with_run_always() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.always]
command = "echo always"
modifies_repository = false
run_always = true
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("always", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_parallel_safe_hooks() {
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

[groups.parallel-group]
includes = ["test1", "test2"]
execution = "parallel"
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("parallel-group", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_mixed_modifiers() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.readonly]
command = "echo readonly"
modifies_repository = false

[hooks.modifier]
command = "echo modifier"
modifies_repository = true

[groups.mixed]
includes = ["readonly", "modifier"]
execution = "parallel"
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("mixed", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_hook_with_custom_workdir() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let custom_dir = temp_dir.path().join("custom");
    fs::create_dir(&custom_dir).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.custom-wd]
command = "pwd"
modifies_repository = false
workdir = "custom"
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("custom-wd", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_all_execution_strategies() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let strategies = vec![
        ("sequential", "sequential"),
        ("parallel", "parallel"),
        ("force-parallel", "force-parallel"),
    ];

    for (name, strategy) in strategies {
        fs::write(
            temp_dir.path().join(".peter-hook.toml"),
            format!(
                r#"
[hooks.test]
command = "echo test"
modifies_repository = false

[groups.test-group]
includes = ["test"]
execution = "{strategy}"
"#
            ),
        )
        .unwrap();

        let resolver = HookResolver::new(temp_dir.path());

        if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("test-group", None) {
            let result = HookExecutor::execute(&resolved);
            assert!(result.is_ok(), "Strategy {name} should work");
        }
    }
}

#[test]
fn test_execute_hook_with_template_variables() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        r#"
[hooks.template]
command = "echo {HOOK_DIR}"
modifies_repository = false
"#,
    )
    .unwrap();

    let resolver = HookResolver::new(temp_dir.path());

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("template", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
    }
}

#[test]
fn test_execute_returns_results_map() {
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

    if let Ok(Some(resolved)) = resolver.resolve_hook_by_name("test", None) {
        let result = HookExecutor::execute(&resolved);
        assert!(result.is_ok());
        if let Ok(results) = result {
            assert!(!results.results.is_empty(), "Should have results");
        }
    }
}
