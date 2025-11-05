//! Git hook capability detection
//!
//! This module determines what capabilities different git hook types have,
//! particularly whether they can provide a file list for hook execution.

/// Determine if a git hook type can provide a list of changed files
///
/// Some git hooks operate on files (pre-commit, pre-push, etc.) while others
/// operate on other artifacts like commit messages (commit-msg, prepare-commit-msg).
///
/// Hooks that can provide files:
/// - pre-commit: staged files
/// - pre-push: files changed between local and remote branches
/// - post-commit, post-merge, post-checkout: files in the recent commit(s)
/// - Working directory hooks: all changed files
///
/// Hooks that cannot provide files:
/// - commit-msg, prepare-commit-msg: operate on commit messages
/// - applypatch-msg: operates on patch messages
///
/// # Arguments
/// * `hook_type` - The name of the git hook (e.g., "pre-commit", "commit-msg")
///
/// # Returns
/// `true` if the hook type can provide a file list, `false` otherwise
#[must_use]
pub fn can_provide_files(hook_type: &str) -> bool {
    matches!(
        hook_type,
        "pre-commit"
            | "pre-push"
            | "post-commit"
            | "post-merge"
            | "post-checkout"
            | "pre-rebase"
            | "post-rewrite"
            | "pre-receive"
            | "post-receive"
            | "update"
            | "post-update"
            | "pre-applypatch"
            | "post-applypatch"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_provide_files_for_commit_hooks() {
        assert!(can_provide_files("pre-commit"));
        assert!(can_provide_files("post-commit"));
    }

    #[test]
    fn test_can_provide_files_for_push_hooks() {
        assert!(can_provide_files("pre-push"));
    }

    #[test]
    fn test_can_provide_files_for_merge_hooks() {
        assert!(can_provide_files("post-merge"));
        assert!(can_provide_files("post-checkout"));
    }

    #[test]
    fn test_cannot_provide_files_for_message_hooks() {
        assert!(!can_provide_files("commit-msg"));
        assert!(!can_provide_files("prepare-commit-msg"));
        assert!(!can_provide_files("applypatch-msg"));
    }

    #[test]
    fn test_can_provide_files_for_server_hooks() {
        assert!(can_provide_files("pre-receive"));
        assert!(can_provide_files("post-receive"));
        assert!(can_provide_files("update"));
        assert!(can_provide_files("post-update"));
    }

    #[test]
    fn test_can_provide_files_for_other_hooks() {
        assert!(can_provide_files("pre-rebase"));
        assert!(can_provide_files("post-rewrite"));
        assert!(can_provide_files("pre-applypatch"));
        assert!(can_provide_files("post-applypatch"));
    }

    #[test]
    fn test_cannot_provide_files_for_unknown_hook() {
        assert!(!can_provide_files("unknown-hook"));
        assert!(!can_provide_files(""));
    }
}
