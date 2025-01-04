pub mod git;

#[cfg(test)]
pub mod temp;

use crate::f_string::PythonFormatString;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagInfo {
    pub dirty: bool,
    pub commit_sha: String,
    pub distance_to_latest_tag: usize,
    pub current_tag: String,
    pub current_version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionInfo {
    /// The name of the current branch.
    pub branch_name: String,
    /// The short branch name.
    ///
    /// Consists of 20 lowercase characters of the branch name with special characters removed.
    pub short_branch_name: String,
    /// The root directory of the Git repository.
    pub repository_root: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagAndRevision {
    pub tag: Option<TagInfo>,
    pub revision: Option<RevisionInfo>,
}

// #[async_trait::async_trait]
pub trait VersionControlSystem {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Open the VCS repository.
    fn open(path: impl Into<PathBuf>) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Get the path to the VCS directory.
    fn path(&self) -> &Path;

    /// Add files to the staging area of the VCS.
    async fn add(&self, files: &[impl AsRef<Path>]) -> Result<(), Self::Error>;

    /// Commit current changes to the VCS.
    async fn commit<A, E, AS, EK, EV>(
        &self,
        message: &str,
        // extra_args: Option<impl IntoIterator<Item = S>>,
        extra_args: A,
        // env: &HashMap<&str, &str>,
        env: E,
    ) -> Result<(), Self::Error>
    where
        A: IntoIterator<Item = AS>,
        E: IntoIterator<Item = (EK, EV)>,
        AS: AsRef<std::ffi::OsStr>,
        EK: AsRef<std::ffi::OsStr>,
        EV: AsRef<std::ffi::OsStr>;

    /// Create a new tag for the VCS.
    async fn tag(&self, name: &str, message: Option<&str>, sign: bool) -> Result<(), Self::Error>;

    /// Get all tags for the VCS
    async fn tags(&self) -> Result<Vec<String>, Self::Error>;

    /// Get the list of dirty files in the VCS.
    async fn dirty_files(&self) -> Result<Vec<PathBuf>, Self::Error>;

    /// Get the information on the latest tag and revision for the VCS.
    async fn latest_tag_and_revision(
        &self,
        tag_name: &PythonFormatString,
        parse_version_regex: &regex::Regex,
        // parse_pattern: &str,
    ) -> Result<TagAndRevision, Self::Error>;
}
