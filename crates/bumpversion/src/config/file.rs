use super::{
    global,
    regex::{Regex, RegexTemplate},
};
use crate::f_string::PythonFormatString;

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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FinalizedFileConfig {
    /// Regex parsing the version string
    pub parse_version_pattern: Regex,
    /// How to serialize back to a version
    pub serialize_version_patterns: Vec<PythonFormatString>,
    /// Template for complete string to search
    pub search: RegexTemplate,
    /// Template for complete string to replace
    pub replace: String,
    /// Ignore missing file when searching and replacing version
    pub ignore_missing_file: bool,
    /// Ignore any missing version when searching and replacing version
    pub ignore_missing_version: bool,
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

pub static PARSE_VERSION_REGEX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
    regex::RegexBuilder::new(r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)")
        .build()
        .unwrap()
        .into()
});

impl Default for FinalizedFileConfig {
    fn default() -> Self {
        use crate::f_string::Value;
        let search = super::regex::RegexTemplate::Escaped(
            [Value::Argument("current_version".to_string())]
                .into_iter()
                .collect(),
        );
        let serialize_version_patterns = vec![PythonFormatString(vec![
            Value::Argument("major".to_string()),
            Value::String(".".to_string()),
            Value::Argument("minor".to_string()),
            Value::String(".".to_string()),
            Value::Argument("patch".to_string()),
        ])];

        Self {
            parse_version_pattern: PARSE_VERSION_REGEX.clone(),
            serialize_version_patterns,
            search,
            replace: "{new_version}".to_string(),
            ignore_missing_version: false,
            ignore_missing_file: false,
        }
    }
}

impl Default for FileConfig {
    fn default() -> Self {
        let default = FinalizedFileConfig::default();
        Self {
            parse_version_pattern: Some(default.parse_version_pattern),
            serialize_version_patterns: Some(default.serialize_version_patterns),
            search: Some(default.search),
            replace: Some(default.replace),
            ignore_missing_version: Some(default.ignore_missing_version),
            ignore_missing_file: Some(default.ignore_missing_file),
        }
    }
}

impl FileConfig {
    /// Finalize the file config.
    ///
    /// All unset configuration options will be set to their default value.
    #[must_use] pub fn finalize(self) -> FinalizedFileConfig {
        let default = FinalizedFileConfig::default();
        FinalizedFileConfig {
            parse_version_pattern: self
                .parse_version_pattern
                .unwrap_or(default.parse_version_pattern),
            serialize_version_patterns: self
                .serialize_version_patterns
                .unwrap_or(default.serialize_version_patterns),
            search: self.search.unwrap_or(default.search),
            replace: self.replace.unwrap_or(default.replace),
            ignore_missing_version: self
                .ignore_missing_version
                .unwrap_or(default.ignore_missing_version),
            ignore_missing_file: self
                .ignore_missing_file
                .unwrap_or(default.ignore_missing_file),
        }
    }
}

impl<'a> super::MergeWith<&'a global::GlobalConfig> for FileConfig {
    fn merge_with(&mut self, other: &'a global::GlobalConfig) {
        self.parse_version_pattern
            .merge_with(other.parse_version_pattern.as_ref());
        self.serialize_version_patterns
            .merge_with(other.serialize_version_patterns.as_ref());
        self.search.merge_with(other.search.as_ref());
        self.replace.merge_with(other.replace.as_ref());
        self.ignore_missing_file
            .merge_with(other.ignore_missing_files.as_ref());
        self.ignore_missing_version
            .merge_with(other.ignore_missing_version.as_ref());
    }
}
