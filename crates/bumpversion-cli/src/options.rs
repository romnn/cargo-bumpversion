use bumpversion::config;
use clap::Parser;
use color_eyre::eyre;
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
        env = "BUMPVERSION_PARSE"
    )]
    pub parse_version_pattern: Option<String>,

    #[clap(
        long = "serialize",
        help = "how to format what is parsed back to a version",
        env = "BUMPVERSION_SERIALIZE"
    )]
    pub serialize_version_patterns: Option<Vec<String>>,

    #[clap(
        long = "search",
        help = "template for complete string to search",
        env = "BUMPVERSION_SEARCH"
    )]
    pub search: Option<String>,

    #[clap(
        long = "replace",
        help = "template for complete string to replace",
        env = "BUMPVERSION_REPLACE"
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
        env = "BUMPVERSION_TAG_NAME"
    )]
    pub tag_name: Option<String>,

    #[clap(
        long = "tag-message",
        help = "tag message",
        env = "BUMPVERSION_TAG_MESSAGE"
    )]
    pub tag_message: Option<String>,

    #[clap(
        short = 'm',
        long = "message",
        help = "commit message",
        env = "BUMPVERSION_MESSAGE"
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

pub fn fix(options: &mut Options) {
    // HACK(roman):
    //
    // For some reason, clap v4 may set `options.allow_dirty = Some(false)` when using
    // `clap::ArgAction::SetTrue` and the flag is not specified.
    //
    // It's fine to check for these cases, since `clap::ArgAction::SetTrue` does not allow
    // users to set `--allow-dirty=false`.
    for boolean_option in [
        &mut options.allow_dirty,
        &mut options.no_allow_dirty,
        &mut options.regex,
        &mut options.no_regex,
        &mut options.no_configured_files,
        &mut options.ignore_missing_files,
        &mut options.no_ignore_missing_files,
        &mut options.ignore_missing_version,
        &mut options.no_ignore_missing_version,
        &mut options.dry_run,
        &mut options.commit,
        &mut options.no_commit,
        &mut options.tag,
        &mut options.no_tag,
        &mut options.sign_tags,
        &mut options.no_sign_tag,
    ] {
        if *boolean_option != Some(true) {
            *boolean_option = None;
        }
    }
}

pub fn parse_positional_arguments(
    options: &mut Options,
    components: &config::VersionComponentConfigs,
) -> eyre::Result<(Option<String>, Vec<PathBuf>)> {
    let mut cli_files = vec![];
    let mut bump: Option<String> = options
        .bump
        .as_ref()
        .map(AsRef::as_ref)
        .map(ToString::to_string);

    // first, check for invalid flags
    for arg in &options.args {
        if arg.starts_with("--") {
            eyre::bail!("unknown flag {arg:?}");
        }
    }

    if !options.args.is_empty() {
        if options.bump.is_none() {
            // first argument must be version component to bump
            let component = options.args.remove(0);
            if components.contains_key(&component) {
                bump = Some(component);

                // remaining arguments are files
                cli_files.extend(options.args.drain(..).map(PathBuf::from));
            } else {
                eyre::bail!(
                    "first argument must be one of the version components {:?}",
                    components.keys().collect::<Vec<_>>()
                )
            }
        } else {
            // assume all arguments are files to run on
            cli_files.extend(options.args.drain(..).map(PathBuf::from));
        }
    }
    Ok((bump, cli_files))
}

pub fn global_cli_config(options: &Options) -> eyre::Result<bumpversion::config::GlobalConfig> {
    let search_as_regex = options
        .allow_dirty
        .or(options.no_allow_dirty.invert())
        .unwrap_or(false);

    let search = options
        .search
        .as_ref()
        .map(|search| {
            let format_string = bumpversion::f_string::PythonFormatString::parse(search)?;
            let search = if search_as_regex {
                bumpversion::config::RegexTemplate::Regex(format_string)
            } else {
                bumpversion::config::RegexTemplate::Escaped(format_string)
            };
            Ok::<_, eyre::Report>(search)
        })
        .transpose()?;

    let parse_version_pattern = options
        .parse_version_pattern
        .as_deref()
        .map(bumpversion::config::Regex::try_from)
        .transpose()?;

    let serialize_version_patterns = options
        .serialize_version_patterns
        .as_ref()
        .map(|patterns| {
            patterns
                .iter()
                .map(String::as_str)
                .map(bumpversion::f_string::PythonFormatString::parse)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    let tag_name = options
        .tag_name
        .as_deref()
        .map(bumpversion::f_string::PythonFormatString::parse)
        .transpose()?;

    let tag_message = options
        .tag_name
        .as_deref()
        .map(bumpversion::f_string::PythonFormatString::parse)
        .transpose()?;

    let commit_message = options
        .commit_message
        .as_deref()
        .map(bumpversion::f_string::PythonFormatString::parse)
        .transpose()?;

    let cli_overrides = bumpversion::config::GlobalConfig {
        allow_dirty: options.allow_dirty.or(options.no_allow_dirty.invert()),
        current_version: options.current_version.clone(),
        parse_version_pattern,
        serialize_version_patterns,
        search,
        replace: options.replace.clone(),
        no_configured_files: options.no_configured_files,
        ignore_missing_files: options
            .ignore_missing_files
            .or(options.no_ignore_missing_files.invert()),
        ignore_missing_version: options
            .ignore_missing_version
            .or(options.no_ignore_missing_version.invert()),
        dry_run: options.dry_run,
        commit: options.commit.or(options.no_commit.invert()),
        tag: options.tag.or(options.no_tag.invert()),
        sign_tags: options.sign_tags.or(options.no_sign_tag.invert()),
        tag_name,
        tag_message,
        commit_message,
        commit_args: options.commit_args.clone(),
        ..bumpversion::config::GlobalConfig::empty()
    };
    Ok(cli_overrides)
}
