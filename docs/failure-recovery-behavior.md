# Failure Recovery and Error Handling Behavior

Test-driven documentation of how peter-hook handles hook failures and errors.

## Key Behaviors

### 1. All Hooks Execute Despite Failures

**Behavior**: Peter-hook executes ALL hooks in a group, even if some fail.

**Rationale**: This "run all, report all" approach gives complete feedback about all validation failures in a single run, rather than requiring multiple fix-run cycles.

**Examples**:
```toml
[groups.pre-commit]
includes = ["hook-a", "hook-b", "hook-c"]
```

If `hook-a` fails:
- ✓ `hook-b` still executes
- ✓ `hook-c` still executes
- ✗ Overall result: FAILURE

### 2. Mixed Execution Continues Through Phases

**Behavior**: When using `execution_strategy = "parallel"` with both parallel and sequential hooks, ALL phases execute even if earlier phases fail.

**Example**:
```toml
[hooks.parallel-1]
modifies_repository = false  # Can run in parallel

[hooks.sequential-1]
modifies_repository = true   # Must run sequentially

[groups.pre-commit]
includes = ["parallel-1", "sequential-1"]
execution_strategy = "parallel"
```

If `parallel-1` fails:
- Phase 1 (parallel): `parallel-1` executes and fails
- Phase 2 (sequential): `sequential-1` STILL executes
- Overall result: FAILURE

**Test**: `test_mixed_execution_continues_despite_failures`

### 3. Sequential Execution Continues

**Behavior**: Even with `execution_strategy = "sequential"`, all hooks execute sequentially regardless of failures.

**Example**:
```toml
[groups.pre-commit]
includes = ["first", "second", "third"]
execution_strategy = "sequential"
```

If `first` fails:
- ✓ `first` executes and fails
- ✓ `second` still executes
- ✓ `third` still executes
- Overall result: FAILURE

**Test**: `test_sequential_hooks_all_execute_despite_failures`

### 4. Parallel Execution Collects All Results

**Behavior**: When hooks run in parallel, all complete regardless of individual failures.

**Example**:
```toml
[groups.pre-commit]
includes = ["test-1", "test-2", "test-3", "test-4", "test-5"]
execution_strategy = "parallel"
```

If `test-3` fails:
- All 5 hooks run concurrently
- 4 pass, 1 fails
- Overall result: FAILURE (any failure fails the group)

**Test**: `test_parallel_hooks_one_fails_others_complete`

### 5. Dependencies Control Order, Not Failure Propagation

**Behavior**: The `depends_on` field ensures execution order but does NOT prevent dependent hooks from running if dependencies fail.

**Example**:
```toml
[hooks.parent]
command = "exit 1"  # Fails

[hooks.child]
depends_on = ["parent"]
command = "echo 'I still run'"

[groups.pre-commit]
includes = ["parent", "child"]
```

**Result**:
- `parent` runs first (because of dependency)
- `parent` fails
- `child` STILL runs (dependency controls order, not execution)
- Overall result: FAILURE

**Rationale**: Dependencies are for ordering (e.g., "run formatter before linter"), not for conditional execution.

**Test**: `test_dependencies_control_order_not_failure`

### 6. Timeout is Treated as Failure

**Behavior**: Hooks that exceed their `timeout_seconds` are killed and treated as failures.

**Example**:
```toml
[hooks.slow]
command = "sleep 10"
timeout_seconds = 1
```

**Result**:
- Hook starts executing
- After 1 second, process is killed (SIGKILL)
- Partial output is captured and included in error
- Overall result: FAILURE

**Test**: `test_hook_timeout_is_treated_as_failure`

### 7. Exit Codes

**Behavior**: Any non-zero exit code is treated as failure.

**Examples**:
- `exit 0` → SUCCESS
- `exit 1` → FAILURE
- `exit 2` → FAILURE
- `exit 127` → FAILURE
- Command not found → FAILURE
- Signal termination → FAILURE

**Test**: `test_hook_with_complex_failure_exit_codes`

### 8. Error Message Propagation

**Behavior**: Error messages include:
- Hook name
- Exit code
- stdout content
- stderr content
- For timeouts: partial output before timeout

**Example Output**:
```
=== Hook Execution Summary ===
[FAIL] my-validation-hook: exit code 1
  stdout: Processing files...
  stderr: Error: Missing required field 'name'
```

**Test**: `test_error_messages_include_hook_names`

### 9. Partial Success Still Fails

**Behavior**: If ANY hook fails, the overall result is FAILURE, regardless of how many succeed.

**Examples**:
- 99 hooks pass, 1 fails → FAILURE
- 1 hook passes, 1 fails → FAILURE
- 0 hooks pass, 5 fail → FAILURE

**Rationale**: Git hooks should be all-or-nothing: all validations must pass to proceed.

**Test**: `test_partial_parallel_success_still_fails`

### 10. Nonexistent Commands

**Behavior**: Hooks with commands that don't exist fail gracefully with clear error messages.

**Example**:
```toml
[hooks.oops]
command = "this-doesnt-exist"
```

**Result**:
- Clear error: "Failed to execute" or "command not found"
- Hook marked as FAILED
- Includes hook name in error
- Overall result: FAILURE

**Test**: `test_nonexistent_command_failure`

### 11. Multiple Failures Reported

**Behavior**: When multiple hooks fail, all failures are reported in the output.

**Example**: 3 hooks fail in parallel
- All 3 failures appear in execution summary
- Each failure shows hook name and exit code
- stderr/stdout included for each

**Test**: `test_multiple_failures_all_reported`

### 12. Dry Run Never Fails

**Behavior**: `--dry-run` flag shows what would execute but doesn't actually run hooks, and always succeeds.

**Example**:
```bash
peter-hook run pre-commit --dry-run
```

**Result**:
- Shows which hooks would run
- Shows which files would be processed
- EXIT CODE: 0 (always succeeds)

**Test**: `test_dry_run_shows_failures_but_doesnt_fail`

## Design Philosophy

Peter-hook's failure handling follows these principles:

1. **Complete Feedback**: Run all validations to give developers complete feedback in one run
2. **Fail Fast for Commit**: Return non-zero exit to prevent commits when any validation fails
3. **No Silent Failures**: All failures are reported with clear error messages
4. **Predictable Behavior**: Consistent execution regardless of failure location
5. **Developer-Friendly**: Multiple failures don't require multiple fix-run cycles

## Comparison with Other Tools

**vs. Traditional Git Hooks**:
- Traditional: First failure stops execution
- Peter-hook: All hooks execute, all failures reported

**vs. Pre-commit**:
- Pre-commit: Similar "run all" behavior
- Peter-hook: Adds hierarchical configuration and parallel execution

## Performance Implications

**Advantages**:
- Single run provides complete validation feedback
- Parallel execution minimizes total run time
- Developers fix all issues at once

**Trade-offs**:
- May run hooks that would be "blocked" by earlier failures
- Slightly longer total execution time if early failures would have prevented later hooks

**Mitigation**:
- Use dependencies to enforce ordering when needed
- Use `timeout_seconds` to prevent runaway hooks
- Use parallel execution to minimize wall-clock time

## Exit Code Behavior

Peter-hook returns:
- **0**: All hooks succeeded
- **Non-zero**: At least one hook failed

Git uses this exit code to determine whether to proceed with the commit/push.

## Resource Cleanup

**On Failure**:
- Temp files are cleaned up
- Processes are properly terminated
- No resource leaks

**On Timeout**:
- Process is killed (SIGKILL)
- Partial output is captured
- Temp files are cleaned up

**On Interrupt** (Ctrl+C):
- Graceful shutdown of running hooks
- Cleanup of temporary resources

## Best Practices

1. **Fast Failures First**: Put quick validation hooks before slow ones
2. **Clear Error Messages**: Use stderr to provide actionable error messages
3. **Appropriate Timeouts**: Set `timeout_seconds` based on expected runtime
4. **Meaningful Exit Codes**: Use standard exit codes (0 = success, 1 = failure)
5. **Dependencies for Ordering**: Use `depends_on` when hook order matters (e.g., format before lint)

## Test Coverage

11 comprehensive tests validate failure handling:
- Parallel execution with failures
- Sequential execution with failures
- Mixed execution strategies
- Dependency ordering
- Timeout handling
- Error message propagation
- Exit code handling
- Partial success scenarios
- Dry run behavior
- Nonexistent commands
- Multiple failures

**Status**: ✅ All tests PASS
**Total Test Count**: 169 tests (158 before + 11 failure recovery)
