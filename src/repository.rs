//! Provides the core Repository implementation.

use crate::error::GitError;
use crate::types::{BranchName, GitUrl, Result};
use crate::models::*;
use std::env;
use std::ffi::OsStr;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

/// Represents a local Git repository located at a specific path.
///
/// Provides methods to execute common Git commands within that repository context.
#[derive(Debug, Clone)]
pub struct Repository {
    pub(crate) location: PathBuf,
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
    /// * The `git` executable cannot be found or executed.
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
    /// Returns `GitError` if the `git init` command fails or `git` cannot be executed.
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
    /// Returns `GitError` if the `git checkout` command fails (e.g., branch already exists) or `git` cannot be executed.
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
    /// Returns `GitError` if the `git checkout` command fails (e.g., branch doesn't exist, uncommitted changes) or `git` cannot be executed.
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
    /// Returns `GitError` if the `git add` command fails or `git` cannot be executed.
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
    /// Returns `GitError` if the `git rm` command fails or `git` cannot be executed.
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
    /// Returns `GitError` if the `git commit` command fails (e.g., nothing to commit, conflicts) or `git` cannot be executed.
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
    /// Returns `GitError` if the `git commit` command fails (e.g., nothing staged, conflicts) or `git` cannot be executed.
    pub fn commit_staged(&self, message: &str) -> Result<()> {
        execute_git(&self.location, &["commit", "-m", message])
    }

    /// Pushes the current branch to its configured upstream remote branch.
    ///
    /// Equivalent to `git push`.
    /// Requires the current branch to have a configured upstream.
    ///
    /// # Errors
    /// Returns `GitError` if the `git push` command fails (e.g., no upstream, network error, rejected push) or `git` cannot be executed.
    pub fn push(&self) -> Result<()> {
        execute_git(&self.location, &["push"])
    }

    /// Pushes the current branch to a specified remote and sets the upstream configuration.
    ///
    /// Equivalent to `git push -u <upstream_remote> <upstream_branch>`.
    ///
    /// # Arguments
    /// * `upstream_remote` - The name of the remote (e.g., "origin").
    /// * `upstream_branch` - The name of the branch on the remote.
    ///
    /// # Errors
    /// Returns `GitError` if the `git push` command fails (e.g., invalid remote/branch, network error, rejected push) or `git` cannot be executed.
    pub fn push_to_upstream(
        &self,
        upstream_remote: &str,
        upstream_branch: &BranchName,
    ) -> Result<()> {
        execute_git(
            &self.location,
            &["push", "-u", upstream_remote, upstream_branch.as_ref()],
        )
    }

    /// Adds a new remote repository reference.
    ///
    /// Equivalent to `git remote add <name> <url>`.
    ///
    /// # Arguments
    /// * `name` - The name for the new remote (e.g., "origin", "upstream").
    /// * `url` - The URL of the remote repository.
    ///
    /// # Errors
    /// Returns `GitError` if the `git remote add` command fails (e.g., remote name already exists) or `git` cannot be executed.
    pub fn add_remote(&self, name: &str, url: &GitUrl) -> Result<()> {
        execute_git(&self.location, &["remote", "add", name, url.as_ref()])
    }

    /// Fetches updates from a specified remote repository.
    ///
    /// Equivalent to `git fetch <remote>`.
    ///
    /// # Arguments
    /// * `remote` - The name of the remote to fetch from.
    ///
    /// # Errors
    /// Returns `GitError` if the `git fetch` command fails (e.g., invalid remote, network error) or `git` cannot be executed.
    pub fn fetch_remote(&self, remote: &str) -> Result<()> {
        execute_git(&self.location, &["fetch", remote])
    }

    /// Creates and checks out a new branch starting from a given point (e.g., another branch, commit hash, tag).
    ///
    /// Equivalent to `git checkout -b <branch_name> <startpoint>`.
    ///
    /// # Arguments
    /// * `branch_name` - The name for the new branch.
    /// * `startpoint` - The reference to branch from (e.g., "main", "origin/main", "v1.0", commit hash).
    ///
    /// # Errors
    /// Returns `GitError` if the `git checkout` command fails (e.g., invalid startpoint, branch already exists) or `git` cannot be executed.
    pub fn create_branch_from_startpoint(
        &self,
        branch_name: &BranchName,
        startpoint: &str,
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
    /// A `Vec<String>` containing the branch names.
    ///
    /// # Errors
    /// Returns `GitError` if the `git branch` command fails or `git` cannot be executed.
    pub fn list_branches(&self) -> Result<Vec<String>> {
        execute_git_fn(
            &self.location,
            &["branch", "--list", "--format=%(refname:short)"], // Added --list for clarity
            |output| Ok(output.lines().map(|line| line.to_owned()).collect()),
        )
    }

    /// Lists files currently staged for commit (added).
    ///
    /// Parses the output of `git status -s`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of added files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` if the `git status` command fails or `git` cannot be executed.
    pub fn list_added(&self) -> Result<Vec<String>> {
        git_status(&self, "A") // Status code for Added
    }

    /// Lists tracked files that have been modified but not staged.
    ///
    /// Parses the output of `git status -s`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of modified files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` if the `git status` command fails or `git` cannot be executed.
    pub fn list_modified(&self) -> Result<Vec<String>> {
        git_status(&self, " M") // Status code for Modified (note space)
    }

    /// Lists files that are not tracked by Git.
    ///
    /// Parses the output of `git status -s`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the paths of untracked files relative to the repository root.
    ///
    /// # Errors
    /// Returns `GitError` if the `git status` command fails or `git` cannot be executed.
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
    /// Returns `GitError` if the `git ls-files` command fails or `git` cannot be executed.
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
    /// * `remote_name` - The name of the remote (e.g., "origin").
    ///
    /// # Returns
    /// The URL as a `String`.
    ///
    /// # Errors
    /// Returns `GitError` if the command fails (e.g., remote doesn't exist, no URL configured) or `git` cannot be executed.
    pub fn show_remote_uri(&self, remote_name: &str) -> Result<String> {
        execute_git_fn(
            &self.location,
            &[
                "config",
                "--get",
                &format!("remote.{}.url", remote_name), // format! creates String, which is AsRef<OsStr>
            ],
            |output| Ok(output.trim().to_owned()),
        )
    }

    /// Lists the names of all configured remotes.
    ///
    /// Equivalent to `git remote show`.
    ///
    /// # Returns
    /// A `Vec<String>` containing the remote names.
    ///
    /// # Errors
    /// Returns `GitError::NoRemoteRepositorySet` if no remotes are configured.
    /// Returns other `GitError` variants if the command fails or `git` cannot be executed.
    pub fn list_remotes(&self) -> Result<Vec<String>> {
        execute_git_fn(&self.location, &["remote"], |output| {
            // Simpler: 'git remote' lists names
            let remotes: Vec<String> = output.lines().map(|line| line.trim().to_owned()).collect();
            if remotes.is_empty() {
                // Check config instead, as 'git remote' might succeed with no output
                // A better check might be trying to list remote URLs or use a plumbing command
                // For now, assume empty output means no remotes, but add a check.
                let config_check = self.cmd_out(["config", "--get-regexp", r"^remote\..*\.url"]);
                match config_check {
                    Ok(lines) if lines.is_empty() => Err(GitError::NoRemoteRepositorySet),
                    Ok(_) => Ok(remotes), // Remotes exist even if 'git remote' was empty (unlikely)
                    Err(e) => Err(e),     // Propagate config check error
                }
            } else {
                Ok(remotes)
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
    /// The commit hash as a `String`.
    ///
    /// # Errors
    /// Returns `GitError` if the `git rev-parse` command fails (e.g., not a Git repository, no commits yet) or `git` cannot be executed.
    pub fn get_hash(&self, short: bool) -> Result<String> {
        let args: &[&str] = if short {
            &["rev-parse", "--short", "HEAD"]
        } else {
            &["rev-parse", "HEAD"]
        };
        execute_git_fn(&self.location, args, |output| Ok(output.trim().to_owned()))
    }

    /// Executes an arbitrary Git command within the repository context.
    /// Does not capture or process output (useful for commands with side-effects only).
    ///
    /// # Arguments
    /// * `args` - An iterator yielding command-line arguments for Git (e.g., `["log", "--oneline"]`).
    ///
    /// # Errors
    /// Returns `GitError` if the command fails or `git` cannot be executed.
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
    /// Returns `GitError` if the command fails, `git` cannot be executed, or output is not valid UTF-8.
    pub fn cmd_out<I, S>(&self, args: I) -> Result<Vec<String>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        execute_git_fn(&self.location, args, |output| {
            Ok(output.lines().map(|line| line.to_owned()).collect())
        })
    }

    // --- Operations for Structured Types ---

    /// Gets detailed information about a commit.
    ///
    /// # Arguments
    /// * `commit_ref` - The commit reference (hash, branch name, etc.). If `None`, uses HEAD.
    ///
    /// # Returns
    /// A `Commit` struct with commit details.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub fn get_commit(&self, commit_ref: Option<&str>) -> Result<Commit> {
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

        execute_git_fn(&self.location, args, |output| {
            Commit::from_show_format(output).ok_or_else(|| GitError::GitError {
                stdout: output.to_string(),
                stderr: "Failed to parse commit information".to_string(),
            })
        })
    }

    /// Gets the current status of the repository.
    ///
    /// # Returns
    /// A `StatusResult` struct with status details.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub fn status(&self) -> Result<StatusResult> {
        // Get the porcelain status
        let porcelain_output = execute_git_fn(
            &self.location,
            &["status", "--porcelain=v2", "--branch"],
            |output| Ok(output.to_string())
        )?;

        let mut branch = None;
        let mut files = Vec::new();
        let mut merging = false;
        let mut rebasing = false;
        let mut cherry_picking = false;

        for line in porcelain_output.lines() {
            if line.starts_with("# branch.head ") {
                branch = Some(line.trim_start_matches("# branch.head ").to_string());
            } else if line.starts_with("# branch.oid ") {
                // Branch object id, we could store this if needed
            } else if line.starts_with("# branch.upstream ") {
                // Upstream branch, we could store this if needed
            } else if line.starts_with("1 ") || line.starts_with("2 ") || line.starts_with("u ") {
                // Parse file status
                let parts: Vec<&str> = line.split(' ').collect();
                if parts.len() >= 2 {
                    let status_code = if parts[0] == "1" && parts.len() >= 3 {
                        // Ordinary changed entries format: 1 XY path
                        let xy = parts[1];
                        if xy.len() >= 2 {
                            (xy.chars().nth(0).unwrap(), xy.chars().nth(1).unwrap())
                        } else {
                            (' ', ' ')
                        }
                    } else if parts[0] == "2" && parts.len() >= 9 {
                        // Renamed/copied entries format: 2 XY path1 path2
                        let xy = parts[1];
                        if xy.len() >= 2 {
                            (xy.chars().nth(0).unwrap(), xy.chars().nth(1).unwrap())
                        } else {
                            (' ', ' ')
                        }
                    } else if parts[0] == "u" && parts.len() >= 5 {
                        // Unmerged entries format: u XY subtype path
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

        // Check for special states
        let git_dir = self.location.join(".git");

        if std::path::Path::new(&git_dir.join("MERGE_HEAD")).exists() {
            merging = true;
        }

        if std::path::Path::new(&git_dir.join("rebase-apply")).exists()
            || std::path::Path::new(&git_dir.join("rebase-merge")).exists() {
            rebasing = true;
        }

        if std::path::Path::new(&git_dir.join("CHERRY_PICK_HEAD")).exists() {
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

    /// Lists branches with detailed information.
    ///
    /// # Returns
    /// A vector of `Branch` structs with branch details.
    ///
    /// # Errors
    /// Returns `GitError` if the operation fails or `git` cannot be executed.
    pub fn list_branches_info(&self) -> Result<Vec<Branch>> {
        execute_git_fn(
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
        )
    }
}

// --- Rebasing Operations ---

impl Repository {
    /// Rebases the current branch onto another branch or reference.
    ///
    /// Equivalent to `git rebase <target_branch>`.
    ///
    /// # Arguments
    /// * `target_branch` - The branch or reference to rebase onto.
    ///
    /// # Errors
    /// Returns `GitError` if the rebase operation fails (e.g., conflicts) or `git` cannot be executed.
    pub fn rebase(&self, target_branch: &str) -> Result<()> {
        execute_git(&self.location, &["rebase", target_branch])
    }

    /// Continues a rebase operation after resolving conflicts.
    ///
    /// Equivalent to `git rebase --continue`.
    ///
    /// # Errors
    /// Returns `GitError` if the continue operation fails or `git` cannot be executed.
    pub fn rebase_continue(&self) -> Result<()> {
        execute_git(&self.location, &["rebase", "--continue"])
    }

    /// Aborts a rebase operation and returns to the pre-rebase state.
    ///
    /// Equivalent to `git rebase --abort`.
    ///
    /// # Errors
    /// Returns `GitError` if the abort operation fails or `git` cannot be executed.
    pub fn rebase_abort(&self) -> Result<()> {
        execute_git(&self.location, &["rebase", "--abort"])
    }
}

// --- Cherry-Pick Operations ---

impl Repository {
    /// Cherry-picks one or more commits into the current branch.
    ///
    /// Equivalent to `git cherry-pick <commit>...`.
    ///
    /// # Arguments
    /// * `commits` - A vector of commit references (hashes, branch names, etc.) to cherry-pick.
    ///
    /// # Errors
    /// Returns `GitError` if the cherry-pick operation fails (e.g., conflicts) or `git` cannot be executed.
    pub fn cherry_pick<S: AsRef<OsStr>>(&self, commits: Vec<S>) -> Result<()> {
        let mut args: Vec<&OsStr> = Vec::with_capacity(commits.len() + 1);
        args.push("cherry-pick".as_ref());
        for commit in commits.iter() {
            args.push(commit.as_ref());
        }
        execute_git(&self.location, args)
    }

    /// Continues a cherry-pick operation after resolving conflicts.
    ///
    /// Equivalent to `git cherry-pick --continue`.
    ///
    /// # Errors
    /// Returns `GitError` if the continue operation fails or `git` cannot be executed.
    pub fn cherry_pick_continue(&self) -> Result<()> {
        execute_git(&self.location, &["cherry-pick", "--continue"])
    }

    /// Aborts a cherry-pick operation.
    ///
    /// Equivalent to `git cherry-pick --abort`.
    ///
    /// # Errors
    /// Returns `GitError` if the abort operation fails or `git` cannot be executed.
    pub fn cherry_pick_abort(&self) -> Result<()> {
        execute_git(&self.location, &["cherry-pick", "--abort"])
    }
}

// --- Helper Functions ---

/// Helper to parse specific lines from `git status -s` output.
fn git_status(repo: &Repository, prefix: &str) -> Result<Vec<String>> {
    execute_git_fn(&repo.location, &["status", "--porcelain"], |output| {
        // --porcelain is more stable than -s
        Ok(output
            .lines()
            // Status codes can be XY PATH or XY ORIG_PATH -> PATH (renames)
            // We only care about the final path for simple cases.
            .filter_map(|line| {
                if line.starts_with(prefix) {
                    // Handle potential rename "XY ORIG -> NEW" by taking the part after " -> " if present
                    line.split(" -> ")
                        .last()
                        // Otherwise take the part after the status code (XY<space>)
                        .unwrap_or(&line[prefix.len()..])
                        .trim_start() // Trim leading space if no rename
                        .to_owned()
                        .into() // Convert to Option<String>
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
    F: FnOnce(&str) -> Result<R>, // Changed to FnOnce as it's called at most once
{
    let process_output = Command::new("git")
        .current_dir(p.as_ref())
        .args(args)
        .output();

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
            // We could potentially match e.kind() for more specific errors if needed
            eprintln!("Failed to execute git command: {}", e); // Log the OS error
            Err(GitError::Execution)
        }
    }
}