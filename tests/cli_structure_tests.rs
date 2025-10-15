#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Tests for CLI structure and command definitions

use clap::{CommandFactory, Parser};
use peter_hook::cli::{Cli, Commands, ConfigCommand};

#[test]
fn test_cli_has_all_subcommands() {
    let cmd = Cli::command();

    // Verify all expected visible subcommands are present
    let subcommands: Vec<_> = cmd
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set())
        .map(clap::Command::get_name)
        .collect();

    assert!(
        subcommands.contains(&"install"),
        "Missing 'install' subcommand"
    );
    assert!(
        subcommands.contains(&"uninstall"),
        "Missing 'uninstall' subcommand"
    );
    assert!(subcommands.contains(&"run"), "Missing 'run' subcommand");
    assert!(
        subcommands.contains(&"validate"),
        "Missing 'validate' subcommand"
    );
    assert!(subcommands.contains(&"list"), "Missing 'list' subcommand");
    assert!(
        subcommands.contains(&"list-worktrees"),
        "Missing 'list-worktrees' subcommand"
    );
    assert!(
        subcommands.contains(&"config"),
        "Missing 'config' subcommand"
    );
    assert!(subcommands.contains(&"lint"), "Missing 'lint' subcommand");
    assert!(
        subcommands.contains(&"version"),
        "Missing 'version' subcommand"
    );
    assert!(
        subcommands.contains(&"license"),
        "Missing 'license' subcommand"
    );
    assert!(
        subcommands.contains(&"completions"),
        "Missing 'completions' subcommand"
    );
    assert!(
        subcommands.contains(&"doctor"),
        "Missing 'doctor' subcommand"
    );
    assert!(
        subcommands.contains(&"update"),
        "Missing 'update' subcommand"
    );

    // Should have exactly 13 visible subcommands
    assert_eq!(
        subcommands.len(),
        13,
        "Expected 13 visible subcommands, got {}",
        subcommands.len()
    );
}

#[test]
fn test_cli_name_and_about() {
    let cmd = Cli::command();

    assert_eq!(cmd.get_name(), "peter-hook");
    assert!(cmd.get_about().is_some());
    let about = cmd.get_about().unwrap().to_string();
    assert!(about.contains("hierarchical") || about.contains("git hooks"));
}

#[test]
fn test_hidden_completion_helpers_exist() {
    let cmd = Cli::command();
    let run_targets = cmd
        .get_subcommands()
        .find(|sub| sub.get_name() == "_run-targets")
        .expect("Missing _run-targets helper");
    assert!(run_targets.is_hide_set(), "_run-targets should be hidden");

    let lint_targets = cmd
        .get_subcommands()
        .find(|sub| sub.get_name() == "_lint-targets")
        .expect("Missing _lint-targets helper");
    assert!(lint_targets.is_hide_set(), "_lint-targets should be hidden");
}

#[test]
fn test_cli_has_debug_flag() {
    let cmd = Cli::command();

    let debug_arg = cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("debug"));

    assert!(debug_arg.is_some(), "Missing --debug flag");
    assert!(
        debug_arg.unwrap().is_global_set(),
        "--debug should be global"
    );
}

#[test]
fn test_install_command_has_required_args() {
    let cmd = Cli::command();
    let install_cmd = cmd
        .find_subcommand("install")
        .expect("install subcommand not found");

    // Check for force flag
    let force_arg = install_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("force"));
    assert!(force_arg.is_some(), "Missing --force flag");

    // Check for worktree-strategy flag
    let strategy_arg = install_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("worktree-strategy"));
    assert!(strategy_arg.is_some(), "Missing --worktree-strategy flag");
}

#[test]
fn test_uninstall_command_has_yes_flag() {
    let cmd = Cli::command();
    let uninstall_cmd = cmd
        .find_subcommand("uninstall")
        .expect("uninstall subcommand not found");

    let yes_arg = uninstall_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("yes"));
    assert!(yes_arg.is_some(), "Missing --yes flag");
}

#[test]
fn test_run_command_structure() {
    let cmd = Cli::command();
    let run_cmd = cmd
        .find_subcommand("run")
        .expect("run subcommand not found");

    // Should have event positional argument
    let has_event_arg = run_cmd
        .get_positionals()
        .any(|arg| arg.get_id().as_str() == "event");
    assert!(has_event_arg, "Missing 'event' positional argument");

    // Should have --all-files flag
    let all_files_arg = run_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("all-files"));
    assert!(all_files_arg.is_some(), "Missing --all-files flag");

    // Should have --dry-run flag
    let dry_run_arg = run_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("dry-run"));
    assert!(dry_run_arg.is_some(), "Missing --dry-run flag");
}

#[test]
fn test_validate_command_structure() {
    let cmd = Cli::command();
    let validate_cmd = cmd
        .find_subcommand("validate")
        .expect("validate subcommand not found");

    // Should have --trace-imports flag
    let trace_arg = validate_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("trace-imports"));
    assert!(trace_arg.is_some(), "Missing --trace-imports flag");

    // Should have --json flag
    let json_arg = validate_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("json"));
    assert!(json_arg.is_some(), "Missing --json flag");
}

#[test]
fn test_config_command_has_subcommands() {
    let cmd = Cli::command();
    let config_cmd = cmd
        .find_subcommand("config")
        .expect("config subcommand not found");

    let subcommands: Vec<_> = config_cmd
        .get_subcommands()
        .map(clap::Command::get_name)
        .collect();

    assert!(
        subcommands.contains(&"show"),
        "Missing 'show' subcommand under config"
    );
    assert!(
        subcommands.contains(&"init"),
        "Missing 'init' subcommand under config"
    );
    assert!(
        subcommands.contains(&"validate"),
        "Missing 'validate' subcommand under config"
    );
}

#[test]
fn test_config_init_has_force_and_allow_local_flags() {
    let cmd = Cli::command();
    let config_cmd = cmd
        .find_subcommand("config")
        .expect("config subcommand not found");
    let init_cmd = config_cmd
        .find_subcommand("init")
        .expect("init subcommand not found");

    let force_arg = init_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("force"));
    assert!(force_arg.is_some(), "Missing --force flag in config init");

    let allow_local_arg = init_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("allow-local"));
    assert!(
        allow_local_arg.is_some(),
        "Missing --allow-local flag in config init"
    );
}

#[test]
fn test_lint_command_structure() {
    let cmd = Cli::command();
    let lint_cmd = cmd
        .find_subcommand("lint")
        .expect("lint subcommand not found");

    // Should have hook_name positional argument
    let has_hook_name = lint_cmd
        .get_positionals()
        .any(|arg| arg.get_id().as_str() == "hook_name");
    assert!(has_hook_name, "Missing 'hook_name' positional argument");

    // Should have --dry-run flag
    let dry_run_arg = lint_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("dry-run"));
    assert!(dry_run_arg.is_some(), "Missing --dry-run flag in lint");
}

#[test]
fn test_completions_command_has_shell_arg() {
    let cmd = Cli::command();
    let completions_cmd = cmd
        .find_subcommand("completions")
        .expect("completions subcommand not found");

    // Should have shell positional argument
    let has_shell = completions_cmd
        .get_positionals()
        .any(|arg| arg.get_id().as_str() == "shell");
    assert!(has_shell, "Missing 'shell' positional argument");
}

#[test]
fn test_update_command_structure() {
    let cmd = Cli::command();
    let update_cmd = cmd
        .find_subcommand("update")
        .expect("update subcommand not found");

    // Should have optional version positional
    let has_version = update_cmd
        .get_positionals()
        .any(|arg| arg.get_id().as_str() == "version");
    assert!(has_version, "Missing 'version' positional argument");

    // Should have --force flag
    let force_arg = update_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("force"));
    assert!(force_arg.is_some(), "Missing --force flag in update");

    // Should have --install-dir flag
    let install_dir_arg = update_cmd
        .get_arguments()
        .find(|arg| arg.get_long() == Some("install-dir"));
    assert!(
        install_dir_arg.is_some(),
        "Missing --install-dir flag in update"
    );
}

#[test]
fn test_version_and_license_commands_have_no_args() {
    let cmd = Cli::command();

    let version_cmd = cmd
        .find_subcommand("version")
        .expect("version subcommand not found");
    assert_eq!(
        version_cmd.get_arguments().count(),
        0,
        "version command should have no arguments"
    );

    let license_cmd = cmd
        .find_subcommand("license")
        .expect("license subcommand not found");
    assert_eq!(
        license_cmd.get_arguments().count(),
        0,
        "license command should have no arguments"
    );
}

#[test]
fn test_list_and_list_worktrees_have_no_args() {
    let cmd = Cli::command();

    let list_cmd = cmd
        .find_subcommand("list")
        .expect("list subcommand not found");
    assert_eq!(
        list_cmd.get_arguments().count(),
        0,
        "list command should have no arguments"
    );

    let list_worktrees_cmd = cmd
        .find_subcommand("list-worktrees")
        .expect("list-worktrees subcommand not found");
    assert_eq!(
        list_worktrees_cmd.get_arguments().count(),
        0,
        "list-worktrees command should have no arguments"
    );
}

#[test]
fn test_doctor_command_has_no_args() {
    let cmd = Cli::command();
    let doctor_cmd = cmd
        .find_subcommand("doctor")
        .expect("doctor subcommand not found");
    assert_eq!(
        doctor_cmd.get_arguments().count(),
        0,
        "doctor command should have no arguments"
    );
}

#[test]
fn test_cli_parsing_install_with_flags() {
    // Test parsing install command with flags
    let result = Cli::try_parse_from([
        "peter-hook",
        "install",
        "--force",
        "--worktree-strategy",
        "per-worktree",
    ]);
    assert!(result.is_ok(), "Failed to parse install with flags");

    if let Commands::Install {
        force,
        worktree_strategy,
    } = result.unwrap().command
    {
        assert!(force);
        assert_eq!(worktree_strategy, "per-worktree");
    } else {
        panic!("Expected Install command");
    }
}

#[test]
fn test_cli_parsing_run_with_args() {
    let result = Cli::try_parse_from([
        "peter-hook",
        "run",
        "pre-commit",
        "--all-files",
        "--dry-run",
        "extra",
        "args",
    ]);
    assert!(result.is_ok(), "Failed to parse run with args");

    if let Commands::Run {
        event,
        all_files,
        dry_run,
        git_args,
    } = result.unwrap().command
    {
        assert_eq!(event, "pre-commit");
        assert!(all_files);
        assert!(dry_run);
        assert_eq!(git_args, vec!["extra", "args"]);
    } else {
        panic!("Expected Run command");
    }
}

#[test]
fn test_cli_parsing_config_subcommands() {
    // Test config show
    let result = Cli::try_parse_from(["peter-hook", "config", "show"]);
    assert!(result.is_ok());
    if let Commands::Config { subcommand } = result.unwrap().command {
        assert!(matches!(subcommand, ConfigCommand::Show));
    } else {
        panic!("Expected Config command");
    }

    // Test config init
    let result = Cli::try_parse_from(["peter-hook", "config", "init", "--force", "--allow-local"]);
    assert!(result.is_ok());
    if let Commands::Config { subcommand } = result.unwrap().command {
        if let ConfigCommand::Init { force, allow_local } = subcommand {
            assert!(force);
            assert!(allow_local);
        } else {
            panic!("Expected Init subcommand");
        }
    } else {
        panic!("Expected Config command");
    }
}

#[test]
fn test_cli_parsing_with_debug_flag() {
    let result = Cli::try_parse_from(["peter-hook", "--debug", "version"]);
    assert!(result.is_ok());
    let cli = result.unwrap();
    assert!(cli.debug, "Debug flag should be true");
    assert!(matches!(cli.command, Commands::Version));
}

#[test]
fn test_cli_invalid_worktree_strategy() {
    let result = Cli::try_parse_from(["peter-hook", "install", "--worktree-strategy", "invalid"]);
    assert!(result.is_err(), "Should reject invalid worktree strategy");
}
