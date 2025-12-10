# Configuration File Rename: .peter-hook.toml → .peter-hook.toml

**Date:** 2025-12-10
**Version:** 5.0.0 (Breaking Change)
**Status:** Approved

## Overview

Rename the configuration file from `.peter-hook.toml` to `.peter-hook.toml` as a breaking change in version 5.0.0.

### Goal

- Update configuration file discovery to use `.peter-hook.toml` instead of `.peter-hook.toml`
- Provide clear, actionable error messages when deprecated `.peter-hook.toml` files are detected
- Hard-fail immediately when deprecated files exist (no grace period)

### Rationale

The new name `.peter-hook.toml`:
- Follows dotfile conventions for tool configuration
- Reduces visual clutter in repository root
- Makes the file more discoverable as tool-specific config
- Aligns with modern CLI tool practices (e.g., `.prettierrc`, `.eslintrc`)

## Implementation Strategy

### 1. Core Changes

**Update `find_config_file()` in `src/hooks/resolver.rs`:**

```rust
pub fn find_config_file(&self) -> Result<Option<PathBuf>> {
    let mut current = self.current_dir.as_path();

    loop {
        let config_path = current.join(".peter-hook.toml");  // Changed from ".peter-hook.toml"
        if config_path.exists() {
            return Ok(Some(config_path));
        }

        // Walk up to parent directory
        match current.parent() {
            Some(parent) => current = parent,
            None => return Ok(None),
        }
    }
}
```

### 2. Deprecation Detection

**Add deprecation checker in `src/main.rs`:**

Create a new function that walks the repository tree looking for `.peter-hook.toml` files:

```rust
fn check_for_deprecated_config_files() -> Result<()> {
    let repo = GitRepository::find_from_dir(&env::current_dir()?)?;
    let mut deprecated_files = Vec::new();

    // Walk repository from root, respecting .gitignore
    // Collect all paths ending in ".peter-hook.toml"

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

**Hook into main() early:**

Call this check right after parsing CLI args for all commands EXCEPT:
- `version` - Should always work
- `license` - Should always work

All other commands (`run`, `install`, `validate`, `lint`, `list`, `config`, etc.) should fail if deprecated files exist.

### 3. Error Message Behavior

**For single deprecated file:**
```
Error: .peter-hook.toml is no longer supported. Rename to .peter-hook.toml

Found deprecated files:
  - ./.peter-hook.toml

Run in each directory: mv .peter-hook.toml .peter-hook.toml
```

**For multiple deprecated files (monorepo):**
```
Error: .peter-hook.toml is no longer supported. Rename to .peter-hook.toml

Found deprecated files:
  - ./.peter-hook.toml
  - ./backend/.peter-hook.toml
  - ./frontend/.peter-hook.toml

Run in each directory: mv .peter-hook.toml .peter-hook.toml
```

## Test Updates

### Categories of Changes

**1. Test fixtures and setup code:**
```rust
// Before:
std::fs::write(&temp_dir.join(".peter-hook.toml"), config_content)?;

// After:
std::fs::write(&temp_dir.join(".peter-hook.toml"), config_content)?;
```

**2. Test assertions checking file paths:**
```rust
// Before:
assert!(temp_dir.join(".peter-hook.toml").exists());

// After:
assert!(temp_dir.join(".peter-hook.toml").exists());
```

**3. Error message assertions:**
```rust
// Before:
assert!(output.contains(".peter-hook.toml"));

// After:
assert!(output.contains(".peter-hook.toml"));
```

### New Deprecation Tests

Add tests in `tests/deprecation_tests.rs`:

1. **Test single deprecated file detection:**
   - Create `.peter-hook.toml` in temp repo
   - Run any command (except `version`/`license`)
   - Assert exit code 1
   - Assert error message contains file path and fix command

2. **Test multiple deprecated files:**
   - Create `.peter-hook.toml` in multiple subdirectories
   - Run command
   - Assert all files are listed in error message

3. **Test version/license commands still work:**
   - Create `.peter-hook.toml` in temp repo
   - Run `peter-hook version`
   - Assert success (exit 0)
   - Run `peter-hook license`
   - Assert success (exit 0)

4. **Test new config name works:**
   - Create `.peter-hook.toml` in temp repo
   - Run various commands
   - Assert all work correctly

### Bulk Updates

**Approach:** Use find/replace with manual verification:

```bash
# Find all occurrences
rg "hooks\.toml" --files-with-matches

# Replace in source files
rg "hooks\.toml" -l | xargs sed -i '' 's/hooks\.toml/.peter-hook.toml/g'
```

Then manually review changes to ensure correctness, especially in:
- Comments and documentation strings
- Error messages
- File path constructions

**Validation:** Run full test suite after bulk changes to catch any issues.

## Documentation Updates

### User-Facing Documentation

1. **README.md**
   - Update Quick Start examples
   - Update all code blocks showing config file

2. **docs/quickstart.rst**
   - Change config file name in examples

3. **docs/configuration.rst**
   - Update file name references
   - Update example paths

4. **docs/architecture.rst**
   - Update technical references

5. **docs/examples.rst**
   - Update all example code blocks

6. **CLAUDE.md** (both root and project-specific)
   - Update agent instructions with new file name

### Example Files

**Rename example files:**
- `examples/file-targeting.toml` → `examples/.peter-hook-file-targeting.toml`
- `examples/parallel-.peter-hook.toml` → `examples/.peter-hook-parallel.toml`
- `examples/advanced-features.toml` → `examples/.peter-hook-advanced.toml`
- `examples/hooks-with-imports.toml` → `examples/.peter-hook-with-imports.toml`

**Keep as-is:**
- `examples/hooks.lib.toml` - This is a library file for imports, not a main config

### Project's Own Config

Rename the project's own configuration file:
```bash
mv /Users/jfb/Projects/rust/peter-hook/.peter-hook.toml /Users/jfb/Projects/rust/peter-hook/.peter-hook.toml
```

### CHANGELOG.md

Add entry for version 5.0.0:

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

## Implementation Order

1. **Add deprecation detection** (new code)
   - Implement `check_for_deprecated_config_files()` in `src/main.rs`
   - Hook into command routing
   - Test manually with `.peter-hook.toml` present

2. **Update core resolver** (modify existing)
   - Change `find_config_file()` in `src/hooks/resolver.rs`
   - Update constant/string literal from `".peter-hook.toml"` to `".peter-hook.toml"`

3. **Update tests** (bulk changes)
   - Run find/replace across test files
   - Add new deprecation tests
   - Run full test suite and fix failures

4. **Update documentation** (after code works)
   - Update all doc files with new config name
   - Update examples and code blocks

5. **Rename example files** (after docs)
   - Rename all example `.toml` files (except library files)
   - Update any cross-references

6. **Rename project's own config** (dogfooding)
   - `mv .peter-hook.toml .peter-hook.toml`
   - Test that peter-hook still works for development

7. **Update CHANGELOG and bump version** (final step)
   - Add 5.0.0 entry to CHANGELOG.md
   - Use `just release major` to bump to 5.0.0
   - Create release

## Risks and Mitigations

### Risk 1: Missing References

**Risk:** Some `.peter-hook.toml` references may be missed in bulk updates.

**Mitigation:**
- Use comprehensive grep/ripgrep search after changes
- Run full test suite (all 425+ occurrences should be covered by tests)
- Manual testing with both old and new config names
- Search for literal string `".peter-hook.toml"` in codebase after changes

### Risk 2: Breaking User Workflows

**Risk:** Users' existing workflows will break immediately on upgrade.

**Mitigation:**
- Clear, actionable error messages with exact fix commands
- Major version bump (5.0.0) signals breaking change
- Comprehensive migration guide in CHANGELOG
- Release notes with prominent migration instructions
- Consider GitHub release announcement with migration guide

### Risk 3: Import Paths in Library Files

**Risk:** Library files imported by projects may have wrong references.

**Mitigation:**
- Library files like `hooks.lib.toml` keep their names (they're imported, not discovered)
- Only the main config file that peter-hook searches for needs `.peter-hook.toml` name
- Imported files can have any name
- Document this clearly in migration guide

### Risk 4: CI/CD Pipeline Breakage

**Risk:** Users' CI pipelines may break if they reference `.peter-hook.toml` in scripts.

**Mitigation:**
- Major version bump signals need for review before upgrading
- Error message clearly shows what changed
- Users can pin to 4.x versions if not ready to migrate
- Document migration in release notes

## Testing Validation Checklist

Before release, verify:

- [ ] All existing tests pass with new config name
- [ ] New deprecation tests verify error behavior
- [ ] Manual testing: `.peter-hook.toml` triggers error with correct message
- [ ] Manual testing: `.peter-hook.toml` works correctly
- [ ] Manual testing: Multiple deprecated files all listed in error
- [ ] Manual testing: `version` and `license` commands work even with `.peter-hook.toml`
- [ ] Manual testing: All other commands fail with `.peter-hook.toml`
- [ ] Grep/ripgrep search finds no remaining `.peter-hook.toml` references in code (except deprecation checker)
- [ ] Documentation examples all use `.peter-hook.toml`
- [ ] Example files renamed and working
- [ ] Project's own config renamed and working

## Files Requiring Changes

**Source code (8 files):**
- `src/hooks/resolver.rs` - Update `find_config_file()`
- `src/main.rs` - Add deprecation check
- `src/git/installer.rs` - Update comments/references
- `src/hooks/executor.rs` - Update comments
- `src/hooks/hierarchical.rs` - Update references
- `src/doctor.rs` - Update references
- `src/config/parser.rs` - Update comments
- `src/config/global.rs` - Update comments

**Test files (~50 files):**
- All files in `tests/` directory
- Add new `tests/deprecation_tests.rs`

**Documentation (10 files):**
- `README.md`
- `CLAUDE.md` (root and project)
- `docs/*.rst` (all docs)
- `CHANGELOG.md`

**Examples (5 files):**
- All files in `examples/` directory (rename and update)

**Project config:**
- `.peter-hook.toml` → `.peter-hook.toml`

**Total:** ~75 files requiring updates

## Success Criteria

1. ✅ Peter-hook searches for `.peter-hook.toml` instead of `.peter-hook.toml`
2. ✅ Presence of `.peter-hook.toml` causes immediate error with helpful message
3. ✅ Error message lists ALL deprecated files in repository
4. ✅ Error message provides exact fix command
5. ✅ All tests pass with new configuration name
6. ✅ All documentation updated
7. ✅ Version bumped to 5.0.0
8. ✅ CHANGELOG has comprehensive migration guide
