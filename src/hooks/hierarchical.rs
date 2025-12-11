//! Hierarchical hook resolution for monorepos
//!
//! This module implements per-file hook resolution where each changed file
//! finds its nearest hooks.toml and uses that configuration. This enables
//! monorepo-style setups where different subdirectories have different quality
//! gates.

use crate::{
    config::{ExecutionStrategy, HookConfig, HookDefinition},
    git::ChangeDetectionMode,
    hooks::{ResolvedHooks, WorktreeContext},
    trace,
};
use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
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


/// Find the nearest hooks.toml file for a given file path
///
/// Walks up from the file's directory to find the nearest hooks.toml file.
/// Stops at the repository root.
///
/// # Arguments
///
/// * `file_path` - The file to find config for
/// * `repo_root` - The repository root (don't search above this)
///
/// # Returns
///
/// Path to nearest config file, or None if not found
fn find_nearest_config_for_file(file_path: &Path, repo_root: &Path) -> Option<PathBuf> {
    // Start from the file's directory
    let mut current = if file_path.is_file() {
        file_path.parent()?
    } else {
        file_path
    };

    // Canonicalize paths for comparison
    let repo_root_canonical = repo_root.canonicalize().ok()?;

    loop {
        let config_path = current.join("hooks.toml");
        if config_path.exists() {
            return Some(config_path);
        }

        // Check if we've reached the repo root
        if let Ok(current_canonical) = current.canonicalize() {
            if current_canonical == repo_root_canonical {
                break;
            }
        }

        // Move up one directory
        current = current.parent()?;
    }

    None
}

/// Check if a hook should run based on file patterns and changed files
///
/// # Errors
///
/// Returns an error if glob patterns are invalid
fn should_run_hook(
    hook_def: &HookDefinition,
    changed_files: Option<&[PathBuf]>,
) -> Result<bool> {
    use crate::git::FilePatternMatcher;

    // If run_always is true, always run
    if hook_def.run_always {
        return Ok(true);
    }

    // If no file patterns specified, always run
    let Some(patterns) = &hook_def.files else {
        return Ok(true);
    };

    // If no changed files provided, always run (file filtering disabled)
    let Some(files) = changed_files else {
        return Ok(true);
    };

    // Check if any changed files match the patterns
    let matcher = FilePatternMatcher::new(patterns).context("Failed to compile file patterns")?;

    Ok(matcher.matches_any(files))
}

/// Resolve the working directory for a hook
fn resolve_working_directory(
    hook_def: &HookDefinition,
    config_dir: &Path,
    repo_root: &Path,
) -> PathBuf {
    if hook_def.run_at_root {
        return repo_root.to_path_buf();
    }

    hook_def.workdir.as_ref().map_or_else(
        || config_dir.to_path_buf(),
        |workdir| {
            let path = Path::new(workdir);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                config_dir.join(path)
            }
        },
    )
}

/// Resolve all hooks in a group recursively
///
/// # Errors
///
/// Returns an error if hook resolution fails
fn resolve_group_hooks(
    group: &crate::config::HookGroup,
    config: &HookConfig,
    config_dir: &Path,
    config_path: &Path,
    repo_root: &Path,
    resolved_hooks: &mut HashMap<String, crate::hooks::ResolvedHook>,
    changed_files: Option<&[PathBuf]>,
) -> Result<()> {
    let mut visited = HashSet::new();
    resolve_group_hooks_recursive(
        group,
        config,
        config_dir,
        config_path,
        repo_root,
        resolved_hooks,
        &mut visited,
        changed_files,
    )
}

/// Internal recursive group resolution
///
/// # Errors
///
/// Returns an error if hook resolution fails
#[allow(clippy::too_many_arguments)]
fn resolve_group_hooks_recursive(
    group: &crate::config::HookGroup,
    config: &HookConfig,
    config_dir: &Path,
    config_path: &Path,
    repo_root: &Path,
    resolved_hooks: &mut HashMap<String, crate::hooks::ResolvedHook>,
    visited: &mut HashSet<String>,
    changed_files: Option<&[PathBuf]>,
) -> Result<()> {
    for include in &group.includes {
        if visited.contains(include) {
            continue; // Avoid infinite loops
        }
        visited.insert(include.clone());

        // Try to resolve as individual hook first
        if let Some(hooks) = &config.hooks {
            if let Some(hook_def) = hooks.get(include) {
                // Skip hooks that require files when no files are available
                if hook_def.requires_files && changed_files.is_none() {
                    trace!(
                        "Skipping hook '{}' because it requires files but none are available",
                        include
                    );
                    continue;
                }

                // Apply file filtering
                if should_run_hook(hook_def, changed_files)? {
                    let working_directory =
                        resolve_working_directory(hook_def, config_dir, repo_root);

                    let resolved = crate::hooks::ResolvedHook {
                        definition: hook_def.clone(),
                        working_directory,
                        source_file: config_path.to_path_buf(),
                    };
                    resolved_hooks.insert(include.clone(), resolved);
                }
                continue;
            }
        }

        // Try to resolve as nested group
        if let Some(groups) = &config.groups {
            if let Some(nested_group) = groups.get(include) {
                resolve_group_hooks_recursive(
                    nested_group,
                    config,
                    config_dir,
                    config_path,
                    repo_root,
                    resolved_hooks,
                    visited,
                    changed_files,
                )?;
            }
        }
    }

    Ok(())
}

/// Resolve hooks for a specific event from a single config file (no merging)
///
/// This function resolves hooks directly from the nearest config file without
/// walking up the directory tree or merging with parent configs.
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
    // Load ONLY the nearest config (no parent walking or merging)
    let config = HookConfig::from_file(nearest_config_path)?;
    let config_dir = nearest_config_path
        .parent()
        .context("Config file has no parent directory")?;

    // Look for hooks that match the event name
    let mut resolved_hooks_map = HashMap::new();
    let mut execution_strategy = ExecutionStrategy::Sequential;

    // First, try to find a direct hook with the exact event name
    if let Some(hooks) = &config.hooks {
        if let Some(hook_def) = hooks.get(event) {
            // Apply file filtering
            if should_run_hook(hook_def, changed_files)? {
                let working_directory = resolve_working_directory(hook_def, config_dir, repo_root);

                let resolved = crate::hooks::ResolvedHook {
                    definition: hook_def.clone(),
                    working_directory,
                    source_file: nearest_config_path.to_path_buf(),
                };
                resolved_hooks_map.insert(event.to_string(), resolved);
            }
        }
    }

    // Check if it's a group
    if let Some(groups) = &config.groups {
        if let Some(group) = groups.get(event) {
            // Check if this is a placeholder group
            if group.placeholder == Some(true) {
                // Placeholder groups don't run any hooks
                return Ok(None);
            }

            execution_strategy = group.get_execution_strategy();
            resolve_group_hooks(
                group,
                &config,
                config_dir,
                nearest_config_path,
                repo_root,
                &mut resolved_hooks_map,
                changed_files,
            )?;
        }
    }

    if resolved_hooks_map.is_empty() {
        return Ok(None);
    }

    Ok(Some(ResolvedHooks {
        config_path: nearest_config_path.to_path_buf(),
        hooks: resolved_hooks_map,
        execution_strategy,
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
    trace!("--- Grouping Files by Config ---");
    trace!(
        "Grouping {} files by their nearest config",
        changed_files.len()
    );

    // Map from config path to list of files
    let mut config_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    // For each file, find its nearest config (for grouping)
    for file in changed_files {
        let absolute_file = if file.is_absolute() {
            file.clone()
        } else {
            repo_root.join(file)
        };

        // Find the nearest config for grouping
        if let Some(nearest_config) = find_nearest_config_for_file(&absolute_file, repo_root) {
            trace!("  {} -> {}", file.display(), nearest_config.display());
            config_map
                .entry(nearest_config)
                .or_default()
                .push(file.clone());
        } else {
            trace!("  {} -> NO CONFIG (will be skipped)", file.display());
            // No config found for this file - it will be skipped
            // This is expected behavior for files without hook configuration
        }
    }

    trace!("Found {} unique config locations", config_map.len());

    // Now resolve hooks for each config (standalone, no merging)
    let mut groups = Vec::new();
    for (config_path, files) in config_map {
        trace!(
            "Resolving hooks for config: {} ({} files)",
            config_path.display(),
            files.len()
        );
        // Resolve hooks directly from this config (no parent merging)
        if let Some(resolved_hooks) = resolve_event_for_config(
            &config_path,
            event,
            repo_root,
            Some(&files),
            worktree_context,
        )? {
            trace!(
                "  ✓ Resolved {} hooks for this group",
                resolved_hooks.hooks.len()
            );
            groups.push(ConfigGroup {
                config_path,
                files,
                resolved_hooks,
            });
        } else {
            trace!("  ✗ Event '{}' not defined for this config", event);
        }
    }

    trace!("--- End File Grouping ---");
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
    trace!("=== Hierarchical Resolution Started ===");
    trace!("Event: {}", event);
    trace!("Repo root: {}", repo_root.display());
    trace!("Current dir: {}", current_dir.display());
    trace!("Change mode: {:?}", change_mode);

    // Get changed files if we have a detection mode
    let changed_files = if let Some(mode) = change_mode {
        trace!("Detecting changed files with mode: {:?}", mode);
        let detector = crate::git::GitChangeDetector::new(repo_root)
            .context("Failed to create git change detector")?;
        let files = detector
            .get_changed_files(&mode)
            .context("Failed to detect changed files")?;
        trace!("Detected {} changed files", files.len());
        for (i, file) in files.iter().enumerate().take(10) {
            trace!("  [{}] {}", i + 1, file.display());
        }
        if files.len() > 10 {
            trace!("  ... and {} more files", files.len() - 10);
        }
        files
    } else {
        trace!("No change detection mode - using --all-files or dry-run");
        // If no change mode (--all-files), use current directory to find config
        // and return empty files list to trigger run_always hooks
        Vec::new()
    };

    if changed_files.is_empty() {
        trace!("No changed files - resolving from current directory");
        // No files changed - find nearest config from current directory
        let Some(nearest_config) = find_nearest_config_for_file(current_dir, repo_root) else {
            trace!("No config file found - returning empty result");
            return Ok(Vec::new());
        };

        trace!(
            "Resolving event '{}' from nearest config: {}",
            event,
            nearest_config.display()
        );
        if let Some(resolved) = resolve_event_for_config(
            &nearest_config,
            event,
            repo_root,
            None, // No files to filter
            worktree_context,
        )? {
            trace!(
                "✓ Event resolved successfully with {} hooks",
                resolved.hooks.len()
            );
            return Ok(vec![ConfigGroup {
                config_path: nearest_config,
                files: Vec::new(),
                resolved_hooks: resolved,
            }]);
        }
        trace!("✗ Event '{}' not defined in any config", event);
        return Ok(Vec::new());
    }

    trace!(
        "Grouping {} changed files by their nearest config",
        changed_files.len()
    );
    let groups = group_files_by_config(&changed_files, repo_root, event, worktree_context)?;
    trace!("Created {} config groups", groups.len());
    for (i, group) in groups.iter().enumerate() {
        trace!(
            "  Group[{}]: {} (with {} files, {} hooks)",
            i,
            group.config_path.display(),
            group.files.len(),
            group.resolved_hooks.hooks.len()
        );
    }
    trace!("=== Hierarchical Resolution Complete ===");
    Ok(groups)
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
    fn test_find_nearest_config_for_file() {
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

        // File in subdir should find nearest config (src/hooks.toml)
        let file = repo_root.join("src/subdir/file.rs");
        let config = find_nearest_config_for_file(&file, repo_root);
        assert_eq!(config, Some(repo_root.join("src/hooks.toml")));

        // File at root should find root hooks.toml
        let file = repo_root.join("root.rs");
        let config = find_nearest_config_for_file(&file, repo_root);
        assert_eq!(config, Some(repo_root.join("hooks.toml")));
    }

    #[test]
    fn test_no_config_merging_child_only_uses_own_hooks() {
        // Test that child configs DO NOT inherit from parent configs
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

        // Child defines ONLY test (should NOT inherit format/lint)
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

        // Create a worktree context
        let worktree_context = WorktreeContext {
            is_worktree: false,
            worktree_name: None,
            repo_root: repo_root.to_path_buf(),
            common_dir: repo_root.to_path_buf(),
            working_dir: repo_root.to_path_buf(),
        };

        // Resolve from child config - should only get test, NOT format/lint
        let resolved = resolve_event_for_config(
            &repo_root.join("src/hooks.toml"),
            "pre-commit",
            repo_root,
            None,
            &worktree_context,
        )
        .unwrap()
        .unwrap();

        // Should ONLY have test hook, no inheritance from parent
        assert_eq!(resolved.hooks.len(), 1);
        assert!(resolved.hooks.contains_key("test"));
        assert!(!resolved.hooks.contains_key("format"));
        assert!(!resolved.hooks.contains_key("lint"));
    }
}
