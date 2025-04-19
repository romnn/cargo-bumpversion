//! Version control integration layer.
//!
//! Defines the `VersionControlSystem` trait and related data structures
//! for interacting with git and other VCS backends.
pub mod git;

#[cfg(test)]
pub mod temp;

use crate::f_string::PythonFormatString;
use std::future::Future;
use std::path::{Path, PathBuf};

/// Information about the latest VCS tag, including version and commit data.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagInfo {
    /// Whether the repository contains dirty files
    pub dirty: bool,
    /// The current commit SHA hash.
    pub commit_sha: String,
    /// The distance to the latest tag.
    pub distance_to_latest_tag: usize,
    /// The current tag.
    pub current_tag: String,
    /// The current version.
    pub current_version: String,
}

/// Information about the current VCS revision (branch, repository root).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionInfo {
    /// The name of the current branch.
    pub branch_name: String,
    /// The short branch name.
    ///
    /// Consists of 20 lowercase characters of the branch name with special characters removed.
    pub short_branch_name: String,
    /// The root directory of the repository.
    pub repository_root: PathBuf,
}

/// Combined container for both optional `TagInfo` and `RevisionInfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagAndRevision {
    /// The current tag information.
    pub tag: Option<TagInfo>,
    /// The latest revision.
    pub revision: Option<RevisionInfo>,
}

/// Abstract interface for version control systems.
///
/// Implementors can open repositories, add/commit files, tag releases, and query history.
pub trait VersionControlSystem {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Open the VCS repository at the given path.
    fn open(path: impl Into<PathBuf>) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Return the root path of the repository.
    fn path(&self) -> &Path;

    /// Stage a set of files for commit.
    fn add<P>(
        &self,
        files: impl IntoIterator<Item = P>,
    ) -> impl Future<Output = Result<(), Self::Error>>
    where
        P: AsRef<std::ffi::OsStr>;

    /// Create a commit with the given message and environment.
    fn commit<A, E, AS, EK, EV>(
        &self,
        message: &str,
        extra_args: A,
        env: E,
    ) -> impl Future<Output = Result<(), Self::Error>>
    where
        A: IntoIterator<Item = AS>,
        E: IntoIterator<Item = (EK, EV)>,
        AS: AsRef<std::ffi::OsStr>,
        EK: AsRef<std::ffi::OsStr>,
        EV: AsRef<std::ffi::OsStr>;

    /// Create a new tag (annotated or lightweight) in the repository.
    fn tag(
        &self,
        name: &str,
        message: Option<&str>,
        sign: bool,
    ) -> impl Future<Output = Result<(), Self::Error>>;

    /// List all tags in the repository.
    fn tags(&self) -> impl Future<Output = Result<Vec<String>, Self::Error>>;

    /// List files with uncommitted changes.
    fn dirty_files(&self) -> impl Future<Output = Result<Vec<PathBuf>, Self::Error>>;

    /// Retrieve combined tag and revision metadata using the given templates.
    fn latest_tag_and_revision(
        &self,
        tag_name: &PythonFormatString,
        parse_version_regex: &regex::Regex,
    ) -> impl Future<Output = Result<TagAndRevision, Self::Error>>;
}
