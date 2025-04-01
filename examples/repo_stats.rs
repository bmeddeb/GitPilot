// examples/repo_stats.rs
//
// This example demonstrates how to use the GitPilot library to analyze a Git repository
// and generate statistics about it.

use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;
use std::collections::HashMap;
use chrono::{DateTime, Local, TimeZone}; // Make sure chrono is in Cargo.toml for the example

// Import the GitPilot library
use GitPilot::Repository;
// Updated imports
use GitPilot::types::{GitUrl, BranchName, Remote, CommitHash, Result as GitResult};
use GitPilot::models::{Commit, StatusResult, FileStatus, Branch}; // Import specific models used

// Struct definitions remain the same
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
        // Return Ok to avoid panic
        return Ok(());
    }

    let repo_path = PathBuf::from(&args[1]);
    let mut repo = None;

    // Repository opening/cloning logic remains the same
    if args.len() >= 4 && args[2] == "--clone" {
        let url = GitUrl::from_str(&args[3])?;
        println!("Cloning repository from {} to {}...", url, repo_path.display());
        repo = Some(Repository::clone(url, &repo_path)?);
    } else if repo_path.exists() {
        repo = Some(Repository::new(&repo_path));
    } else {
        eprintln!("Error: Directory does not exist: {}. Use --clone to clone a repository.", repo_path.display());
        // Return Ok to avoid panic
        return Ok(());
    }

    // Use expect for simplicity in example, real code might handle None better
    let repo = repo.expect("Repository should have been opened or cloned");

    // Get basic repository information
    println!("Repository Analysis for: {}", repo_path.display());
    println!("==========================");

    // Get current branch
    let branches = repo.list_branches_info()?;
    let current_branch = branches.iter().find(|b| b.is_head);

    if let Some(branch) = current_branch {
        // branch.name is BranchName, works with Display
        println!("Current branch: {}", branch.name);
    } else {
        println!("Not on any branch (detached HEAD)");
    }

    // Get remote URLs
    // list_remotes now returns Vec<Remote>
    let remotes_result = repo.list_remotes();

    println!("\nRemotes:");
    match remotes_result {
        Ok(remote_list) => {
            if remote_list.is_empty() {
                println!("  (No remotes configured)");
            } else {
                for remote_name in remote_list { // remote_name is Remote
                    // show_remote_uri takes &Remote, returns GitUrl
                    match repo.show_remote_uri(&remote_name) {
                        Ok(url) => {
                            // name and url work with Display
                            println!("  {} -> {}", remote_name, url);
                        },
                        Err(e) => {
                            println!("  {} -> (Failed to get URL: {})", remote_name, e);
                        }
                    }
                }
            }
        },
        Err(ref e) if matches!(e, GitPilot::error::GitError::NoRemoteRepositorySet) => {
            println!("  (No remotes configured)");
        }
        Err(e) => {
            eprintln!("  Failed to list remotes: {}", e);
        }
    }


    // Get HEAD commit
    match repo.get_commit(None) { // Returns Commit (which should use CommitHash internally)
        Ok(head_commit) => {
            println!("\nCurrent HEAD:");
            // head_commit.short_hash is CommitHash, works with Display
            println!("  Commit: {} ({})",
                     head_commit.short_hash,
                     // FIX: Add .latest() to convert LocalResult -> Option
                     Local.timestamp_opt(head_commit.timestamp as i64, 0)
                          .latest() // <-- Add this
                          .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                          .unwrap_or_else(|| "Invalid Date".to_string())
            );
            println!("  Author: {} <{}>", head_commit.author_name, head_commit.author_email);
            println!("  Message: {}", head_commit.message);
        },
        Err(e) => eprintln!("Failed to get HEAD commit: {}", e),
    }

    // Calculate commit statistics
    println!("\nAnalyzing repository history...");

    // Get all commit hashes
    let log_output = repo.cmd_out(["log", "--format=%H"])?; // Returns Vec<String>
    let total_commits = log_output.len();
    println!("Total commits found: {}", total_commits);

    // Limiting analysis for performance in example
    let limit = std::cmp::min(100, total_commits);
    if limit < total_commits {
        println!("Analyzing stats for the latest {} commits...", limit);
    } else {
        println!("Analyzing stats for all {} commits...", limit);
    }


    let mut commit_stats = Vec::new();
    for i in 0..limit {
        let commit_hash_str = &log_output[i]; // This is &String

        // Get commit details
        // get_commit takes Option<&str>, &String derefs to &str - OK
        if let Ok(commit) = repo.get_commit(Some(commit_hash_str)) {
            // For each commit, calculate the diff statistics
            let mut stats = CommitStats {
                author: commit.author_name.clone(),
                timestamp: commit.timestamp,
                added_lines: 0,
                removed_lines: 0,
                files_changed: 0,
            };

            // Calculate diff with the first parent if it exists
            // commit.parents is Vec<CommitHash>
            if let Some(parent_hash) = commit.parents.first() { // Use first() to get Option<&CommitHash>
                // --- FIX: Pass refs correctly to cmd_out ---
                // parent_hash is &CommitHash, use as_ref() -> &str
                // commit_hash_str is &String, use as_ref() -> &str or rely on deref
                let diff_output = repo.cmd_out([
                    "diff",
                    "--numstat",
                    parent_hash.as_ref(), // &str from &CommitHash
                    commit_hash_str.as_ref(), // &str from &String
                ])?;
                // --- End Fix ---

                stats.files_changed = diff_output.len();

                for diff_line in diff_output {
                    let parts: Vec<&str> = diff_line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        // numstat format is <added> <removed> <path>
                        // Handle '-' for binary files
                        if let Ok(added) = parts[0].parse::<usize>() {
                            stats.added_lines += added;
                        }
                        if let Ok(removed) = parts[1].parse::<usize>() {
                            stats.removed_lines += removed;
                        }
                    }
                }
            } else {
                // Initial commit - try diffing against the empty tree?
                // `git diff --numstat 4b825dc642cb6eb9a060e54bf8d69288fbee4904` (empty tree hash)
                // Or just count lines in the commit using `git show --numstat <commit>`
                // For simplicity in example, we'll skip diff for initial commit.
                stats.files_changed = 0; // Assume 0 diff for initial commit in this example
            }

            commit_stats.push(stats);
        } else {
            eprintln!("Warning: Failed to get commit details for {}", commit_hash_str);
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
    println!("\nAuthor Statistics (Top {} Commits):", limit);
    println!("{:<25} {:<10} {:<10} {:<10} {:<15} {:<15}",
             "Author", "Commits", "Added", "Removed", "First Commit", "Last Commit");
    println!("{}", "-".repeat(90)); // Adjusted separator length

    // Sort authors for consistent output, e.g., by commit count
    let mut sorted_authors: Vec<_> = author_stats.iter().collect();
    sorted_authors.sort_by(|a, b| b.1.commits.cmp(&a.1.commits));

    for (author, stats) in sorted_authors {
        let first_date = Local.timestamp_opt(stats.first_commit as i64, 0)
            .latest() // <-- Add this
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "N/A".to_string());
        let last_date = Local.timestamp_opt(stats.last_commit as i64, 0)
            .latest() // <-- Add this
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "N/A".to_string());

        // Limit author name display width
        let display_author = if author.len() > 23 { &author[..22] } else { author };

        println!("{:<25} {:<10} {:<10} {:<10} {:<15} {:<15}",
                 display_author, stats.commits, stats.added_lines, stats.removed_lines, first_date, last_date);
    }

    // Calculate overall statistics
    let total_authors = author_stats.len();
    let total_added = commit_stats.iter().map(|s| s.added_lines).sum::<usize>();
    let total_removed = commit_stats.iter().map(|s| s.removed_lines).sum::<usize>();
    let total_changed_files = commit_stats.iter().map(|s| s.files_changed).sum::<usize>();

    println!("\nOverall Statistics (Top {} Commits):", limit);
    println!("  Total commits analyzed: {}", commit_stats.len());
    println!("  Total unique authors: {}", total_authors);
    println!("  Total lines added: {}", total_added);
    println!("  Total lines removed: {}", total_removed);
    println!("  Avg files changed per commit: {:.2}", if !commit_stats.is_empty() { total_changed_files as f64 / commit_stats.len() as f64 } else { 0.0 });


    // Calculate commit frequency over the analyzed period
    if let (Some(latest), Some(earliest)) = (
        commit_stats.iter().map(|s| s.timestamp).max(),
        commit_stats.iter().map(|s| s.timestamp).min()
    ) {
        if latest > earliest { // Avoid division by zero if only one commit analyzed
            let days = (latest - earliest) as f64 / (60.0 * 60.0 * 24.0);
            if days >= 1.0 { // Only show if period is at least a day
                let commits_per_day = commit_stats.len() as f64 / days;
                println!("  Commit frequency: {:.2} commits/day (over {:.1} days analyzed)",
                         commits_per_day, days);
            } else {
                println!("  Analysis period less than a day.");
            }
        } else if !commit_stats.is_empty() {
            println!("  Only one commit timepoint analyzed.");
        }
    }

    // Get current repository status
    match repo.status() { // Returns StatusResult (which should use BranchName internally)
        Ok(status) => {
            println!("\nCurrent Repository Status:");
            // status.branch is Option<BranchName>, format it
            let branch_display = status.branch
                .map(|b| b.to_string())
                .unwrap_or_else(|| "(Detached HEAD)".to_string());
            println!("  Branch: {}", branch_display);
            println!("  Is Clean: {}", status.is_clean);

            if !status.files.is_empty() {
                println!("  Working Directory Changes: {}", status.files.len());

                // Added use GitPilot::models::FileStatus; at the top
                for entry in status.files.iter().take(10) { // Limit output
                    // Use simple match for display
                    let status_str = match entry.status {
                        FileStatus::Modified => "Modified",
                        FileStatus::Added => "Added",
                        FileStatus::Deleted => "Deleted (WT)", // Clarify Working Tree delete
                        FileStatus::DeletedStaged => "Deleted (Staged)",
                        FileStatus::Renamed => "Renamed",
                        FileStatus::Copied => "Copied",
                        FileStatus::UpdatedButUnmerged => "Unmerged",
                        FileStatus::Untracked => "Untracked",
                        FileStatus::Ignored => "Ignored",
                        FileStatus::Unmodified => "Unmodified", // Should ideally be filtered by is_clean
                    };
                    println!("    {:<18}: {}", status_str, entry.path.display());
                }
                if status.files.len() > 10 { println!("    ... and {} more.", status.files.len() - 10); }
            }

            if status.merging { println!("  Repository is in MERGE state."); }
            if status.rebasing { println!("  Repository is in REBASE state."); }
            if status.cherry_picking { println!("  Repository is in CHERRY-PICK state."); }

        },
        Err(e) => eprintln!("Failed to get repository status: {}", e),
    }

    println!("\nAnalysis complete!");
    Ok(())
}