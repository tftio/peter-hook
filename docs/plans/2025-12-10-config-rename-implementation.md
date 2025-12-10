# Configuration File Rename Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename configuration file from `.peter-hook.toml` to `.peter-hook.toml` with deprecation detection

**Architecture:** Update config file discovery to search for `.peter-hook.toml`, add early validation that detects and reports all deprecated `.peter-hook.toml` files in repository, fail immediately with helpful error message listing all files and fix commands.

**Tech Stack:** Rust 1.86.0, walkdir crate for filesystem traversal, ignore crate for .gitignore respect

---

## Task 1: Add deprecation detection function

**Files:**
- Modify: `src/main.rs` (add new function around line 100)
- No tests yet (will add in Task 3)

**Step 1: Add walkdir and ignore crate dependencies**

Add to `Cargo.toml` dependencies section (if not already present):

```toml
walkdir = "2"
ignore = "0.4"
```

Expected: These crates are likely already dependencies. Check first with `grep walkdir Cargo.toml`.

**Step 2: Import required modules in src/main.rs**

Add these imports at the top of `src/main.rs`:

```rust
use walkdir::WalkDir;
use ignore::WalkBuilder;
```

**Step 3: Write deprecation detection function**

Add this function in `src/main.rs` after the imports and before `main()`:

```rust
/// Check for deprecated .peter-hook.toml files in the repository
///
/// Walks the repository tree respecting .gitignore and collects all
/// .peter-hook.toml files. If any are found, prints error message and exits.
fn check_for_deprecated_config_files() -> Result<()> {
    use std::env;

    // Try to find repository root
    let current_dir = env::current_dir().context("Failed to get current directory")?;

    let repo = match GitRepository::find_from_dir(&current_dir) {
        Ok(repo) => repo,
        Err(_) => {
            // Not in a git repository, skip check
            return Ok(());
        }
    };

    let repo_root = repo.git_dir().parent()
        .context("Failed to get repository root")?;

    // Walk repository respecting .gitignore
    let mut deprecated_files = Vec::new();

    for entry in WalkBuilder::new(repo_root)
        .hidden(false)  // Include hidden directories
        .git_ignore(true)  // Respect .gitignore
        .build()
    {
        let entry = entry.context("Failed to read directory entry")?;

        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            if let Some(file_name) = entry.path().file_name() {
                if file_name == ".peter-hook.toml" {
                    // Store relative path from repo root
                    let relative_path = entry.path()
                        .strip_prefix(repo_root)
                        .unwrap_or(entry.path());
                    deprecated_files.push(relative_path.to_path_buf());
                }
            }
        }
    }

    if !deprecated_files.is_empty() {
        eprintln!("Error: .peter-hook.toml is no longer supported. Rename to .peter-hook.toml\n");
        eprintln!("Found deprecated files:");
        for file in &deprecated_files {
            eprintln!("  - {}", file.display());
        }
        eprintln!("\nRun in each directory: mv .peter-hook.toml .peter-hook.toml");
        std::process::exit(1);
    }

    Ok(())
}
```

**Step 4: Hook deprecation check into main()**

Find the `main()` function in `src/main.rs` and add the check early, right after CLI parsing but before command dispatch.

Find this code pattern (around line 150-200):

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Debug/trace setup code...
```

Add the deprecation check after CLI parsing and before the match statement on `cli.command`:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Debug/trace setup code...

    // Check for deprecated config files (skip for version/license commands)
    match &cli.command {
        Commands::Version | Commands::License => {
            // Skip deprecation check for these commands
        }
        _ => {
            // Run deprecation check for all other commands
            check_for_deprecated_config_files()?;
        }
    }

    // Rest of main() continues...
    match cli.command {
```

**Step 5: Verify it compiles**

Run: `rustup run 1.86.0 cargo build`

Expected: SUCCESS (no errors)

**Step 6: Commit deprecation detection**

```bash
git add Cargo.toml src/main.rs
git commit -m "feat: add deprecation detection for .peter-hook.toml files"
```

---

## Task 2: Update core resolver to use .peter-hook.toml

**Files:**
- Modify: `src/hooks/resolver.rs:76-82`

**Step 1: Update find_config_file() to search for .peter-hook.toml**

In `src/hooks/resolver.rs`, find the `find_config_file()` method (around line 76):

```rust
pub fn find_config_file(&self) -> Result<Option<PathBuf>> {
    let mut current = self.current_dir.as_path();

    loop {
        let config_path = current.join(".peter-hook.toml");
        if config_path.exists() {
            return Ok(Some(config_path));
        }
```

Change to:

```rust
pub fn find_config_file(&self) -> Result<Option<PathBuf>> {
    let mut current = self.current_dir.as_path();

    loop {
        let config_path = current.join(".peter-hook.toml");
        if config_path.exists() {
            return Ok(Some(config_path));
        }
```

**Step 2: Verify it compiles**

Run: `rustup run 1.86.0 cargo build`

Expected: SUCCESS

**Step 3: Commit resolver update**

```bash
git add src/hooks/resolver.rs
git commit -m "feat: update resolver to search for .peter-hook.toml"
```

---

## Task 3: Add deprecation tests

**Files:**
- Create: `tests/deprecation_tests.rs`

**Step 1: Create deprecation test file**

Create `tests/deprecation_tests.rs`:

```rust
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

/// Helper to create a test repository with a .peter-hook.toml file
fn create_repo_with_deprecated_config() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init git repo");

    // Create deprecated .peter-hook.toml
    let config_path = temp_dir.path().join(".peter-hook.toml");
    fs::write(&config_path, "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n")
        .unwrap();

    (temp_dir, config_path)
}

#[test]
fn test_deprecation_error_on_single_file() {
    let (temp_dir, _) = create_repo_with_deprecated_config();

    // Try to run any command (except version/license)
    let mut cmd = Command::cargo_bin("peter-hook").unwrap();
    let output = cmd
        .arg("validate")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should exit with error
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check error message contains key information
    assert!(stderr.contains(".peter-hook.toml is no longer supported"));
    assert!(stderr.contains(".peter-hook.toml"));
    assert!(stderr.contains(".peter-hook.toml"));  // Should list the file
    assert!(stderr.contains("mv .peter-hook.toml .peter-hook.toml"));  // Should show fix
}

#[test]
fn test_deprecation_error_lists_multiple_files() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init git repo");

    // Create multiple deprecated .peter-hook.toml files
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n"
    ).unwrap();

    fs::create_dir_all(temp_dir.path().join("backend")).unwrap();
    fs::write(
        temp_dir.path().join("backend/.peter-hook.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n"
    ).unwrap();

    fs::create_dir_all(temp_dir.path().join("frontend")).unwrap();
    fs::write(
        temp_dir.path().join("frontend/.peter-hook.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n"
    ).unwrap();

    // Try to run validate
    let mut cmd = Command::cargo_bin("peter-hook").unwrap();
    let output = cmd
        .arg("validate")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should list all three files
    assert!(stderr.contains(".peter-hook.toml"));
    assert!(stderr.contains("backend/.peter-hook.toml") || stderr.contains("backend\\.peter-hook.toml"));
    assert!(stderr.contains("frontend/.peter-hook.toml") || stderr.contains("frontend\\.peter-hook.toml"));
}

#[test]
fn test_version_command_bypasses_deprecation_check() {
    let (temp_dir, _) = create_repo_with_deprecated_config();

    // Version command should work even with deprecated config
    let mut cmd = Command::cargo_bin("peter-hook").unwrap();
    let output = cmd
        .arg("version")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should succeed
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("peter-hook"));  // Should show version
}

#[test]
fn test_license_command_bypasses_deprecation_check() {
    let (temp_dir, _) = create_repo_with_deprecated_config();

    // License command should work even with deprecated config
    let mut cmd = Command::cargo_bin("peter-hook").unwrap();
    let output = cmd
        .arg("license")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should succeed
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MIT") || stdout.contains("Apache"));  // Should show license
}

#[test]
fn test_new_config_name_works() {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init git repo");

    // Create NEW config with correct name
    fs::write(
        temp_dir.path().join(".peter-hook.toml"),
        "[hooks.test]\ncommand = \"echo test\"\nmodifies_repository = false\n"
    ).unwrap();

    // Validate should work
    let mut cmd = Command::cargo_bin("peter-hook").unwrap();
    let output = cmd
        .arg("validate")
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Should succeed or show "Configuration is valid"
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should NOT contain deprecation error
    assert!(!stderr.contains(".peter-hook.toml is no longer supported"));
    assert!(!stdout.contains(".peter-hook.toml is no longer supported"));
}
```

**Step 2: Run deprecation tests**

Run: `rustup run 1.86.0 cargo test --test deprecation_tests`

Expected: All tests should PASS

**Step 3: Commit deprecation tests**

```bash
git add tests/deprecation_tests.rs
git commit -m "test: add deprecation detection tests"
```

---

## Task 4: Bulk update test files

**Files:**
- Modify: All files in `tests/` directory (~50 files)

**Step 1: Find all test files with .peter-hook.toml references**

Run: `grep -r "hooks\.toml" tests/ --files-with-matches`

Expected: List of ~30-40 test files

**Step 2: Bulk replace .peter-hook.toml with .peter-hook.toml in tests**

Run in worktree root:

```bash
find tests/ -name "*.rs" -type f -exec sed -i '' 's/hooks\.toml/.peter-hook.toml/g' {} +
```

Note: On Linux, use `sed -i` without the empty string argument.

**Step 3: Verify changes look correct**

Run: `git diff tests/ | head -100`

Expected: See changes like:
- `".peter-hook.toml"` → `".peter-hook.toml"`
- `.join(".peter-hook.toml")` → `.join(".peter-hook.toml")`
- Test strings updated

**Step 4: Run all tests to verify**

Run: `rustup run 1.86.0 cargo test --all`

Expected: All tests PASS (may take 2-3 minutes)

If tests fail, review failures and fix individually. Common issues:
- Hardcoded paths that need manual update
- Test assertions checking for specific strings
- File existence checks

**Step 5: Commit test updates**

```bash
git add tests/
git commit -m "test: update all tests to use .peter-hook.toml"
```

---

## Task 5: Update source code comments and strings

**Files:**
- Modify: `src/git/installer.rs` (comments around line 286, 299)
- Modify: `src/main.rs` (error messages around line 352, 819)

**Step 1: Update installer comments**

In `src/git/installer.rs`, find the git hook script template comments (around lines 286-299):

Change:
```rust
# Edit your .peter-hook.toml configuration instead
```

To:
```rust
# Edit your .peter-hook.toml configuration instead
```

**Step 2: Update error message in main.rs**

In `src/main.rs`, find the "No .peter-hook.toml file found" message (around line 819):

Change:
```rust
println!("No .peter-hook.toml file found in current directory or parent directories");
```

To:
```rust
println!("No .peter-hook.toml file found in current directory or parent directories");
```

**Step 3: Update hint message in main.rs**

In `src/main.rs`, find the hint about checking configuration (around line 352):

Change:
```rust
println!("💡 \x1b[36mTip:\x1b[0m Check your \x1b[33m.peter-hook.toml\x1b[0m configuration");
```

To:
```rust
println!("💡 \x1b[36mTip:\x1b[0m Check your \x1b[33m.peter-hook.toml\x1b[0m configuration");
```

**Step 4: Search for any remaining .peter-hook.toml in source**

Run: `grep -r "hooks\.toml" src/ --color`

Expected: Should only find references in the deprecation checker function we wrote

**Step 5: Verify compilation**

Run: `rustup run 1.86.0 cargo build`

Expected: SUCCESS

**Step 6: Commit source code updates**

```bash
git add src/
git commit -m "refactor: update comments and error messages to reference .peter-hook.toml"
```

---

## Task 6: Update documentation files

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `docs/configuration.rst`
- Modify: `docs/quickstart.rst`
- Modify: `docs/architecture.rst`
- Modify: `docs/examples.rst`
- Modify: `docs/global_config.rst`
- Modify: `docs/cli.rst`
- Modify: `docs/overview.rst`
- Modify: `docs/templating.rst`

**Step 1: Bulk replace in documentation**

Run in worktree root:

```bash
find docs/ README.md CLAUDE.md -type f \( -name "*.md" -o -name "*.rst" \) -exec sed -i '' 's/hooks\.toml/.peter-hook.toml/g' {} +
```

**Step 2: Verify changes**

Run: `git diff docs/ README.md CLAUDE.md | head -200`

Expected: See documentation examples updated with new filename

**Step 3: Check for any remaining references**

Run: `grep -r "hooks\.toml" docs/ README.md CLAUDE.md`

Expected: No results (all should be updated)

**Step 4: Manually review README.md**

The README is user-facing and critical. Open `README.md` and verify:
- Quick Start section uses `.peter-hook.toml`
- All code examples use correct filename
- Installation instructions make sense

**Step 5: Commit documentation updates**

```bash
git add docs/ README.md CLAUDE.md
git commit -m "docs: update all documentation to reference .peter-hook.toml"
```

---

## Task 7: Rename and update example files

**Files:**
- Rename: `examples/file-targeting.toml` → `examples/.peter-hook-file-targeting.toml`
- Rename: `examples/parallel-.peter-hook.toml` → `examples/.peter-hook-parallel.toml`
- Rename: `examples/advanced-features.toml` → `examples/.peter-hook-advanced.toml`
- Rename: `examples/hooks-with-imports.toml` → `examples/.peter-hook-with-imports.toml`
- Keep: `examples/hooks.lib.toml` (it's a library file, not main config)

**Step 1: Rename example files**

Run these commands:

```bash
git mv examples/file-targeting.toml examples/.peter-hook-file-targeting.toml
git mv examples/parallel-.peter-hook.toml examples/.peter-hook-parallel.toml
git mv examples/advanced-features.toml examples/.peter-hook-advanced.toml
git mv examples/hooks-with-imports.toml examples/.peter-hook-with-imports.toml
```

**Step 2: Update content of example files**

Update any internal references to `.peter-hook.toml` in the example files:

Run: `sed -i '' 's/hooks\.toml/.peter-hook.toml/g' examples/.peter-hook-*.toml`

**Step 3: Update docs/examples.rst**

In `docs/examples.rst`, update any references to the renamed example files.

**Step 4: Verify examples**

Run: `ls -la examples/`

Expected: Should see `.peter-hook-*.toml` files and `hooks.lib.toml`

**Step 5: Commit example file renames**

```bash
git add examples/
git commit -m "refactor: rename example files to use .peter-hook-*.toml naming"
```

---

## Task 8: Rename project's own configuration file

**Files:**
- Rename: `.peter-hook.toml` → `.peter-hook.toml`

**Step 1: Rename the project's own config file**

Run in worktree root:

```bash
git mv .peter-hook.toml .peter-hook.toml
```

**Step 2: Verify the project still works**

Run: `rustup run 1.86.0 cargo run -- validate`

Expected: Should show "✓ Configuration is valid"

**Step 3: Test hook installation still works**

Run: `rustup run 1.86.0 cargo run -- install --force`

Expected: Should install hooks successfully

**Step 4: Commit config rename**

```bash
git add .peter-hook.toml
git commit -m "refactor: rename project config to .peter-hook.toml"
```

---

## Task 9: Update CHANGELOG and bump version

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `VERSION`
- Modify: `Cargo.toml`

**Step 1: Add CHANGELOG entry**

Add this entry at the top of `CHANGELOG.md` (after the header):

```markdown
## [5.0.0] - 2025-12-10

### BREAKING CHANGES

- **Configuration file renamed from `.peter-hook.toml` to `.peter-hook.toml`**
  - Peter-hook now searches for `.peter-hook.toml` instead of `.peter-hook.toml`
  - If `.peter-hook.toml` files are detected, peter-hook will error and refuse to run
  - Migration: Rename all `.peter-hook.toml` files to `.peter-hook.toml`
  - Commands affected: All commands except `version` and `license`

**Migration guide:**

For single configuration:
```bash
mv .peter-hook.toml .peter-hook.toml
```

For monorepos with multiple configurations:
```bash
# Find all deprecated files
find . -name ".peter-hook.toml" -type f

# Rename each one
cd backend && mv .peter-hook.toml .peter-hook.toml
cd ../frontend && mv .peter-hook.toml .peter-hook.toml
```

**Why this change:**
- Follows dotfile conventions for tool configuration
- Reduces visual clutter in repository root
- Makes the file more discoverable as tool-specific config
- Aligns with modern CLI tool practices
```

**Step 2: Use versioneer to bump to 5.0.0**

Run: `versioneer major`

Expected: Updates VERSION and Cargo.toml to 5.0.0

**Step 3: Verify version synchronization**

Run: `cat VERSION && grep '^version' Cargo.toml | head -1`

Expected: Both should show 5.0.0

**Step 4: Commit version bump**

```bash
git add CHANGELOG.md VERSION Cargo.toml
git commit -m "chore: bump version to 5.0.0"
```

---

## Task 10: Final verification

**Files:**
- None (verification only)

**Step 1: Run full test suite**

Run: `rustup run 1.86.0 cargo test --all`

Expected: All tests PASS

**Step 2: Search for any remaining .peter-hook.toml references**

Run: `grep -r "hooks\.toml" --exclude-dir=target --exclude-dir=.git`

Expected: Should only find:
- In deprecation checker code (src/main.rs)
- In deprecation test file (tests/deprecation_tests.rs)
- Possibly in CHANGELOG.md (explaining the change)

**Step 3: Build release binary**

Run: `rustup run 1.86.0 cargo build --release`

Expected: SUCCESS

**Step 4: Manual smoke test with deprecated config**

```bash
# Create temp directory with old config
mkdir -p /tmp/test-peter-hook
cd /tmp/test-peter-hook
git init
echo '[hooks.test]\ncommand = "echo test"\nmodifies_repository = false' > .peter-hook.toml

# Try to run peter-hook
/path/to/worktree/target/release/peter-hook validate
```

Expected: Should show error message about deprecated config

**Step 5: Manual smoke test with new config**

```bash
# Rename to new config
mv .peter-hook.toml .peter-hook.toml

# Try to run peter-hook
/path/to/worktree/target/release/peter-hook validate
```

Expected: Should show "✓ Configuration is valid"

**Step 6: Review git log**

Run: `git log --oneline`

Expected: Should see clean commit history with all tasks completed

---

## Task 11: Prepare for merge

**Files:**
- None (git operations only)

**Step 1: Push branch to remote**

Run: `git push -u origin feature/config-rename-5.0.0`

Expected: Branch pushed successfully

**Step 2: Switch back to main worktree**

Run: `cd /Users/jfb/Projects/rust/peter-hook`

**Step 3: Create pull request**

Use GitHub CLI or web interface:

```bash
gh pr create \
  --title "feat!: rename configuration file from .peter-hook.toml to .peter-hook.toml" \
  --body "$(cat <<'EOF'
## Summary

Breaking change: Renames configuration file from `.peter-hook.toml` to `.peter-hook.toml`.

## Changes

- Config discovery now searches for `.peter-hook.toml`
- Deprecation detection: errors if `.peter-hook.toml` files exist
- Error message lists all deprecated files with fix instructions
- Updated all tests, docs, and examples
- Version bumped to 5.0.0

## Migration

For users upgrading from 4.x to 5.0.0:

```bash
# Single config
mv .peter-hook.toml .peter-hook.toml

# Monorepo with multiple configs
find . -name ".peter-hook.toml" -exec sh -c 'mv "$1" "$(dirname "$1")/.peter-hook.toml"' _ {} \;
```

## Testing

- ✅ All existing tests updated and passing
- ✅ New deprecation tests added
- ✅ Manual testing with both old and new configs
- ✅ Verified error messages are helpful

## Breaking Change Notice

This is a major version bump (5.0.0) due to the breaking change in configuration file naming.

Closes #XXX (if there's an issue)
EOF
)"
```

**Step 4: Request review (if team workflow)**

Add reviewers or wait for CI to pass.

---

## Success Criteria

- [ ] All tests pass with new configuration name
- [ ] Deprecation detection works and lists all files
- [ ] Error messages are clear and actionable
- [ ] `version` and `license` commands bypass check
- [ ] All documentation updated
- [ ] All examples updated
- [ ] Project's own config renamed
- [ ] Version bumped to 5.0.0
- [ ] CHANGELOG has migration guide
- [ ] No remaining `.peter-hook.toml` references except in deprecation code

---

## Notes

- This is a breaking change requiring major version bump
- Users will need to rename their config files manually
- Error message provides exact commands to fix
- Library files (like `hooks.lib.toml`) keep their names
- Only main config files need to be renamed
