pub mod ini;
pub mod pyproject_toml;
pub mod toml;

use crate::{
    f_string::{MissingArgumentError, PythonFormatString},
    files::IoError,
};
use indexmap::IndexMap;
use std::collections::HashMap;
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

#[derive(Debug, Clone)]
pub struct Regex(pub regex::Regex);

impl std::ops::Deref for Regex {
    type Target = regex::Regex;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Ord for Regex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        std::cmp::Ord::cmp(self.0.as_str(), other.0.as_str())
    }
}

impl PartialOrd for Regex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(std::cmp::Ord::cmp(&self, &other))
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(self.0.as_str(), other.0.as_str())
    }
}

impl Eq for Regex {}

impl std::hash::Hash for Regex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl From<regex::Regex> for Regex {
    fn from(value: regex::Regex) -> Self {
        Self(value)
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum RegexTemplateError {
    #[error(transparent)]
    MissingArgument(#[from] MissingArgumentError),
    #[error(transparent)]
    Regex(#[from] regex::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegexTemplate {
    Regex(PythonFormatString),
    Escaped(PythonFormatString),
}

impl std::fmt::Display for RegexTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref().to_string())
    }
}

impl AsRef<PythonFormatString> for RegexTemplate {
    fn as_ref(&self) -> &PythonFormatString {
        match self {
            Self::Regex(s) | Self::Escaped(s) => &s,
        }
    }
}

impl RegexTemplate {
    #[must_use]
    pub fn is_regex(&self) -> bool {
        matches!(self, Self::Regex(_))
    }

    #[must_use]
    pub fn is_escaped(&self) -> bool {
        matches!(self, Self::Escaped(_))
    }

    pub fn format<K, V>(
        &self,
        values: &HashMap<K, V>,
        strict: bool,
    ) -> Result<regex::Regex, RegexTemplateError>
    where
        K: std::borrow::Borrow<str>,
        K: std::hash::Hash + Eq,
        V: AsRef<str>,
    {
        let raw_pattern = match self {
            Self::Regex(format_string) => {
                let escaped_values: HashMap<&str, String> = values
                    .iter()
                    .map(|(k, v)| (k.borrow(), regex::escape(v.as_ref())))
                    .collect();

                format_string.format(&escaped_values, strict)?
            }
            Self::Escaped(format_string) => {
                let raw_pattern = format_string.format(values, strict)?;
                regex::escape(&raw_pattern)
            }
        };
        let pattern = regex::RegexBuilder::new(&raw_pattern)
            .multi_line(true)
            .build()?;
        Ok(pattern)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalConfig {
    /// Don't abort if working directory is dirty
    pub allow_dirty: Option<bool>,
    /// Version that needs to be updated
    pub current_version: Option<String>,
    /// Regex parsing the version string
    pub parse_version_pattern: Option<Regex>,
    /// How to serialize back to a version
    pub serialize_version_patterns: Option<Vec<PythonFormatString>>,
    /// Template for complete string to search
    pub search: Option<RegexTemplate>,
    /// Template for complete string to replace
    pub replace: Option<String>,
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
    pub commit: Option<bool>,
    /// Create a tag in version control
    pub tag: Option<bool>,
    /// Sign tags if created
    pub sign_tags: Option<bool>,
    /// Tag name (only works with --tag)
    pub tag_name: Option<PythonFormatString>,
    /// Tag message
    pub tag_message: Option<PythonFormatString>,
    /// Commit message
    pub commit_message: Option<PythonFormatString>,
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
    #[must_use]
    pub fn empty() -> Self {
        Self {
            allow_dirty: None,
            current_version: None,
            parse_version_pattern: None,
            serialize_version_patterns: None,
            search: None,
            replace: None,
            // regex: None,
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
    /// Regex parsing the version string
    pub parse_version_pattern: Option<Regex>,
    /// How to serialize back to a version
    pub serialize_version_patterns: Option<Vec<PythonFormatString>>,
    /// Template for complete string to search
    pub search: Option<RegexTemplate>,
    /// Template for complete string to replace
    pub replace: Option<String>,
    /// Ignore missing file when searching and replacing version
    pub ignore_missing_file: Option<bool>,
    /// Ignore any missing version when searching and replacing version
    pub ignore_missing_version: Option<bool>,
}

impl FileConfig {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            parse_version_pattern: None,
            serialize_version_patterns: None,
            search: None,
            replace: None,
            ignore_missing_file: None,
            ignore_missing_version: None,
        }
    }
}

pub mod defaults {
    use crate::f_string::{PythonFormatString, Value};
    use once_cell::sync::Lazy;
    use regex::{Regex, RegexBuilder};

    pub const PARSE_VERSION_PATTERN: &str = r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)";
    pub static PARSE_VERSION_REGEX: Lazy<Regex> =
        Lazy::new(|| RegexBuilder::new(PARSE_VERSION_PATTERN).build().unwrap());

    pub static SERIALIZE_VERSION_PATTERNS: Lazy<Vec<PythonFormatString>> = Lazy::new(|| {
        vec![PythonFormatString(vec![
            Value::Argument("major".to_string()),
            Value::String(".".to_string()),
            Value::Argument("minor".to_string()),
            Value::String(".".to_string()),
            Value::Argument("patch".to_string()),
        ])]
    });

    pub const SEARCH: Lazy<super::RegexTemplate> = Lazy::new(|| {
        super::RegexTemplate::Escaped(
            [Value::Argument("current_version".to_string())]
                .into_iter()
                .collect(),
        )
    });
    pub const REPLACE: &str = "{new_version}";

    pub static TAG_NAME: Lazy<PythonFormatString> = Lazy::new(|| {
        [
            Value::String(String::from("v")),
            Value::Argument("new_version".to_string()),
        ]
        .into_iter()
        .collect()
    });

    pub static TAG_MESSAGE: Lazy<PythonFormatString> = Lazy::new(|| {
        PythonFormatString(vec![
            Value::String("Bump version: ".to_string()),
            Value::Argument("current_version".to_string()),
            Value::String(" → ".to_string()),
            Value::Argument("new_version".to_string()),
        ])
    });

    pub const IGNORE_MISSING_VERSION: bool = false;
    pub const IGNORE_MISSING_FILES: bool = false;
    pub const CREATE_TAG: bool = false;
    pub const SIGN_TAGS: bool = false;
    pub const ALLOW_DIRTY: bool = false;
    pub const COMMIT: bool = false;

    pub static COMMIT_MESSAGE: Lazy<PythonFormatString> = Lazy::new(|| {
        PythonFormatString(vec![
            Value::String("Bump version: ".to_string()),
            Value::Argument("current_version".to_string()),
            Value::String(" → ".to_string()),
            Value::Argument("new_version".to_string()),
        ])
    });
}

impl GlobalConfig {
    pub fn default() -> Self {
        Self {
            parse_version_pattern: Some(defaults::PARSE_VERSION_REGEX.clone().into()),
            serialize_version_patterns: Some(defaults::SERIALIZE_VERSION_PATTERNS.clone()),
            search: Some(defaults::SEARCH.clone()),
            replace: Some(defaults::REPLACE.to_string()),
            ignore_missing_version: Some(defaults::IGNORE_MISSING_VERSION),
            ignore_missing_files: Some(defaults::IGNORE_MISSING_FILES),
            tag: Some(defaults::CREATE_TAG),
            sign_tags: Some(defaults::SIGN_TAGS),
            tag_name: Some(defaults::TAG_NAME.clone()),
            tag_message: Some(defaults::TAG_MESSAGE.clone()),
            allow_dirty: Some(defaults::ALLOW_DIRTY),
            commit: Some(defaults::COMMIT),
            commit_message: Some(defaults::COMMIT_MESSAGE.clone()),
            ..GlobalConfig::empty()
        }
    }
}

impl FileConfig {
    pub fn default() -> Self {
        Self {
            parse_version_pattern: Some(defaults::PARSE_VERSION_REGEX.clone().into()),
            serialize_version_patterns: Some(defaults::SERIALIZE_VERSION_PATTERNS.clone()),
            search: Some(defaults::SEARCH.clone()),
            replace: Some(defaults::REPLACE.to_string()),
            ignore_missing_version: Some(defaults::IGNORE_MISSING_VERSION),
            ignore_missing_file: Some(defaults::IGNORE_MISSING_FILES),
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
        // self.regex.merge_with(other.regex.as_ref());
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
        // self.regex.merge_with(other.regex.as_ref());
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

pub type FileConfigs = Vec<(InputFile, FileConfig)>;
pub type VersionComponentConfigs = IndexMap<String, VersionComponentSpec>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub global: GlobalConfig,
    pub files: FileConfigs,
    pub components: VersionComponentConfigs,
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
        for (_, file_config) in &mut self.files {
            file_config.merge_with(&self.global);
        }
    }

    /// Apply defaults.
    pub fn apply_defaults(&mut self, defaults: &GlobalConfig) {
        self.global.merge_with(defaults);
        for (_, file_config) in &mut self.files {
            file_config.merge_with(defaults);
        }
    }
}

/// A change to make to a file
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileChange {
    pub parse_version_pattern: Regex,
    pub serialize_version_patterns: Vec<PythonFormatString>,
    pub search: RegexTemplate,
    pub replace: String,
    pub ignore_missing_version: bool,
    pub ignore_missing_file: bool,
    // If specified, and has an appropriate extension, will be treated as a data file
    // pub key_path: Option<String>,
    pub include_bumps: Option<Vec<String>>,
    pub exclude_bumps: Option<Vec<String>>,
}

impl FileChange {
    pub fn new(file_config: FileConfig, components: &VersionComponentConfigs) -> Self {
        Self {
            parse_version_pattern: file_config
                .parse_version_pattern
                .unwrap_or(defaults::PARSE_VERSION_REGEX.clone().into()),
            serialize_version_patterns: file_config
                .serialize_version_patterns
                .unwrap_or(defaults::SERIALIZE_VERSION_PATTERNS.clone()),
            // TODO: make this an enum that is either regex or string?
            search: file_config.search.unwrap_or(defaults::SEARCH.clone()),
            replace: file_config.replace.unwrap_or(defaults::REPLACE.to_string()),
            ignore_missing_version: file_config
                .ignore_missing_version
                .unwrap_or(defaults::IGNORE_MISSING_VERSION),
            ignore_missing_file: file_config
                .ignore_missing_file
                .unwrap_or(defaults::IGNORE_MISSING_FILES),
            include_bumps: Some(components.keys().cloned().collect()),
            // key_path: None,
            exclude_bumps: None,
        }
    }

    #[must_use]
    pub fn will_bump_component(&self, component: &str) -> bool {
        self.include_bumps
            .as_ref()
            .is_some_and(|bumps| bumps.iter().any(|c| c.as_str() == component))
    }

    #[must_use]
    pub fn will_not_bump_component(&self, component: &str) -> bool {
        self.exclude_bumps
            .as_ref()
            .is_some_and(|bumps| bumps.iter().any(|c| c.as_str() == component))
    }
}

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
    /// - `CalVer` components ignore this."""
    pub optional_value: Option<String>,

    /// The possible values for the component.
    ///
    /// If it and `calver_format` is None, the component is numeric.
    pub values: Vec<String>,

    /// The first value to increment from
    pub first_value: Option<String>,

    /// Should the component always increment, even if it is not necessary?
    pub always_increment: bool,

    /// The format string for a `CalVer` component
    pub calver_format: Option<String>,

    /// The name of the component this component depends on
    pub depends_on: Option<String>,
}

/// Make sure all version components are included
pub fn version_component_configs(config: &Config) -> VersionComponentConfigs {
    let parsing_groups: Vec<String> = match &config.global.parse_version_pattern {
        Some(parse) => parse
            .capture_names()
            .flatten()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        None => vec![],
    };
    let component_configs: VersionComponentConfigs = parsing_groups
        .into_iter()
        .map(|label| {
            let is_independent = label.starts_with('$');
            let mut spec = match config.components.get(&label) {
                Some(part) => part.clone(),
                None => VersionComponentSpec::default(),
            };
            spec.independent.merge_with(Some(&is_independent));
            (label, spec)
        })
        .collect();
    component_configs
}

#[cfg(test)]
mod tests {
    use super::{Config, GlobalConfig, VersionComponentConfigs, VersionComponentSpec};
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use similar_asserts::assert_eq as sim_assert_eq;

    // impl From<Vec<crate::f_string::Value>> for super::FormatStringOrRegex {
    //     fn from(value: Vec<crate::f_string::Value>) -> Self {
    //         Self::FormatString(super::PythonFormatString(value))
    //     }
    // }

    #[test]
    fn test_get_all_component_configs_dependent() -> eyre::Result<()> {
        crate::tests::init();
        let config = Config {
            global: GlobalConfig {
                parse_version_pattern: Some(
                    regex::Regex::new(r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)")?.into(),
                ),
                ..GlobalConfig::empty()
            },
            files: vec![],
            components: [].into_iter().collect(),
        };
        let component_configs = super::version_component_configs(&config);
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
            global: GlobalConfig {
                parse_version_pattern: Some(
                    regex::Regex::new(r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)")?.into(),
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
        let component_configs = super::version_component_configs(&config);
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
