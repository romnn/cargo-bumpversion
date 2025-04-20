//! Configuration parsing and merging.
//!
//! Provides support for reading bumpversion configuration from various file formats (TOML, INI),
//! applying defaults, and finalizing settings for version bump operations.
pub mod change;
pub mod defaults;
pub mod file;
pub mod global;
pub mod ini;
pub mod pyproject_toml;
pub mod regex;
pub mod toml;
pub mod version;

pub use change::FileChange;
pub use file::{FileConfig, FinalizedFileConfig};
pub use global::{GlobalConfig, GlobalConfigFinalized};
pub use regex::{Regex, RegexTemplate};
pub use version::{VersionComponentConfigs, VersionComponentSpec};

use crate::files::IoError;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
/// Errors that can occur while reading or parsing configuration files.
pub enum Error {
    /// I/O error accessing config file.
    #[error(transparent)]
    IoError(#[from] IoError),
    /// TOML parsing error for a config file.
    #[error("failed to parse {path:?}")]
    Toml {
        /// Path to the problematic config file.
        path: PathBuf,
        #[source]
        source: pyproject_toml::ParseError,
    },
    /// INI parsing error for a config file.
    #[error("failed to parse {path:?}")]
    Ini {
        /// Path to the problematic config file.
        path: PathBuf,
        #[source]
        source: ini::ParseError,
    },
    /// Cargo.toml parsing not yet supported or failed.
    #[error("failed to parse {path:?}")]
    CargoToml {
        /// Path to the Cargo.toml file.
        path: PathBuf,
        // #[source]
        // source: ini::ParseError,
    },
    /// Background task join error.
    #[error("failed to join spawned task")]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Diagnostics(#[from] crate::diagnostics::Error),
}

/// Enumeration of recognized configuration file types for bumpversion.
/// Supported configuration file types and their paths.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigFile {
    // A `bumpversion.toml` configuration file (TOML)
    /// `.bumpversion.toml` file.
    BumpversionToml(PathBuf),
    // A `pyproject.toml` configuration file (TOML)
    /// `pyproject.toml` file.
    PyProject(PathBuf),
    // A `bumpverison.cfg` configuration file (ini)
    /// `bumpversion.cfg` INI config.
    BumpversionCfg(PathBuf),
    // A `setup.cfg` configuration file (ini)
    /// `setup.cfg` INI config.
    SetupCfg(PathBuf),
    // A `Cargo.toml` configuration file (TOML)
    /// `Cargo.toml` file for workspace/package metadata.
    CargoToml(PathBuf),
}

impl ConfigFile {
    #[must_use]
    pub fn path(&self) -> &Path {
        #[allow(clippy::match_same_arms)]
        match self {
            Self::BumpversionToml(path) => path.as_ref(),
            Self::PyProject(path) => path.as_ref(),
            Self::BumpversionCfg(path) => path.as_ref(),
            Self::SetupCfg(path) => path.as_ref(),
            Self::CargoToml(path) => path.as_ref(),
        }
    }
}

/// Return the list of config files to search in `dir` in order.
///
/// Yields each candidate `ConfigFile` type with its expected path.
pub fn config_file_locations(dir: &Path) -> impl Iterator<Item = ConfigFile> + use<'_> {
    [
        ConfigFile::BumpversionToml(dir.join(".bumpversion.toml")),
        ConfigFile::BumpversionCfg(dir.join(".bumpversion.cfg")),
        ConfigFile::PyProject(dir.join("pyproject.toml")),
        ConfigFile::SetupCfg(dir.join("setup.cfg")),
        ConfigFile::CargoToml(dir.join("Cargo.toml")),
    ]
    .into_iter()
}

pub trait MergeWith<T> {
    fn merge_with(&mut self, other: T);
}

impl<'a, T> MergeWith<Option<&'a T>> for Option<T>
where
    T: Clone,
{
    fn merge_with(&mut self, other: Option<&'a T>) {
        if self.is_none() {
            *self = other.cloned();
        }
    }
}

/// Specifies an input file path or glob pattern to include in version replacement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputFile {
    /// A specific file path.
    Path(PathBuf),
    /// A glob pattern matching multiple files.
    GlobPattern {
        /// Glob pattern string, e.g., `src/**/*.rs`.
        pattern: String,
        /// Optional list of patterns to exclude.
        exclude_patterns: Option<Vec<String>>,
    },
}

impl InputFile {
    pub fn glob(pattern: impl Into<String>) -> Self {
        Self::GlobPattern {
            pattern: pattern.into(),
            exclude_patterns: None,
        }
    }

    #[must_use]
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path.as_path()),
            Self::GlobPattern { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Mutable configuration collected from parsing sources, before defaults are applied.
pub struct Config {
    /// Global configuration settings.
    pub global: global::GlobalConfig,
    /// File-specific configuration entries.
    pub files: Vec<(InputFile, file::FileConfig)>,
    /// Version components to parse and serialize.
    pub components: version::VersionComponentConfigs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Finalized configuration with defaults applied, ready for version bump operations.
pub struct FinalizedConfig {
    /// Fully resolved global configuration with defaults applied.
    pub global: global::GlobalConfigFinalized,
    /// Finalized per-file configurations.
    pub files: Vec<(InputFile, file::FinalizedFileConfig)>,
    /// Version component specifications.
    pub components: version::VersionComponentConfigs,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: global::GlobalConfig::empty(),
            files: Vec::new(),
            components: version::VersionComponentConfigs::default(),
        }
    }
}

impl Config {
    /// Merge global settings into each file-specific configuration.
    pub fn merge_file_configs_with_global_config(&mut self) {
        for (_, file_config) in &mut self.files {
            file_config.merge_with(&self.global);
        }
    }

    // /// Apply defaults.
    // pub fn apply_defaults(&mut self, defaults: &global::GlobalConfig) {
    //     self.global.merge_with(defaults);
    //     for (_, file_config) in &mut self.files {
    //         file_config.merge_with(defaults);
    //     }
    // }

    /// Finalize and resolve all configuration options.
    ///
    /// Unset values are filled with defaults from global settings.
    #[must_use]
    pub fn finalize(mut self) -> FinalizedConfig {
        self.merge_file_configs_with_global_config();
        FinalizedConfig {
            global: self.global.finalize(),
            files: self
                .files
                .into_iter()
                .map(|(path, config)| (path, config.finalize()))
                .collect(),
            components: self.components,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Config, global,
        version::{self, VersionComponentConfigs, VersionComponentSpec},
    };
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use similar_asserts::assert_eq as sim_assert_eq;

    #[test]
    fn test_get_all_component_configs_dependent() -> eyre::Result<()> {
        crate::tests::init();
        let config = Config {
            global: global::GlobalConfig {
                parse_version_pattern: Some(
                    regex::Regex::new(r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)")?.into(),
                ),
                ..global::GlobalConfig::empty()
            },
            files: vec![],
            components: [].into_iter().collect(),
        };
        let config = config.finalize();
        let component_configs = version::version_component_configs(&config);
        sim_assert_eq!(
            component_configs,
            [
                (
                    "major".to_string(),
                    VersionComponentSpec {
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
                (
                    "minor".to_string(),
                    VersionComponentSpec {
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
                (
                    "patch".to_string(),
                    VersionComponentSpec {
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
            ]
            .into_iter()
            .collect::<IndexMap<_, _>>()
        );

        Ok(())
    }

    #[test]
    fn test_get_all_component_configs_with_parts() -> eyre::Result<()> {
        crate::tests::init();
        let config = Config {
            global: global::GlobalConfig {
                parse_version_pattern: Some(
                    regex::Regex::new(r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)")?.into(),
                ),
                ..global::GlobalConfig::empty()
            },
            files: vec![],
            components: [
                (
                    "major".to_string(),
                    VersionComponentSpec {
                        independent: Some(false),
                        values: vec!["value1".to_string(), "value2".to_string()],
                        ..VersionComponentSpec::default()
                    },
                ),
                (
                    "minor".to_string(),
                    VersionComponentSpec {
                        independent: Some(true),
                        values: vec!["value3".to_string(), "value4".to_string()],
                        ..VersionComponentSpec::default()
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };
        let config = config.finalize();
        let component_configs = version::version_component_configs(&config);
        sim_assert_eq!(
            component_configs,
            [
                (
                    "major".to_string(),
                    VersionComponentSpec {
                        independent: Some(false),
                        values: vec!["value1".to_string(), "value2".to_string()],
                        ..VersionComponentSpec::default()
                    }
                ),
                (
                    "minor".to_string(),
                    VersionComponentSpec {
                        independent: Some(true),
                        values: vec!["value3".to_string(), "value4".to_string()],
                        ..VersionComponentSpec::default()
                    }
                ),
                (
                    "patch".to_string(),
                    VersionComponentSpec {
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
            ]
            .into_iter()
            .collect::<VersionComponentConfigs>()
        );

        Ok(())
    }
}
