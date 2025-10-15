//! Shell completion generation with dynamic run/lint targets.

use clap::CommandFactory;
use clap_complete::Shell;
use std::io::{self, Write};

use super::Cli;

/// Generate shell completions with dynamic target support for `run` and `lint`.
pub fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();

    print_instructions(shell, &bin_name);

    let mut buffer = Vec::new();
    clap_complete::generate(shell, &mut cmd, &bin_name, &mut buffer);

    let script = String::from_utf8(buffer).expect("clap_complete generated invalid UTF-8");
    let augmented = augment_script(shell, &bin_name, script);

    let mut stdout = io::stdout().lock();
    stdout
        .write_all(augmented.as_bytes())
        .expect("failed to write completions");
}

fn print_instructions(shell: Shell, bin_name: &str) {
    println!("# Shell completion for {bin_name}");
    println!("#");
    println!("# To enable completions, add this to your shell config:");
    println!("#");

    match shell {
        Shell::Bash => {
            println!("# For bash (~/.bashrc):");
            println!("#   source <({bin_name} completions bash)");
        }
        Shell::Zsh => {
            println!("# For zsh (~/.zshrc):");
            println!("#   {bin_name} completions zsh > ~/.zsh/completions/_{bin_name}");
            println!("#   # Ensure fpath includes ~/.zsh/completions");
        }
        Shell::Fish => {
            println!("# For fish (~/.config/fish/config.fish):");
            println!("#   {bin_name} completions fish | source");
        }
        _ => {
            println!("# For {shell}:");
            println!("#   {bin_name} completions {shell} > /path/to/completions/_{bin_name}");
        }
    }

    println!();
}

fn augment_script(shell: Shell, bin_name: &str, script: String) -> String {
    match shell {
        Shell::Bash => augment_bash(bin_name, script),
        Shell::Zsh => augment_zsh(bin_name, script),
        Shell::Fish => augment_fish(bin_name, script),
        _ => script,
    }
}

fn augment_bash(bin_name: &str, script: String) -> String {
    let mut updated = script;

    replace_case_block(
        &mut updated,
        "        peter__hook__run)",
        &format!(
            concat!(
                "        peter__hook__run)\n",
                "            opts=\"-h --all-files --dry-run --debug --help <EVENT> [GIT_ARGS]...\"\n",
                "            if [[ ${{cur}} == -* ]]; then\n",
                "                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )\n",
                "                return 0\n",
                "            fi\n",
                "            if [[ ${{COMP_CWORD}} -eq 2 ]]; then\n",
                "                local targets\n",
                "                targets=\"$({bin_name} _run-targets)\"\n",
                "                COMPREPLY=( $(compgen -W \"${{targets}}\" -- \"${{cur}}\") )\n",
                "                return 0\n",
                "            fi\n",
                "            COMPREPLY=()\n",
                "            return 0\n",
                "            ;;\n"
            ),
            bin_name = bin_name
        ),
    );

    replace_case_block(
        &mut updated,
        "        peter__hook__lint)",
        &format!(
            concat!(
                "        peter__hook__lint)\n",
                "            opts=\"-h --dry-run --debug --help <HOOK_NAME>\"\n",
                "            if [[ ${{cur}} == -* ]]; then\n",
                "                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )\n",
                "                return 0\n",
                "            fi\n",
                "            if [[ ${{COMP_CWORD}} -eq 2 ]]; then\n",
                "                local targets\n",
                "                targets=\"$({bin_name} _lint-targets)\"\n",
                "                COMPREPLY=( $(compgen -W \"${{targets}}\" -- \"${{cur}}\") )\n",
                "                return 0\n",
                "            fi\n",
                "            COMPREPLY=()\n",
                "            return 0\n",
                "            ;;\n"
            ),
            bin_name = bin_name
        ),
    );

    let helper = format!(
        concat!(
            "\n_{sanitized}_list_run_targets() {{\n",
            "    {bin_name} _run-targets\n",
            "}}\n",
            "\n_{sanitized}_list_lint_targets() {{\n",
            "    {bin_name} _lint-targets\n",
            "}}\n"
        ),
        sanitized = bin_name.replace('-', "_"),
        bin_name = bin_name
    );

    updated.push_str(&helper);
    updated
}

fn augment_zsh(bin_name: &str, script: String) -> String {
    let sanitized = bin_name.replace('-', "_");
    let run_func = format!("_{sanitized}_run_events");
    let lint_func = format!("_{sanitized}_lint_targets");

    let mut updated = script.replace(
        ":event -- The git hook event (pre-commit, pre-push, etc.):_default",
        format!(
            ":event -- The git hook event (pre-commit, pre-push, etc.):{run}",
            run = run_func
        )
        .as_str(),
    );

    updated = updated.replace(
        ":hook_name -- Name of the hook or group to run:_default",
        format!(
            ":hook_name -- Name of the hook or group to run:{lint}",
            lint = lint_func
        )
        .as_str(),
    );

    let helper_template = r#"
__RUN__() {
    local -a targets
    targets=(${(@f)$(__BIN__ _run-targets)})
    if (( $#targets )); then
        _describe 'git hook events' targets
    else
        _message 'no hook events found'
    fi
}

__LINT__() {
    local -a targets
    targets=(${(@f)$(__BIN__ _lint-targets)})
    if (( $#targets )); then
        _describe 'hooks or groups' targets
    else
        _message 'no hooks found'
    fi
}
"#;

    let helper = helper_template
        .replace("__RUN__", &run_func)
        .replace("__LINT__", &lint_func)
        .replace("__BIN__", bin_name);

    updated.push_str(&helper);
    updated
}

fn augment_fish(bin_name: &str, script: String) -> String {
    let mut updated = script;
    let sanitized = bin_name.replace('-', "_");
    let helper = format!(
        concat!(
            "\nfunction __fish_{sanitized}_run_targets\n",
            "    {bin_name} _run-targets\n",
            "end\n",
            "\nfunction __fish_{sanitized}_lint_targets\n",
            "    {bin_name} _lint-targets\n",
            "end\n",
            "\nfunction __fish_{sanitized}_run_needs_event\n",
            "    set -l tokens (commandline -opc)\n",
            "    if test (count $tokens) -eq 0\n",
            "        return 1\n",
            "    end\n",
            "    set -e tokens[1]\n",
            "    set -l found 0\n",
            "    for token in $tokens\n",
            "        switch $token\n",
            "        case 'run'\n",
            "            set found 1\n",
            "            continue\n",
            "        case '-*'\n",
            "            continue\n",
            "        default\n",
            "            if test $found -eq 1\n",
            "                return 1\n",
            "            end\n",
            "        end\n",
            "    end\n",
            "    test $found -eq 1\n",
            "end\n",
            "\nfunction __fish_{sanitized}_lint_needs_target\n",
            "    set -l tokens (commandline -opc)\n",
            "    if test (count $tokens) -eq 0\n",
            "        return 1\n",
            "    end\n",
            "    set -e tokens[1]\n",
            "    set -l found 0\n",
            "    for token in $tokens\n",
            "        switch $token\n",
            "        case 'lint'\n",
            "            set found 1\n",
            "            continue\n",
            "        case '-*'\n",
            "            continue\n",
            "        default\n",
            "            if test $found -eq 1\n",
            "                return 1\n",
            "            end\n",
            "        end\n",
            "    end\n",
            "    test $found -eq 1\n",
            "end\n",
            "\ncomplete -c {bin_name} -n \"__fish_{sanitized}_using_subcommand run; and __fish_{sanitized}_run_needs_event\" -f -a \"(__fish_{sanitized}_run_targets)\"\n",
            "complete -c {bin_name} -n \"__fish_{sanitized}_using_subcommand lint; and __fish_{sanitized}_lint_needs_target\" -f -a \"(__fish_{sanitized}_lint_targets)\"\n"
        ),
        bin_name = bin_name,
        sanitized = sanitized
    );

    updated.push_str(&helper);
    updated
}

fn replace_case_block(script: &mut String, marker: &str, replacement: &str) {
    if let Some(start) = script.find(marker) {
        if let Some(rel_end) = script[start..].find("\n            ;;") {
            let end = start + rel_end + "\n            ;;".len();
            script.replace_range(start..end, replacement);
        }
    }
}
