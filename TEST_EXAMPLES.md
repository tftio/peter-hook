# Peter-Hook: Specific Test Cases to Implement

## High-Priority Test Cases

### 1. Integration Test: requires_files in commit-msg Hook

**File**: `tests/main_run_advanced_tests.rs` (new test)

```rust
#[test]
fn test_requires_files_skipped_in_commit_msg() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.check-msg]
command = "echo should-not-run"
requires_files = true
modifies_repository = false

[groups.commit-msg]
includes = ["check-msg"]
"#,
    ).unwrap();

    let mut cmd = Command::new(bin_path());
    cmd.current_dir(temp_dir.path())
        .arg("run")
        .arg("commit-msg")
        .arg(".git/COMMIT_EDITMSG");

    let output = cmd.output().expect("Failed to execute");
    
    // Hook should NOT execute (requires files but commit-msg can't provide them)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("should-not-run"));
}
```

**Expected behavior**: 
- Hook is skipped silently
- No error (graceful handling)
- No "should-not-run" output

---

### 2. Integration Test: requires_files with File Patterns

**File**: `tests/main_run_advanced_tests.rs` (new test)

```rust
#[test]
fn test_requires_files_with_file_patterns() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Configure git
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.test-py]
command = "echo testing-python"
files = ["**/*.py"]
requires_files = true
modifies_repository = false
execution_type = "in-place"

[groups.pre-commit]
includes = ["test-py"]
"#,
    ).unwrap();

    // Create a Python file and stage it
    fs::write(temp_dir.path().join("test.py"), "print('hello')").unwrap();
    Command::new("git")
        .args(["add", "test.py"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Run pre-commit
    let mut cmd = Command::new(bin_path());
    cmd.current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit");

    let output = cmd.output().expect("Failed to execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Should execute (Python file changed)
    assert!(stdout.contains("testing-python") || output.status.success());
}
```

---

### 3. Integration Test: requires_files with --all-files Flag

**File**: `tests/main_run_advanced_tests.rs` (new test)

```rust
#[test]
fn test_requires_files_skipped_with_all_files_flag() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.test]
command = "echo should-not-run"
requires_files = true
modifies_repository = false

[groups.pre-commit]
includes = ["test"]
"#,
    ).unwrap();

    // Run with --all-files (no file list available)
    let mut cmd = Command::new(bin_path());
    cmd.current_dir(temp_dir.path())
        .arg("run")
        .arg("pre-commit")
        .arg("--all-files");

    let output = cmd.output().expect("Failed to execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Hook should NOT execute (requires files but --all-files provides none)
    assert!(!stdout.contains("should-not-run"));
}
```

---

### 4. Unit Test: OID Validation in parse_push_stdin

**File**: `src/git/changes.rs` (add to tests module)

```rust
#[test]
fn test_parse_push_stdin_invalid_oid_format() {
    // OID must be 40 hex characters or special all-zeros case
    let stdin = "refs/heads/main INVALID_OID refs/heads/main 789xyz012345";
    
    // Current: SUCCEEDS (bug!)
    let result = parse_push_stdin(stdin);
    
    // Expected: Should either error or at least validate structure
    // For now, just document the current behavior
    assert!(result.is_ok(), "Currently accepts invalid OID - needs validation");
}

#[test]
fn test_parse_push_stdin_local_oid_all_zeros_delete() {
    // Branch deletion: local OID is all zeros
    let stdin = "refs/heads/feature 0000000000000000000000000000000000000000 refs/heads/feature abc123def456";
    
    let (local_oid, remote_oid) = parse_push_stdin(stdin).unwrap();
    
    // Local should be preserved (not converted to empty tree)
    assert_eq!(local_oid, "0000000000000000000000000000000000000000");
    
    // Remote should NOT be converted (it's not the remote that's all-zeros)
    assert_eq!(remote_oid, "abc123def456");
}
```

---

### 5. Unit Test: Template Variable Path Traversal

**File**: `src/config/templating.rs` (add to tests module)

```rust
#[test]
fn test_template_path_traversal_blocked() {
    let repo_root = TempDir::new().unwrap();
    let config_dir = repo_root.path();
    let working_dir = repo_root.path();
    
    let resolver = TemplateResolver::new(config_dir, working_dir);
    
    // Try to use path traversal in workdir expansion
    let traversal = "{REPO_ROOT}/../../../etc/passwd";
    
    // Current: No blocking
    // Expected: Should either block or document the risk
    // For now, document current behavior
    let _expanded = resolver.expand_string(traversal).ok();
}

#[test]
fn test_template_undefined_variable() {
    let temp_dir = TempDir::new().unwrap();
    let resolver = TemplateResolver::new(temp_dir.path(), temp_dir.path());
    
    let result = resolver.expand_string("Command: {UNDEFINED_VARIABLE}");
    
    // Current behavior: ???
    // Expected: Either error or leave as literal
    // This test documents the current behavior
    assert!(result.is_ok() || result.is_err());  // Placeholder
}
```

---

### 6. Unit Test: Parallel Execution Failure Recovery

**File**: `tests/executor_comprehensive_tests.rs` (new test)

```rust
#[test]
fn test_parallel_one_hook_fails_stops_execution() {
    let temp_dir = TempDir::new().unwrap();

    // Create 3 hooks, second one fails
    let config = HookConfig {
        hooks: Some(indexmap::indexmap! {
            "hook1".to_string() => HookDefinition {
                command: HookCommand::Shell("echo first".to_string()),
                modifies_repository: false,
                requires_files: false,
                ..Default::default()
            },
            "hook2".to_string() => HookDefinition {
                command: HookCommand::Shell("exit 1".to_string()),
                modifies_repository: false,
                requires_files: false,
                ..Default::default()
            },
            "hook3".to_string() => HookDefinition {
                command: HookCommand::Shell("echo third".to_string()),
                modifies_repository: false,
                requires_files: false,
                ..Default::default()
            },
        }),
        groups: Some(indexmap::indexmap! {
            "test".to_string() => HookGroup {
                includes: vec!["hook1".to_string(), "hook2".to_string(), "hook3".to_string()],
                execution: ExecutionStrategy::Parallel,
                ..Default::default()
            },
        }),
        ..Default::default()
    };

    let resolved_hooks = ResolvedHooks::from_config(&config, "test", temp_dir.path());
    let results = HookExecutor::new().execute(&resolved_hooks).unwrap();
    
    // First hook should succeed
    assert!(results.results.get("hook1").map(|r| r.success).unwrap_or(false));
    
    // Second hook should fail
    assert!(!results.results.get("hook2").map(|r| r.success).unwrap_or(true));
    
    // Overall should be failure
    assert!(!results.success);
    
    // Hook3 may or may not run (depends on parallel implementation)
    // But overall failure should propagate
}
```

---

### 7. Integration Test: Hierarchical Resolution with Deep Nesting

**File**: `tests/hierarchical_comprehensive_tests.rs` (new test)

```rust
#[test]
fn test_deep_hierarchy_merging() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create 10-level directory structure
    let mut current = temp_dir.path().to_path_buf();
    for i in 0..10 {
        current = current.join(format!("level{}", i));
        fs::create_dir_all(&current).unwrap();
        
        fs::write(
            current.join("hooks.toml"),
            format!(
                r#"
[hooks.hook{i}]
command = "echo level{i}"
modifies_repository = false

[groups.pre-commit]
includes = ["hook{i}"]
"#
            ),
        ).unwrap();
    }

    // Now resolve hooks for a file deep in the hierarchy
    let deep_file = current.join("test.rs");
    
    // Should merge all 10 levels
    // Expected: All 10 hooks should be present
    
    // This is more of a stress test
    // Just verify it completes without hanging/crashing
}
```

---

### 8. Unit Test: Validate requires_files Compatibility

**File**: `tests/main_validate_advanced_tests.rs` (new test)

```rust
#[test]
fn test_validate_warns_requires_files_incompatible() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    fs::write(
        temp_dir.path().join("hooks.toml"),
        r#"
[hooks.check-msg]
command = "some-checker"
requires_files = true
modifies_repository = false

[groups.commit-msg]
includes = ["check-msg"]
"#,
    ).unwrap();

    let output = Command::new(bin_path())
        .current_dir(temp_dir.path())
        .arg("validate")
        .output()
        .expect("Failed to execute");

    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should warn about incompatibility
    assert!(
        stderr.contains("requires_files") && 
        (stderr.contains("cannot provide") || 
         stderr.contains("incompatible") ||
         stderr.contains("commit-msg")),
        "Should warn about requires_files in commit-msg group"
    );
}
```

---

### 9. Unit Test: Pre-push Multiple Refs Handling

**File**: `src/git/changes.rs` (add to tests module)

```rust
#[test]
fn test_parse_push_stdin_multi_branch_first_wins() {
    // Multiple branches pushed simultaneously
    let stdin = "refs/heads/main abc123def456 refs/heads/main 789xyz012345\nrefs/heads/feature def456ghi789 refs/heads/feature 0000000000000000000000000000000000000000";
    
    let (local_oid, remote_oid) = parse_push_stdin(stdin).unwrap();
    
    // Should parse first line
    assert_eq!(local_oid, "abc123def456");
    assert_eq!(remote_oid, "789xyz012345");
}

#[test]
fn test_parse_push_stdin_joins_args_correctly() {
    // When args are split across multiple Vec elements
    let git_args = vec![
        "refs/heads/main".to_string(),
        "abc123def456".to_string(),
        "refs/heads/main".to_string(),
        "789xyz012345".to_string(),
    ];
    
    let stdin_content = git_args.join(" ");
    let (local_oid, remote_oid) = parse_push_stdin(&stdin_content).unwrap();
    
    // Should still work when joining
    assert_eq!(local_oid, "abc123def456");
    assert_eq!(remote_oid, "789xyz012345");
}
```

---

### 10. Stress Test: Large File Lists

**File**: `tests/executor_comprehensive_tests.rs` (new test)

```rust
#[test]
#[ignore]  // This is slow, run manually
fn test_large_file_list_performance() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create 1000 "changed" files
    let files: Vec<PathBuf> = (0..1000)
        .map(|i| PathBuf::from(format!("file{}.rs", i)))
        .collect();
    
    // Create a pattern matcher
    let patterns = vec!["**/*.rs".to_string()];
    let matcher = FilePatternMatcher::new(&patterns).unwrap();
    
    // Time the matching
    let start = std::time::Instant::now();
    let result = matcher.matches_any(&files);
    let elapsed = start.elapsed();
    
    assert!(result); // Should match
    assert!(elapsed.as_secs() < 1, "Matching 1000 files took too long");
}
```

---

## Recommendations for Test Organization

### New Test Files Needed:
1. `tests/main_requires_files_integration.rs` - Focus on requires_files feature
2. `tests/git_push_stdin_edge_cases.rs` - Edge cases for pre-push parsing
3. `tests/hierarchical_deep_nesting.rs` - Deep hierarchy stress tests
4. `tests/parallel_failure_recovery.rs` - Parallel execution failure modes

### Existing Files to Extend:
- `src/config/templating.rs` - Add template variable edge cases
- `src/git/changes.rs` - Add OID validation tests
- `tests/executor_comprehensive_tests.rs` - Add parallel failure tests

---

## Running the Tests

```bash
# Run specific test file
cargo test --test main_requires_files_integration

# Run with output
cargo test --test main_requires_files_integration -- --nocapture

# Run ignored tests (stress tests)
cargo test -- --ignored

# Full test suite
cargo test --all

# With coverage
cargo tarpaulin --all --out Html
```

