//! Git hook installation and management

use crate::{
    config::HookConfig,
    git::{GitRepository, WorktreeHookStrategy},
    hooks::HookResolver,
};
use anyhow::{Context, Result};
use std::path::Path;

/// Git hook installer and manager
pub struct GitHookInstaller {
    /// The git repository to work with
    repository: GitRepository,
    /// Path to the peter-hook binary
    binary_path: String,
    /// Strategy for handling worktree hooks
    worktree_strategy: WorktreeHookStrategy,
}

/// Supported git hook events
pub const SUPPORTED_HOOKS: &[&str] = &[
    "pre-commit",
    "commit-msg",
    "pre-push",
    "post-commit",
    "post-merge",
    "post-checkout",
    "pre-rebase",
    "post-rewrite",
    "pre-receive",
    "post-receive",
    "update",
    "post-update",
    "pre-applypatch",
    "post-applypatch",
    "applypatch-msg",
];

impl GitHookInstaller {
    /// Create a new git hook installer
    ///
    /// # Errors
    ///
    /// Returns an error if the git repository cannot be found or if the binary
    /// path is invalid
    pub fn new() -> Result<Self> {
        Self::with_strategy(WorktreeHookStrategy::default())
    }

    /// Create a new git hook installer with a specific worktree strategy
    ///
    /// # Errors
    ///
    /// Returns an error if the git repository cannot be found or if the binary
    /// path is invalid
    pub fn with_strategy(strategy: WorktreeHookStrategy) -> Result<Self> {
        let repository =
            GitRepository::find_from_current_dir().context("Failed to find git repository")?;

        // Try to find the binary path
        let binary_path = Self::detect_binary_path();

        Ok(Self {
            repository,
            binary_path,
            worktree_strategy: strategy,
        })
    }

    /// Create a new installer for a specific repository and binary path
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // GitRepository contains non-const fields
    pub fn with_repository_and_binary(repository: GitRepository, binary_path: String) -> Self {
        Self {
            repository,
            binary_path,
            worktree_strategy: WorktreeHookStrategy::default(),
        }
    }

    /// Create a new installer for a specific repository, binary path, and
    /// strategy
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // GitRepository contains non-const fields
    pub fn with_repository_binary_and_strategy(
        repository: GitRepository,
        binary_path: String,
        strategy: WorktreeHookStrategy,
    ) -> Self {
        Self {
            repository,
            binary_path,
            worktree_strategy: strategy,
        }
    }

    /// Install hooks for all events that have configurations
    ///
    /// # Errors
    ///
    /// Returns an error if hook installation fails
    pub fn install_all(&self) -> Result<InstallationReport> {
        let resolver = HookResolver::new(&self.repository.root);
        let mut report = InstallationReport {
            installed: Vec::new(),
            skipped: Vec::new(),
            backed_up: Vec::new(),
            errors: Vec::new(),
        };

        self.repository.ensure_hooks_directory()?;

        // Check each supported hook event
        for &hook_event in SUPPORTED_HOOKS {
            match self.install_hook(hook_event, &resolver) {
                Ok(action) => match action {
                    InstallAction::Installed => report.installed.push(hook_event.to_string()),
                    InstallAction::Skipped(reason) => {
                        report.skipped.push((hook_event.to_string(), reason));
                    }
                    InstallAction::BackedUp(backup_path) => {
                        report.backed_up.push((hook_event.to_string(), backup_path));
                        report.installed.push(hook_event.to_string());
                    }
                },
                Err(e) => report
                    .errors
                    .push((hook_event.to_string(), format!("{e:#}"))),
            }
        }

        Ok(report)
    }

    /// Install a hook for a specific event
    ///
    /// # Errors
    ///
    /// Returns an error if hook installation fails
    pub fn install_hook(&self, hook_event: &str, resolver: &HookResolver) -> Result<InstallAction> {
        // Check if we have configuration for this event
        match resolver.resolve_hooks(hook_event)? {
            Some(_) => {
                // We have real hooks configuration, install the hook
                self.install_hook_script(hook_event)
            }
            None => {
                // No hooks resolved - but check if there's a placeholder group
                if let Some(config_path) = resolver.find_config_file()? {
                    let config = HookConfig::from_file(&config_path)?;
                    if let Some(groups) = &config.groups {
                        if let Some(group) = groups.get(hook_event) {
                            if group.placeholder == Some(true) {
                                // Placeholder group found - install hook script for hierarchical
                                // resolution
                                return self.install_hook_script(hook_event);
                            }
                        }
                    }
                }
                // No configuration and no placeholder
                Ok(InstallAction::Skipped("No configuration found".to_string()))
            }
        }
    }

    /// Get the effective hooks directory based on worktree strategy
    fn get_effective_hooks_dir(&self) -> std::path::PathBuf {
        let effective_strategy = match self.worktree_strategy {
            WorktreeHookStrategy::Detect => {
                // Auto-detect: check if worktree-specific hooks already exist
                if self.repository.is_worktree {
                    let worktree_hooks_dir = self.repository.get_worktree_hooks_dir();
                    if worktree_hooks_dir.exists()
                        && std::fs::read_dir(&worktree_hooks_dir)
                            .map(|mut entries| entries.any(|_| true))
                            .unwrap_or(false)
                    {
                        WorktreeHookStrategy::PerWorktree
                    } else {
                        WorktreeHookStrategy::Shared
                    }
                } else {
                    WorktreeHookStrategy::Shared
                }
            }
            strategy => strategy,
        };

        match effective_strategy {
            WorktreeHookStrategy::Shared => self.repository.get_common_hooks_dir().to_path_buf(),
            WorktreeHookStrategy::PerWorktree => {
                if self.repository.is_worktree {
                    self.repository.get_worktree_hooks_dir()
                } else {
                    // For main repository, per-worktree is same as shared
                    self.repository.get_common_hooks_dir().to_path_buf()
                }
            }
            WorktreeHookStrategy::Detect => unreachable!("Already resolved above"),
        }
    }

    /// Setup worktree configuration if needed
    fn setup_worktree_config(&self, hooks_dir: &Path) -> Result<()> {
        if self.worktree_strategy == WorktreeHookStrategy::PerWorktree
            && self.repository.is_worktree
        {
            // Ensure hooks directory exists
            if !hooks_dir.exists() {
                std::fs::create_dir_all(hooks_dir).with_context(|| {
                    format!(
                        "Failed to create worktree hooks directory: {}",
                        hooks_dir.display()
                    )
                })?;
            }

            // TODO: Set git config for worktree-specific hooks
            // This would require running: git config --worktree core.hookspath
            // <path> For now, we'll just create the directory
            // structure
        }
        Ok(())
    }

    /// Install the actual hook script
    fn install_hook_script(&self, hook_event: &str) -> Result<InstallAction> {
        let effective_hooks_dir = self.get_effective_hooks_dir();
        let hook_path = effective_hooks_dir.join(hook_event);

        // Setup worktree configuration if needed
        self.setup_worktree_config(&effective_hooks_dir)?;

        // Check if hook already exists at the effective location
        if hook_path.exists() {
            let content = std::fs::read_to_string(&hook_path)
                .with_context(|| format!("Failed to read hook file: {}", hook_path.display()))?;

            let is_managed = content.contains("# Generated by peter-hook");

            if is_managed {
                // Already managed by us, just update it
                self.write_hook_script(&hook_path, hook_event)?;
                return Ok(InstallAction::Installed);
            }
            // Existing hook not managed by us, back it up
            let backup_path = Self::backup_existing_hook(&hook_path)?;
            self.write_hook_script(&hook_path, hook_event)?;
            return Ok(InstallAction::BackedUp(backup_path));
        }

        // No existing hook, create new one
        self.write_hook_script(&hook_path, hook_event)?;
        Ok(InstallAction::Installed)
    }

    /// Write the hook script content
    fn write_hook_script(&self, hook_path: &Path, hook_event: &str) -> Result<()> {
        let script_content = self.generate_hook_script(hook_event);

        std::fs::write(hook_path, script_content)
            .with_context(|| format!("Failed to write hook script: {}", hook_path.display()))?;

        // Make executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(hook_path)?.permissions();
            permissions.set_mode(0o755); // rwxr-xr-x
            std::fs::set_permissions(hook_path, permissions).with_context(|| {
                format!("Failed to make hook executable: {}", hook_path.display())
            })?;
        }

        Ok(())
    }

    /// Generate the hook script content
    fn generate_hook_script(&self, hook_event: &str) -> String {
        match hook_event {
            "commit-msg" | "pre-push" | "post-receive" | "update" => {
                // These hooks receive arguments from git
                format!(
                    r#"#!/bin/sh
# Generated by peter-hook
# Do not edit this file directly - it will be overwritten
# Edit your hooks.toml configuration instead

exec "{}" run {} "$@"
"#,
                    self.binary_path, hook_event
                )
            }
            _ => {
                // Standard hooks with no arguments
                format!(
                    r#"#!/bin/sh
# Generated by peter-hook
# Do not edit this file directly - it will be overwritten
# Edit your hooks.toml configuration instead

exec "{}" run {}
"#,
                    self.binary_path, hook_event
                )
            }
        }
    }

    /// Backup an existing hook file
    fn backup_existing_hook(hook_path: &Path) -> Result<String> {
        let backup_path = format!("{}.backup", hook_path.display());
        std::fs::copy(hook_path, &backup_path)
            .with_context(|| format!("Failed to backup existing hook to {backup_path}"))?;
        Ok(backup_path)
    }

    /// Detect the path to the peter-hook binary
    fn detect_binary_path() -> String {
        // Try current executable path first
        if let Ok(current_exe) = std::env::current_exe() {
            return current_exe.display().to_string();
        }

        // Try to find in PATH
        if let Ok(output) = std::process::Command::new("which")
            .arg("peter-hook")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                let path = path.trim();
                if !path.is_empty() && Path::new(path).exists() {
                    return path.to_string();
                }
            }
        }

        // Fallback to assuming it's in PATH
        "peter-hook".to_string()
    }

    /// Uninstall peter-hook managed hooks
    #[must_use]
    pub fn uninstall_all(&self) -> UninstallationReport {
        let mut report = UninstallationReport {
            removed: Vec::new(),
            restored: Vec::new(),
            errors: Vec::new(),
        };

        for &hook_event in SUPPORTED_HOOKS {
            match self.uninstall_hook(hook_event) {
                Ok(action) => match action {
                    UninstallAction::Removed => report.removed.push(hook_event.to_string()),
                    UninstallAction::Restored(backup_path) => {
                        report.restored.push((hook_event.to_string(), backup_path));
                    }
                    UninstallAction::NotManaged | UninstallAction::NotFound => {
                        // Hook exists but not managed by us, or no hook exists
                        // - skip
                    }
                },
                Err(e) => report
                    .errors
                    .push((hook_event.to_string(), format!("{e:#}"))),
            }
        }

        report
    }

    /// Uninstall a specific hook
    fn uninstall_hook(&self, hook_event: &str) -> Result<UninstallAction> {
        let Some(hook_info) = self.repository.get_hook_info(hook_event)? else {
            return Ok(UninstallAction::NotFound);
        };

        if !hook_info.is_managed {
            return Ok(UninstallAction::NotManaged);
        }

        // Remove the managed hook
        std::fs::remove_file(&hook_info.path)
            .with_context(|| format!("Failed to remove hook: {}", hook_info.path.display()))?;

        // Check for backup file
        let backup_path = format!("{}.backup", hook_info.path.display());
        if Path::new(&backup_path).exists() {
            // Restore the backup
            std::fs::rename(&backup_path, &hook_info.path)
                .with_context(|| format!("Failed to restore backup: {backup_path}"))?;
            Ok(UninstallAction::Restored(backup_path))
        } else {
            Ok(UninstallAction::Removed)
        }
    }
}

/// Result of hook installation
#[derive(Debug)]
pub enum InstallAction {
    /// Hook was installed successfully
    Installed,
    /// Hook installation was skipped with reason
    Skipped(String),
    /// Existing hook was backed up and new hook installed
    BackedUp(String),
}

/// Result of hook uninstallation
#[derive(Debug)]
pub enum UninstallAction {
    /// Hook was removed
    Removed,
    /// Hook was removed and backup was restored
    Restored(String),
    /// Hook exists but is not managed by peter-hook
    NotManaged,
    /// No hook found
    NotFound,
}

/// Report of installation operations
#[derive(Debug)]
pub struct InstallationReport {
    /// Successfully installed hooks
    pub installed: Vec<String>,
    /// Skipped hooks with reasons
    pub skipped: Vec<(String, String)>,
    /// Backed up hooks
    pub backed_up: Vec<(String, String)>,
    /// Errors during installation
    pub errors: Vec<(String, String)>,
}

/// Report of uninstallation operations
#[derive(Debug)]
pub struct UninstallationReport {
    /// Removed hooks
    pub removed: Vec<String>,
    /// Restored hooks with backup paths
    pub restored: Vec<(String, String)>,
    /// Errors during uninstallation
    pub errors: Vec<(String, String)>,
}

impl InstallationReport {
    /// Print a summary of the installation
    pub fn print_summary(&self) {
        println!("Git Hook Installation Summary:");
        println!("=============================");

        if !self.installed.is_empty() {
            println!("✅ Installed hooks: {}", self.installed.join(", "));
        }

        if !self.backed_up.is_empty() {
            println!("💾 Backed up existing hooks:");
            for (hook, backup) in &self.backed_up {
                println!("  {hook} → {backup}");
            }
        }

        if !self.skipped.is_empty() {
            println!("⏭️  Skipped hooks:");
            for (hook, reason) in &self.skipped {
                println!("  {hook}: {reason}");
            }
        }

        if !self.errors.is_empty() {
            println!("❌ Errors:");
            for (hook, error) in &self.errors {
                println!("  {hook}: {error}");
            }
        }

        let total_success = self.installed.len() + self.backed_up.len();
        if total_success > 0 {
            println!("\n🎉 Successfully configured {total_success} git hooks!");
            println!("Your hooks are now active and will run automatically with git commands.");
        }
    }

    /// Check if the installation was completely successful
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

impl UninstallationReport {
    /// Print a summary of the uninstallation
    pub fn print_summary(&self) {
        println!("Git Hook Uninstallation Summary:");
        println!("===============================");

        if !self.removed.is_empty() {
            println!("🗑️  Removed hooks: {}", self.removed.join(", "));
        }

        if !self.restored.is_empty() {
            println!("🔄 Restored hooks:");
            for (hook, backup) in &self.restored {
                println!("  {hook} ← {backup}");
            }
        }

        if !self.errors.is_empty() {
            println!("❌ Errors:");
            for (hook, error) in &self.errors {
                println!("  {hook}: {error}");
            }
        }

        let total_operations = self.removed.len() + self.restored.len();
        if total_operations > 0 {
            println!("\n✅ Successfully processed {total_operations} git hooks.");
        }
    }

    /// Check if the uninstallation was completely successful
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository as Git2Repository;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_repo_with_config(
        temp_dir: &Path,
        config_content: &str,
    ) -> (GitRepository, PathBuf) {
        // Initialize a real git repository for tests
        let _ = Git2Repository::init(temp_dir).unwrap();
        let git_dir = temp_dir.join(".git");
        let hooks_dir = git_dir.join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        let config_path = temp_dir.join("hooks.toml");
        std::fs::write(&config_path, config_content).unwrap();

        let repo = GitRepository::find_from_dir(temp_dir).unwrap();
        (repo, config_path)
    }

    #[test]
    fn test_installer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let (repo, _) =
            create_test_repo_with_config(temp_dir.path(), "[hooks.test]\ncommand = 'echo test'\n");

        let installer =
            GitHookInstaller::with_repository_and_binary(repo, "test-binary".to_string());

        assert_eq!(installer.binary_path, "test-binary");
    }

    #[test]
    fn test_hook_script_generation() {
        let temp_dir = TempDir::new().unwrap();
        let (repo, _) = create_test_repo_with_config(temp_dir.path(), "");

        let installer = GitHookInstaller::with_repository_and_binary(
            repo,
            "/usr/local/bin/peter-hook".to_string(),
        );

        let script = installer.generate_hook_script("pre-commit");

        assert!(script.contains("#!/bin/sh"));
        assert!(script.contains("# Generated by peter-hook"));
        assert!(script.contains("exec \"/usr/local/bin/peter-hook\" run pre-commit"));
    }

    #[test]
    fn test_install_with_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[hooks.pre-commit]
command = "echo 'pre-commit hook'"
modifies_repository = false
"#;

        let (repo, _) = create_test_repo_with_config(temp_dir.path(), config_content);
        let installer =
            GitHookInstaller::with_repository_and_binary(repo.clone(), "peter-hook".to_string());

        let resolver = HookResolver::new(&repo.root);
        let action = installer.install_hook("pre-commit", &resolver).unwrap();

        match action {
            InstallAction::Installed => {
                assert!(repo.hook_exists("pre-commit"));
                let info = repo.get_hook_info("pre-commit").unwrap().unwrap();
                assert!(info.is_managed);
                assert!(info.is_executable);
            }
            _ => panic!("Expected hook to be installed"),
        }
    }

    #[test]
    fn test_backup_existing_hook() {
        let temp_dir = TempDir::new().unwrap();
        let (repo, _) = create_test_repo_with_config(
            temp_dir.path(),
            "[hooks.pre-commit]\ncommand = 'echo test'\n",
        );

        // Create existing hook
        let existing_content = "#!/bin/sh\necho 'existing hook'\n";
        std::fs::write(repo.hook_path("pre-commit"), existing_content).unwrap();

        let installer =
            GitHookInstaller::with_repository_and_binary(repo.clone(), "peter-hook".to_string());

        let resolver = HookResolver::new(&repo.root);
        let action = installer.install_hook("pre-commit", &resolver).unwrap();

        match action {
            InstallAction::BackedUp(backup_path) => {
                // Original hook should be backed up
                assert!(Path::new(&backup_path).exists());
                let backup_content = std::fs::read_to_string(&backup_path).unwrap();
                assert_eq!(backup_content, existing_content);

                // New hook should be installed
                let info = repo.get_hook_info("pre-commit").unwrap().unwrap();
                assert!(info.is_managed);
            }
            _ => panic!("Expected existing hook to be backed up"),
        }
    }

    #[test]
    fn test_install_with_placeholder_group() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[groups.pre-commit]
includes = []
placeholder = true
description = "Placeholder for hierarchical resolution"
"#;

        let (repo, _) = create_test_repo_with_config(temp_dir.path(), config_content);
        let installer =
            GitHookInstaller::with_repository_and_binary(repo.clone(), "peter-hook".to_string());

        let resolver = HookResolver::new(&repo.root);
        let action = installer.install_hook("pre-commit", &resolver).unwrap();

        // Placeholder groups should trigger installation
        match action {
            InstallAction::Installed => {
                assert!(repo.hook_exists("pre-commit"));
                let info = repo.get_hook_info("pre-commit").unwrap().unwrap();
                assert!(info.is_managed);
                assert!(info.is_executable);
            }
            _ => panic!("Expected hook to be installed with placeholder group"),
        }
    }

    #[test]
    fn test_install_placeholder_creates_hook_script() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[groups.pre-push]
includes = []
placeholder = true
"#;

        let (repo, _) = create_test_repo_with_config(temp_dir.path(), config_content);
        let installer =
            GitHookInstaller::with_repository_and_binary(repo.clone(), "peter-hook".to_string());

        let resolver = HookResolver::new(&repo.root);
        installer.install_hook("pre-push", &resolver).unwrap();

        // Verify hook script exists and is executable
        let hook_path = repo.hook_path("pre-push");
        assert!(hook_path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::metadata(&hook_path).unwrap().permissions();
            assert_eq!(permissions.mode() & 0o111, 0o111); // Executable bits set
        }

        // Verify script content
        let content = std::fs::read_to_string(&hook_path).unwrap();
        assert!(content.contains("# Generated by peter-hook"));
        assert!(content.contains("exec \"peter-hook\" run pre-push"));
    }

    #[test]
    fn test_install_skips_non_placeholder_empty() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[groups.empty-group]
includes = []
"#;

        let (repo, _) = create_test_repo_with_config(temp_dir.path(), config_content);
        let installer =
            GitHookInstaller::with_repository_and_binary(repo.clone(), "peter-hook".to_string());

        let resolver = HookResolver::new(&repo.root);
        let action = installer.install_hook("empty-group", &resolver).unwrap();

        // Empty groups without placeholder should be skipped (backward compatibility)
        match action {
            InstallAction::Skipped(_) => {
                assert!(!repo.hook_exists("empty-group"));
            }
            _ => panic!("Expected empty group without placeholder to be skipped"),
        }
    }
}
