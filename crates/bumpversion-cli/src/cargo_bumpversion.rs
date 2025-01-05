#![forbid(unsafe_code)]

mod logging;
mod options;

use bumpversion::{Config, GitRepository};
use clap::Parser;
use color_eyre::eyre;
use options::Opts;

fn main() -> eyre::Result<()> {
    let opts: Opts = Opts::parse();
    let cwd = std::env::current_dir()?;
    // .ok_or(anyhow::anyhow!("could not determine current working dir"))?;
    // let repo = NativeRepo::open(cwd);
    let repo = GitRepository::native(cwd);
    // let current_version = repo.latest_tag_info();
    // println!("current version: {}", current_version);

    // let config = Config::open(opts.config_file);
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
