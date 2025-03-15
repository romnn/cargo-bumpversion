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
pub enum Error {
    #[error(transparent)]
    IoError(#[from] IoError),
    #[error("failed to parse {path:?}")]
    Toml {
        path: PathBuf,
        #[source]
        source: pyproject_toml::ParseError,
    },
    #[error("failed to parse {path:?}")]
    Ini {
        path: PathBuf,
        #[source]
        source: ini::ParseError,
    },
    #[error("failed to parse {path:?}")]
    CargoToml {
        path: PathBuf,
        // #[source]
        // source: ini::ParseError,
    },
    #[error("failed to join spawned task")]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Diagnostics(#[from] crate::diagnostics::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigFile {
    // A `bumpversion.toml` configuration file (TOML)
    BumpversionToml(PathBuf),
    // A `pyproject.toml` configuration file (TOML)
    PyProject(PathBuf),
    // A `bumpverison.cfg` configuration file (ini)
    BumpversionCfg(PathBuf),
    // A `setup.cfg` configuration file (ini)
    SetupCfg(PathBuf),
    // A `Cargo.toml` configuration file (TOML)
    CargoToml(PathBuf),
}

impl ConfigFile {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::BumpversionToml(path) => path.as_ref(),
            Self::PyProject(path) => path.as_ref(),
            Self::BumpversionCfg(path) => path.as_ref(),
            Self::SetupCfg(path) => path.as_ref(),
            Self::CargoToml(path) => path.as_ref(),
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputFile {
    Path(PathBuf),
    GlobPattern {
        pattern: String,
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
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub global: global::GlobalConfig,
    pub files: Vec<(InputFile, file::FileConfig)>,
    pub components: version::VersionComponentConfigs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedConfig {
    pub global: global::GlobalConfigFinalized,
    pub files: Vec<(InputFile, file::FinalizedFileConfig)>,
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
    /// Merge global config with per-file configurations
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

    /// Finalize the configuration
    ///
    /// All unset configuration options will be set to their default value.
    #[must_use] pub fn finalize(mut self) -> FinalizedConfig {
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
        global,
        version::{self, VersionComponentConfigs, VersionComponentSpec},
        Config,
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
