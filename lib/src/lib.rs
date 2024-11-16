#![forbid(unsafe_code)]
#![allow(warnings)]

pub mod backend;
pub mod command;
pub mod config;
pub mod error;
pub mod utils;
pub mod version;

use std::path::PathBuf;

// #[derive(thiserror::Error, Debug)]
// pub enum Error {
//     #[error("backend error: {0}")]
//     Backend(
//         #[source]
//         #[from]
//         backend::Error,
//     ),
// }

#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub dirty: bool,
    pub commit_sha: String,
    pub distance_to_latest_tag: usize,
    pub current_version: String,
}

// pub struct GitRepository<R>
// where
//     R: backend::GitBackend,
// {
//     repo: R,
// }
//
// impl<R> GitRepository<R>
// where
//     R: backend::GitBackend,
// {
//     pub fn open<P: Into<PathBuf>>(path: P) -> Result<Self, R::Error> {
//         let repo = R::open(path)?;
//         Ok(Self { repo })
//     }
// }

#[cfg(test)]
pub mod tests {
    macro_rules! assert_eq_vec {
        ($left:expr, $right:expr $(,)?) => {
            $left.sort();
            $right.sort();
            similar_asserts::assert_eq!($left, $right);
        };
        ($left:expr, $right:expr, $($arg:tt)+) => {
            $left.sort();
            $right.sort();
            similar_asserts::assert_eq!($left, $right, $($arg)+);
        };
    }
    pub(crate) use assert_eq_vec;
}
