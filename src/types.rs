//! Defines core data types like URLs and Branch names for the Git library.
use super::GitError;
use once_cell::sync::Lazy; // Import Lazy
use regex::Regex;
#[cfg(feature = "serde")]
use serde::{de, Deserialize, Deserializer};
use std::hash::{Hash};
use std::str::FromStr;
use std::{
    ffi::OsStr, // Import OsStr
    fmt,
    fmt::{Display, Formatter},
    result::Result as stdResult,
};

/// A specialized `Result` type for Git operations.
pub type Result<A> = stdResult<A, GitError>;

// Use Lazy to initialize the Regex safely and only once
static GIT_URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Regex from https://github.com/jonschlinkert/is-git-url - Compile time checked
    Regex::new("(?:git|ssh|https?|git@[-\\w.]+):(//)?(.*?)(\\.git)(/?|\\#[-\\d\\w._]+?)$")
        .expect("Invalid static Git URL regex") // Expect here is okay for static regex
});

/// Represents a validated Git URL.
///
/// Can be created from a string using `FromStr`, which validates the format.
#[derive(Debug, Clone)] // Added Clone
pub struct GitUrl {
    pub(crate) value: String,
}

impl FromStr for GitUrl {
    type Err = GitError;

    /// Parses a string into a `GitUrl`, returning `Err(GitError::InvalidUrl)` if
    /// the string does not match the expected Git URL pattern.
    fn from_str(value: &str) -> Result<Self> {
        if GIT_URL_REGEX.is_match(value) {
            Ok(GitUrl {
                value: String::from(value),
            })
        } else {
            Err(GitError::InvalidUrl(value.to_string()))
        }
    }
}

impl Display for GitUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

// Implement AsRef<str> and AsRef<OsStr> for convenience
impl AsRef<str> for GitUrl {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl AsRef<OsStr> for GitUrl {
    fn as_ref(&self) -> &OsStr {
        self.value.as_ref()
    }
}

/// Represents a validated Git branch name (or more generally, a reference name).
///
/// Can be created from a string using `FromStr`, which validates the format
/// according to Git's reference naming rules.
#[derive(Debug, Clone)] // Added Clone
pub struct BranchName {
    pub(crate) value: String,
}

impl FromStr for BranchName {
    type Err = GitError;

    /// Parses a string into a `BranchName`, returning `Err(GitError::InvalidRefName)` if
    /// the string does not conform to Git's reference naming rules.
    fn from_str(s: &str) -> Result<Self> {
        if is_valid_reference_name(s) {
            Ok(BranchName {
                value: String::from(s),
            })
        } else {
            Err(GitError::InvalidRefName(s.to_string()))
        }
    }
}

impl Display for BranchName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

// Implement AsRef<str> and AsRef<OsStr> for convenience
impl AsRef<str> for BranchName {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl AsRef<OsStr> for BranchName {
    fn as_ref(&self) -> &OsStr {
        self.value.as_ref()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for BranchName {
    /// Deserializes a string into a `BranchName`, validating the format.
    fn deserialize<D>(deserializer: D) -> stdResult<BranchName, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        BranchName::from_str(&s).map_err(de::Error::custom)
    }
}

// --- Internal validation logic ---

const INVALID_REFERENCE_CHARS: [char; 5] = [' ', '~', '^', ':', '\\'];
const INVALID_REFERENCE_START: &str = "-";
const INVALID_REFERENCE_END: &str = ".";

/// Checks if a string is a valid Git reference name based on common rules.
///
/// Rules approximated from `git check-ref-format`.
/// See: https://git-scm.com/docs/git-check-ref-format
fn is_valid_reference_name(name: &str) -> bool {
    !name.is_empty() // Cannot be empty
        && !name.starts_with(INVALID_REFERENCE_START) // Cannot start with "-"
        && !name.starts_with('.') // <--- ADD THIS: Cannot start with "."
        && !name.starts_with('/') // <--- ADD THIS: Cannot start with "/"
        && !name.ends_with(INVALID_REFERENCE_END)   // Cannot end with "."
        && !name.ends_with('/')   // <--- ADD THIS: Cannot end with "/"
        && name.chars().all(|c| {
        !c.is_ascii_control() && INVALID_REFERENCE_CHARS.iter().all(|invalid| c != *invalid)
    })
        && !name.contains("/.")
        && !name.contains("@{")
        && !name.contains("..")
        && name != "@"
        // Rule: Cannot contain consecutive /'s (checked by !name.contains("//"))
        // Rule: Cannot contain sequence /*, ?, [ (checked below)
        && !name.contains("//") && !name.contains("/*") && !name.contains('?') && !name.contains('[') && !name.contains(']')
}
// --- CommitHash Type ---
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitHash {
    value: String,
}

impl FromStr for CommitHash {
    type Err = GitError;

    fn from_str(s: &str) -> Result<Self> {
        let len = s.len();
        // Basic Git SHA-1 hash validation (4 to 40 hex chars)
        if (len >= 4 && len <= 40) && s.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(CommitHash {
                value: s.to_ascii_lowercase(), // Store consistently lowercase
            })
        } else {
            Err(GitError::InvalidCommitHash(s.to_string()))
        }
    }
}

impl Display for CommitHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<str> for CommitHash {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl AsRef<OsStr> for CommitHash {
    fn as_ref(&self) -> &OsStr {
        self.value.as_ref()
    }
}

// --- Remote Type ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Remote {
    value: String,
}

impl FromStr for Remote {
    type Err = GitError;

    fn from_str(s: &str) -> Result<Self> {
        // Basic validation: non-empty, no whitespace, no control characters.
        // Git might allow more, but this covers common safe cases.
        if !s.is_empty()
            && !s
                .chars()
                .any(|c| c.is_ascii_whitespace() || c.is_ascii_control())
        {
            Ok(Remote {
                value: s.to_string(),
            })
        } else {
            Err(GitError::InvalidRemoteName(s.to_string()))
        }
    }
}

impl Display for Remote {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<str> for Remote {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl AsRef<OsStr> for Remote {
    fn as_ref(&self) -> &OsStr {
        self.value.as_ref()
    }
}

// --- Tag Type ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag {
    value: String,
}

impl FromStr for Tag {
    type Err = GitError;

    fn from_str(s: &str) -> Result<Self> {
        // Tags follow standard Git reference naming rules
        if is_valid_reference_name(s) {
            Ok(Tag {
                value: String::from(s),
            })
        } else {
            // Reuse existing error variant for invalid refs
            Err(GitError::InvalidRefName(s.to_string()))
        }
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl AsRef<OsStr> for Tag {
    fn as_ref(&self) -> &OsStr {
        self.value.as_ref()
    }
}

// --- Stash Type ---

static STASH_REF_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^stash@\{(\d+)\}$").expect("Invalid static Stash Ref regex"));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stash {
    value: String,
    // index: usize, // Could parse and store index if needed later
}

impl FromStr for Stash {
    type Err = GitError;

    fn from_str(s: &str) -> Result<Self> {
        if STASH_REF_REGEX.is_match(s) {
            // Optionally parse the index:
            // let caps = STASH_REF_REGEX.captures(s).unwrap(); // Safe after is_match
            // let index: usize = caps[1].parse().unwrap_or(usize::MAX); // Handle parse error?

            Ok(Stash {
                value: s.to_string(),
                // index,
            })
        } else {
            Err(GitError::InvalidStashRef(s.to_string()))
        }
    }
}

impl Display for Stash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<str> for Stash {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl AsRef<OsStr> for Stash {
    fn as_ref(&self) -> &OsStr {
        self.value.as_ref()
    }
}
// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_git_urls() {
        let valid_urls = vec![
            "git://github.com/ember-cli/ember-cli.git#ff786f9f",
            "git://github.com/ember-cli/ember-cli.git#gh-pages",
            "git://github.com/ember-cli/ember-cli.git#master",
            "git://github.com/ember-cli/ember-cli.git#Quick-Fix",
            "git://github.com/ember-cli/ember-cli.git#quick_fix",
            "git://github.com/ember-cli/ember-cli.git#v0.1.0",
            "git://host.xz/path/to/repo.git/",
            "git://host.xz/~user/path/to/repo.git/",
            "git@192.168.101.127:user/project.git",
            "git@github.com:user/project.git",
            "git@github.com:user/some-project.git",
            "git@github.com:user/some_project.git",
            "http://192.168.101.127/user/project.git",
            "http://github.com/user/project.git",
            "http://host.xz/path/to/repo.git/",
            "https://192.168.101.127/user/project.git",
            "https://github.com/user/project.git",
            "https://host.xz/path/to/repo.git/",
            "https://username::;*%$:@github.com/username/repository.git",
            "https://username:$fooABC@:@github.com/username/repository.git",
            "https://username:password@github.com/username/repository.git",
            "ssh://host.xz/path/to/repo.git/",
            "ssh://host.xz/~/path/to/repo.git",
            "ssh://host.xz/~user/path/to/repo.git/",
            "ssh://host.xz:port/path/to/repo.git/",
            "ssh://user@host.xz/path/to/repo.git/",
            "ssh://user@host.xz/~/path/to/repo.git",
            "ssh://user@host.xz/~user/path/to/repo.git/",
            "ssh://user@host.xz:port/path/to/repo.git/",
        ];

        for url in valid_urls.iter() {
            assert!(GitUrl::from_str(url).is_ok(), "Expected valid: {}", url);
        }
    }

    #[test]
    fn test_invalid_git_urls() {
        let invalid_urls = vec![
            "/path/to/repo.git/",
            "file:///path/to/repo.git/",
            "file://~/path/to/repo.git/",
            "git@github.com:user/some_project.git/foo",
            "git@github.com:user/some_project.gitfoo",
            "host.xz:/path/to/repo.git/",
            "host.xz:path/to/repo.git", // Often works with git CLI, but doesn't fit the strict regex
            "host.xz:~user/path/to/repo.git/",
            "path/to/repo.git/",
            "rsync://host.xz/path/to/repo.git/",
            "user@host.xz:/path/to/repo.git/", // Same as host.xz:path...
            "user@host.xz:path/to/repo.git",
            "user@host.xz:~user/path/to/repo.git/",
            "~/path/to/repo.git",
        ];

        for url in invalid_urls.iter() {
            assert!(GitUrl::from_str(url).is_err(), "Expected invalid: {}", url);
        }
    }

    #[test]
    fn test_valid_reference_names() {
        let valid_references = vec![
            "avalidreference",
            "a/valid/ref",
            "a-valid-ref",
            "v1.0.0",
            "HEAD", // Although special, it's structurally valid
            "feature/new_stuff",
            "fix_123",
        ];

        for reference_name in valid_references.iter() {
            assert!(
                is_valid_reference_name(reference_name),
                "Expected valid: {}",
                reference_name
            );
            assert!(
                BranchName::from_str(reference_name).is_ok(),
                "Expected OK: {}",
                reference_name
            );
        }
    }

    #[test]
    fn test_invalid_reference_names() {
        let invalid_references = vec![
            "", // Empty
            "double..dot",
            "inavlid^character",
            "invalid~character",
            "invalid:character",
            "invalid\\character",
            "@",
            "inavlid@{sequence",
            ".start", // Does not start with .
            "end.",
            "/start", // Does not start with /
            "end/",   // Does not end with /
            "with space",
            "with\tcontrol",
            "with//double",
            "path/./dotslash",
            "-startwithdash",
        ];

        for reference_name in invalid_references.iter() {
            assert!(
                !is_valid_reference_name(reference_name),
                "Expected invalid: {}",
                reference_name
            );
            assert!(
                BranchName::from_str(reference_name).is_err(),
                "Expected Err: {}",
                reference_name
            );
        }
    }
}

#[test]
fn test_valid_commit_hash() {
    assert!(CommitHash::from_str("deadbeef").is_ok());
    assert!(CommitHash::from_str("1234567").is_ok());
    assert!(CommitHash::from_str("abcdef0123456789abcdef0123456789abcdef01").is_ok());
    assert_eq!(
        CommitHash::from_str("DEADBEEF").unwrap().value,
        "deadbeef" // Ensure lowercase storage
    );
}

#[test]
fn test_invalid_commit_hash() {
    assert!(CommitHash::from_str("").is_err()); // Too short
    assert!(CommitHash::from_str("abc").is_err()); // Too short
    assert!(CommitHash::from_str("deadbeef_").is_err()); // Invalid char
    assert!(CommitHash::from_str("gfedcba").is_err()); // Invalid hex char 'g'
    assert!(CommitHash::from_str("abcdef0123456789abcdef0123456789abcdef01X").is_err());
    // Too long (if 40 max)
}

#[test]
fn test_valid_remote_name() {
    assert!(Remote::from_str("origin").is_ok());
    assert!(Remote::from_str("upstream").is_ok());
    assert!(Remote::from_str("my-remote_1").is_ok());
}

#[test]
fn test_invalid_remote_name() {
    assert!(Remote::from_str("").is_err()); // Empty
    assert!(Remote::from_str("my remote").is_err()); // Space
    assert!(Remote::from_str("my\tremote").is_err()); // Tab
    assert!(Remote::from_str("my/remote").is_ok()); // Slashes might be ok technically? Test git. Let's allow for now.
    assert!(Remote::from_str("my\nremote").is_err()); // Control char
}

#[test]
fn test_valid_tag_name() {
    // Reuses branch name validation logic implicitly
    assert!(Tag::from_str("v1.0.0").is_ok());
    assert!(Tag::from_str("release/2025-03-31").is_ok());
    assert!(Tag::from_str("my-tag").is_ok());
}

#[test]
fn test_invalid_tag_name() {
    // Reuses branch name validation logic implicitly
    assert!(Tag::from_str("").is_err());
    assert!(Tag::from_str("my tag").is_err()); // Space
    assert!(Tag::from_str("inv@{lid").is_err()); // Invalid sequence
}

#[test]
fn test_valid_stash_ref() {
    assert!(Stash::from_str("stash@{0}").is_ok());
    assert!(Stash::from_str("stash@{123}").is_ok());
}

#[test]
fn test_invalid_stash_ref() {
    assert!(Stash::from_str("").is_err());
    assert!(Stash::from_str("stash@").is_err());
    assert!(Stash::from_str("stash@{abc}").is_err());
    assert!(Stash::from_str("stash{0}").is_err());
    assert!(Stash::from_str("stash@{0").is_err());
    assert!(Stash::from_str("my-stash@{0}").is_err());
}
