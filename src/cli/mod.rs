pub mod completions;

use clap::{Parser, Subcommand};

/// Command-line interface for peter hook manager
#[derive(Parser)]
#[command(name = "peter-hook")]
#[command(about = "A hierarchical git hooks manager for monorepos")]
#[command(disable_version_flag = true)]
pub struct Cli {
    /// Enable debug mode (verbose output with colors)
    #[arg(long, global = true)]
    pub debug: bool,

    /// Enable trace mode (show hook resolution decisions)
    #[arg(long, global = true)]
    pub trace: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Install hooks for the current repository
    Install {
        /// Force installation even if hooks already exist
        #[arg(long)]
        force: bool,
        /// Worktree hook installation strategy
        #[arg(long, default_value = "shared", value_parser = clap::builder::PossibleValuesParser::new(["shared", "per-worktree", "detect"]))]
        worktree_strategy: String,
    },
    /// Uninstall git-hook-manager managed hooks
    Uninstall {
        /// Remove hooks without prompting for confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Run hooks for a specific git event
    Run {
        /// The git hook event (pre-commit, pre-push, etc.)
        event: String,
        /// Run on all files instead of only changed files
        #[arg(long)]
        all_files: bool,
        /// Show what would run without executing hooks
        #[arg(long)]
        dry_run: bool,
        /// Additional arguments passed from git (e.g., commit message file,
        /// refs)
        #[arg(trailing_var_arg = true)]
        git_args: Vec<String>,
    },
    /// Validate hook configuration
    Validate {
        /// Trace imports and show merge/override diagnostics
        #[arg(long)]
        trace_imports: bool,
        /// Output diagnostics as JSON (use with --trace-imports)
        #[arg(long)]
        json: bool,
    },
    /// List installed git hooks
    List,
    /// List worktrees and their hook configuration
    ListWorktrees,
    /// Manage global configuration
    Config {
        /// Configuration management subcommand
        #[command(subcommand)]
        subcommand: ConfigCommand,
    },
    /// Run hooks in lint mode (current directory as root, all matching files)
    Lint {
        /// Name of the hook or group to run
        hook_name: String,
        /// Show what would run without executing hooks
        #[arg(long)]
        dry_run: bool,
    },
    /// Show version information
    Version,
    /// Show license information
    License,
    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, etc.)
        shell: clap_complete::Shell,
    },
    /// Internal helper: list run targets for shell completions
    #[command(name = "_run-targets", hide = true)]
    RunTargets,
    /// Internal helper: list lint targets for shell completions
    #[command(name = "_lint-targets", hide = true)]
    LintTargets,
    /// Check health and configuration
    Doctor,
}

/// Configuration management subcommands
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Show current global configuration
    Show,
    /// Initialize default global configuration file
    Init {
        /// Overwrite existing configuration file
        #[arg(long)]
        force: bool,
        /// Enable imports from $HOME/.local/peter-hook
        #[arg(long)]
        allow_local: bool,
    },
    /// Validate current configuration and check allowlist
    Validate,
}
