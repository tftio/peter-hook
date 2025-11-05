//! Git repository integration

pub mod capabilities;
pub mod changes;
pub mod installer;
pub mod lint;
pub mod repository;
pub mod worktree;

pub use capabilities::*;
pub use changes::*;
pub use installer::*;
pub use lint::*;
pub use repository::*;
pub use worktree::*;
