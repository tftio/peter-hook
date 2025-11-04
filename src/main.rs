//! Peter Hook - A hierarchical git hooks manager for monorepos

use anyhow::{Context, Result};
use clap::Parser;
use peter_hook::{
    HookCommand,
    cli::{Cli, Commands, ConfigCommand},
    config::GlobalConfig,
    debug,
    git::{
        ChangeDetectionMode, GitHookInstaller, GitRepository, SUPPORTED_HOOKS, WorktreeHookStrategy,
    },
    hooks::{HookExecutor, HookResolver},
};
use std::{
    env,
    io::{self, IsTerminal, Write},
    process,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Enable debug mode if requested
    if cli.debug {
        debug::enable();
    }

    match cli.command {
        Commands::Install {
            force,
            worktree_strategy,
        } => install_hooks(force, &worktree_strategy),
        Commands::Uninstall { yes } => uninstall_hooks(yes),
        Commands::Run {
            event,
            git_args,
            all_files,
            dry_run,
        } => run_hooks(&event, &git_args, all_files, dry_run),
        Commands::Validate {
            trace_imports,
            json,
        } => validate_config(trace_imports, json),
        Commands::List => list_hooks(),
        Commands::ListWorktrees => list_worktrees(),
        Commands::Config { subcommand } => handle_config_command(&subcommand),
        Commands::Lint { hook_name, dry_run } => run_lint_mode(&hook_name, dry_run),
        Commands::Version => {
            show_version();
            Ok(())
        }
        Commands::License => {
            show_license();
            Ok(())
        }
        Commands::Completions { shell } => {
            peter_hook::cli::completions::generate_completions(shell);
            Ok(())
        }
        Commands::RunTargets => {
            print_run_targets();
            Ok(())
        }
        Commands::LintTargets => print_lint_targets(),
        Commands::Doctor => {
            let exit_code = peter_hook::doctor::run_doctor();
            if exit_code != 0 {
                process::exit(exit_code);
            }
            Ok(())
        }
    }
}

/// Install git hooks for the current repository
fn install_hooks(force: bool, worktree_strategy: &str) -> Result<()> {
    println!("Installing git hooks...");

    // Parse the worktree strategy
    let strategy: WorktreeHookStrategy = worktree_strategy
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid worktree strategy: {worktree_strategy}"))?;

    let installer = GitHookInstaller::with_strategy(strategy)
        .context("Failed to initialize git hook installer")?;

    if !force {
        // Check if any hooks would be overwritten
        let repo = GitRepository::find_from_current_dir()?;
        let existing_hooks = repo.list_hooks()?;
        if !existing_hooks.is_empty()
            && existing_hooks
                .iter()
                .find_map(|hook| {
                    repo.get_hook_info(hook).ok().flatten().and_then(|info| {
                        if info.is_managed {
                            Some(hook.clone())
                        } else {
                            None
                        }
                    })
                })
                .is_none()
        {
            println!("⚠️  Found existing git hooks that are not managed by peter-hook:");
            for hook in &existing_hooks {
                println!("  - {hook}");
            }
            println!("\nUse --force to backup existing hooks and install peter-hook hooks.");
            println!("Or use 'peter-hook uninstall' to remove existing hooks first.");
            return Ok(());
        }
    }

    let report = installer
        .install_all()
        .context("Failed to install git hooks")?;

    report.print_summary();

    if !report.is_success() {
        process::exit(1);
    }

    Ok(())
}

/// Uninstall peter-hook managed hooks
fn uninstall_hooks(yes: bool) -> Result<()> {
    if !yes {
        println!("This will remove all peter-hook managed hooks from your repository.");
        println!("Backed up hooks will be restored if they exist.");
        print!("Are you sure you want to continue? [y/N]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Uninstall cancelled.");
            return Ok(());
        }
    }

    let installer = GitHookInstaller::new().context("Failed to initialize git hook installer")?;

    let report = installer.uninstall_all();

    report.print_summary();

    if !report.is_success() {
        process::exit(1);
    }

    Ok(())
}

/// List all git hooks in the repository
fn list_hooks() -> Result<()> {
    let repo = GitRepository::find_from_current_dir().context("Failed to find git repository")?;

    let hooks = repo.list_hooks().context("Failed to list git hooks")?;

    if hooks.is_empty() {
        println!("No git hooks found in this repository.");
        return Ok(());
    }

    println!("Git hooks in this repository:");
    println!("============================");

    for hook_name in hooks {
        if let Some(info) = repo.get_hook_info(&hook_name)? {
            let status = if info.is_managed {
                "🔧 managed"
            } else {
                "📄 custom"
            };

            let executable = if info.is_executable { "✅" } else { "❌" };

            println!(
                "{} {} {} (executable: {})",
                executable,
                hook_name,
                status,
                if info.is_executable { "yes" } else { "no" }
            );
        }
    }

    Ok(())
}

/// Show version information
fn show_version() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}

/// Show license information
fn show_license() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("License: {}", env!("CARGO_PKG_LICENSE"));
    println!("Repository: {}", env!("CARGO_PKG_REPOSITORY"));
    println!();
    println!("MIT License");
    println!();
    println!("Permission is hereby granted, free of charge, to any person obtaining a copy");
    println!("of this software and associated documentation files (the \"Software\"), to deal");
    println!("in the Software without restriction, including without limitation the rights");
    println!("to use, copy, modify, merge, publish, distribute, sublicense, and/or sell");
    println!("copies of the Software, and to permit persons to whom the Software is");
    println!("furnished to do so, subject to the following conditions:");
    println!();
    println!("The above copyright notice and this permission notice shall be included in all");
    println!("copies or substantial portions of the Software.");
    println!();
    println!("THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR");
    println!("IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,");
    println!("FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE");
    println!("AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER");
    println!("LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,");
    println!("OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE");
    println!("SOFTWARE.");
}

/// Print supported git hook names for completion scripts
fn print_run_targets() {
    for event in SUPPORTED_HOOKS {
        println!("{event}");
    }
}

/// Print available hook/group names for lint completions
fn print_lint_targets() -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current working directory")?;
    let resolver = HookResolver::new(&current_dir);
    let names = resolver
        .list_hook_names()
        .context("Failed to load hook configuration for completions")?;

    for name in names {
        println!("{name}");
    }

    Ok(())
}

/// Run hooks for a specific git event
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn run_hooks(event: &str, _git_args: &[String], all_files: bool, dry_run: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current working directory")?;

    // Get repository information for hierarchical resolution
    let repo = GitRepository::find_from_current_dir().context("Failed to find git repository")?;

    // Create worktree context
    let worktree_context = peter_hook::hooks::WorktreeContext {
        is_worktree: repo.is_worktree,
        worktree_name: repo.get_worktree_name().map(ToString::to_string),
        repo_root: repo.root.clone(),
        common_dir: repo.common_dir.clone(),
        working_dir: current_dir.clone(),
    };

    // Determine change detection mode based on event type (unless --all-files is
    // specified)
    let change_mode = if all_files {
        None // No file filtering when --all-files is specified
    } else {
        match event {
            "pre-commit" => Some(ChangeDetectionMode::Staged),
            "pre-push" => Some(ChangeDetectionMode::Push {
                remote: "origin".to_string(),
                remote_branch: "main".to_string(), // TODO: detect actual default branch
            }),
            "commit-msg" | "prepare-commit-msg" => None, // Message hooks don't filter by files
            "post-commit" | "post-merge" | "post-checkout" => {
                Some(ChangeDetectionMode::CommitRange {
                    from: "HEAD^".to_string(),
                    to: "HEAD".to_string(),
                })
            }
            _ => Some(ChangeDetectionMode::WorkingDirectory), // Default for other hooks
        }
    };

    // Use hierarchical resolution to find hooks for each changed file
    let groups = peter_hook::hooks::resolve_hooks_hierarchically(
        event,
        change_mode,
        &repo.root,
        &current_dir,
        &worktree_context,
    )
    .context("Failed to resolve hooks hierarchically")?;

    if groups.is_empty() {
        // No config groups found
        if io::stdout().is_terminal() {
            println!("❌ \x1b[33mNo hooks configured for event:\x1b[0m \x1b[1m{event}\x1b[0m");
            println!("💡 \x1b[36mTip:\x1b[0m Check your \x1b[33mhooks.toml\x1b[0m configuration");
        } else {
            println!("No hooks found for event: {event}");
        }
    } else {
        // We have at least one config group with hooks
        // For backwards compatibility with the display logic, use the first group's
        // resolved_hooks for display purposes
        let first_resolved = &groups[0].resolved_hooks;
        let resolved_hooks = first_resolved;
        if debug::is_enabled() && io::stdout().is_terminal() {
            println!(
                "\x1b[38;5;201m🎪 \x1b[1m\x1b[38;5;51mPETER-HOOK EXECUTION EXTRAVAGANZA!\x1b[0m"
            );
            println!(
                "\x1b[38;5;198m📋 Config: \x1b[38;5;87m{}\x1b[0m",
                resolved_hooks.config_path.display()
            );

            if let Some(ref changed_files) = resolved_hooks.changed_files {
                println!(
                    "\x1b[38;5;214m🎯 \x1b[1m\x1b[38;5;208mFile targeting activated!\x1b[0m \
                     \x1b[38;5;118m{} files detected\x1b[0m",
                    changed_files.len()
                );
                if changed_files.is_empty() {
                    println!(
                        "\x1b[38;5;226m⚡ \x1b[1mNo files changed - hooks may skip for maximum \
                         speed!\x1b[0m"
                    );
                } else {
                    // Show first few files with rotating emojis
                    let file_emojis = ["📄", "📝", "🔧", "⚙️", "🎨", "🚀"];
                    for (i, file) in changed_files.iter().take(6).enumerate() {
                        let emoji = file_emojis[i % file_emojis.len()];
                        println!(
                            "\x1b[38;5;147m    {} \x1b[38;5;183m{}\x1b[0m",
                            emoji,
                            file.display()
                        );
                    }
                    if changed_files.len() > 6 {
                        println!(
                            "\x1b[38;5;147m    🌟 \x1b[38;5;105m... and {} more files!\x1b[0m",
                            changed_files.len() - 6
                        );
                    }
                }
            }

            println!(
                "\x1b[38;5;46m🚀 \x1b[1m\x1b[38;5;82mLaunching {} hooks for event:\x1b[0m \
                 \x1b[38;5;226m{}\x1b[0m",
                resolved_hooks.hooks.len(),
                event
            );

            // Show hook configuration summary with crazy colors and emojis
            println!(
                "\x1b[38;5;198m🎭 \x1b[1m\x1b[38;5;207mHOOK CONFIGURATION EXTRAVAGANZA!\x1b[0m"
            );

            // Group hooks by file patterns for visual organization
            let mut pattern_groups = std::collections::HashMap::new();
            for (hook_name, hook) in &resolved_hooks.hooks {
                let patterns = hook.definition.files.as_ref().map_or_else(
                    || {
                        if hook.definition.run_always {
                            "🌍 ALL FILES (run_always)".to_string()
                        } else {
                            "🎯 NO PATTERNS".to_string()
                        }
                    },
                    |files| files.join(", "),
                );
                pattern_groups
                    .entry(patterns)
                    .or_insert_with(Vec::new)
                    .push(hook_name);
            }

            let colors = [196, 208, 226, 118, 51, 99, 201, 165, 129, 93];
            for (i, (pattern, hooks)) in pattern_groups.iter().enumerate() {
                let color = colors[i % colors.len()];
                let emoji = match i % 8 {
                    0 => "🐍",
                    1 => "⚡",
                    2 => "🔧",
                    3 => "🎨",
                    4 => "🛡️",
                    5 => "📊",
                    6 => "🌐",
                    _ => "✨",
                };
                println!("\x1b[38;5;{color}{emoji} Pattern: \x1b[38;5;159m{pattern}\x1b[0m");
                for hook in hooks {
                    println!("\x1b[38;5;147m      🎪 \x1b[38;5;183m{hook}\x1b[0m");
                }
            }

            println!("\x1b[38;5;198m{}\x1b[0m", "═".repeat(60));
        } else if io::stdout().is_terminal() {
            // Fun terminal output when writing to TTY
            println!("\n🎯 \x1b[1m\x1b[36mHook Configuration Found\x1b[0m");
            println!("📂 \x1b[33m{}\x1b[0m", resolved_hooks.config_path.display());

            if let Some(ref changed_files) = resolved_hooks.changed_files {
                if changed_files.is_empty() {
                    println!(
                        "📋 \x1b[33mNo files changed\x1b[0m - some hooks may be \
                         \x1b[90mskipped\x1b[0m"
                    );
                } else {
                    println!(
                        "📁 \x1b[32m{}\x1b[0m changed files detected",
                        changed_files.len()
                    );
                    if changed_files.len() <= 5 {
                        for file in changed_files {
                            println!("   \x1b[90m•\x1b[0m \x1b[37m{}\x1b[0m", file.display());
                        }
                    } else {
                        for file in changed_files.iter().take(3) {
                            println!("   \x1b[90m•\x1b[0m \x1b[37m{}\x1b[0m", file.display());
                        }
                        println!(
                            "   \x1b[90m... and {} more files\x1b[0m",
                            changed_files.len() - 3
                        );
                    }
                }
            }

            let hook_emoji = match resolved_hooks.hooks.len() {
                1 => "🚀",
                2..=3 => "⚡",
                4..=6 => "🎪",
                _ => "🌟",
            };

            println!(
                "\n{} \x1b[1m\x1b[35mExecuting {} hooks\x1b[0m for event: \x1b[1m\x1b[33m{}\x1b[0m",
                hook_emoji,
                resolved_hooks.hooks.len(),
                event
            );

            // Show hook names in a nice format
            let hook_names: Vec<_> = resolved_hooks.hooks.keys().collect();
            if hook_names.len() <= 4 {
                println!(
                    "🔧 Hooks: {}",
                    hook_names
                        .iter()
                        .map(|&name| format!("\x1b[36m{name}\x1b[0m"))
                        .collect::<Vec<_>>()
                        .join("\x1b[90m, \x1b[0m")
                );
            } else {
                println!(
                    "🔧 Hooks: {} and {} others",
                    hook_names
                        .iter()
                        .take(3)
                        .map(|&name| format!("\x1b[36m{name}\x1b[0m"))
                        .collect::<Vec<_>>()
                        .join("\x1b[90m, \x1b[0m"),
                    hook_names.len() - 3
                );
            }
            println!();
        } else {
            // Plain output for non-TTY (pipes, redirects, etc.)
            println!(
                "Found hooks configuration: {}",
                resolved_hooks.config_path.display()
            );
            if let Some(ref changed_files) = resolved_hooks.changed_files {
                println!("Detected {} changed files", changed_files.len());
                if changed_files.is_empty() {
                    println!("No files changed - some hooks may be skipped");
                }
            }
            println!(
                "Running {} hooks for event: {}",
                resolved_hooks.hooks.len(),
                event
            );
        }

        // Handle dry-run mode
        if dry_run {
            if io::stdout().is_terminal() {
                println!("🔍 \x1b[1m\x1b[36mDry Run Mode\x1b[0m - showing what would execute:");
                println!(
                    "📋 \x1b[33m{}\x1b[0m hooks would run:",
                    resolved_hooks.hooks.len()
                );

                for (name, hook) in &resolved_hooks.hooks {
                    let cmd_str = match &hook.definition.command {
                        HookCommand::Shell(cmd) => cmd.clone(),
                        HookCommand::Args(args) => args.join(" "),
                    };
                    println!("   🎯 \x1b[36m{name}\x1b[0m: \x1b[90m{cmd_str}\x1b[0m");
                    println!(
                        "      📂 Working dir: \x1b[90m{}\x1b[0m",
                        hook.working_directory.display()
                    );
                    if let Some(ref patterns) = hook.definition.files {
                        println!(
                            "      📄 File patterns: \x1b[90m{}\x1b[0m",
                            patterns.join(", ")
                        );
                    }
                    if hook.definition.run_always {
                        println!("      ⚡ Always runs (ignores file changes)");
                    }
                }

                if let Some(ref changed_files) = resolved_hooks.changed_files {
                    println!(
                        "\n📁 \x1b[32m{}\x1b[0m changed files detected:",
                        changed_files.len()
                    );
                    for file in changed_files.iter().take(10) {
                        println!("   \x1b[90m•\x1b[0m \x1b[37m{}\x1b[0m", file.display());
                    }
                    if changed_files.len() > 10 {
                        println!(
                            "   \x1b[90m... and {} more files\x1b[0m",
                            changed_files.len() - 10
                        );
                    }
                } else {
                    println!("\n📂 File filtering disabled - would run on all files");
                }
            } else {
                println!(
                    "DRY RUN: {} hooks would run for event: {}",
                    resolved_hooks.hooks.len(),
                    event
                );
                for (name, hook) in &resolved_hooks.hooks {
                    let cmd_str = match &hook.definition.command {
                        HookCommand::Shell(cmd) => cmd.clone(),
                        HookCommand::Args(args) => args.join(" "),
                    };
                    println!("  {name} - {cmd_str}");
                }
                if let Some(ref changed_files) = resolved_hooks.changed_files {
                    println!("Changed files: {}", changed_files.len());
                } else {
                    println!("File filtering disabled");
                }
            }
            return Ok(());
        }

        // Execute all config groups hierarchically
        let results = HookExecutor::execute_multiple(&groups).context("Failed to execute hooks")?;

        if debug::is_enabled() && io::stdout().is_terminal() {
            println!("\x1b[38;5;198m{}\x1b[0m", "═".repeat(60));
            if results.success {
                println!(
                    "\x1b[38;5;46m🎊 \x1b[1m\x1b[38;5;82mALL HOOKS SUCCEEDED!\x1b[0m \
                     \x1b[38;5;46m🎊\x1b[0m"
                );
                println!(
                    "\x1b[38;5;118m✨ Your code is \
                     \x1b[1m\x1b[38;5;159mPERFECT\x1b[0m\x1b[38;5;118m! Ready to commit! ✨\x1b[0m"
                );
            } else {
                println!(
                    "\x1b[38;5;196m💥 \x1b[1m\x1b[38;5;199mSOME HOOKS FAILED!\x1b[0m \
                     \x1b[38;5;196m💥\x1b[0m"
                );
                let failed = results.get_failed_hooks();
                println!(
                    "\x1b[38;5;197m🚨 Failed hooks: \x1b[38;5;167m{}\x1b[0m",
                    failed.join(", ")
                );
            }
            println!("\x1b[38;5;198m{}\x1b[0m", "═".repeat(60));
            results.print_summary();
        } else if !debug::is_enabled() && io::stdout().is_terminal() {
            // Fun completion message for successful runs (non-debug TTY output)
            if results.success {
                let success_messages = [
                    "🎉 All hooks passed! Your code is looking great!",
                    "✨ Perfect! All checks completed successfully!",
                    "🚀 Excellent work! All hooks are happy!",
                    "🎊 Fantastic! Everything looks good to go!",
                    "💫 Outstanding! All validation passed!",
                ];
                let message = success_messages[resolved_hooks.hooks.len() % success_messages.len()];
                println!("\n{message}");

                // Show quick summary without hook output (happy path)
                let passed_count = results.results.len();
                println!(
                    "✅ \x1b[32m{}\x1b[0m hook{} completed successfully\n",
                    passed_count,
                    if passed_count == 1 { "" } else { "s" }
                );
            } else {
                println!("\n💥 \x1b[31mSome hooks failed!\x1b[0m");
                let failed = results.get_failed_hooks();
                println!("❌ Failed: \x1b[31m{}\x1b[0m\n", failed.join(", "));

                // Print detailed summary for failures to show what went wrong
                results.print_summary();
            }
        } else {
            // Always print full summary for non-TTY or when piped/redirected
            results.print_summary();
        }

        if !results.success {
            process::exit(1);
        }
    }

    Ok(())
}

/// Validate hook configuration
fn validate_config(trace_imports: bool, json: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current working directory")?;

    let resolver = HookResolver::new(&current_dir);

    match resolver.find_config_file()? {
        Some(config_path) => {
            println!("Validating config file: {}", config_path.display());

            // Try to parse the configuration
            if trace_imports {
                match peter_hook::HookConfig::from_file_with_trace(&config_path) {
                    Ok((config, diag)) => {
                        println!("✓ Configuration is valid");

                        let hook_names = config.get_hook_names();
                        if hook_names.is_empty() {
                            println!("  No hooks or groups defined");
                        } else {
                            println!("  Found {} hooks/groups:", hook_names.len());
                            for name in hook_names {
                                println!("    - {name}");
                            }
                        }

                        if json {
                            // Print diagnostics as JSON
                            match serde_json::to_string_pretty(&diag) {
                                Ok(s) => println!("{s}"),
                                Err(e) => eprintln!("Failed to serialize diagnostics: {e:#}"),
                            }
                        } else {
                            // Human-readable diagnostics
                            if diag.imports.is_empty() {
                                println!("(no imports)");
                            } else {
                                println!("Imports (order):");
                                for r in &diag.imports {
                                    println!("  {} -> {}", r.from, r.resolved);
                                }
                            }
                            if !diag.overrides.is_empty() {
                                println!("Overrides:");
                                for o in &diag.overrides {
                                    println!(
                                        "  {} {}: {} -> {}",
                                        o.kind, o.name, o.previous, o.new
                                    );
                                }
                            }
                            if !diag.cycles.is_empty() {
                                println!("Cycles (skipped):");
                                for c in &diag.cycles {
                                    println!("  {c}");
                                }
                            }
                            if !diag.unused.is_empty() {
                                println!("Unused imports (no contributions):");
                                for u in &diag.unused {
                                    println!("  {u}");
                                }
                            }
                        }

                        // Display detailed hook composition
                        print_hook_details(&config);
                    }
                    Err(e) => {
                        eprintln!("✗ Configuration is invalid: {e:#}");
                        process::exit(1);
                    }
                }
            } else {
                match peter_hook::HookConfig::from_file(&config_path) {
                    Ok(config) => {
                        println!("✓ Configuration is valid");

                        let hook_names = config.get_hook_names();
                        if hook_names.is_empty() {
                            println!("  No hooks or groups defined");
                        } else {
                            println!("  Found {} hooks/groups:", hook_names.len());
                            for name in hook_names {
                                println!("    - {name}");
                            }
                        }

                        // Display detailed hook composition
                        print_hook_details(&config);
                    }
                    Err(e) => {
                        eprintln!("✗ Configuration is invalid: {e:#}");
                        process::exit(1);
                    }
                }
            }
        }
        None => {
            println!("No hooks.toml file found in current directory or parent directories");
        }
    }

    Ok(())
}

/// Print detailed information about all hooks and groups in the configuration
fn print_hook_details(config: &peter_hook::HookConfig) {
    use peter_hook::config::{ExecutionType, HookCommand};

    println!("\n═══════════════════════════════════════════════════════════");
    println!("                HOOK COMPOSITION DETAILS");
    println!("═══════════════════════════════════════════════════════════\n");

    // Print individual hooks
    if let Some(hooks) = &config.hooks {
        if !hooks.is_empty() {
            println!("📋 Individual Hooks:\n");
            let mut sorted_hooks: Vec<_> = hooks.iter().collect();
            sorted_hooks.sort_by_key(|(name, _)| *name);

            for (name, hook) in sorted_hooks {
                println!("┌─ 🔧 Hook: {name}");

                // Command
                let cmd_str = match &hook.command {
                    HookCommand::Shell(cmd) => format!("shell: {cmd}"),
                    HookCommand::Args(args) => format!("args: [{}]", args.join(", ")),
                };
                println!("│  Command: {cmd_str}");

                // Description
                if let Some(desc) = &hook.description {
                    println!("│  Description: {desc}");
                }

                // Safety flag
                let safety = if hook.modifies_repository {
                    "⚠️  MODIFIES REPOSITORY (runs sequentially)"
                } else {
                    "✅ Read-only (can run in parallel)"
                };
                println!("│  Safety: {safety}");

                // Execution type
                let exec_type = match hook.execution_type {
                    ExecutionType::PerFile => "per-file (files passed as arguments)",
                    ExecutionType::InPlace => "in-place (runs once without file args)",
                    ExecutionType::Other => "other (uses template variables)",
                };
                println!("│  Execution Type: {exec_type}");

                // File patterns
                if let Some(files) = &hook.files {
                    println!("│  File Patterns: [{}]", files.join(", "));
                } else if hook.run_always {
                    println!("│  File Patterns: ⚡ run_always=true (ignores file changes)");
                } else {
                    println!("│  File Patterns: (none - runs on any file change)");
                }

                // Working directory
                if let Some(workdir) = &hook.workdir {
                    println!("│  Working Directory: {workdir}");
                } else {
                    println!("│  Working Directory: (config file directory)");
                }

                // Run at root flag
                if hook.run_at_root {
                    println!("│  Run Location: 📁 Repository root");
                }

                // Dependencies
                if let Some(deps) = &hook.depends_on {
                    println!("│  Dependencies: [{}]", deps.join(", "));
                }

                // Environment variables
                if let Some(env) = &hook.env {
                    if !env.is_empty() {
                        println!("│  Environment Variables:");
                        let mut sorted_env: Vec<_> = env.iter().collect();
                        sorted_env.sort_by_key(|(k, _)| *k);
                        for (key, value) in sorted_env {
                            println!("│    {key} = {value}");
                        }
                    }
                }

                println!("└─────────────────────────────────────────────────────────\n");
            }
        }
    }

    // Print groups
    if let Some(groups) = &config.groups {
        if !groups.is_empty() {
            println!("📦 Hook Groups:\n");
            let mut sorted_groups: Vec<_> = groups.iter().collect();
            sorted_groups.sort_by_key(|(name, _)| *name);

            for (name, group) in sorted_groups {
                println!("┌─ 🎯 Group: {name}");

                // Description
                if let Some(desc) = &group.description {
                    println!("│  Description: {desc}");
                }

                // Placeholder flag
                if group.placeholder == Some(true) {
                    println!("│  Type: ⚙️  PLACEHOLDER (enables hierarchical resolution)");
                } else {
                    // Execution strategy
                    let strategy = group.get_execution_strategy();
                    let strategy_str = match strategy {
                        peter_hook::config::ExecutionStrategy::Sequential => "sequential",
                        peter_hook::config::ExecutionStrategy::Parallel => {
                            "parallel (safe - respects modifies_repository)"
                        }
                        peter_hook::config::ExecutionStrategy::ForceParallel => {
                            "force-parallel (⚠️  unsafe - ignores safety flags)"
                        }
                    };
                    println!("│  Execution Strategy: {strategy_str}");
                }

                // Includes
                if group.includes.is_empty() {
                    println!("│  Includes: (none)");
                } else {
                    println!("│  Includes: [{}]", group.includes.join(", "));
                }

                println!("└─────────────────────────────────────────────────────────\n");
            }
        }
    }

    println!("═══════════════════════════════════════════════════════════");
}

/// Run hooks in lint mode
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn run_lint_mode(hook_name: &str, dry_run: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current working directory")?;

    let resolver = HookResolver::new(&current_dir);

    if let Some(resolved_hooks) = resolver.resolve_hooks_for_lint(hook_name)? {
        if debug::is_enabled() && io::stdout().is_terminal() {
            println!("\x1b[38;5;201m🎪 \x1b[1m\x1b[38;5;51mPETER-HOOK LINT MODE!\x1b[0m");
            println!(
                "\x1b[38;5;198m📋 Config: \x1b[38;5;87m{}\x1b[0m",
                resolved_hooks.config_path.display()
            );
            println!(
                "\x1b[38;5;46m🎯 \x1b[1m\x1b[38;5;82mLinting with hook:\x1b[0m \
                 \x1b[38;5;226m{hook_name}\x1b[0m"
            );

            if let Some(ref all_files) = resolved_hooks.changed_files {
                println!(
                    "\x1b[38;5;214m📁 \x1b[1m\x1b[38;5;208mDiscovered {} files\x1b[0m",
                    all_files.len()
                );
                for (i, file) in all_files.iter().take(6).enumerate() {
                    let emoji = ["📄", "📝", "🔧", "⚙️", "🎨", "🚀"][i % 6];
                    println!(
                        "\x1b[38;5;147m    {} \x1b[38;5;183m{}\x1b[0m",
                        emoji,
                        file.display()
                    );
                }
                if all_files.len() > 6 {
                    println!(
                        "\x1b[38;5;147m    🌟 \x1b[38;5;105m... and {} more files!\x1b[0m",
                        all_files.len() - 6
                    );
                }
            }

            println!("\x1b[38;5;198m{}\x1b[0m", "═".repeat(60));
        } else if io::stdout().is_terminal() {
            println!("\n🎯 \x1b[1m\x1b[36mLint Mode:\x1b[0m \x1b[1m\x1b[33m{hook_name}\x1b[0m");
            println!("📂 \x1b[33m{}\x1b[0m", resolved_hooks.config_path.display());

            if let Some(ref all_files) = resolved_hooks.changed_files {
                println!("📁 \x1b[32m{}\x1b[0m files discovered", all_files.len());
            }

            if resolved_hooks.hooks.len() > 1 {
                println!(
                    "🔗 Resolves to \x1b[36m{}\x1b[0m hooks",
                    resolved_hooks.hooks.len()
                );
            }
            println!();
        } else {
            println!(
                "Lint mode: Running hook '{hook_name}' on {}",
                resolved_hooks.config_path.display()
            );
            if let Some(ref all_files) = resolved_hooks.changed_files {
                println!("Discovered {} files", all_files.len());
            }
        }

        // Handle dry-run mode
        if dry_run {
            if io::stdout().is_terminal() {
                println!("🔍 \x1b[1m\x1b[36mDry Run Mode\x1b[0m - showing what would execute:");

                for (name, hook) in &resolved_hooks.hooks {
                    let cmd_str = match &hook.definition.command {
                        HookCommand::Shell(cmd) => cmd.clone(),
                        HookCommand::Args(args) => args.join(" "),
                    };
                    println!("   🎯 \x1b[36m{name}\x1b[0m: \x1b[90m{cmd_str}\x1b[0m");
                    println!(
                        "      📂 Working dir: \x1b[90m{}\x1b[0m",
                        hook.working_directory.display()
                    );
                    if let Some(ref patterns) = hook.definition.files {
                        println!(
                            "      📄 File patterns: \x1b[90m{}\x1b[0m",
                            patterns.join(", ")
                        );
                    }
                }
            } else {
                println!(
                    "DRY RUN: Lint mode would run {} hooks",
                    resolved_hooks.hooks.len()
                );
                for (name, hook) in &resolved_hooks.hooks {
                    let cmd_str = match &hook.definition.command {
                        HookCommand::Shell(cmd) => cmd.clone(),
                        HookCommand::Args(args) => args.join(" "),
                    };
                    println!("  {name} - {cmd_str}");
                }
            }
            return Ok(());
        }

        let results = HookExecutor::execute(&resolved_hooks)
            .context("Failed to execute hooks in lint mode")?;

        if debug::is_enabled() && io::stdout().is_terminal() {
            println!("\x1b[38;5;198m{}\x1b[0m", "═".repeat(60));
            if results.success {
                println!(
                    "\x1b[38;5;46m🎊 \x1b[1m\x1b[38;5;82mLINT SUCCEEDED!\x1b[0m \
                     \x1b[38;5;46m🎊\x1b[0m"
                );
            } else {
                println!(
                    "\x1b[38;5;196m💥 \x1b[1m\x1b[38;5;199mLINT FAILED!\x1b[0m \
                     \x1b[38;5;196m💥\x1b[0m"
                );
                let failed = results.get_failed_hooks();
                println!(
                    "\x1b[38;5;197m🚨 Failed hooks: \x1b[38;5;167m{}\x1b[0m",
                    failed.join(", ")
                );
            }
            println!("\x1b[38;5;198m{}\x1b[0m", "═".repeat(60));
            results.print_summary();
        } else if !debug::is_enabled() && io::stdout().is_terminal() {
            if results.success {
                println!("🎉 Lint passed! All checks completed successfully!");
                println!(
                    "✅ \x1b[32m{}\x1b[0m hook{} completed successfully\n",
                    results.results.len(),
                    if results.results.len() == 1 { "" } else { "s" }
                );
            } else {
                println!("💥 \x1b[31mLint failed!\x1b[0m");
                let failed = results.get_failed_hooks();
                println!("❌ Failed: \x1b[31m{}\x1b[0m\n", failed.join(", "));
                results.print_summary();
            }
        } else {
            results.print_summary();
        }

        if !results.success {
            process::exit(1);
        }
    } else {
        if io::stdout().is_terminal() {
            println!("❌ \x1b[31mHook not found:\x1b[0m \x1b[1m{hook_name}\x1b[0m");
            println!(
                "💡 \x1b[36mTip:\x1b[0m Run \x1b[33mpeter-hook validate\x1b[0m to see available \
                 hooks"
            );
        } else {
            println!("No hook found with name: {hook_name}");
        }
        process::exit(1);
    }

    Ok(())
}

/// List all worktrees and their hook configuration
fn list_worktrees() -> Result<()> {
    let repo = GitRepository::find_from_current_dir().context("Failed to find git repository")?;

    let worktrees = repo.list_worktrees().context("Failed to list worktrees")?;

    if worktrees.is_empty() {
        println!("No worktrees found in this repository.");
        return Ok(());
    }

    println!("Git worktrees in this repository:");
    println!("=================================");

    for worktree in worktrees {
        let current_indicator = if worktree.is_current {
            " (current)"
        } else {
            ""
        };
        let main_indicator = if worktree.is_main { " [main]" } else { "" };

        println!(
            "📁 {}{}{}",
            worktree.name, main_indicator, current_indicator
        );
        println!("   Path: {}", worktree.path.display());

        // Check for hooks in this worktree
        let hooks_dir = if worktree.is_main {
            repo.get_common_hooks_dir().to_path_buf()
        } else {
            // For non-main worktrees, check both shared and worktree-specific locations
            let common_hooks = repo.get_common_hooks_dir();
            let worktree_hooks = worktree.path.join(".git/hooks");

            if worktree_hooks.exists() {
                worktree_hooks
            } else {
                common_hooks.to_path_buf()
            }
        };

        if hooks_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&hooks_dir) {
                let mut hook_files: Vec<_> = entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| {
                        let path = entry.path();
                        if path.is_file() {
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .filter(|name| !name.ends_with(".sample") && !name.starts_with('.'))
                                .map(ToString::to_string)
                        } else {
                            None
                        }
                    })
                    .collect();

                hook_files.sort();

                if hook_files.is_empty() {
                    println!("   Hooks: none");
                } else {
                    let hooks_type = if worktree.is_main || hooks_dir == repo.get_common_hooks_dir()
                    {
                        "shared"
                    } else {
                        "worktree-specific"
                    };
                    println!("   Hooks ({}): {}", hooks_type, hook_files.join(", "));
                }
            } else {
                println!("   Hooks: unable to read hooks directory");
            }
        } else {
            println!("   Hooks: none");
        }

        println!();
    }

    Ok(())
}

/// Handle global configuration management commands
fn handle_config_command(subcommand: &ConfigCommand) -> Result<()> {
    match subcommand {
        ConfigCommand::Show => show_global_config(),
        ConfigCommand::Init { force, allow_local } => init_global_config(*force, *allow_local),
        ConfigCommand::Validate => validate_global_config(),
    }
}

/// Show current global configuration
fn show_global_config() -> Result<()> {
    let config_path = GlobalConfig::config_path()?;

    if !config_path.exists() {
        println!(
            "No global configuration file found at: {}",
            config_path.display()
        );
        println!("Run 'peter-hook config init' to create one.");
        return Ok(());
    }

    let config = GlobalConfig::load()?;
    let content = toml::to_string_pretty(&config).context("Failed to serialize configuration")?;

    println!("Global configuration ({}):", config_path.display());
    println!("{content}");

    Ok(())
}

/// Initialize default global configuration file
fn init_global_config(force: bool, allow_local: bool) -> Result<()> {
    let config_path = GlobalConfig::config_path()?;

    if config_path.exists() && !force {
        println!(
            "Configuration file already exists: {}",
            config_path.display()
        );
        println!("Use --force to overwrite it.");
        return Ok(());
    }

    let mut config = GlobalConfig::default();
    config.security.allow_local = allow_local;
    config.save()?;

    println!("✓ Created global configuration: {}", config_path.display());
    println!();
    if allow_local {
        let local_dir = GlobalConfig::get_local_dir()?;
        println!("✓ Absolute imports enabled from: {}", local_dir.display());
        println!();
        println!("You can now use imports like:");
        println!(
            "  imports = [\"{}/<your-hooks>.toml\"]",
            local_dir.display()
        );
    } else {
        println!("ℹ  Absolute imports disabled (default)");
        println!("   Use --allow-local flag to enable imports from $HOME/.local/peter-hook");
    }

    Ok(())
}

/// Validate global configuration
fn validate_global_config() -> Result<()> {
    let config_path = GlobalConfig::config_path()?;

    if !config_path.exists() {
        println!("✓ No global configuration file (using defaults)");
        println!("  - allow_local: false (absolute imports disabled)");
        return Ok(());
    }

    let config = GlobalConfig::load().context("Failed to load global configuration")?;

    println!("✓ Global configuration is valid: {}", config_path.display());
    println!();

    if config.security.allow_local {
        let local_dir = GlobalConfig::get_local_dir()?;
        let exists = local_dir.exists();
        let status = if exists { "✓" } else { "?" };

        println!("Absolute imports: ✓ ENABLED");
        println!("  {} Local directory: {}", status, local_dir.display());

        if exists {
            // List .toml files in the directory
            if let Ok(entries) = std::fs::read_dir(&local_dir) {
                let toml_files: Vec<_> = entries
                    .filter_map(std::result::Result::ok)
                    .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("toml"))
                    .collect();

                if !toml_files.is_empty() {
                    println!("     Available hook files:");
                    for entry in toml_files {
                        println!("       - {}", entry.file_name().to_string_lossy());
                    }
                }
            }
        } else {
            println!("     (directory does not exist yet - will be created when needed)");
        }
    } else {
        println!("Absolute imports: ✗ DISABLED");
        println!("  Use 'peter-hook config init --allow-local' to enable");
    }

    Ok(())
}
