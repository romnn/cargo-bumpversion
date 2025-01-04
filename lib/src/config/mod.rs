pub mod ini;
pub mod pyproject_toml;
pub mod toml;

use crate::{
    diagnostics::{DiagnosticExt, FileId, Printer, Span, Spanned},
    f_string::{OwnedPythonFormatString, OwnedValue},
};
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

fn deserialize_python_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalConfig {
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
    // #[serde(deserialize_with = "deserialize_python_bool", default)]
    pub commit: Option<bool>,
    /// Create a tag in version control
    // #[serde(deserialize_with = "deserialize_python_bool", default)]
    pub tag: Option<bool>,
    /// Sign tags if created
    pub sign_tags: Option<bool>,
    /// Tag name (only works with --tag)
    pub tag_name: Option<OwnedPythonFormatString>,
    // pub tag_name: Option<String>,
    /// Tag message
    pub tag_message: Option<OwnedPythonFormatString>,
    // pub tag_message: Option<String>,
    /// Commit message
    // #[serde(rename = "message")]
    pub commit_message: Option<OwnedPythonFormatString>,
    // pub commit_message: Option<String>,
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

impl GlobalConfig {
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileConfig {
    // /// Don't abort if working directory is dirty
    // pub allow_dirty: Option<bool>,
    // /// Version that needs to be updated
    // pub current_version: Option<String>,
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
    // /// Only replace the version in files specified on the command line.
    // ///
    // /// When enabled, the files from the configuration file are ignored
    // pub no_configured_files: Option<bool>,
    /// Ignore missing file when searching and replacing version
    pub ignore_missing_file: Option<bool>,
    /// Ignore any missing version when searching and replacing version
    pub ignore_missing_version: Option<bool>,
    // /// Don't write any files, just pretend
    // pub dry_run: Option<bool>,
    // /// Commit to version control
    // // #[serde(deserialize_with = "deserialize_python_bool", default)]
    // pub commit: Option<bool>,
    // /// Create a tag in version control
    // // #[serde(deserialize_with = "deserialize_python_bool", default)]
    // pub tag: Option<bool>,
    // /// Sign tags if created
    // pub sign_tags: Option<bool>,
    // /// Tag name (only works with --tag)
    // pub tag_name: Option<String>,
    // /// Tag message
    // pub tag_message: Option<String>,
    // /// Commit message
    // // #[serde(rename = "message")]
    // pub commit_message: Option<OwnedPythonFormatString>,
    // // pub commit_message: Option<String>,
    // /// Extra arguments to commit command
    // pub commit_args: Option<String>,
    //
    // // extra stuff
    // /// Setup hooks
    // pub setup_hooks: Option<Vec<String>>,
    // /// Pre-commit hooks
    // pub pre_commit_hooks: Option<Vec<String>>,
    // /// Post-commit hooks
    // pub post_commit_hooks: Option<Vec<String>>,
    // /// Included paths
    // pub included_paths: Option<Vec<PathBuf>>,
    // /// Excluded paths
    // pub excluded_paths: Option<Vec<PathBuf>>,
}

impl FileConfig {
    pub fn empty() -> Self {
        Self {
            // allow_dirty: None,
            // current_version: None,
            parse_version_pattern: None,
            serialize_version_patterns: None,
            search: None,
            replace: None,
            regex: None,
            // no_configured_files: None,
            ignore_missing_file: None,
            ignore_missing_version: None,
            // dry_run: None,
            // commit: None,
            // tag: None,
            // sign_tags: None,
            // tag_name: None,
            // tag_message: None,
            // commit_message: None,
            // commit_args: None,
            // setup_hooks: None,
            // pre_commit_hooks: None,
            // post_commit_hooks: None,
            // included_paths: None,
            // excluded_paths: None,
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

pub static DEFAULT_SERIALIZE_VERSION_PATTERNS: once_cell::sync::Lazy<Vec<String>> =
    once_cell::sync::Lazy::new(|| vec!["{major}.{minor}.{patch}".to_string()]);

pub const DEFAULT_SEARCH: &str = "{current_version}";
pub const DEFAULT_SEARCH_IS_REGEX: bool = false;
pub const DEFAULT_REPLACE: &str = "{new_version}";

pub static DEFAULT_TAG_NAME: once_cell::sync::Lazy<OwnedPythonFormatString> =
    once_cell::sync::Lazy::new(|| {
        OwnedPythonFormatString(vec![
            OwnedValue::String(String::from("v")),
            OwnedValue::Argument("new_version".to_string()),
        ])
    });

pub static DEFAULT_TAG_MESSAGE: once_cell::sync::Lazy<OwnedPythonFormatString> =
    once_cell::sync::Lazy::new(|| {
        OwnedPythonFormatString(vec![
            OwnedValue::String("Bump version: ".to_string()),
            OwnedValue::Argument("current_version".to_string()),
            OwnedValue::String(" → ".to_string()),
            OwnedValue::Argument("new_version".to_string()),
        ])
    });

pub const DEFAULT_IGNORE_MISSING_VERSION: bool = false;
pub const DEFAULT_IGNORE_MISSING_FILES: bool = false;
pub const DEFAULT_CREATE_TAG: bool = false;
pub const DEFAULT_SIGN_TAGS: bool = false;
pub const DEFAULT_ALLOW_DIRTY: bool = false;
pub const DEFAULT_COMMIT: bool = false;

pub static DEFAULT_COMMIT_MESSAGE: once_cell::sync::Lazy<OwnedPythonFormatString> =
    once_cell::sync::Lazy::new(|| {
        OwnedPythonFormatString(vec![
            OwnedValue::String("Bump version: ".to_string()),
            OwnedValue::Argument("current_version".to_string()),
            OwnedValue::String(" → ".to_string()),
            OwnedValue::Argument("new_version".to_string()),
        ])
    });

impl GlobalConfig {
    pub fn default() -> Self {
        Self {
            parse_version_pattern: Some(DEFAULT_PARSE_VERSION_PATTERN.to_string()), // TODO: use regex here?
            serialize_version_patterns: Some(DEFAULT_SERIALIZE_VERSION_PATTERNS.clone()),
            search: Some(DEFAULT_SEARCH.to_string()),
            replace: Some(DEFAULT_REPLACE.to_string()),
            regex: Some(DEFAULT_SEARCH_IS_REGEX),
            ignore_missing_version: Some(DEFAULT_IGNORE_MISSING_VERSION),
            ignore_missing_files: Some(DEFAULT_IGNORE_MISSING_FILES),
            tag: Some(DEFAULT_CREATE_TAG),
            sign_tags: Some(DEFAULT_SIGN_TAGS),
            tag_name: Some(DEFAULT_TAG_NAME.clone()),
            tag_message: Some(DEFAULT_TAG_MESSAGE.clone()),
            allow_dirty: Some(DEFAULT_ALLOW_DIRTY),
            commit: Some(DEFAULT_COMMIT),
            commit_message: Some(DEFAULT_COMMIT_MESSAGE.clone()),
            ..GlobalConfig::empty()
        }
    }
}

impl FileConfig {
    pub fn default() -> Self {
        Self {
            parse_version_pattern: Some(DEFAULT_PARSE_VERSION_PATTERN.to_string()), // TODO: use regex here?
            serialize_version_patterns: Some(DEFAULT_SERIALIZE_VERSION_PATTERNS.clone()),
            search: Some(DEFAULT_SEARCH.to_string()),
            replace: Some(DEFAULT_REPLACE.to_string()),
            regex: Some(DEFAULT_SEARCH_IS_REGEX),
            ignore_missing_version: Some(DEFAULT_IGNORE_MISSING_VERSION),
            ignore_missing_file: Some(DEFAULT_IGNORE_MISSING_FILES),
            // tag: Some(false),
            // sign_tags: Some(false),
            // tag_name: Some(DEFAULT_TAG_NAME.to_string()),
            // tag_message: Some("Bump version: {current_version} → {new_version}".to_string()),
            // allow_dirty: Some(false),
            // commit: Some(false),
            // // commit_message: Some("Bump version: {current_version} → {new_version}".to_string()),
            // commit_message: Some(OwnedPythonFormatString(vec![
            //     OwnedValue::String("Bump version: ".to_string()),
            //     OwnedValue::Argument("current_version".to_string()),
            //     OwnedValue::String(" → ".to_string()),
            //     OwnedValue::Argument("new_version".to_string()),
            //     // "Bump version: {current_version} → {new_version}",
            // ])),
            // ..FileConfig::empty()
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

impl<'a> MergeWith<&'a GlobalConfig> for FileConfig {
    fn merge_with(&mut self, other: &'a GlobalConfig) {
        self.parse_version_pattern
            .merge_with(other.parse_version_pattern.as_ref());
        self.serialize_version_patterns
            .merge_with(other.serialize_version_patterns.as_ref());
        self.search.merge_with(other.search.as_ref());
        self.replace.merge_with(other.replace.as_ref());
        self.regex.merge_with(other.regex.as_ref());
        self.ignore_missing_file
            .merge_with(other.ignore_missing_files.as_ref());
        self.ignore_missing_version
            .merge_with(other.ignore_missing_version.as_ref());
    }
}

impl<'a> MergeWith<&'a GlobalConfig> for GlobalConfig {
    fn merge_with(&mut self, other: &'a GlobalConfig) {
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

// impl FileConfig {
//     pub fn merge_with(&mut self, other: &GlobalConfig) {
//         self.parse_version_pattern
//             .merge_with(other.parse_version_pattern.as_ref());
//         self.serialize_version_patterns
//             .merge_with(other.serialize_version_patterns.as_ref());
//         self.search.merge_with(other.search.as_ref());
//         self.replace.merge_with(other.replace.as_ref());
//         self.regex.merge_with(other.regex.as_ref());
//     }
// }

// impl FileConfig {
//     pub fn merge_with(&mut self, other: &Self) {
//         self.allow_dirty.merge_with(other.allow_dirty.as_ref());
//         self.current_version
//             .merge_with(other.current_version.as_ref());
//         self.parse_version_pattern
//             .merge_with(other.parse_version_pattern.as_ref());
//         self.serialize_version_patterns
//             .merge_with(other.serialize_version_patterns.as_ref());
//         self.search.merge_with(other.search.as_ref());
//         self.replace.merge_with(other.replace.as_ref());
//         self.regex.merge_with(other.regex.as_ref());
//         self.no_configured_files
//             .merge_with(other.no_configured_files.as_ref());
//         self.ignore_missing_files
//             .merge_with(other.ignore_missing_files.as_ref());
//         self.ignore_missing_version
//             .merge_with(other.ignore_missing_version.as_ref());
//         self.dry_run.merge_with(other.dry_run.as_ref());
//         self.commit.merge_with(other.commit.as_ref());
//         self.tag.merge_with(other.tag.as_ref());
//         self.sign_tags.merge_with(other.sign_tags.as_ref());
//         self.tag_name.merge_with(other.tag_name.as_ref());
//         self.tag_message.merge_with(other.tag_message.as_ref());
//         self.commit_message
//             .merge_with(other.commit_message.as_ref());
//         self.commit_args.merge_with(other.commit_args.as_ref());
//         self.setup_hooks.merge_with(other.setup_hooks.as_ref());
//         self.pre_commit_hooks
//             .merge_with(other.pre_commit_hooks.as_ref());
//         self.post_commit_hooks
//             .merge_with(other.post_commit_hooks.as_ref());
//         self.included_paths
//             .merge_with(other.included_paths.as_ref());
//         self.excluded_paths
//             .merge_with(other.excluded_paths.as_ref());
//     }
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

pub type FileConfigs = Vec<(InputFile, FileConfig)>;
pub type VersionComponentConfigs = IndexMap<String, VersionComponentSpec>;
// pub type Parts = IndexMap<String, VersionComponentSpec>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub global: GlobalConfig,
    // pub global: FileConfig,
    // pub files: IndexMap<PathBuf, FileConfig>,
    pub files: FileConfigs,
    pub components: VersionComponentConfigs,
    // pub path: Option<PathBuf>,
    // pub parts: Vec<String, PartConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: GlobalConfig::empty(),
            files: FileConfigs::default(),
            components: VersionComponentConfigs::default(),
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
    // pub fn apply_defaults(&mut self, defaults: &FileConfig) {
    pub fn apply_defaults(&mut self, defaults: &GlobalConfig) {
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
    // pub parse_pattern: Option<String>,
    pub parse_version_pattern: String,
    pub serialize_version_patterns: Vec<String>,
    // pub search: Option<String>,
    pub search: String,
    // pub replace: Option<String>,
    pub replace: String,
    pub regex: bool,
    pub ignore_missing_version: bool,
    pub ignore_missing_file: bool,
    // pub filename: Option<PathBuf>,
    // Conflicts with filename. If both are specified, glob wins
    // pub glob: Option<String>,
    // pub glob_exclude: Option<String>,
    // If specified, and has an appropriate extension, will be treated as a data file
    pub key_path: Option<String>,
    pub include_bumps: Option<Vec<String>>,
    pub exclude_bumps: Option<Vec<String>>,
}

impl FileChange {
    pub fn new(file_config: FileConfig, components: &VersionComponentConfigs) -> Self {
        Self {
            parse_version_pattern: file_config
                .parse_version_pattern
                .unwrap_or(DEFAULT_PARSE_VERSION_PATTERN.to_string()),
            serialize_version_patterns: file_config
                .serialize_version_patterns
                .unwrap_or(DEFAULT_SERIALIZE_VERSION_PATTERNS.clone()),
            // TODO: make this an enum that is either regex or string?
            search: file_config.search.unwrap_or(DEFAULT_SEARCH.to_string()),
            replace: file_config.replace.unwrap_or(DEFAULT_REPLACE.to_string()),
            regex: file_config.regex.unwrap_or(DEFAULT_SEARCH_IS_REGEX),
            ignore_missing_version: file_config
                .ignore_missing_version
                .unwrap_or(DEFAULT_IGNORE_MISSING_VERSION),
            ignore_missing_file: file_config
                .ignore_missing_file
                .unwrap_or(DEFAULT_IGNORE_MISSING_FILES),
            include_bumps: Some(components.keys().cloned().collect()),
            key_path: None,
            exclude_bumps: None,
        }
    }

    // /// Render the search pattern and return the compiled regex pattern and
    // /// the raw pattern.
    // ///
    // /// # Returns
    // /// A tuple of the compiled regex pattern and the raw pattern as a string.
    // fn get_search_pattern(
    //     search: &OwnedPythonFormatString,
    //     ctx: &HashMap<&str, &str>,
    // ) -> eyre::Result<(regex::Regex, String)> {
    //     // tracing::debug!("rendering search pattern with context");
    //
    //     // the default search pattern is escaped,
    //     // so we can still use it in a regex
    //     let strict = true;
    //     let raw_pattern = search.format(ctx, strict)?;
    //     let default = regex::RegexBuilder::new(&regex::escape(&raw_pattern))
    //         .multi_line(true)
    //         .build()?;
    //     // , re.MULTILINE | re.DOTALL)
    //     // if not self.regex:
    //     //     logger.debug("No RegEx flag detected. Searching for the default pattern: '%s'", default.pattern)
    //     //     return default, raw_pattern
    //
    //     let regex_context = ctx.iter().map(|(k, v)| (*k, regex::escape(v))).collect();
    //     let regex_pattern = search.format(&regex_context, strict)?;
    //
    //     match regex::RegexBuilder::new(&regex_pattern)
    //         .multi_line(true)
    //         .build()
    //     {
    //         Ok(regex_pattern) => {
    //             tracing::debug!("searching for regex {}", regex_pattern.as_str());
    //             return Ok((regex_pattern, raw_pattern));
    //         }
    //         Err(err) => {
    //             tracing::error!("invalid regex {:?}: {:?}", default, err);
    //         }
    //     }
    //
    //     tracing::debug!(pattern = ?raw_pattern, "invalid regex, searching for default pattern");
    //
    //     Ok((default, raw_pattern))
    // }

    /// Render the search pattern and return the compiled regex pattern and the raw pattern
    pub fn search_pattern<K, V>(&self, ctx: &HashMap<K, V>) -> eyre::Result<regex::Regex>
    where
        K: std::borrow::Borrow<str>,
        K: std::hash::Hash + Eq,
        V: AsRef<str>,
    {
        tracing::debug!("rendering search pattern with context");
        // the default search pattern is escaped, so we can still use it in a regex
        let strict = true;
        let search = OwnedPythonFormatString::parse(&self.search)?;
        let raw_pattern = search.format(ctx, strict)?;
        let default_regex = regex::RegexBuilder::new(&regex::escape(raw_pattern.as_str()))
            .multi_line(true)
            .build()?;

        if !self.regex {
            tracing::debug!(
                pattern = default_regex.as_str(),
                "searching for default pattern"
            );
            return Ok(default_regex);
        }

        let ctx: HashMap<&str, String> = ctx
            .into_iter()
            .map(|(k, v)| (k.borrow(), regex::escape(v.as_ref())))
            .collect();
        let regex_pattern = search.format(&ctx, strict)?;
        let search_regex = regex::RegexBuilder::new(&regex_pattern)
            .multi_line(true)
            .build()?;
        tracing::debug!(pattern = search_regex.as_str(), "searching for the regex");

        Ok(search_regex)
    }

    // let file_change = FileChange {
    //     parse_pattern: file_config.parse_version_pattern.unwrap_or(DEFAULT_PARSE),
    //     serialize_patterns: file_config.serialize_version_patterns,
    //     search: file_config.search,
    //     replace: file_config.replace,
    //     regex: file_config.regex,
    //     ignore_missing_version: file_config.ignore_missing_version,
    //     ignore_missing_files: file_config.ignore_missing_files,
    //     include_bumps: Some(parts.keys().cloned().collect()),
    //     key_path: None,
    //     exclude_bumps: None,
    // };

    // pub fn merge_with(&mut self, other: &Self) {
    //     self.parse_pattern.merge_with(other.parse_pattern.as_ref());
    //     self.serialize_patterns
    //         .merge_with(other.serialize_patterns.as_ref());
    //     self.search.merge_with(other.search.as_ref());
    //     self.replace.merge_with(other.replace.as_ref());
    //     self.regex.merge_with(other.regex.as_ref());
    //     self.ignore_missing_version
    //         .merge_with(other.ignore_missing_version.as_ref());
    //     self.ignore_missing_files
    //         .merge_with(other.ignore_missing_files.as_ref());
    //     // self.filename.merge_with(other.filename.as_ref());
    //     // self.glob.merge_with(other.glob.as_ref());
    //     // self.glob_exclude.merge_with(other.glob_exclude.as_ref());
    //     self.key_path.merge_with(other.key_path.as_ref());
    //     self.include_bumps.merge_with(other.include_bumps.as_ref());
    //     self.exclude_bumps.merge_with(other.exclude_bumps.as_ref());
    // }

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

// #[deprecated]
// pub fn get_all_file_configs(
//     config: &Config,
//     // parts: &IndexMap<String, VersionComponentSpec>,
//     parts: &Parts,
// ) -> Vec<(InputFile, FileChange)> {
//     config
//         .files
//         .iter()
//         .cloned()
//         .map(|(input_file, file_config)| {
//             let global = config.global.clone();
//             let file_change = FileChange {
//                 // parse: file_config.parse.or(global.parse),
//                 // serialize: file_config.serialize.or(global.serialize),
//                 // search: file_config.search.or(global.search),
//                 // replace: file_config.replace.or(global.replace),
//                 // regex: file_config.regex.or(global.regex),
//                 parse_pattern: file_config.parse_version_pattern.unwrap_or(DEFAULT_PARSE),
//                 serialize_patterns: file_config.serialize_version_patterns,
//                 search: file_config.search,
//                 replace: file_config.replace,
//                 regex: file_config.regex,
//                 ignore_missing_version: file_config.ignore_missing_version,
//                 // .or(config.global.ignore_missing_version),
//                 ignore_missing_files: file_config.ignore_missing_files,
//                 // .or(config.global.ignore_missing_files),
//                 include_bumps: Some(parts.keys().cloned().collect()),
//                 // glob: None,
//                 // glob_exclude: None,
//                 key_path: None,
//                 exclude_bumps: None,
//                 // glob: file_config.glob.or(config.global.glob),
//                 // glob_exclude: file_config.glob.or(config.global.glob),
//                 // filename: path.as_path().map(Path::to_path_buf),
//             };
//             (input_file, file_change)
//         })
//         .collect()
// }

/// Configuration of a version component.
///
/// This is used to read in the configuration from the bumpversion config file.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionComponentSpec {
    /// Is the component independent of the other components?
    pub independent: Option<bool>,

    /// The value that is optional to include in the version.
    ///
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

/// Make sure all version components are included
pub fn version_component_configs(config: &Config) -> eyre::Result<VersionComponentConfigs> {
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
    let part_configs: VersionComponentConfigs = parsing_groups
        .into_iter()
        .map(|label| {
            let is_independent = label.starts_with("$");
            let mut spec = match config.components.get(&label) {
                Some(part) => part.clone(),
                None => VersionComponentSpec::default(),
            };
            spec.independent.merge_with(Some(&is_independent));
            (label, spec)
        })
        .collect();
    Ok(part_configs)
}

#[cfg(test)]
mod tests {
    use super::{Config, FileConfig, GlobalConfig, VersionComponentConfigs, VersionComponentSpec};
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use similar_asserts::assert_eq as sim_assert_eq;

    #[test]
    fn test_get_all_part_configs_dependent() -> eyre::Result<()> {
        crate::tests::init();
        let config = Config {
            global: GlobalConfig {
                parse_version_pattern: Some(
                    r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)".to_string(),
                ),
                ..GlobalConfig::empty()
            },
            files: vec![],
            components: [].into_iter().collect(),
        };
        let part_configs = super::version_component_configs(&config)?;
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
            global: GlobalConfig {
                parse_version_pattern: Some(
                    r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)".to_string(),
                ),
                ..GlobalConfig::empty()
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
        let part_configs = super::version_component_configs(&config)?;
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
            .collect::<VersionComponentConfigs>()
        );

        Ok(())
    }
}
