#![allow(warnings)]

use anyhow::Result;
use bumpversion::{Config, GitRepository};
use clap::Parser;
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
    config_file: Option<PathBuf>,

    #[clap(
        short = 'v',
        long = "verbose",
        help = "print verbose logging",
        parse(from_occurrences)
    )]
    verbosity: u8,

    #[clap(long = "list", help = "list machine readable information", action = clap::ArgAction::SetTrue)]
    list: Option<bool>,

    #[clap(long = "allow-dirty", help = "don't abort if working directory is dirty", action = clap::ArgAction::SetTrue)]
    allow_dirty: Option<bool>,

    #[clap(long = "current-version", help = "version that needs to be updated")]
    current_version: Option<String>,

    #[clap(
        long = "parse",
        help = "regex parsing the version string",
        // default_value = r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)"
    )]
    parse: Option<String>,

    #[clap(
        long = "serialize",
        help = "how to serialize back to a version",
        // default_value = "{major}.{minor}.{patch}"
    )]
    serialize: Option<String>,

    #[clap(
        long = "search",
        help = "template for complete string to search",
        // default_value = "{current_version}"
    )]
    search: Option<String>,

    #[clap(
        long = "replace",
        help = "template for complete string to replace",
        // default_value = "{new_version}"
    )]
    replace: Option<String>,

    #[clap(long = "no-configured-files", help = "only replace the version in files specified on the command line, ignoring the files from the configuration file.", action = clap::ArgAction::SetTrue)]
    no_configured_files: bool,

    #[clap(short = 'n', long = "dry-run", help = "don't write any files, just pretend.", action = clap::ArgAction::SetTrue)]
    dry_run: Option<bool>,

    #[clap(long = "commit", help = "commit to version control", action = clap::ArgAction::SetTrue)]
    commit: Option<bool>,

    #[clap(long = "no-commit", help = "do not commit to version control", action = clap::ArgAction::SetTrue)]
    no_commit: Option<bool>,

    #[clap(long = "tag", help = "create a tag in version control", action = clap::ArgAction::SetTrue)]
    tag: Option<bool>,

    #[clap(long = "no-tag", help = "do not create a tag in version control", action = clap::ArgAction::SetTrue)]
    no_tag: Option<bool>,

    #[clap(long = "sign-tag", help = "sign tags if created", action = clap::ArgAction::SetTrue)]
    sign_tag: Option<bool>,

    #[clap(long = "no-sign-tag", help = "do not sign tags if created", action = clap::ArgAction::SetTrue)]
    no_sign_tag: Option<bool>,

    #[clap(
        long = "tag-name",
        help = "tag name (only works with --tag)",
        // default_value = "v{new_version}"
    )]
    tag_name: Option<String>,

    #[clap(
        long = "tag-message",
        help = "tag message",
        // default_value = "bump: {current_version} → {new_version}"
    )]
    tag_message: Option<String>,

    #[clap(
        short = 'm',
        long = "message",
        help = "commit message",
        // default_value = "bump: {current_version} → {new_version}"
    )]
    commit_message: Option<String>,

    #[clap(long = "commit-args", help = "extra arguments to commit command")]
    commit_args: Option<String>,

    #[clap(subcommand)]
    bump: Bump,
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let cwd = std::env::current_dir()?;
    // .ok_or(anyhow::anyhow!("could not determine current working dir"))?;
    // let repo = NativeRepo::open(cwd);
    let repo = GitRepository::native(cwd);
    // let current_version = repo.latest_tag_info();
    // println!("current version: {}", current_version);

    let config = Config::open(opts.config_file);
    // config_file = _determine_config_file(explicit_config)
    // config, config_file_exists, config_newlines, part_configs, files = _load_configuration(
    //     config_file, explicit_config, defaults,
    // )
    //
    // version_config = _setup_versionconfig(known_args, part_configs)
    // current_version = version_config.parse(known_args.current_version)

    // # calculate the desired new version
    // new_version = _assemble_new_version(
    //     context, current_version, defaults, known_args.current_version, positionals, version_config
    // )

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
    Ok(())
}
