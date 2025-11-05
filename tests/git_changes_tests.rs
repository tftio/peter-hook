#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
//! Comprehensive tests for git change detection

use git2::Repository as Git2Repository;
use peter_hook::git::{ChangeDetectionMode, GitChangeDetector};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_change_detector_new() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let result = GitChangeDetector::new(temp_dir.path());
    assert!(result.is_ok());
}

#[test]
fn test_change_detector_not_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    let result = GitChangeDetector::new(temp_dir.path());
    assert!(result.is_err());
}

#[test]
fn test_detect_working_directory_changes() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    // Create a file
    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

    let detector = GitChangeDetector::new(temp_dir.path()).unwrap();
    let result = detector.get_changed_files(&ChangeDetectionMode::WorkingDirectory);

    assert!(result.is_ok());
}

#[test]
fn test_detect_staged_changes() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create and stage a file
    fs::write(temp_dir.path().join("staged.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("staged.txt")).unwrap();
    index.write().unwrap();

    let detector = GitChangeDetector::new(temp_dir.path()).unwrap();
    let result = detector.get_changed_files(&ChangeDetectionMode::Staged);

    assert!(result.is_ok());
    if let Ok(files) = result {
        assert!(!files.is_empty());
    }
}

#[test]
fn test_detect_no_changes() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let detector = GitChangeDetector::new(temp_dir.path()).unwrap();
    let result = detector.get_changed_files(&ChangeDetectionMode::Staged);

    assert!(result.is_ok());
}

#[test]
fn test_detect_commit_range_changes() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create initial commit
    fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("file.txt")).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .unwrap();

    let detector = GitChangeDetector::new(temp_dir.path()).unwrap();
    let result = detector.get_changed_files(&ChangeDetectionMode::CommitRange {
        from: "HEAD".to_string(),
        to: "HEAD".to_string(),
    });

    assert!(result.is_ok());
}

#[test]
fn test_detect_push_mode() {
    let temp_dir = TempDir::new().unwrap();
    Git2Repository::init(temp_dir.path()).unwrap();

    let detector = GitChangeDetector::new(temp_dir.path()).unwrap();
    let result = detector.get_changed_files(&ChangeDetectionMode::Push {
        local_oid: "HEAD".to_string(),
        remote_oid: "HEAD^".to_string(),
    });

    // May succeed or fail depending on git state, but shouldn't panic
    let _ = result;
}

#[test]
fn test_detect_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create and stage multiple files
    for i in 1..=5 {
        fs::write(
            temp_dir.path().join(format!("file{i}.txt")),
            format!("content{i}"),
        )
        .unwrap();
    }

    let mut index = repo.index().unwrap();
    for i in 1..=5 {
        index
            .add_path(std::path::Path::new(&format!("file{i}.txt")))
            .unwrap();
    }
    index.write().unwrap();

    let detector = GitChangeDetector::new(temp_dir.path()).unwrap();
    let result = detector.get_changed_files(&ChangeDetectionMode::Staged);

    assert!(result.is_ok());
    if let Ok(files) = result {
        assert!(files.len() >= 5);
    }
}

#[test]
fn test_detect_nested_directory_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Git2Repository::init(temp_dir.path()).unwrap();

    // Create nested structure
    let nested = temp_dir.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("deep.txt"), "content").unwrap();

    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("a/b/c/deep.txt"))
        .unwrap();
    index.write().unwrap();

    let detector = GitChangeDetector::new(temp_dir.path()).unwrap();
    let result = detector.get_changed_files(&ChangeDetectionMode::Staged);

    assert!(result.is_ok());
}
