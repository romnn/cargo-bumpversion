use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

#[cfg(feature = "native")]
pub mod native;

#[cfg(test)]
pub mod temp;

pub trait GitBackend {
    type Error: std::error::Error + Send + Sync + 'static;

    fn open<P: Into<PathBuf>>(path: P) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn repo_dir(&self) -> &Path;

    fn add<P>(&self, files: &[P]) -> Result<(), Self::Error>
    where
        P: AsRef<Path>;

    fn commit(&self, message: &str) -> Result<(), Self::Error>;

    fn tag(&self, name: &str, message: Option<&str>, sign: bool) -> Result<(), Self::Error>;

    fn dirty_files(&self) -> Result<Vec<PathBuf>, Self::Error>;

    fn latest_tag_info(&self, pattern: Option<&str>) -> Result<Option<crate::Tag>, Self::Error>;
}
