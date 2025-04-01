//! Provides asynchronous versions of the Git operations.

use crate::error::GitError;
use crate::types::{BranchName, GitUrl, Result};
use crate::models::{
    Commit, StatusEntry, FileStatus, Branch, StatusResult, Remote, Tag,
    StashEntry, Worktree, BlameLine, DiffResult, DiffFile, DiffHunk,
    DiffLine, DiffLineType, ConfigEntry, ConfigScope, Submodule, LogResult
};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::str;

// These imports are now guarded by the "async" feature flag
use tokio::process::Command;

/// Represents a local Git repository with async operations.
///
/// This struct mirrors the functionality of the synchronous `Repository`
/// but uses asynchronous I/O for Git operations.
#[derive(Debug, Clone)]
pub struct AsyncRepository {
    location: PathBuf,
}

impl AsyncRepository {
    /// Creates an `AsyncRepository` instance pointing to an existing local Git repository.
    ///
    /// This does *not* check if the path is actually a valid Git repository.
    /// Operations will fail later if it's not.
    ///
    /// # Arguments
    /// * `p` - The path to the local repository's root directory.
    pub fn new<P: AsRef<Path>>(p: P) -> AsyncRepository {
        AsyncRepository {
            location: PathBuf::from(p.as_ref()),
        }
    }

    /// Clones a remote Git repository into a specified local path.
    ///
    /// Equivalent to `git clone <url> <path>`.
    ///
    /// # Arguments
    /// * `url` - The URL of the remote repository.
    /// * `p` - The target local path where the repository should be cloned.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn clone<P: AsRef<Path>>(url: GitUrl, p: P) -> Result<AsyncRepository> {
        let p_ref = p.as_ref();
        let cwd = tokio::fs::canonicalize(".").await
            .map_err(|_| GitError::WorkingDirectoryInaccessible)?;

        // Pass URL and Path directly as OsStr compatible args
        let args: Vec<&OsStr> = vec!["clone".as_ref(), url.as_ref(), p_ref.as_os_str()];

        execute_git_async(cwd, args).await?; // Execute in CWD, cloning *into* p

        Ok(AsyncRepository {
            location: PathBuf::from(p_ref),
        })
    }

    /// Initializes a new Git repository in the specified directory.
    ///
    /// Equivalent to `git init <path>`.
    ///
    /// # Arguments
    /// * `p` - The path to the directory to initialize.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn init<P: AsRef<Path>>(p: P) -> Result<AsyncRepository> {
        let p_ref = p.as_ref();
        execute_git_async(&p_ref, &["init"]).await?;
        Ok(AsyncRepository {
            location: PathBuf::from(p_ref),
        })
    }

    /// Creates and checks out a new local branch.
    ///
    /// Equivalent to `git checkout -b <branch_name>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name for the new branch.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn create_local_branch(&self, branch_name: &BranchName) -> Result<()> {
        execute_git_async(
            &self.location,
            &["checkout", "-b", branch_name.as_ref()],
        ).await
    }

    /// Checks out an existing local branch.
    ///
    /// Equivalent to `git checkout <branch_name>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name of the branch to switch to.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn switch_branch(&self, branch_name: &BranchName) -> Result<()> {
        execute_git_async(&self.location, &["checkout", branch_name.as_ref()]).await
    }

    /// Adds file contents to the Git index (staging area).
    ///
    /// Equivalent to `git add <pathspec>...`.
    ///
    /// # Arguments
    /// * `pathspecs` - A vector of file paths or patterns to add.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn add<S: AsRef<OsStr>>(&self, pathspecs: Vec<S>) -> Result<()> {
        let mut args: Vec<&OsStr> = Vec::with_capacity(pathspecs.len() + 1);
        args.push("add".as_ref());
        for spec in pathspecs.iter() {
            args.push(spec.as_ref());
        }
        execute_git_async(&self.location, args).await
    }

    /// Commits files currently in the staging area (index).
    ///
    /// Equivalent to `git commit -m <message>`.
    ///
    /// # Arguments
    /// * `message` - The commit message.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn commit_staged(&self, message: &str) -> Result<()> {
        execute_git_async(&self.location, &["commit", "-m", message]).await
    }

    /// Pushes the current branch to its configured upstream remote branch.
    ///
    /// Equivalent to `git push`.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn push(&self) -> Result<()> {
        execute_git_async(&self.location, &["push"]).await
    }

    /// Pushes the current branch to a specified remote and sets the upstream configuration.
    ///
    /// Equivalent to `git push -u <upstream_remote> <upstream_branch>`.
    ///
    /// # Arguments
    /// * `upstream_remote` - The name of the remote.
    /// * `upstream_branch` - The name of the branch on the remote.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn push_to_upstream(
        &self,
        upstream_remote: &str,
        upstream_branch: &BranchName,
    ) -> Result<()> {
        execute_git_async(
            &self.location,
            &["push", "-u", upstream_remote, upstream_branch.as_ref()],
        ).await
    }

    /// Lists the names of all local branches.
    ///
    /// Equivalent to `git branch --format='%(refname:short)'`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the branch names.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn list_branches(&self) -> Result<Vec<String>> {
        execute_git_fn_async(
            &self.location,
            &["branch", "--list", "--format=%(refname:short)"],
            |output| Ok(output.lines().map(|line| line.to_owned()).collect()),
        ).await
    }

    /// Gets the URL configured for a specific remote.
    ///
    /// Equivalent to `git config --get remote.<remote_name>.url`.
    ///
    /// # Arguments
    /// * `remote_name` - The name of the remote.
    ///
    /// # Returns
    /// The URL as a `String`.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn show_remote_uri(&self, remote_name: &str) -> Result<String> {
        execute_git_fn_async(
            &self.location,
            &[
                "config",
                "--get",
                &format!("remote.{}.url", remote_name),
            ],
            |output| Ok(output.trim().to_owned()),
        ).await
    }

    /// Lists branches with detailed information.
    ///
    /// # Returns
    /// A vector of `Branch` structs with branch details.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn list_branches_info(&self) -> Result<Vec<Branch>> {
        execute_git_fn_async(
            &self.location,
            &["branch", "--list", "-v", "--format=%(refname:short) %(objectname) %(HEAD) %(upstream:short)"],
            |output| {
                let mut branches = Vec::new();

                for line in output.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let name_str = parts[0];
                        let commit = parts[1].to_string();
                        let is_head = parts[2] == "*";

                        let upstream = if parts.len() >= 4 {
                            Some(parts[3].to_string())
                        } else {
                            None
                        };

                        // Parse the branch name, skipping invalid ones
                        if let Ok(name) = BranchName::from_str(name_str) {
                            branches.push(Branch {
                                name,
                                commit,
                                is_head,
                                upstream,
                            });
                        }
                    }
                }

                Ok(branches)
            }
        ).await
    }

    /// Gets detailed information about a commit.
    ///
    /// # Arguments
    /// * `commit_ref` - The commit reference. If `None`, uses HEAD.
    ///
    /// # Returns
    /// A `Commit` struct with commit details.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn get_commit(&self, commit_ref: Option<&str>) -> Result<Commit> {
        let format = "%H%n\
                     shortcommit %h%n\
                     author_name %an%n\
                     author_email %ae%n\
                     timestamp %at%n\
                     %P%n\
                     message %s";

        let args = match commit_ref {
            Some(c) => vec!["show", "--no-patch", &format!("--format={}", format), c],
            None => vec!["show", "--no-patch", &format!("--format={}", format)],
        };

        execute_git_fn_async(&self.location, args, |output| {
            Commit::from_show_format(output).ok_or_else(|| GitError::GitError {
                stdout: output.to_string(),
                stderr: "Failed to parse commit information".to_string(),
            })
        }).await
    }

    /// Gets the current status of the repository.
    ///
    /// # Returns
    /// A `StatusResult` struct with status details.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub async fn status(&self) -> Result<StatusResult> {
        // Get the porcelain status
        let porcelain_output = execute_git_fn_async(
            &self.location,
            &["status", "--porcelain=v2", "--branch"],
            |output| Ok(output.to_string())
        ).await?;

        let mut branch = None;
        let mut files = Vec::new();
        let mut merging = false;
        let mut rebasing = false;
        let mut cherry_picking = false;

        for line in porcelain_output.lines() {
            if line.starts_with("# branch.head ") {
                branch = Some(line.trim_start_matches("# branch.head ").to_string());
            } else if line.starts_with("1 ") || line.starts_with("2 ") || line.starts_with("u ") {
                // Parse file status
                let parts: Vec<&str> = line.split(' ').collect();
                if parts.len() >= 2 {
                    let status_code = if parts[0] == "1" && parts.len() >= 3 {
                        // Ordinary changed entries
                        let xy = parts[1];
                        if xy.len() >= 2 {
                            (xy.chars().nth(0).unwrap(), xy.chars().nth(1).unwrap())
                        } else {
                            (' ', ' ')
                        }
                    } else if parts[0] == "2" && parts.len() >= 9 {
                        // Renamed/copied entries
                        let xy = parts[1];
                        if xy.len() >= 2 {
                            (xy.chars().nth(0).unwrap(), xy.chars().nth(1).unwrap())
                        } else {
                            (' ', ' ')
                        }
                    } else if parts[0] == "u" && parts.len() >= 5 {
                        // Unmerged entries
                        let xy = parts[1];
                        if xy.len() >= 2 {
                            (xy.chars().nth(0).unwrap(), xy.chars().nth(1).unwrap())
                        } else {
                            (' ', ' ')
                        }
                    } else {
                        (' ', ' ')
                    };

                    let status = FileStatus::from_porcelain_code(status_code.0, status_code.1);

                    let path_index = if parts[0] == "1" {
                        2 // For ordinary changes
                    } else if parts[0] == "2" {
                        3 // For renamed/copied entries, path2 is at index 3
                    } else if parts[0] == "u" {
                        4 // For unmerged entries
                    } else {
                        2 // Default
                    };

                    if parts.len() > path_index {
                        let path = parts[path_index].to_string();

                        let original_path = if parts[0] == "2" && parts.len() > 2 {
                            // For renamed/copied entries, path1 is the original path
                            Some(PathBuf::from(parts[2]))
                        } else {
                            None
                        };

                        files.push(StatusEntry {
                            path: PathBuf::from(path),
                            status,
                            original_path,
                        });
                    }
                }
            } else if line.starts_with("? ") {
                // Untracked file
                if line.len() > 2 {
                    let path = line[2..].to_string();
                    files.push(StatusEntry {
                        path: PathBuf::from(path),
                        status: FileStatus::Untracked,
                        original_path: None,
                    });
                }
            }
        }

        // Check for special states asynchronously
        let git_dir = self.location.join(".git");

        let merge_exists = tokio::fs::try_exists(git_dir.join("MERGE_HEAD")).await.unwrap_or(false);
        if merge_exists {
            merging = true;
        }

        let rebase_apply_exists = tokio::fs::try_exists(git_dir.join("rebase-apply")).await.unwrap_or(false);
        let rebase_merge_exists = tokio::fs::try_exists(git_dir.join("rebase-merge")).await.unwrap_or(false);
        if rebase_apply_exists || rebase_merge_exists {
            rebasing = true;
        }

        let cherry_pick_exists = tokio::fs::try_exists(git_dir.join("CHERRY_PICK_HEAD")).await.unwrap_or(false);
        if cherry_pick_exists {
            cherry_picking = true;
        }

        let is_clean = files.is_empty();

        Ok(StatusResult {
            branch,
            files,
            merging,
            rebasing,
            cherry_picking,
            is_clean,
        })
    }

    /// Executes an arbitrary Git command within the repository context.
    ///
    /// # Arguments
    /// * `args` - An iterator yielding command-line arguments for Git.
    ///
    /// # Errors
    /// Returns `GitError` if the command fails or `git` cannot be executed.
    pub async fn cmd<I, S>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        execute_git_async(&self.location, args).await
    }

    /// Executes an arbitrary Git command within the repository context and returns its standard output.
    ///
    /// # Arguments
    /// * `args` - An iterator yielding command-line arguments for Git.
    ///
    /// # Returns
    /// A `Vec<String>` where each element is a line from the command's standard output.
    ///
    /// # Errors
    /// Returns `GitError` if the command fails or `git` cannot be executed.
    pub async fn cmd_out<I, S>(&self, args: I) -> Result<Vec<String>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        execute_git_fn_async(&self.location, args, |output| {
            Ok(output.lines().map(|line| line.to_owned()).collect())
        }).await
    }
}

// --- Private Helper Functions for async operations ---

/// Executes a Git command asynchronously, discarding successful output.
async fn execute_git_async<I, S, P>(p: P, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    execute_git_fn_async(p, args, |_| Ok(())).await
}

/// Executes a Git command asynchronously and processes its stdout on success using a closure.
/// Handles errors, including capturing stderr on failure.
async fn execute_git_fn_async<I, S, P, F, R>(p: P, args: I, process: F) -> Result<R>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
    F: FnOnce(&str) -> Result<R>,
{
    let process_output = Command::new("git")
        .current_dir(p.as_ref())
        .args(args)
        .output()
        .await;

    match process_output {
        Ok(output) => {
            if output.status.success() {
                // Attempt to decode stdout as UTF-8
                match str::from_utf8(&output.stdout) {
                    Ok(stdout_str) => process(stdout_str), // Process the valid UTF-8 stdout
                    Err(_) => Err(GitError::Undecodable),  // Stdout wasn't valid UTF-8
                }
            } else {
                // Command failed, try to capture stdout and stderr
                let stdout = str::from_utf8(&output.stdout)
                    .map(|s| s.trim_end().to_owned()) // Trim trailing newline
                    .unwrap_or_else(|_| String::from("[stdout: undecodable UTF-8]"));
                let stderr = str::from_utf8(&output.stderr)
                    .map(|s| s.trim_end().to_owned()) // Trim trailing newline
                    .unwrap_or_else(|_| String::from("[stderr: undecodable UTF-8]"));

                // Return the specific GitError variant with captured output
                Err(GitError::GitError { stdout, stderr })
            }
        }
        Err(e) => {
            // Failed to even execute the command (e.g., git not found, permissions)
            eprintln!("Failed to execute git command: {}", e); // Log the OS error
            Err(GitError::Execution)
        }
    }
}