# Changelog

All notable changes to this project will be documented in this file.

## [5.0.0] - 2025-12-10

### BREAKING CHANGES

- **Configuration file renamed from `hooks.toml` to `.peter-hook.toml`**
  - Peter-hook now searches for `.peter-hook.toml` instead of `hooks.toml`
  - If `hooks.toml` files are detected, peter-hook will error and refuse to run
  - Migration: Rename all `hooks.toml` files to `.peter-hook.toml`
  - Commands affected: All commands except `version` and `license`

**Migration guide:**

For single configuration:
```bash
mv hooks.toml .peter-hook.toml
```

For monorepos with multiple configurations:
```bash
# Find all deprecated files
find . -name "hooks.toml" -type f

# Rename each one
cd backend && mv hooks.toml .peter-hook.toml
cd ../frontend && mv hooks.toml .peter-hook.toml
```

**Why this change:**
- Follows dotfile conventions for tool configuration
- Reduces visual clutter in repository root
- Makes the file more discoverable as tool-specific config
- Aligns with modern CLI tool practices

## [Unreleased]

### Added
- **Per-file hierarchical hook resolution**: Each changed file now independently finds its nearest `hooks.toml` configuration
  - Files in different subdirectories can use different hook configurations in the same commit
  - Automatic fallback to parent configs when a specific event is not defined locally
  - Groups changed files by their resolved configuration for efficient execution
  - Enables true monorepo patterns with path-specific validation rules
- New `src/hooks/hierarchical.rs` module implementing the per-file resolution system
- `HookExecutor::execute_multiple()` method for executing hooks from multiple configuration groups

### Changed
- **Breaking**: Hook resolution now operates per-file instead of finding a single config for the entire repository
  - Previously: One `hooks.toml` applied to all changed files
  - Now: Each file finds its nearest `hooks.toml`, allowing different subdirectories to have different hooks
- Updated `run_hooks()` in `main.rs` to use hierarchical resolution by default
- Enhanced documentation in README.md with detailed hierarchical resolution examples

### Technical Details
- Hook resolution walks up from each changed file's directory to find the nearest `hooks.toml`
- Configs can define some events (e.g., `pre-commit`) while inheriting others (e.g., `pre-push`) from parent directories
- Multiple config groups execute sequentially, with results aggregated into a single report
- All existing tests pass, plus 2 new tests for hierarchical resolution

## [1.0.9] - 2025-09-23

### Added
- Added `license` subcommand to display MIT license information

### Changed
- Moved from `help` subcommand to standard `--help` flag using clap

### Fixed
- Fixed install script `temp_dir` variable scoping issue in EXIT trap that caused "unbound variable" errors

## [1.0.8] - 2025-09-23

### Fixed
- Fixed install script bug where log messages were outputting to stdout instead of stderr, causing version detection to fail with "bad range in URL" error

## [0.3.0] - 2025-09-10

### Added
- Expose changed files to hook commands via environment variables:
  - `CHANGED_FILES`: space-delimited list of repo-relative paths
  - `CHANGED_FILES_LIST`: newline-delimited list of repo-relative paths
  - `CHANGED_FILES_FILE`: absolute path to a temporary file containing the newline-delimited list
- Per-hook filtering of changed files based on the hook's `files = [..]` patterns
- Documentation in README for the new environment variables

### Notes
- Variables are populated when running with `--files`; otherwise they are set but empty (`CHANGED_FILES_FILE` is an empty string).
- Backward compatible; no breaking changes.
