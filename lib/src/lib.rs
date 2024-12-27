#![forbid(unsafe_code)]
#![allow(warnings)]

pub mod backend;
pub mod command;
pub mod config;
pub mod diagnostics;
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

#[cfg(test)]
pub mod tests {
    use color_eyre::eyre;

    macro_rules! sim_assert_eq_sorted {
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
    pub(crate) use sim_assert_eq_sorted;

    static INIT: std::sync::Once = std::sync::Once::new();

    /// Initialize test
    ///
    /// This ensures color_eyre is setup once.
    pub fn init() {
        INIT.call_once(|| {
            color_eyre::install().ok();
        });
    }
}
