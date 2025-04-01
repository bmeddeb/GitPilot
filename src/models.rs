//! Provides structured types representing Git data.

use crate::types::BranchName;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a Git commit.
#[derive(Debug, Clone)]
pub struct Commit {
    /// The commit hash.
    pub hash: String,

    /// The abbreviated hash (often used in displays).
    pub short_hash: String,

    /// The commit author's name.
    pub author_name: String,

    /// The commit author's email.
    pub author_email: String,

    /// The commit timestamp (seconds since Unix epoch).
    pub timestamp: u64,

    /// The commit message.
    pub message: String,

    /// Parent commit hashes.
    pub parents: Vec<String>,
}

impl Commit {
    /// Parses a commit from the output of `git show --format=...`.
    pub(crate) fn from_show_format(output: &str) -> Option<Commit> {
        let mut hash = String::new();
        let mut short_hash = String::new();
        let mut author_name = String::new();
        let mut author_email = String::new();
        let mut timestamp = 0;
        let mut message = String::new();
        let mut parents = Vec::new();

        for line in output.lines() {
            if line.starts_with("commit ") {
                hash = line.trim_start_matches("commit ").to_string();
            } else if line.starts_with("shortcommit ") {
                short_hash = line.trim_start_matches("shortcommit ").to_string();
            } else if line.starts_with("author_name ") {
                author_name = line.trim_start_matches("author_name ").to_string();
            } else if line.starts_with("author_email ") {
                author_email = line.trim_start_matches("author_email ").to_string();
            } else if line.starts_with("timestamp ") {
                if let Ok(ts) = line.trim_start_matches("timestamp ").parse::<u64>() {
                    timestamp = ts;
                }
            } else if line.starts_with("parent ") {
                parents.push(line.trim_start_matches("parent ").to_string());
            } else if line.starts_with("message ") {
                message = line.trim_start_matches("message ").to_string();
            }
        }

        if hash.is_empty() {
            return None;
        }

        Some(Commit {
            hash,
            short_hash,
            author_name,
            author_email,
            timestamp,
            message,
            parents,
        })
    }

    /// Returns the commit date as a SystemTime.
    pub fn date(&self) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(self.timestamp)
    }
}

/// Represents a file status from `git status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// The file is unmodified.
    Unmodified,

    /// The file is modified but not staged.
    Modified,

    /// The file is added to the staging area.
    Added,

    /// The file is deleted but the deletion is not staged.
    Deleted,

    /// The file's deletion is staged.
    DeletedStaged,

    /// The file is renamed.
    Renamed,

    /// The file is copied.
    Copied,

    /// The file has both staged and unstaged changes.
    UpdatedButUnmerged,

    /// The file is untracked.
    Untracked,

    /// The file is ignored.
    Ignored,
}

impl FileStatus {
    /// Parses a file status from a git status porcelain format code.
    pub(crate) fn from_porcelain_code(index: char, worktree: char) -> FileStatus {
        match (index, worktree) {
            (' ', 'M') => FileStatus::Modified,
            ('M', ' ') => FileStatus::Added, // Modified in index
            ('M', 'M') => FileStatus::UpdatedButUnmerged,
            ('A', ' ') => FileStatus::Added,
            ('A', 'M') => FileStatus::UpdatedButUnmerged,
            ('D', ' ') => FileStatus::DeletedStaged,
            (' ', 'D') => FileStatus::Deleted,
            ('R', ' ') => FileStatus::Renamed,
            ('C', ' ') => FileStatus::Copied,
            ('?', '?') => FileStatus::Untracked,
            ('!', '!') => FileStatus::Ignored,
            _ => FileStatus::Unmodified,
        }
    }
}

/// Represents a file in the repository with its status.
#[derive(Debug, Clone)]
pub struct StatusEntry {
    /// The path to the file relative to the repository root.
    pub path: PathBuf,

    /// The status of the file.
    pub status: FileStatus,

    /// For renamed files, the original path.
    pub original_path: Option<PathBuf>,
}

/// Represents a Git tag.
#[derive(Debug, Clone)]
pub struct Tag {
    /// The name of the tag.
    pub name: String,

    /// The commit hash the tag points to.
    pub target: String,

    /// Whether the tag is annotated.
    pub annotated: bool,

    /// For annotated tags, the tag message.
    pub message: Option<String>,
}

/// Represents a Git remote.
#[derive(Debug, Clone)]
pub struct Remote {
    /// The name of the remote.
    pub name: String,

    /// The URL of the remote.
    pub url: String,

    /// The fetch refspec.
    pub fetch: Option<String>,
}

/// Represents a Git branch.
#[derive(Debug, Clone)]
pub struct Branch {
    /// The name of the branch.
    pub name: BranchName,

    /// The commit hash the branch points to.
    pub commit: String,

    /// Whether the branch is the current HEAD.
    pub is_head: bool,

    /// The upstream branch, if any.
    pub upstream: Option<String>,
}

/// Represents the result of a `git status` command.
#[derive(Debug, Clone)]
pub struct StatusResult {
    /// The current branch.
    pub branch: Option<String>,

    /// The files in the repository with their status.
    pub files: Vec<StatusEntry>,

    /// Whether the repository is in a merge state.
    pub merging: bool,

    /// Whether the repository is in a rebase state.
    pub rebasing: bool,

    /// Whether the repository is in a cherry-pick state.
    pub cherry_picking: bool,

    /// Whether the working directory is clean.
    pub is_clean: bool,
}

/// Represents a line of blame information.
#[derive(Debug, Clone)]
pub struct BlameLine {
    /// The commit hash.
    pub hash: String,

    /// The author's name.
    pub author: String,

    /// The line number in the original file.
    pub original_line: usize,

    /// The line number in the final file.
    pub final_line: usize,

    /// The timestamp (seconds since Unix epoch).
    pub timestamp: u64,

    /// The line content.
    pub content: String,
}

/// Represents the result of a `git diff` command.
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// The files that were changed.
    pub files: Vec<DiffFile>,
}

/// Represents a file in a diff.
#[derive(Debug, Clone)]
pub struct DiffFile {
    /// The path to the file.
    pub path: PathBuf,

    /// For renamed or copied files, the original path.
    pub old_path: Option<PathBuf>,

    /// The hunks of diff information.
    pub hunks: Vec<DiffHunk>,

    /// The lines added in this file.
    pub added_lines: usize,

    /// The lines removed in this file.
    pub removed_lines: usize,

    /// Whether the file is a binary file.
    pub is_binary: bool,

    /// The file mode before the change.
    pub old_mode: Option<String>,

    /// The file mode after the change.
    pub new_mode: Option<String>,
}

/// Represents a hunk in a diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// The starting line in the original file.
    pub old_start: usize,

    /// The number of lines in the original file.
    pub old_lines: usize,

    /// The starting line in the new file.
    pub new_start: usize,

    /// The number of lines in the new file.
    pub new_lines: usize,

    /// The lines in the hunk.
    pub lines: Vec<DiffLine>,
}

/// Represents a line in a diff hunk.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// The content of the line.
    pub content: String,

    /// The type of line.
    pub line_type: DiffLineType,
}

/// Represents the type of a diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    /// The line is in both the original and new file.
    Context,

    /// The line was added in the new file.
    Added,

    /// The line was removed in the original file.
    Removed,
}

/// Represents a stash entry.
#[derive(Debug, Clone)]
pub struct StashEntry {
    /// The stash reference (e.g., "stash@{0}").
    pub reference: String,

    /// The branch the stash was created from.
    pub branch: Option<String>,

    /// The commit message of the stash.
    pub message: String,
}

/// Represents a worktree.
#[derive(Debug, Clone)]
pub struct Worktree {
    /// The path to the worktree.
    pub path: PathBuf,

    /// The commit hash the worktree is at.
    pub head: String,

    /// The branch the worktree is on, if any.
    pub branch: Option<String>,

    /// Whether this is the main worktree.
    pub is_main: bool,

    /// Whether the worktree is bare.
    pub is_bare: bool,

    /// Whether the worktree is prunable.
    pub is_prunable: bool,
}

/// Represents a config entry.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    /// The key of the config entry.
    pub key: String,

    /// The value of the config entry.
    pub value: String,

    /// The scope of the config entry.
    pub scope: ConfigScope,
}

/// Represents the scope of a config entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    /// The config entry is in the system config.
    System,

    /// The config entry is in the global config.
    Global,

    /// The config entry is in the repository config.
    Local,

    /// The config entry is in a worktree config.
    Worktree,
}

/// Represents a submodule.
#[derive(Debug, Clone)]
pub struct Submodule {
    /// The name of the submodule.
    pub name: String,

    /// The path to the submodule.
    pub path: PathBuf,

    /// The URL of the submodule.
    pub url: String,

    /// The branch the submodule is tracking.
    pub branch: Option<String>,
}

/// Represents the result of a `git log` command.
#[derive(Debug, Clone)]
pub struct LogResult {
    /// The commits in the log.
    pub commits: Vec<Commit>,
}

/// Represents a Git reference (branch, tag, etc.).
#[derive(Debug, Clone)]
pub struct Reference {
    /// The name of the reference.
    pub name: String,

    /// The type of the reference.
    pub ref_type: ReferenceType,

    /// The commit hash the reference points to.
    pub target: String,
}

/// Represents the type of a Git reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceType {
    /// The reference is a local branch.
    LocalBranch,

    /// The reference is a remote branch.
    RemoteBranch,

    /// The reference is a tag.
    Tag,

    /// The reference is a note.
    Note,

    /// The reference is of another type.
    Other,
}