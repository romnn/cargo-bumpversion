use clap::Parser;
use std::path::PathBuf;

pub trait Invert {
    fn invert(self) -> Self;
}

impl Invert for Option<bool> {
    fn invert(self) -> Self {
        self.map(|value| !value)
    }
}

#[derive(Parser, Debug, Clone)]
pub enum BumpCommand {
    #[clap(name = "major")]
    Major,
    #[clap(name = "minor")]
    Minor,
    #[clap(name = "patch")]
    Patch,
}

impl AsRef<str> for BumpCommand {
    fn as_ref(&self) -> &str {
        match self {
            BumpCommand::Major => "major",
            BumpCommand::Minor => "minor",
            BumpCommand::Patch => "patch",
        }
    }
}

/// Logging flags to `#[command(flatten)]` into your CLI
#[derive(clap::Args, Debug, Clone, Copy, Default)]
pub struct Verbosity {
    #[arg(
        long,
        short = 'v',
        action = clap::ArgAction::Count,
        global = true,
        help = "Increase logging verbosity",
        long_help = None,
    )]
    pub verbose: u8,

    #[arg(
        long,
        short = 'q',
        action = clap::ArgAction::Count,
        global = true,
        help = "Decrease logging verbosity",
        long_help = None,
        conflicts_with = "verbose",
    )]
    pub quiet: u8,
}

#[derive(Parser, Debug, Clone)]
#[clap(
    name = "bumpversion",
    version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown"),
    about = "bump git version",
    author = "romnn <contact@romnn.com>",
)]
pub struct Options {
    #[clap(
        long = "dir",
        help = "repository directory to run bumpversion in",
        env = "BUMPVERSION_DIR"
    )]
    pub dir: Option<PathBuf>,

    #[clap(
        long = "config-file",
        help = "config file to read most of the variables from",
        env = "BUMPVERSION_CONFIG_FILE"
    )]
    pub config_file: Option<PathBuf>,

    #[arg(
        long = "color",
        env = "BUMPVERSION_COLOR",
        help = "enable or disable color"
    )]
    pub color_choice: Option<termcolor::ColorChoice>,

    #[command(flatten)]
    pub verbosity: Verbosity,

    #[arg(
        long = "log",
        env = "BUMPVERSION_LOG_LEVEL",
        aliases = ["log-level"],
        help = "Log level. When using a more sophisticated logging setup using RUST_LOG environment variable, this option is overwritten."
    )]
    pub log_level: Option<tracing::metadata::Level>,

    #[arg(
        long = "log-format",
        env = "BUMPVERSION_LOG_FORMAT",
        help = "log format (json or pretty)"
    )]
    pub log_format: Option<crate::logging::LogFormat>,

    #[clap(
        long = "allow-dirty",
        help = "don't abort if working directory is dirty",
        env = "BUMPVERSION_ALLOW_DIRTY",
        action = clap::ArgAction::SetTrue,
    )]
    pub allow_dirty: Option<bool>,

    #[clap(
        long = "no-allow-dirty",
        help = "explicitly abort if dirty",
        env = "BUMPVERSION_NO_ALLOW_DIRTY",
        action = clap::ArgAction::SetTrue,
    )]
    pub no_allow_dirty: Option<bool>,

    #[clap(
        long = "current-version",
        help = "version that needs to be updated",
        env = "BUMPVERSION_CURRENT_VERSION"
    )]
    pub current_version: Option<String>,

    #[clap(
        long = "new-version",
        help = "new version that should be in the files",
        env = "BUMPVERSION_NEW_VERSION"
    )]
    pub new_version: Option<String>,

    #[clap(
        long = "parse",
        help = "regex parsing the version string",
        env = "BUMPVERSION_PARSE",
        // default_value = r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)"
    )]
    pub parse_pattern: Option<String>,

    #[clap(
        long = "serialize",
        help = "how to format what is parsed back to a version",
        env = "BUMPVERSION_SERIALIZE",
        // default_value = "{major}.{minor}.{patch}"
    )]
    pub serialize: Vec<String>,

    #[clap(
        long = "search",
        help = "template for complete string to search",
        env = "BUMPVERSION_SEARCH",
        // default_value = "{current_version}"
    )]
    pub search: Option<String>,

    #[clap(
        long = "replace",
        help = "template for complete string to replace",
        env = "BUMPVERSION_REPLACE",
        // default_value = "{new_version}"
    )]
    pub replace: Option<String>,

    #[clap(
        long = "regex",
        help = "treat the search parameter as a regular expression",
        env = "BUMPVERSION_REGEX"
    )]
    pub regex: Option<bool>,

    #[clap(
        long = "no-regex",
        help = "explicitly do not treat the search parameter as a regular expression",
        env = "BUMPVERSION_NO_REGEX"
    )]
    pub no_regex: Option<bool>,

    #[clap(
        long = "no-configured-files", 
        help = "only replace the version in files specified on the command line, ignoring the files from the configuration file",
        env = "BUMPVERSION_NO_CONFIGURED_FILES",
        action = clap::ArgAction::SetTrue,
    )]
    pub no_configured_files: Option<bool>,

    #[clap(
        long = "ignore-missing-files", 
        help = "ignore any missing files when searching and replacing in files",
        env = "BUMPVERSION_IGNORE_MISSING_FILES",
        action = clap::ArgAction::SetTrue,
    )]
    pub ignore_missing_files: Option<bool>,

    #[clap(
        long = "no-ignore-missing-files", 
        help = "do not allow missing files when searching and replacing in files",
        env = "BUMPVERSION_NO_IGNORE_MISSING_FILES",
        action = clap::ArgAction::SetTrue,
    )]
    pub no_ignore_missing_files: Option<bool>,

    #[clap(
        long = "ignore-missing-version", 
        help = "ignore any missing versions when searching and replacing in files",
        env = "BUMPVERSION_IGNORE_MISSING_VERSION",
        action = clap::ArgAction::SetTrue,
    )]
    pub ignore_missing_version: Option<bool>,

    #[clap(
        long = "no-ignore-missing-version", 
        help = "do not allow missing versions when searching and replacing in files",
        env = "BUMPVERSION_NO_IGNORE_MISSING_VERSION",
        action = clap::ArgAction::SetTrue,
    )]
    pub no_ignore_missing_version: Option<bool>,

    #[clap(
        short = 'n',
        long = "dry-run",
        help = "don't write any files, just pretend.",
        env = "BUMPVERSION_DRY_RUN",
        action = clap::ArgAction::SetTrue
    )]
    pub dry_run: Option<bool>,

    #[clap(
        long = "commit",
        help = "commit to version control",
        env = "BUMPVERSION_COMMIT",
        action = clap::ArgAction::SetTrue,
    )]
    pub commit: Option<bool>,

    #[clap(
        long = "no-commit",
        help = "do not commit to version control",
        env = "BUMPVERSION_NO_COMMIT",
        action = clap::ArgAction::SetTrue,
    )]
    pub no_commit: Option<bool>,

    #[clap(
        long = "tag",
        help = "create a tag in version control",
        env = "BUMPVERSION_TAG",
        action = clap::ArgAction::SetTrue,
    )]
    pub tag: Option<bool>,

    #[clap(
        long = "no-tag",
        help = "do not create a tag in version control",
        env = "BUMPVERSION_NO_TAG",
        action = clap::ArgAction::SetTrue,
    )]
    pub no_tag: Option<bool>,

    #[clap(
        long = "sign-tags",
        help = "sign tags if created",
        env = "BUMPVERSION_SIGN_TAGS",
        action = clap::ArgAction::SetTrue,
    )]
    pub sign_tags: Option<bool>,

    #[clap(
        long = "no-sign-tags",
        help = "do not sign tags if created",
        env = "BUMPVERSION_NO_SIGN_TAGS",
        action = clap::ArgAction::SetTrue,
    )]
    pub no_sign_tag: Option<bool>,

    #[clap(
        long = "tag-name",
        help = "tag name (only works with --tag)",
        env = "BUMPVERSION_TAG_NAME",
        // default_value = "v{new_version}"
    )]
    pub tag_name: Option<String>,

    #[clap(
        long = "tag-message",
        help = "tag message",
        env = "BUMPVERSION_TAG_MESSAGE",
        // default_value = "bump: {current_version} → {new_version}"
    )]
    pub tag_message: Option<String>,

    #[clap(
        short = 'm',
        long = "message",
        help = "commit message",
        env = "BUMPVERSION_MESSAGE",
        // default_value = "bump: {current_version} → {new_version}"
    )]
    pub commit_message: Option<String>,

    #[clap(
        long = "commit-args",
        help = "extra arguments to commit command",
        env = "BUMPVERSION_COMMIT_ARGS"
    )]
    pub commit_args: Option<String>,

    #[clap(subcommand)]
    pub bump: Option<BumpCommand>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}
