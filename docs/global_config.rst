Global Configuration
====================

Peter Hook supports global user-wide configuration stored in ``~/.config/peter-hook/config.toml``. This configuration controls security settings and system-wide behavior.

Configuration File Location
----------------------------

The global configuration file is stored at:

.. code-block:: text

   ~/.config/peter-hook/config.toml

This file is optional. If it doesn't exist, peter-hook uses default settings with maximum security restrictions.

Configuration Structure
-----------------------

.. code-block:: toml

   [security]
   allow_local = false  # Enable imports from ~/.local/peter-hook

Security Settings
-----------------

allow_local
^^^^^^^^^^^

Controls whether hooks can be imported from the shared local directory ``$HOME/.local/peter-hook``.

**Default:** ``false``

**Purpose:** When enabled, allows you to maintain a personal library of reusable hooks that can be imported with absolute paths across all your repositories.

**Location:** ``$HOME/.local/peter-hook/``

**Example:**

.. code-block:: toml

   # ~/.config/peter-hook/config.toml
   [security]
   allow_local = true

With this setting enabled, you can create shared hooks in ``~/.local/peter-hook/`` and import them:

.. code-block:: toml

   # Any repository's .peter-hook.toml
   imports = ["/home/user/.local/peter-hook/common-.peter-hook.toml"]

   [hooks.format]
   command = "cargo fmt"
   modifies_repository = true

**Security:** Absolute imports are ONLY allowed from ``$HOME/.local/peter-hook``. All other absolute paths are rejected. Additionally, symlink attacks are prevented through path canonicalization.

Managing Global Configuration
------------------------------

Initialize Configuration
^^^^^^^^^^^^^^^^^^^^^^^^

Create the default global configuration file:

.. code-block:: bash

   # Create default config (allow_local = false)
   peter-hook config init

   # Create config with local imports enabled
   peter-hook config init --allow-local

   # Overwrite existing config
   peter-hook config init --force

View Configuration
^^^^^^^^^^^^^^^^^^

Display the current global configuration:

.. code-block:: bash

   peter-hook config show

Output example:

.. code-block:: toml

   [security]
   allow_local = false

Validate Configuration
^^^^^^^^^^^^^^^^^^^^^^

Check if the global configuration file is valid:

.. code-block:: bash

   peter-hook config validate

This command verifies:

- Configuration file syntax (valid TOML)
- All required fields are present
- Field values are valid

Using the Shared Local Directory
---------------------------------

When ``allow_local = true``, you can create a personal hook library in ``~/.local/peter-hook/``:

**Directory structure:**

.. code-block:: text

   ~/.local/peter-hook/
   ├── rust-.peter-hook.toml         # Rust-specific hooks
   ├── python-.peter-hook.toml       # Python-specific hooks
   └── security-.peter-hook.toml     # Security scanning hooks

**Example shared hook library** (``~/.local/peter-hook/rust-.peter-hook.toml``):

.. code-block:: toml

   [hooks.rust-fmt]
   command = "cargo fmt --all"
   description = "Format all Rust code"
   modifies_repository = true
   files = ["**/*.rs", "Cargo.toml"]

   [hooks.rust-clippy]
   command = "cargo clippy --all-targets -- -D warnings"
   description = "Run Clippy with strict warnings"
   modifies_repository = false
   files = ["**/*.rs", "Cargo.toml"]

   [hooks.rust-test]
   command = "cargo test --all"
   description = "Run all tests"
   modifies_repository = false
   files = ["**/*.rs", "Cargo.toml", "tests/**/*"]

**Importing in any repository:**

.. code-block:: toml

   # .peter-hook.toml in any Rust project
   imports = ["/home/user/.local/peter-hook/rust-.peter-hook.toml"]

   [groups.pre-commit]
   includes = ["rust-fmt", "rust-clippy", "rust-test"]
   execution = "parallel"

   # Override specific hooks if needed
   [hooks.rust-test]
   command = "cargo test --all --release"  # Run tests in release mode

Security Model
--------------

**Absolute Path Restrictions:**

1. By default (``allow_local = false``), ALL absolute imports are rejected
2. When ``allow_local = true``, ONLY paths under ``$HOME/.local/peter-hook`` are allowed
3. All other absolute paths are rejected regardless of settings

**Symlink Attack Prevention:**

Peter Hook canonicalizes all paths to prevent symlink-based attacks. Even if a symlink exists within ``~/.local/peter-hook/`` that points outside the directory, the import will be rejected.

**Example of rejected import:**

.. code-block:: bash

   # This will be REJECTED even with allow_local = true
   ln -s /etc/passwd ~/.local/peter-hook/malicious.toml

The symlink will be followed and the real path (``/etc/passwd``) will be checked, causing the import to fail.

Default Behavior
----------------

If no global configuration file exists:

- ``allow_local = false`` (absolute imports disabled)
- Maximum security restrictions
- No warnings or errors

This ensures safe defaults for users who don't need global configuration.

Migration and Compatibility
---------------------------

The global configuration system was introduced to support shared hook libraries while maintaining security. Existing repositories without global configuration continue to work with default security settings.

**Upgrading from older versions:**

Older versions of peter-hook that don't support global configuration will simply ignore the ``~/.config/peter-hook/config.toml`` file. No migration is needed.
