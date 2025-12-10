//! Hook execution engine

use crate::{
    config::{ExecutionStrategy, ExecutionType, HookCommand, TemplateResolver},
    git::FilePatternMatcher,
    hooks::{DependencyResolver, ResolvedHook, ResolvedHooks},
    output::formatter,
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

/// Executes resolved hooks
pub struct HookExecutor {
    /// Whether to run hooks in parallel when possible
    #[allow(dead_code)]
    parallel: bool,
}

/// Result of hook execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Exit code of the hook
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Whether the hook succeeded (exit code 0)
    pub success: bool,
}

/// Results from executing multiple hooks
#[derive(Debug, Clone)]
pub struct ExecutionResults {
    /// Results for each hook by name
    pub results: HashMap<String, ExecutionResult>,
    /// Overall success (all hooks succeeded)
    pub success: bool,
}

impl HookExecutor {
    /// Create a new hook executor
    #[must_use]
    pub const fn new() -> Self {
        Self { parallel: false }
    }

    /// Create a new hook executor with parallel execution enabled
    #[must_use]
    pub const fn with_parallel() -> Self {
        Self { parallel: true }
    }

    /// Execute multiple configuration groups (for hierarchical resolution)
    ///
    /// This executes hooks from multiple configurations, each in their own
    /// directory. Groups are executed sequentially with fail-fast semantics:
    ///
    /// 1. Groups are processed in order
    /// 2. Each group executes its hooks according to its execution strategy
    /// 3. **On failure**: Execution stops immediately; remaining groups are
    ///    skipped
    /// 4. **On success**: Proceeds to the next group
    ///
    /// This follows traditional git hook behavior where any failure blocks the
    /// git operation (commit, push, etc.). Failed groups do NOT roll back or
    /// undo previous successful groups.
    ///
    /// All results are aggregated into a single `ExecutionResults` with hook
    /// names prefixed by config path when multiple configs are involved.
    ///
    /// # Errors
    ///
    /// Returns an error if any hook fails to execute due to system issues
    /// (e.g., command not found, permission denied)
    pub fn execute_multiple(groups: &[crate::hooks::ConfigGroup]) -> Result<ExecutionResults> {
        let mut all_results = HashMap::new();
        let mut overall_success = true;

        for group in groups {
            let results = Self::execute(&group.resolved_hooks).with_context(|| {
                format!(
                    "Failed to execute hooks from config: {}",
                    group.config_path.display()
                )
            })?;

            if !results.success {
                overall_success = false;
            }

            // Merge results (prefix hook names with config path for uniqueness if needed)
            for (name, result) in results.results {
                // If we have multiple configs, prefix the hook name to avoid collisions
                let unique_name = if groups.len() > 1 {
                    format!("{}:{}", group.config_path.display(), name)
                } else {
                    name
                };
                all_results.insert(unique_name, result);
            }

            // Stop on first failure (traditional git hook behavior)
            if !results.success {
                break;
            }
        }

        Ok(ExecutionResults {
            results: all_results,
            success: overall_success,
        })
    }

    /// Execute all resolved hooks using their configured execution strategy
    ///
    /// # Errors
    ///
    /// Returns an error if any hook fails to execute due to system issues
    /// (not hook failure - that's reported in the results)
    pub fn execute(resolved_hooks: &ResolvedHooks) -> Result<ExecutionResults> {
        // Check if we need dependency resolution
        let needs_dependencies = resolved_hooks
            .hooks
            .values()
            .any(|hook| hook.definition.depends_on.is_some());

        if needs_dependencies {
            Self::execute_with_dependencies(resolved_hooks)
        } else {
            Self::execute_with_strategy(resolved_hooks, resolved_hooks.execution_strategy)
        }
    }

    /// Execute hooks with a specific execution strategy
    ///
    /// # Errors
    ///
    /// Returns an error if any hook fails to execute due to system issues
    pub fn execute_with_strategy(
        resolved_hooks: &ResolvedHooks,
        strategy: ExecutionStrategy,
    ) -> Result<ExecutionResults> {
        match strategy {
            ExecutionStrategy::Sequential => Self::execute_sequential(resolved_hooks),
            ExecutionStrategy::Parallel => Self::execute_parallel_safe(resolved_hooks),
            ExecutionStrategy::ForceParallel => Ok(Self::execute_parallel_unsafe(resolved_hooks)),
        }
    }

    /// Execute hooks sequentially (original behavior)
    fn execute_sequential(resolved_hooks: &ResolvedHooks) -> Result<ExecutionResults> {
        let mut results = HashMap::new();
        let mut overall_success = true;

        for (name, hook) in &resolved_hooks.hooks {
            let result = Self::execute_single_hook(
                name,
                hook,
                &resolved_hooks.worktree_context,
                resolved_hooks.changed_files.as_deref(),
            )
            .with_context(|| format!("Failed to execute hook: {name}"))?;

            if !result.success {
                overall_success = false;
            }

            results.insert(name.clone(), result);
        }

        Ok(ExecutionResults {
            results,
            success: overall_success,
        })
    }

    /// Execute hooks in parallel, respecting repository modification safety
    fn execute_parallel_safe(resolved_hooks: &ResolvedHooks) -> Result<ExecutionResults> {
        // Separate hooks into safe-to-parallelize and repository-modifying
        let mut safe_hooks = Vec::new();
        let mut modifying_hooks = Vec::new();

        for (name, hook) in &resolved_hooks.hooks {
            if hook.definition.modifies_repository {
                modifying_hooks.push((name.clone(), hook));
            } else {
                safe_hooks.push((name.clone(), hook));
            }
        }

        let results = Arc::new(Mutex::new(HashMap::new()));
        let overall_success = Arc::new(Mutex::new(true));

        // First, run all safe hooks in parallel
        if !safe_hooks.is_empty() {
            let mut handles = Vec::new();

            for (name, hook) in safe_hooks {
                let name = name.clone();
                let hook = hook.clone();
                let results = Arc::clone(&results);
                let overall_success = Arc::clone(&overall_success);

                let worktree_context = resolved_hooks.worktree_context.clone();
                let changed_files = resolved_hooks.changed_files.clone();
                let handle = thread::spawn(move || {
                    match Self::execute_single_hook(
                        &name,
                        &hook,
                        &worktree_context,
                        changed_files.as_deref(),
                    ) {
                        Ok(result) => {
                            let success = result.success;
                            results.lock().unwrap().insert(name, result);
                            if !success {
                                *overall_success.lock().unwrap() = false;
                            }
                        }
                        Err(e) => {
                            // Create a failed result for execution errors
                            let result = ExecutionResult {
                                exit_code: -1,
                                stdout: String::new(),
                                stderr: format!("Execution error: {e:#}"),
                                success: false,
                            };
                            results.lock().unwrap().insert(name, result);
                            *overall_success.lock().unwrap() = false;
                        }
                    }
                });
                handles.push(handle);
            }

            // Wait for all parallel hooks to complete
            for handle in handles {
                if handle.join().is_err() {
                    *overall_success.lock().unwrap() = false;
                }
            }
        }

        // Then, run repository-modifying hooks sequentially
        for (name, hook) in modifying_hooks {
            let result = Self::execute_single_hook(
                &name,
                hook,
                &resolved_hooks.worktree_context,
                resolved_hooks.changed_files.as_deref(),
            )
            .with_context(|| format!("Failed to execute hook: {name}"))?;

            if !result.success {
                *overall_success.lock().unwrap() = false;
            }

            results.lock().unwrap().insert(name.clone(), result);
        }

        let results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
        let overall_success = Arc::try_unwrap(overall_success)
            .unwrap()
            .into_inner()
            .unwrap();

        Ok(ExecutionResults {
            results,
            success: overall_success,
        })
    }

    /// Execute all hooks in parallel (unsafe - ignores repository modification)
    fn execute_parallel_unsafe(resolved_hooks: &ResolvedHooks) -> ExecutionResults {
        let results = Arc::new(Mutex::new(HashMap::new()));
        let overall_success = Arc::new(Mutex::new(true));
        let mut handles = Vec::new();

        for (name, hook) in &resolved_hooks.hooks {
            let name = name.clone();
            let hook = hook.clone();
            let results = Arc::clone(&results);
            let overall_success = Arc::clone(&overall_success);

            let worktree_context = resolved_hooks.worktree_context.clone();
            let changed_files = resolved_hooks.changed_files.clone();
            let handle = thread::spawn(move || {
                match Self::execute_single_hook(
                    &name,
                    &hook,
                    &worktree_context,
                    changed_files.as_deref(),
                ) {
                    Ok(result) => {
                        let success = result.success;
                        results.lock().unwrap().insert(name, result);
                        if !success {
                            *overall_success.lock().unwrap() = false;
                        }
                    }
                    Err(e) => {
                        let result = ExecutionResult {
                            exit_code: -1,
                            stdout: String::new(),
                            stderr: format!("Execution error: {e:#}"),
                            success: false,
                        };
                        results.lock().unwrap().insert(name, result);
                        *overall_success.lock().unwrap() = false;
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all hooks to complete
        for handle in handles {
            if handle.join().is_err() {
                *overall_success.lock().unwrap() = false;
            }
        }

        let results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
        let overall_success = Arc::try_unwrap(overall_success)
            .unwrap()
            .into_inner()
            .unwrap();

        ExecutionResults {
            results,
            success: overall_success,
        }
    }

    /// Execute hooks respecting dependencies
    fn execute_with_dependencies(resolved_hooks: &ResolvedHooks) -> Result<ExecutionResults> {
        let mut resolver = DependencyResolver::new();
        let hook_names: Vec<String> = resolved_hooks.hooks.keys().cloned().collect();

        // Build dependency graph
        for (name, hook) in &resolved_hooks.hooks {
            let dependencies = hook.definition.depends_on.clone().unwrap_or_default();
            resolver.add_hook(name.clone(), dependencies);
        }

        // Resolve execution plan
        let plan = resolver
            .resolve(&hook_names)
            .context("Failed to resolve hook dependencies")?;

        let mut all_results = HashMap::new();
        let mut overall_success = true;

        // Execute hooks phase by phase
        for phase in &plan.phases {
            let mut phase_results = HashMap::new();

            if phase.parallel && phase.hooks.len() > 1 {
                // Execute phase hooks in parallel
                let results = Arc::new(Mutex::new(HashMap::new()));
                let phase_success = Arc::new(Mutex::new(true));
                let mut handles = Vec::new();

                for hook_name in &phase.hooks {
                    let hook = &resolved_hooks.hooks[hook_name];
                    let name = hook_name.clone();
                    let hook = hook.clone();
                    let results = Arc::clone(&results);
                    let phase_success = Arc::clone(&phase_success);

                    let worktree_context = resolved_hooks.worktree_context.clone();
                    let changed_files = resolved_hooks.changed_files.clone();
                    let handle = thread::spawn(move || {
                        match Self::execute_single_hook(
                            &name,
                            &hook,
                            &worktree_context,
                            changed_files.as_deref(),
                        ) {
                            Ok(result) => {
                                let success = result.success;
                                results.lock().unwrap().insert(name, result);
                                if !success {
                                    *phase_success.lock().unwrap() = false;
                                }
                            }
                            Err(e) => {
                                let result = ExecutionResult {
                                    exit_code: -1,
                                    stdout: String::new(),
                                    stderr: format!("Execution error: {e:#}"),
                                    success: false,
                                };
                                results.lock().unwrap().insert(name, result);
                                *phase_success.lock().unwrap() = false;
                            }
                        }
                    });
                    handles.push(handle);
                }

                // Wait for all hooks in this phase
                for handle in handles {
                    if handle.join().is_err() {
                        *phase_success.lock().unwrap() = false;
                    }
                }

                phase_results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
                let phase_success = Arc::try_unwrap(phase_success)
                    .unwrap()
                    .into_inner()
                    .unwrap();

                if !phase_success {
                    overall_success = false;
                    // Stop execution if any hook in this phase failed
                    all_results.extend(phase_results);
                    break;
                }
            } else {
                // Execute phase hooks sequentially
                for hook_name in &phase.hooks {
                    let hook = &resolved_hooks.hooks[hook_name];
                    let result = Self::execute_single_hook(
                        hook_name,
                        hook,
                        &resolved_hooks.worktree_context,
                        resolved_hooks.changed_files.as_deref(),
                    )
                    .with_context(|| format!("Failed to execute hook: {hook_name}"))?;

                    let success = result.success;
                    phase_results.insert(hook_name.clone(), result);

                    if !success {
                        // Stop execution if hook failed
                        all_results.extend(phase_results);
                        return Ok(ExecutionResults {
                            results: all_results,
                            success: false,
                        });
                    }
                }
            }

            all_results.extend(phase_results);
        }

        Ok(ExecutionResults {
            results: all_results,
            success: overall_success,
        })
    }

    /// Execute a single hook
    #[allow(clippy::too_many_lines, clippy::option_if_let_else)]
    fn execute_single_hook(
        name: &str,
        hook: &ResolvedHook,
        worktree_context: &crate::hooks::resolver::WorktreeContext,
        changed_files: Option<&[PathBuf]>,
    ) -> Result<ExecutionResult> {
        match hook.definition.execution_type {
            ExecutionType::PerFile => {
                Self::execute_per_file_hook(name, hook, worktree_context, changed_files)
            }
            ExecutionType::InPlace => {
                Self::execute_in_place_hook(name, hook, worktree_context, changed_files)
            }
            ExecutionType::Other => {
                Self::execute_other_hook(name, hook, worktree_context, changed_files)
            }
        }
    }

    /// Execute hook with files passed as individual arguments (per-file mode)
    fn execute_per_file_hook(
        name: &str,
        hook: &ResolvedHook,
        worktree_context: &crate::hooks::resolver::WorktreeContext,
        changed_files: Option<&[PathBuf]>,
    ) -> Result<ExecutionResult> {
        // Get relevant changed files based on hook's file patterns
        let relevant_changed = Self::filter_relevant_files(hook, changed_files);

        // Skip execution if no files match (whether pattern specified or not)
        if relevant_changed.is_empty() && !hook.definition.run_always {
            return Ok(ExecutionResult {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                success: true,
            });
        }

        // Determine execution directory (same logic as execute_command_parts)
        let execution_dir = if hook.definition.run_at_root {
            &worktree_context.repo_root
        } else {
            &hook.working_directory
        };

        // Transform file paths from repo-relative to execution-directory-relative
        let transformed_files = Self::transform_file_paths(
            &relevant_changed,
            &worktree_context.repo_root,
            execution_dir,
        );

        // Build base command without template resolution (per-file doesn't use
        // {CHANGED_FILES})
        let config_dir = hook
            .source_file
            .parent()
            .context("Hook source file has no parent directory")?;
        let template_resolver = TemplateResolver::with_worktree_context(
            config_dir,
            &hook.working_directory,
            worktree_context,
        );

        let mut base_command_parts = match &hook.definition.command {
            HookCommand::Shell(cmd) => {
                let resolved_cmd = template_resolver
                    .resolve_string(cmd)
                    .context("Failed to resolve command template")?;
                vec!["sh".to_string(), "-c".to_string(), resolved_cmd]
            }
            HookCommand::Args(args) => {
                if args.is_empty() {
                    return Err(anyhow::anyhow!("Empty command for hook: {name}"));
                }
                template_resolver
                    .resolve_command_args(args)
                    .context("Failed to resolve command arguments")?
            }
        };

        // Add transformed files as individual arguments
        for file in &transformed_files {
            base_command_parts.push(file.to_string_lossy().to_string());
        }

        // Execute the command with file arguments
        Self::execute_command_parts(name, hook, worktree_context, &base_command_parts)
    }

    /// Execute hook once in config directory without file arguments (in-place
    /// mode)
    fn execute_in_place_hook(
        name: &str,
        hook: &ResolvedHook,
        worktree_context: &crate::hooks::resolver::WorktreeContext,
        changed_files: Option<&[PathBuf]>,
    ) -> Result<ExecutionResult> {
        // Get relevant changed files for filtering check
        let relevant_changed = Self::filter_relevant_files(hook, changed_files);

        // Skip execution if no files match (whether pattern specified or not)
        if relevant_changed.is_empty() && !hook.definition.run_always {
            return Ok(ExecutionResult {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                success: true,
            });
        }

        // Build command without file arguments for in-place execution
        let config_dir = hook
            .source_file
            .parent()
            .context("Hook source file has no parent directory")?;
        let template_resolver = TemplateResolver::with_worktree_context(
            config_dir,
            &hook.working_directory,
            worktree_context,
        );

        let command_parts = match &hook.definition.command {
            HookCommand::Shell(cmd) => {
                let resolved_cmd = template_resolver
                    .resolve_string(cmd)
                    .context("Failed to resolve command template")?;
                vec!["sh".to_string(), "-c".to_string(), resolved_cmd]
            }
            HookCommand::Args(args) => {
                if args.is_empty() {
                    return Err(anyhow::anyhow!("Empty command for hook: {name}"));
                }
                template_resolver
                    .resolve_command_args(args)
                    .context("Failed to resolve command arguments")?
            }
        };

        // Execute once in the config directory (or custom workdir)
        Self::execute_command_parts(name, hook, worktree_context, &command_parts)
    }

    /// Execute hook using template variables (other/manual mode) - original
    /// behavior
    fn execute_other_hook(
        name: &str,
        hook: &ResolvedHook,
        worktree_context: &crate::hooks::resolver::WorktreeContext,
        changed_files: Option<&[PathBuf]>,
    ) -> Result<ExecutionResult> {
        // This is the original implementation - delegate to the original logic
        Self::execute_original_hook(name, hook, worktree_context, changed_files)
    }

    /// Filter files based on hook's file patterns
    fn filter_relevant_files(
        hook: &ResolvedHook,
        changed_files: Option<&[PathBuf]>,
    ) -> Vec<PathBuf> {
        let Some(cf) = changed_files else {
            return Vec::new();
        };

        hook.definition.files.as_ref().map_or_else(
            || cf.to_vec(),
            |patterns| {
                FilePatternMatcher::new(patterns).map_or_else(
                    |_| cf.to_vec(),
                    |matcher| cf.iter().filter(|p| matcher.matches(p)).cloned().collect(),
                )
            },
        )
    }

    /// Transform file paths from repo-relative to execution-directory-relative
    ///
    /// When hooks run from a subdirectory config, Git provides paths relative
    /// to repo root, but the hook needs paths relative to its execution
    /// directory.
    ///
    /// Example:
    /// - Repo root: /repo
    /// - Execution dir: /repo/projects/backend
    /// - Git path: projects/backend/test.py
    /// - Transformed: test.py
    fn transform_file_paths(
        files: &[PathBuf],
        repo_root: &Path,
        execution_dir: &Path,
    ) -> Vec<PathBuf> {
        files
            .iter()
            .map(|file| {
                // Make the file path absolute by joining with repo root
                let abs_file = repo_root.join(file);

                // Try to strip the execution directory prefix to get relative path
                // If execution_dir is not a prefix of abs_file, keep the original path
                abs_file
                    .strip_prefix(execution_dir)
                    .map_or_else(|_| file.clone(), std::path::Path::to_path_buf)
            })
            .collect()
    }

    /// Execute command parts with proper setup
    #[allow(clippy::too_many_lines)]
    fn execute_command_parts(
        name: &str,
        hook: &ResolvedHook,
        worktree_context: &crate::hooks::resolver::WorktreeContext,
        command_parts: &[String],
    ) -> Result<ExecutionResult> {
        use std::io::Read;
        use wait_timeout::ChildExt;

        if command_parts.is_empty() {
            return Err(anyhow::anyhow!("Empty command for hook: {name}"));
        }

        let config_dir = hook
            .source_file
            .parent()
            .context("Hook source file has no parent directory")?;
        let template_resolver = TemplateResolver::with_worktree_context(
            config_dir,
            &hook.working_directory,
            worktree_context,
        );

        // Build command
        let mut command = Command::new(&command_parts[0]);
        if command_parts.len() > 1 {
            command.args(&command_parts[1..]);
        }

        // Set working directory
        let working_dir = if let Some(workdir_template) = &hook.definition.workdir {
            let resolved_workdir = template_resolver
                .resolve_string(workdir_template)
                .context("Failed to resolve workdir template")?;
            PathBuf::from(resolved_workdir)
        } else if hook.definition.run_at_root {
            // If run_at_root is true, use the repository root
            worktree_context.repo_root.clone()
        } else {
            hook.working_directory.clone()
        };
        command.current_dir(&working_dir);

        // Set environment variables
        if let Some(env) = &hook.definition.env {
            let resolved_env = template_resolver
                .resolve_env(env)
                .context("Failed to resolve environment variable templates")?;
            for (key, value) in resolved_env {
                command.env(key, value);
            }
        }

        // Configure stdio
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        // Debug output
        if crate::debug::is_enabled() {
            if std::io::stderr().is_terminal() {
                eprintln!(
                    "\x1b[38;5;220m⚡ \x1b[1m\x1b[38;5;196mEXECUTING:\x1b[0m \
                     \x1b[38;5;226m{name}\x1b[0m"
                );
                eprintln!("\x1b[38;5;75m🎬 Command: \x1b[38;5;155m{command_parts:?}\x1b[0m");
            } else {
                eprintln!("[DEBUG] Executing hook: {name}");
                eprintln!("[DEBUG] Command: {command_parts:?}");
            }
        }

        // Execute command with timeout
        let timeout = std::time::Duration::from_secs(hook.definition.timeout_seconds);
        let mut child = command
            .spawn()
            .with_context(|| format!("Failed to spawn hook command: {name}"))?;

        // Take stdout and stderr handles before waiting
        let mut stdout_handle = child.stdout.take().context("Failed to capture stdout")?;
        let mut stderr_handle = child.stderr.take().context("Failed to capture stderr")?;

        // Spawn threads to read stdout and stderr in parallel
        // This prevents deadlocks from full pipe buffers
        let stdout_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            stdout_handle.read_to_end(&mut buf).ok();
            buf
        });

        let stderr_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            stderr_handle.read_to_end(&mut buf).ok();
            buf
        });

        // Wait for the command with timeout
        let status_option = child
            .wait_timeout(timeout)
            .with_context(|| format!("Failed to wait for hook command: {name}"))?;

        let (exit_code, stdout, stderr, success) = if let Some(status) = status_option {
            // Process finished within timeout - collect output from threads
            let stdout_buf = stdout_thread.join().unwrap_or_default();
            let stderr_buf = stderr_thread.join().unwrap_or_default();

            let stdout = String::from_utf8_lossy(&stdout_buf).to_string();
            let stderr = String::from_utf8_lossy(&stderr_buf).to_string();
            let exit_code = status.code().unwrap_or(-1);
            let success = status.success();

            (exit_code, stdout, stderr, success)
        } else {
            // Timeout occurred - kill the process
            let _ = child.kill();
            let _ = child.wait(); // Reap the process

            // Still try to collect partial output
            let stdout_buf = stdout_thread.join().unwrap_or_default();
            let stderr_buf = stderr_thread.join().unwrap_or_default();
            let stdout = String::from_utf8_lossy(&stdout_buf);
            let stderr = String::from_utf8_lossy(&stderr_buf);

            return Err(anyhow::anyhow!(
                "Hook '{name}' exceeded timeout of {} seconds and was killed\nPartial stdout: \
                 {stdout}\nPartial stderr: {stderr}",
                hook.definition.timeout_seconds
            ));
        };

        // Debug output for result
        if crate::debug::is_enabled() && std::io::stderr().is_terminal() {
            if success {
                eprintln!(
                    "\x1b[38;5;46m🎉 \x1b[1m\x1b[38;5;82mSUCCESS:\x1b[0m \
                     \x1b[38;5;226m{name}\x1b[0m"
                );
            } else {
                eprintln!(
                    "\x1b[38;5;196m💥 \x1b[1m\x1b[38;5;199mFAILED:\x1b[0m \
                     \x1b[38;5;226m{name}\x1b[0m"
                );
            }
        }

        Ok(ExecutionResult {
            exit_code,
            stdout,
            stderr,
            success,
        })
    }

    /// Create temporary file for changed files list
    fn create_changed_files_temp_file(relevant_changed: &[PathBuf]) -> Option<PathBuf> {
        if relevant_changed.is_empty() {
            None
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let tmp_path = std::env::temp_dir().join(format!(
                "peter-hook-changed-{}-{}.lst",
                std::process::id(),
                now
            ));
            let changed_list = relevant_changed
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n");

            if std::fs::write(&tmp_path, &changed_list).is_ok() {
                Some(tmp_path)
            } else {
                None
            }
        }
    }

    /// Print debug output for changed files
    fn print_changed_files_debug(name: &str, relevant_changed: &[PathBuf]) {
        if crate::debug::is_enabled() {
            if std::io::stderr().is_terminal() {
                eprintln!(
                    "\x1b[38;5;200m🎯 \x1b[1m\x1b[38;5;51mExecuting hook:\x1b[0m \
                     \x1b[38;5;226m{name}\x1b[0m"
                );
                eprintln!(
                    "\x1b[38;5;75m  🎪 Files matching patterns: \x1b[38;5;118m{}\x1b[0m",
                    relevant_changed.len()
                );
                for (i, file) in relevant_changed.iter().enumerate() {
                    let emoji = match i % 4 {
                        0 => "📄",
                        1 => "📝",
                        2 => "📊",
                        _ => "🔧",
                    };
                    eprintln!(
                        "\x1b[38;5;147m    {emoji} \x1b[38;5;183m{}\x1b[0m",
                        file.display()
                    );
                }
            } else {
                eprintln!("[DEBUG] Executing hook: {name}");
                eprintln!(
                    "[DEBUG]   Files matching patterns: {}",
                    relevant_changed.len()
                );
                for file in relevant_changed {
                    eprintln!("[DEBUG]     {}", file.display());
                }
            }
        }
    }

    /// Build command from hook definition with template resolution
    fn build_command_from_hook(
        hook: &ResolvedHook,
        template_resolver: &TemplateResolver,
        name: &str,
        worktree_context: &crate::hooks::resolver::WorktreeContext,
    ) -> Result<Command> {
        let mut command = match &hook.definition.command {
            HookCommand::Shell(cmd) => {
                let resolved_cmd = template_resolver
                    .resolve_string(cmd)
                    .context("Failed to resolve command template")?;

                if crate::debug::is_enabled() {
                    if std::io::stderr().is_terminal() {
                        eprintln!(
                            "\x1b[38;5;208m🧙‍♂️ \x1b[1m\x1b[38;5;198mShell command resolved:\x1b[0m"
                        );
                        eprintln!("\x1b[38;5;141m  🔮 Original: \x1b[38;5;87m{cmd}\x1b[0m");
                        eprintln!(
                            "\x1b[38;5;141m  ✨ Resolved: \x1b[38;5;155m{resolved_cmd}\x1b[0m"
                        );
                    } else {
                        eprintln!("[DEBUG] Shell command resolved:");
                        eprintln!("[DEBUG]   Original: {cmd}");
                        eprintln!("[DEBUG]   Resolved: {resolved_cmd}");
                    }
                }

                let mut command = Command::new("sh");
                command.args(["-c", &resolved_cmd]);
                command
            }
            HookCommand::Args(args) => {
                if args.is_empty() {
                    return Err(anyhow::anyhow!("Empty command for hook: {name}"));
                }
                let resolved_args = template_resolver
                    .resolve_command_args(args)
                    .context("Failed to resolve command arguments")?;

                if crate::debug::is_enabled() {
                    if std::io::stderr().is_terminal() {
                        eprintln!(
                            "\x1b[38;5;165m🚀 \x1b[1m\x1b[38;5;51mArgs command resolved:\x1b[0m"
                        );
                        eprintln!("\x1b[38;5;141m  🎭 Original: \x1b[38;5;87m{args:?}\x1b[0m");
                        eprintln!(
                            "\x1b[38;5;141m  🎨 Resolved: \x1b[38;5;155m{resolved_args:?}\x1b[0m"
                        );

                        // Rainbow command display
                        let colors = [196, 208, 226, 118, 51, 99, 201];
                        eprint!("\x1b[38;5;141m  🌈 Command: ");
                        for (i, arg) in resolved_args.iter().enumerate() {
                            let color = colors[i % colors.len()];
                            eprint!("\x1b[38;5;{color}m{arg}\x1b[0m ");
                        }
                        eprintln!();
                    } else {
                        eprintln!("[DEBUG] Args command resolved:");
                        eprintln!("[DEBUG]   Original: {args:?}");
                        eprintln!("[DEBUG]   Resolved: {resolved_args:?}");
                    }
                }

                let mut command = Command::new(&resolved_args[0]);
                if resolved_args.len() > 1 {
                    command.args(&resolved_args[1..]);
                }
                command
            }
        };

        // Set working directory (resolve template if needed)
        let working_dir = if let Some(workdir_template) = &hook.definition.workdir {
            let resolved_workdir = template_resolver
                .resolve_string(workdir_template)
                .context("Failed to resolve workdir template")?;
            PathBuf::from(resolved_workdir)
        } else if hook.definition.run_at_root {
            // If run_at_root is true, use the repository root
            worktree_context.repo_root.clone()
        } else {
            hook.working_directory.clone()
        };
        command.current_dir(&working_dir);

        // Set environment variables with template resolution
        if let Some(env) = &hook.definition.env {
            let resolved_env = template_resolver
                .resolve_env(env)
                .context("Failed to resolve environment variable templates")?;
            for (key, value) in resolved_env {
                command.env(key, value);
            }
        }

        // Configure stdio
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        Ok(command)
    }

    /// Print debug output for execution results
    fn print_execution_debug_output(
        name: &str,
        success: bool,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
    ) {
        if crate::debug::is_enabled() {
            if std::io::stderr().is_terminal() {
                if success {
                    eprintln!(
                        "\x1b[38;5;46m🎉 \x1b[1m\x1b[38;5;82mHook SUCCESS:\x1b[0m \
                         \x1b[38;5;226m{name}\x1b[0m \x1b[38;5;46m(exit: {exit_code})\x1b[0m"
                    );
                } else {
                    eprintln!(
                        "\x1b[38;5;196m💥 \x1b[1m\x1b[38;5;199mHook FAILED:\x1b[0m \
                         \x1b[38;5;226m{name}\x1b[0m \x1b[38;5;196m(exit: {exit_code})\x1b[0m"
                    );
                    if !stderr.is_empty() {
                        eprintln!(
                            "\x1b[38;5;197m  ⚠️  stderr: \x1b[38;5;167m{}\x1b[0m",
                            stderr.trim()
                        );
                    }
                }
                if !stdout.is_empty() {
                    eprintln!(
                        "\x1b[38;5;117m  📤 stdout: \x1b[38;5;152m{}\x1b[0m",
                        stdout.trim()
                    );
                }
            } else {
                eprintln!(
                    "[DEBUG] Hook {}: {} (exit: {})",
                    if success { "SUCCESS" } else { "FAILED" },
                    name,
                    exit_code
                );
                if !stdout.is_empty() {
                    eprintln!("[DEBUG]   stdout: {}", stdout.trim());
                }
                if !stderr.is_empty() {
                    eprintln!("[DEBUG]   stderr: {}", stderr.trim());
                }
            }
        }
    }

    /// Original hook execution logic (for Other execution type)
    #[allow(clippy::too_many_lines)]
    fn execute_original_hook(
        name: &str,
        hook: &ResolvedHook,
        worktree_context: &crate::hooks::resolver::WorktreeContext,
        changed_files: Option<&[PathBuf]>,
    ) -> Result<ExecutionResult> {
        use std::io::Read;
        use wait_timeout::ChildExt;

        // Create template resolver with worktree context
        let config_dir = hook
            .source_file
            .parent()
            .context("Hook source file has no parent directory")?;
        let mut template_resolver = TemplateResolver::with_worktree_context(
            config_dir,
            &hook.working_directory,
            worktree_context,
        );

        // Determine relevant changed files based on patterns
        let relevant_changed: Vec<PathBuf> = changed_files.map_or_else(Vec::new, |cf| {
            hook.definition.files.as_ref().map_or_else(
                || cf.to_vec(),
                |patterns| {
                    FilePatternMatcher::new(patterns).map_or_else(
                        |_| cf.to_vec(),
                        |matcher| cf.iter().filter(|p| matcher.matches(p)).cloned().collect(),
                    )
                },
            )
        });

        // Determine execution directory (same logic as execute_command_parts)
        let execution_dir = if hook.definition.run_at_root {
            &worktree_context.repo_root
        } else {
            &hook.working_directory
        };

        // Transform file paths from repo-relative to execution-directory-relative
        let transformed_files = Self::transform_file_paths(
            &relevant_changed,
            &worktree_context.repo_root,
            execution_dir,
        );

        // Create temp file for changed files if needed (using transformed paths)
        let changed_files_file = Self::create_changed_files_temp_file(&transformed_files);

        // Debug output for changed files
        Self::print_changed_files_debug(name, &transformed_files);

        // Set changed files in template resolver (using transformed paths)
        template_resolver.set_changed_files(&transformed_files, changed_files_file.as_deref());

        // Build command with template resolution
        let mut command =
            Self::build_command_from_hook(hook, &template_resolver, name, worktree_context)?;

        // Debug output right before execution
        if crate::debug::is_enabled() {
            if std::io::stderr().is_terminal() {
                eprintln!(
                    "\x1b[38;5;220m⚡ \x1b[1m\x1b[38;5;196mABOUT TO EXECUTE:\x1b[0m \
                     \x1b[38;5;226m{name}\x1b[0m"
                );
                eprintln!("\x1b[38;5;75m🎬 \x1b[1mStarting execution NOW...\x1b[0m");
            } else {
                eprintln!("[DEBUG] About to execute hook: {name}");
                eprintln!("[DEBUG] Starting execution...");
            }
        }

        // Execute command with timeout
        let timeout = std::time::Duration::from_secs(hook.definition.timeout_seconds);
        let mut child = command
            .spawn()
            .with_context(|| format!("Failed to spawn hook command: {name}"))?;

        // Take stdout and stderr handles before waiting
        let mut stdout_handle = child.stdout.take().context("Failed to capture stdout")?;
        let mut stderr_handle = child.stderr.take().context("Failed to capture stderr")?;

        // Spawn threads to read stdout and stderr in parallel
        // This prevents deadlocks from full pipe buffers
        let stdout_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            stdout_handle.read_to_end(&mut buf).ok();
            buf
        });

        let stderr_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            stderr_handle.read_to_end(&mut buf).ok();
            buf
        });

        // Wait for the command with timeout
        let status_option = child
            .wait_timeout(timeout)
            .with_context(|| format!("Failed to wait for hook command: {name}"))?;

        let (exit_code, stdout, stderr, success) = if let Some(status) = status_option {
            // Process finished within timeout - collect output from threads
            let stdout_buf = stdout_thread.join().unwrap_or_default();
            let stderr_buf = stderr_thread.join().unwrap_or_default();

            let stdout = String::from_utf8_lossy(&stdout_buf).to_string();
            let stderr = String::from_utf8_lossy(&stderr_buf).to_string();
            let exit_code = status.code().unwrap_or(-1);
            let success = status.success();

            (exit_code, stdout, stderr, success)
        } else {
            // Timeout occurred - kill the process
            let _ = child.kill();
            let _ = child.wait(); // Reap the process

            // Still try to collect partial output
            let stdout_buf = stdout_thread.join().unwrap_or_default();
            let stderr_buf = stderr_thread.join().unwrap_or_default();
            let stdout = String::from_utf8_lossy(&stdout_buf);
            let stderr = String::from_utf8_lossy(&stderr_buf);

            // Cleanup temp file before returning error
            if let Some(p) = changed_files_file {
                let _ = std::fs::remove_file(p);
            }

            return Err(anyhow::anyhow!(
                "Hook '{name}' exceeded timeout of {} seconds and was killed\nPartial stdout: \
                 {stdout}\nPartial stderr: {stderr}",
                hook.definition.timeout_seconds
            ));
        };

        // Cleanup temp file, if any
        if let Some(p) = changed_files_file {
            let _ = std::fs::remove_file(p);
        }

        // Debug output for results
        Self::print_execution_debug_output(name, success, exit_code, &stdout, &stderr);

        Ok(ExecutionResult {
            exit_code,
            stdout,
            stderr,
            success,
        })
    }
}

impl Default for HookExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionResults {
    /// Print a summary of execution results
    pub fn print_summary(&self) {
        let fmt = formatter();

        println!("{}", fmt.section_header("Hook Execution Summary"));

        for (name, result) in &self.results {
            println!(
                "{}",
                fmt.hook_result(name, result.success, result.exit_code)
            );

            if !result.stdout.is_empty() {
                println!("  stdout: {}", result.stdout.trim());
            }

            if !result.stderr.is_empty() {
                println!("  stderr: {}", result.stderr.trim());
            }
        }

        println!("{}", fmt.overall_result(self.success));
    }

    /// Print execution with progress bar (TTY only)
    pub fn print_with_progress(&self, hook_names: &[String]) {
        let fmt = formatter();

        if let Some(pb) = fmt.create_progress_bar(hook_names.len() as u64) {
            pb.set_message("Starting hooks...");

            for (i, name) in hook_names.iter().enumerate() {
                pb.set_message(format!("Running {name}"));
                pb.set_position(i as u64);

                // Simulate some work for demo
                std::thread::sleep(Duration::from_millis(100));

                if let Some(result) = self.results.get(name) {
                    let status = if result.success { "✅" } else { "❌" };
                    pb.println(format!("{status} {name}"));
                }
            }

            pb.finish_with_message("Hook execution completed!");
        } else {
            // Fallback to regular summary for non-TTY
            self.print_summary();
        }
    }

    /// Get failed hooks
    #[must_use]
    pub fn get_failed_hooks(&self) -> Vec<&str> {
        self.results
            .iter()
            .filter_map(|(name, result)| {
                if result.success {
                    None
                } else {
                    Some(name.as_str())
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HookCommand, HookDefinition};
    use std::{collections::HashMap, path::PathBuf};

    fn create_test_hook(command: HookCommand, workdir: Option<String>) -> ResolvedHook {
        ResolvedHook {
            definition: HookDefinition {
                command,
                workdir,
                env: None,
                description: None,
                modifies_repository: false,
                files: None,
                run_always: true, // Always run in tests since we pass None for changed_files
                requires_files: false, // Default to false for tests
                depends_on: None,
                execution_type: crate::config::parser::ExecutionType::PerFile,
                run_at_root: false,
                timeout_seconds: 300,
            },
            working_directory: std::env::temp_dir(),
            source_file: PathBuf::from("test.toml"),
        }
    }

    fn create_test_worktree_context() -> crate::hooks::resolver::WorktreeContext {
        crate::hooks::resolver::WorktreeContext {
            is_worktree: false,
            worktree_name: None,
            repo_root: std::env::temp_dir(),
            common_dir: std::env::temp_dir().join(".git"),
            working_dir: std::env::temp_dir(),
        }
    }

    #[test]
    fn test_execute_shell_command_success() {
        let hook = create_test_hook(HookCommand::Shell("echo 'hello world'".to_string()), None);

        let worktree_context = create_test_worktree_context();
        let result =
            HookExecutor::execute_single_hook("test", &hook, &worktree_context, None).unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello world");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn test_execute_shell_command_failure() {
        let hook = create_test_hook(HookCommand::Shell("exit 1".to_string()), None);

        let worktree_context = create_test_worktree_context();
        let result =
            HookExecutor::execute_single_hook("test", &hook, &worktree_context, None).unwrap();

        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_execute_args_command() {
        let hook = create_test_hook(
            HookCommand::Args(vec![
                "echo".to_string(),
                "hello".to_string(),
                "args".to_string(),
            ]),
            None,
        );

        let worktree_context = create_test_worktree_context();
        let result =
            HookExecutor::execute_single_hook("test", &hook, &worktree_context, None).unwrap();

        assert!(result.success);
        assert_eq!(result.stdout.trim(), "hello args");
    }

    #[test]
    fn test_execute_multiple_hooks() {
        let mut hooks = HashMap::new();

        hooks.insert(
            "success".to_string(),
            create_test_hook(HookCommand::Shell("exit 0".to_string()), None),
        );

        hooks.insert(
            "failure".to_string(),
            create_test_hook(HookCommand::Shell("exit 1".to_string()), None),
        );

        let resolved_hooks = ResolvedHooks {
            config_path: PathBuf::from("test.toml"),
            hooks,
            execution_strategy: ExecutionStrategy::Sequential,
            changed_files: None,
            worktree_context: create_test_worktree_context(),
        };

        let results = HookExecutor::execute(&resolved_hooks).unwrap();

        assert!(!results.success); // Overall failure due to one failed hook
        assert_eq!(results.results.len(), 2);

        assert!(results.results["success"].success);
        assert!(!results.results["failure"].success);

        let failed = results.get_failed_hooks();
        assert_eq!(failed, vec!["failure"]);
    }

    #[test]
    fn test_parallel_safe_execution() {
        let mut hooks = HashMap::new();

        // Safe hooks that can run in parallel
        hooks.insert(
            "lint".to_string(),
            create_test_hook_with_modification(HookCommand::Shell("echo lint".to_string()), false),
        );

        hooks.insert(
            "test".to_string(),
            create_test_hook_with_modification(HookCommand::Shell("echo test".to_string()), false),
        );

        // Repository-modifying hook that must run sequentially
        hooks.insert(
            "format".to_string(),
            create_test_hook_with_modification(HookCommand::Shell("echo format".to_string()), true),
        );

        let resolved_hooks = ResolvedHooks {
            config_path: PathBuf::from("test.toml"),
            hooks,
            execution_strategy: ExecutionStrategy::Parallel,
            changed_files: None,
            worktree_context: create_test_worktree_context(),
        };

        let results = HookExecutor::execute(&resolved_hooks).unwrap();

        assert!(results.success);
        assert_eq!(results.results.len(), 3);
        assert!(results.results["lint"].success);
        assert!(results.results["test"].success);
        assert!(results.results["format"].success);
    }

    #[test]
    fn test_sequential_execution() {
        let mut hooks = HashMap::new();

        hooks.insert(
            "hook1".to_string(),
            create_test_hook(HookCommand::Shell("echo hook1".to_string()), None),
        );

        hooks.insert(
            "hook2".to_string(),
            create_test_hook(HookCommand::Shell("echo hook2".to_string()), None),
        );

        let resolved_hooks = ResolvedHooks {
            config_path: PathBuf::from("test.toml"),
            hooks,
            execution_strategy: ExecutionStrategy::Sequential,
            changed_files: None,
            worktree_context: create_test_worktree_context(),
        };

        let results = HookExecutor::execute(&resolved_hooks).unwrap();

        assert!(results.success);
        assert_eq!(results.results.len(), 2);
    }

    #[test]
    fn test_force_parallel_execution() {
        let mut hooks = HashMap::new();

        // Even repository-modifying hooks run in parallel (unsafe mode)
        hooks.insert(
            "format1".to_string(),
            create_test_hook_with_modification(
                HookCommand::Shell("echo format1".to_string()),
                true,
            ),
        );

        hooks.insert(
            "format2".to_string(),
            create_test_hook_with_modification(
                HookCommand::Shell("echo format2".to_string()),
                true,
            ),
        );

        let resolved_hooks = ResolvedHooks {
            config_path: PathBuf::from("test.toml"),
            hooks,
            execution_strategy: ExecutionStrategy::ForceParallel,
            changed_files: None,
            worktree_context: create_test_worktree_context(),
        };

        let results = HookExecutor::execute(&resolved_hooks).unwrap();

        assert!(results.success);
        assert_eq!(results.results.len(), 2);
    }

    fn create_test_hook_with_modification(
        command: HookCommand,
        modifies_repository: bool,
    ) -> ResolvedHook {
        ResolvedHook {
            definition: HookDefinition {
                command,
                workdir: None,
                env: None,
                description: None,
                modifies_repository,
                files: None,
                run_always: false,
                requires_files: false,
                depends_on: None,
                execution_type: crate::config::parser::ExecutionType::PerFile,
                run_at_root: false,
                timeout_seconds: 300,
            },
            working_directory: std::env::temp_dir(),
            source_file: PathBuf::from("test.toml"),
        }
    }

    #[test]
    fn test_env_vars_filtered_changed_files() {
        // Hook with file filter should receive only matching changes
        let hook = ResolvedHook {
            definition: HookDefinition {
                command: HookCommand::Shell(
                    "printf '%s\n' '{CHANGED_FILES}' && printf '%s\n' '{CHANGED_FILES_LIST}' && \
                     cat '{CHANGED_FILES_FILE}'"
                        .to_string(),
                ),
                workdir: None,
                env: None,
                description: None,
                modifies_repository: false,
                files: Some(vec!["**/*.rs".to_string()]),
                run_always: false,
                requires_files: false,
                depends_on: None,
                execution_type: crate::config::parser::ExecutionType::Other,
                run_at_root: false,
                timeout_seconds: 300,
            },
            working_directory: std::env::temp_dir(),
            source_file: PathBuf::from("test.toml"),
        };
        let worktree_context = create_test_worktree_context();
        let changes = vec![PathBuf::from("src/a.rs"), PathBuf::from("README.md")];
        let result =
            HookExecutor::execute_single_hook("filtered", &hook, &worktree_context, Some(&changes))
                .unwrap();
        assert!(result.success);
        let out = result.stdout;
        assert!(out.contains("src/a.rs"));
        assert!(!out.contains("README.md"));
    }

    #[test]
    fn test_env_vars_all_changed_files_no_filter() {
        let hook = ResolvedHook {
            definition: HookDefinition {
                command: HookCommand::Shell("printf '%s\n' '{CHANGED_FILES}'".to_string()),
                workdir: None,
                env: None,
                description: None,
                modifies_repository: false,
                files: None,
                run_always: false,
                requires_files: false,
                depends_on: None,
                execution_type: crate::config::parser::ExecutionType::Other,
                run_at_root: false,
                timeout_seconds: 300,
            },
            working_directory: std::env::temp_dir(),
            source_file: PathBuf::from("test.toml"),
        };
        let worktree_context = create_test_worktree_context();
        let changes = vec![PathBuf::from("a"), PathBuf::from("b/c")];
        let result =
            HookExecutor::execute_single_hook("nofilter", &hook, &worktree_context, Some(&changes))
                .unwrap();
        assert!(result.success);
        let out = result.stdout;
        assert!(out.contains('a'));
        assert!(out.contains("b/c"));
    }

    #[test]
    fn test_env_vars_empty_when_no_changes() {
        let hook = ResolvedHook {
            definition: HookDefinition {
                command: HookCommand::Shell(
                    "printf '[%s]-[%s]-[%s]\n' '{CHANGED_FILES}' '{CHANGED_FILES_LIST}' \
                     '{CHANGED_FILES_FILE}'"
                        .to_string(),
                ),
                workdir: None,
                env: None,
                description: None,
                modifies_repository: false,
                files: None,
                run_always: false,
                requires_files: false,
                depends_on: None,
                execution_type: crate::config::parser::ExecutionType::Other,
                run_at_root: false,
                timeout_seconds: 300,
            },
            working_directory: std::env::temp_dir(),
            source_file: PathBuf::from("test.toml"),
        };
        let worktree_context = create_test_worktree_context();
        let result =
            HookExecutor::execute_single_hook("empty", &hook, &worktree_context, None).unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("[]-[]-[]"));
    }

    #[test]
    fn test_run_at_root_flag_execution() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");
        let config_dir = temp_dir.path().join("subdir");
        fs::create_dir_all(&config_dir).expect("create config dir");

        let worktree_context = crate::hooks::resolver::WorktreeContext {
            repo_root: temp_dir.path().to_path_buf(),
            common_dir: temp_dir.path().to_path_buf(),
            working_dir: config_dir.clone(),
            is_worktree: false,
            worktree_name: None,
        };

        // Hook with run_at_root = true should run at repo root
        let hook_at_root = ResolvedHook {
            definition: HookDefinition {
                command: HookCommand::Shell("pwd".to_string()),
                workdir: None,
                env: None,
                description: None,
                modifies_repository: false,
                files: None,
                run_always: false,
                requires_files: false,
                depends_on: None,
                execution_type: crate::config::parser::ExecutionType::Other,
                run_at_root: true,
                timeout_seconds: 300,
            },
            source_file: config_dir.join(".peter-hook.toml"),
            working_directory: config_dir.clone(),
        };

        // Hook with run_at_root = false should run at config directory
        let hook_at_config = ResolvedHook {
            definition: HookDefinition {
                command: HookCommand::Shell("pwd".to_string()),
                workdir: None,
                env: None,
                description: None,
                modifies_repository: false,
                files: None,
                run_always: false,
                requires_files: false,
                depends_on: None,
                execution_type: crate::config::parser::ExecutionType::Other,
                timeout_seconds: 300,
                run_at_root: false,
            },
            source_file: config_dir.join(".peter-hook.toml"),
            working_directory: config_dir.clone(),
        };

        // Test hook with run_at_root = true
        let result_root =
            HookExecutor::execute_single_hook("root", &hook_at_root, &worktree_context, None)
                .unwrap();
        assert!(result_root.success);
        let root_pwd = result_root.stdout.trim();
        // Use canonical paths for comparison due to macOS temp directory symlinks
        let canonical_temp = temp_dir.path().canonicalize().expect("canonicalize temp");
        let canonical_root_pwd = PathBuf::from(root_pwd)
            .canonicalize()
            .expect("canonicalize root pwd");
        assert_eq!(canonical_root_pwd, canonical_temp);

        // Test hook with run_at_root = false
        let result_config =
            HookExecutor::execute_single_hook("config", &hook_at_config, &worktree_context, None)
                .unwrap();
        assert!(result_config.success);
        let config_pwd = result_config.stdout.trim();
        // Use canonical paths for comparison due to macOS temp directory symlinks
        let canonical_config = config_dir.canonicalize().expect("canonicalize config");
        let canonical_config_pwd = PathBuf::from(config_pwd)
            .canonicalize()
            .expect("canonicalize config pwd");
        assert_eq!(canonical_config_pwd, canonical_config);
    }
}
