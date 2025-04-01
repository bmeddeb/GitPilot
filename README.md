# GitPilot

GitPilot is a comprehensive Rust wrapper around the Git command-line interface, providing a safe and ergonomic API for Git operations in Rust applications.

## Features

- **Type-safe API**: Strong types for Git URLs, branch names, and other Git concepts
- **Comprehensive error handling**: Detailed error types for better error management
- **Structured data types**: Parse Git output into structured Rust types
- **Async support**: Optional async API using Tokio for non-blocking Git operations
- **Serde support**: Optional serialization/deserialization for GitPilot types

## Installation

Add GitPilot to your `Cargo.toml`:

```toml
[dependencies]
GitPilot = "0.2.0"
```

To enable additional features:

```toml
[dependencies]
GitPilot = { version = "0.2.0", features = ["async", "serde"] }
```

## Requirements

- Git must be installed and available in your PATH
- Rust 1.56 or later

## Basic Usage

```rust
use GitPilot::Repository;
use GitPilot::types::{GitUrl, BranchName};
use std::path::Path;
use std::str::FromStr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open an existing repository
    let repo = Repository::new("./my_project");
    
    // List all branches
    let branches = repo.list_branches()?;
    println!("Branches: {:?}", branches);
    
    // Get repository status
    let status = repo.status()?;
    if status.is_clean {
        println!("Working directory is clean");
    } else {
        println!("You have uncommitted changes");
    }
    
    // Create a new branch
    let new_branch = BranchName::from_str("feature/new-api")?;
    repo.create_local_branch(&new_branch)?;
    
    // Make some changes and commit them
    // (first modify some files...)
    repo.add(vec!["src/lib.rs"])?;
    repo.commit_staged("Add new API features")?;
    
    // Push to remote
    repo.push_to_upstream("origin", &new_branch)?;
    
    Ok(())
}
```

## Advanced Features

### Structured Data Types

GitPilot provides structured types for Git data:

```rust
// Get detailed commit information
let commit = repo.get_commit(None)?; // Current HEAD
println!("Commit: {} by {}", commit.short_hash, commit.author_name);
println!("Message: {}", commit.message);

// Get detailed status information
let status = repo.status()?;
for file in &status.files {
    println!("File: {:?}, Status: {:?}", file.path, file.status);
}
```

### Asynchronous API

Enable the `async` feature to use non-blocking Git operations:

```rust
use GitPilot::AsyncRepository;
use GitPilot::types::{GitUrl, BranchName};
use std::path::Path;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Clone a repository asynchronously
    let url = GitUrl::from_str("https://github.com/rust-lang/rust.git")?;
    let repo = AsyncRepository::clone(url, "./rust").await?;
    
    // List branches asynchronously
    let branches = repo.list_branches().await?;
    println!("Branches: {:?}", branches);
    
    Ok(())
}
```

## Feature Flags

- `serde`: Enables serialization/deserialization of GitPilot types
- `async`: Enables asynchronous Git operations using Tokio
- `full`: Enables all features

## Examples

See the `examples/` directory for more usage examples:
- `repo_stats.rs`: Analyze a Git repository and generate statistics
- `async_clone.rs`: Clone repositories asynchronously

## License

This project is licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request or open an Issue.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request