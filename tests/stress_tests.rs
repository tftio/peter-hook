//! Stress tests for peter-hook
//!
//! Tests system behavior under extreme conditions:
//! - Deep configuration hierarchies (10 levels)
//! - Large file sets (1000+ files)
//! - Large hook groups (50+ hooks)
//! - Performance benchmarks
//! - Resource usage validation

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;

/// Helper to create a git repository with configuration
fn setup_test_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Configure git
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Disable GPG signing
    Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    temp_dir
}

/// Get the peter-hook binary path
fn peter_hook_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("peter-hook")
}

#[test]
fn test_deep_hierarchy_10_levels() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create 10-level deep directory structure with hooks.toml at each level
    let mut current_path = repo_path.to_path_buf();
    for level in 0..10 {
        current_path = current_path.join(format!("level{level}"));
        fs::create_dir_all(&current_path).unwrap();

        // Each level adds a hook
        let config = format!(
            r#"
[hooks.hook-level-{level}]
command = "echo 'Hook at level {level}'"
modifies_repository = false
timeout_seconds = 5

[groups.pre-commit]
includes = ["hook-level-{level}"]
description = "Hooks at level {level}"
"#
        );
        fs::write(current_path.join("hooks.toml"), config).unwrap();
    }

    // Create and stage a file at the deepest level
    let deepest_path = repo_path.join("level0/level1/level2/level3/level4/level5/level6/level7/level8/level9");
    fs::write(deepest_path.join("test.txt"), "content").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Measure hierarchical resolution time
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(&deepest_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should execute hook from deepest level (nearest wins)
    assert!(
        combined.contains("level 9") || combined.contains("level9"),
        "Should execute hook from deepest level.\nOutput: {combined}"
    );

    assert!(output.status.success(), "Command should succeed");

    // Performance assertion: deep hierarchy should resolve in under 2 seconds
    assert!(
        duration.as_secs() < 2,
        "Deep hierarchy resolution took too long: {:?}",
        duration
    );

    println!("✓ 10-level hierarchy resolved in {:?}", duration);
}

#[test]
fn test_large_file_set_1000_files() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create hook that processes files
    let config = r#"
[hooks.file-counter]
command = "echo 'Processing files'"
modifies_repository = false
files = ["**/*.txt"]
timeout_seconds = 30

[groups.pre-commit]
includes = ["file-counter"]
description = "File processing"
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create 1000 text files in nested directories
    let start_creation = Instant::now();
    for i in 0..1000 {
        let dir = repo_path.join(format!("dir{}", i / 100));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("file{i}.txt")), format!("content {i}")).unwrap();
    }
    let creation_duration = start_creation.elapsed();
    println!("✓ Created 1000 files in {:?}", creation_duration);

    // Stage all files
    let start_stage = Instant::now();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let stage_duration = start_stage.elapsed();
    println!("✓ Staged 1000 files in {:?}", stage_duration);

    // Run hook with large file set
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("Processing files"),
        "Hook should execute.\nOutput: {combined}"
    );

    assert!(output.status.success(), "Command should succeed with 1000 files");

    // Performance assertion: should handle 1000 files in under 5 seconds
    assert!(
        duration.as_secs() < 5,
        "Processing 1000 files took too long: {:?}",
        duration
    );

    println!("✓ Processed 1000 files in {:?}", duration);
}

#[test]
fn test_large_hook_group_50_hooks() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create configuration with 50 hooks
    let mut config = String::from("");
    let mut includes = Vec::new();

    for i in 0..50 {
        config.push_str(&format!(
            r#"
[hooks.hook-{i}]
command = "echo 'Hook {i} executed'"
modifies_repository = false
timeout_seconds = 5

"#
        ));
        includes.push(format!("\"hook-{i}\""));
    }

    config.push_str(&format!(
        r#"
[groups.pre-commit]
includes = [{}]
description = "Large hook group"
execution_strategy = "parallel"
"#,
        includes.join(", ")
    ));

    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run all 50 hooks in parallel
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Verify at least some hooks executed
    let hook_count = (0..50)
        .filter(|i| combined.contains(&format!("Hook {i} executed")) || combined.contains(&format!("hook-{i}")))
        .count();

    assert!(
        hook_count > 0,
        "At least some hooks should execute.\nOutput: {combined}"
    );

    assert!(output.status.success(), "Command should succeed with 50 hooks");

    // Performance assertion: parallel execution of 50 simple hooks should complete in under 10 seconds
    assert!(
        duration.as_secs() < 10,
        "Executing 50 hooks took too long: {:?}",
        duration
    );

    println!("✓ Executed 50 hooks in parallel in {:?}", duration);
}

#[test]
fn test_sequential_hooks_performance() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create 20 sequential hooks (reasonable number for sequential execution)
    let mut config = String::from("");
    let mut includes = Vec::new();

    for i in 0..20 {
        config.push_str(&format!(
            r#"
[hooks.seq-hook-{i}]
command = "echo 'Sequential hook {i}'"
modifies_repository = false
timeout_seconds = 5

"#
        ));
        includes.push(format!("\"seq-hook-{i}\""));
    }

    config.push_str(&format!(
        r#"
[groups.pre-commit]
includes = [{}]
description = "Sequential hooks"
execution_strategy = "sequential"
"#,
        includes.join(", ")
    ));

    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run 20 hooks sequentially
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    assert!(output.status.success(), "Sequential execution should succeed");

    // Performance assertion: 20 sequential hooks should complete in under 5 seconds
    assert!(
        duration.as_secs() < 5,
        "Sequential execution of 20 hooks took too long: {:?}",
        duration
    );

    println!("✓ Executed 20 sequential hooks in {:?}", duration);
}

#[test]
fn test_validate_command_performance_complex_config() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create complex configuration with dependencies and imports
    let mut config = String::from("");

    // Create 30 hooks with complex dependencies
    for i in 0..30 {
        let deps = if i > 0 {
            format!("depends_on = [\"perf-hook-{}\"]", i - 1)
        } else {
            String::from("")
        };

        config.push_str(&format!(
            r#"
[hooks.perf-hook-{i}]
command = "echo 'Hook {i}'"
modifies_repository = false
files = ["**/*.rs", "**/*.toml"]
{deps}

"#
        ));
    }

    config.push_str(
        r#"
[groups.pre-commit]
includes = ["perf-hook-29"]
description = "Complex dependency chain"

[groups.pre-push]
includes = ["perf-hook-15", "perf-hook-20"]
description = "Multiple groups"
"#,
    );

    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Measure validate command performance
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["validate"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    assert!(output.status.success(), "Validate should succeed");

    // Performance assertion: validation should complete in under 1 second
    assert!(
        duration.as_millis() < 1000,
        "Validation took too long: {:?}",
        duration
    );

    println!("✓ Validated complex config in {:?}", duration);
}

#[test]
fn test_memory_efficient_large_config() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create a very large configuration (100 hooks)
    let mut config = String::from("");
    let mut includes = Vec::new();

    for i in 0..100 {
        config.push_str(&format!(
            r#"
[hooks.mem-hook-{i}]
command = "true"
modifies_repository = false
files = ["**/*.txt", "**/*.rs", "**/*.toml", "**/*.md", "**/*.yml"]
description = "Test hook {i} for memory efficiency testing with long description"
timeout_seconds = 300

"#
        ));
        includes.push(format!("\"mem-hook-{i}\""));
    }

    // Split into multiple groups
    config.push_str(&format!(
        r#"
[groups.pre-commit]
includes = [{}]
description = "First 50 hooks"
execution_strategy = "parallel"

[groups.pre-push]
includes = [{}]
description = "Last 50 hooks"
execution_strategy = "parallel"
"#,
        includes[0..50].join(", "),
        includes[50..100].join(", ")
    ));

    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Just validate the config (don't execute to save time)
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["validate"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    assert!(
        output.status.success(),
        "Should handle 100 hooks in configuration"
    );

    // Should validate efficiently even with 100 hooks
    assert!(
        duration.as_secs() < 2,
        "Validation of 100-hook config took too long: {:?}",
        duration
    );

    println!("✓ Validated 100-hook configuration in {:?}", duration);
}

#[test]
fn test_file_discovery_performance_deep_tree() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create deep directory tree (5 levels, 10 dirs per level = 10^5 = 100,000 potential paths)
    // But we'll be more modest: 5 levels, 5 dirs per level = 5^5 = 3,125 paths
    fn create_tree(path: &PathBuf, depth: u32, breadth: u32) {
        if depth == 0 {
            // Create a file at leaf
            fs::write(path.join("leaf.txt"), "content").unwrap();
            return;
        }

        for i in 0..breadth {
            let subdir = path.join(format!("dir{i}"));
            fs::create_dir_all(&subdir).unwrap();
            create_tree(&subdir, depth - 1, breadth);
        }
    }

    let start_creation = Instant::now();
    create_tree(&repo_path.to_path_buf(), 5, 5);
    let creation_duration = start_creation.elapsed();
    println!("✓ Created deep tree in {:?}", creation_duration);

    // Create hook config
    let config = r#"
[hooks.tree-walker]
command = "echo 'Found files'"
modifies_repository = false
files = ["**/*.txt"]
timeout_seconds = 30

[groups.pre-commit]
includes = ["tree-walker"]
"#;
    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Stage all files
    let start_stage = Instant::now();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let stage_duration = start_stage.elapsed();
    println!("✓ Staged deep tree in {:?}", stage_duration);

    // Run hook with deep tree
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    assert!(output.status.success(), "Should handle deep directory tree");

    // Should handle deep tree efficiently
    assert!(
        duration.as_secs() < 10,
        "Processing deep tree took too long: {:?}",
        duration
    );

    println!("✓ Processed deep directory tree in {:?}", duration);
}

#[test]
fn test_mixed_execution_strategies_performance() {
    let temp_dir = setup_test_repo();
    let repo_path = temp_dir.path();

    // Create mix of modifying and non-modifying hooks
    let mut config = String::from("");

    // 10 non-modifying hooks (can run in parallel)
    for i in 0..10 {
        config.push_str(&format!(
            r#"
[hooks.parallel-hook-{i}]
command = "echo 'Parallel {i}'"
modifies_repository = false
timeout_seconds = 5

"#
        ));
    }

    // 5 modifying hooks (must run sequentially)
    for i in 0..5 {
        config.push_str(&format!(
            r#"
[hooks.sequential-hook-{i}]
command = "echo 'Sequential {i}'"
modifies_repository = true
timeout_seconds = 5

"#
        ));
    }

    config.push_str(
        r#"
[groups.pre-commit]
includes = [
    "parallel-hook-0", "parallel-hook-1", "parallel-hook-2", "parallel-hook-3", "parallel-hook-4",
    "parallel-hook-5", "parallel-hook-6", "parallel-hook-7", "parallel-hook-8", "parallel-hook-9",
    "sequential-hook-0", "sequential-hook-1", "sequential-hook-2", "sequential-hook-3", "sequential-hook-4"
]
description = "Mixed execution"
execution_strategy = "parallel"
"#,
    );

    fs::write(repo_path.join("hooks.toml"), config).unwrap();

    // Create and stage a file
    fs::write(repo_path.join("test.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run mixed execution (should run parallel phase then sequential phase)
    let start = Instant::now();

    let output = Command::new(peter_hook_bin())
        .args(["run", "pre-commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let duration = start.elapsed();

    assert!(output.status.success(), "Mixed execution should succeed");

    // Should complete efficiently despite phase separation
    assert!(
        duration.as_secs() < 8,
        "Mixed execution took too long: {:?}",
        duration
    );

    println!("✓ Mixed execution (10 parallel + 5 sequential) completed in {:?}", duration);
}
