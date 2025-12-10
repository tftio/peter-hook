//! Health check and diagnostics module.

use crate::{HookConfig, git::GitRepository, hooks::HookResolver};
use workhelix_cli_common::{DoctorCheck, DoctorChecks, RepoInfo};

/// Peter-hook doctor checks implementation.
pub struct PeterHookDoctor;

impl DoctorChecks for PeterHookDoctor {
    fn repo_info() -> RepoInfo {
        RepoInfo::new("tftio", "peter-hook")
    }

    fn current_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn tool_checks(&self) -> Vec<DoctorCheck> {
        let mut checks = Vec::new();

        // Git repository checks
        checks.extend(check_git_repository());

        // Configuration checks
        checks.extend(check_configuration());

        checks
    }
}

/// Run doctor command to check health and configuration.
///
/// Returns exit code: 0 if healthy, 1 if issues found.
#[must_use]
pub fn run_doctor() -> i32 {
    let doctor = PeterHookDoctor;
    workhelix_cli_common::run_doctor(&doctor)
}

fn check_git_repository() -> Vec<DoctorCheck> {
    let mut checks = Vec::new();

    match GitRepository::find_from_current_dir() {
        Ok(repo) => {
            checks.push(DoctorCheck::pass("Git repository found"));

            // Check hooks
            match repo.list_hooks() {
                Ok(hooks) => {
                    if hooks.is_empty() {
                        checks.push(DoctorCheck::fail(
                            "Git hooks",
                            "No git hooks installed - run 'peter-hook install' to install hooks",
                        ));
                    } else {
                        // Check if managed by peter-hook
                        let mut managed_count = 0;
                        for hook_name in &hooks {
                            if let Ok(Some(info)) = repo.get_hook_info(hook_name) {
                                if info.is_managed {
                                    managed_count += 1;
                                }
                            }
                        }

                        if managed_count == 0 {
                            checks.push(DoctorCheck::fail(
                                format!("{} git hook(s) found", hooks.len()),
                                "No hooks managed by peter-hook - run 'peter-hook install' to \
                                 install hooks",
                            ));
                        } else {
                            checks.push(DoctorCheck::pass(format!(
                                "{managed_count} hook(s) managed by peter-hook"
                            )));
                        }
                    }
                }
                Err(e) => {
                    checks.push(DoctorCheck::fail("List git hooks", format!("Failed: {e}")));
                }
            }
        }
        Err(e) => {
            checks.push(DoctorCheck::fail(
                "Git repository",
                format!("Not in a git repository: {e}"),
            ));
        }
    }

    checks
}

fn check_configuration() -> Vec<DoctorCheck> {
    let mut checks = Vec::new();
    let resolver = HookResolver::new(std::env::current_dir().unwrap_or_default());

    match resolver.find_config_file() {
        Ok(Some(config_path)) => {
            checks.push(DoctorCheck::pass(format!(
                "Config file found: {}",
                config_path.display()
            )));

            // Try to parse it
            match HookConfig::from_file(&config_path) {
                Ok(config) => {
                    let hook_names = config.get_hook_names();
                    if hook_names.is_empty() {
                        checks.push(DoctorCheck::fail(
                            "Hook configuration",
                            "No hooks or groups defined in .peter-hook.toml",
                        ));
                    } else {
                        checks.push(DoctorCheck::pass(format!(
                            "Found {} hook(s)/group(s)",
                            hook_names.len()
                        )));
                    }
                }
                Err(e) => {
                    checks.push(DoctorCheck::fail(
                        "Config validation",
                        format!("Invalid: {e}"),
                    ));
                }
            }
        }
        Ok(None) => {
            checks.push(DoctorCheck::fail(
                "Configuration file",
                "No .peter-hook.toml found - create one to configure peter-hook",
            ));
        }
        Err(e) => {
            checks.push(DoctorCheck::fail("Config search", format!("Failed: {e}")));
        }
    }

    checks
}
