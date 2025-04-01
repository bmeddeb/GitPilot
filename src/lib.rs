//! A Rust library providing a high-level interface to Git operations
//! by wrapping the `git` command-line tool.
//!
//! This library requires the `git` executable to be installed and accessible
//! in the system's PATH where the Rust program is executed.
//!
//! # Examples
//!
//! ```no_run
//! use GitPilot::Repository;
//! use GitPilot::types::{GitUrl, BranchName};
//! use std::path::Path;
//! use std::str::FromStr;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Clone a repository
//! let repo_url = GitUrl::from_str("https://github.com/rust-lang/rust.git")?;
//! let repo_path = Path::new("./my_rust_clone");
//! let repo = Repository::clone(repo_url, &repo_path)?;
//!
//! // List branches
//! let branches = repo.list_branches()?;
//! println!("Branches: {:?}", branches);
//!
//! // Create and switch to a new branch
//! let new_branch = BranchName::from_str("my-feature")?;
//! repo.create_local_branch(&new_branch)?;
//!
//! // Stage and commit changes
//! repo.add(vec!["src/lib.rs"])?;
//! repo.commit_staged("Implement new feature")?;
//!
//! // Push to remote
//! repo.push_to_upstream("origin", &new_branch)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Feature Flags
//!
//! - `serde`: Enables serialization/deserialization of type structs using the `serde` crate.
//! - `async`: Enables asynchronous Git operations using Tokio.

// First define all our modules
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