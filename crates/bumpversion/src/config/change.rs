use super::{file, regex::RegexTemplate};
use crate::f_string::PythonFormatString;

/// A change to make to a file
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileChange {
    pub parse_version_pattern: super::regex::Regex,
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
    #[must_use]
    pub fn new(
        file_config: file::FinalizedFileConfig,
        components: &super::VersionComponentConfigs,
    ) -> Self {
        Self {
            parse_version_pattern: file_config.parse_version_pattern,
            // .unwrap_or(defaults::PARSE_VERSION_REGEX.clone().into()),
            serialize_version_patterns: file_config.serialize_version_patterns,
            // .unwrap_or(defaults::SERIALIZE_VERSION_PATTERNS.clone()),
            // TODO: make this an enum that is either regex or string?
            search: file_config.search, // .unwrap_or(defaults::SEARCH.clone()),
            replace: file_config.replace, // .unwrap_or(defaults::REPLACE.to_string()),
            ignore_missing_version: file_config.ignore_missing_version,
            // .unwrap_or(defaults::IGNORE_MISSING_VERSION),
            ignore_missing_file: file_config.ignore_missing_file,
            // .unwrap_or(defaults::IGNORE_MISSING_FILES),
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
