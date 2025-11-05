# Peter-Hook Test Coverage Analysis

## Executive Summary

Peter-hook's recent `requires_files` feature implementation is well-structured but reveals **significant gaps in integration testing** across multiple dimensions:

- **No integration tests** for `requires_files` behavior in actual hook execution
- **Pre-push stdin parsing** has minimal edge case coverage
- **Hierarchical resolution** lacks tests for deep nesting and placeholder groups
- **Template variable security** has whitelist checks but no injection/escape tests  
- **Parallel execution** lacks race condition and failure recovery tests
- **Error handling** for git operations is underspecified

---

## 1. REQUIRES_FILES FEATURE - CRITICAL GAPS

### 1.1 Integration Testing Gaps

**Status**: Unit tests exist, but NO integration tests

#### Tests Added (from file-list.md):
- `src/config/parser.rs`: 4 tests for field parsing and validation
- `src/git/capabilities.rs`: 8 tests for hook type capability detection
- `src/git/changes.rs`: 5 tests for pre-push stdin parsing
- **Total**: 17 new tests, all UNIT tests

#### Missing Integration Tests:

```toml
# Missing Test Case 1: requires_files hook in commit-msg group
[hooks.check-msg]
command = "some-checker"
requires_files = true
modifies_repository = false

[groups.commit-msg]
includes = ["check-msg"]  # Should WARN or SKIP at validate time
```

**Expected behavior**: Hook should be skipped in commit-msg context (can't provide files)
**Current test coverage**: ❌ No integration test confirming this behavior

---

```toml
# Missing Test Case 2: requires_files with file pattern filtering
[hooks.test-py]
command = "pytest"
files = ["**/*.py"]
requires_files = true
modifies_repository = false
execution_type = "in-place"

[groups.pre-push]
includes = ["test-py"]
```

**Execution scenarios NOT tested**:
1. Pre-push with Python files changed → Should run
2. Pre-push with NO Python files changed → Should skip
3. Pre-push with mixed file types → Should run (Python files present)
4. `--all-files` flag → Should SKIP (no file list available)
5. Dry-run mode → Should SKIP (no file list)

**Current test coverage**: ❌ Zero integration tests for these scenarios

---

### 1.2 Config Validation Gaps

The `validate_requires_files_compatibility()` function in `main.rs` (lines 960-1001):

**What it does**:
- Warns if `requires_files` hooks appear in incompatible groups (commit-msg, etc.)
- Lists compatible vs incompatible hook types

**What it doesn't do**:
- Doesn't validate hooks used in `--all-files` contexts (no file list)
- Doesn't check if a hook has BOTH `requires_files=true` AND `run_always=true` (should be caught at parse time, but validation doesn't explicitly test this)
- Doesn't warn about `requires_files` in placeholder groups

**Missing test**:
```rust
#[test]
fn test_validate_rejects_requires_files_with_run_always() {
    // This should be caught during parsing, not validation
    // But verify it's actually rejected
}
```

---

### 1.3 Hierarchical Resolution + requires_files Gap

**In `hierarchical.rs` (line 352-359)**:
```rust
// Skip hooks that require files when no files are available
if hook_def.requires_files && changed_files.is_none() {
    trace!("Skipping hook '{}' because it requires files but none are available", hook_name);
    continue;
}
```

**Problem**: This code path is never tested in integration

**Missing test scenarios**:
1. **Parent config has requires_files hook, child overrides without it**
   ```toml
   # root/hooks.toml
   [hooks.lint]
   requires_files = true
   command = "ruff check"
   
   # root/subdir/hooks.toml  
   [hooks.lint]
   requires_files = false  # Child REMOVES the requirement
   command = "ruff check --all"
   ```
   - When does the child's definition take effect?
   - Is requires_files property correctly inherited/overridden?

2. **Placeholder groups with requires_files**
   ```toml
   # root/hooks.toml
   [groups.pre-push]
   placeholder = true
   includes = ["test"]
   
   [hooks.test]
   requires_files = true
   command = "pytest"
   ```
   - Should placeholder groups interact with requires_files?

---

## 2. PRE-PUSH STDIN PARSING - INCOMPLETE EDGE CASES

### Current Test Coverage
Location: `src/git/changes.rs` (lines 662-700)

Tests present:
- ✅ Valid format parsing
- ✅ New branch push (all-zero OID)
- ✅ Empty stdin (error case)
- ✅ Invalid format (too few fields)
- ✅ Multiple lines (takes first)

### Missing Edge Cases

#### 2.1 Malformed OIDs
```rust
#[test]
fn test_parse_push_stdin_invalid_local_oid() {
    // Local OID is invalid hex
    let stdin = "refs/heads/main INVALID refs/heads/main 789xyz012345";
    let err = parse_push_stdin(stdin).unwrap();
    // Currently: SUCCEEDS (accepts any string as OID!)
    // Expected: Should validate OID format
}
```

**Current behavior**: Accepts ANY string as OID, no validation
**Risk**: Invalid git operations later when using bad OID in diff

---

#### 2.2 Symbolic Refs Instead of OIDs
```
# Git can send symbolic refs in stdin (rare but possible)
refs/heads/main refs/heads/main^2 refs/heads/main refs/heads/main^1
```

**Current behavior**: Would accept these as OIDs and pass to git diff
**Risk**: Git diff fails with cryptic error message

---

#### 2.3 Force Push with Delete
```
# Delete remote branch (all-zero local OID)
refs/heads/feature 0000000000000000000000000000000000000000 refs/heads/feature abc123def456
```

**Current test**: ✅ Handles all-zero remote OID (new branch)
**Missing test**: ❌ All-zero LOCAL OID (deleting)

```rust
#[test]
fn test_parse_push_stdin_force_delete_branch() {
    let stdin = "refs/heads/feature 0000000000000000000000000000000000000000 refs/heads/feature abc123def456";
    let (local_oid, remote_oid) = parse_push_stdin(stdin).unwrap();
    // Currently: FAILS OR BEHAVES UNEXPECTEDLY
    // Expected: Should handle gracefully (empty to abc123 = deletion)
}
```

---

#### 2.4 Whitespace Handling
```
# Extra whitespace
"  refs/heads/main   abc123def456   refs/heads/main   789xyz012345  "

# Tabs instead of spaces
"refs/heads/main\tabc123def456\trefs/heads/main\t789xyz012345"
```

**Current code** (line 248): Uses `split_whitespace()`
**Result**: ✅ Handles both tabs and spaces correctly
**Assessment**: Adequate

---

#### 2.5 Non-UTF8 in Ref Names
```
# Ref with non-UTF8 bytes (git allows this)
let stdin = b"refs/heads/\xFF\xFE abc123def456 refs/heads/main 789xyz012345";
```

**Current behavior**: Would panic on `.to_string()` call
**Risk**: Hook crashes instead of handling gracefully

```rust
#[test]
fn test_parse_push_stdin_non_utf8_refs() {
    // Need to handle non-UTF8 gracefully
    // Current code doesn't test this
}
```

---

#### 2.6 Very Long Ref Names (2000+ char)
```
let stdin = format!("refs/heads/{} abc123 refs/heads/main 789xyz", "x".repeat(2000));
```

**Current behavior**: No length validation
**Risk**: OOM attack? (unlikely but not tested)

---

### 2.7 Integration: Pre-push Hook Argument Passing

**In `main.rs` (lines 289-315)**:
```rust
if git_args.is_empty() {
    // Falls back to origin/main
} else {
    let stdin_content = git_args.join(" ");
    match peter_hook::git::parse_push_stdin(&stdin_content) {
        Ok((local_oid, remote_oid)) => Some(ChangeDetectionMode::Push {...}),
        Err(e) => {
            eprintln!("Warning: Failed to parse...");
            // Falls back to origin/main
        }
    }
}
```

**Problem**: Args are joined with spaces, but git passes stdin on separate lines!

**Actual git pre-push stdin format**:
```
refs/heads/main 67890abc... refs/heads/main 12345def...
refs/heads/feature abcd1234... refs/heads/feature 0000000...
```

**Current code treats as**: One space-joined string, takes first line ✅ (works by accident)

**Missing tests**:
```rust
#[test]
fn test_pre_push_multiple_refs_takes_first() {
    // Multiple branches pushed simultaneously
    let git_args = vec![
        "refs/heads/main abc123 refs/heads/main 789xyz".to_string(),
        "refs/heads/feature def456 refs/heads/feature 0000000".to_string(),
    ];
    // Should only process first?
    // Current behavior: Joins all args, creates garbage string
}
```

---

## 3. HIERARCHICAL RESOLUTION - DEEP NESTING & COMPLEXITY GAPS

### 3.1 Very Deep Nesting (10+ levels)

**Current tests** (hierarchical.rs):
- ✅ 3-level hierarchy (line 1001)
- ❌ 10+ level hierarchy

**Missing test**:
```rust
#[test]
fn test_merge_ten_level_deep_hierarchy() {
    // Create root/a/b/c/d/e/f/g/h/i/hooks.toml
    // Each level adds one hook
    // Verify all hooks are found and merged correctly
}
```

**Risk**: Performance degradation at depth (should be O(depth * hooks) but may be worse)

---

### 3.2 Circular Includes in Groups

**Current state**: Depends on `depends_on` cycle detection in `dependencies.rs`

**Missing test**: What if groups reference each other?
```toml
# root/hooks.toml
[groups.pre-commit]
includes = ["lint", "test"]

# root/src/hooks.toml  
[groups.pre-commit]
includes = ["format"]  # OK: extending

# But what if: groups include group names instead of hooks?
[groups.pre-commit]
includes = ["other-group"]  # Is this even allowed?

[groups.other-group]
includes = ["pre-commit"]  # Circular!
```

**Current behavior**: Unclear (not tested)

---

### 3.3 Empty Groups at Deep Levels

**From `hierarchical.rs` test (line 1126)**:
```rust
#[test]
fn test_merge_empty_child_group_still_gets_parent_hooks() {
    // Child has empty includes but still defines the group
    // Result: Should still inherit parent's hooks
}
```

**This test passes** ✅

**Missing edge case**: What if root has group, middle doesn't, child does?
```
root/hooks.toml:      [groups.pre-commit] includes = ["format"]
root/middle/hooks.toml: (no pre-commit group)
root/middle/deep/hooks.toml: [groups.pre-commit] includes = ["test"]
```

**Expected**: format + test
**Current behavior**: Unknown (not tested)

---

### 3.4 Conflicting Execution Strategies at Multiple Levels

**Current test** (line 725-775): Merge uses "most conservative" (sequential)

**Missing test**: ForceParallel at one level, Sequential at another
```toml
# root/hooks.toml
[groups.pre-commit]
execution = "force-parallel"
includes = ["hook1"]

# root/src/hooks.toml
[groups.pre-commit]
execution = "sequential"
includes = ["hook2"]
```

**Current behavior**: Merges to Sequential (safe)
**Missing verification**: Confirm force-parallel is properly demoted

---

### 3.5 Placeholder Groups with Requires_files

**Current behavior**: Placeholder groups skip execution at root but enable subdirs
**Missing test**: Placeholder + requires_files interaction
```toml
[groups.pre-push]
placeholder = true
includes = ["test"]

[hooks.test]
requires_files = true
```

**Questions not tested**:
1. Does placeholder bypass requires_files validation?
2. Do child directories respect requires_files from parent?

---

## 4. FILE FILTERING COMPLEXITY - INTERACTION GAPS

### 4.1 Files Pattern + Run_Always + Requires_Files

**Current validation** (parser.rs):
```rust
// requires_files and run_always are incompatible (mutual exclusivity)
```

**Missing test**: What about interaction with `files` pattern?
```toml
[hooks.lint]
command = "ruff check"
files = ["**/*.py"]
requires_files = true
run_always = false
modifies_repository = false
```

**Scenarios NOT tested**:
1. Python file changed, no files available → Skipped (requires_files)? Or skipped (no match)? Both?
2. No files available at all → Definitely skipped
3. Non-Python file changed → Skipped (pattern match)
4. No changes but has pattern → Skipped (no pattern match + no files)

**Current behavior**: Unclear order of checks

---

### 4.2 Glob Pattern Negation

**Current code** (changes.rs): Uses `glob::Pattern`
**Tested negation patterns**: ❌ None visible

```rust
#[test]
fn test_file_pattern_negation() {
    // Pattern: ["**/*.rs", "!tests/**"]
    // File: tests/test.rs
    // Expected: Should NOT match
    
    let matcher = FilePatternMatcher::new(&["**/*.rs".to_string()]).unwrap();
    // Note: Negation syntax not even shown in tests
}
```

**Risk**: Glob library might support `!` but peter-hook doesn't document it

---

### 4.3 Very Large File Lists (1000+ files)

**Current tests**: No performance testing
**Risk**: 
- Memory usage with 1000+ files in changed_files
- O(n*m) matching against patterns
- No pagination or streaming

**Missing test**:
```rust
#[test]
fn test_large_file_list_performance() {
    // Create 1000 changed files
    // Verify matching completes in < 1 second
}
```

---

### 4.4 Unicode & Special Characters in Paths

**Current code** (changes.rs, line 303):
```rust
let path_str = file_path.to_string_lossy();
```

**Problem**: Uses lossy conversion

**Missing test**:
```rust
#[test]
fn test_special_characters_in_paths() {
    // File: "src/café/test.rs"
    // Should this match pattern "**/*.rs"? 
    
    // File: "src/(test)/file.rs"
    // Does glob treat parens specially?
    
    // File: "src/file [1].rs"
    // Space and bracket handling?
}
```

---

## 5. TEMPLATE VARIABLE EXPANSION - SECURITY & EDGE CASES

### 5.1 Template Variable Whitelist

**Current tests** (templating.rs, line 413-438):
```rust
#[test]
fn test_whitelist_security() {
    // Verifies only whitelisted variables are available
}
```

**Status**: ✅ Security whitelist is tested

### 5.2 Missing Template Variable Tests

#### Undefined Variables
```rust
#[test]
fn test_undefined_template_variable() {
    // Command: "echo {UNDEFINED_VAR}"
    // Current behavior: ???
    // Expected: Should error or leave as literal?
}
```

---

#### Nested Template Variables
```rust
#[test]
fn test_nested_template_variables() {
    let resolver = TemplateResolver::new(...);
    // If HOME_DIR="/home/user" and PATH="$HOME/bin:..."
    // And we have: {HOME_DIR}/bin/{PROJECT_NAME}
    // Does it correctly expand both?
    // What if one template var contains another var's syntax?
}
```

---

#### Template Variable in Template Value
```rust
#[test]
fn test_template_in_env_value() {
    // env = { PATH = "{HOME_DIR}/.local/bin:{PATH}" }
    // Does HOME_DIR get expanded, then PATH, then the full value?
    // Or does it fail due to circular reference attempt?
}
```

---

### 5.3 Path Traversal via Templates

```rust
#[test]
fn test_template_path_traversal() {
    // workdir = "{REPO_ROOT}/../../../etc/passwd"
    // Current behavior: Probably allows (relative paths not restricted)
    // Risk: Hook could write outside repo
}
```

**Current state**: No path sanitization in template expansion
**Current mitigation**: Relies on hook command itself to be safe
**Risk Level**: Medium (user-written hooks could be malicious)

---

### 5.4 Injection via Template Variables

**Current attack vector**:
```toml
# If user can control an env var that gets expanded...
[hooks.lint]
command = "echo {COMPROMISED_VAR}"
env = { COMPROMISED_VAR = "'; rm -rf /;" }  # Shell injection!
```

**Mitigation in place**:
- Commands are executed with `Command::new()` (not shell)
- This prevents shell injection ✅

**But what if**:
```toml
[hooks.lint]
command = "/bin/sh"
args = ["-c", "echo {COMPROMISED_VAR}"]  # Now it IS shell!
```

**Current test coverage**: No test for command execution with shell

---

## 6. PARALLEL EXECUTION - RACE CONDITIONS & FAILURE RECOVERY

### 6.1 Race Conditions in Parallel Mode

**Current implementation** (executor.rs):
- Uses `std::thread` for parallel execution
- Uses `Arc<Mutex<>>` for thread-safe result collection

**Tests present** (executor_comprehensive_tests.rs):
- ✅ test_executor_with_parallel (line 16)
- ✅ test_execute_multiple_hooks_parallel (line 104)
- ✅ test_execute_force_parallel (line 135)
- ✅ test_execute_parallel_safe_hooks (line 416)

**Missing tests**:

#### Race Condition: One Hook Modifies Files While Another Reads
```rust
#[test]
#[ignore] // Timing-dependent, might not always fail
fn test_parallel_race_condition_read_write() {
    // Hook 1: modifies_repository = true (runs sequentially)
    // Hook 2: modifies_repository = false (runs in parallel)
    // Hook 3: modifies_repository = false (tries to read modified files)
    // Question: Are they actually sequenced correctly?
}
```

**Current safety mechanism**: Hooks with `modifies_repository=true` never run in parallel
**Missing test**: Verify this is actually enforced in threaded execution

---

#### One Hook Fails, Others Still Running
```rust
#[test]
fn test_parallel_one_hook_fails_others_stop() {
    // Create 4 hooks
    // Hook 2 fails with exit code 1
    // Expected: All other hooks stop execution
    // Current behavior: ???
}
```

**From executor.rs** (line 98-100):
```rust
// Stop on first failure (traditional git hook behavior)
if !results.success {
    break;
}
```

**Problem**: This is in sequential code path; parallel path unclear

---

#### Deadlock Potential
```rust
// If a hook tries to acquire a lock held by main thread?
// Current: Mutex implementation might deadlock
```

**No stress tests for lock contention**

---

### 6.2 ForceParallel with Repository-Modifying Hooks

**Current behavior** (from parser.rs):
```rust
pub enum ExecutionStrategy {
    Sequential,
    Parallel,
    ForceParallel,  // Ignores modifies_repository flag!
}
```

**Test exists**: ✅ test_execute_force_parallel (line 135)

**Missing test**: What happens when two `modifies_repository=true` hooks run in ForceParallel?

```rust
#[test]
fn test_force_parallel_with_modifying_hooks() {
    // Two hooks both with modifies_repository = true
    // In force-parallel mode
    // Expected: UNSAFE, could corrupt repo
    // Current test: Doesn't verify this is actually unsafe
}
```

**Safety concern**: Documented as "unsafe" but no test of unsafe behavior

---

## 7. ERROR HANDLING - UNDERSPECIFIED BEHAVIOR

### 7.1 Git Command Failures

**Current implementation** (changes.rs, line 202-219):
```rust
fn run_git_command(&self, args: &[&str]) -> Result<String> {
    let output = Command::new("git")...
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Git command failed..."));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

**Missing tests**:

#### Git Not Installed
```rust
#[test]
fn test_git_not_installed() {
    // Set PATH to exclude git
    // Try to detect changes
    // Expected error message should be helpful
}
```

---

#### Not in a Git Repository
```rust
#[test]
fn test_not_in_git_repo() {
    // Create detector in non-git directory
    // Expected: Clear error "Not a git repository"
    // Current: Should work but verify message quality
}
```

---

#### Permission Denied on Repository
```rust
#[test]
fn test_permission_denied_on_git_dir() {
    // Create repo with no read permissions on .git
    // Try to detect changes
    // Expected: Graceful error handling
}
```

---

### 7.2 Hook Command Not Found

**Current behavior** (executor.rs):
```rust
let mut cmd = Command::new(shell_for_platform());
cmd.arg("-c").arg(command_str);
```

**Missing test**:
```rust
#[test]
fn test_hook_command_not_found() {
    // Hook command: "nonexistent-binary-xyz"
    // Expected: Should report clearly that command not found
    // vs other execution errors
}
```

---

### 7.3 Hook Segfault or Hang

```rust
#[test]
#[timeout(5)]
fn test_hook_hangs() {
    // Hook: "sleep 1000"
    // Timeout: 5 seconds
    // Expected: Should timeout gracefully
    // Current implementation: No timeout mechanism!
}
```

**Critical gap**: No timeout handling for hung hooks

---

### 7.4 Hook Output Encoding Issues

```rust
#[test]
fn test_hook_non_utf8_output() {
    // Hook produces binary output
    // Current: Uses `String::from_utf8_lossy()` (lossy!)
    // Expected: Should handle gracefully
}
```

---

## 8. MULTI-CONFIG GROUP EXECUTION - FAILURE MODES

### 8.1 First Group Succeeds, Second Fails

**From executor.rs** (line 98-100):
```rust
// Stop on first failure
if !results.success {
    break;  // Stops after first group fails
}
```

**Missing test**: Clean up after first group if second fails

```rust
#[test]
fn test_multi_group_cleanup_on_failure() {
    // Group 1: Creates temp file, succeeds
    // Group 2: Fails
    // Expected: Temp files cleaned up? Or left for debugging?
    // Current: Behavior undefined
}
```

---

### 8.2 Dry Run with Multiple Groups

```rust
#[test]
fn test_dry_run_multiple_groups() {
    // --dry-run flag with 2 config groups
    // Expected: Shows what would run in each group
    // Current: Does dry-run even work with multiple groups?
}
```

---

### 8.3 Groups with No Matching Files

**From hierarchical.rs** (line 431-435):
```rust
} else {
    trace!("  {} -> NO CONFIG (will be skipped)", file.display());
    // File has no config - skipped
}
```

**Missing test**: If all files get no config, what happens?

```rust
#[test]
fn test_all_files_without_hooks_config() {
    // Changed files but no hooks.toml at all in repo
    // Expected: Graceful "no hooks found"
    // Current: Already tested elsewhere, OK
}
```

---

## SUMMARY OF CRITICAL GAPS

### Tier 1: Security & Correctness Issues

1. **Pre-push stdin parsing accepts invalid OIDs** → Could cause git errors
2. **Template variable path traversal** → Could access files outside repo
3. **No timeout for hung hooks** → Could hang CI/CD
4. **ForceParallel with modifying hooks** → Documented as unsafe, unclear if actually unsafe

### Tier 2: Missing Integration Tests

1. **requires_files hook skipping** → Not tested end-to-end
2. **Hierarchical requires_files override** → Not tested
3. **Parallel hook failure recovery** → Not tested
4. **Multi-config group failure cleanup** → Not tested

### Tier 3: Edge Cases & Performance

1. **Very deep hierarchies (10+ levels)** → Not stress-tested
2. **Large file lists (1000+ files)** → No performance test
3. **Unicode in paths** → Not tested with glob matching
4. **Circular group includes** → Not tested

---

## RECOMMENDATIONS

### Immediate Priority (Security)
- Add validation for OID format in `parse_push_stdin()`
- Add timeout mechanism for long-running hooks
- Document ForceParallel safety guarantees

### High Priority (Correctness)
- Add end-to-end test for requires_files hook skipping
- Add integration tests for hierarchical resolution + requires_files
- Add tests for multi-group failure scenarios

### Medium Priority (Robustness)
- Add timeout tests for hung processes
- Add performance tests for large file lists
- Add tests for non-UTF8 output handling

### Low Priority (Edge Cases)
- Add tests for very deep hierarchies
- Add tests for circular references in groups
- Add tests for all special character combinations

