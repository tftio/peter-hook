//! Hierarchical hook resolution for monorepos
//!
//! This module implements per-file hook resolution where each changed file
//! finds its nearest hooks.toml and uses that configuration. This enables
//! monorepo-style setups where different subdirectories have different quality
//! gates.

use crate::{
    config::{ExecutionStrategy, HookConfig, HookDefinition},
    git::ChangeDetectionMode,
    hooks::{HookResolver, ResolvedHooks, WorktreeContext},
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// A group of files that share the same hook configuration
#[derive(Debug, Clone)]
pub struct ConfigGroup {
    /// The configuration file path
    pub config_path: PathBuf,
    /// Files that use this configuration
    pub files: Vec<PathBuf>,
    /// Resolved hooks for this configuration
    pub resolved_hooks: ResolvedHooks,
}

/// Find all hooks.toml files for a given file path from nearest to root
///
/// Walks up from the file's directory to collect all hooks.toml files.
/// Stops at the repository root.
///
/// # Arguments
///
/// * `file_path` - The file to find configs for
/// * `repo_root` - The repository root (don't search above this)
///
/// # Returns
///
/// Vector of config paths ordered from nearest to root, or empty if none found
fn find_all_configs_for_file(file_path: &Path, repo_root: &Path) -> Vec<PathBuf> {
    let mut configs = Vec::new();

    // Start from the file's directory
    let mut current = if file_path.is_file() {
        match file_path.parent() {
            Some(p) => p,
            None => return configs,
        }
    } else {
        file_path
    };

    // Canonicalize paths for comparison
    let Ok(repo_root_canonical) = repo_root.canonicalize() else {
        return configs;
    };

    loop {
        let config_path = current.join("hooks.toml");
        if config_path.exists() {
            configs.push(config_path);
        }

        // Check if we've reached the repo root
        if let Ok(current_canonical) = current.canonicalize() {
            if current_canonical == repo_root_canonical {
                break;
            }
        }

        // Move up one directory
        current = match current.parent() {
            Some(p) => p,
            None => break,
        };
    }

    configs
}

/// Merged configuration result containing hooks and execution strategy
#[derive(Debug)]
struct MergedConfig {
    /// Merged hook definitions (nearest wins for duplicates)
    hooks: HashMap<String, HookDefinition>,
    /// Merged execution strategy (most conservative)
    execution_strategy: ExecutionStrategy,
    /// The nearest config path (for working directory resolution)
    nearest_config_path: PathBuf,
}

/// Merge multiple config files for a specific event
///
/// Merges configurations from nearest to root:
/// - Groups: Extends includes lists (child adds to parent)
/// - Hooks: Nearest definition wins (deduplication)
/// - Execution: Most conservative (any sequential → all sequential)
///
/// # Arguments
///
/// * `config_paths` - Config paths ordered from nearest to root
/// * `event` - The git hook event name
///
/// # Returns
///
/// Merged configuration or None if event not defined in any config
///
/// # Errors
///
/// Returns an error if config file parsing fails
fn merge_configs_for_event(
    config_paths: &[PathBuf],
    event: &str,
) -> Result<Option<MergedConfig>> {
    if config_paths.is_empty() {
        return Ok(None);
    }

    let mut all_hook_names: Vec<String> = Vec::new();
    let mut execution_strategies: Vec<ExecutionStrategy> = Vec::new();
    let mut hook_definitions: HashMap<String, HookDefinition> = HashMap::new();
    let mut event_found = false;

    // PHASE 1: Collect all hook names and execution strategies from all configs
    // Process from ROOT to NEAREST to build up the merged includes list
    for config_path in config_paths.iter().rev() {
        let config = HookConfig::from_file(config_path)
            .with_context(|| format!("Failed to load config: {}", config_path.display()))?;

        // Check if this config defines the event as a direct hook
        if let Some(hooks) = &config.hooks {
            if let Some(_hook_def) = hooks.get(event) {
                event_found = true;
                if !all_hook_names.contains(&event.to_string()) {
                    all_hook_names.push(event.to_string());
                }
                execution_strategies.push(ExecutionStrategy::Sequential); // Direct hooks are sequential
            }
        }

        // Check if this config defines the event as a group
        if let Some(groups) = &config.groups {
            if let Some(group) = groups.get(event) {
                event_found = true;
                execution_strategies.push(group.get_execution_strategy());

                // Extend the includes list (child adds to parent)
                for include_name in &group.includes {
                    // Add to list if not already present (maintains order, root first)
                    if !all_hook_names.contains(include_name) {
                        all_hook_names.push(include_name.clone());
                    }
                }
            }
        }
    }

    if !event_found {
        return Ok(None);
    }

    // PHASE 2: Find hook definitions for all collected hook names
    // Search from NEAREST to ROOT so that nearest definitions win
    for hook_name in &all_hook_names {
        for config_path in config_paths {
            let config = HookConfig::from_file(config_path)
                .with_context(|| format!("Failed to load config: {}", config_path.display()))?;

            if let Some(hooks) = &config.hooks {
                if let Some(hook_def) = hooks.get(hook_name) {
                    // Insert if not already present (first one found = nearest = wins)
                    hook_definitions
                        .entry(hook_name.clone())
                        .or_insert_with(|| hook_def.clone());
                    break; // Found definition, stop searching for this hook
                }
            }
        }
    }

    // Merge execution strategies: if ANY config says sequential, use sequential
    let execution_strategy = if execution_strategies
        .iter()
        .any(|s| matches!(s, ExecutionStrategy::Sequential))
    {
        ExecutionStrategy::Sequential
    } else {
        ExecutionStrategy::Parallel
    };

    Ok(Some(MergedConfig {
        hooks: hook_definitions,
        execution_strategy,
        nearest_config_path: config_paths[0].clone(),
    }))
}

/// Resolve hooks for a specific event by merging configs from nearest to root
///
/// This function:
/// 1. Collects all config files from nearest to root
/// 2. Merges them (groups extend, hooks deduplicate, execution is most conservative)
/// 3. Returns resolved hooks ready for execution
///
/// # Arguments
///
/// * `nearest_config_path` - Path to the nearest hooks.toml file
/// * `event` - The git hook event (e.g., "pre-commit")
/// * `repo_root` - The repository root
/// * `changed_files` - Optional list of changed files for filtering
/// * `worktree_context` - Worktree context information
///
/// # Returns
///
/// Resolved hooks if the event is defined, None otherwise
///
/// # Errors
///
/// Returns an error if config file parsing fails or hook resolution fails
fn resolve_event_for_config(
    nearest_config_path: &Path,
    event: &str,
    repo_root: &Path,
    changed_files: Option<&[PathBuf]>,
    worktree_context: &WorktreeContext,
) -> Result<Option<ResolvedHooks>> {
    // Find all config files from nearest to root
    let config_dir = nearest_config_path
        .parent()
        .context("Config file has no parent directory")?;
    let config_paths = find_all_configs_for_file(config_dir, repo_root);

    if config_paths.is_empty() {
        return Ok(None);
    }

    // Merge all configs for this event
    let Some(merged) = merge_configs_for_event(&config_paths, event)? else {
        return Ok(None);
    };

    // Build ResolvedHooks from merged config
    let mut resolved_hooks_map = HashMap::new();

    for (hook_name, hook_def) in merged.hooks {
        use crate::hooks::ResolvedHook;

        let working_directory = if hook_def.run_at_root {
            repo_root.to_path_buf()
        } else {
            hook_def.workdir.as_ref().map_or_else(
                || {
                    merged
                        .nearest_config_path
                        .parent()
                        .unwrap_or(repo_root)
                        .to_path_buf()
                },
                |workdir| {
                    let path = Path::new(workdir);
                    if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        merged
                            .nearest_config_path
                            .parent()
                            .unwrap_or(repo_root)
                            .join(path)
                    }
                },
            )
        };

        resolved_hooks_map.insert(
            hook_name,
            ResolvedHook {
                definition: hook_def,
                working_directory,
                source_file: merged.nearest_config_path.clone(),
            },
        );
    }

    Ok(Some(ResolvedHooks {
        config_path: merged.nearest_config_path,
        hooks: resolved_hooks_map,
        execution_strategy: merged.execution_strategy,
        changed_files: changed_files.map(<[PathBuf]>::to_vec),
        worktree_context: worktree_context.clone(),
    }))
}

/// Group changed files by their nearest hooks.toml configuration
///
/// This is the main entry point for hierarchical resolution. For each changed
/// file, it finds the nearest hooks.toml that defines the requested event, then
/// groups files that share the same configuration.
///
/// # Arguments
///
/// * `changed_files` - List of files that have changed
/// * `repo_root` - The repository root directory
/// * `event` - The git hook event to resolve
/// * `worktree_context` - Worktree context information
///
/// # Returns
///
/// A vector of `ConfigGroup`, each containing a config and its associated files
///
/// # Errors
///
/// Returns an error if config file parsing fails or hook resolution fails
pub fn group_files_by_config(
    changed_files: &[PathBuf],
    repo_root: &Path,
    event: &str,
    worktree_context: &WorktreeContext,
) -> Result<Vec<ConfigGroup>> {
    // Map from config path to list of files
    let mut config_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    // For each file, find its nearest config (for grouping)
    for file in changed_files {
        let absolute_file = if file.is_absolute() {
            file.clone()
        } else {
            repo_root.join(file)
        };

        // Find all configs and use the nearest one for grouping
        let configs = find_all_configs_for_file(&absolute_file, repo_root);
        if let Some(nearest_config) = configs.first() {
            config_map
                .entry(nearest_config.clone())
                .or_default()
                .push(file.clone());
        } else {
            // No config found for this file - it will be skipped
            // This is expected behavior for files without hook configuration
        }
    }

    // Now resolve hooks for each config (merging with parent configs)
    let mut groups = Vec::new();
    for (config_path, files) in config_map {
        // Resolve hooks for this config and event (will merge with parents)
        if let Some(resolved_hooks) = resolve_event_for_config(
            &config_path,
            event,
            repo_root,
            Some(&files),
            worktree_context,
        )? {
            groups.push(ConfigGroup {
                config_path,
                files,
                resolved_hooks,
            });
        }
    }

    Ok(groups)
}

/// Resolve hooks hierarchically for all changed files
///
/// This is the main public API for hierarchical resolution. It:
/// 1. Gets the list of changed files based on detection mode
/// 2. Groups files by their nearest config
/// 3. Resolves hooks for each group
///
/// # Arguments
///
/// * `event` - The git hook event (e.g., "pre-commit")
/// * `change_mode` - How to detect changed files
/// * `repo_root` - The repository root
/// * `current_dir` - The current working directory where command was run
/// * `worktree_context` - Worktree context information
///
/// # Returns
///
/// A vector of `ConfigGroup` with resolved hooks for each config
///
/// # Errors
///
/// Returns an error if git operations fail or hook resolution fails
pub fn resolve_hooks_hierarchically(
    event: &str,
    change_mode: Option<ChangeDetectionMode>,
    repo_root: &Path,
    current_dir: &Path,
    worktree_context: &WorktreeContext,
) -> Result<Vec<ConfigGroup>> {
    // Get changed files if we have a detection mode
    let changed_files = if let Some(mode) = change_mode {
        let detector = crate::git::GitChangeDetector::new(repo_root)
            .context("Failed to create git change detector")?;
        detector
            .get_changed_files(&mode)
            .context("Failed to detect changed files")?
    } else {
        // If no change mode (--all-files), use current directory to find config
        // and return empty files list to trigger run_always hooks
        Vec::new()
    };

    if changed_files.is_empty() {
        // No files changed - check if there's a config from current directory
        // This allows --dry-run and --all-files to work from subdirectories
        let current_resolver = HookResolver::new(current_dir);
        if let Some(resolved) = current_resolver.resolve_hooks(event)? {
            return Ok(vec![ConfigGroup {
                config_path: resolved.config_path.clone(),
                files: Vec::new(),
                resolved_hooks: resolved,
            }]);
        }
        return Ok(Vec::new());
    }

    group_files_by_config(&changed_files, repo_root, event, worktree_context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_dir)
            .output()
            .unwrap();

        // Configure git
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_dir)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_dir)
            .output()
            .unwrap();

        temp_dir
    }

    #[test]
    fn test_find_all_configs_for_file() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        // Create nested directory structure
        fs::create_dir_all(repo_root.join("src/subdir")).unwrap();

        // Create config at root
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.test]
command = "echo root"
"#,
        )
        .unwrap();

        // Create config in src
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.test]
command = "echo src"
"#,
        )
        .unwrap();

        // File in subdir should find both configs (nearest first)
        let file = repo_root.join("src/subdir/file.rs");
        let configs = find_all_configs_for_file(&file, repo_root);
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0], repo_root.join("src/hooks.toml"));
        assert_eq!(configs[1], repo_root.join("hooks.toml"));

        // File at root should find only root hooks.toml
        let file = repo_root.join("root.rs");
        let configs = find_all_configs_for_file(&file, repo_root);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0], repo_root.join("hooks.toml"));
    }

    #[test]
    fn test_merge_groups_extends_includes() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root defines pre-commit with format and lint
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.format]
command = "cargo fmt"
modifies_repository = false

[hooks.lint]
command = "cargo clippy"
modifies_repository = false

[groups.pre-commit]
includes = ["format", "lint"]
execution = "parallel"
"#,
        )
        .unwrap();

        // Child adds test to pre-commit
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.test]
command = "cargo test"
modifies_repository = false

[groups.pre-commit]
includes = ["test"]
execution = "parallel"
"#,
        )
        .unwrap();

        // Merge should include format, lint, and test
        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        assert_eq!(merged.hooks.len(), 3);
        assert!(merged.hooks.contains_key("format"));
        assert!(merged.hooks.contains_key("lint"));
        assert!(merged.hooks.contains_key("test"));
    }

    #[test]
    fn test_merge_execution_strategy_most_conservative() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root uses parallel
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.format]
command = "cargo fmt"
modifies_repository = false

[groups.pre-commit]
includes = ["format"]
execution = "parallel"
"#,
        )
        .unwrap();

        // Child uses sequential
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.test]
command = "cargo test"
modifies_repository = false

[groups.pre-commit]
includes = ["test"]
execution = "sequential"
"#,
        )
        .unwrap();

        // Merged should be sequential (most conservative)
        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        assert!(matches!(
            merged.execution_strategy,
            ExecutionStrategy::Sequential
        ));
    }

    #[test]
    fn test_merge_hook_deduplication_nearest_wins() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root defines lint with one command
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.lint]
command = "cargo clippy"
modifies_repository = false

[groups.pre-commit]
includes = ["lint"]
"#,
        )
        .unwrap();

        // Child redefines lint with different command
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.lint]
command = "cargo clippy --all-targets"
modifies_repository = false

[groups.pre-commit]
includes = ["lint"]
"#,
        )
        .unwrap();

        // Merged should use child's lint definition (nearest wins)
        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        assert_eq!(merged.hooks.len(), 1);
        let lint_hook = merged.hooks.get("lint").unwrap();
        match &lint_hook.command {
            crate::config::HookCommand::Shell(cmd) => {
                assert_eq!(cmd, "cargo clippy --all-targets");
            }
            crate::config::HookCommand::Args(_) => panic!("Expected shell command, got args"),
        }
    }

    #[test]
    fn test_merge_with_no_overlap() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root defines format
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.format]
command = "cargo fmt"
modifies_repository = false

[groups.pre-commit]
includes = ["format"]
"#,
        )
        .unwrap();

        // Child defines completely different hooks
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.test]
command = "cargo test"
modifies_repository = false

[groups.pre-commit]
includes = ["test"]
"#,
        )
        .unwrap();

        // Merged should include both hooks
        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        assert_eq!(merged.hooks.len(), 2);
        assert!(merged.hooks.contains_key("format"));
        assert!(merged.hooks.contains_key("test"));
    }

    #[test]
    fn test_merge_child_includes_parent_hook_without_redefining() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root defines lint hook with file patterns
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.lint]
command = "cargo clippy"
modifies_repository = false
files = ["**/*.rs"]

[groups.pre-commit]
includes = ["lint"]
"#,
        )
        .unwrap();

        // Child includes lint but doesn't redefine it
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.test]
command = "cargo test"
modifies_repository = false

[groups.pre-commit]
includes = ["lint", "test"]
"#,
        )
        .unwrap();

        // Merged should use parent's lint definition
        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        assert_eq!(merged.hooks.len(), 2);
        assert!(merged.hooks.contains_key("lint"));
        assert!(merged.hooks.contains_key("test"));

        // Should use parent's lint command
        let lint_hook = merged.hooks.get("lint").unwrap();
        match &lint_hook.command {
            crate::config::HookCommand::Shell(cmd) => {
                assert_eq!(cmd, "cargo clippy");
            }
            crate::config::HookCommand::Args(_) => panic!("Expected shell command"),
        }

        // Should preserve parent's files pattern
        assert!(lint_hook.files.is_some());
        assert_eq!(lint_hook.files.as_ref().unwrap(), &vec!["**/*.rs"]);
    }

    #[test]
    fn test_merge_child_override_loses_parent_properties() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root defines lint with many properties
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.lint]
command = "cargo clippy"
modifies_repository = false
files = ["**/*.rs", "**/*.toml"]

[groups.pre-commit]
includes = ["lint"]
"#,
        )
        .unwrap();

        // Child redefines with minimal config
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.lint]
command = "cargo clippy --all-targets"
modifies_repository = false

[groups.pre-commit]
includes = ["lint"]
"#,
        )
        .unwrap();

        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        let lint_hook = merged.hooks.get("lint").unwrap();

        // Child's command wins
        match &lint_hook.command {
            crate::config::HookCommand::Shell(cmd) => {
                assert_eq!(cmd, "cargo clippy --all-targets");
            }
            crate::config::HookCommand::Args(_) => panic!("Expected shell command"),
        }

        // Parent's files pattern is LOST (complete replacement)
        assert!(lint_hook.files.is_none());
    }

    #[test]
    fn test_merge_three_level_hierarchy() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src/backend")).unwrap();

        // Root level
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.format]
command = "cargo fmt"
modifies_repository = false

[groups.pre-commit]
includes = ["format"]
"#,
        )
        .unwrap();

        // Middle level
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.lint]
command = "cargo clippy"
modifies_repository = false

[groups.pre-commit]
includes = ["lint"]
"#,
        )
        .unwrap();

        // Deepest level
        fs::write(
            repo_root.join("src/backend/hooks.toml"),
            r#"
[hooks.test]
command = "cargo test"
modifies_repository = false

[groups.pre-commit]
includes = ["test"]
"#,
        )
        .unwrap();

        // Should merge all three levels
        let configs = vec![
            repo_root.join("src/backend/hooks.toml"),
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        // All three hooks should be present
        assert_eq!(merged.hooks.len(), 3);
        assert!(merged.hooks.contains_key("format"));
        assert!(merged.hooks.contains_key("lint"));
        assert!(merged.hooks.contains_key("test"));
    }

    #[test]
    fn test_merge_child_includes_hook_parent_didnt_include() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root defines lint but doesn't include it in group
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.lint]
command = "cargo clippy"
modifies_repository = false

[hooks.format]
command = "cargo fmt"
modifies_repository = false

[groups.pre-commit]
includes = ["format"]
"#,
        )
        .unwrap();

        // Child includes lint (which parent didn't include)
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[groups.pre-commit]
includes = ["lint"]
"#,
        )
        .unwrap();

        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        // Should have both format (from parent's group) and lint (from child's group)
        assert_eq!(merged.hooks.len(), 2);
        assert!(merged.hooks.contains_key("format"));
        assert!(merged.hooks.contains_key("lint"));

        // Should use parent's lint definition (since child didn't redefine)
        let lint_hook = merged.hooks.get("lint").unwrap();
        match &lint_hook.command {
            crate::config::HookCommand::Shell(cmd) => {
                assert_eq!(cmd, "cargo clippy");
            }
            crate::config::HookCommand::Args(_) => panic!("Expected shell command"),
        }
    }

    #[test]
    fn test_merge_empty_child_group_still_gets_parent_hooks() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        fs::create_dir_all(repo_root.join("src")).unwrap();

        // Root has hooks
        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[hooks.format]
command = "cargo fmt"
modifies_repository = false

[groups.pre-commit]
includes = ["format"]
"#,
        )
        .unwrap();

        // Child has empty includes but still defines the group
        fs::write(
            repo_root.join("src/hooks.toml"),
            r"
[groups.pre-commit]
includes = []
",
        )
        .unwrap();

        let configs = vec![
            repo_root.join("src/hooks.toml"),
            repo_root.join("hooks.toml"),
        ];
        let merged = merge_configs_for_event(&configs, "pre-commit")
            .unwrap()
            .unwrap();

        // Should still have parent's format hook
        assert_eq!(merged.hooks.len(), 1);
        assert!(merged.hooks.contains_key("format"));
    }
}
