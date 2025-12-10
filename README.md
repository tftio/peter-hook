# Peter Hook

A hierarchical git hooks manager designed for monorepos with safe parallel execution.

> **⚠️ Breaking Change in v3.0.0**: Template syntax changed from `${VAR}` to `{VAR}` for enhanced security. See [Template Variables](#template-variables) section for details.

## Overview

Peter Hook enables different paths within a monorepo to have their own custom git hooks while maintaining repository-wide quality standards. It features intelligent parallel execution that respects file system safety - read-only hooks run concurrently for speed, while repository-modifying hooks run sequentially to prevent conflicts.

## Key Features

- **🏗️ Per-File Hierarchical Resolution**: Each changed file finds its nearest `.peter-hook.toml`, enabling true monorepo patterns with path-specific validation
- **⚡ Safe Parallel Execution**: Automatic parallelization of compatible hooks for 2-3x speed improvement
- **🔗 Hook Composition**: Combine individual hooks into reusable groups with dependency management
- **🛡️ Repository Safety**: File-modifying hooks never run simultaneously, preventing race conditions
- **🎯 Smart Fallback**: Configs inherit missing events from parent directories automatically
- **🌳 Git Worktree Support**: Native support for git worktrees with flexible hook installation strategies
- **🌍 Cross-Platform**: Native binaries for macOS, Linux, and Windows
- **📦 Easy Installation**: Single-command installation with automatic PATH setup

## Quick Start

### Installation

#### Quick Install (Recommended)

Install the latest release directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/workhelix/peter-hook/main/install.sh | sh
```

Or with a custom install directory:

```bash
INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/workhelix/peter-hook/main/install.sh | sh
```

The install script will:
- Auto-detect your OS and architecture
- Download the latest release
- Verify checksums (when available)
- Install to `$HOME/.local/bin` by default
- Prompt before replacing existing installations
- Guide you on adding the directory to your PATH

#### Alternative Install Methods

**Option A — Using GitHub CLI:**

```bash
# Create installation directory
mkdir -p "$HOME/.local/bin"

# Download and extract latest release
gh release download --repo workhelix/peter-hook --pattern '*-apple-darwin.zip' -O - | funzip > "$HOME/.local/bin/peter-hook"
chmod +x "$HOME/.local/bin/peter-hook"

# Add to PATH if not already present
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc  # or ~/.zshrc
source ~/.bashrc  # or source ~/.zshrc

# Verify installation
peter-hook --version
```

**Option B — Manual Download:**

1. Visit [Releases](https://github.com/workhelix/peter-hook/releases)
2. Download the appropriate `peter-hook-{target}.zip` for your platform
3. Extract and copy the binary to a directory in your PATH

**Option C — From Source:**

```bash
git clone https://github.com/workhelix/peter-hook.git
cd peter-hook
cargo build --release
install -m 0755 target/release/peter-hook ~/.local/bin/
```

#### Supported Platforms

- **Linux**: x86_64, aarch64
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)
- **Windows**: x86_64

### Basic Usage

1. **Create a configuration file** (`.peter-hook.toml`):

```toml
# Individual hooks
[hooks.lint]
command = "cargo clippy --all-targets -- -D warnings"
description = "Run Clippy linter"
modifies_repository = false

[hooks.test]
command = "cargo test --all"
description = "Run test suite"
modifies_repository = false

[hooks.format]
command = "cargo fmt"
description = "Format code"
modifies_repository = true

# Hook group with parallel execution
[groups.pre-commit]
includes = ["lint", "test", "format"]
execution = "parallel"
description = "Pre-commit validation - safe hooks run parallel, format runs after"
```

2. **Validate your configuration**:

```bash
peter-hook validate
```

3. **Run hooks manually**:

```bash
# Run hooks for a git event (only changed files)
peter-hook run pre-commit

# Run individual hook in lint mode (all matching files)
peter-hook lint ruff-check
```

4. **Install git hooks**:

```bash
peter-hook install
```

## Configuration Reference

### Complete Hook Definition

```toml
[hooks.example]
# REQUIRED: Command to execute
command = "echo hello"                      # String format
# OR
command = ["echo", "hello", "world"]        # Array format (preferred for complex commands)

# REQUIRED: Repository safety flag
modifies_repository = false                 # true = modifies files, false = read-only

# OPTIONAL: Description
description = "Example hook description"

# OPTIONAL: File targeting (performance optimization)
files = ["**/*.rs", "Cargo.toml"]          # Glob patterns - hook only runs if these files changed
run_always = false                         # true = ignore file changes, always run

# OPTIONAL: Hook dependencies  
depends_on = ["format", "setup"]           # This hook runs after these hooks complete successfully

# OPTIONAL: Working directory
workdir = "custom/path"                    # Relative to config file directory
# OR with templating
workdir = "{REPO_ROOT}/backend"           # Template variables available

# OPTIONAL: Environment variables
env = { KEY = "value" }                    # Simple key-value pairs
# OR with templating
env = { 
    PROJECT_ROOT = "{REPO_ROOT}", 
    BUILD_DIR = "{HOOK_DIR}/target",
    PROJECT_NAME = "{PROJECT_NAME}"
}
```

### Hook Groups

```toml
[groups.example-group]
# REQUIRED: Hooks and groups to include
includes = ["hook1", "hook2", "other-group"]

# OPTIONAL: Execution strategy
execution = "parallel"                     # parallel | sequential | force-parallel

# OPTIONAL: Description  
description = "Example group description"

# DEPRECATED (but supported): Legacy parallel flag
parallel = true                            # Use execution = "parallel" instead
```

### Imports (Hook Libraries)

Share and reuse hooks/groups across files, with local overrides.

```toml
# .peter-hook.toml (project)
imports = ["../hooks.lib.toml", ".hooks/common.toml"]

[groups.pre-commit]
includes = ["lint", "format", "test"]   # names from imported files

# Local definitions override imported ones on name conflicts
[hooks.lint]
command = "cargo clippy -- -D warnings"
modifies_repository = false
```

Rules:
- Paths must be relative to the importing file (absolute paths are rejected).
- All imported files must reside under the repository root.
- Imports are merged in listed order; later imports override earlier ones; local overrides all.
- Recursive imports supported with cycle detection; cycles are ignored safely.

### Execution Strategies Explained

- **`sequential`** (default): Run hooks one after another, respecting dependencies
- **`parallel`**: Intelligent execution:
  - Repository-safe hooks (`modifies_repository = false`) run in parallel
  - Repository-modifying hooks run sequentially  
  - Dependencies always respected
- **`force-parallel`**: Force all hooks to run in parallel (dangerous - can cause file conflicts)

### Template Variables

Peter Hook supports powerful template variables in commands, working directories, and environment variables:

#### Built-in Variables
```toml
{HOOK_DIR}         # Directory containing the .peter-hook.toml file
{WORKING_DIR}      # Current working directory when hook runs
{REPO_ROOT}        # Git repository root directory
{HOOK_DIR_REL}     # Relative path from repo root to hook directory
{WORKING_DIR_REL}  # Relative path from repo root to working directory
{PROJECT_NAME}     # Name of the directory containing .peter-hook.toml
{HOME_DIR}         # User's home directory
{IS_WORKTREE}      # "true" or "false" - whether running in a worktree
{WORKTREE_NAME}    # Name of current worktree (only available in worktrees)
{COMMON_DIR}       # Path to shared git directory (across worktrees)
{CHANGED_FILES}    # Space-delimited list of changed files (file filtering enabled)
{CHANGED_FILES_LIST} # Newline-delimited list of changed files (file filtering enabled)
{CHANGED_FILES_FILE} # Path to temp file containing changed files (file filtering enabled)
```

#### Security Note & Breaking Changes

**Breaking Change in v1.1.0**: Template syntax has changed from shell-style `${VAR}` to secure `{VAR}` syntax:

- ❌ **Old (removed)**: `${VARIABLE_NAME}`, `${PWD##*/}`
- ✅ **New (secure)**: `{VARIABLE_NAME}`, `{PROJECT_NAME}`

For security reasons, only the predefined template variables listed above are available. Arbitrary environment variables are not exposed to prevent potential security vulnerabilities.

#### Template Examples
```toml
[hooks.build]
command = "cargo build --manifest-path={HOOK_DIR}/Cargo.toml"
workdir = "{REPO_ROOT}/target"
env = { 
    CARGO_TARGET_DIR = "{HOOK_DIR}/target",
    PROJECT_NAME = "{PROJECT_NAME}",
    BUILD_MODE = "debug"                     # Static values - no shell expansions
}

[hooks.test-with-context]
command = ["cargo", "test", "--manifest-path={HOOK_DIR}/Cargo.toml", "--", "--test-threads=1"]
description = "Test {PROJECT_NAME} in {HOOK_DIR_REL}"
```

### Git Worktree Support

Peter Hook provides native support for Git worktrees with flexible hook installation strategies and worktree-aware template variables.

#### Worktree Installation Strategies

Choose how hooks are installed across worktrees:

```bash
# Shared hooks (default): All worktrees use the same hooks
peter-hook install --worktree-strategy shared

# Per-worktree hooks: Each worktree has its own hooks
peter-hook install --worktree-strategy per-worktree

# Auto-detect: Use existing strategy if found, otherwise default to shared
peter-hook install --worktree-strategy detect
```

#### Worktree-Specific Template Variables

In addition to standard variables, worktrees provide additional context:

```toml
{IS_WORKTREE}      # "true" or "false" - whether running in a worktree
{WORKTREE_NAME}    # Name of the current worktree (only available in worktrees)
{COMMON_DIR}       # Path to the shared git directory across worktrees
```

#### Worktree Template Examples

```toml
[hooks.worktree-context]
command = "echo 'In worktree: {IS_WORKTREE}'"
description = "Show worktree context"

[hooks.worktree-specific]
command = "echo 'Working in {WORKTREE_NAME}'"
description = "Show current worktree name"

[hooks.backup-logs]
command = "cp logs/*.log {COMMON_DIR}/backup/"
description = "Backup logs to shared directory"
```

#### Managing Worktrees

List all worktrees and their hook status:

```bash
peter-hook list-worktrees
```

Example output:
```
Git worktrees in this repository:
=================================
📁 main [main] (current)
   Path: /Users/dev/project
   Hooks (shared): pre-commit, pre-push

📁 feature-auth
   Path: /Users/dev/project-auth
   Hooks (shared): pre-commit, pre-push

📁 hotfix-123
   Path: /Users/dev/project-hotfix
   Hooks (worktree-specific): pre-commit, custom-deploy
```

#### Worktree Configuration Examples

Per-worktree configuration allows different branches to have different validation requirements:

```toml
# Main repository .peter-hook.toml - Strict validation
[hooks.full-test-suite]
command = "cargo test --all --release"
modifies_repository = false
description = "Complete test suite for main branch"

[hooks.security-audit]
command = "cargo audit"
modifies_repository = false 
description = "Security audit for production"

[groups.pre-commit]
includes = ["full-test-suite", "security-audit"]
execution = "parallel"
```

```toml
# Feature branch worktree .peter-hook.toml - Fast iteration
[hooks.quick-test]
command = "cargo test --lib"
modifies_repository = false
description = "Quick unit tests for development"

[hooks.format-check]  
command = "cargo fmt --check"
modifies_repository = false
description = "Format check only"

[groups.pre-commit]
includes = ["quick-test", "format-check"]
execution = "parallel"
```

### File Pattern Targeting

Optimize performance by only running hooks when relevant files change:

#### Pattern Syntax (Glob Patterns)
```toml
# Language-specific patterns
files = ["**/*.rs", "Cargo.toml"]                    # Rust files
files = ["**/*.py", "requirements*.txt", "*.py"]     # Python files  
files = ["**/*.js", "**/*.ts", "package.json"]       # JavaScript/TypeScript
files = ["**/*.go", "go.mod", "go.sum"]              # Go files

# Build and configuration patterns
files = ["Dockerfile*", "docker-compose*.yml"]       # Docker files
files = ["**/*.yml", "**/*.yaml"]                    # YAML files
files = ["*.toml", "**/*.toml"]                      # TOML files
files = ["**/*.json"]                                # JSON files

# Documentation patterns
files = ["**/*.md", "docs/**/*", "README*"]          # Documentation
files = ["**/*.rst", "**/*.txt"]                     # Text documentation

# Mixed patterns
files = ["src/**/*.rs", "tests/**/*.rs", "Cargo.*"]  # Rust source and config
files = ["frontend/**/*", "!frontend/node_modules"]  # Frontend (excluding node_modules)
```

#### File Targeting Behavior
```toml
# No files specified = always run
[hooks.always-run-hook]
command = "security-scan"
# (no files field means run regardless of changes)

# Specific files = run only if those files changed  
[hooks.rust-only]
command = "cargo clippy"
files = ["**/*.rs"]
# Runs only if .rs files changed

# Force always run = ignore file changes
[hooks.critical-security]
command = "secret-scan" 
files = ["**/*"]           # Would normally check all files
run_always = true          # But this overrides and always runs
```

### Hook Dependencies

Control execution order with dependencies:

```toml
# Basic dependency chain
[hooks.format]
command = "cargo fmt"
modifies_repository = true

[hooks.lint] 
command = "cargo clippy"
depends_on = ["format"]              # Runs after format completes successfully
modifies_repository = false

[hooks.test]
command = "cargo test" 
depends_on = ["lint"]                # Runs after lint completes successfully
modifies_repository = false

# Multiple dependencies
[hooks.integration-test]
command = "cargo test --test integration"
depends_on = ["lint", "test"]        # Runs after BOTH lint and test complete
modifies_repository = false

# Complex dependency tree
[hooks.build]
command = "cargo build"
depends_on = ["format", "lint"]

[hooks.package]
command = "cargo package" 
depends_on = ["test", "build"]       # Depends on test AND build

[groups.release-pipeline]
includes = ["format", "lint", "test", "build", "package"]
execution = "sequential"             # Respects all dependencies
```

### Advanced Command Usage

#### Enable File Targeting
```bash
# Run with automatic file detection (faster)
peter-hook run pre-commit --files

# Run all hooks regardless of files (slower but complete)
peter-hook run pre-commit
```

#### Validate With Import Diagnostics
```bash
# Basic validation
peter-hook validate

# Trace imports (order, overrides, cycles, unused)
peter-hook validate --trace-imports

# JSON diagnostics (useful for tooling)
peter-hook validate --trace-imports --json
```

#### Git Integration
```bash
# Install hooks to run automatically with git
peter-hook install


# Install with force (backup existing hooks)
peter-hook install --force

# Install with worktree strategy
peter-hook install --worktree-strategy shared
peter-hook install --worktree-strategy per-worktree

# List all git hooks
peter-hook list

# List all worktrees and their hooks
peter-hook list-worktrees

# Uninstall peter-hook managed hooks
peter-hook uninstall

# Uninstall without confirmation
peter-hook uninstall --yes
```

#### Hook Management
```bash
# Validate configuration
peter-hook validate

# Validate with import diagnostics
peter-hook validate --trace-imports

# Run hooks for a git event (only changed files)
peter-hook run pre-commit

# Run all files for a git event (ignore change detection)
peter-hook run pre-commit --all-files

# Run hook in lint mode (all matching files)
peter-hook lint ruff-check

# Test hook with git arguments (for commit-msg, pre-push hooks)
peter-hook run commit-msg /tmp/commit-msg-file
```

#### Lint Mode

Lint mode runs hooks on **all matching files** in the current directory (and subdirectories), treating the current directory as the repository root. This is useful for linting tools, formatters, and validators that should check all files, not just changed ones.

**Key Differences from Regular Hooks:**
- Current directory is treated as the repository root (not git root)
- Discovers **all** non-ignored files matching patterns (not just changed files)
- Respects `.gitignore` rules hierarchically up to the git root
- No git operations are performed

```bash
# Run a hook in lint mode
peter-hook lint <hook-name>

# Test what would run without executing
peter-hook lint <hook-name> --dry-run
```

**Example: Python Linting**

```toml
[hooks.ruff-check]
command = ["uvx", "ruff", "check", "--fix"]
description = "Run ruff linter with auto-fix"
modifies_repository = true
files = ["**/*.py"]
```

```bash
# Lint mode: checks ALL .py files in current directory
peter-hook lint ruff-check
```

**Example: In-Place Hook**

```toml
[hooks.unvenv]
command = ["unvenv"]
description = "Prevent Python virtual environments in Git"
modifies_repository = false
execution_type = "in-place"
files = ["**/*.py"]
```

```bash
# Runs unvenv once in config directory
peter-hook lint unvenv
```

**Example: Custom PATH with Lint Mode**

```toml
[hooks.my-custom-tool]
command = ["my-tool", "--check"]
description = "Run custom linting tool"
modifies_repository = false
files = ["**/*.rs"]
# Extend PATH to include custom bin directory
env = { PATH = "{HOME_DIR}/.local/bin:{PATH}" }
```

```bash
# Run custom tool on all Rust files
peter-hook lint my-custom-tool
```

**Example: Hook Groups in Lint Mode**

```toml
[hooks.python-format]
command = ["uvx", "ruff", "format"]
modifies_repository = true
files = ["**/*.py"]

[hooks.python-lint]
command = ["uvx", "ruff", "check"]
modifies_repository = false
files = ["**/*.py"]
depends_on = ["python-format"]

[groups.python-quality]
includes = ["python-format", "python-lint"]
execution = "parallel"
```

```bash
# Run entire group on all Python files
peter-hook lint python-quality
```

**When to Use Lint Mode:**
- 📋 Running formatters/linters on entire codebase
- 🔍 Pre-CI validation of all files in a directory
- 🧹 Cleaning up code quality across entire subproject
- ✅ Validating directory-specific requirements (like `unvenv`)
- 🚀 One-off checks without git operations

**Lint Mode Behavior by Execution Type:**
- `per-file` (default): All matching files passed as arguments → `tool file1.py file2.py file3.py`
- `in-place`: Hook runs once in config directory, tool auto-discovers files → `pytest`, `jest`, `unvenv`
- `other`: Hook receives file list via template variables (`{CHANGED_FILES}`, `{CHANGED_FILES_LIST}`, etc.)

#### Global Configuration
```bash
# Show current global configuration
peter-hook config show

# Initialize global configuration (with absolute imports disabled)
peter-hook config init

# Initialize with absolute imports enabled
peter-hook config init --allow-local

# Validate global configuration
peter-hook config validate
```

## Hierarchical Configuration

Peter Hook implements **true per-file hierarchical resolution** where each changed file independently finds its nearest configuration. This enables powerful monorepo patterns where different subdirectories have different quality gates.

### How It Works

When you run a git hook (e.g., `pre-commit`), Peter Hook:

1. **Detects all changed files** from git (staged, working directory, or push range)
2. **For each changed file**, walks up from that file's directory to find the nearest `.peter-hook.toml`
3. **Checks if that config defines the requested event** (e.g., `pre-commit`)
4. **Falls back to parent configs** if the event isn't defined locally
5. **Groups files** that share the same configuration
6. **Executes each group's hooks** from that config's directory

### Example Hierarchy

```
/monorepo/
├── .git
├── .peter-hook.toml                          # Defines: pre-push
├── backend/
│   ├── .peter-hook.toml                      # Defines: pre-commit
│   └── api/
│       ├── .peter-hook.toml                  # Defines: pre-push
│       └── server.rs                   # File A
└── frontend/
    └── app.js                          # File B
```

**Scenario: Modify `backend/api/server.rs` (File A)**
- **pre-commit hook**: Uses `/monorepo/backend/.peter-hook.toml` (walks up, finds first config with pre-commit)
- **pre-push hook**: Uses `/monorepo/backend/api/.peter-hook.toml` (nearest config defines it)

**Scenario: Modify `frontend/app.js` (File B)**
- **pre-commit hook**: Uses `/monorepo/.peter-hook.toml` (no frontend/.peter-hook.toml, falls back to root)
- **pre-push hook**: Uses `/monorepo/.peter-hook.toml` (defined at root)

**Scenario: Modify both files simultaneously**
- Peter Hook executes hooks from **both** configs in the same commit:
  - `backend/api/server.rs` → runs hooks from `backend/` and `backend/api/`
  - `frontend/app.js` → runs hooks from root `/`
  - All hooks run in their respective directories with correct context

### Fallback Resolution

If a config doesn't define the requested event, Peter Hook automatically searches parent directories:

```
/monorepo/
├── .peter-hook.toml                          # Defines: pre-commit, pre-push
└── microservices/
    ├── .peter-hook.toml                      # Defines: pre-commit only
    └── auth/
        └── src/
            └── lib.rs
```

When `microservices/auth/src/lib.rs` is modified:
- **pre-commit**: Uses `/monorepo/microservices/.peter-hook.toml` (found locally)
- **pre-push**: Uses `/monorepo/.peter-hook.toml` (falls back to parent, `microservices/.peter-hook.toml` doesn't define it)

### Benefits

**🎯 Path-Specific Quality Gates**
- Backend requires type checking and integration tests
- Frontend requires bundle size checks and visual regression tests
- Shared libraries require extra validation
- Each team controls their own standards

**⚡ Selective Execution**
- Only run hooks for the paths that actually changed
- Multiple teams can work simultaneously without interference
- Faster feedback loops for focused changes

**🔧 Gradual Migration**
- Add strict rules to new code without breaking legacy code
- Incrementally adopt standards across a large codebase
- Experimental features can have their own validation

**📦 Logical Boundaries**
- Each microservice, package, or module has its own hooks
- Monorepo structure matches development team structure
- Clear ownership and responsibility boundaries

### Real-World Example

```toml
# /monorepo/.peter-hook.toml - Repository-wide safety net
[groups.pre-push]
includes = ["security-scan", "secret-detection"]
execution = "parallel"
description = "Security checks for all code"

# /monorepo/backend/.peter-hook.toml - Backend quality standards
[groups.pre-commit]
includes = ["rust-format", "rust-clippy", "rust-test"]
execution = "parallel"
description = "Rust validation pipeline"

# /monorepo/frontend/.peter-hook.toml - Frontend quality standards
[groups.pre-commit]
includes = ["prettier", "eslint", "jest"]
execution = "parallel"
description = "JavaScript validation pipeline"

# /monorepo/shared/.peter-hook.toml - Library quality standards
[groups.pre-commit]
includes = ["format", "lint", "test", "doc-check", "api-compat"]
execution = "sequential"
description = "Strict validation for shared code"
```

**Result**: Each team's hooks run automatically based on which files they touch, with zero configuration by developers.

## Complete Real-World Examples

### Multi-Language Monorepo Configuration

```toml
# ===== RUST BACKEND =====
[hooks.rust-format]
command = "cargo fmt --manifest-path={HOOK_DIR}/Cargo.toml"
description = "Format Rust code"
modifies_repository = true
files = ["**/*.rs"]
workdir = "{REPO_ROOT}/backend"

[hooks.rust-lint]
command = "cargo clippy --manifest-path={HOOK_DIR}/Cargo.toml -- -D warnings"
description = "Lint Rust code (after formatting)"
modifies_repository = false
files = ["**/*.rs", "Cargo.toml"]
depends_on = ["rust-format"]
workdir = "{REPO_ROOT}/backend"

[hooks.rust-test]
command = "cargo test --manifest-path={HOOK_DIR}/Cargo.toml"
description = "Test Rust code (after linting)"
modifies_repository = false  
files = ["**/*.rs", "Cargo.toml"]
depends_on = ["rust-lint"]
workdir = "{REPO_ROOT}/backend"
env = { RUST_BACKTRACE = "1" }

# ===== FRONTEND =====
[hooks.js-format]
command = "npm run format"
description = "Format JavaScript/TypeScript"
modifies_repository = true
files = ["**/*.js", "**/*.ts", "**/*.jsx", "**/*.tsx"]
workdir = "{REPO_ROOT}/frontend"

[hooks.js-lint]  
command = "npm run lint"
description = "Lint JavaScript/TypeScript (after formatting)"
modifies_repository = false
files = ["**/*.js", "**/*.ts", "package.json"]
depends_on = ["js-format"]
workdir = "{REPO_ROOT}/frontend"

[hooks.js-test]
command = "npm test -- --passWithNoTests"
description = "Test JavaScript/TypeScript (after linting)"
modifies_repository = false
files = ["**/*.js", "**/*.ts", "**/*.test.*"]
depends_on = ["js-lint"]
workdir = "{REPO_ROOT}/frontend"

# ===== SECURITY (ALWAYS RUN) =====
[hooks.security-scan]
command = "semgrep --config=auto {REPO_ROOT}"
description = "Security scan (always runs)"
modifies_repository = false
run_always = true

[hooks.secret-scan]
command = "gitleaks detect --source={REPO_ROOT}"
description = "Secret detection (always runs)"  
modifies_repository = false
run_always = true

# ===== SMART GROUPS =====
[groups.backend-pipeline]
includes = ["rust-format", "rust-lint", "rust-test"]
execution = "sequential"  # Respects dependencies: format → lint → test
description = "Complete backend validation"

[groups.frontend-pipeline]
includes = ["js-format", "js-lint", "js-test"] 
execution = "sequential"  # Respects dependencies: format → lint → test
description = "Complete frontend validation"

[groups.security-suite]
includes = ["security-scan", "secret-scan"]
execution = "parallel"    # Both always run, no dependencies
description = "Security validation"

# ===== MAIN HOOKS =====
[groups.pre-commit]
includes = ["backend-pipeline", "frontend-pipeline", "security-suite"]
execution = "parallel"
description = "Complete validation with file targeting and dependencies"
# Result: Only relevant language pipelines run based on changed files
#         Security always runs regardless of changes
#         Dependencies respected within each pipeline

[groups.pre-push] 
includes = ["security-suite", "rust-test", "js-test"]
execution = "parallel"
description = "Quick validation before push"
```

### File Targeting in Action

```bash
# Scenario 1: Only Rust files changed
# Changed files: ["backend/src/lib.rs", "backend/Cargo.toml"]
peter-hook run-hook pre-commit
# Result: Only rust-format → rust-lint → rust-test + security runs
#         Frontend pipeline skipped (no JS/TS files changed)
#         5x faster than running everything

# Scenario 2: Only documentation changed
# Changed files: ["README.md", "docs/api.md"]
peter-hook run-hook pre-commit  
# Result: Only security-suite runs (has run_always = true)
#         All language-specific hooks skipped
#         10x faster than running everything

# Scenario 3: Mixed changes
# Changed files: ["backend/src/lib.rs", "frontend/src/app.js", "package.json"]
peter-hook run-hook pre-commit
# Result: backend-pipeline + frontend-pipeline + security-suite
#         All relevant hooks run, nothing wasted

# Scenario 4: Run everything regardless of files
peter-hook run-hook pre-commit --all-files
# Result: All hooks run (slower but comprehensive)
```

### Performance Benefits

**Real-World Monorepo Impact:**
- **🚀 5-10x faster** for single-language changes
- **🎯 Intelligent targeting** - only run what's needed
- **⚡ Parallel execution** where safe
- **🔒 Dependency guarantees** - formatters always run before linters
- **🛡️ Repository safety** - no file conflicts from parallel modification

## Repository Structure

This project follows standard Rust conventions:

- **`src/config/`**: TOML configuration parsing and validation
- **`src/hooks/`**: Hook resolution and execution engine  
- **`src/cli/`**: Command-line interface
- **`tests/`**: Integration tests and test fixtures
- **`examples/`**: Example configurations

## Development

### Prerequisites

- Rust 1.70.0 or later
- Standard development tools (git, etc.)

### Building

```bash
# Clone and build
git clone https://github.com/workhelix/peter-hook.git
cd peter-hook
cargo build --release

# Run tests
cargo test

# Run with strict linting
cargo clippy --all-targets -- -D warnings
```

### Running Locally

```bash
# Validate configuration
cargo run -- validate

# Trace imports (human-readable)
cargo run -- validate --trace-imports

# Trace imports (JSON)
cargo run -- validate --trace-imports --json

# Run hooks
cargo run -- run pre-commit

# See all options
cargo run -- --help
```

## Version Management

**Automated Release Process** - This project uses `versioneer` for atomic version management:

### Required Tools
- **`versioneer`**: Synchronizes versions across Cargo.toml and VERSION files
- **`peter-hook`**: Git hooks enforce version consistency validation
- **Automated release script**: `./scripts/release.sh` handles complete release workflow

### Version Management Rules
1. **NEVER manually edit Cargo.toml version** - Use versioneer instead
2. **NEVER create git tags manually** - Use `versioneer tag` or release script
3. **ALWAYS use automated release workflow** - Prevents version/tag mismatches

### Release Commands
```bash
# Automated release (recommended)
./scripts/release.sh patch   # 1.0.10 -> 1.0.11
./scripts/release.sh minor   # 1.0.10 -> 1.1.0
./scripts/release.sh major   # 1.0.10 -> 2.0.0

# Manual version management (advanced)
versioneer patch             # Bump version
versioneer sync              # Synchronize version files
versioneer verify            # Check version consistency
versioneer tag               # Create matching git tag
```

### Quality Gates
- **Pre-push hooks**: Verify version file synchronization and tag consistency
- **GitHub Actions**: Validate tag version matches Cargo.toml before release
- **Binary verification**: Confirm built binary reports expected version
- **Release script**: Runs full quality pipeline (tests, lints, audits) before release

### Troubleshooting
- **Version mismatch errors**: Run `versioneer verify` and `versioneer sync`
- **Tag conflicts**: Use `versioneer tag` instead of `git tag`
- **Failed releases**: Check GitHub Actions logs for version validation errors

## Contributing

We welcome contributions! Please see our comprehensive CI/CD pipeline that ensures:

- ✅ Zero compiler warnings with strict linting
- ✅ High test coverage requirement (70%+ minimum)
- ✅ Cross-platform compatibility testing
- ✅ Security audits and dependency management
- ✅ Automated releases with checksums

## Architecture

Built with safety and performance in mind:

- **Thread-safe parallel execution** using Rust's ownership system
- **Zero-copy configuration** with efficient TOML parsing
- **Minimal dependencies** for security and reliability
- **Cross-platform native binaries** for optimal performance
- **Comprehensive error handling** with detailed diagnostics

## License

MIT License - see [LICENSE](LICENSE) file for details.