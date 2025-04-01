// examples/async_clone.rs
//
// This example demonstrates the use of the async API in GitPilot to
// clone a Git repository and then perform some basic operations.

use std::error::Error;
use std::path::Path;
use std::str::FromStr;
use tokio::fs;

use GitPilot::types::{GitUrl, BranchName, Result as GitResult};
use GitPilot::AsyncRepository;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <repo_url> <target_directory>", args[0]);
        eprintln!("Example: {} https://github.com/rust-lang/rust-analyzer.git ./rust-analyzer", args[0]);
        return Ok(());
    }

    let url_str = &args[1];
    let target_dir = &args[2];

    // Parse the Git URL
    let url = match GitUrl::from_str(url_str) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Error parsing Git URL: {}", e);
            return Ok(());
        }
    };

    let target_path = Path::new(target_dir);

    // Check if the target directory already exists
    if target_path.exists() {
        eprintln!("Target directory already exists: {}", target_dir);
        eprintln!("Do you want to use this directory anyway? (y/N)");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborting clone operation.");
            return Ok(());
        }
    } else {
        // Create the target directory
        println!("Creating directory: {}", target_dir);
        fs::create_dir_all(target_path).await?;
    }

    // Clone the repository
    println!("Cloning {} into {}", url_str, target_dir);
    let start_time = std::time::Instant::now();

    let repo = match AsyncRepository::clone(url, target_path).await {
        Ok(repo) => {
            let duration = start_time.elapsed();
            println!("Clone completed in {:.2} seconds", duration.as_secs_f64());
            repo
        },
        Err(e) => {
            eprintln!("Failed to clone repository: {}", e);
            return Ok(());
        }
    };

    // Get repository information
    println!("\nRepository information:");
    println!("=====================");

    // List branches
    match repo.list_branches().await {
        Ok(branches) => {
            println!("Branches:");
            for branch in branches {
                println!("  {}", branch);
            }
        },
        Err(e) => eprintln!("Failed to list branches: {}", e),
    }

    // Get the current commit
    match repo.get_commit(None).await {
        Ok(commit) => {
            println!("\nCurrent commit:");
            println!("  Hash: {}", commit.hash);
            println!("  Short hash: {}", commit.short_hash);
            println!("  Author: {} <{}>", commit.author_name, commit.author_email);
            println!("  Message: {}", commit.message);
        },
        Err(e) => eprintln!("Failed to get current commit: {}", e),
    }

    // Get status
    match repo.status().await {
        Ok(status) => {
            println!("\nStatus:");
            println!("  Branch: {}", status.branch.unwrap_or_else(|| "Unknown".to_string()));
            println!("  Clean: {}", status.is_clean);
            println!("  Files: {}", status.files.len());
        },
        Err(e) => eprintln!("Failed to get status: {}", e),
    }

    // Example: Create a new branch
    let new_branch_name = "example-branch";
    match BranchName::from_str(new_branch_name) {
        Ok(branch_name) => {
            println!("\nCreating new branch: {}", new_branch_name);
            match repo.create_local_branch(&branch_name).await {
                Ok(_) => println!("  Branch created successfully"),
                Err(e) => eprintln!("  Failed to create branch: {}", e),
            }
        },
        Err(e) => eprintln!("Invalid branch name: {}", e),
    }

    println!("\nAsync operations completed successfully!");
    Ok(())
}