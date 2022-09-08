#![allow(warnings)]

use clap::Parser;
// use git::objs::tree;
// use git_repository as git;
use std::path::{Path, PathBuf};

// #[derive(thiserror::Error, Debug)]
// pub enum Error {
//     #[error("image error: {0}")]
//     Git(#[from] String),
//     // Git(#[from] git::Error),
//     // #[error("border error: {0}")]
//     // Border(#[from] BorderError),

//     // #[error("io error: {0}")]
//     // Io(#[from] std::io::Error),
// }

#[derive(Parser, Debug, Clone)]
struct ApplyOpts {
    #[clap(short = 'i', long = "image")]
    images: Vec<PathBuf>,

    #[clap(short = 'o', long = "output")]
    output: Option<PathBuf>,

    #[clap(short = 'b', long = "border")]
    border: Option<String>,

    #[clap(long = "width")]
    output_width: Option<u32>,

    #[clap(long = "height")]
    output_height: Option<u32>,
}

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
    config_file: Option<PathBuf>,

    #[clap(
        short = 'v',
        long = "verbose",
        help = "print verbose loggin",
        parse(from_occurrences)
    )]
    verbosity: u8,

    #[clap(long = "list", help = "list machine readable information", action = clap::ArgAction::SetTrue)]
    list: bool,

    #[clap(long = "allow-dirty", help = "don't abort if working directory is dirty", action = clap::ArgAction::SetTrue)]
    allow_dirty: bool,

    #[clap(long = "current-version", help = "version that needs to be updated")]
    current_version: Option<String>,

    #[clap(
        long = "parse",
        help = "regex parsing the version string",
        default_value = r"(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)"
    )]
    parse: String,

    #[clap(
        long = "serialize",
        help = "how to serialize back to a version",
        default_value = "{major}.{minor}.{patch}"
    )]
    serialize: String,

    #[clap(
        long = "search",
        help = "template for complete string to search",
        default_value = "{current_version}"
    )]
    search: String,

    #[clap(
        long = "replace",
        help = "template for complete string to replace",
        default_value = "{new_version}"
    )]
    replace: String,

    #[clap(long = "no-configured-files", help = "only replace the version in files specified on the command line, ignoring the files from the configuration file.", action = clap::ArgAction::SetTrue)]
    no_configured_files: bool,

    #[clap(short = 'n', long = "dry-run", help = "don't write any files, just pretend.", action = clap::ArgAction::SetTrue)]
    dry_run: bool,

    #[clap(long = "commit", help = "commit to version control", action = clap::ArgAction::SetTrue)]
    commit: bool,

    #[clap(long = "no-commit", help = "do not commit to version control", action = clap::ArgAction::SetTrue)]
    no_commit: bool,

    #[clap(long = "tag", help = "create a tag in version control", action = clap::ArgAction::SetTrue)]
    tag: bool,

    #[clap(long = "no-tag", help = "do not create a tag in version control", action = clap::ArgAction::SetTrue)]
    no_tag: bool,

    #[clap(long = "sign-tag", help = "sign tags if created", action = clap::ArgAction::SetTrue)]
    sign_tag: bool,

    #[clap(long = "no-sign-tag", help = "do not sign tags if created", action = clap::ArgAction::SetTrue)]
    no_sign_tag: bool,

    #[clap(
        long = "tag-name",
        help = "tag name (only works with --tag)",
        default_value = "v{new_version}"
    )]
    tag_name: String,

    #[clap(
        long = "tag-message",
        help = "tag message",
        default_value = "bump: {current_version} → {new_version}"
    )]
    tag_message: String,

    #[clap(
        short = 'm',
        long = "message",
        help = "commit message",
        default_value = "bump: {current_version} → {new_version}"
    )]
    commit_message: String,

    #[clap(long = "commit-args", help = "extra arguments to commit command")]
    commit_args: Option<String>,

    #[clap(subcommand)]
    bump: Bump,
}

fn main() {
    let opts: Opts = Opts::parse();

    // if not os.path.exists(".bumpversion.cfg") and os.path.exists("setup.cfg"):
    //     return "setup.cfg"
    // return ".bumpversion.cfg"

    // if let Some(subcommand) = opts.commands {
    // match subcommand.bump {
    //     Bump::Major => {}
    //     Bump::Major => {}
    //     Bump::Major => {}
    // }
    // }
}
