pub mod ini;
pub mod pyproject_toml;
pub mod toml;

use crate::diagnostics::{DiagnosticExt, FileId, Printer, Span, Spanned};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use color_eyre::eyre;
use color_eyre::owo_colors::OwoColorize;
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    BumpversionToml(pyproject_toml::Error),
    #[error(transparent)]
    PyProject(pyproject_toml::Error),
    #[error(transparent)]
    SetupCfg(#[from] ini::Error),
    #[error("TODO")]
    CargoToml(()),
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid integer: {0}")]
    BadInt(#[from] std::num::TryFromIntError),

    #[error("{0}")]
    Unknown(String),
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

impl From<String> for ParseError {
    fn from(message: String) -> Self {
        Self::Unknown(message)
    }
}

// #[derive(thiserror::Error, Debug)]
// pub enum Error {
//     #[error("failed to read config file: {0}")]
//     Io(#[from] std::io::Error),
//
//     #[error("failed to parse config file: {0}")]
//     Parse(#[from] ParseError),
// }

fn deserialize_python_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    // let s: &str = serde::de::Deserialize::deserialize(deserializer)?;
    let s: Option<String> = serde::de::Deserialize::deserialize(deserializer).ok();
    let Some(s) = s else {
        return Ok(None);
    };
    match s.as_str() {
        "" => Ok(None),
        "True" | "true" => Ok(Some(true)),
        "False" | "false" => Ok(Some(false)),
        _ => Err(serde::de::Error::unknown_variant(
            &s,
            &["True", "true", "False", "false"],
        )),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
pub struct FileConfig {
    /// Don't abort if working directory is dirty
    pub allow_dirty: Option<bool>,
    /// Version that needs to be updated
    pub current_version: Option<String>,
    /// Regex parsing the version string
    pub parse_version_pattern: Option<String>,
    /// How to serialize back to a version
    pub serialize_version_patterns: Option<Vec<String>>,
    /// Template for complete string to search
    pub search: Option<String>,
    /// Template for complete string to replace
    pub replace: Option<String>,
    /// Treat the search parameter as a regular expression
    pub regex: Option<bool>,
    /// Only replace the version in files specified on the command line.
    ///
    /// When enabled, the files from the configuration file are ignored
    pub no_configured_files: Option<bool>,
    /// Ignore any missing files when searching and replacing in files
    pub ignore_missing_files: Option<bool>,
    /// Ignore any missing version when searching and replacing in files
    pub ignore_missing_version: Option<bool>,
    /// Don't write any files, just pretend
    pub dry_run: Option<bool>,
    /// Commit to version control
    #[serde(deserialize_with = "deserialize_python_bool", default)]
    pub commit: Option<bool>,
    /// Create a tag in version control
    #[serde(deserialize_with = "deserialize_python_bool", default)]
    pub tag: Option<bool>,
    /// Sign tags if created
    pub sign_tags: Option<bool>,
    /// Tag name (only works with --tag)
    pub tag_name: Option<String>,
    /// Tag message
    pub tag_message: Option<String>,
    /// Commit message
    #[serde(rename = "message")]
    pub commit_message: Option<String>,
    /// Extra arguments to commit command
    pub commit_args: Option<String>,

    // extra stuff
    /// Setup hooks
    pub setup_hooks: Option<Vec<String>>,
    /// Pre-commit hooks
    pub pre_commit_hooks: Option<Vec<String>>,
    /// Post-commit hooks
    pub post_commit_hooks: Option<Vec<String>>,
    /// Included paths
    pub included_paths: Option<Vec<PathBuf>>,
    /// Excluded paths
    pub excluded_paths: Option<Vec<PathBuf>>,
}

impl FileConfig {
    pub fn empty() -> Self {
        Self {
            allow_dirty: None,
            current_version: None,
            parse_version_pattern: None,
            serialize_version_patterns: None,
            search: None,
            replace: None,
            regex: None,
            no_configured_files: None,
            ignore_missing_files: None,
            ignore_missing_version: None,
            dry_run: None,
            commit: None,
            tag: None,
            sign_tags: None,
            tag_name: None,
            tag_message: None,
            commit_message: None,
            commit_args: None,
            setup_hooks: None,
            pre_commit_hooks: None,
            post_commit_hooks: None,
            included_paths: None,
            excluded_paths: None,
        }
    }
}

pub const DEFAULT_PARSE_VERSION_PATTERN: &str = r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)";
pub static DEFAULT_PARSE_VERSION_REGEX: once_cell::sync::Lazy<regex::Regex> =
    once_cell::sync::Lazy::new(|| {
        regex::RegexBuilder::new(DEFAULT_PARSE_VERSION_PATTERN)
            .build()
            .unwrap()
    });

pub const DEFAULT_TAG_NAME: &str = r"v{new_version}";

impl FileConfig {
    pub fn default() -> Self {
        Self {
            parse_version_pattern: Some(DEFAULT_PARSE_VERSION_PATTERN.to_string()), // TODO: use regex here?
            serialize_version_patterns: Some(vec!["{major}.{minor}.{patch}".to_string()]),
            search: Some("{current_version}".to_string()),
            replace: Some("{new_version}".to_string()),
            regex: Some(false),
            ignore_missing_version: Some(false),
            ignore_missing_files: Some(false),
            tag: Some(false),
            sign_tags: Some(false),
            tag_name: Some(DEFAULT_TAG_NAME.to_string()),
            tag_message: Some("Bump version: {current_version} → {new_version}".to_string()),
            allow_dirty: Some(false),
            commit: Some(false),
            commit_message: Some("Bump version: {current_version} → {new_version}".to_string()),
            ..FileConfig::empty() // current_version: None,
                                  // commit_args: None,
                                  // setup_hooks: None,
                                  // pre_commit_hooks: None,
                                  // post_commit_hooks: None,
        }
    }
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

impl FileConfig {
    pub fn merge_with(&mut self, other: &Self) {
        self.allow_dirty.merge_with(other.allow_dirty.as_ref());
        self.current_version
            .merge_with(other.current_version.as_ref());
        self.parse_version_pattern
            .merge_with(other.parse_version_pattern.as_ref());
        self.serialize_version_patterns
            .merge_with(other.serialize_version_patterns.as_ref());
        self.search.merge_with(other.search.as_ref());
        self.replace.merge_with(other.replace.as_ref());
        self.regex.merge_with(other.regex.as_ref());
        self.no_configured_files
            .merge_with(other.no_configured_files.as_ref());
        self.ignore_missing_files
            .merge_with(other.ignore_missing_files.as_ref());
        self.ignore_missing_version
            .merge_with(other.ignore_missing_version.as_ref());
        self.dry_run.merge_with(other.dry_run.as_ref());
        self.commit.merge_with(other.commit.as_ref());
        self.tag.merge_with(other.tag.as_ref());
        self.sign_tags.merge_with(other.sign_tags.as_ref());
        self.tag_name.merge_with(other.tag_name.as_ref());
        self.tag_message.merge_with(other.tag_message.as_ref());
        self.commit_message
            .merge_with(other.commit_message.as_ref());
        self.commit_args.merge_with(other.commit_args.as_ref());
        self.setup_hooks.merge_with(other.setup_hooks.as_ref());
        self.pre_commit_hooks
            .merge_with(other.pre_commit_hooks.as_ref());
        self.post_commit_hooks
            .merge_with(other.post_commit_hooks.as_ref());
        self.included_paths
            .merge_with(other.included_paths.as_ref());
        self.excluded_paths
            .merge_with(other.excluded_paths.as_ref());
    }
}

// #[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
// pub struct PartConfig {
//     pub independent: Option<bool>,
//     pub optional_value: Option<String>,
//     pub values: Vec<String>,
//
//     /// Is the component independent of the other components?
//     pub independent: Option<bool>,
//     /// """The value that is optional to include in the version.
//
//     /// - Defaults to first value in values or 0 in the case of numeric.
//     /// - Empty string means nothing is optional.
//     /// - CalVer components ignore this."""
//     pub optional_value: Option<String>,
//
//     /// The possible values for the component.
//     ///
//     /// If it and `calver_format` is None, the component is numeric.
//     pub values: Vec<String>,
//     /// The first value to increment from
//     pub first_value: Option<String>,
//
//     /// Should the component always increment, even if it is not necessary?
//     pub always_increment: bool,
//
//     /// The format string for a CalVer component
//     pub calver_format: Option<String>,
//
//     /// The name of the component this component depends on
//     pub depends_on: Option<String>,
// }

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

    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path.as_path()),
            _ => None,
        }
    }
}

pub type Files = Vec<(InputFile, FileConfig)>;
pub type Parts = IndexMap<String, VersionComponentSpec>;
// pub type Parts = IndexMap<String, PartConfig>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub global: FileConfig,
    // pub files: IndexMap<PathBuf, FileConfig>,
    pub files: Files,
    pub parts: Parts,
    // pub path: Option<PathBuf>,
    // pub parts: Vec<String, PartConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: FileConfig::empty(),
            files: Files::default(),
            parts: Parts::default(),
        }
    }
}

impl Config {
    /// Merge global config with per-file configurations
    pub fn merge_global_config(&mut self) {
        for (_, file_config) in self.files.iter_mut() {
            file_config.merge_with(&self.global);
        }
    }

    /// Apply defaults.
    pub fn apply_defaults(&mut self, defaults: &FileConfig) {
        self.global.merge_with(defaults);
        for (_, file_config) in self.files.iter_mut() {
            file_config.merge_with(defaults);
        }
    }
}

/// A change to make to a file
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileChange {
    // pub file: InputFile,
    pub parse_pattern: Option<String>,
    pub serialize_patterns: Option<Vec<String>>,
    pub search: Option<String>,
    pub replace: Option<String>,
    pub regex: Option<bool>,
    pub ignore_missing_version: Option<bool>,
    pub ignore_missing_files: Option<bool>,
    // pub filename: Option<PathBuf>,
    // Conflicts with filename. If both are specified, glob wins
    // pub glob: Option<String>,
    // pub glob_exclude: Option<String>,
    // If specified, and has an appropriate extension, will be treated as a data file
    pub key_path: Option<String>,
    // pub include_bumps: Parts, //  Option<String>,
    pub include_bumps: Option<Vec<String>>,
    pub exclude_bumps: Option<Vec<String>>,
}

impl FileChange {
    pub fn defaults() -> Self {
        Self {
            regex: Some(false),
            ignore_missing_files: Some(false),
            ignore_missing_version: Some(false),
            ..FileChange::default()
        }
    }

    pub fn new(file_config: FileConfig, parts: &Parts) -> Self {
        Self {
            // parse: file_config.parse.or(global.parse),
            // serialize: file_config.serialize.or(global.serialize),
            // search: file_config.search.or(global.search),
            // replace: file_config.replace.or(global.replace),
            // regex: file_config.regex.or(global.regex),
            parse_pattern: file_config.parse_version_pattern,
            serialize_patterns: file_config.serialize_version_patterns,
            search: file_config.search,
            replace: file_config.replace,
            regex: file_config.regex,
            ignore_missing_version: file_config.ignore_missing_version,
            // .or(config.global.ignore_missing_version),
            ignore_missing_files: file_config.ignore_missing_files,
            // .or(config.global.ignore_missing_files),
            include_bumps: Some(parts.keys().cloned().collect()),
            // glob: None,
            // glob_exclude: None,
            key_path: None,
            exclude_bumps: None,
            // glob: file_config.glob.or(config.global.glob),
            // glob_exclude: file_config.glob.or(config.global.glob),
            // filename: path.as_path().map(Path::to_path_buf),
        }
    }

    pub fn merge_with(&mut self, other: &Self) {
        self.parse_pattern.merge_with(other.parse_pattern.as_ref());
        self.serialize_patterns
            .merge_with(other.serialize_patterns.as_ref());
        self.search.merge_with(other.search.as_ref());
        self.replace.merge_with(other.replace.as_ref());
        self.regex.merge_with(other.regex.as_ref());
        self.ignore_missing_version
            .merge_with(other.ignore_missing_version.as_ref());
        self.ignore_missing_files
            .merge_with(other.ignore_missing_files.as_ref());
        // self.filename.merge_with(other.filename.as_ref());
        // self.glob.merge_with(other.glob.as_ref());
        // self.glob_exclude.merge_with(other.glob_exclude.as_ref());
        self.key_path.merge_with(other.key_path.as_ref());
        self.include_bumps.merge_with(other.include_bumps.as_ref());
        self.exclude_bumps.merge_with(other.exclude_bumps.as_ref());
    }

    pub fn will_bump_component(&self, component: &str) -> bool {
        self.include_bumps
            .as_ref()
            .is_some_and(|bumps| bumps.iter().find(|c| c.as_str() == component).is_some())
    }

    pub fn will_not_bump_component(&self, component: &str) -> bool {
        self.exclude_bumps
            .as_ref()
            .is_some_and(|bumps| bumps.iter().find(|c| c.as_str() == component).is_some())
    }
}

#[deprecated]
pub fn get_all_file_configs(
    config: &Config,
    // parts: &IndexMap<String, VersionComponentSpec>,
    parts: &Parts,
) -> Vec<(InputFile, FileChange)> {
    config
        .files
        .iter()
        .cloned()
        .map(|(input_file, file_config)| {
            let global = config.global.clone();
            let file_change = FileChange {
                // parse: file_config.parse.or(global.parse),
                // serialize: file_config.serialize.or(global.serialize),
                // search: file_config.search.or(global.search),
                // replace: file_config.replace.or(global.replace),
                // regex: file_config.regex.or(global.regex),
                parse_pattern: file_config.parse_version_pattern,
                serialize_patterns: file_config.serialize_version_patterns,
                search: file_config.search,
                replace: file_config.replace,
                regex: file_config.regex,
                ignore_missing_version: file_config.ignore_missing_version,
                // .or(config.global.ignore_missing_version),
                ignore_missing_files: file_config.ignore_missing_files,
                // .or(config.global.ignore_missing_files),
                include_bumps: Some(parts.keys().cloned().collect()),
                // glob: None,
                // glob_exclude: None,
                key_path: None,
                exclude_bumps: None,
                // glob: file_config.glob.or(config.global.glob),
                // glob_exclude: file_config.glob.or(config.global.glob),
                // filename: path.as_path().map(Path::to_path_buf),
            };
            (input_file, file_change)
        })
        .collect()
}

/// Configuration of a version component.
///
/// This is used to read in the configuration from the bumpversion config file.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionComponentSpec {
    /// Is the component independent of the other components?
    pub independent: Option<bool>,
    /// """The value that is optional to include in the version.

    /// - Defaults to first value in values or 0 in the case of numeric.
    /// - Empty string means nothing is optional.
    /// - CalVer components ignore this."""
    pub optional_value: Option<String>,

    /// The possible values for the component.
    ///
    /// If it and `calver_format` is None, the component is numeric.
    pub values: Vec<String>,
    /// The first value to increment from
    pub first_value: Option<String>,

    /// Should the component always increment, even if it is not necessary?
    pub always_increment: bool,

    /// The format string for a CalVer component
    pub calver_format: Option<String>,

    /// The name of the component this component depends on
    pub depends_on: Option<String>,
}

// impl VersionComponentSpec {
//     pub fn from_part(part: &VersionComponentSpec) -> Self {
//         Self {
//             independent: part.independent,
//             optional_value: part.optional_value.clone(),
//             values: part.values.clone(),
//         }
//     }
// }

/// Make sure all version components are included
pub fn get_all_part_configs(
    config: &Config,
    // ) -> eyre::Result<IndexMap<String, VersionComponentSpec>> {
) -> eyre::Result<Parts> {
    let parsing_groups: Vec<String> = match &config.global.parse_version_pattern {
        Some(parse) => {
            let re = regex::Regex::new(parse)?;
            re.capture_names()
                .filter_map(|name| name)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        }
        None => vec![],
    };
    // let part_configs: IndexMap<String, > = parsing_groups
    let part_configs: Parts = parsing_groups
        .into_iter()
        .map(|label| {
            let is_independent = label.starts_with("$");
            let mut spec = match config.parts.get(&label) {
                // Some(part) => VersionComponentSpec{..VersionComponentSpec::from_part(part)},
                // Some(part) => VersionComponentSpec::from_part(part),
                Some(part) => part.clone(),
                None => VersionComponentSpec::default(),
                // None => VersionComponentSpec::default(),
                // None => VersionComponentSpec {
                //     independent: Some(is_independent),
                //     ..VersionComponentSpec::default()
                // },
            };
            spec.independent.merge_with(Some(&is_independent));
            (label, spec)
        })
        .collect();
    Ok(part_configs)
}

#[cfg(test)]
mod tests {
    use super::{Config, FileConfig, Parts, VersionComponentSpec};
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use similar_asserts::assert_eq as sim_assert_eq;

    #[test]
    fn test_get_all_part_configs_dependent() -> eyre::Result<()> {
        crate::tests::init();
        let config = Config {
            global: FileConfig {
                parse_version_pattern: Some(
                    r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)".to_string(),
                ),
                ..FileConfig::empty()
            },
            files: vec![],
            parts: [].into_iter().collect(),
        };
        let part_configs = super::get_all_part_configs(&config)?;
        sim_assert_eq!(
            part_configs,
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
    fn test_get_all_part_configs_with_parts() -> eyre::Result<()> {
        crate::tests::init();
        let config = Config {
            global: FileConfig {
                parse_version_pattern: Some(
                    r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)".to_string(),
                ),
                ..FileConfig::empty()
            },
            files: vec![],
            parts: [
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
        let part_configs = super::get_all_part_configs(&config)?;
        sim_assert_eq!(
            part_configs,
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
            .collect::<Parts>()
        );

        Ok(())
    }
}

// impl Config {
//     pub fn parse<W>(
//         path: impl AsRef<Path>,
//         printer: &Printer<W>,
//         strict: bool,
//         diagnostics: &mut Vec<Diagnostic<FileId>>,
//     ) -> eyre::Result<Option<Self>> {
//         let path = path.as_ref();
//         let config = std::fs::read_to_string(path)?;
//         let file_id = printer.add_source_file(path.to_string_lossy().to_string(), config.clone());
//
//         match path.extension().and_then(|ext| ext.to_str()) {
//             Some("cfg") => {
//                 tracing::warn!("the .cfg file format is deprecated. Please use .toml instead");
//                 let options = serde_ini_spanned::value::Options {
//                     strict,
//                     ..serde_ini_spanned::value::Options::default()
//                 };
//                 Self::from_ini(&config, options, file_id, strict, diagnostics).map_err(Into::into)
//             }
//             None | Some("toml") => {
//                 Self::from_pyproject_toml(&config, file_id, strict, diagnostics).map_err(Into::into)
//             }
//             Some(other) => {
//                 eyre::bail!("unknown config file format: {other:?}");
//             }
//         }
//     }
// }

// [tool.bumpversion]
// current_version = "0.28.1"
// commit = true
// commit_args = "--no-verify"
// tag = true
// tag_name = "{new_version}"
// allow_dirty = true
// parse = "(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\.(?P<dev>post)\\d+\\.dev\\d+)?"
// serialize = [
//     "{major}.{minor}.{patch}.{dev}{$PR_NUMBER}.dev{distance_to_latest_tag}",
//     "{major}.{minor}.{patch}"
// ]
// message = "Version updated from {current_version} to {new_version}"
//
// [tool.bumpversion.parts.dev]
// values = ["release", "post"]
//
// [[tool.bumpversion.files]]
// filename = "bumpversion/__init__.py"
//
// [[tool.bumpversion.files]]
// filename = "CHANGELOG.md"
// search = "Unreleased"
//
// [[tool.bumpversion.files]]
// filename = "CHANGELOG.md"
// search = "{current_version}...HEAD"
// replace = "{current_version}...{new_version}"
//
// [[tool.bumpversion.files]]
// filename = "action.yml"
// search = "bump-my-version=={current_version}"
// replace = "bump-my-version=={new_version}"
//
// [[tool.bumpversion.files]]
// filename = "Dockerfile"
// search = "created=\\d{{4}}-\\d{{2}}-\\d{{2}}T\\d{{2}}:\\d{{2}}:\\d{{2}}Z"
// replace = "created={utcnow:%Y-%m-%dT%H:%M:%SZ}"
// regex = true
//
// [[tool.bumpversion.files]]
// filename = "Dockerfile"

// impl Default for Config {
//     fn default() -> Self {
//         Self {
//             // verbosity: 0,
//             // list: false,
//             allow_dirty: false,
//             current_version: None,
//             parse: r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)".into(),
//             serialize: "{major}.{minor}.{patch}".into(),
//             search: "{current_version}".into(),
//             replace: "{new_version}".into(),
//             // no_configured_files: false,
//             dry_run: false,
//             commit: false,
//             no_commit: true,
//             tag: false,
//             no_tag: true,
//             sign_tag: false,
//             no_sign_tag: true,
//             tag_name: "v{new_version}".into(),
//             tag_message: "bump: {current_version} → {new_version}".into(),
//             commit_message: "bump: {current_version} → {new_version}".into(),
//             commit_args: None,
//         }
//     }
// }

// impl Config {
//     pub fn from_ini<S: Into<String>>(content: S) -> Result<Self, ParseError> {
//         let mut config = Self::default();
//         config.load_ini(content)?;
//         Ok(config)
//     }
//
//     pub fn load_ini<S: Into<String>>(&mut self, content: S) -> Result<(), ParseError> {
//         use std::cmp::max;
//
//         // ident, block, stmt, expr, pat, ty, lifetime, literal, path, meta, tt, item, vis
//         macro_rules! get {
//             ($config:ident, $getter:ident, $component:literal, $field:ident) => {
//                 if let Some(val) = $config.$getter("bumpversion", stringify!($field))? {
//                     self.$field = val;
//                 }
//             };
//         }
//
//         let mut config = Ini::new();
//         let _ = config.read(content.into()).unwrap();
//
//         if let Some(verbosity) = config.getuint("bumpversion", "verbosity")? {
//             self.verbosity = max(self.verbosity, verbosity.try_into()?);
//         }
//         // get!(config, getbool, "bumpversion", list);
//         // get!(config, getbool, "bumpversion", allow_dirty);
//         // get!(config, get, "bumpversion", current_version);
//         // get!(config, get, "bumpversion", parse);
//         // get!(config, get, "bumpversion", serialize);
//         // get!(config, getbool, "bumpversion", dry_run);
//         // get!(config, getbool, "bumpversion", commit);
//         // get!(config, getbool, "bumpversion", no_commit);
//         // get!(config, getbool, "bumpversion", tag);
//         // get!(config, getbool, "bumpversion", no_tag);
//         // get!(config, getbool, "bumpversion", sign_tag);
//         // get!(config, getbool, "bumpversion", no_sign_tag);
//         // get!(config, get, "bumpversion", tag_name);
//         // get!(config, get, "bumpversion", tag_message);
//         // get!(config, get, "bumpversion", commit_message);
//
//         // verbosity
//         // list
//         // allow_dirty
//         // current_version
//         // parse
//         // serialize
//         // dry_run
//         // commit
//         // no_commit
//         // tag
//         // no_tag
//         // sign_tag
//         // no_sign_tag
//         // tag_name
//         // tag_message
//         // commit_message
//         // }
//         for section in config.sections() {
//             println!("section: {}", section);
//             // search
//             // replace
//             // parse
//             // serialize
//         }
//
//         // config
//         //     .load_ini(&mut config_file)
//         //     .map_err(|message| Error::ConfigFile { message })?;
//         // .map_err(?;
//         // println!("config: {:?}", config);
//         // if let Some(main_section) = config_file.get("bumpversion") {
//         // println!("bumpversion: {:?}", main_section);
//         Ok(())
//     }
//
//     pub fn from_reader<R: io::Read + io::Seek>(mut reader: R) -> Result<Self, ParseError> {
//         let mut buffered = io::BufReader::new(reader);
//         let mut buf = String::new();
//         buffered.read_to_string(&mut buf)?;
//         Self::from_ini(buf)
//     }
//
//     pub fn open<P: Into<PathBuf>>(path: Option<P>) -> Result<Option<Self>, Error> {
//         let path = match path {
//             Some(p) => Some(p.into()),
//             None => {
//                 let cwd = std::env::current_dir()?;
//                 let bumpversion_cfg = cwd.join(".bumpversion.cfg");
//                 let setup_cfg = cwd.join("setup.cfg");
//                 if bumpversion_cfg.is_file() {
//                     Some(bumpversion_cfg)
//                 } else if setup_cfg.is_file() {
//                     Some(setup_cfg)
//                 } else {
//                     None
//                 }
//             }
//         };
//         match path {
//             Some(path) => {
//                 let config = Self::from_reader(fs::File::open(path)?)?;
//                 Ok(Some(config))
//             }
//             None => Ok(None),
//         }
//     }
// }
