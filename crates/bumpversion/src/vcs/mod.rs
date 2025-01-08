pub mod git;

#[cfg(test)]
pub mod temp;

use crate::f_string::PythonFormatString;
use std::future::Future;
use std::path::{Path, PathBuf};

/// Info on the latest tag of the VCS.
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

/// Info on the latest revision of the VCS.
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

/// Wrapper that contains both the current tag and the latest revision of the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagAndRevision {
    /// The current tag information.
    pub tag: Option<TagInfo>,
    /// The latest revision.
    pub revision: Option<RevisionInfo>,
}

pub trait VersionControlSystem {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Open the VCS repository.
    fn open(path: impl Into<PathBuf>) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Get the path to the VCS directory.
    fn path(&self) -> &Path;

    /// Add files to the staging area of the VCS.
    fn add<P>(
        &self,
        files: impl IntoIterator<Item = P>,
    ) -> impl Future<Output = Result<(), Self::Error>>
    where
        P: AsRef<std::ffi::OsStr>;

    /// Commit current changes to the VCS.
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

    /// Create a new tag for the VCS.
    fn tag(
        &self,
        name: &str,
        message: Option<&str>,
        sign: bool,
    ) -> impl Future<Output = Result<(), Self::Error>>;

    /// Get all tags for the VCS
    fn tags(&self) -> impl Future<Output = Result<Vec<String>, Self::Error>>;

    /// Get the list of dirty files in the VCS.
    fn dirty_files(&self) -> impl Future<Output = Result<Vec<PathBuf>, Self::Error>>;

    /// Get the information on the latest tag and revision for the VCS.
    fn latest_tag_and_revision(
        &self,
        tag_name: &PythonFormatString,
        parse_version_regex: &regex::Regex,
    ) -> impl Future<Output = Result<TagAndRevision, Self::Error>>;
}
