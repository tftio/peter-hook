Templating
==========

Template variables are available in ``command``, ``workdir``, and ``env`` fields. They are resolved at runtime using repository context and predefined variables.

Built-in Variables
------------------

- ``{HOOK_DIR}``: Directory containing the ``.peter-hook.toml``
- ``{WORKING_DIR}``: Working directory when the hook runs
- ``{REPO_ROOT}``: Git repository root
- ``{HOOK_DIR_REL}``: ``HOOK_DIR`` relative to repo root
- ``{WORKING_DIR_REL}``: ``WORKING_DIR`` relative to repo root
- ``{PROJECT_NAME}``: Name of directory containing ``.peter-hook.toml``
- ``{HOME_DIR}``: User home directory
- ``{PATH}``: Current PATH environment variable (useful for extending PATH)
- ``{IS_WORKTREE}``: "true" or "false" - whether running in a worktree
- ``{WORKTREE_NAME}``: Name of current worktree (only available in worktrees)
- ``{COMMON_DIR}``: Path to shared git directory (across worktrees)
- ``{CHANGED_FILES}``: Space-delimited list of changed files (with --files)
- ``{CHANGED_FILES_LIST}``: Newline-delimited list of changed files (with --files)
- ``{CHANGED_FILES_FILE}``: Path to temp file containing changed files (with --files)

Security Note
-------------

For security reasons, only the predefined template variables listed above are available. Arbitrary environment variables are not exposed to prevent potential security vulnerabilities.

Examples
--------

.. code-block:: toml

   [hooks.build]
   command = "cargo build --manifest-path={HOOK_DIR}/Cargo.toml"
   workdir = "{REPO_ROOT}/target"
   env = { CARGO_TARGET_DIR = "{HOOK_DIR}/target", PROJECT_NAME = "{PROJECT_NAME}" }

   [hooks.test-with-context]
   command = ["cargo", "test", "--manifest-path={HOOK_DIR}/Cargo.toml", "--", "--test-threads=1"]
   description = "Test {PROJECT_NAME} in {HOOK_DIR_REL}"

   [hooks.changed-files-example]
   command = "ruff check {CHANGED_FILES}"
   description = "Run linter on changed files"
   files = ["**/*.py"]

   # Extending PATH to include custom tool directory
   [hooks.custom-tool]
   command = "my-tool --check"
   modifies_repository = false
   env = { PATH = "{HOME_DIR}/.local/bin:{PATH}" }

   # Alternative: use absolute path directly
   [hooks.custom-tool-direct]
   command = "{HOME_DIR}/.local/bin/my-tool --check"
   modifies_repository = false
