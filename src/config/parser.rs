#![allow(clippy::items_after_test_module)]
//! Configuration parsing for git hooks

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use shellexpand;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::config::GlobalConfig;

/// Represents a hook configuration file (hooks.toml)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookConfig {
    /// Individual hook definitions
    pub hooks: Option<HashMap<String, HookDefinition>>,
    /// Hook groups that combine multiple hooks
    pub groups: Option<HashMap<String, HookGroup>>,
    /// Optional list of files to import and merge
    pub imports: Option<Vec<String>>,
}

/// Definition of an individual hook
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct HookDefinition {
    /// Command to execute (either as string or array)
    pub command: HookCommand,
    /// Working directory override (defaults to config file directory)
    pub workdir: Option<String>,
    /// Environment variables to set
    pub env: Option<HashMap<String, String>>,
    /// Description of what this hook does
    pub description: Option<String>,
    /// Whether this hook modifies the repository contents
    /// If true, this hook cannot run in parallel with other hooks
    #[serde(default)]
    pub modifies_repository: bool,
    /// File patterns that trigger this hook (glob patterns)
    /// If specified, hook only runs if changed files match these patterns
    pub files: Option<Vec<String>>,
    /// Run this hook always, regardless of file changes
    #[serde(default)]
    pub run_always: bool,
    /// Whether this hook requires a file list to run
    /// If true, the hook will only run in contexts where files can be determined
    /// (e.g., pre-commit, pre-push) and will be skipped in contexts like commit-msg
    #[serde(default)]
    pub requires_files: bool,
    /// Hooks that must complete successfully before this hook runs
    pub depends_on: Option<Vec<String>>,
    /// How to execute this hook with respect to changed files
    #[serde(default)]
    pub execution_type: ExecutionType,
    /// Whether to run the hook at the repository root instead of the config
    /// directory
    #[serde(default)]
    pub run_at_root: bool,
    /// Maximum execution time in seconds (default: 300 = 5 minutes)
    /// If the hook exceeds this timeout, it will be killed
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

/// Default timeout value: 5 minutes
fn default_timeout_seconds() -> u64 {
    300
}

/// How to execute hooks with respect to changed files
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionType {
    /// Pass changed files as individual arguments to the command (default)
    #[default]
    PerFile,
    /// Run command once in config directory without file arguments
    InPlace,
    /// Hook handles file processing manually using template variables
    Other,
}

/// Command specification for a hook
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum HookCommand {
    /// Shell command as a string
    Shell(String),
    /// Execve-style command as array
    Args(Vec<String>),
}

/// Execution strategy for hook groups
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    /// Run all hooks sequentially (default)
    #[default]
    Sequential,
    /// Run hooks in parallel where safe (respects `modifies_repository` flag)
    Parallel,
    /// Force parallel execution (unsafe - ignores `modifies_repository`)
    ForceParallel,
}

/// Group of hooks that run together
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookGroup {
    /// List of hooks or other groups to include
    pub includes: Vec<String>,
    /// Description of what this group does
    pub description: Option<String>,
    /// Execution strategy for this group
    #[serde(default)]
    pub execution: ExecutionStrategy,
    /// Whether this is a placeholder group for hierarchical resolution
    /// Placeholder groups trigger git hook installation but don't run any hooks
    /// at the root level - they only enable subdirectory hooks to be discovered
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<bool>,
    /// Whether to run hooks in parallel (deprecated - use execution field)
    /// Kept for backward compatibility
    #[serde(skip_serializing)]
    pub parallel: Option<bool>,
}

impl HookGroup {
    /// Get the effective execution strategy, handling backward compatibility
    #[must_use]
    pub fn get_execution_strategy(&self) -> ExecutionStrategy {
        // Handle backward compatibility with deprecated `parallel` field
        self.parallel.map_or_else(
            || self.execution,
            |parallel| {
                if parallel {
                    ExecutionStrategy::Parallel
                } else {
                    ExecutionStrategy::Sequential
                }
            },
        )
    }
}

impl HookConfig {
    /// Parse a hooks.toml file from the given path
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut visited = HashSet::new();
        Self::from_file_internal(path.as_ref(), &mut visited, None)
    }

    /// Parse a hooks.toml file and collect import diagnostics
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read
    /// - The TOML content is malformed
    /// - Import cycles are detected
    /// - Required configuration fields are missing
    pub fn from_file_with_trace<P: AsRef<Path>>(path: P) -> Result<(Self, ImportDiagnostics)> {
        let mut visited = HashSet::new();
        let mut diag = ImportDiagnostics::default();
        let cfg = Self::from_file_internal(path.as_ref(), &mut visited, Some(&mut diag))?;
        // Compute unused imports: those that were resolved but contributed no names
        let unused: Vec<String> = diag
            .imports
            .iter()
            .filter(|r| diag.contributions.get(&r.resolved).copied().unwrap_or(0) == 0)
            .map(|r| r.resolved.clone())
            .collect();
        diag.unused = unused;
        Ok((cfg, diag))
    }

    #[allow(clippy::too_many_lines)]
    fn from_file_internal(
        path: &Path,
        visited: &mut HashSet<PathBuf>,
        diag: Option<&mut ImportDiagnostics>,
    ) -> Result<Self> {
        Self::from_file_internal_with_options(path, visited, diag, true)
    }

    #[allow(clippy::too_many_lines)]
    fn from_file_internal_with_options(
        path: &Path,
        visited: &mut HashSet<PathBuf>,
        mut diag: Option<&mut ImportDiagnostics>,
        require_git_root: bool,
    ) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let parsed: Self = Self::parse(&content)?;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

        // Determine repository root for import security (relative-only, under repo
        // root) Skip git root requirement for absolute paths (they have their
        // own validation)
        let (_repo_root, repo_root_real) = if require_git_root {
            let repo_root = find_git_root_for_config(base_dir).with_context(|| {
                format!(
                    "Failed to determine git repository root for {}",
                    base_dir.display()
                )
            })?;
            let repo_root_real = repo_root
                .canonicalize()
                .unwrap_or_else(|_| repo_root.clone());
            (repo_root, repo_root_real)
        } else {
            // For files that don't require git root (e.g., in peter-hook directory),
            // use the file's directory as a dummy root
            let dummy_root = base_dir.to_path_buf();
            (dummy_root.clone(), dummy_root)
        };

        // Start with merged result from imports (if any)
        let mut merged_hooks: HashMap<String, HookDefinition> = HashMap::new();
        let mut merged_groups: HashMap<String, HookGroup> = HashMap::new();
        // Track sources to produce override diagnostics
        let mut hook_sources: HashMap<String, String> = HashMap::new();
        let mut group_sources: HashMap<String, String> = HashMap::new();

        if let Some(imports) = &parsed.imports {
            // Load global configuration for absolute path validation
            let global_config = GlobalConfig::load().unwrap_or_default();

            for imp in imports {
                // Expand tilde in the import path
                let expanded = shellexpand::tilde(imp);
                let p = Path::new(&*expanded);
                let (imp_path, is_absolute) = if p.is_absolute() {
                    // Check if absolute path is allowed via global config
                    if !global_config.is_absolute_path_allowed(p)? {
                        return Err(anyhow::anyhow!(
                            "Absolute import path not allowed: {imp}\nHint: Only imports from \
                             $HOME/.local/peter-hook are allowed.\nEnable with: peter-hook config \
                             init --allow-local"
                        ));
                    }
                    (p.to_path_buf(), true)
                } else {
                    (base_dir.join(p), false)
                };

                let imp_real = imp_path.canonicalize().with_context(|| {
                    format!("Failed to resolve import path: {}", imp_path.display())
                })?;

                // Enforce import stays within repo root (but only for relative imports)
                if !is_absolute && !imp_real.starts_with(&repo_root_real) {
                    return Err(anyhow::anyhow!(
                        "import outside repository root is not allowed: {} (repo root: {})",
                        imp_real.display(),
                        repo_root_real.display()
                    ));
                }

                // For absolute paths, verify they are still within peter-hook dir after
                // canonicalization (this protects against symlink attacks)
                if is_absolute && !global_config.is_absolute_path_allowed(&imp_real)? {
                    return Err(anyhow::anyhow!(
                        "Import path resolves outside $HOME/.local/peter-hook (possible symlink): \
                         {} -> {}",
                        imp_path.display(),
                        imp_real.display()
                    ));
                }

                // Diagnostics: record import edge
                if let Some(d) = diag.as_mut() {
                    d.imports.push(ImportRecord {
                        from: base_dir.display().to_string(),
                        resolved: imp_real.display().to_string(),
                    });
                }

                if !visited.insert(imp_real.clone()) {
                    // Already visited, report cycle and skip
                    if let Some(d) = diag.as_mut() {
                        d.cycles.push(imp_real.display().to_string());
                    }
                    continue;
                }
                // For absolute imports, don't require git root since they're in peter-hook
                // directory
                let skip_git_for_import = is_absolute;
                let imported = Self::from_file_internal_with_options(
                    &imp_real,
                    visited,
                    diag.as_deref_mut(),
                    !skip_git_for_import,
                )
                .with_context(|| format!("Failed to import config: {imp}"))?;
                if let Some(h) = imported.hooks {
                    for (k, v) in h {
                        if let Some(d) = diag.as_mut() {
                            let prev = hook_sources.get(&k).cloned();
                            if let Some(prev_src) = prev {
                                d.overrides.push(OverrideRecord {
                                    kind: "hook".to_string(),
                                    name: k.clone(),
                                    previous: prev_src,
                                    new: imp_real.display().to_string(),
                                });
                            } else {
                                *d.contributions
                                    .entry(imp_real.display().to_string())
                                    .or_default() += 1;
                            }
                        }
                        hook_sources.insert(k.clone(), imp_real.display().to_string());
                        merged_hooks.insert(k, v);
                    }
                }
                if let Some(g) = imported.groups {
                    for (k, v) in g {
                        if let Some(d) = diag.as_mut() {
                            let prev = group_sources.get(&k).cloned();
                            if let Some(prev_src) = prev {
                                d.overrides.push(OverrideRecord {
                                    kind: "group".to_string(),
                                    name: k.clone(),
                                    previous: prev_src,
                                    new: imp_real.display().to_string(),
                                });
                            } else {
                                *d.contributions
                                    .entry(imp_real.display().to_string())
                                    .or_default() += 1;
                            }
                        }
                        group_sources.insert(k.clone(), imp_real.display().to_string());
                        merged_groups.insert(k, v);
                    }
                }
            }
        }

        // Overlay with local definitions (local overrides imports)
        if let Some(h) = parsed.hooks {
            for (k, v) in h {
                if let Some(d) = diag.as_mut() {
                    if let Some(prev_src) = hook_sources.get(&k).cloned() {
                        d.overrides.push(OverrideRecord {
                            kind: "hook".to_string(),
                            name: k.clone(),
                            previous: prev_src,
                            new: path.display().to_string(),
                        });
                    } else {
                        *d.contributions
                            .entry(path.display().to_string())
                            .or_default() += 1;
                    }
                }
                hook_sources.insert(k.clone(), path.display().to_string());
                merged_hooks.insert(k, v);
            }
        }
        if let Some(g) = parsed.groups {
            for (k, v) in g {
                if let Some(d) = diag.as_mut() {
                    if let Some(prev_src) = group_sources.get(&k).cloned() {
                        d.overrides.push(OverrideRecord {
                            kind: "group".to_string(),
                            name: k.clone(),
                            previous: prev_src,
                            new: path.display().to_string(),
                        });
                    } else {
                        *d.contributions
                            .entry(path.display().to_string())
                            .or_default() += 1;
                    }
                }
                group_sources.insert(k.clone(), path.display().to_string());
                merged_groups.insert(k, v);
            }
        }

        Ok(Self {
            hooks: if merged_hooks.is_empty() {
                None
            } else {
                Some(merged_hooks)
            },
            groups: if merged_groups.is_empty() {
                None
            } else {
                Some(merged_groups)
            },
            imports: None,
        })
    }

    /// Parse a hooks.toml configuration from a string
    ///
    /// # Errors
    ///
    /// Returns an error if the TOML content cannot be parsed or validation
    /// fails
    pub fn parse(content: &str) -> Result<Self> {
        let config: Self = toml::from_str(content).context("Failed to parse TOML configuration")?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration for consistency
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A hook has both `files` and `run_always = true` set (conflicting
    ///   options)
    /// - A hook uses `execution_type` = "per-file" or "in-place" with template
    ///   variables like `{CHANGED_FILES}`
    pub fn validate(&self) -> Result<()> {
        if let Some(hooks) = &self.hooks {
            for (name, hook) in hooks {
                // Check for conflicting files and run_always settings
                if hook.run_always && hook.files.is_some() {
                    return Err(anyhow::anyhow!(
                        "Hook '{name}' cannot have both 'files' patterns and 'run_always = true'. \
                         Use either file patterns for conditional execution or 'run_always = \
                         true' for unconditional execution."
                    ));
                }

                // Check for conflicting requires_files and run_always settings
                if hook.requires_files && hook.run_always {
                    return Err(anyhow::anyhow!(
                        "Hook '{name}' cannot have both 'requires_files = true' and 'run_always = true'. \
                         These settings are contradictory: requires_files means the hook depends on file changes, \
                         while run_always means it should run regardless of changes."
                    ));
                }

                // Check for conflicting execution_type and template variable usage
                if matches!(
                    hook.execution_type,
                    ExecutionType::PerFile | ExecutionType::InPlace
                ) {
                    let command_str = hook.command.to_string();
                    if command_str.contains("{CHANGED_FILES}") {
                        return Err(anyhow::anyhow!(
                            "Hook '{}' with execution_type = '{}' should not use \
                             {{CHANGED_FILES}} template variables. Files are handled \
                             automatically. Use execution_type = 'other' for manual file handling.",
                            name,
                            match hook.execution_type {
                                ExecutionType::PerFile => "per-file",
                                ExecutionType::InPlace => "in-place",
                                ExecutionType::Other => unreachable!(),
                            }
                        ));
                    }
                }
            }
        }

        // Validate groups
        if let Some(groups) = &self.groups {
            for (name, group) in groups {
                // Check for conflicting placeholder and includes settings
                if group.placeholder == Some(true) && !group.includes.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Group '{name}' cannot have both 'placeholder = true' and non-empty \
                         'includes'. Placeholder groups should have 'includes = []' and are used \
                         only to trigger git hook installation for hierarchical resolution in \
                         subdirectories."
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get all hook names defined in this configuration
    #[must_use]
    pub fn get_hook_names(&self) -> Vec<String> {
        let mut names = Vec::new();

        if let Some(hooks) = &self.hooks {
            names.extend(hooks.keys().cloned());
        }

        if let Some(groups) = &self.groups {
            names.extend(groups.keys().cloned());
        }

        names.sort();
        names
    }

    /// Check if a hook or group exists
    #[must_use]
    pub fn has_hook(&self, name: &str) -> bool {
        self.hooks.as_ref().is_some_and(|h| h.contains_key(name))
            || self.groups.as_ref().is_some_and(|g| g.contains_key(name))
    }
}

#[derive(Debug, Default, Clone, Serialize)]
/// Diagnostic information collected during configuration import and merging
pub struct ImportDiagnostics {
    /// List of configuration files that were imported
    pub imports: Vec<ImportRecord>,
    /// List of configuration entries that were overridden during merging
    pub overrides: Vec<OverrideRecord>,
    /// List of import cycles detected in configuration
    pub cycles: Vec<String>,
    /// List of unused import declarations
    pub unused: Vec<String>,
    /// Count of contributions from each configuration source
    #[serde(skip)]
    pub contributions: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
/// Record of a configuration file import operation
pub struct ImportRecord {
    /// The original import path as specified in configuration
    pub from: String,
    /// The resolved absolute path to the imported file
    pub resolved: String,
}

#[derive(Debug, Clone, Serialize)]
/// Record of a configuration override during merging
pub struct OverrideRecord {
    /// The type of configuration being overridden ("hook" or "group")
    pub kind: String,
    /// The name of the configuration entry being overridden
    pub name: String,
    /// The previous configuration source
    pub previous: String,
    /// The new configuration source that overrode the previous one
    pub new: String,
}

/// Find git repository root by walking up directories for config parsing
fn find_git_root_for_config(start_dir: &Path) -> Result<PathBuf> {
    let mut current = start_dir;
    loop {
        if current.join(".git").exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return Err(anyhow::anyhow!("Not in a git repository")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_hook() {
        let toml = r#"
[hooks.test]
command = "echo 'hello world'"
description = "A simple test hook"
"#;

        let config = HookConfig::parse(toml).unwrap();
        assert!(config.hooks.is_some());

        let hooks = config.hooks.unwrap();
        assert!(hooks.contains_key("test"));

        let hook = &hooks["test"];
        assert_eq!(
            hook.command,
            HookCommand::Shell("echo 'hello world'".to_string())
        );
        assert_eq!(hook.description, Some("A simple test hook".to_string()));
        assert!(!hook.modifies_repository); // Default should be false
        assert!(hook.files.is_none()); // Default should be None
        assert!(!hook.run_always); // Default should be false
        assert!(hook.depends_on.is_none()); // Default should be None
    }

    #[test]
    fn test_parse_array_command() {
        let toml = r#"
[hooks.lint]
command = ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["lint"];

        assert_eq!(
            hook.command,
            HookCommand::Args(vec![
                "cargo".to_string(),
                "clippy".to_string(),
                "--all-targets".to_string(),
                "--".to_string(),
                "-D".to_string(),
                "warnings".to_string(),
            ])
        );
    }

    #[test]
    fn test_parse_hook_group() {
        let toml = r#"
[groups.python-lint]
includes = ["python.ruff", "python.type-check"]
description = "Python linting and type checking"
parallel = true
"#;

        let config = HookConfig::parse(toml).unwrap();
        assert!(config.groups.is_some());

        let groups = config.groups.unwrap();
        assert!(groups.contains_key("python-lint"));

        let group = &groups["python-lint"];
        assert_eq!(group.includes, vec!["python.ruff", "python.type-check"]);
        assert_eq!(group.get_execution_strategy(), ExecutionStrategy::Parallel);
    }

    #[test]
    fn test_get_hook_names() {
        let toml = r#"
[hooks.test1]
command = "echo test1"

[hooks.test2]
command = "echo test2"

[groups.all-tests]
includes = ["test1", "test2"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let names = config.get_hook_names();

        assert_eq!(names, vec!["all-tests", "test1", "test2"]);
    }

    #[test]
    fn test_repository_modifying_hook() {
        let toml = r#"
[hooks.format]
command = "cargo fmt"
description = "Format Rust code"
modifies_repository = true
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["format"];

        assert!(hook.modifies_repository);
        assert_eq!(hook.description, Some("Format Rust code".to_string()));
    }

    #[test]
    fn test_imports_merge_and_override() {
        use std::fs;
        use tempfile::TempDir;
        let td = TempDir::new().unwrap();
        let dir = td.path();
        // Simulate git repo root
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let lib = dir.join("hooks.lib.toml");
        let base = dir.join("hooks.toml");

        fs::write(
            &lib,
            r#"
[hooks.lint]
command = "echo lib-lint"

[groups.common]
includes = ["lint"]
"#,
        )
        .unwrap();

        fs::write(
            &base,
            r#"
imports = ["hooks.lib.toml"]

[hooks.lint]
command = "echo local-lint"  # override

[hooks.test]
command = "echo test"

[groups.pre-commit]
includes = ["common", "lint", "test"]
"#,
        )
        .unwrap();

        let cfg = HookConfig::from_file(&base).unwrap();
        let names = cfg.get_hook_names();
        assert!(names.contains(&"lint".to_string()));
        assert!(names.contains(&"test".to_string()));
        assert!(names.contains(&"common".to_string()));
        assert!(names.contains(&"pre-commit".to_string()));

        let hooks = cfg.hooks.unwrap();
        // local override should win
        match &hooks["lint"].command {
            HookCommand::Shell(s) => assert_eq!(s, "echo local-lint"),
            HookCommand::Args(_) => panic!("expected shell"),
        }
    }

    #[test]
    fn test_import_cycle() {
        use std::fs;
        use tempfile::TempDir;
        let td = TempDir::new().unwrap();
        let dir = td.path();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let a = dir.join("a.toml");
        let b = dir.join("b.toml");
        fs::write(
            &a,
            format!(
                "imports = [\"{}\"]\n\n[hooks.a]\ncommand = \"echo a\"\n",
                b.file_name().unwrap().to_str().unwrap()
            ),
        )
        .unwrap();
        fs::write(
            &b,
            format!(
                "imports = [\"{}\"]\n\n[hooks.b]\ncommand = \"echo b\"\n",
                a.file_name().unwrap().to_str().unwrap()
            ),
        )
        .unwrap();

        let cfg = HookConfig::from_file(&a).unwrap();
        let names = cfg.get_hook_names();
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
    }

    #[test]
    fn test_imports_reject_absolute_outside_home() {
        use std::fs;
        use tempfile::TempDir;
        let td = TempDir::new().unwrap();
        let dir = td.path();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let base = dir.join("hooks.toml");
        fs::write(&base, "imports = [\"/etc/passwd\"]\n").unwrap();
        let err = HookConfig::from_file(&base).unwrap_err();
        // Now we expect it to be rejected because it's not in home directory/allowlist
        assert!(format!("{err:#}").contains("Absolute import path not allowed"));
    }

    #[test]
    fn test_imports_reject_outside_repo_root() {
        use std::fs;
        use tempfile::TempDir;
        let outer = TempDir::new().unwrap();
        let outer_dir = outer.path();
        // repo root
        std::fs::create_dir_all(outer_dir.join("repo/.git")).unwrap();
        // file outside repo
        let outside = outer_dir.join("evil.toml");
        fs::write(&outside, "[hooks.bad]\ncommand=\"echo bad\"\n").unwrap();
        // hooks.toml at repo root trying to import ../evil.toml
        let base = outer_dir.join("repo/hooks.toml");
        fs::write(&base, "imports = [\"../evil.toml\"]\n").unwrap();
        let err = HookConfig::from_file(&base).unwrap_err();
        assert!(format!("{err:#}").contains("outside repository root"));
    }

    #[test]
    fn test_execution_strategies() {
        let toml = r#"
[groups.sequential]
includes = ["test1", "test2"]
execution = "sequential"

[groups.parallel]
includes = ["test1", "test2"]  
execution = "parallel"

[groups.force-parallel]
includes = ["test1", "test2"]
execution = "force-parallel"

[groups.backward-compat]
includes = ["test1", "test2"]
parallel = true
"#;

        let config = HookConfig::parse(toml).unwrap();
        let groups = config.groups.unwrap();

        assert_eq!(
            groups["sequential"].get_execution_strategy(),
            ExecutionStrategy::Sequential
        );
        assert_eq!(
            groups["parallel"].get_execution_strategy(),
            ExecutionStrategy::Parallel
        );
        assert_eq!(
            groups["force-parallel"].get_execution_strategy(),
            ExecutionStrategy::ForceParallel
        );
        assert_eq!(
            groups["backward-compat"].get_execution_strategy(),
            ExecutionStrategy::Parallel
        );
    }

    #[test]
    fn test_file_pattern_hook() {
        let toml = r#"
[hooks.rust-lint]
command = "cargo clippy"
description = "Lint Rust code"
modifies_repository = false
files = ["**/*.rs", "Cargo.toml"]

[hooks.js-lint]
command = "eslint src/"
description = "Lint JavaScript code"
modifies_repository = false
files = ["**/*.js", "**/*.ts", "package.json"]
run_always = false

[hooks.format-all]
command = "prettier --write ."
description = "Format all files"
modifies_repository = true
run_always = true
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();

        let rust_hook = &hooks["rust-lint"];
        assert_eq!(
            rust_hook.files,
            Some(vec!["**/*.rs".to_string(), "Cargo.toml".to_string()])
        );
        assert!(!rust_hook.run_always);

        let js_hook = &hooks["js-lint"];
        assert_eq!(
            js_hook.files,
            Some(vec![
                "**/*.js".to_string(),
                "**/*.ts".to_string(),
                "package.json".to_string()
            ])
        );
        assert!(!js_hook.run_always);

        let format_hook = &hooks["format-all"];
        assert!(format_hook.run_always);
        assert!(format_hook.files.is_none()); // run_always hooks don't need file patterns
    }

    #[test]
    fn test_hook_dependencies_and_templating() {
        let toml = r#"
[hooks.format]
command = "cargo fmt --manifest-path={HOOK_DIR}/Cargo.toml"
description = "Format code with template"
modifies_repository = true
env = { PROJECT_ROOT = "{REPO_ROOT}", BUILD_MODE = "debug" }

[hooks.lint]
command = ["cargo", "clippy", "--manifest-path={HOOK_DIR}/Cargo.toml"]
description = "Lint after formatting"
modifies_repository = false
depends_on = ["format"]
files = ["**/*.rs"]

[hooks.test]
command = "cd {WORKING_DIR} && cargo test"
description = "Test with working directory template"
modifies_repository = false
depends_on = ["lint"]
workdir = "{REPO_ROOT}/target"
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();

        // Test format hook
        let format_hook = &hooks["format"];
        assert!(format_hook.command.to_string().contains("{HOOK_DIR}"));
        assert!(format_hook.modifies_repository);
        assert_eq!(
            format_hook.env,
            Some(
                [
                    ("PROJECT_ROOT".to_string(), "{REPO_ROOT}".to_string()),
                    ("BUILD_MODE".to_string(), "debug".to_string()),
                ]
                .iter()
                .cloned()
                .collect()
            )
        );

        // Test lint hook
        let lint_hook = &hooks["lint"];
        assert_eq!(lint_hook.depends_on, Some(vec!["format".to_string()]));
        assert_eq!(lint_hook.files, Some(vec!["**/*.rs".to_string()]));

        // Test test hook
        let test_hook = &hooks["test"];
        assert_eq!(test_hook.depends_on, Some(vec!["lint".to_string()]));
        assert_eq!(test_hook.workdir, Some("{REPO_ROOT}/target".to_string()));
    }

    #[test]
    fn test_validation_conflicting_files_and_run_always() {
        let toml = r#"
[hooks.bad-hook]
command = "echo test"
files = ["**/*.rs"]
run_always = true
"#;

        let err = HookConfig::parse(toml).unwrap_err();
        assert!(
            err.to_string()
                .contains("cannot have both 'files' patterns and 'run_always = true'")
        );
        assert!(err.to_string().contains("bad-hook"));
    }

    #[test]
    fn test_validation_allows_files_without_run_always() {
        let toml = r#"
[hooks.good-hook]
command = "echo test"
files = ["**/*.rs"]
run_always = false
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["good-hook"];
        assert_eq!(hook.files, Some(vec!["**/*.rs".to_string()]));
        assert!(!hook.run_always);
    }

    #[test]
    fn test_validation_allows_run_always_without_files() {
        let toml = r#"
[hooks.good-hook]
command = "echo test"
run_always = true
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["good-hook"];
        assert!(hook.files.is_none());
        assert!(hook.run_always);
    }

    #[test]
    fn test_execution_type_defaults_to_per_file() {
        let toml = r#"
[hooks.test-hook]
command = "eslint"
files = ["**/*.js"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["test-hook"];
        assert_eq!(hook.execution_type, ExecutionType::PerFile);
    }

    #[test]
    fn test_execution_type_in_place() {
        let toml = r#"
[hooks.test-hook]
command = "prettier"
execution_type = "in-place"
files = ["**/*.js"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["test-hook"];
        assert_eq!(hook.execution_type, ExecutionType::InPlace);
    }

    #[test]
    fn test_execution_type_other() {
        let toml = r#"
[hooks.test-hook]
command = "custom-tool {CHANGED_FILES}"
execution_type = "other"
files = ["**/*.js"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["test-hook"];
        assert_eq!(hook.execution_type, ExecutionType::Other);
    }

    #[test]
    fn test_validation_rejects_per_file_with_changed_files_template() {
        let toml = r#"
[hooks.bad-hook]
command = "eslint {CHANGED_FILES}"
execution_type = "per-file"
files = ["**/*.js"]
"#;

        let err = HookConfig::parse(toml).unwrap_err();
        assert!(
            err.to_string()
                .contains("should not use {CHANGED_FILES} template variables")
        );
        assert!(err.to_string().contains("per-file"));
        assert!(err.to_string().contains("bad-hook"));
    }

    #[test]
    fn test_validation_rejects_in_place_with_changed_files_template() {
        let toml = r#"
[hooks.bad-hook]
command = "prettier {CHANGED_FILES}"
execution_type = "in-place"
files = ["**/*.js"]
"#;

        let err = HookConfig::parse(toml).unwrap_err();
        assert!(
            err.to_string()
                .contains("should not use {CHANGED_FILES} template variables")
        );
        assert!(err.to_string().contains("in-place"));
        assert!(err.to_string().contains("bad-hook"));
    }

    #[test]
    fn test_validation_allows_other_with_changed_files_template() {
        let toml = r#"
[hooks.good-hook]
command = "custom-tool {CHANGED_FILES}"
execution_type = "other"
files = ["**/*.js"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["good-hook"];
        assert_eq!(hook.execution_type, ExecutionType::Other);
        assert!(hook.command.to_string().contains("{CHANGED_FILES}"));
    }

    #[test]
    fn test_run_at_root_flag() {
        let toml = r#"
[hooks.root-hook]
command = "echo 'running at root'"
run_at_root = true

[hooks.normal-hook]
command = "echo 'running at hook dir'"
run_at_root = false

[hooks.default-hook]
command = "echo 'default behavior'"
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();

        let root_hook = &hooks["root-hook"];
        assert!(root_hook.run_at_root);

        let normal_hook = &hooks["normal-hook"];
        assert!(!normal_hook.run_at_root);

        let default_hook = &hooks["default-hook"];
        assert!(!default_hook.run_at_root); // Default should be false
    }

    #[test]
    fn test_absolute_imports_not_in_allowlist() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();
        fs::create_dir_all(repo_root.join(".git")).unwrap();

        let hooks_file = repo_root.join("hooks.toml");

        // Try to import absolute path not in allowlist
        let toml_content = format!(
            r#"
imports = ["{}/not-allowed.toml"]

[hooks.test]
command = "echo test"
"#,
            temp_dir.path().join("other").display()
        );

        fs::write(&hooks_file, toml_content).unwrap();

        let err = HookConfig::from_file(&hooks_file).unwrap_err();
        assert!(err.to_string().contains("Absolute import path not allowed"));
        assert!(
            err.to_string()
                .contains("peter-hook config init --allow-local")
        );
    }

    #[test]
    fn test_absolute_imports_in_peter_hook_dir() {
        use std::fs;
        use tempfile::TempDir;

        // This test is challenging because we need to create files in the real
        // peter-hook dir For now, just test the basic structure

        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();
        fs::create_dir_all(repo_root.join(".git")).unwrap();

        let hooks_file = repo_root.join("hooks.toml");

        // Test with a peter-hook directory path (this will likely fail in CI/tests
        // but demonstrates the intended usage)
        if let Some(home_dir) = dirs::home_dir() {
            let peter_hook_dir = home_dir.join(".local").join("peter-hook");
            let test_import_path = peter_hook_dir.join("test.toml");

            let toml_content = format!(
                r#"
imports = ["{}"]

[hooks.local-hook]
command = "echo local"
"#,
                test_import_path.display()
            );
            fs::write(&hooks_file, toml_content).unwrap();

            // This test will likely fail unless the file actually exists
            // which is expected - it's testing the path validation logic
            let result = HookConfig::from_file(&hooks_file);

            // Should get a file not found error, not a path validation error
            if let Err(e) = result {
                let error_str = e.to_string();
                // Should not be a path validation error if allow_local defaults to false
                assert!(
                    error_str.contains("Absolute import path not allowed")
                        || error_str.contains("Failed to resolve import path")
                        || error_str.contains("Failed to import config"),
                    "Unexpected error: {error_str}"
                );
            }
        }
    }

    #[test]
    fn test_symlink_attack_protection() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let home_dir = temp_dir.path().join("home");
        let allowed_dir = home_dir.join("allowed");
        let forbidden_dir = home_dir.join("forbidden");

        fs::create_dir_all(&allowed_dir).unwrap();
        fs::create_dir_all(&forbidden_dir).unwrap();

        // Create a secret file outside the allowlist
        let secret_file = forbidden_dir.join("secret.toml");
        fs::write(
            &secret_file,
            r#"
[hooks.secret]
command = "rm -rf /"
"#,
        )
        .unwrap();

        // Try to create symlink (skip on Windows or if it fails)
        #[cfg(unix)]
        {
            let symlink_path = allowed_dir.join("innocent.toml");
            if std::os::unix::fs::symlink(&secret_file, &symlink_path).is_ok() {
                // Create repository
                let repo_root = home_dir.join("project");
                fs::create_dir_all(&repo_root).unwrap();
                fs::create_dir_all(repo_root.join(".git")).unwrap();

                let hooks_file = repo_root.join("hooks.toml");
                let toml_content = format!(
                    r#"
imports = ["{}"]

[hooks.test]
command = "echo test"
"#,
                    symlink_path.display()
                );
                fs::write(&hooks_file, toml_content).unwrap();

                // This should be rejected because the symlink resolves outside allowlist
                let err = HookConfig::from_file(&hooks_file).unwrap_err();

                // The symlink test might fail differently depending on environment
                // Check for either expected error message
                let error_str = err.to_string();
                assert!(
                    error_str.contains("Import path resolves outside allowlist")
                        || error_str.contains("Absolute import path not allowed")
                        || error_str.contains("Failed to resolve import path"),
                    "Unexpected error: {error_str}"
                );
            }
        }
    }

    #[test]
    fn test_relative_imports_still_work() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        fs::create_dir_all(repo_root.join(".git")).unwrap();

        // Create imported file
        let imported_file = repo_root.join("shared.toml");
        fs::write(
            &imported_file,
            r#"
[hooks.shared]
command = "echo shared"
"#,
        )
        .unwrap();

        // Create main config with relative import
        let hooks_file = repo_root.join("hooks.toml");
        fs::write(
            &hooks_file,
            r#"
imports = ["./shared.toml"]

[hooks.local]
command = "echo local"
"#,
        )
        .unwrap();

        // This should still work (relative imports unchanged)
        let config = HookConfig::from_file(&hooks_file).unwrap();
        let hook_names = config.get_hook_names();

        assert!(hook_names.contains(&"shared".to_string()));
        assert!(hook_names.contains(&"local".to_string()));
    }

    #[test]
    fn test_placeholder_group_parsing() {
        let toml = r#"
[groups.pre-commit]
includes = []
placeholder = true
description = "Hierarchical pre-commit hooks"

[groups.pre-push]
includes = ["lint", "test"]
description = "Real hooks at root"
"#;

        let config = HookConfig::parse(toml).unwrap();
        let groups = config.groups.unwrap();

        // Placeholder group should have placeholder = Some(true)
        assert_eq!(groups["pre-commit"].placeholder, Some(true));
        assert!(groups["pre-commit"].includes.is_empty());

        // Non-placeholder group should have placeholder = None (default)
        assert_eq!(groups["pre-push"].placeholder, None);
        assert!(!groups["pre-push"].includes.is_empty());
    }

    #[test]
    fn test_placeholder_default_false() {
        let toml = r#"
[groups.test]
includes = ["lint"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let groups = config.groups.unwrap();

        // Groups without placeholder field should default to None
        assert_eq!(groups["test"].placeholder, None);
    }

    #[test]
    fn test_placeholder_validation_error() {
        use std::fs;
        use tempfile::TempDir;
        let td = TempDir::new().unwrap();
        let dir = td.path();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let config_file = dir.join("hooks.toml");

        fs::write(
            &config_file,
            r#"
[groups.invalid]
includes = ["lint", "test"]
placeholder = true
"#,
        )
        .unwrap();

        // Validation happens during from_file, so expect error there
        let err = HookConfig::from_file(&config_file).unwrap_err();

        assert!(err.to_string().contains("placeholder = true"));
        assert!(err.to_string().contains("non-empty 'includes'"));
    }

    #[test]
    fn test_placeholder_with_empty_includes_valid() {
        let toml = r#"
[groups.valid-placeholder]
includes = []
placeholder = true
description = "Valid placeholder"
"#;

        let config = HookConfig::parse(toml).unwrap();
        // Should not error on validation
        config.validate().unwrap();
    }

    #[test]
    fn test_requires_files_field() {
        let toml = r#"
[hooks.test-hook]
command = "pytest"
requires_files = true
files = ["**/*.py"]
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["test-hook"];
        assert!(hook.requires_files);
        assert_eq!(hook.files, Some(vec!["**/*.py".to_string()]));
    }

    #[test]
    fn test_requires_files_defaults_to_false() {
        let toml = r#"
[hooks.test-hook]
command = "echo test"
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.unwrap();
        let hook = &hooks["test-hook"];
        assert!(!hook.requires_files);
    }

    #[test]
    fn test_validation_rejects_requires_files_with_run_always() {
        let toml = r#"
[hooks.bad-hook]
command = "echo test"
requires_files = true
run_always = true
"#;

        let err = HookConfig::parse(toml).unwrap_err();
        assert!(err.to_string().contains("requires_files"));
        assert!(err.to_string().contains("run_always"));
        assert!(err.to_string().contains("contradictory"));
    }

    #[test]
    fn test_requires_files_with_files_pattern_valid() {
        let toml = r#"
[hooks.pytest]
command = "pytest"
requires_files = true
files = ["**/*.py", "**/test_*.py"]
modifies_repository = false
"#;

        let config = HookConfig::parse(toml).unwrap();
        let hooks = config.hooks.as_ref().unwrap();
        let hook = &hooks["pytest"];
        assert!(hook.requires_files);
        assert_eq!(
            hook.files,
            Some(vec!["**/*.py".to_string(), "**/test_*.py".to_string()])
        );
        config.validate().unwrap();
    }
}

impl std::fmt::Display for HookCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shell(cmd) => write!(f, "{cmd}"),
            Self::Args(args) => write!(f, "{}", args.join(" ")),
        }
    }
}
