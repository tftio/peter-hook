Configuration
=============

Peter Hook reads configuration from the nearest ``.peter-hook.toml`` file to the current working directory. Child directories override parent configurations: the nearest file wins.

Hook Definition
---------------

.. code-block:: toml

   [hooks.example]
   command = "echo hello"                   # string or array form
   # command = ["echo", "hello"]          # preferred for complex commands
   description = "Example hook"             # optional description
   modifies_repository = false              # true -> runs sequentially (required)
   execution_type = "per-file"              # how files are passed: per-file | in-place | other
   workdir = "custom/path"                  # optional working directory (relative or absolute)
   env = { KEY = "value" }                  # environment variables (supports templates)
   files = ["**/*.rs", "Cargo.toml"]       # glob patterns for file targeting
   depends_on = ["format", "setup"]        # hook dependencies
   run_always = false                       # ignore file changes when true (incompatible with files)
   run_at_root = false                      # run at repository root instead of config directory

Execution Types
---------------

The ``execution_type`` field controls how changed files are passed to hook commands. There are three modes:

**per-file** (default)
  Files are passed as individual command-line arguments to the hook command.

  .. code-block:: toml

     [hooks.eslint]
     command = "eslint"
     execution_type = "per-file"  # default
     modifies_repository = false
     files = ["**/*.js"]

  **Runs:** ``cd /config/dir && eslint file1.js file2.js file3.js``

  **Use for:** Standard linters/formatters that accept file lists (eslint, ruff, prettier)

**in-place**
  Runs once in the configuration directory without passing file arguments. The tool auto-discovers files.

  .. code-block:: toml

     [hooks.pytest]
     command = "pytest"
     execution_type = "in-place"
     modifies_repository = false
     files = ["**/*.py"]

  **Runs:** ``cd /config/dir && pytest`` (pytest discovers test files itself)

  **Use for:** Test runners (pytest, jest, cargo test), directory scanners that find files themselves

**other**
  Hook uses template variables for manual file handling (see :doc:`templating`).

  .. code-block:: toml

     [hooks.custom]
     command = "my-tool {CHANGED_FILES}"
     execution_type = "other"
     modifies_repository = false
     files = ["**/*.rs"]

  **Runs:** ``cd /config/dir && my-tool file1.rs file2.rs``

  **Use for:** Custom scripts, complex pipelines, non-standard file argument patterns

Working Directory Control
--------------------------

By default, hooks run in the directory containing their ``.peter-hook.toml`` file. Use ``run_at_root = true`` to override this and run at the repository root instead:

.. code-block:: toml

   [hooks.build]
   command = "make build"
   modifies_repository = true
   run_at_root = true  # runs at repository root, not config directory

Hook Groups
-----------

Imports (Hook Libraries)
------------------------

You can split reusable hooks/groups into separate TOML files and import them into your project ``.peter-hook.toml``. Use ``peter-hook validate --trace-imports`` to inspect how imports were resolved, any overrides, cycles that were skipped, and unused imports. Add ``--json`` to emit machine-readable diagnostics.

.. code-block:: toml

   # .peter-hook.toml
   imports = ["../hooks.lib.toml", ".hooks/common.toml"]

   [groups.pre-commit]
   includes = ["lint", "format", "test"]  # names from imported files

   # Local override wins on same name
   [hooks.lint]
   command = "cargo clippy -- -D warnings"
   modifies_repository = false

Rules:

- Paths must be relative to the importing file
- Absolute imports are only allowed from ``$HOME/.local/peter-hook`` when enabled via ``peter-hook config init --allow-local``
- Imported files must be located under the git repository root (or in the allowed local directory)
- Imports merge in order; later imports override earlier ones on duplicate names
- Local definitions override imported ones
- Recursive imports are supported with cycle detection

.. code-block:: toml

   [groups.example-group]
   includes = ["hook1", "hook2", "other-group"]
   execution = "parallel"               # sequential | parallel | force-parallel
   description = "Example group"
   # parallel = true                     # deprecated; kept for backward-compat

Execution Strategies
--------------------

- ``sequential``: run hooks one after another, respecting dependencies
- ``parallel``: run read-only hooks together; repository-modifying hooks run after, sequentially
- ``force-parallel``: run all hooks in parallel (unsafe; ignores ``modifies_repository``)

Repository Safety Rules
-----------------------

- Hooks with ``modifies_repository = true`` never run in parallel with other hooks
- Hooks with ``modifies_repository = false`` can run in parallel with each other
- Mixed groups run in phases: safe hooks first (parallel), then modifying hooks (sequential)

