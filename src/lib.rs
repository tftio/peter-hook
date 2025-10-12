//! Git Hook Manager - A hierarchical git hooks manager for monorepos
//!
//! This crate provides a system for managing git hooks in monorepo environments
//! with support for hierarchical configuration and hook composition.

/// Command-line interface
pub mod cli;
/// Configuration parsing and management
pub mod config;
/// Debug state management
pub mod debug;
/// Health check and diagnostics
pub mod doctor;
/// Git repository integration
pub mod git;
/// Hook resolution and execution system
pub mod hooks;
/// Output formatting utilities
pub mod output;

pub use config::*;
pub use git::*;
pub use hooks::*;
pub use output::*;
