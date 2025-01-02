#![forbid(unsafe_code)]
#![allow(warnings)]

pub mod backend;
pub mod command;
pub mod config;
pub mod context;
pub mod diagnostics;
pub mod error;
pub mod f_string;
pub mod files;
pub mod hooks;
pub mod utils;
pub mod version;

// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub struct Version {
//     // pub dirty: bool,
//     // pub commit_sha: String,
//     // pub distance_to_latest_tag: usize,
//     // pub current_tag: String,
//     // pub current_version: String,
// }

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bump {
    /// Bump major version
    Major,
    /// Bump minor version
    Minor,
    /// Bump patch version
    Patch,
    /// Bump custom version component
    Other(String),
}

impl Bump {
    pub fn name(&self) -> &str {
        match self {
            Self::Other(component) => component.as_str(),
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Patch => "patch",
        }
    }

    pub fn as_str(&self) -> &str {
        self.name()
    }
}

impl std::fmt::Display for Bump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Other(component) => write!(f, "{component}"),
            Self::Major => write!(f, "major"),
            Self::Minor => write!(f, "minor"),
            Self::Patch => write!(f, "patch"),
        }
    }
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
    /// This ensures `color_eyre` is setup once.
    pub fn init() {
        INIT.call_once(|| {
            color_eyre::install().ok();
        });
    }
}
