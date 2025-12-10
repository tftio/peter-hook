Quickstart
==========

1. Create ``.peter-hook.toml`` in your repo (or subdirectory):

.. code-block:: toml

   [hooks.lint]
   command = "cargo clippy --all-targets -- -D warnings"
   description = "Run Clippy linter"
   modifies_repository = false

   [hooks.test]
   command = "cargo test --all"
   description = "Run test suite"
   modifies_repository = false

   [hooks.format]
   command = "cargo fmt"
   description = "Format code"
   modifies_repository = true

   [groups.pre-commit]
   includes = ["lint", "test", "format"]
   execution = "parallel"
   description = "Safe hooks run in parallel; format runs after"

2. Validate your configuration:

.. code-block:: bash

   peter-hook validate

3. Run hooks manually:

.. code-block:: bash

   peter-hook run pre-commit              # run on changed files (if hooks have 'files' patterns)
   peter-hook run pre-commit --all-files  # run on all matching files instead of only changed files
   peter-hook run pre-commit --dry-run    # show what would run without executing

4. Install git hooks (optional):

.. code-block:: bash

   peter-hook install

