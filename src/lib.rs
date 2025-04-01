//! A Rust library providing a high-level interface to Git operations
//! by wrapping the `git` command-line tool.
//!
//! This library requires the `git` executable to be installed and accessible
//! in the system's PATH where the Rust program is executed.
//!

pub mod error;
pub mod types;
pub mod models;
pub mod repository;

// Feature-gated modules
#[cfg(feature = "async")]
pub mod async_git;

// Re-export key types
pub use crate::error::GitError;
pub use crate::repository::Repository;
pub use crate::types::{BranchName, GitUrl, Result};

// Conditional re-exports based on features
#[cfg(feature = "async")]
pub use crate::async_git::AsyncRepository;

// Re-export all modules
pub mod prelude {
    //! Convenient import for common GitPilot types and traits.
    pub use crate::error::GitError;
    pub use crate::repository::Repository;
    pub use crate::types::{BranchName, GitUrl, Result};
    pub use crate::models::*;

    #[cfg(feature = "async")]
    pub use crate::async_git::AsyncRepository;
}