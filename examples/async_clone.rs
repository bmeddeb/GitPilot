// examples/async_clone.rs
//
// This example demonstrates the use of the async API in GitPilot to
// clone a Git repository and then perform some basic operations.

use std::error::Error;
use std::path::Path;
use std::str::FromStr;
use tokio::fs;

use GitPilot::AsyncRepository;
use GitPilot::types::{BranchName, GitUrl}; // Import types used

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <repo_url> <target_directory>", args[0]);
        eprintln!(
            "Example: {} https://github.com/rust-lang/rust-analyzer.git ./rust-analyzer",
            args[0]
        );
        // Return Ok to avoid panic on incorrect usage
        return Ok(());
    }

    let url_str = &args[1];
    let target_dir = &args[2];

    // Parse the Git URL
    let url = match GitUrl::from_str(url_str) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Error parsing Git URL: {}", e);
            // Return Ok to avoid panic
            return Ok(());
        }
    };

    let target_path = Path::new(target_dir);

    // Check if the target directory already exists
    if target_path.exists() {
        // Use async exists check if preferred, but sync is often fine for startup checks
        // if tokio::fs::metadata(target_path).await.is_ok() { ... }

        eprintln!("Target directory already exists: {}", target_dir);
        // Optionally remove the prompt for non-interactive use
        eprintln!("Do you want to remove it and continue? (y/N)");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?; // Sync read is okay for CLI prompt

        if input.trim().eq_ignore_ascii_case("y") {
            println!("Removing existing directory...");
            fs::remove_dir_all(target_path).await?; // Use async remove
        } else {
            eprintln!("Aborting clone operation. Target directory exists.");
            return Ok(());
        }
    }

    // Ensure the directory exists now (create if removed or didn't exist)
    // Create the parent directory if necessary as well
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).await?;
    }


    // Clone the repository
    println!("Cloning {} into {}", url_str, target_dir);
    let start_time = std::time::Instant::now();

    let repo = match AsyncRepository::clone(url, target_path).await {
        Ok(repo) => {
            let duration = start_time.elapsed();
            println!(
                "Clone completed successfully in {:.2} seconds",
                duration.as_secs_f64()
            );
            repo
        }
        Err(e) => {
            eprintln!("Failed to clone repository: {}", e);
            // Attempt cleanup if clone failed after creating directory
            if !target_path.exists() { // Check if clone partially created it
                let _ = fs::remove_dir_all(target_path).await;
            }
            // Return Ok to avoid panic
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
            if branches.is_empty() {
                println!("  (No local branches found - possibly a bare clone?)");
            } else {
                // This works because BranchName implements Display
                for branch in branches {
                    println!("  {}", branch);
                }
            }
        }
        Err(e) => eprintln!("Failed to list branches: {}", e),
    }

    // Get the current commit
    match repo.get_commit(None).await {
        Ok(commit) => {
            println!("\nCurrent commit (HEAD):");
            // This works because CommitHash implements Display
            println!("  Hash: {}", commit.hash);
            println!("  Short hash: {}", commit.short_hash);
            println!("  Author: {} <{}>", commit.author_name, commit.author_email);
            // Assuming commit message is single line from format %s
            println!("  Message: {}", commit.message);
        }
        Err(e) => eprintln!("Failed to get current commit: {}", e),
    }

    // Get status
    match repo.status().await {
        Ok(status) => {
            println!("\nStatus:");
            // --- FIX: Handle Option<BranchName> ---
            let branch_display = status
                .branch // This is Option<BranchName>
                .map(|b_name| b_name.to_string()) // Map BranchName to String
                .unwrap_or_else(|| "(Detached HEAD or unknown)".to_string()); // Provide default
            println!("  Current Branch: {}", branch_display);
            // --- End Fix ---
            println!("  Is Clean: {}", status.is_clean);
            if !status.files.is_empty() {
                println!("  Changed Files: {}", status.files.len());
                // Optionally print file details
                // for entry in status.files.iter().take(5) { // Print first 5
                //     println!("    - {:?}: {}", entry.status, entry.path.display());
                // }
                // if status.files.len() > 5 { println!("    ..."); }
            } else {
                println!("  Changed Files: 0");
            }
        }
        Err(e) => eprintln!("Failed to get status: {}", e),
    }

    // Example: Create a new branch
    let new_branch_name_str = "pilot-git-example-branch";
    match BranchName::from_str(new_branch_name_str) {
        Ok(branch_name) => {
            println!("\nAttempting to create new branch: {}", new_branch_name_str);
            match repo.create_local_branch(&branch_name).await {
                Ok(_) => println!("  Branch '{}' created successfully", branch_name),
                Err(ref e) if e.to_string().contains("already exists") => {
                    println!("  Branch '{}' already exists, switching to it.", branch_name);
                    if let Err(switch_e) = repo.switch_branch(&branch_name).await {
                        eprintln!("  Failed to switch to existing branch '{}': {}", branch_name, switch_e);
                    } else {
                        println!("  Switched to branch '{}'", branch_name);
                    }
                }
                Err(e) => eprintln!("  Failed to create branch '{}': {}", branch_name, e),
            }
        }
        Err(e) => eprintln!("Invalid branch name provided in example: {}", e), // Should not happen for valid literal
    }

    println!("\nAsync example operations completed!");
    Ok(())
}