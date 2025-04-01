//! Defines core data types like URLs and Branch names for the Git library.
use super::GitError;
use once_cell::sync::Lazy; // Import Lazy
use regex::Regex;
#[cfg(feature = "serde")]
use serde::{de, Deserialize, Deserializer};
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
        && !name.starts_with(INVALID_REFERENCE_START)
        && !name.ends_with(INVALID_REFERENCE_END)
        && name.chars().all(|c| {
            !c.is_ascii_control() && INVALID_REFERENCE_CHARS.iter().all(|invalid| c != *invalid)
        })
        && !name.contains("/.")
        && !name.contains("@{")
        && !name.contains("..")
        && name != "@"
        // Further rule: Cannot contain sequences like //, /*, ?, [*] - simplified check
        && !name.contains("//") && !name.contains("/*") && !name.contains('?') && !name.contains('[') && !name.contains(']')
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
