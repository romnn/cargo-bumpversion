#![allow(warnings)]

pub mod backend;
pub mod config;
pub mod error;
pub mod utils;
pub mod version;

// use backend::*;
pub use config::*;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("backend error: {0}")]
    Backend(#[source] #[from] backend::Error),
}

pub struct GitRepository<R>
where
    R: backend::GitRepository,
{
    repo: R,
}

#[cfg(feature = "native")]
impl GitRepository<backend::native::NativeGitRepository> {
    pub fn native<P: Into<PathBuf>>(path: P) -> Result<Self, Error> {
        Self::open(path)
    }
}

impl<R> GitRepository<R>
where
    R: backend::GitRepository,
{
    pub fn open<P: Into<PathBuf>>(path: P) -> Result<Self, Error> {
        let repo = R::open(path)?;
        Ok(Self { repo })
    }
}
