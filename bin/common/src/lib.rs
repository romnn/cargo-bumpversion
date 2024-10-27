#![allow(warnings)]

// use bumpversion::{Config, GitRepository};
use clap::Parser;
use color_eyre::eyre;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
enum Bump {
    #[clap(name = "major")]
    Major,
    #[clap(name = "minor")]
    Minor,
    #[clap(name = "patch")]
    Patch,
}

#[derive(Parser, Debug, Clone)]
#[clap(
    name = "bumpversion",
    version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown"),
    about = "bump git version",
    author = "romnn <contact@romnn.com>",
)]
pub struct Opts {
    #[clap(
        long = "config-file",
        help = "Config file to read most of the variables from (default: .bumpversion.cfg)"
    )]
    pub config_file: Option<PathBuf>,

    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    #[clap(long = "list", help = "list machine readable information", action = clap::ArgAction::SetTrue)]
    pub list: Option<bool>,

    #[clap(long = "allow-dirty", help = "don't abort if working directory is dirty", action = clap::ArgAction::SetTrue)]
    pub allow_dirty: Option<bool>,

    #[clap(long = "current-version", help = "version that needs to be updated")]
    pub current_version: Option<String>,

    #[clap(
        long = "parse",
        help = "regex parsing the version string",
        // default_value = r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)"
    )]
    pub parse: Option<String>,

    #[clap(
        long = "serialize",
        help = "how to serialize back to a version",
        // default_value = "{major}.{minor}.{patch}"
    )]
    pub serialize: Option<String>,

    #[clap(
        long = "search",
        help = "template for complete string to search",
        // default_value = "{current_version}"
    )]
    pub search: Option<String>,

    #[clap(
        long = "replace",
        help = "template for complete string to replace",
        // default_value = "{new_version}"
    )]
    pub replace: Option<String>,

    #[clap(long = "no-configured-files", help = "only replace the version in files specified on the command line, ignoring the files from the configuration file.", action = clap::ArgAction::SetTrue)]
    pub no_configured_files: bool,

    #[clap(short = 'n', long = "dry-run", help = "don't write any files, just pretend.", action = clap::ArgAction::SetTrue)]
    pub dry_run: Option<bool>,

    #[clap(long = "commit", help = "commit to version control", action = clap::ArgAction::SetTrue)]
    pub commit: Option<bool>,

    #[clap(long = "no-commit", help = "do not commit to version control", action = clap::ArgAction::SetTrue)]
    pub no_commit: Option<bool>,

    #[clap(long = "tag", help = "create a tag in version control", action = clap::ArgAction::SetTrue)]
    pub tag: Option<bool>,

    #[clap(long = "no-tag", help = "do not create a tag in version control", action = clap::ArgAction::SetTrue)]
    pub no_tag: Option<bool>,

    #[clap(long = "sign-tag", help = "sign tags if created", action = clap::ArgAction::SetTrue)]
    pub sign_tag: Option<bool>,

    #[clap(long = "no-sign-tag", help = "do not sign tags if created", action = clap::ArgAction::SetTrue)]
    pub no_sign_tag: Option<bool>,

    #[clap(
        long = "tag-name",
        help = "tag name (only works with --tag)",
        // default_value = "v{new_version}"
    )]
    pub tag_name: Option<String>,

    #[clap(
        long = "tag-message",
        help = "tag message",
        // default_value = "bump: {current_version} → {new_version}"
    )]
    pub tag_message: Option<String>,

    #[clap(
        short = 'm',
        long = "message",
        help = "commit message",
        // default_value = "bump: {current_version} → {new_version}"
    )]
    pub commit_message: Option<String>,

    #[clap(long = "commit-args", help = "extra arguments to commit command")]
    pub commit_args: Option<String>,

    #[clap(subcommand)]
    pub bump: Bump,
}
