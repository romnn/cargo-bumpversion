#![allow(warnings)]

pub mod backend;
pub mod error;
pub mod utils;

pub use backend::*;
// use std::path::{Path, PathBuf};

// #[cfg(not(any(feature = "foo", feature = "bar")))]
// compile_error!("Either feature \"foo\" or \"bar\" must be enabled for this crate.");

// pub struct GitRepository {
//     inner: git::Repository,
// }

// impl GitRepository {
//     pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
//         let inner = git::Repository::discover(path)?;
//         Ok(Self { inner })
//     }

//     pub fn path(&self) -> &Path {
//         self.inner.git_dir()
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use anyhow::Result;
//     use tempdir::TempDir;

//     struct TempGitRepository {
//         inner: GitRepository,
//         dir: TempDir,
//     }

//     impl TempGitRepository {
//         pub fn new() -> Result<Self> {
//             Self::with_name(random_string_of_length(10))
//         }

//         pub fn with_name(name: &str) -> Result<Self> {
//             let dir = TempDir::new(name)?;
//             let inner = GitRepository::open(dir.path());
//             // let repo = git::init_bare(git_dir)?;
//             // println!("Repo (bare): {:?}", repo.git_dir());
//             // let mut tree = git::objs::Tree::empty();
//             // let empty_tree_id = repo.write_object(&tree)?;

//             Self { inner, dir }
//         }
//     }

//     // fn create_empty_git() -> Result<()> {

//     #[test]
//     fn test_create_empty_git() -> Result<()> {
//         // let git_dir = std::env::args_os()
//         //     .nth(1)
//         //     .context("First argument needs to be the directory to initialize the repository in")?;
//         let repo = TempGitRepository::new()?;

//         // let author = git::actor::SignatureRef {
//         //     name: "Maria Sanchez".into(),
//         //     email: "maria@example.com".into(),
//         //     time: git_date::Time::now_local_or_utc(),
//         // };
//         // let initial_commit_id = repo.commit(
//         //     "HEAD",
//         //     author,
//         //     author,
//         //     "initial commit",
//         //     empty_tree_id,
//         //     git::commit::NO_PARENT_IDS,
//         // )?;
//     }
// }
