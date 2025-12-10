# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a git hooks manager designed for monorepos, allowing individual paths within a monorepo to have custom hooks. The system supports hierarchical hook definitions with TOML configuration files and safe parallel execution.

## Development Commands

### Essential Commands
```bash
# Build the project
cargo build

# Run all tests
cargo test

# Run strict linting
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt

# Run the complete pre-commit check
cargo run -- run pre-commit

# Run a specific hook in lint mode (all matching files)
cargo run -- lint <hook-name>

# Validate configuration
cargo run -- validate

# Generate shell completions
cargo run -- completions bash|zsh|fish

# Health check and update notifications
cargo run -- doctor

# Self-update to latest version
cargo run -- update
```

### Testing Individual Components
```bash
# Test configuration parsing only
cargo test config::

# Test hook resolution only
cargo test hooks::resolver::

# Test parallel execution
cargo test hooks::executor::test_parallel_safe_execution
```

### Release Management

This project uses `just` for release automation. **NEVER manually bump versions or create tags.**

```bash
# Create a release (runs full quality pipeline, bumps version, creates tag)
just release patch   # 3.0.0 -> 3.0.1
just release minor   # 3.0.0 -> 3.1.0
just release major   # 3.0.0 -> 4.0.0

# The release recipe:
# 1. Validates prerequisites (clean working directory, on main branch, up-to-date with origin)
# 2. Runs all quality gates (tests, audit, deny, pre-commit)
# 3. Bumps version using versioneer
# 4. Creates commit: "chore: bump version to X.Y.Z"
# 5. Creates tag: "vX.Y.Z"
# 6. Prompts for confirmation
# 7. Pushes to GitHub (triggers automated release workflow)

# Manual version operations (for development only)
just version-show           # Show current version
just bump-version patch     # Bump version only (no release)
```

**Version Management Rules:**
- NEVER edit `Cargo.toml` or `VERSION` files manually
- NEVER create git tags manually
- ALWAYS use `just release` for releases
- Breaking changes require `just release major`

## Architecture Overview

### Core Components
- **Config Parser** (`src/config/parser.rs`): TOML parsing with parallel execution flags
- **Hook Resolver** (`src/hooks/resolver.rs`): Hierarchical configuration resolution
- **Hook Executor** (`src/hooks/executor.rs`): Safe parallel execution engine
- **CLI Interface** (`src/cli/mod.rs`): Command-line interface

### Key Features
- **Hierarchical Configuration**: Nearest `.peter-hook.toml` file wins
- **Safe Parallel Execution**: Repository-modifying hooks run sequentially, read-only hooks run in parallel
- **Hook Groups**: Combine individual hooks with execution strategies
- **Cross-platform**: Rust implementation supporting macOS, Linux, Windows

## Configuration System

### Hook Definition Structure
```toml
[hooks.example]
command = "echo hello"                # Required: command to run
description = "Example hook"          # Optional: description
modifies_repository = false           # Required: safety flag for parallel execution
execution_type = "per-file"          # Optional: how files are passed (per-file | in-place | other)
workdir = "custom/path"              # Optional: override working directory
env = { KEY = "value" }              # Optional: environment variables (supports template variables)
files = ["**/*.rs", "Cargo.toml"]    # Optional: file patterns for targeting
depends_on = ["format", "setup"]     # Optional: hook dependencies
run_always = false                   # Optional: ignore file changes (incompatible with files and requires_files)
requires_files = false               # Optional: require file list to run (incompatible with run_always)
run_at_root = false                  # Optional: run at repository root instead of config directory
timeout_seconds = 300                # Optional: maximum execution time in seconds (default: 300 = 5 minutes)
```

**Example: Using tools from custom PATH locations**
```toml
[hooks.my-custom-tool]
command = "my-tool"
modifies_repository = false
# Extend PATH to include custom bin directory
env = { PATH = "{HOME_DIR}/.local/bin:{PATH}" }
```

### Execution Types (How Files Are Passed)

Three execution types control how changed files are passed to hook commands:

#### `per-file` (default)
Files passed as individual command-line arguments.

```toml
[hooks.eslint]
command = "eslint"
execution_type = "per-file"  # default
files = ["**/*.js"]
```
**Runs:** `cd /config/dir && eslint file1.js file2.js file3.js`

**Use for:** Standard linters/formatters that accept file lists (eslint, ruff, prettier with files)

#### `in-place`
Runs once in config directory without file arguments. Tool auto-discovers files.

```toml
[hooks.pytest]
command = "pytest"
execution_type = "in-place"
files = ["**/*.py"]
```
**Runs:** `cd /config/dir && pytest` (pytest discovers test files itself)

**Use for:** Test runners (pytest, jest, cargo test), directory scanners (unvenv)

#### `other`
Hook uses template variables for manual file handling.

```toml
[hooks.custom]
command = "my-tool {CHANGED_FILES}"
execution_type = "other"
files = ["**/*.rs"]
```
**Runs:** `cd /config/dir && my-tool file1.rs file2.rs`

**Use for:** Custom scripts, complex pipelines, non-standard file argument patterns

### File Filtering Behavior

**Three states control when hooks run:**

1. **`files` exists** → Only run when matching files found
   ```toml
   files = ["**/*.py"]  # Runs only if .py files changed
   ```

2. **No `files` pattern** → Runs when any files in this config group changed
   ```toml
   # No files field = runs if any file in this scope changed
   ```

3. **`run_always = true`** → Runs regardless of changes
   ```toml
   run_always = true  # Always runs (incompatible with files pattern)
   ```

### Requiring File Lists

The `requires_files` flag ensures hooks only run when peter-hook can provide a file list. This is useful for hooks that depend on knowing which files changed (like test runners or linters).

**When to use `requires_files = true`:**
- Test hooks that should only run when relevant files change
- Expensive operations that should skip if no files are available
- Hooks that need file context to function properly

**Compatible hook types** (can provide files):
- `pre-commit` - Gets staged files
- `pre-push` - Gets files in the push changeset
- `post-commit`, `post-merge`, `post-checkout` - Gets files in recent commits
- Other file-based hooks

**Incompatible hook types** (cannot provide files):
- `commit-msg`, `prepare-commit-msg` - Operate on commit messages
- `applypatch-msg` - Operates on patch messages

**Example: Test hook that only runs in pre-push when files change**
```toml
[hooks.pytest]
command = "pytest"
description = "Run Python tests only when Python files change"
modifies_repository = false
execution_type = "in-place"
files = ["**/*.py", "**/test_*.py"]  # Only match Python test files
requires_files = true                 # Skip if no files available

[groups.pre-push]
includes = ["pytest"]
description = "Pre-push validation"
```

In this example:
- In `pre-push`: Runs only if Python files changed in the push
- In `commit-msg`: Skipped (can't provide files)
- With `--all-files`: Skipped (no file list available)

**Validation:** The `peter-hook validate` command checks for incompatible configurations and warns if `requires_files` hooks are used in groups that cannot provide files.

### Hook Timeout

All hooks have a configurable timeout to prevent hung processes from blocking the workflow indefinitely.

**Default behavior:**
- Default timeout: 300 seconds (5 minutes)
- Hooks exceeding timeout are killed automatically
- Partial output before timeout is captured and included in error message

**Configuration:**
```toml
[hooks.my-hook]
command = "long-running-command"
modifies_repository = false
timeout_seconds = 600  # Override default: allow 10 minutes
```

**Timeout behavior:**
- Timer starts when hook process spawns
- If hook completes within timeout: normal success/failure handling
- If hook exceeds timeout:
  - Process is killed (SIGKILL on Unix, TerminateProcess on Windows)
  - Error returned with timeout message
  - Any partial stdout/stderr captured before timeout is included in error
  - Hook is considered failed

**When to adjust timeout:**
- **Increase** for legitimately slow operations (full test suites, large builds, code generation)
- **Decrease** for hooks that should be fast (linters, formatters, simple validators)
- **Default** is appropriate for most hooks

**Example: Slow test suite with custom timeout**
```toml
[hooks.integration-tests]
command = "pytest tests/integration --verbose"
description = "Full integration test suite (may take 10+ minutes)"
modifies_repository = false
execution_type = "in-place"
timeout_seconds = 900  # 15 minutes for comprehensive test suite

[groups.pre-push]
includes = ["integration-tests"]
description = "Pre-push validation"
```

**Timeout errors provide diagnostic information:**
```
Error: Hook 'integration-tests' exceeded timeout of 900 seconds and was killed
Partial stdout: Running test_api_endpoints...
PASSED tests/integration/test_api.py::test_login
PASSED tests/integration/test_api.py::test_logout
[... more output ...]
Partial stderr: WARNING: Test database cleanup incomplete
```

### Execution Strategies (Parallelism)
- `sequential`: Run hooks one after another (default)
- `parallel`: Run safely in parallel (respects `modifies_repository` flag)
- `force-parallel`: Run all hooks in parallel (unsafe - ignores safety flags)

### Repository Safety Rules
- Hooks with `modifies_repository = true` NEVER run in parallel with other hooks
- Hooks with `modifies_repository = false` can run in parallel with each other
- Mixed groups run in phases: parallel safe hooks first, then sequential modifying hooks

## Code Quality Standards

- **Zero warnings policy**: All code must pass `cargo clippy -- -D warnings`
- **100% test coverage goal**: Comprehensive unit and integration tests
- **Cross-platform compatibility**: Primary macOS, support Linux/Windows
- **Security-first**: Regular dependency audits, no unsafe code allowed
- **Rust version pinning**: Project uses Rust 1.85.0 (pinned via rust-toolchain.toml)

### Rust Version Management

**The project pins Rust version to ensure identical linting locally and in CI.**

- **Current version:** 1.86.0 (see `rust-toolchain.toml`)
- **Local usage:** `cargo` automatically uses the pinned version
- **CI usage:** All workflows use the same pinned version
- **Updating:** Edit `rust-toolchain.toml` and update all workflow files

**Why pinning?**
- Guarantees clippy lints are identical everywhere
- Prevents surprise breakages from new lints
- Reproducible builds and deterministic CI
- Controlled Rust version updates

**How to update Rust version:**
1. Update `rust-toolchain.toml` channel to new version
2. Update `.github/workflows/*.yml` files to use same version
3. Test locally: `cargo clippy --all-targets -- -D warnings`
4. Update `Cargo.toml` rust-version field to match
5. Commit all changes together

## Important Implementation Details

- Hook scripts run from their configuration file directory by default (NOT git root)
- Use `run_at_root = true` to override this behavior and run at the repository root
- Hierarchical resolution: child directories override parent configurations
- Thread-safe parallel execution with proper error handling
- Backward compatibility maintained for deprecated `parallel` field in groups

### Multi-Config Group Execution Behavior

When multiple config groups are involved (different `.peter-hook.toml` files for different changed files), peter-hook executes them sequentially with fail-fast semantics:

**Execution Order:**
1. Groups are processed in the order they are resolved (typically by file path)
2. Each group's hooks execute according to their execution strategy (sequential/parallel)
3. **On failure**: Execution stops immediately; remaining groups are NOT executed
4. **On success**: Proceeds to the next group

**Example Scenario:**
```
Changed files:
  - backend/api.rs      → Config Group A (backend/.peter-hook.toml)
  - frontend/app.tsx    → Config Group B (frontend/.peter-hook.toml)
  - docs/README.md      → Config Group C (docs/.peter-hook.toml)

Execution flow:
  Group A (backend): Run hooks → SUCCESS ✓
  Group B (frontend): Run hooks → FAILURE ✗
  Group C (docs): SKIPPED (not executed)

Final result: FAILURE (git commit/push is blocked)
```

**Rationale:**
This behavior follows traditional git hook semantics where any failure blocks the git operation. This prevents partially-validated changes from being committed/pushed.

**Important Notes:**
- Failed groups do NOT roll back or undo previous successful groups
- Each group executes in its own context (config directory)
- Hook names are prefixed with config path in output for clarity
- Use `--dry-run` to preview execution without actually running hooks

## Advanced Features

### Template Variables

Template variables use `{VARIABLE_NAME}` syntax and can be used in:
- `command` field (shell commands or arguments)
- `env` field (environment variable values)
- `workdir` field (working directory paths)

**Available template variables:**
- `{HOOK_DIR}` - Directory containing the .peter-hook.toml file
- `{REPO_ROOT}` - Git repository root directory
- `{PROJECT_NAME}` - Name of the directory containing .peter-hook.toml
- `{HOME_DIR}` - User's home directory (from $HOME)
- `{PATH}` - Current PATH environment variable
- `{WORKING_DIR}` - Current working directory
- `{CHANGED_FILES}` - Space-delimited list of changed files (when using `--files`)
- `{CHANGED_FILES_LIST}` - Newline-delimited list of changed files
- `{CHANGED_FILES_FILE}` - Path to temporary file containing changed files

**Common use cases:**
```toml
# Run tool from custom PATH location (Method 1: extend PATH)
[hooks.custom-tool]
command = "my-tool --check"
env = { PATH = "{HOME_DIR}/.local/bin:{PATH}" }

# Run tool from custom PATH location (Method 2: absolute path)
[hooks.custom-tool-direct]
command = "{HOME_DIR}/.local/bin/my-tool --check"

# Use repository root in command
[hooks.build]
command = "make -C {REPO_ROOT} build"

# Set environment variables with templates
[hooks.test]
command = "pytest"
env = {
  PROJECT_ROOT = "{REPO_ROOT}",
  BUILD_DIR = "{REPO_ROOT}/target",
  PATH = "{HOME_DIR}/.local/bin:{PATH}"
}
```

**Security note:** Only whitelisted template variables are available. Arbitrary environment variables are not exposed to prevent security issues.

### Hook Dependencies  
- Use `depends_on = ["hook1", "hook2"]` to ensure execution order
- Automatic topological sorting with cycle detection
- Dependencies respected even in parallel execution groups

### File Pattern Targeting
- Use `files = ["**/*.rs"]` to run hooks only when specific files change
- Supports glob patterns for precise targeting
- Use `run_always = true` to bypass file filtering
- Enable with `--files` flag: `peter-hook run pre-commit --files`

### Lint Mode
- Run hooks on ALL matching files with `lint <hook-name>`
- Treats current directory as repository root
- Discovers all non-ignored files respecting .gitignore
- No git operations - pure file discovery and execution
- Usage: `peter-hook lint <hook-name> [--dry-run]`
- Perfect for:
  - Running linters/formatters on entire codebase
  - Pre-CI validation without git operations
  - Per-directory validation (e.g., `unvenv`)
  - One-off quality checks

**Execution modes in lint:**
- `per-file`: Files passed as arguments (e.g., `ruff check file1.py file2.py`)
- `in-place`: Runs once in config directory without file arguments (e.g., `jest`, `pytest`)
- `other`: Uses template variables for manual file handling

### Git Integration
- Supports 15+ git hook events (pre-commit, commit-msg, pre-push, etc.)
- Automatic git argument passing for hooks that need them
- Smart change detection for file targeting (working directory vs push changes)