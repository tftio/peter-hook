//! Hierarchical hook resolution for monorepos
//!
//! This module implements per-file hook resolution where each changed file
//! finds its nearest hooks.toml and uses that configuration. This enables
//! monorepo-style setups where different subdirectories have different quality
//! gates.

use crate::{
    config::HookConfig,
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

/// Find the nearest hooks.toml file for a given file path
///
/// Walks up from the file's directory to find the first hooks.toml.
/// Stops at the repository root.
///
/// # Arguments
///
/// * `file_path` - The file to find a config for
/// * `repo_root` - The repository root (don't search above this)
///
/// # Returns
///
/// The path to the nearest hooks.toml, or None if not found
fn find_config_for_file(file_path: &Path, repo_root: &Path) -> Option<PathBuf> {
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

/// Resolve hooks for a specific event from a configuration file
///
/// This function loads the config and checks if it defines the requested event.
/// If not found and `fallback_search` is true, it searches parent directories.
///
/// # Arguments
///
/// * `config_path` - Path to the hooks.toml file
/// * `event` - The git hook event (e.g., "pre-commit")
/// * `repo_root` - The repository root
/// * `fallback_search` - Whether to search parent configs if event not found
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
    config_path: &Path,
    event: &str,
    repo_root: &Path,
    fallback_search: bool,
    changed_files: Option<&[PathBuf]>,
    worktree_context: &WorktreeContext,
) -> Result<Option<ResolvedHooks>> {
    let config = HookConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config: {}", config_path.display()))?;

    // Check if this config defines the event
    let has_event = config.hooks.as_ref().is_some_and(|h| h.contains_key(event))
        || config
            .groups
            .as_ref()
            .is_some_and(|g| g.contains_key(event));

    if has_event {
        // Resolve hooks from this config using the existing resolver
        let config_dir = config_path
            .parent()
            .context("Config file has no parent directory")?;
        let resolver = HookResolver::new(config_dir);

        // Use the existing resolution logic but with our config context
        return resolver.resolve_hooks_with_files(event, None).map(|opt| {
            opt.map(|mut resolved| {
                // Override the changed files with our filtered list
                resolved.changed_files = changed_files.map(<[PathBuf]>::to_vec);
                // Update the worktree context
                resolved.worktree_context = worktree_context.clone();
                resolved
            })
        });
    }

    // Event not found in this config
    if fallback_search {
        // Search parent directories for a config that defines this event
        if let Some(parent_dir) = config_path.parent() {
            if let Some(grandparent) = parent_dir.parent() {
                if let Some(parent_config) = find_config_for_file(grandparent, repo_root) {
                    if parent_config != config_path {
                        // Recursively check parent config
                        return resolve_event_for_config(
                            &parent_config,
                            event,
                            repo_root,
                            true,
                            changed_files,
                            worktree_context,
                        );
                    }
                }
            }
        }
    }

    Ok(None)
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

    // For each file, find its config
    for file in changed_files {
        let absolute_file = if file.is_absolute() {
            file.clone()
        } else {
            repo_root.join(file)
        };

        if let Some(config_path) = find_config_for_file(&absolute_file, repo_root) {
            config_map
                .entry(config_path)
                .or_default()
                .push(file.clone());
        } else {
            // No config found for this file - it will be skipped
            // This is expected behavior for files without hook configuration
        }
    }

    // Now resolve hooks for each config
    let mut groups = Vec::new();
    for (config_path, files) in config_map {
        // Resolve hooks for this config and event
        if let Some(resolved_hooks) = resolve_event_for_config(
            &config_path,
            event,
            repo_root,
            true, // Enable fallback search to parent configs
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
    fn test_find_config_for_file() {
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

        // Create config in subdir
        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[hooks.test]
command = "echo src"
"#,
        )
        .unwrap();

        // File in subdir should find src/hooks.toml
        let file = repo_root.join("src/subdir/file.rs");
        let config = find_config_for_file(&file, repo_root).unwrap();
        assert_eq!(config, repo_root.join("src/hooks.toml"));

        // File at root should find root hooks.toml
        let file = repo_root.join("root.rs");
        let config = find_config_for_file(&file, repo_root).unwrap();
        assert_eq!(config, repo_root.join("hooks.toml"));
    }

    #[test]
    fn test_hierarchical_config_selection() {
        let temp_dir = create_test_repo();
        let repo_root = temp_dir.path();

        // Create structure:
        // /hooks.toml (defines pre-push)
        // /src/hooks.toml (defines pre-commit)
        // /src/deep/hooks.toml (defines pre-push)

        fs::create_dir_all(repo_root.join("src/deep")).unwrap();

        fs::write(
            repo_root.join("hooks.toml"),
            r#"
[groups.pre-push]
includes = []
description = "root pre-push"
"#,
        )
        .unwrap();

        fs::write(
            repo_root.join("src/hooks.toml"),
            r#"
[groups.pre-commit]
includes = []
description = "src pre-commit"
"#,
        )
        .unwrap();

        fs::write(
            repo_root.join("src/deep/hooks.toml"),
            r#"
[groups.pre-push]
includes = []
description = "deep pre-push"
"#,
        )
        .unwrap();

        // File in src/ should use src/hooks.toml for pre-commit
        let file = repo_root.join("src/file.rs");
        let config = find_config_for_file(&file, repo_root).unwrap();
        assert_eq!(config, repo_root.join("src/hooks.toml"));

        // File in src/deep/ should use src/deep/hooks.toml for pre-push
        let file = repo_root.join("src/deep/file.rs");
        let config = find_config_for_file(&file, repo_root).unwrap();
        assert_eq!(config, repo_root.join("src/deep/hooks.toml"));
    }
}
