// This example demonstrates how to use the GitPilot library to analyze a Git repository
// and generate statistics about it.

use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;
use std::collections::HashMap;
use chrono::{DateTime, Local, TimeZone};

// Update the import paths to match your crate name
use GitPilot::Repository;
use GitPilot::types::{GitUrl, BranchName, Result as GitResult};
use GitPilot::models::{Commit, DiffLineType};

struct CommitStats {
    author: String,
    timestamp: u64,
    added_lines: usize,
    removed_lines: usize,
    files_changed: usize,
}

struct AuthorStats {
    commits: usize,
    added_lines: usize,
    removed_lines: usize,
    files_changed: usize,
    first_commit: u64,
    last_commit: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <repository_path> [--clone <repo_url>]", args[0]);
        return Ok(());
    }

    let repo_path = PathBuf::from(&args[1]);
    let mut repo = None;

    if args.len() >= 4 && args[2] == "--clone" {
        // Clone the repository if requested
        let url = GitUrl::from_str(&args[3])?;
        println!("Cloning repository from {} to {}...", url, repo_path.display());
        repo = Some(Repository::clone(url, &repo_path)?);
    } else if repo_path.exists() {
        // Open an existing repository
        repo = Some(Repository::new(&repo_path));
    } else {
        eprintln!("Error: Directory does not exist. Use --clone to clone a repository.");
        return Ok(());
    }

    let repo = repo.unwrap();

    // Get basic repository information
    println!("Repository Analysis");
    println!("==================");

    // Get current branch
    let branches = repo.list_branches_info()?;
    let current_branch = branches.iter().find(|b| b.is_head);

    if let Some(branch) = current_branch {
        println!("Current branch: {}", branch.name);
    } else {
        println!("Not on any branch (detached HEAD)");
    }

    // Get remote URLs
    let remotes = repo.list_remotes()?;
    println!("\nRemotes:");
    for remote in &remotes {
        println!("  {} -> {}", remote.name(), remote.url);
    }

    // Get commit history (limited to 100 commits for this example)
    let log_result = repo.log_parsed(Some(100), None)?;
    println!("\nFound {} commits", log_result.commits.len());

    // Calculate commit statistics
    let mut commit_stats = Vec::new();
    for commit in &log_result.commits {
        // For each commit, calculate the diff statistics
        let mut stats = CommitStats {
            author: commit.author_name.clone(),
            timestamp: commit.timestamp,
            added_lines: 0,
            removed_lines: 0,
            files_changed: 0,
        };

        // If this is not the first commit, calculate diff with the parent
        if !commit.parents.is_empty() {
            let parent = &commit.parents[0];
            let diff_result = repo.diff_parsed(Some(parent), Some(&commit.hash), None)?;

            stats.files_changed = diff_result.files.len();

            for file in &diff_result.files {
                stats.added_lines += file.added_lines;
                stats.removed_lines += file.removed_lines;
            }
        }

        commit_stats.push(stats);
    }

    // Aggregate statistics by author
    let mut author_stats = HashMap::new();
    for stats in &commit_stats {
        let entry = author_stats.entry(stats.author.clone()).or_insert_with(|| AuthorStats {
            commits: 0,
            added_lines: 0,
            removed_lines: 0,
            files_changed: 0,
            first_commit: stats.timestamp,
            last_commit: stats.timestamp,
        });

        entry.commits += 1;
        entry.added_lines += stats.added_lines;
        entry.removed_lines += stats.removed_lines;
        entry.files_changed += stats.files_changed;
        entry.first_commit = entry.first_commit.min(stats.timestamp);
        entry.last_commit = entry.last_commit.max(stats.timestamp);
    }

    // Display author statistics
    println!("\nAuthor Statistics:");
    println!("{:<20} {:<10} {:<10} {:<10} {:<15} {:<15}",
             "Author", "Commits", "Added", "Removed", "First Commit", "Last Commit");
    println!("{}", "-".repeat(80));

    for (author, stats) in author_stats {
        let first_date = Local.timestamp_opt(stats.first_commit as i64, 0)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        let last_date = Local.timestamp_opt(stats.last_commit as i64, 0)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();

        println!("{:<20} {:<10} {:<10} {:<10} {:<15} {:<15}",
                 author, stats.commits, stats.added_lines, stats.removed_lines, first_date, last_date);
    }

    // Calculate commit frequency
    let time_span = if let (Some(latest), Some(earliest)) = (
        commit_stats.iter().map(|s| s.timestamp).max(),
        commit_stats.iter().map(|s| s.timestamp).min()
    ) {
        let days = (latest - earliest) as f64 / (60.0 * 60.0 * 24.0);
        if days > 0.0 {
            let commits_per_day = commit_stats.len() as f64 / days;
            println!("\nCommit frequency: {:.2} commits per day over {:.1} days",
                     commits_per_day, days);
        }
    };

    // Get current status
    let status = repo.status()?;
    println!("\nCurrent Repository Status:");
    println!("{}", Repository::format_status(&status));

    Ok(())
}