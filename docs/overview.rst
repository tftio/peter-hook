Overview
========

Peter Hook enables different paths within a monorepo to have their own custom git hooks while maintaining repository-wide quality standards. It features intelligent parallel execution that respects file system safety: read-only hooks run concurrently for speed, while repository-modifying hooks run sequentially to prevent conflicts.

Key Features
------------

- Hierarchical configuration: nearest ``.peter-hook.toml`` file wins
- Safe parallel execution: repository-modifying hooks never run concurrently
- Hook composition: reusable groups with execution strategies
- File targeting: run hooks only when matching files change
- Cross-platform: native binary for macOS, Linux, Windows

Repository Structure
--------------------

- ``src/config/``: TOML configuration parsing and validation
- ``src/hooks/``: hook resolution, dependencies, and execution
- ``src/git/``: git repository detection, hook installer, change detection
- ``src/cli/``: command-line interface
- ``examples/``: configuration examples

