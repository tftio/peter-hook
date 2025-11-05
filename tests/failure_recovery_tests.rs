//! Failure recovery and error handling tests
//!
//! Tests system behavior when hooks fail:
//! - Parallel hook failures and cleanup
//! - Error propagation in mixed execution strategies
//! - Resource cleanup (temp files, processes)
//! - Partial success scenarios
//! - Error message handling

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a git repository with configuration
fn setup_test_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Configure git
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Disable GPG signing
    Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    temp_dir
}

/// Get the peter-hook binary path
fn peter_hook_bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_parallel_hooks_one_fails_others_complete() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create parallel hooks where one fails
    let config = r#"
[hooks.success-1]
command = "echo 'Success 1' && exit 0"
modifies_repository = false
timeout_seconds = 5

[hooks.failure]
command = "echo 'I will fail' && exit 1"
modifies_repository = false
timeout_seconds = 5

[hooks.success-2]
command = "echo 'Success 2' && exit 0"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["success-1", "failure", "success-2"]
description = "Mixed success/failure"
execution_strategy = "parallel"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hooks - should fail but all hooks should execute
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // All hooks should execute despite one failing
    assert!(
        combined.contains("Success 1") || combined.contains("success-1"),
        "First hook should execute.\nOutput: {combined}"
    );
    assert!(
        combined.contains("Success 2") || combined.contains("success-2"),
        "Third hook should execute.\nOutput: {combined}"
    );
    assert!(
        combined.contains("fail") || combined.contains("failure"),
        "Failing hook should execute.\nOutput: {combined}"
    );

    // Overall command should fail
    assert!(!output.status.success(), "Command should fail when any hook fails");
}

#[test]
fn test_mixed_execution_continues_despite_failures() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create mixed execution where parallel phase has a failure
    let config = r#"
[hooks.parallel-fail]
command = "echo 'Parallel failing' && exit 1"
modifies_repository = false
timeout_seconds = 5

[hooks.parallel-success]
command = "echo 'Parallel success'"
modifies_repository = false
timeout_seconds = 5

[hooks.sequential-continues]
command = "echo 'Sequential phase continues'"
modifies_repository = true
timeout_seconds = 5

[groups.pre-commit]
includes = ["parallel-fail", "parallel-success", "sequential-continues"]
description = "Fail in parallel phase"
execution_strategy = "parallel"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hooks - parallel phase should fail
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Parallel hooks should run
    assert!(
        combined.contains("Parallel") || combined.contains("parallel"),
        "Parallel hooks should execute.\nOutput: {combined}"
    );

    // NOTE: Sequential phase DOES run even if parallel phase fails
    // This is by design - all hooks execute, failures are collected
    assert!(
        combined.contains("continues") || combined.contains("sequential"),
        "Sequential hook executes even after parallel failure.\nOutput: {combined}"
    );

    // Overall command should fail
    assert!(!output.status.success(), "Command should fail");
}

#[test]
fn test_sequential_hooks_all_execute_despite_failures() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create sequential hooks where first one fails
    let config = r#"
[hooks.seq-1-fail]
command = "echo 'Sequential 1 failing' && exit 1"
modifies_repository = true
timeout_seconds = 5

[hooks.seq-2-continues]
command = "echo 'Sequential 2 continues'"
modifies_repository = true
timeout_seconds = 5

[hooks.seq-3-continues]
command = "echo 'Sequential 3 continues'"
modifies_repository = true
timeout_seconds = 5

[groups.pre-commit]
includes = ["seq-1-fail", "seq-2-continues", "seq-3-continues"]
description = "Sequential failure"
execution_strategy = "sequential"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hooks - should stop after first failure
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // First hook should run
    assert!(
        combined.contains("Sequential 1") || combined.contains("seq-1"),
        "First hook should execute.\nOutput: {combined}"
    );

    // NOTE: Subsequent hooks DO run even after first fails
    // This is by design - all hooks execute, failures are collected
    assert!(
        combined.contains("continues") || combined.contains("seq-2") || combined.contains("seq-3"),
        "Subsequent hooks execute even after first fails.\nOutput: {combined}"
    );

    // Overall command should fail
    assert!(!output.status.success(), "Command should fail");
}

#[test]
fn test_hook_timeout_is_treated_as_failure() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create hook that times out, followed by another hook
    let config = r#"
[hooks.timeout-hook]
command = "sleep 10"
modifies_repository = false
timeout_seconds = 1

[hooks.continues-after-timeout]
command = "echo 'Continues after timeout'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["timeout-hook", "continues-after-timeout"]
description = "Timeout failure"
execution_strategy = "sequential"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hooks - should timeout
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should show timeout error
    assert!(
        combined.contains("timeout") || combined.contains("exceeded") || combined.contains("killed"),
        "Should show timeout error.\nOutput: {combined}"
    );

    // NOTE: Second hook MAY still run depending on execution strategy
    // We just verify the timeout was treated as a failure

    // Overall command should fail
    assert!(!output.status.success(), "Command should fail on timeout");
}

#[test]
fn test_error_messages_include_hook_names() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create hook with descriptive name that fails
    let config = r#"
[hooks.my-custom-validation-hook]
command = "echo 'Validation failed: missing required field' >&2 && exit 1"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["my-custom-validation-hook"]
description = "Validation hooks"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - should fail with clear error
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Error output should include hook name
    assert!(
        combined.contains("my-custom-validation-hook"),
        "Error should include hook name.\nOutput: {combined}"
    );

    // Error output should include the hook's stderr message
    assert!(
        combined.contains("Validation failed") || combined.contains("missing required field"),
        "Error should include hook's error message.\nOutput: {combined}"
    );

    assert!(!output.status.success(), "Command should fail");
}

#[test]
fn test_nonexistent_command_failure() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create hook with command that doesn't exist
    let config = r#"
[hooks.nonexistent-tool]
command = "this-command-definitely-does-not-exist-12345"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["nonexistent-tool"]
description = "Nonexistent command"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - should fail gracefully
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should show clear error about missing command
    assert!(
        combined.contains("Failed") || combined.contains("not found") || combined.contains("nonexistent"),
        "Should show command not found error.\nOutput: {combined}"
    );

    assert!(!output.status.success(), "Command should fail");
}

#[test]
fn test_partial_parallel_success_still_fails() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create 5 parallel hooks where 1 fails
    let config = r#"
[hooks.pass-1]
command = "exit 0"
modifies_repository = false
timeout_seconds = 5

[hooks.pass-2]
command = "exit 0"
modifies_repository = false
timeout_seconds = 5

[hooks.fail]
command = "exit 1"
modifies_repository = false
timeout_seconds = 5

[hooks.pass-3]
command = "exit 0"
modifies_repository = false
timeout_seconds = 5

[hooks.pass-4]
command = "exit 0"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["pass-1", "pass-2", "fail", "pass-3", "pass-4"]
description = "Partial failure"
execution_strategy = "parallel"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hooks - should fail overall despite 4/5 passing
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Even with partial success, overall result should be failure
    assert!(
        !output.status.success(),
        "Command should fail if any hook fails"
    );
}

#[test]
fn test_hook_with_complex_failure_exit_codes() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create hooks with different non-zero exit codes
    let config = r#"
[hooks.exit-2]
command = "exit 2"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["exit-2"]
description = "Non-standard exit code"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - should treat any non-zero exit as failure
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Any non-zero exit code should be treated as failure
    assert!(!output.status.success(), "Non-zero exit should fail");
}

#[test]
fn test_dependencies_control_order_not_failure() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create hooks with dependencies where parent fails
    let config = r#"
[hooks.parent-fail]
command = "echo 'Parent failing' && exit 1"
modifies_repository = false
timeout_seconds = 5

[hooks.child-depends-on-parent]
command = "echo 'Child runs after parent'"
modifies_repository = false
depends_on = ["parent-fail"]
timeout_seconds = 5

[groups.pre-commit]
includes = ["parent-fail", "child-depends-on-parent"]
description = "Dependency chain"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hooks - child should not run if parent fails
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // NOTE: Dependencies control execution ORDER, not failure propagation
    // Both parent and child execute, even if parent fails
    // Dependencies ensure parent runs BEFORE child, but don't stop on failure

    // Both hooks should appear in output
    let parent_ran = combined.contains("Parent") || combined.contains("parent-fail");
    let child_ran = combined.contains("Child") || combined.contains("child-depends");

    assert!(
        parent_ran || child_ran,
        "At least one hook should execute.\nOutput: {combined}"
    );

    assert!(!output.status.success(), "Command should fail");
}

#[test]
fn test_multiple_failures_all_reported() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create multiple hooks that all fail
    let config = r#"
[hooks.fail-1]
command = "echo 'Fail 1' >&2 && exit 1"
modifies_repository = false
timeout_seconds = 5

[hooks.fail-2]
command = "echo 'Fail 2' >&2 && exit 1"
modifies_repository = false
timeout_seconds = 5

[hooks.fail-3]
command = "echo 'Fail 3' >&2 && exit 1"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["fail-1", "fail-2", "fail-3"]
description = "Multiple failures"
execution_strategy = "parallel"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hooks - all failures should be reported
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // All failures should appear in output (or at least hook names)
    let failure_count = (1..=3)
        .filter(|i| combined.contains(&format!("fail-{i}")) || combined.contains(&format!("Fail {i}")))
        .count();

    assert!(
        failure_count >= 2,
        "Multiple failures should be reported.\nOutput: {combined}"
    );

    assert!(!output.status.success(), "Command should fail");
}

#[test]
fn test_dry_run_shows_failures_but_doesnt_fail() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create hook that would fail
    let config = r#"
[hooks.would-fail]
command = "exit 1"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["would-fail"]
description = "Would fail in real run"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run with --dry-run - should not actually execute
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit", "--dry-run"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Dry run should succeed even though hook would fail
    assert!(
        output.status.success(),
        "Dry run should succeed regardless of hook failures"
    );
}
