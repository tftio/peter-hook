//! Git change detection utilities

use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

/// Detects changed files in a git repository
pub struct GitChangeDetector {
    /// Git repository root
    repo_root: PathBuf,
}

/// Types of git changes to detect
#[derive(Debug, Clone)]
pub enum ChangeDetectionMode {
    /// Changes in working directory (staged + unstaged + untracked)
    WorkingDirectory,
    /// Only staged changes (for pre-commit hooks)
    Staged,
    /// Changes being pushed (for pre-push)
    Push {
        /// Remote name (usually "origin")
        remote: String,
        /// Branch being pushed to
        remote_branch: String,
    },
    /// Changes in a specific commit range
    CommitRange {
        /// Start commit (exclusive)
        from: String,
        /// End commit (inclusive)  
        to: String,
    },
}

impl GitChangeDetector {
    /// Create a new change detector for the given repository
    ///
    /// # Errors
    ///
    /// Returns an error if the git repository cannot be accessed
    pub fn new<P: AsRef<Path>>(repo_root: P) -> Result<Self> {
        let repo_root = repo_root.as_ref().to_path_buf();

        // Verify this is a git repository
        if !repo_root.join(".git").exists() {
            return Err(anyhow::anyhow!(
                "Not a git repository: {}",
                repo_root.display()
            ));
        }

        Ok(Self { repo_root })
    }

    /// Get changed files based on the detection mode
    ///
    /// # Errors
    ///
    /// Returns an error if git commands fail or output cannot be parsed
    pub fn get_changed_files(&self, mode: &ChangeDetectionMode) -> Result<Vec<PathBuf>> {
        match mode {
            ChangeDetectionMode::WorkingDirectory => self.get_working_directory_changes(),
            ChangeDetectionMode::Staged => self.get_staged_changes(),
            ChangeDetectionMode::Push {
                remote,
                remote_branch,
            } => self.get_push_changes(remote, remote_branch),
            ChangeDetectionMode::CommitRange { from, to } => {
                self.get_commit_range_changes(from, to)
            }
        }
    }

    /// Get files changed in working directory (staged + unstaged)
    fn get_working_directory_changes(&self) -> Result<Vec<PathBuf>> {
        let mut changed_files = HashSet::new();

        // Get staged changes (exclude deleted files)
        let staged_output = self.run_git_command(&["diff", "--cached", "--name-status"])?;
        for line in staged_output.lines() {
            if let Some((status, rest)) = line.split_once('\t') {
                if !status.starts_with('D') {
                    // Skip deleted files
                    // Handle renames (R) and copies (C): format is "status\told_name\tnew_name"
                    // For renames/copies, we want the destination (new) file
                    let filename = if status.starts_with('R') || status.starts_with('C') {
                        // Split on tab to get old and new filenames, use the new one
                        rest.split('\t').nth(1).unwrap_or(rest)
                    } else {
                        rest
                    };
                    changed_files.insert(PathBuf::from(filename));
                }
            }
        }

        // Get unstaged changes (exclude deleted files)
        let unstaged_output = self.run_git_command(&["diff", "--name-status"])?;
        for line in unstaged_output.lines() {
            if let Some((status, rest)) = line.split_once('\t') {
                if !status.starts_with('D') {
                    // Skip deleted files
                    // Handle renames (R) and copies (C): format is "status\told_name\tnew_name"
                    let filename = if status.starts_with('R') || status.starts_with('C') {
                        rest.split('\t').nth(1).unwrap_or(rest)
                    } else {
                        rest
                    };
                    changed_files.insert(PathBuf::from(filename));
                }
            }
        }

        // Get untracked files (these are always additions, never deletions)
        let untracked_output =
            self.run_git_command(&["ls-files", "--others", "--exclude-standard"])?;
        for line in untracked_output.lines() {
            if !line.trim().is_empty() {
                changed_files.insert(PathBuf::from(line.trim()));
            }
        }

        Ok(changed_files.into_iter().collect())
    }

    /// Get only staged changes (for pre-commit hooks)
    fn get_staged_changes(&self) -> Result<Vec<PathBuf>> {
        // Get only staged changes using git diff --cached (exclude deleted files)
        let staged_output = self.run_git_command(&["diff", "--cached", "--name-status"])?;

        let mut changed_files = Vec::new();
        for line in staged_output.lines() {
            if let Some((status, rest)) = line.split_once('\t') {
                if !status.starts_with('D') {
                    // Skip deleted files
                    // Handle renames (R) and copies (C): format is "status\told_name\tnew_name"
                    let filename = if status.starts_with('R') || status.starts_with('C') {
                        rest.split('\t').nth(1).unwrap_or(rest)
                    } else {
                        rest
                    };
                    changed_files.push(PathBuf::from(filename));
                }
            }
        }

        Ok(changed_files)
    }

    /// Get files changed in push (compare local branch with remote)
    fn get_push_changes(&self, remote: &str, remote_branch: &str) -> Result<Vec<PathBuf>> {
        let remote_ref = format!("{remote}/{remote_branch}");
        let diff_output = self.run_git_command(&["diff", "--name-status", &remote_ref, "HEAD"])?;

        let mut changed_files = Vec::new();
        for line in diff_output.lines() {
            if let Some((status, rest)) = line.split_once('\t') {
                if !status.starts_with('D') {
                    // Skip deleted files
                    // Handle renames (R) and copies (C): format is "status\told_name\tnew_name"
                    let filename = if status.starts_with('R') || status.starts_with('C') {
                        rest.split('\t').nth(1).unwrap_or(rest)
                    } else {
                        rest
                    };
                    changed_files.push(PathBuf::from(filename));
                }
            }
        }

        Ok(changed_files)
    }

    /// Get files changed in a commit range
    fn get_commit_range_changes(&self, from: &str, to: &str) -> Result<Vec<PathBuf>> {
        let range = format!("{from}..{to}");
        let diff_output = self.run_git_command(&["diff", "--name-status", &range])?;

        let mut changed_files = Vec::new();
        for line in diff_output.lines() {
            if let Some((status, rest)) = line.split_once('\t') {
                if !status.starts_with('D') {
                    // Skip deleted files
                    // Handle renames (R) and copies (C): format is "status\told_name\tnew_name"
                    let filename = if status.starts_with('R') || status.starts_with('C') {
                        rest.split('\t').nth(1).unwrap_or(rest)
                    } else {
                        rest
                    };
                    changed_files.push(PathBuf::from(filename));
                }
            }
        }

        Ok(changed_files)
    }

    /// Run a git command and return stdout
    fn run_git_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .with_context(|| format!("Failed to run git command: git {}", args.join(" ")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Git command failed: git {}\nError: {}",
                args.join(" "),
                stderr
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// File pattern matcher using glob patterns
pub struct FilePatternMatcher {
    /// Compiled glob patterns
    patterns: Vec<glob::Pattern>,
}

impl FilePatternMatcher {
    /// Create a new pattern matcher from glob patterns
    ///
    /// # Errors
    ///
    /// Returns an error if any glob pattern is invalid
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut compiled_patterns = Vec::new();

        for pattern in patterns {
            let compiled = glob::Pattern::new(pattern)
                .with_context(|| format!("Invalid glob pattern: {pattern}"))?;
            compiled_patterns.push(compiled);
        }

        Ok(Self {
            patterns: compiled_patterns,
        })
    }

    /// Check if any of the patterns match the given file path
    #[must_use]
    pub fn matches(&self, file_path: &Path) -> bool {
        if self.patterns.is_empty() {
            return true; // No patterns means match everything
        }

        let path_str = file_path.to_string_lossy();

        self.patterns.iter().any(|pattern| {
            pattern.matches(&path_str) ||
            // Also try with just the filename
            file_path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| pattern.matches(name))
        })
    }

    /// Check if any files in the list match the patterns
    #[must_use]
    pub fn matches_any(&self, files: &[PathBuf]) -> bool {
        if self.patterns.is_empty() {
            return true; // No patterns means always match
        }

        files.iter().any(|file| self.matches(file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_git_repo(temp_dir: &Path) -> PathBuf {
        let git_dir = temp_dir.join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir)
            .output()
            .unwrap();

        // Configure git for tests
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir)
            .output()
            .unwrap();

        temp_dir.to_path_buf()
    }

    #[test]
    fn test_change_detector_creation() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = create_test_git_repo(temp_dir.path());

        let detector = GitChangeDetector::new(&repo_dir).unwrap();
        assert_eq!(detector.repo_root, repo_dir);
    }

    #[test]
    fn test_working_directory_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = create_test_git_repo(temp_dir.path());

        // Create and add a file
        let test_file = repo_dir.join("test.rs");
        fs::write(&test_file, "fn main() {}").unwrap();

        let detector = GitChangeDetector::new(&repo_dir).unwrap();
        let changes = detector.get_working_directory_changes().unwrap();

        assert!(changes.contains(&PathBuf::from("test.rs")));
    }

    #[test]
    fn test_file_pattern_matcher() {
        let patterns = vec!["**/*.rs".to_string(), "*.toml".to_string()];

        let matcher = FilePatternMatcher::new(&patterns).unwrap();

        // Should match Rust files
        assert!(matcher.matches(&PathBuf::from("src/main.rs")));
        assert!(matcher.matches(&PathBuf::from("tests/test.rs")));
        assert!(matcher.matches(&PathBuf::from("lib/deep/nested/file.rs")));

        // Should match TOML files in root
        assert!(matcher.matches(&PathBuf::from("Cargo.toml")));
        assert!(matcher.matches(&PathBuf::from("config.toml")));

        // Should not match other files
        assert!(!matcher.matches(&PathBuf::from("README.md")));
        assert!(!matcher.matches(&PathBuf::from("src/config/file.js")));

        // Note: "*.toml" pattern only matches files in root, not nested
        // But our matcher also checks filename, so this will match
        assert!(matcher.matches(&PathBuf::from("nested/Cargo.toml"))); // Matches by filename
    }

    #[test]
    fn test_pattern_matches_any() {
        let patterns = vec!["**/*.py".to_string()];
        let matcher = FilePatternMatcher::new(&patterns).unwrap();

        let mixed_files = vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("scripts/build.py"),
            PathBuf::from("README.md"),
        ];

        assert!(matcher.matches_any(&mixed_files)); // Contains build.py

        let no_python_files = vec![PathBuf::from("src/main.rs"), PathBuf::from("README.md")];

        assert!(!matcher.matches_any(&no_python_files)); // No Python files
    }

    #[test]
    fn test_empty_patterns() {
        let matcher = FilePatternMatcher::new(&[]).unwrap();

        // Empty patterns should match everything
        assert!(matcher.matches(&PathBuf::from("any/file.ext")));
        assert!(matcher.matches_any(&[PathBuf::from("test.rs")]));
    }

    #[test]
    fn test_deleted_files_excluded() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = create_test_git_repo(temp_dir.path());
        let detector = GitChangeDetector::new(&repo_dir).unwrap();

        // Create, add, and commit a file
        let test_file = repo_dir.join("test.rs");
        fs::write(&test_file, "fn main() {}").unwrap();

        Command::new("git")
            .args(["add", "test.rs"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Add test file"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Create another file and delete the first
        let new_file = repo_dir.join("new.rs");
        fs::write(&new_file, "fn new() {}").unwrap();
        std::fs::remove_file(&test_file).unwrap();

        // Stage the new file and the deletion
        Command::new("git")
            .args(["add", "new.rs"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["add", "-u"]) // Stage deletions
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Test staged changes - should only include new.rs, not deleted test.rs
        let staged_changes = detector.get_staged_changes().unwrap();
        assert!(staged_changes.contains(&PathBuf::from("new.rs")));
        assert!(!staged_changes.contains(&PathBuf::from("test.rs")));

        // Test working directory changes - should include new.rs (untracked) but not
        // test.rs (deleted)
        let working_changes = detector.get_working_directory_changes().unwrap();
        assert!(working_changes.contains(&PathBuf::from("new.rs")));
        assert!(!working_changes.contains(&PathBuf::from("test.rs")));
    }

    #[test]
    fn test_renamed_files_tracked() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = create_test_git_repo(temp_dir.path());
        let detector = GitChangeDetector::new(&repo_dir).unwrap();

        // Create, add, and commit a file
        let old_file = repo_dir.join("old_name.rs");
        fs::write(&old_file, "fn main() {}").unwrap();

        Command::new("git")
            .args(["add", "old_name.rs"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Add file"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Rename the file and stage the rename
        let new_file = repo_dir.join("new_name.rs");
        std::fs::rename(&old_file, &new_file).unwrap();

        Command::new("git")
            .args(["add", "-A"]) // Stage rename
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Test staged changes - should include the NEW filename, not the old one
        let staged_changes = detector.get_staged_changes().unwrap();
        assert!(
            staged_changes.contains(&PathBuf::from("new_name.rs")),
            "Should contain the new filename after rename"
        );
        assert!(
            !staged_changes.contains(&PathBuf::from("old_name.rs")),
            "Should not contain the old filename after rename"
        );

        // Test working directory changes
        let working_changes = detector.get_working_directory_changes().unwrap();
        assert!(
            working_changes.contains(&PathBuf::from("new_name.rs")),
            "Working directory should contain the new filename"
        );
    }

    #[test]
    fn test_renamed_files_in_commit_range() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = create_test_git_repo(temp_dir.path());
        let detector = GitChangeDetector::new(&repo_dir).unwrap();

        // Create, add, and commit a file
        let old_file = repo_dir.join("original.rs");
        fs::write(&old_file, "fn main() {}").unwrap();

        Command::new("git")
            .args(["add", "original.rs"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Add original file"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Get the commit hash
        let first_commit = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();
        let first_commit_hash = String::from_utf8_lossy(&first_commit.stdout)
            .trim()
            .to_string();

        // Rename the file and commit
        let new_file = repo_dir.join("renamed.rs");
        std::fs::rename(&old_file, &new_file).unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Rename file"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Test commit range - should show the NEW filename
        let range_changes = detector
            .get_commit_range_changes(&first_commit_hash, "HEAD")
            .unwrap();

        assert!(
            range_changes.contains(&PathBuf::from("renamed.rs")),
            "Commit range should contain the new filename after rename"
        );
        assert!(
            !range_changes.contains(&PathBuf::from("original.rs")),
            "Commit range should not contain the old filename"
        );
    }

    #[test]
    fn test_copied_files_tracked() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = create_test_git_repo(temp_dir.path());
        let detector = GitChangeDetector::new(&repo_dir).unwrap();

        // Create, add, and commit a file
        let original_file = repo_dir.join("template.rs");
        fs::write(&original_file, "fn template() {}").unwrap();

        Command::new("git")
            .args(["add", "template.rs"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Add template"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Copy the file (requires making a change to detect as copy)
        let copied_file = repo_dir.join("copied.rs");
        fs::copy(&original_file, &copied_file).unwrap();
        // Modify the copy slightly so git detects it as a copy rather than identical
        fs::write(&copied_file, "fn template() {} // modified").unwrap();

        Command::new("git")
            .args(["add", "copied.rs"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        // Test staged changes - should include the copied file
        let staged_changes = detector.get_staged_changes().unwrap();
        assert!(
            staged_changes.contains(&PathBuf::from("copied.rs")),
            "Should contain the copied filename"
        );
    }
}
