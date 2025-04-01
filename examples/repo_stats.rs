// examples/repo_stats.rs
//
// This example demonstrates how to use the GitPilot library to analyze a Git repository
// and generate statistics about it.

use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;
use std::collections::HashMap;
use chrono::{DateTime, Local, TimeZone};

// Import the GitPilot library
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
    let remotes = match repo.list_remotes() {
        Ok(remote_names) => {
            let mut remote_urls = Vec::new();
            for name in remote_names {
                if let Ok(url) = repo.show_remote_uri(&name) {
                    remote_urls.push((name, url));
                }
            }
            remote_urls
        },
        Err(_) => Vec::new(),
    };

    println!("\nRemotes:");
    for (name, url) in &remotes {
        println!("  {} -> {}", name, url);
    }

    // Get HEAD commit
    match repo.get_commit(None) {
        Ok(head_commit) => {
            println!("\nCurrent HEAD:");
            println!("  Commit: {} ({})",
                     head_commit.short_hash,
                     Local.timestamp_opt(head_commit.timestamp as i64, 0)
                         .unwrap()
                         .format("%Y-%m-%d %H:%M:%S")
            );
            println!("  Author: {} <{}>", head_commit.author_name, head_commit.author_email);
            println!("  Message: {}", head_commit.message);
        },
        Err(e) => eprintln!("Failed to get HEAD commit: {}", e),
    }

    // Calculate commit statistics
    println!("\nAnalyzing repository history...");

    // Get all commits
    let log_output = repo.cmd_out(["log", "--format=%H"])?;
    let total_commits = log_output.len();
    println!("Total commits: {}", total_commits);

    // Limiting to 100 commits for performance
    let limit = std::cmp::min(100, total_commits);
    println!("Analyzing last {} commits", limit);

    let mut commit_stats = Vec::new();
    for i in 0..limit {
        let commit_hash = &log_output[i];

        // Get commit details
        if let Ok(commit) = repo.get_commit(Some(commit_hash)) {
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
                let diff_output = repo.cmd_out(["diff", "--numstat", parent, commit_hash])?;

                stats.files_changed = diff_output.len();

                for diff_line in diff_output {
                    let parts: Vec<&str> = diff_line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        if let Ok(added) = parts[0].parse::<usize>() {
                            stats.added_lines += added;
                        }
                        if let Ok(removed) = parts[1].parse::<usize>() {
                            stats.removed_lines += removed;
                        }
                    }
                }
            }

            commit_stats.push(stats);
        }
    }

    // Aggregate statistics by author
    let mut author_stats = HashMap::new();
    for stats in &commit_stats {
        let entry = author_stats.entry(stats.author.clone()).or_insert_with(|| AuthorStats {
            commits: 0,
            added_lines: 0,
            removed_lines: 0,
            files_changed: 0,
            first_commit: std::u64::MAX,
            last_commit: 0,
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

    // Fixed: Use a reference to author_stats to avoid moving it
    for (author, stats) in &author_stats {
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

    // Calculate overall statistics
    let total_authors = author_stats.len();
    let total_added = commit_stats.iter().map(|s| s.added_lines).sum::<usize>();
    let total_removed = commit_stats.iter().map(|s| s.removed_lines).sum::<usize>();
    let total_changed_files = commit_stats.iter().map(|s| s.files_changed).sum::<usize>();

    println!("\nOverall Statistics:");
    println!("  Total commits analyzed: {}", commit_stats.len());
    println!("  Total authors: {}", total_authors);
    println!("  Total lines added: {}", total_added);
    println!("  Total lines removed: {}", total_removed);
    println!("  Total files changed: {}", total_changed_files);

    // Calculate commit frequency
    if let (Some(latest), Some(earliest)) = (
        commit_stats.iter().map(|s| s.timestamp).max(),
        commit_stats.iter().map(|s| s.timestamp).min()
    ) {
        let days = (latest - earliest) as f64 / (60.0 * 60.0 * 24.0);
        if days > 0.0 {
            let commits_per_day = commit_stats.len() as f64 / days;
            println!("  Commit frequency: {:.2} commits per day over {:.1} days",
                     commits_per_day, days);
        }
    }

    // Get current repository status
    match repo.status() {
        Ok(status) => {
            println!("\nCurrent Repository Status:");
            if status.is_clean {
                println!("  Working directory is clean");
            } else {
                println!("  Modified files: {}", status.files.len());

                for entry in &status.files {
                    let status_str = match entry.status {
                        GitPilot::models::FileStatus::Modified => "modified",
                        GitPilot::models::FileStatus::Added => "added",
                        GitPilot::models::FileStatus::Deleted => "deleted",
                        GitPilot::models::FileStatus::DeletedStaged => "deleted (staged)",
                        GitPilot::models::FileStatus::Untracked => "untracked",
                        _ => "other",
                    };

                    println!("    {}: {}", status_str, entry.path.display());
                }
            }

            if status.merging {
                println!("  Repository is currently in the middle of a merge");
            }

            if status.rebasing {
                println!("  Repository is currently in the middle of a rebase");
            }
        },
        Err(e) => eprintln!("Failed to get repository status: {}", e),
    }

    println!("\nAnalysis complete!");
    Ok(())
}