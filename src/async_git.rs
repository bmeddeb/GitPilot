//! Provides asynchronous versions of the Git operations using tokio.

use crate::error::GitError;
// Import specific types for integration
use crate::types::{BranchName, CommitHash, GitUrl, Remote, Result}; // Added CommitHash, Remote
use crate::models::{
                     Commit, StatusEntry, FileStatus, Branch, StatusResult,
};
use std::ffi::OsStr;
use std::io::ErrorKind; // Needed for GitNotFound check
use std::path::{Path, PathBuf};
use std::str::{self, FromStr}; // Added FromStr for parsing

// Use tokio::process::Command for async execution
use tokio::process::Command;

/// Represents a local Git repository with async operations.
///
/// This struct mirrors the functionality of the synchronous `Repository`
/// but uses asynchronous I/O (via tokio) for Git operations.
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

    /// Clones a remote Git repository into a specified local path asynchronously.
    ///
    /// Equivalent to `git clone <url> <path>`.
    ///
    /// # Arguments
    /// * `url` - The URL of the remote repository.
    /// * `p` - The target local path where the repository should be cloned.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn clone<P: AsRef<Path>>(url: GitUrl, p: P) -> Result<AsyncRepository> {
        let p_ref = p.as_ref();
        // Use tokio's async canonicalize or just get current dir if needed
        // Note: Cloning doesn't strictly need the CWD if target path is absolute or relative to process CWD.
        // Using "." might be sufficient if running from expected location.
        let cwd = PathBuf::from("."); // Simplified CWD handling for clone

        let args: Vec<&OsStr> = vec!["clone".as_ref(), url.as_ref(), p_ref.as_os_str()];

        execute_git_async(cwd, args).await?; // Execute in CWD, cloning *into* p

        Ok(AsyncRepository {
            location: PathBuf::from(p_ref),
        })
    }

    /// Initializes a new Git repository in the specified directory asynchronously.
    ///
    /// Equivalent to `git init <path>`.
    ///
    /// # Arguments
    /// * `p` - The path to the directory to initialize.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn init<P: AsRef<Path>>(p: P) -> Result<AsyncRepository> {
        let p_ref = p.as_ref();
        execute_git_async(&p_ref, &["init"]).await?;
        Ok(AsyncRepository {
            location: PathBuf::from(p_ref),
        })
    }

    /// Creates and checks out a new local branch asynchronously.
    ///
    /// Equivalent to `git checkout -b <branch_name>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name for the new branch.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn create_local_branch(&self, branch_name: &BranchName) -> Result<()> {
        execute_git_async(
            &self.location,
            &["checkout", "-b", branch_name.as_ref()],
        ).await
    }

    /// Checks out an existing local branch asynchronously.
    ///
    /// Equivalent to `git checkout <branch_name>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name of the branch to switch to.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn switch_branch(&self, branch_name: &BranchName) -> Result<()> {
        execute_git_async(&self.location, &["checkout", branch_name.as_ref()]).await
    }

    /// Adds file contents to the Git index (staging area) asynchronously.
    ///
    /// Equivalent to `git add <pathspec>...`.
    ///
    /// # Arguments
    /// * `pathspecs` - A vector of file paths or patterns to add.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn add<S: AsRef<OsStr>>(&self, pathspecs: Vec<S>) -> Result<()> {
        let mut args: Vec<&OsStr> = Vec::with_capacity(pathspecs.len() + 1);
        args.push("add".as_ref());
        // Note: Need to handle lifetime if pathspecs is consumed. Cloning or iterating refs is safer.
        let refs: Vec<_> = pathspecs.iter().map(|s| s.as_ref()).collect();
        args.extend(refs);
        execute_git_async(&self.location, args).await
    }

    /// Removes files from the working tree and the index asynchronously.
    /// Added based on sync Repository.
    ///
    /// # Arguments
    /// * `pathspecs` - A vector of file paths or patterns to remove.
    /// * `force` - If `true`, corresponds to the `-f` flag (force removal).
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn remove<S: AsRef<OsStr>>(&self, pathspecs: Vec<S>, force: bool) -> Result<()> {
        let mut args: Vec<&OsStr> = Vec::with_capacity(pathspecs.len() + 2);
        args.push("rm".as_ref());
        if force {
            args.push("-f".as_ref());
        }
        let refs: Vec<_> = pathspecs.iter().map(|s| s.as_ref()).collect();
        args.extend(refs);
        execute_git_async(&self.location, args).await
    }

    /// Stages all tracked, modified/deleted files and commits them asynchronously.
    /// Added based on sync Repository.
    ///
    /// Equivalent to `git commit -am <message>`.
    ///
    /// # Arguments
    /// * `message` - The commit message.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn stage_and_commit_all_modified(&self, message: &str) -> Result<()> {
        execute_git_async(&self.location, &["commit", "-am", message]).await
    }


    /// Commits files currently in the staging area (index) asynchronously.
    ///
    /// Equivalent to `git commit -m <message>`.
    ///
    /// # Arguments
    /// * `message` - The commit message.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn commit_staged(&self, message: &str) -> Result<()> {
        execute_git_async(&self.location, &["commit", "-m", message]).await
    }

    /// Pushes the current branch to its configured upstream remote branch asynchronously.
    ///
    /// Equivalent to `git push`.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn push(&self) -> Result<()> {
        execute_git_async(&self.location, &["push"]).await
    }

    /// Pushes the current branch to a specified remote and sets the upstream configuration asynchronously.
    ///
    /// Equivalent to `git push -u <upstream_remote> <upstream_branch>`.
    ///
    /// # Arguments
    /// * `upstream_remote` - The name of the remote. (Changed to &Remote)
    /// * `upstream_branch` - The name of the branch on the remote.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn push_to_upstream(
        &self,
        upstream_remote: &Remote, // Changed type
        upstream_branch: &BranchName,
    ) -> Result<()> {
        execute_git_async(
            &self.location,
            &[
                "push",
                "-u",
                upstream_remote.as_ref(), // Use AsRef
                upstream_branch.as_ref(),
            ],
        ).await
    }

    /// Adds a new remote repository reference asynchronously.
    /// Added based on sync Repository.
    ///
    /// Equivalent to `git remote add <name> <url>`.
    ///
    /// # Arguments
    /// * `name` - The name for the new remote.
    /// * `url` - The URL of the remote repository.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn add_remote(&self, name: &Remote, url: &GitUrl) -> Result<()> { // Changed type
        execute_git_async(&self.location, &["remote", "add", name.as_ref(), url.as_ref()]).await // Use AsRef
    }

    /// Fetches updates from a specified remote repository asynchronously.
    /// Added based on sync Repository.
    ///
    /// Equivalent to `git fetch <remote>`.
    ///
    /// # Arguments
    /// * `remote` - The name of the remote to fetch from.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn fetch_remote(&self, remote: &Remote) -> Result<()> { // Changed type
        execute_git_async(&self.location, &["fetch", remote.as_ref()]).await // Use AsRef
    }

    /// Creates and checks out a new branch starting from a given point asynchronously.
    /// Added based on sync Repository.
    ///
    /// Equivalent to `git checkout -b <branch_name> <startpoint>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name for the new branch.
    /// * `startpoint` - The reference to branch from (e.g., "main", commit hash).
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn create_branch_from_startpoint(
        &self,
        branch_name: &BranchName,
        startpoint: &str, // Keeping as &str for flexibility
    ) -> Result<()> {
        execute_git_async(
            &self.location,
            &[
                "checkout",
                "-b",
                branch_name.as_ref(),
                startpoint,
            ],
        ).await
    }


    /// Lists the names of all local branches asynchronously.
    ///
    /// Equivalent to `git branch --format='%(refname:short)'`.
    ///
    /// # Returns
    /// A `Vec<BranchName>` containing the branch names. (Changed return type)
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn list_branches(&self) -> Result<Vec<BranchName>> { // Changed return type
        execute_git_fn_async(
            &self.location,
            &["branch", "--list", "--format=%(refname:short)"],
            |output| {
                output
                    .lines()
                    .map(|line| BranchName::from_str(line.trim())) // Parse each line
                    .collect::<Result<Vec<BranchName>>>() // Collect into Result<Vec<...>>
            },
        ).await
    }

    /// Lists all files currently tracked by Git in the working directory asynchronously.
    /// Added based on sync Repository.
    ///
    /// Equivalent to `git ls-files`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of tracked files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn list_tracked(&self) -> Result<Vec<String>> {
        execute_git_fn_async(&self.location, &["ls-files"], |output| {
            Ok(output.lines().map(|line| line.to_owned()).collect())
        }).await
    }


    /// Gets the URL configured for a specific remote asynchronously.
    ///
    /// Equivalent to `git config --get remote.<remote_name>.url`.
    ///
    /// # Arguments
    /// * `remote_name` - The name of the remote. (Changed to &Remote)
    ///
    /// # Returns
    /// The URL as a `GitUrl`. (Changed return type)
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn show_remote_uri(&self, remote_name: &Remote) -> Result<GitUrl> { // Changed args & return type
        execute_git_fn_async(
            &self.location,
            &[
                "config",
                "--get",
                // --- FIX: Pass remote_name directly ---
                &format!("remote.{}.url", remote_name),
            ],
            |output| GitUrl::from_str(output.trim()), // Parse output into GitUrl
        ).await
    }

    /// Lists the names of all configured remotes asynchronously.
    /// Added based on sync Repository.
    ///
    /// Equivalent to `git remote`.
    ///
    /// # Returns
    /// A `Vec<Remote>` containing the remote names.
    ///
    /// # Errors
    /// Returns `GitError::NoRemoteRepositorySet` if no remotes are configured.
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn list_remotes(&self) -> Result<Vec<Remote>> { // Changed return type
        execute_git_fn_async(&self.location, &["remote"], |output| {
            let remote_names: Vec<&str> = output.lines().map(|line| line.trim()).collect();
            if remote_names.is_empty() {
                // Re-check using config asynchronously
                // This requires cmd_out_async or similar - let's implement that first
                // For now, return error if empty, assuming config check happens elsewhere or is deferred
                Err(GitError::NoRemoteRepositorySet)
                // TODO: Implement async config check later if needed
            } else {
                remote_names
                    .into_iter()
                    .map(Remote::from_str) // Parse each name
                    .collect::<Result<Vec<Remote>>>() // Collect into Result<Vec<...>>
            }
        }).await
    }

    /// Obtains the commit hash (SHA-1) of the current `HEAD` asynchronously.
    /// Added based on sync Repository.
    ///
    /// Equivalent to `git rev-parse [--short] HEAD`.
    ///
    /// # Arguments
    /// * `short` - If `true`, returns the abbreviated short hash.
    ///
    /// # Returns
    /// The commit hash as a `CommitHash`.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn get_hash(&self, short: bool) -> Result<CommitHash> { // Changed return type
        let args: &[&str] = if short {
            &["rev-parse", "--short", "HEAD"]
        } else {
            &["rev-parse", "HEAD"]
        };
        execute_git_fn_async(
            &self.location,
            args,
            |output| CommitHash::from_str(output.trim()), // Parse output
        ).await
    }


    /// Lists branches with detailed information asynchronously.
    ///
    /// # Returns
    /// A vector of `Branch` structs with branch details. (Note: Assumes Branch fields updated)
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn list_branches_info(&self) -> Result<Vec<Branch>> { // Assuming Branch uses CommitHash
        execute_git_fn_async(
            &self.location,
            &["branch", "--list", "-v", "--format=%(refname:short) %(objectname) %(HEAD) %(upstream:short)"],
            |output| {
                let mut branches = Vec::new();

                for line in output.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let name_str = parts[0];
                        let commit_str = parts[1]; // &str
                        let is_head = parts[2] == "*";

                        let upstream = if parts.len() >= 4 {
                            Some(parts[3].to_string())
                        } else {
                            None
                        };

                        // --- FIX: Parse commit_str into CommitHash ---
                        if let Ok(name) = BranchName::from_str(name_str) {
                            if let Ok(commit_hash) = CommitHash::from_str(commit_str) { // Parse here
                                branches.push(Branch {
                                    name,
                                    commit: commit_hash, // Assign CommitHash
                                    is_head,
                                    upstream,
                                });
                            } else {
                                eprintln!("Warning: Could not parse commit hash '{}' for async branch '{}'", commit_str, name_str);
                            }
                        } else {
                            eprintln!("Warning: Could not parse branch name '{}' for async", name_str);
                        }
                        // --- End Fix ---
                    }
                }
                Ok(branches)
            }
        ).await
    }

    /// Gets detailed information about a commit asynchronously.
    ///
    /// # Arguments
    /// * `commit_ref` - The commit reference. If `None`, uses HEAD.
    ///
    /// # Returns
    /// A `Commit` struct with commit details. (Note: Assumes Commit fields updated)
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn get_commit(&self, commit_ref: Option<&str>) -> Result<Commit> {
        let format = "%H%n\
                     shortcommit %h%n\
                     author_name %an%n\
                     author_email %ae%n\
                     timestamp %at%n\
                     %P%n\
                     message %s";

        // --- FIX: Use let binding for format string ---
        let format_arg = format!("--format={}", format);

        let args = match commit_ref {
            Some(c) => vec!["show", "--no-patch", &format_arg, c],
            None => vec!["show", "--no-patch", &format_arg],
        };
        // --- End Fix ---

        execute_git_fn_async(&self.location, args, |output| {
            Commit::from_show_format(output).ok_or_else(|| GitError::GitError {
                stdout: output.to_string(),
                stderr: "Failed to parse commit information".to_string(),
            })
        }).await
    }

    /// Gets the current status of the repository asynchronously.
    ///
    /// # Returns
    /// A `StatusResult` struct with status details. (Note: Assumes StatusResult fields updated)
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn status(&self) -> Result<StatusResult> {
        let porcelain_output = execute_git_fn_async(
            &self.location,
            &["status", "--porcelain=v2", "--branch"],
            |output| Ok(output.to_string())
        ).await?;

        let mut branch_name_str = None;
        let mut files = Vec::new();
        let mut merging = false;
        let mut rebasing = false;
        let mut cherry_picking = false;

        for line in porcelain_output.lines() {
            if line.starts_with("# branch.head ") {
                branch_name_str = Some(line.trim_start_matches("# branch.head ").to_string());
            } else if line.starts_with("# branch.oid ") { // Ignore
            } else if line.starts_with("# branch.upstream ") { // Ignore
            } else if line.starts_with("1 ") || line.starts_with("2 ") || line.starts_with("u ") {
                let parts: Vec<&str> = line.split(' ').collect();
                if parts.len() >= 2 {
                    let xy = parts[1];
                    let status_code = if xy.len() >= 2 {
                        (xy.chars().nth(0).unwrap(), xy.chars().nth(1).unwrap())
                    } else {
                        (' ', ' ')
                    };
                    let status = FileStatus::from_porcelain_code(status_code.0, status_code.1);

                    let path_part = line.split('\t').next().unwrap_or(line);
                    let path_components: Vec<&str> = path_part.split(' ').collect();

                    if let Some(path_str) = path_components.iter().rev().find(|s| !s.is_empty()) {
                        let original_path_str = if line.contains('\t') {
                            line.split('\t').nth(1)
                        } else {
                            None
                        };

                        files.push(StatusEntry {
                            path: PathBuf::from(path_str),
                            status,
                            original_path: original_path_str.map(PathBuf::from),
                        });
                    }
                }
            } else if line.starts_with("? ") {
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

        // Parse the branch name string into Option<BranchName>
        let branch = branch_name_str.and_then(|s| BranchName::from_str(&s).ok());

        // Check for special states asynchronously
        let git_dir = self.location.join(".git");

        // Use tokio::fs::try_exists for async checks
        let merge_exists = tokio::fs::try_exists(git_dir.join("MERGE_HEAD")).await.unwrap_or(false);
        if merge_exists { merging = true; }

        let rebase_apply_exists = tokio::fs::try_exists(git_dir.join("rebase-apply")).await.unwrap_or(false);
        let rebase_merge_exists = tokio::fs::try_exists(git_dir.join("rebase-merge")).await.unwrap_or(false);
        if rebase_apply_exists || rebase_merge_exists { rebasing = true; }

        let cherry_pick_exists = tokio::fs::try_exists(git_dir.join("CHERRY_PICK_HEAD")).await.unwrap_or(false);
        if cherry_pick_exists { cherry_picking = true; }

        // Determine if clean (ignoring untracked/ignored)
        let is_clean = files.iter().all(|f|
            matches!(f.status, FileStatus::Unmodified | FileStatus::Ignored)
        );

        // --- FIX: Removed duplicate field and assuming StatusResult expects Option<BranchName> ---
        Ok(StatusResult {
            branch: branch, // Assign the Option<BranchName> directly
            files,
            merging,
            rebasing,
            cherry_picking,
            is_clean,
        })
        // --- End Fix ---
    }

    /// Executes an arbitrary Git command asynchronously within the repository context.
    ///
    /// # Arguments
    /// * `args` - An iterator yielding command-line arguments for Git.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn cmd<I, S>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        execute_git_async(&self.location, args).await
    }

    /// Executes an arbitrary Git command asynchronously and returns its standard output.
    ///
    /// # Arguments
    /// * `args` - An iterator yielding command-line arguments for Git.
    ///
    /// # Returns
    /// A `Vec<String>` where each element is a line from the command's standard output.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
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

// --- Added Async Rebasing Operations ---

impl AsyncRepository {
    /// Rebases the current branch onto another branch or reference asynchronously.
    ///
    /// # Arguments
    /// * `target_branch` - The branch or reference to rebase onto.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn rebase(&self, target_branch: &str) -> Result<()> {
        execute_git_async(&self.location, &["rebase", target_branch]).await
    }

    /// Continues a rebase operation after resolving conflicts asynchronously.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn rebase_continue(&self) -> Result<()> {
        execute_git_async(&self.location, &["rebase", "--continue"]).await
    }

    /// Aborts a rebase operation asynchronously.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn rebase_abort(&self) -> Result<()> {
        execute_git_async(&self.location, &["rebase", "--abort"]).await
    }
}

// --- Added Async Cherry-Pick Operations ---

impl AsyncRepository {
    /// Cherry-picks one or more commits into the current branch asynchronously.
    ///
    /// # Arguments
    /// * `commits` - A vector of commit references.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn cherry_pick<S: AsRef<OsStr>>(&self, commits: Vec<S>) -> Result<()> {
        let mut args: Vec<&OsStr> = Vec::with_capacity(commits.len() + 1);
        args.push("cherry-pick".as_ref());
        let refs: Vec<_> = commits.iter().map(|s| s.as_ref()).collect();
        args.extend(refs);
        execute_git_async(&self.location, args).await
    }

    /// Continues a cherry-pick operation after resolving conflicts asynchronously.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn cherry_pick_continue(&self) -> Result<()> {
        execute_git_async(&self.location, &["cherry-pick", "--continue"]).await
    }

    /// Aborts a cherry-pick operation asynchronously.
    ///
    /// # Errors
    /// Returns `GitError` (including `GitNotFound`).
    pub async fn cherry_pick_abort(&self) -> Result<()> {
        execute_git_async(&self.location, &["cherry-pick", "--abort"]).await
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
    let command_result = Command::new("git")
        .current_dir(p.as_ref())
        .args(args)
        .output()
        .await; // Use .await for tokio::process::Command

    match command_result {
        Ok(output) => {
            if output.status.success() {
                match str::from_utf8(&output.stdout) {
                    Ok(stdout_str) => process(stdout_str),
                    Err(_) => Err(GitError::Undecodable),
                }
            } else {
                let stdout = str::from_utf8(&output.stdout)
                    .map(|s| s.trim_end().to_owned())
                    .unwrap_or_else(|_| String::from("[stdout: undecodable UTF-8]"));
                let stderr = str::from_utf8(&output.stderr)
                    .map(|s| s.trim_end().to_owned())
                    .unwrap_or_else(|_| String::from("[stderr: undecodable UTF-8]"));
                Err(GitError::GitError { stdout, stderr })
            }
        }
        Err(e) => {
            // --- FIX: Added GitNotFound Check ---
            if e.kind() == ErrorKind::NotFound {
                Err(GitError::GitNotFound) // Return the specific error
            } else {
                eprintln!("Failed to execute async git command: {}", e); // Log the OS error
                Err(GitError::Execution) // Return the original generic execution error
            }
            // --- End of Fix ---
        }
    }
}