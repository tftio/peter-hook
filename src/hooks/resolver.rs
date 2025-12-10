//! Hierarchical hook resolution system

use crate::{
    config::{ExecutionStrategy, HookConfig, HookDefinition, HookGroup},
    git::{
        ChangeDetectionMode, FilePatternMatcher, GitChangeDetector, GitRepository,
        LintFileDiscovery,
    },
};
use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

/// Resolves hooks hierarchically from the filesystem
pub struct HookResolver {
    /// Current working directory where hook resolution starts
    current_dir: PathBuf,
}

/// Result of hook resolution containing all applicable hooks
#[derive(Debug, Clone)]
pub struct ResolvedHooks {
    /// The configuration file that was used
    pub config_path: PathBuf,
    /// Individual hooks to execute
    pub hooks: HashMap<String, ResolvedHook>,
    /// Execution strategy for this group of hooks
    pub execution_strategy: ExecutionStrategy,
    /// Changed files (if file filtering is enabled)
    pub changed_files: Option<Vec<PathBuf>>,
    /// Worktree context information
    pub worktree_context: WorktreeContext,
}

/// A resolved hook ready for execution
#[derive(Debug, Clone)]
pub struct ResolvedHook {
    /// Original hook definition
    pub definition: HookDefinition,
    /// Directory where this hook should run
    pub working_directory: PathBuf,
    /// Source configuration file
    pub source_file: PathBuf,
}

/// Worktree context information for template expansion and hook resolution
#[derive(Debug, Clone)]
pub struct WorktreeContext {
    /// Whether we are in a worktree
    pub is_worktree: bool,
    /// Name of the current worktree (None for main repository)
    pub worktree_name: Option<String>,
    /// Path to the repository root
    pub repo_root: PathBuf,
    /// Path to the common git directory (shared across worktrees)
    pub common_dir: PathBuf,
    /// Path to the working directory
    pub working_dir: PathBuf,
}

impl HookResolver {
    /// Create a new hook resolver for the current directory
    pub fn new<P: AsRef<Path>>(current_dir: P) -> Self {
        Self {
            current_dir: current_dir.as_ref().to_path_buf(),
        }
    }

    /// Find the nearest .peter-hook.toml file by walking up the directory tree
    ///
    /// # Errors
    ///
    /// Returns an error if there are filesystem access issues
    pub fn find_config_file(&self) -> Result<Option<PathBuf>> {
        let mut current = self.current_dir.as_path();

        loop {
            let config_path = current.join(".peter-hook.toml");
            if config_path.exists() {
                return Ok(Some(config_path));
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => return Ok(None),
            }
        }
    }

    /// Resolve hooks for a given git event (e.g., "pre-commit", "pre-push")
    ///
    /// # Errors
    ///
    /// Returns an error if config file parsing fails or filesystem access
    /// issues occur
    pub fn resolve_hooks(&self, event: &str) -> Result<Option<ResolvedHooks>> {
        self.resolve_hooks_with_files(event, None)
    }

    /// Resolve hooks for a given git event with file change detection
    ///
    /// # Errors
    ///
    /// Returns an error if config file parsing fails or git operations fail
    pub fn resolve_hooks_with_files(
        &self,
        event: &str,
        change_mode: Option<ChangeDetectionMode>,
    ) -> Result<Option<ResolvedHooks>> {
        let Some(config_path) = self.find_config_file()? else {
            return Ok(None);
        };

        let config = HookConfig::from_file(&config_path)?;
        let config_dir = config_path
            .parent()
            .context("Config file has no parent directory")?;

        // Get repository information for worktree context
        let repo = GitRepository::find_from_dir(&self.current_dir)
            .context("Failed to find git repository")?;

        // Create worktree context
        let worktree_context = WorktreeContext {
            is_worktree: repo.is_worktree,
            worktree_name: repo.get_worktree_name().map(ToString::to_string),
            repo_root: repo.root.clone(),
            common_dir: repo.common_dir,
            working_dir: self.current_dir.clone(),
        };

        // Get changed files if file filtering is requested
        let changed_files = if let Some(mode) = change_mode {
            let detector = GitChangeDetector::new(&self.current_dir)
                .context("Failed to create git change detector")?;
            Some(
                detector
                    .get_changed_files(&mode)
                    .context("Failed to detect changed files")?,
            )
        } else {
            None
        };

        // Look for hooks that match the event name
        let mut resolved_hooks = HashMap::new();
        let mut execution_strategy = ExecutionStrategy::Sequential;

        // First, try to find a hook or group with the exact event name
        if let Some(hooks) = &config.hooks {
            if let Some(hook_def) = hooks.get(event) {
                // Apply file filtering
                if Self::should_run_hook(hook_def, changed_files.as_ref())? {
                    let resolved = ResolvedHook {
                        definition: hook_def.clone(),
                        working_directory: Self::resolve_working_directory(hook_def, config_dir),
                        source_file: config_path.clone(),
                    };
                    resolved_hooks.insert(event.to_string(), resolved);
                }
            }
        }

        if let Some(groups) = &config.groups {
            if let Some(group) = groups.get(event) {
                // Check if this is a placeholder group
                if group.placeholder == Some(true) {
                    // Placeholder groups don't run any hooks at this level
                    // They only trigger installation for hierarchical resolution
                    return Ok(None);
                }

                execution_strategy = group.get_execution_strategy();
                self.resolve_group_with_files(
                    group,
                    &config,
                    config_dir,
                    &config_path,
                    &mut resolved_hooks,
                    changed_files.as_ref(),
                )?;
            }
        }

        if resolved_hooks.is_empty() {
            return Ok(None);
        }

        Ok(Some(ResolvedHooks {
            config_path,
            hooks: resolved_hooks,
            execution_strategy,
            changed_files,
            worktree_context,
        }))
    }

    /// List all hook and group names available in the nearest configuration.
    ///
    /// Placeholder groups are filtered out because they do not represent
    /// executable targets.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration parsing fails or the configuration
    /// directory cannot be determined.
    pub fn list_hook_names(&self) -> Result<Vec<String>> {
        let Some(config_path) = self.find_config_file()? else {
            return Ok(Vec::new());
        };

        let config = HookConfig::from_file(&config_path)?;
        let mut names = Vec::new();

        if let Some(hooks) = &config.hooks {
            names.extend(hooks.keys().cloned());
        }

        if let Some(groups) = &config.groups {
            names.extend(groups.iter().filter_map(|(name, group)| {
                if group.placeholder == Some(true) {
                    None
                } else {
                    Some(name.clone())
                }
            }));
        }

        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Resolve hooks in lint mode (current directory as root, all matching
    /// non-ignored files)
    ///
    /// # Errors
    ///
    /// Returns an error if config file parsing fails or file discovery fails
    pub fn resolve_hooks_for_lint(&self, hook_name: &str) -> Result<Option<ResolvedHooks>> {
        let Some(config_path) = self.find_config_file()? else {
            return Ok(None);
        };

        let config = HookConfig::from_file(&config_path)?;

        // Discover all non-ignored files in current directory
        let discovery = LintFileDiscovery::new(&self.current_dir);
        let all_files = discovery
            .discover_files()
            .context("Failed to discover files for lint mode")?;

        // In lint mode, the current directory acts as the "repo root"
        let lint_repo_root = discovery.repo_root().as_ref().map_or_else(
            || self.current_dir.clone(),
            |repo_root| repo_root.to_path_buf(),
        );

        // Create worktree context for lint mode
        // In lint mode, we treat current directory as the repository root
        let worktree_context = WorktreeContext {
            is_worktree: false,
            worktree_name: None,
            repo_root: lint_repo_root,
            common_dir: self.current_dir.clone(), // No separate common dir in lint mode
            working_dir: self.current_dir.clone(),
        };

        // Look for the specific hook by name
        let mut resolved_hooks = HashMap::new();
        let mut execution_strategy = ExecutionStrategy::Sequential;

        // Check if it's a direct hook
        if let Some(hooks) = &config.hooks {
            if let Some(hook_def) = hooks.get(hook_name) {
                // In lint mode, always run the hook (file filtering happens during execution)
                // We provide all_files so the executor can filter based on patterns
                let resolved = ResolvedHook {
                    definition: hook_def.clone(),
                    // In lint mode, run in current directory
                    working_directory: self.current_dir.clone(),
                    source_file: config_path.clone(),
                };
                resolved_hooks.insert(hook_name.to_string(), resolved);
            }
        }

        // Check if it's a group
        if let Some(groups) = &config.groups {
            if let Some(group) = groups.get(hook_name) {
                execution_strategy = group.get_execution_strategy();
                // In lint mode, we pass Some(&all_files) to enable file filtering
                self.resolve_group_for_lint(group, &config, &config_path, &mut resolved_hooks)?;
            }
        }

        if resolved_hooks.is_empty() {
            return Ok(None);
        }

        Ok(Some(ResolvedHooks {
            config_path,
            hooks: resolved_hooks,
            execution_strategy,
            changed_files: Some(all_files), /* In lint mode, "changed files" are all discovered
                                             * files */
            worktree_context,
        }))
    }

    /// Resolve a specific hook by name
    ///
    /// # Errors
    ///
    /// Returns an error if config file parsing fails or git operations fail
    pub fn resolve_hook_by_name(
        &self,
        hook_name: &str,
        change_mode: Option<ChangeDetectionMode>,
    ) -> Result<Option<ResolvedHooks>> {
        let Some(config_path) = self.find_config_file()? else {
            return Ok(None);
        };

        let config = HookConfig::from_file(&config_path)?;
        let config_dir = config_path
            .parent()
            .context("Config file has no parent directory")?;

        // Get repository information for worktree context
        let repo = GitRepository::find_from_dir(&self.current_dir)
            .context("Failed to find git repository")?;

        // Create worktree context
        let worktree_context = WorktreeContext {
            is_worktree: repo.is_worktree,
            worktree_name: repo.get_worktree_name().map(ToString::to_string),
            repo_root: repo.root.clone(),
            common_dir: repo.common_dir,
            working_dir: self.current_dir.clone(),
        };

        // Get changed files if change mode is specified
        let changed_files = if let Some(mode) = change_mode {
            let detector = GitChangeDetector::new(&self.current_dir)
                .context("Failed to create git change detector")?;
            Some(
                detector
                    .get_changed_files(&mode)
                    .context("Failed to detect changed files")?,
            )
        } else {
            None
        };

        // Look for the specific hook by name
        let mut resolved_hooks = HashMap::new();
        let mut execution_strategy = ExecutionStrategy::Sequential;

        // Check if it's a direct hook
        if let Some(hooks) = &config.hooks {
            if let Some(hook_def) = hooks.get(hook_name) {
                // Apply file filtering
                if Self::should_run_hook(hook_def, changed_files.as_ref())? {
                    let resolved = ResolvedHook {
                        definition: hook_def.clone(),
                        working_directory: Self::resolve_working_directory(hook_def, config_dir),
                        source_file: config_path.clone(),
                    };
                    resolved_hooks.insert(hook_name.to_string(), resolved);
                }
            }
        }

        // Check if it's a group
        if let Some(groups) = &config.groups {
            if let Some(group) = groups.get(hook_name) {
                // Check if this is a placeholder group
                if group.placeholder == Some(true) {
                    // Placeholder groups don't run any hooks
                    return Ok(None);
                }

                execution_strategy = group.get_execution_strategy();
                self.resolve_group_with_files(
                    group,
                    &config,
                    config_dir,
                    &config_path,
                    &mut resolved_hooks,
                    changed_files.as_ref(),
                )?;
            }
        }

        if resolved_hooks.is_empty() {
            return Ok(None);
        }

        Ok(Some(ResolvedHooks {
            config_path,
            hooks: resolved_hooks,
            execution_strategy,
            changed_files,
            worktree_context,
        }))
    }

    /// Resolve the working directory for a hook
    fn resolve_working_directory(hook_def: &HookDefinition, config_dir: &Path) -> PathBuf {
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

    /// Check if a hook should run based on file patterns and changed files
    ///
    /// # Errors
    ///
    /// Returns an error if glob patterns are invalid
    fn should_run_hook(
        hook_def: &HookDefinition,
        changed_files: Option<&Vec<PathBuf>>,
    ) -> Result<bool> {
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
        let matcher =
            FilePatternMatcher::new(patterns).context("Failed to compile file patterns")?;

        Ok(matcher.matches_any(files))
    }

    /// Resolve all hooks in a group for lint mode
    ///
    /// # Errors
    ///
    /// Returns an error if hook resolution fails
    fn resolve_group_for_lint(
        &self,
        group: &HookGroup,
        config: &HookConfig,
        config_path: &Path,
        resolved_hooks: &mut HashMap<String, ResolvedHook>,
    ) -> Result<()> {
        let mut visited = HashSet::new();
        self.resolve_group_recursive_for_lint(
            group,
            config,
            config_path,
            resolved_hooks,
            &mut visited,
        )
    }

    /// Internal recursive group resolution for lint mode
    ///
    /// # Errors
    ///
    /// Returns an error if hook resolution fails
    fn resolve_group_recursive_for_lint(
        &self,
        group: &HookGroup,
        config: &HookConfig,
        config_path: &Path,
        resolved_hooks: &mut HashMap<String, ResolvedHook>,
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        for include in &group.includes {
            if visited.contains(include) {
                continue; // Avoid infinite loops
            }
            visited.insert(include.clone());

            // Try to resolve as individual hook first
            if let Some(hooks) = &config.hooks {
                if let Some(hook_def) = hooks.get(include) {
                    // In lint mode, always include the hook (file filtering during execution)
                    let resolved = ResolvedHook {
                        definition: hook_def.clone(),
                        working_directory: self.current_dir.clone(), // Run in current directory
                        source_file: config_path.to_path_buf(),
                    };
                    resolved_hooks.insert(include.clone(), resolved);
                    continue;
                }
            }

            // Try to resolve as group
            if let Some(groups) = &config.groups {
                if let Some(nested_group) = groups.get(include) {
                    self.resolve_group_recursive_for_lint(
                        nested_group,
                        config,
                        config_path,
                        resolved_hooks,
                        visited,
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Resolve all hooks in a group recursively with file filtering
    ///
    /// # Errors
    ///
    /// Returns an error if hook resolution fails
    fn resolve_group_with_files(
        &self,
        group: &HookGroup,
        config: &HookConfig,
        config_dir: &Path,
        config_path: &Path,
        resolved_hooks: &mut HashMap<String, ResolvedHook>,
        changed_files: Option<&Vec<PathBuf>>,
    ) -> Result<()> {
        let mut visited = HashSet::new();
        self.resolve_group_recursive_with_files(
            group,
            config,
            config_dir,
            config_path,
            resolved_hooks,
            &mut visited,
            changed_files,
        )
    }

    /// Internal recursive group resolution with file filtering
    ///
    /// # Errors
    ///
    /// Returns an error if hook resolution fails
    #[allow(clippy::only_used_in_recursion)]
    #[allow(clippy::too_many_arguments)]
    fn resolve_group_recursive_with_files(
        &self,
        group: &HookGroup,
        config: &HookConfig,
        config_dir: &Path,
        config_path: &Path,
        resolved_hooks: &mut HashMap<String, ResolvedHook>,
        visited: &mut HashSet<String>,
        changed_files: Option<&Vec<PathBuf>>,
    ) -> Result<()> {
        for include in &group.includes {
            if visited.contains(include) {
                continue; // Avoid infinite loops
            }
            visited.insert(include.clone());

            // Try to resolve as individual hook first
            if let Some(hooks) = &config.hooks {
                if let Some(hook_def) = hooks.get(include) {
                    // Apply file filtering
                    if Self::should_run_hook(hook_def, changed_files)? {
                        let resolved = ResolvedHook {
                            definition: hook_def.clone(),
                            working_directory: Self::resolve_working_directory(
                                hook_def, config_dir,
                            ),
                            source_file: config_path.to_path_buf(),
                        };
                        resolved_hooks.insert(include.clone(), resolved);
                    }
                    continue;
                }
            }

            // Try to resolve as group
            if let Some(groups) = &config.groups {
                if let Some(nested_group) = groups.get(include) {
                    self.resolve_group_recursive_with_files(
                        nested_group,
                        config,
                        config_dir,
                        config_path,
                        resolved_hooks,
                        visited,
                        changed_files,
                    )?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HookCommand;
    use git2::Repository as Git2Repository;
    use tempfile::TempDir;

    fn create_test_config(dir: &Path, content: &str) -> PathBuf {
        let config_path = dir.join(".peter-hook.toml");
        std::fs::write(&config_path, content).unwrap();
        config_path
    }

    #[test]
    fn test_find_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create nested directory structure
        let nested = root.join("projects/frontend/src");
        std::fs::create_dir_all(&nested).unwrap();

        // Create config file in middle directory
        create_test_config(&root.join("projects"), "[hooks]");

        let resolver = HookResolver::new(&nested);
        let config_path = resolver.find_config_file().unwrap().unwrap();

        assert_eq!(config_path, root.join("projects/.peter-hook.toml"));
    }

    #[test]
    fn test_resolve_simple_hook() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        // Initialize a real git repository for resolution
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.pre-commit]
command = "echo 'running pre-commit'"
description = "Simple pre-commit hook"
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);
        let result = resolver.resolve_hooks("pre-commit").unwrap().unwrap();

        assert_eq!(result.hooks.len(), 1);
        assert!(result.hooks.contains_key("pre-commit"));

        let hook = &result.hooks["pre-commit"];
        assert_eq!(hook.working_directory, root);
        assert_eq!(
            hook.definition.command,
            HookCommand::Shell("echo 'running pre-commit'".to_string())
        );
    }

    #[test]
    fn test_resolve_hook_group() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        // Initialize a real git repository for resolution
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.lint]
command = "cargo clippy"

[hooks.test]
command = "cargo test"

[groups.pre-commit]
includes = ["lint", "test"]
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);
        let result = resolver.resolve_hooks("pre-commit").unwrap().unwrap();

        assert_eq!(result.hooks.len(), 2);
        assert!(result.hooks.contains_key("lint"));
        assert!(result.hooks.contains_key("test"));
    }

    #[test]
    fn test_no_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let resolver = HookResolver::new(root);
        let result = resolver.resolve_hooks("pre-commit").unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_hook_by_name_simple_hook() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        // Initialize a real git repository for resolution
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.lint]
command = "cargo clippy"
description = "Run linting"

[hooks.test]
command = "cargo test"
description = "Run tests"
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);
        let result = resolver
            .resolve_hook_by_name("lint", None)
            .unwrap()
            .unwrap();

        assert_eq!(result.hooks.len(), 1);
        assert!(result.hooks.contains_key("lint"));

        let hook = &result.hooks["lint"];
        assert_eq!(hook.working_directory, root);
        assert_eq!(
            hook.definition.command,
            HookCommand::Shell("cargo clippy".to_string())
        );
    }

    #[test]
    fn test_resolve_hook_by_name_group() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        // Initialize a real git repository for resolution
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.lint]
command = "cargo clippy"

[hooks.test]
command = "cargo test"

[hooks.format]
command = "cargo fmt"

[groups.quality]
includes = ["lint", "test", "format"]
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);
        let result = resolver
            .resolve_hook_by_name("quality", None)
            .unwrap()
            .unwrap();

        assert_eq!(result.hooks.len(), 3);
        assert!(result.hooks.contains_key("lint"));
        assert!(result.hooks.contains_key("test"));
        assert!(result.hooks.contains_key("format"));
    }

    #[test]
    fn test_resolve_hook_by_name_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        // Initialize a real git repository for resolution
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.lint]
command = "cargo clippy"
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);
        let result = resolver.resolve_hook_by_name("nonexistent", None).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_hook_by_name_with_file_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        // Initialize a real git repository for resolution
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.rust-only]
command = "cargo clippy"
files = ["**/*.rs"]

[hooks.all-files]
command = "echo 'all files'"

[hooks.always-run]
command = "echo 'always'"
run_always = true
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);

        // Test with file filtering enabled - should return changed_files
        let result = resolver
            .resolve_hook_by_name("always-run", Some(ChangeDetectionMode::WorkingDirectory))
            .unwrap()
            .unwrap();
        assert_eq!(result.hooks.len(), 1);
        assert!(result.changed_files.is_some());

        // Test with file filtering disabled - should not return changed_files
        let result = resolver
            .resolve_hook_by_name("always-run", None)
            .unwrap()
            .unwrap();
        assert_eq!(result.hooks.len(), 1);
        assert!(result.changed_files.is_none());

        // Test file-specific hook with no matching files - may return empty hooks
        let result = resolver
            .resolve_hook_by_name("rust-only", Some(ChangeDetectionMode::WorkingDirectory))
            .unwrap();
        if let Some(resolved) = result {
            assert!(resolved.changed_files.is_some());
            // Hook may or may not be included depending on file matches
        }
    }

    #[test]
    fn test_resolve_placeholder_group_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[groups.pre-commit]
includes = []
placeholder = true
description = "Placeholder for hierarchical resolution"
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);
        let result = resolver.resolve_hooks("pre-commit").unwrap();

        // Placeholder groups should return None (no hooks to run)
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_placeholder_vs_real_group() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.lint]
command = "echo lint"
modifies_repository = false

[groups.placeholder-group]
includes = []
placeholder = true

[groups.real-group]
includes = ["lint"]
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);

        // Placeholder group returns None
        let placeholder_result = resolver.resolve_hooks("placeholder-group").unwrap();
        assert!(placeholder_result.is_none());

        // Real group returns hooks
        let real_result = resolver.resolve_hooks("real-group").unwrap();
        assert!(real_result.is_some());
        assert_eq!(real_result.unwrap().hooks.len(), 1);
    }

    #[test]
    fn test_list_hook_names_excludes_placeholders() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let _ = Git2Repository::init(root).unwrap();

        let config_content = r#"
[hooks.lint]
command = "cargo clippy"

[hooks.test]
command = "cargo test"

[groups.all]
includes = ["lint", "test"]

[groups.placeholder]
placeholder = true
includes = []
"#;

        create_test_config(root, config_content);

        let resolver = HookResolver::new(root);
        let names = resolver.list_hook_names().unwrap();

        assert_eq!(names, vec!["all", "lint", "test"]);
        assert!(!names.contains(&"placeholder".to_string()));
    }
}
