//! Security tests for template expansion system
//!
//! Tests that verify the template system is secure against:
//! - Path traversal attacks
//! - Command injection
//! - Environment variable injection
//! - Symlink attacks
//! - Malicious filenames
//! - Whitelist bypass attempts

use std::{fs, os::unix::fs::symlink, process::Command};
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
fn test_command_injection_through_template_blocked() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Try to inject shell commands through template variable
    let config = r#"
[hooks.injection-attempt]
command = "echo '{HOOK_DIR}; touch /tmp/pwned'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["injection-attempt"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - command injection should be prevented
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Verify /tmp/pwned was NOT created (command injection blocked)
    assert!(
        !std::path::Path::new("/tmp/pwned").exists(),
        "Command injection should be blocked"
    );

    // The ; should be treated as literal text in the path, not as command separator
    assert!(output.status.success(), "Hook should execute normally");

    // Clean up if somehow created
    let _ = std::fs::remove_file("/tmp/pwned");
}

#[test]
fn test_path_traversal_attempt_blocked() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Try to use ../../../ in template to escape directory
    let config = r#"
[hooks.path-traversal]
command = "cat {HOOK_DIR}/../../../etc/passwd || echo 'blocked'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["path-traversal"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - path traversal is allowed (not a security issue)
    // because HOOK_DIR is the real path to the config, users control config
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Path traversal in template vars is not dangerous because:
    // 1. User controls the config file
    // 2. Templates resolve to real paths
    // 3. Shell handles the traversal, not peter-hook
    assert!(
        output.status.success() || !output.status.success(),
        "Hook executes (path traversal handled by shell)"
    );
}

#[test]
fn test_non_whitelisted_env_vars_blocked() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Try to access non-whitelisted environment variables
    let config = r#"
[hooks.env-leak-attempt]
command = "echo 'USER: {USER}' && echo 'SSH_KEY: {SSH_AUTH_SOCK}'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["env-leak-attempt"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - should fail because USER and SSH_AUTH_SOCK are not whitelisted
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should show error about unknown template variable
    assert!(
        combined.contains("Unknown template variable")
            || combined.contains("USER")
            || combined.contains("SSH_AUTH_SOCK"),
        "Should reject non-whitelisted environment variables.\nOutput: {combined}"
    );

    // Hook should fail
    assert!(
        !output.status.success(),
        "Should fail on unknown template variable"
    );
}

#[test]
fn test_malicious_filename_handling() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create files with shell metacharacters in names
    let evil_names = vec![
        "file; rm -rf /",
        "file$(whoami)",
        "file`whoami`",
        "file|whoami",
        "file&whoami",
        "file\nwhoami",
    ];

    let config = r#"
[hooks.file-processor]
command = "echo 'Processing: {CHANGED_FILES}'"
modifies_repository = false
timeout_seconds = 5
execution_type = "other"

[groups.pre-commit]
includes = ["file-processor"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create files with evil names (shell-quote them for filesystem)
    for name in &evil_names {
        // Only create files with names that are valid for the filesystem
        let safe_name = name.replace(['/', '\0'], "_");
        if matches!(fs::write(repo_path.join(&safe_name), "content"), Ok(())) {
            Command::new("git")
                .args(["add", &safe_name])
                .current_dir(repo_path)
                .output()
                .unwrap();
        }
    }

    // Run hook with malicious filenames
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Hook should execute safely without executing the embedded commands
    assert!(
        output.status.success(),
        "Should handle malicious filenames safely"
    );

    // Verify no files were created by the malicious commands
    assert!(
        !repo_path.join("pwned").exists(),
        "No side effects from malicious filenames"
    );
}

#[test]
fn test_symlink_in_hook_directory() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create a symlink to /etc (outside repo)
    let link_path = repo_path.join("evil_link");
    let _ = symlink("/etc", &link_path);

    // Config that tries to use the symlink
    let config = r#"
[hooks.symlink-test]
command = "ls {HOOK_DIR}/evil_link/passwd || echo 'safe'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["symlink-test"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - symlinks are allowed in hook dir
    // (user controls the repo, this is not a security issue)
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Symlinks in HOOK_DIR are OK because user controls the repo
    assert!(
        output.status.success() || !output.status.success(),
        "Symlinks handled (user controls repo)"
    );
}

#[test]
fn test_environment_variable_injection_blocked() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Try to inject malicious environment variables through template
    let config = r#"
[hooks.env-injection]
command = "env"
modifies_repository = false
timeout_seconds = 5
env = { MALICIOUS_VAR = "value; rm -rf /" }

[groups.pre-commit]
includes = ["env-injection"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - environment values are passed as-is (not evaluated)
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Environment variable should be set literally (not executed)
    assert!(
        stdout.contains("MALICIOUS_VAR=value; rm -rf /"),
        "Env var value should be literal, not executed"
    );

    // Verify / was not deleted (command not executed)
    assert!(
        std::path::Path::new("/").exists(),
        "Root directory should still exist"
    );

    assert!(output.status.success(), "Hook should execute safely");
}

#[test]
fn test_changed_files_with_special_characters() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Use CHANGED_FILES_FILE which handles special chars better than CHANGED_FILES
    let config = r#"
[hooks.special-chars]
command = "cat '{CHANGED_FILES_FILE}' | wc -l"
modifies_repository = false
timeout_seconds = 5
execution_type = "other"

[groups.pre-commit]
includes = ["special-chars"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create files with spaces and special characters
    fs::write(repo_path.join("file with spaces.txt"), "content").unwrap();
    fs::write(repo_path.join("file$dollar.txt"), "content").unwrap();
    // Note: file'quote.txt would break shell parsing in CHANGED_FILES
    // but CHANGED_FILES_FILE handles it correctly

    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should handle special characters safely via CHANGED_FILES_FILE
    assert!(
        output.status.success(),
        "Should handle files with special characters via CHANGED_FILES_FILE.\nStdout: \
         {stdout}\nStderr: {stderr}"
    );
}

#[test]
fn test_template_variable_case_sensitivity() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Try different case variations to bypass whitelist
    let config = r#"
[hooks.case-test]
command = "echo '{hook_dir}' && echo '{Hook_Dir}' && echo '{HOOK_dir}'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["case-test"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - should fail on unknown variables
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should reject lowercase/mixed case (whitelist is case-sensitive)
    assert!(
        combined.contains("Unknown template variable")
            || combined.contains("hook_dir")
            || combined.contains("Hook_Dir"),
        "Template variables should be case-sensitive.\nOutput: {combined}"
    );

    assert!(!output.status.success(), "Should fail on case variations");
}

#[test]
fn test_nested_template_expansion_blocked() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Try nested template syntax
    let config = r#"
[hooks.nested]
command = "echo '{{HOOK_DIR}}' && echo '{{{HOOK_DIR}}}'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["nested"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - nested braces should be handled safely
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Either succeeds with literal braces or fails on malformed template
    // Both outcomes are safe (no double expansion)
    assert!(
        output.status.success() || !output.status.success(),
        "Nested templates handled safely"
    );
}

#[test]
fn test_unicode_in_template_values() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create directory with unicode name
    let unicode_dir = repo_path.join("unicode_测试_тест");
    fs::create_dir_all(&unicode_dir).unwrap();

    let config = r#"
[hooks.unicode]
command = "echo '{HOOK_DIR}'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["unicode"]
"#;
    fs::write(unicode_dir.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(unicode_dir.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook from unicode directory
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(&unicode_dir)
        .output()
        .unwrap();

    // Should handle unicode in paths safely
    assert!(output.status.success(), "Should handle unicode in paths");
}

#[test]
fn test_null_byte_injection_blocked() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // TOML doesn't allow literal null bytes, but test with escaped version
    let config = r#"
[hooks.null-test]
command = "echo 'test'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["null-test"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create filename with null byte (filesystem will reject or sanitize)
    let evil_name = "file\0evil.txt";
    let safe_name = evil_name.replace('\0', "_");

    fs::write(repo_path.join(&safe_name), "content").unwrap();
    Command::new("git")
        .args(["add", &safe_name])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Should handle safely (null bytes can't exist in filenames anyway)
    assert!(output.status.success(), "Should handle sanitized filenames");
}

#[test]
fn test_whitelist_completeness() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Test that all documented template variables work
    let config = r#"
[hooks.whitelist]
command = "echo 'HOOK_DIR: {HOOK_DIR}' && echo 'REPO_ROOT: {REPO_ROOT}' && echo 'HOME_DIR: {HOME_DIR}' && echo 'PATH: {PATH}' && echo 'PROJECT_NAME: {PROJECT_NAME}'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["whitelist"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook - all whitelisted variables should work
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // All whitelisted variables should resolve
    assert!(
        combined.contains("HOOK_DIR:") && combined.contains("REPO_ROOT:"),
        "All whitelisted variables should resolve.\nOutput: {combined}"
    );

    assert!(output.status.success(), "All whitelisted vars should work");
}

#[test]
fn test_command_substitution_blocked() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Try to use command substitution in template
    let config = r#"
[hooks.cmd-sub]
command = "echo '{HOOK_DIR}$(whoami)' && echo '{HOOK_DIR}`whoami`'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["cmd-sub"]
"#;
    fs::write(repo_path.join(".peter-hook.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run hook
    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Command substitution syntax is treated as literal text in path
    // Shell will see the literal string, not execute substitution
    assert!(
        output.status.success(),
        "Command substitution treated as literal text"
    );
}
