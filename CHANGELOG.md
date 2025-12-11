# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
- **BREAKING**: Removed configuration merging/inheritance behavior
  - Previously: Child `hooks.toml` files would merge with parent configs, inheriting and extending hook definitions
  - Now: Each `hooks.toml` is completely standalone - no inheritance from parent directories
  - Impact: If you relied on child configs inheriting hooks from parents, you must now explicitly redefine all hooks needed in each child config
  - Rationale: Simplifies behavior - each directory's hooks are self-contained and explicit
- Updated documentation to clarify per-directory independent configuration
- Removed obsolete merge-related tests and functions

### Technical Details
- Removed `merge_configs_for_event()` and related merging infrastructure from `src/hooks/hierarchical.rs`
- Replaced `find_all_configs_for_file()` with simpler `find_nearest_config_for_file()`
- Each directory still uses its nearest `hooks.toml`, but only that config is used (no walking up to merge parents)
- Multi-config execution still works: different directories can have different configs in the same commit

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
