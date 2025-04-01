//! Provides structured types representing Git data.

// Updated imports to include specific types
use crate::types::{BranchName, CommitHash, GitUrl, Remote, Stash, Tag}; // Added specific types
use crate::error::GitError; // Keep error import
use std::path::PathBuf;
use std::str::FromStr; // Needed for parsing within models
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a Git commit.
#[derive(Debug, Clone)]
pub struct Commit {
    /// The commit hash. (Now CommitHash)
    pub hash: CommitHash,
    /// The abbreviated hash. (Now CommitHash)
    pub short_hash: CommitHash,
    /// The commit author's name.
    pub author_name: String,
    /// The commit author's email.
    pub author_email: String,
    /// The commit timestamp (seconds since Unix epoch).
    pub timestamp: u64,
    /// The commit message.
    pub message: String,
    /// Parent commit hashes. (Now Vec<CommitHash>)
    pub parents: Vec<CommitHash>,
}

impl Commit {
    /// Parses a commit from the output of `git show --format=...`.
    pub(crate) fn from_show_format(output: &str) -> Option<Commit> {
        let mut hash_str = None;
        let mut short_hash_str = None;
        let mut author_name = String::new();
        let mut author_email = String::new();
        let mut timestamp = 0;
        let mut message = String::new();
        let mut parent_hashes_str = String::new();

        for line in output.lines() {
            if hash_str.is_none() && !line.is_empty() {
                hash_str = Some(line.to_string());
            } else if line.starts_with("shortcommit ") {
                short_hash_str = Some(line.trim_start_matches("shortcommit ").to_string());
            } else if line.starts_with("author_name ") {
                author_name = line.trim_start_matches("author_name ").to_string();
            } else if line.starts_with("author_email ") {
                author_email = line.trim_start_matches("author_email ").to_string();
            } else if line.starts_with("timestamp ") {
                timestamp = line.trim_start_matches("timestamp ").parse::<u64>().ok()?;
            } else if !line.starts_with("message ") && parent_hashes_str.is_empty() && hash_str.is_some() && short_hash_str.is_some() {
                parent_hashes_str = line.to_string();
            } else if line.starts_with("message ") {
                message = line.trim_start_matches("message ").to_string();
            }
        }

        // --- FIX START ---
        // Add '&' to pass a reference (&str) to from_str
        let hash = CommitHash::from_str(&hash_str?).ok()?;
        let short_hash = CommitHash::from_str(&short_hash_str?).ok()?;
        // --- FIX END ---

        let parents = parent_hashes_str
            .split_whitespace()
            .map(CommitHash::from_str) // from_str expects &str, split_whitespace yields &str - OK
            .collect::<std::result::Result<Vec<_>, _>>()
            .ok()?;

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

    // date() method remains the same
    pub fn date(&self) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(self.timestamp)
    }
}
/// Represents a file status from `git status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Unmodified,
    Modified,
    Added,
    Deleted,
    DeletedStaged, // Represents 'D ' in porcelain v1
    Renamed,
    Copied,
    UpdatedButUnmerged,
    Untracked,
    Ignored,
}

impl FileStatus {
    /// Parses a file status from a git status porcelain v1/v2 XY code.
    pub(crate) fn from_porcelain_code(index: char, worktree: char) -> FileStatus {
        // Based on git-status(1) man page documentation for --porcelain=v1
        match (index, worktree) {
            (' ', 'M') => FileStatus::Modified,         // WT modified
            ('M', _)   => FileStatus::Added,            // Index modified (staged)
            ('A', _)   => FileStatus::Added,            // Index added (staged)
            ('D', _)   => FileStatus::DeletedStaged,    // Index deleted (staged)
            ('R', _)   => FileStatus::Renamed,          // Index renamed (staged)
            ('C', _)   => FileStatus::Copied,           // Index copied (staged)
            ('U', _)   => FileStatus::UpdatedButUnmerged, // Unmerged
            (_,   'D') => FileStatus::Deleted,          // WT deleted
            // Note: (' ', ' ') should be unmodified, handled below.
            // ('T', _) => FileStatus::TypeChanged, // Type Change (Staged) - Add if needed
            // (_, 'T') => FileStatus::TypeChanged, // Type Change (WT) - Add if needed
            ('?', '?') => FileStatus::Untracked,
            ('!', '!') => FileStatus::Ignored,
            _          => FileStatus::Unmodified, // Includes (' ', ' ')
        }
    }
}

/// Represents a file in the repository with its status.
#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub path: PathBuf,
    pub status: FileStatus,
    pub original_path: Option<PathBuf>,
}

/// Represents a Git tag (distinct from the Tag type). Renamed to avoid conflict.
#[derive(Debug, Clone)]
pub struct TagInfo { // Renamed from Tag to avoid conflict with types::Tag
    /// The name of the tag. (Now types::Tag)
    pub name: Tag,
    /// The commit hash the tag points to. (Now CommitHash)
    pub target: CommitHash,
    /// Whether the tag is annotated.
    pub annotated: bool,
    /// For annotated tags, the tag message.
    pub message: Option<String>,
}

/// Represents a Git remote (distinct from the Remote type). Renamed to avoid conflict.
#[derive(Debug, Clone)]
pub struct RemoteInfo { // Renamed from Remote to avoid conflict with types::Remote
    /// The name of the remote. (Now types::Remote)
    pub name: Remote,
    /// The URL of the remote. (Now GitUrl)
    pub url: GitUrl,
    /// The fetch refspec.
    pub fetch: Option<String>,
}

/// Represents a Git branch.
#[derive(Debug, Clone)]
pub struct Branch {
    /// The name of the branch. (Already BranchName)
    pub name: BranchName,
    /// The commit hash the branch points to. (Now CommitHash)
    pub commit: CommitHash,
    /// Whether the branch is the current HEAD.
    pub is_head: bool,
    /// The upstream branch ref string (e.g., "origin/main"). Kept as String for now.
    pub upstream: Option<String>,
}

/// Represents the result of a `git status` command.
#[derive(Debug, Clone)]
pub struct StatusResult {
    /// The current branch name, if on a branch. (Now Option<BranchName>)
    pub branch: Option<BranchName>,
    /// The files in the repository with their status.
    pub files: Vec<StatusEntry>,
    /// Whether the repository is in a merge state.
    pub merging: bool,
    /// Whether the repository is in a rebase state.
    pub rebasing: bool,
    /// Whether the repository is in a cherry-pick state.
    pub cherry_picking: bool,
    /// Whether the working directory is clean (no changes, excluding untracked/ignored).
    pub is_clean: bool,
}

/// Represents a line of blame information.
#[derive(Debug, Clone)]
pub struct BlameLine {
    /// The commit hash. (Now CommitHash)
    pub hash: CommitHash,
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
    pub files: Vec<DiffFile>,
}

/// Represents a file in a diff.
#[derive(Debug, Clone)]
pub struct DiffFile {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub hunks: Vec<DiffHunk>,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub is_binary: bool,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
}

/// Represents a hunk in a diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<DiffLine>,
}

/// Represents a line in a diff hunk.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub line_type: DiffLineType,
}

/// Represents the type of a diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    Context,
    Added,
    Removed,
}

/// Represents a stash entry.
#[derive(Debug, Clone)]
pub struct StashEntry {
    /// The stash reference. (Now types::Stash)
    pub reference: Stash,
    /// The branch the stash was created from. Kept as String for now.
    pub branch: Option<String>,
    /// The commit message of the stash.
    pub message: String,
}

/// Represents a worktree.
#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    /// The commit hash the worktree is at. (Now CommitHash)
    pub head: CommitHash,
    /// The branch the worktree is on, if any. Kept as String for now.
    pub branch: Option<String>,
    pub is_main: bool,
    pub is_bare: bool,
    pub is_prunable: bool,
}

/// Represents a config entry.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub scope: ConfigScope,
}

/// Represents the scope of a config entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    System,
    Global,
    Local,
    Worktree,
}

/// Represents a submodule.
#[derive(Debug, Clone)]
pub struct Submodule {
    pub name: String,
    pub path: PathBuf,
    /// The URL of the submodule. (Now GitUrl)
    pub url: GitUrl,
    /// The branch the submodule is tracking. Kept as String for now.
    pub branch: Option<String>,
}

/// Represents the result of a `git log` command.
#[derive(Debug, Clone)]
pub struct LogResult {
    /// The commits in the log. (Now uses updated Commit model)
    pub commits: Vec<Commit>,
}

/// Represents a Git reference (branch, tag, etc.).
#[derive(Debug, Clone)]
pub struct Reference {
    /// The name of the reference. (Kept as String for generic refs)
    pub name: String,
    /// The type of the reference.
    pub ref_type: ReferenceType,
    /// The commit hash the reference points to. (Now CommitHash)
    pub target: CommitHash,
}

/// Represents the type of a Git reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceType {
    LocalBranch,
    RemoteBranch,
    Tag,
    Note,
    Other,
}