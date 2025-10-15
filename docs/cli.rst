CLI Reference
=============

Usage
-----

.. code-block:: text

   peter-hook [--debug] <COMMAND> [OPTIONS]

Global Options
--------------

- ``--debug``: Enable debug mode with verbose output and colorful diagnostic messages

Commands
--------

install
^^^^^^^

Install hooks for the current repository. Creates managed shell scripts under ``.git/hooks`` that delegate to ``peter-hook``.

Options:

- ``--force``: Backup existing non-managed hooks and install anyway
- ``--worktree-strategy``: Worktree hook installation strategy (shared, per-worktree, detect)

uninstall
^^^^^^^^^

Remove peter-hook managed hooks. Restores backups when present.

Options:

- ``--yes``: Do not prompt for confirmation

run
^^^

Run hooks for a specific git event (e.g., ``pre-commit``, ``pre-push``, ``commit-msg``).

Positional:

- ``event``: Git hook event

Options:

- ``--all-files``: Run on all files instead of only changed files
- ``--dry-run``: Show what would run without executing hooks
- ``git_args``: Additional arguments passed from git

validate
^^^^^^^^

Parse and validate the nearest ``hooks.toml``. Prints discovered hooks and groups.

Options:

- ``--trace-imports``: Show import order, overrides, cycles, and unused imports
- ``--json``: Output diagnostics as JSON (use with ``--trace-imports``)


list
^^^^

List installed hooks in ``.git/hooks`` and show whether they are managed by peter-hook.

lint
^^^^

Run hooks in lint mode. Treats the current directory as the repository root and runs hooks on all matching files (not just changed files), respecting .gitignore rules.

Positional:

- ``hook_name``: Name of the hook or group to run

Options:

- ``--dry-run``: Show what would run without executing hooks

list-worktrees
^^^^^^^^^^^^^^

List worktrees and their hook configuration.

config
^^^^^^

Manage global configuration.

Subcommands:

- ``show``: Show current global configuration
- ``init``: Initialize global configuration

  - ``--allow-local``: Enable imports from ``$HOME/.local/peter-hook``
  - ``--force``: Overwrite existing configuration file

- ``validate``: Validate global configuration

version
^^^^^^^

Show version information.

license
^^^^^^^

Show license information for peter-hook and its dependencies.

completions
^^^^^^^^^^^

Generate shell completion scripts.

Positional:

- ``shell``: Shell type (bash, zsh, fish, powershell, elvish)

The generated scripts now include dynamic completions for key verbs:

- ``run`` suggests only git hook events that peter-hook knows how to install.
- ``lint`` suggests hook and group names discovered from the nearest ``hooks.toml`` (placeholder groups are skipped).

.. note::

   The completion scripts call back into ``peter-hook`` using internal helper
   subcommands (``_run-targets`` and ``_lint-targets``). They are hidden from
   normal help output and meant solely for powering completions.

Usage example:

.. code-block:: bash

   # Install completions for bash
   peter-hook completions bash > /etc/bash_completion.d/peter-hook

   # Install completions for zsh
   peter-hook completions zsh > ~/.zsh/completion/_peter-hook

doctor
^^^^^^

Run health checks and configuration validation. Checks for:

- Repository git configuration
- Hook installation status
- Configuration file validity
- Available updates

update
^^^^^^

Update peter-hook to the latest version (or a specific version).

Positional:

- ``version``: Specific version to install (optional, defaults to latest)

Options:

- ``--force``: Force update even if already up-to-date
- ``--install-dir <PATH>``: Custom installation directory

Usage example:

.. code-block:: bash

   # Update to latest version
   peter-hook update

   # Update to specific version
   peter-hook update 1.5.0

   # Force reinstall current version
   peter-hook update --force
