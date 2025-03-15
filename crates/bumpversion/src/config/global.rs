use super::regex::{Regex, RegexTemplate};
use crate::f_string::PythonFormatString;
use std::path::PathBuf;

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
    /// Additional files
    ///
    /// This is useful for files such as lockfiles, which should be regenerated after the version
    /// bump in a pre-commit hook.
    pub additional_files: Option<Vec<PathBuf>>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalConfigFinalized {
    /// Don't abort if working directory is dirty
    pub allow_dirty: bool,
    /// Version that needs to be updated
    pub current_version: Option<String>,
    /// Regex parsing the version string
    pub parse_version_pattern: Regex,
    /// How to serialize back to a version
    pub serialize_version_patterns: Vec<PythonFormatString>,
    /// Template for complete string to search
    pub search: RegexTemplate,
    /// Template for complete string to replace
    pub replace: String,
    /// Only replace the version in files specified on the command line.
    ///
    /// When enabled, the files from the configuration file are ignored
    pub no_configured_files: bool,
    /// Ignore any missing files when searching and replacing in files
    pub ignore_missing_files: bool,
    /// Ignore any missing version when searching and replacing in files
    pub ignore_missing_version: bool,
    /// Don't write any files, just pretend
    pub dry_run: bool,
    /// Commit to version control
    pub commit: bool,
    /// Create a tag in version control
    pub tag: bool,
    /// Sign tags if created
    pub sign_tags: bool,
    /// Tag name (only works with --tag)
    pub tag_name: PythonFormatString,
    /// Tag message
    pub tag_message: PythonFormatString,
    /// Commit message
    pub commit_message: PythonFormatString,
    /// Extra arguments to commit command
    pub commit_args: Option<String>,

    // extra stuff
    /// Setup hooks
    pub setup_hooks: Vec<String>,
    /// Pre-commit hooks
    pub pre_commit_hooks: Vec<String>,
    /// Post-commit hooks
    pub post_commit_hooks: Vec<String>,
    /// Included paths
    pub included_paths: Option<Vec<PathBuf>>,
    /// Excluded paths
    pub excluded_paths: Option<Vec<PathBuf>>,
    /// Additional files to add.
    ///
    /// This is useful for files such as lockfiles, which should be regenerated after the version
    /// bump in a pre-commit hook.
    pub additional_files: Option<Vec<PathBuf>>,
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
            additional_files: None,
        }
    }
}

impl Default for GlobalConfigFinalized {
    fn default() -> Self {
        use crate::f_string::Value;
        let file_config = super::file::FinalizedFileConfig::default();
        let tag_name = [
            Value::String(String::from("v")),
            Value::Argument("new_version".to_string()),
        ]
        .into_iter()
        .collect();

        let tag_message = PythonFormatString(vec![
            Value::String("Bump version: ".to_string()),
            Value::Argument("current_version".to_string()),
            Value::String(" → ".to_string()),
            Value::Argument("new_version".to_string()),
        ]);
        let commit_message = PythonFormatString(vec![
            Value::String("Bump version: ".to_string()),
            Value::Argument("current_version".to_string()),
            Value::String(" → ".to_string()),
            Value::Argument("new_version".to_string()),
        ]);
        Self {
            allow_dirty: false,
            current_version: None,
            parse_version_pattern: file_config.parse_version_pattern,
            serialize_version_patterns: file_config.serialize_version_patterns,
            search: file_config.search,
            replace: file_config.replace,
            no_configured_files: false,
            ignore_missing_version: file_config.ignore_missing_version,
            ignore_missing_files: file_config.ignore_missing_file,
            dry_run: false,
            commit: false,
            tag: false,
            sign_tags: false,
            tag_name,
            tag_message,
            commit_message,
            commit_args: None,
            setup_hooks: vec![],
            pre_commit_hooks: vec![],
            post_commit_hooks: vec![],
            included_paths: None,
            excluded_paths: None,
            additional_files: None,
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        let default = GlobalConfigFinalized::default();
        Self {
            allow_dirty: Some(default.allow_dirty),
            current_version: default.current_version,
            parse_version_pattern: Some(default.parse_version_pattern),
            serialize_version_patterns: Some(default.serialize_version_patterns),
            search: Some(default.search),
            replace: Some(default.replace),
            no_configured_files: Some(default.no_configured_files),
            ignore_missing_files: Some(default.ignore_missing_files),
            ignore_missing_version: Some(default.ignore_missing_version),
            dry_run: Some(default.dry_run),
            commit: Some(default.commit),
            tag: Some(default.tag),
            sign_tags: Some(default.sign_tags),
            tag_name: Some(default.tag_name),
            tag_message: Some(default.tag_message),
            commit_message: Some(default.commit_message),
            commit_args: default.commit_args,
            setup_hooks: Some(default.setup_hooks),
            pre_commit_hooks: Some(default.pre_commit_hooks),
            post_commit_hooks: Some(default.post_commit_hooks),
            included_paths: default.included_paths,
            excluded_paths: default.excluded_paths,
            additional_files: default.additional_files,
        }
    }
}

impl GlobalConfig {
    /// Finalize the global config.
    ///
    /// All unset configuration options will be set to their default value.
    #[must_use] pub fn finalize(self) -> GlobalConfigFinalized {
        let default = GlobalConfigFinalized::default();
        GlobalConfigFinalized {
            allow_dirty: self.allow_dirty.unwrap_or(default.allow_dirty),
            current_version: self.current_version.or(default.current_version),
            parse_version_pattern: self
                .parse_version_pattern
                .unwrap_or(default.parse_version_pattern),
            serialize_version_patterns: self
                .serialize_version_patterns
                .unwrap_or(default.serialize_version_patterns),
            search: self.search.unwrap_or(default.search),
            replace: self.replace.unwrap_or(default.replace),
            no_configured_files: self
                .no_configured_files
                .unwrap_or(default.no_configured_files),
            ignore_missing_files: self
                .ignore_missing_files
                .unwrap_or(default.ignore_missing_files),
            ignore_missing_version: self
                .ignore_missing_version
                .unwrap_or(default.ignore_missing_version),
            dry_run: self.dry_run.unwrap_or(default.dry_run),
            commit: self.commit.unwrap_or(default.commit),
            tag: self.tag.unwrap_or(default.tag),
            sign_tags: self.sign_tags.unwrap_or(default.sign_tags),
            tag_name: self.tag_name.unwrap_or(default.tag_name),
            tag_message: self.tag_message.unwrap_or(default.tag_message),
            commit_message: self.commit_message.unwrap_or(default.commit_message),
            commit_args: self.commit_args.or(default.commit_args),
            setup_hooks: self.setup_hooks.unwrap_or(default.setup_hooks),
            pre_commit_hooks: self.pre_commit_hooks.unwrap_or(default.pre_commit_hooks),
            post_commit_hooks: self.post_commit_hooks.unwrap_or(default.post_commit_hooks),
            included_paths: self.included_paths.or(default.included_paths),
            excluded_paths: self.excluded_paths.or(default.excluded_paths),
            additional_files: self.additional_files.or(default.additional_files),
        }
    }
}

impl<'a> super::MergeWith<&'a GlobalConfig> for GlobalConfig {
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
        self.additional_files
            .merge_with(other.additional_files.as_ref());
    }
}
