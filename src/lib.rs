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
//! let repo_url = GitUrl::from_str("[https://github.com/rust-lang/rust.git](https://github.com/rust-lang/rust.git)")?;
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
//! // (Modify files...)
//!
//! // Stage changes
//! repo.add(vec!["src/lib.rs"])?;
//!
//! // Commit staged changes
//! repo.commit_staged("Implement new feature")?;
//!
//! // Add a remote
//! let upstream_url = GitUrl::from_str("[https://example.com/my-upstream.git](https://example.com/my-upstream.git)")?;
//! // Assume Remote type exists and add_remote takes it:
//! // let remote_name = Remote::from_str("upstream")?;
//! // repo.add_remote(&remote_name, &upstream_url)?;
//! // For now, using &str as per current signature:
//! repo.add_remote("upstream", &upstream_url)?;
//!
//! // Push to the new branch on origin
//! repo.push_to_upstream("origin", &new_branch)?;
//!
//! # Ok(())
//! # }
//! ```

use error::GitError;
use std::env;
use std::ffi::OsStr;
use std::io::ErrorKind; // Correctly added
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
// NOTE: You will need to add the new types here once they are defined in types.rs
// e.g. use types::{BranchName, GitUrl, CommitHash, Remote, Tag, Stash, Result};
use types::{BranchName, GitUrl, Remote, Result};

pub mod error;
pub mod types;

/// Represents a local Git repository located at a specific path.
///
/// Provides methods to execute common Git commands within that repository context.
#[derive(Debug, Clone)]
pub struct Repository {
    location: PathBuf,
}

impl Repository {
    /// Creates a `Repository` instance pointing to an existing local Git repository.
    ///
    /// This does *not* check if the path is actually a valid Git repository.
    /// Operations will fail later if it's not.
    ///
    /// # Arguments
    /// * `p` - The path to the local repository's root directory.
    pub fn new<P: AsRef<Path>>(p: P) -> Repository {
        Repository {
            location: PathBuf::from(p.as_ref()),
        }
    }

    /// Clones a remote Git repository into a specified local path.
    ///
    /// Equivalent to `git clone <url> <path>`.
    ///
    /// # Arguments
    /// * `url` - The URL of the remote repository (`GitUrl` ensures basic format validity).
    /// * `p` - The target local path where the repository should be cloned.
    ///
    /// # Errors
    /// Returns `GitError` if:
    /// * The `git` command fails (e.g., network error, invalid URL, path exists and is not empty).
    /// * The `git` executable cannot be found (`GitError::GitNotFound`).
    /// * Other execution errors occur (`GitError::Execution`).
    /// * The working directory is inaccessible.
    /// * Git output cannot be decoded.
    pub fn clone<P: AsRef<Path>>(url: GitUrl, p: P) -> Result<Repository> {
        let p_ref = p.as_ref();
        let cwd = env::current_dir().map_err(|_| GitError::WorkingDirectoryInaccessible)?;

        // Pass URL and Path directly as OsStr compatible args
        let args: Vec<&OsStr> = vec!["clone".as_ref(), url.as_ref(), p_ref.as_os_str()];

        execute_git(cwd, args)?; // Execute in CWD, cloning *into* p

        Ok(Repository {
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
    /// Returns `GitError` if the `git init` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn init<P: AsRef<Path>>(p: P) -> Result<Repository> {
        let p_ref = p.as_ref();
        execute_git(&p_ref, &["init"])?; // Execute 'git init' within the target dir
        Ok(Repository {
            location: PathBuf::from(p_ref),
        })
    }

    /// Creates and checks out a new local branch.
    ///
    /// Equivalent to `git checkout -b <branch_name>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name for the new branch (`BranchName` ensures basic format validity).
    ///
    /// # Errors
    /// Returns `GitError` if the `git checkout` command fails (e.g., branch already exists) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn create_local_branch(&self, branch_name: &BranchName) -> Result<()> {
        execute_git(
            &self.location,
            &["checkout", "-b", branch_name.as_ref()], // Use AsRef<str> -> AsRef<OsStr>
        )
    }

    /// Checks out an existing local branch.
    ///
    /// Equivalent to `git checkout <branch_name>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name of the branch to switch to.
    ///
    /// # Errors
    /// Returns `GitError` if the `git checkout` command fails (e.g., branch doesn't exist, uncommitted changes) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn switch_branch(&self, branch_name: &BranchName) -> Result<()> {
        execute_git(&self.location, &["checkout", branch_name.as_ref()])
    }

    /// Adds file contents to the Git index (staging area).
    ///
    /// Equivalent to `git add <pathspec>...`.
    ///
    /// # Arguments
    /// * `pathspecs` - A vector of file paths or patterns (e.g., `"."`, `"src/main.rs"`) to add.
    ///
    /// # Errors
    /// Returns `GitError` if the `git add` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn add<S: AsRef<OsStr>>(&self, pathspecs: Vec<S>) -> Result<()> {
        let mut args: Vec<&OsStr> = Vec::with_capacity(pathspecs.len() + 1);
        args.push("add".as_ref());
        for spec in pathspecs.iter() {
            args.push(spec.as_ref());
        }
        execute_git(&self.location, args)
    }

    /// Removes files from the working tree and the index.
    ///
    /// Equivalent to `git rm [-f] <pathspec>...`.
    ///
    /// # Arguments
    /// * `pathspecs` - A vector of file paths or patterns to remove.
    /// * `force` - If `true`, corresponds to the `-f` flag (force removal).
    ///
    /// # Errors
    /// Returns `GitError` if the `git rm` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn remove<S: AsRef<OsStr>>(&self, pathspecs: Vec<S>, force: bool) -> Result<()> {
        let mut args: Vec<&OsStr> = Vec::with_capacity(pathspecs.len() + 2);
        args.push("rm".as_ref());
        if force {
            args.push("-f".as_ref());
        }
        for spec in pathspecs.iter() {
            args.push(spec.as_ref());
        }
        execute_git(&self.location, args)
    }

    /// Stages all tracked, modified/deleted files and commits them.
    ///
    /// Equivalent to `git commit -am <message>`.
    /// **Note:** This does *not* stage new (untracked) files. Use `add` first for those.
    /// Use `commit_staged` to commit only what is already staged.
    ///
    /// # Arguments
    /// * `message` - The commit message.
    ///
    /// # Errors
    /// Returns `GitError` if the `git commit` command fails (e.g., nothing to commit, conflicts) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn stage_and_commit_all_modified(&self, message: &str) -> Result<()> {
        execute_git(&self.location, &["commit", "-am", message])
    }

    /// Commits files currently in the staging area (index).
    ///
    /// Equivalent to `git commit -m <message>`.
    /// Does not automatically stage any files. Use `add` beforehand.
    ///
    /// # Arguments
    /// * `message` - The commit message.
    ///
    /// # Errors
    /// Returns `GitError` if the `git commit` command fails (e.g., nothing staged, conflicts) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn commit_staged(&self, message: &str) -> Result<()> {
        execute_git(&self.location, &["commit", "-m", message])
    }

    /// Pushes the current branch to its configured upstream remote branch.
    ///
    /// Equivalent to `git push`.
    /// Requires the current branch to have a configured upstream.
    ///
    /// # Errors
    /// Returns `GitError` if the `git push` command fails (e.g., no upstream, network error, rejected push) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn push(&self) -> Result<()> {
        execute_git(&self.location, &["push"])
    }

    /// Pushes the current branch to a specified remote and sets the upstream configuration.
    ///
    /// Equivalent to `git push -u <upstream_remote> <upstream_branch>`.
    ///
    /// # Arguments
    /// * `upstream_remote` - The name of the remote (e.g., "origin"). // TODO: Change to &Remote type later
    /// * `upstream_branch` - The name of the branch on the remote.
    ///
    /// # Errors
    /// Returns `GitError` if the `git push` command fails (e.g., invalid remote/branch, network error, rejected push) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn push_to_upstream(
        &self,
        upstream_remote: &str,
        upstream_branch: &BranchName,
    ) -> Result<()> {
        execute_git(
            &self.location,
            &[
                "push",
                "-u",
                upstream_remote.as_ref(),
                upstream_branch.as_ref(),
            ],
        )
    }

    /// Adds a new remote repository reference.
    ///
    /// Equivalent to `git remote add <name> <url>`.
    ///
    /// # Arguments
    /// * `name` - The name for the new remote (e.g., "origin", "upstream"). // TODO: Change to &Remote type later
    /// * `url` - The URL of the remote repository.
    ///
    /// # Errors
    /// Returns `GitError` if the `git remote add` command fails (e.g., remote name already exists) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn add_remote(&self, name: &str, url: &GitUrl) -> Result<()> {
        // TODO: Change name to &Remote type later
        execute_git(&self.location, &["remote", "add", name, url.as_ref()])
    }

    /// Fetches updates from a specified remote repository.
    ///
    /// Equivalent to `git fetch <remote>`.
    ///
    /// # Arguments
    /// * `remote` - The name of the remote to fetch from. // TODO: Change to &Remote type later
    ///
    /// # Errors
    /// Returns `GitError` if the `git fetch` command fails (e.g., invalid remote, network error) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn fetch_remote(&self, remote: &str) -> Result<()> {
        // TODO: Change remote to &Remote type later
        execute_git(&self.location, &["fetch", remote])
    }

    /// Creates and checks out a new branch starting from a given point (e.g., another branch, commit hash, tag).
    ///
    /// Equivalent to `git checkout -b <branch_name> <startpoint>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name for the new branch.
    /// * `startpoint` - The reference to branch from (e.g., "main", "origin/main", "v1.0", commit hash). // TODO: Could accept &CommitHash or &Tag later
    ///
    /// # Errors
    /// Returns `GitError` if the `git checkout` command fails (e.g., invalid startpoint, branch already exists) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn create_branch_from_startpoint(
        &self,
        branch_name: &BranchName,
        startpoint: &str, // TODO: Could accept &CommitHash or &Tag later
    ) -> Result<()> {
        execute_git(
            &self.location,
            &[
                "checkout",
                "-b",
                branch_name.as_ref(), // Use AsRef directly
                startpoint,
            ],
        )
    }

    /// Lists the names of all local branches.
    ///
    /// Equivalent to `git branch --format='%(refname:short)'`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the branch names. // TODO: Change return to Result<Vec<BranchName>> later?
    ///
    /// # Errors
    /// Returns `GitError` if the `git branch` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn list_branches(&self) -> Result<Vec<String>> {
        // TODO: Change return to Result<Vec<BranchName>> later?
        execute_git_fn(
            &self.location,
            &["branch", "--list", "--format=%(refname:short)"], // Added --list for clarity
            |output| Ok(output.lines().map(|line| line.to_owned()).collect()),
        )
    }

    /// Lists files currently staged for commit (added).
    ///
    /// Parses the output of `git status --porcelain`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of added files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` if the `git status` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn list_added(&self) -> Result<Vec<String>> {
        git_status(&self, "A") // Status code for Added
    }

    /// Lists tracked files that have been modified but not staged.
    ///
    /// Parses the output of `git status --porcelain`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of modified files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` if the `git status` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn list_modified(&self) -> Result<Vec<String>> {
        git_status(&self, " M") // Status code for Modified (note space)
    }

    /// Lists files that are not tracked by Git.
    ///
    /// Parses the output of `git status --porcelain`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of untracked files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` if the `git status` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn list_untracked(&self) -> Result<Vec<String>> {
        git_status(&self, "??") // Status code for Untracked
    }

    /// Lists all files currently tracked by Git in the working directory.
    ///
    /// Equivalent to `git ls-files`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of tracked files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` if the `git ls-files` command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn list_tracked(&self) -> Result<Vec<String>> {
        execute_git_fn(&self.location, &["ls-files"], |output| {
            Ok(output.lines().map(|line| line.to_owned()).collect())
        })
    }

    /// Gets the URL configured for a specific remote.
    ///
    /// Equivalent to `git config --get remote.<remote_name>.url`.
    ///
    /// # Arguments
    /// * `remote_name` - The name of the remote (e.g., "origin"). // TODO: Change to &Remote type later
    ///
    /// # Returns
    /// The URL as a `String`. // TODO: Change return to Result<GitUrl> later?
    ///
    /// # Errors
    /// Returns `GitError` if the command fails (e.g., remote doesn't exist, no URL configured) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn show_remote_uri(&self, remote_name: &str) -> Result<String> {
        // TODO: Change remote_name to &Remote, return to Result<GitUrl>
        execute_git_fn(
            &self.location,
            &[
                "config",
                "--get",
                &format!("remote.{}.url", remote_name), // format! creates String, which is AsRef<OsStr>
            ],
            |output| Ok(output.trim().to_owned()), // TODO: Parse this into GitUrl
        )
    }

    /// Lists the names of all configured remotes.
    ///
    /// Equivalent to `git remote`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the remote names. // TODO: Change return to Result<Vec<Remote>> later
    ///
    /// # Errors
    /// Returns `GitError::NoRemoteRepositorySet` if no remotes are configured.
    /// Returns other `GitError` variants if the command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn list_remotes(&self) -> Result<Vec<String>> {
        // TODO: Change return to Result<Vec<Remote>> later
        execute_git_fn(&self.location, &["remote"], |output| {
            // Simpler: 'git remote' lists names
            let remotes: Vec<String> = output.lines().map(|line| line.trim().to_owned()).collect();
            if remotes.is_empty() {
                // Check config as 'git remote' might succeed with no output
                let config_check = self.cmd_out(["config", "--get-regexp", r"^remote\..*\.url"]);
                match config_check {
                    Ok(lines) if lines.is_empty() => Err(GitError::NoRemoteRepositorySet),
                    Ok(_) => Ok(remotes), // Remotes exist even if 'git remote' was empty (unlikely)
                    Err(e) => Err(e),     // Propagate config check error
                }
            } else {
                Ok(remotes) // TODO: Parse these into Remote type
            }
        })
    }

    /// Obtains the commit hash (SHA-1) of the current `HEAD`.
    ///
    /// Equivalent to `git rev-parse [--short] HEAD`.
    ///
    /// # Arguments
    /// * `short` - If `true`, returns the abbreviated short hash.
    ///
    /// # Returns
    /// The commit hash as a `String`. // TODO: Change return to Result<CommitHash> later
    ///
    /// # Errors
    /// Returns `GitError` if the `git rev-parse` command fails (e.g., not a Git repository, no commits yet) or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn get_hash(&self, short: bool) -> Result<String> {
        // TODO: Change return to Result<CommitHash> later
        let args: &[&str] = if short {
            &["rev-parse", "--short", "HEAD"]
        } else {
            &["rev-parse", "HEAD"]
        };
        execute_git_fn(&self.location, args, |output| {
            Ok(output.trim().to_owned()) // TODO: Parse this into CommitHash
        })
    }

    /// Executes an arbitrary Git command within the repository context.
    /// Does not capture or process output (useful for commands with side-effects only).
    ///
    /// # Arguments
    /// * `args` - An iterator yielding command-line arguments for Git (e.g., `["log", "--oneline"]`).
    ///
    /// # Errors
    /// Returns `GitError` if the command fails or `git` cannot be executed (`GitNotFound`, `Execution`).
    pub fn cmd<I, S>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        execute_git(&self.location, args)
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
    /// Returns `GitError` if the command fails, `git` cannot be executed (`GitNotFound`, `Execution`), or output is not valid UTF-8.
    pub fn cmd_out<I, S>(&self, args: I) -> Result<Vec<String>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        execute_git_fn(&self.location, args, |output| {
            Ok(output.lines().map(|line| line.to_owned()).collect())
        })
    }
}

// --- Private Helper Functions ---

/// Helper to parse specific lines from `git status --porcelain` output.
fn git_status(repo: &Repository, prefix: &str) -> Result<Vec<String>> {
    execute_git_fn(&repo.location, &["status", "--porcelain"], |output| {
        Ok(output
            .lines()
            .filter_map(|line| {
                if line.starts_with(prefix) {
                    line.split(" -> ")
                        .last()
                        .unwrap_or(&line[prefix.len()..])
                        .trim_start()
                        .to_owned()
                        .into()
                } else {
                    None
                }
            })
            .collect())
    })
}

/// Executes a Git command, discarding successful output.
fn execute_git<I, S, P>(p: P, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    execute_git_fn(p, args, |_| Ok(()))
}

/// Executes a Git command and processes its stdout on success using a closure.
/// Handles errors, including capturing stderr on failure.
fn execute_git_fn<I, S, P, F, R>(p: P, args: I, process: F) -> Result<R>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
    F: FnOnce(&str) -> Result<R>,
{
    let command_result = Command::new("git") // Store the command for clarity
        .current_dir(p.as_ref())
        .args(args)
        .output();

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
            // Check if the error was specifically "command not found"
            if e.kind() == ErrorKind::NotFound {
                Err(GitError::GitNotFound) // Return the new specific error
            } else {
                // For any other OS-level execution error (permissions, etc.)
                eprintln!("Failed to execute git command: {}", e); // Log the OS error
                Err(GitError::Execution) // Return the original generic execution error
            }
        }
    }
}
