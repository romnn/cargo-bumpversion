#![forbid(unsafe_code)]
#![allow(warnings)]

mod logging;
mod options;

use bumpversion::{
    backend::{native, GitBackend},
    config,
};
use clap::Parser;
use color_eyre::eyre::{self, WrapErr};

fn main() -> eyre::Result<()> {
    if std::env::var("RUST_SPANTRACE").is_err() {
        std::env::set_var("RUST_SPANTRACE", "0");
    }

    color_eyre::install()?;

    let options = options::Options::parse();
    let color_choice = options.color_choice.unwrap_or(termcolor::ColorChoice::Auto);
    let log_level = options.log_level.or_else(|| {
        options
            .verbosity
            .log_level()
            .map(logging::ToLogLevel::to_log_level)
    });
    let (log_format, use_color) =
        logging::setup_logging(log_level, options.log_format, color_choice)?;

    let cwd = std::env::current_dir().wrap_err("could not determine current working dir")?;
    let repo = native::GitRepository::open(&cwd)?;

    let current_version = repo.latest_tag_info(None)?;
    tracing::debug!(?current_version, "current");

    // find config file
    let config_files = config::config_file_locations(&cwd);
    config_files
        .map(|config_file| {
            let path = config_file.path();
            if !config_file.path().is_file() {
                return Ok(None);
            };
            let config = std::fs::read_to_string(path)?;
            let config = match config_file {
                config::ConfigFile::BumpversionToml(path) => todo!(""),
                config::ConfigFile::SetupCfg(path) => {
                    Some(config::ini::SetupCfgINI::from_str(&config))
                }
                // config::ConfigFile::PyProject(path)=> Some(config::toml::PyProjectToml::from_str(&config)),
                // config::ConfigFile::CargoToml(path)=> Some(config::toml::CargoToml:::from_str(&config)),
                other => todo!("{other:?}"),
            };
            Ok::<Option<_>, native::Error>(config)
        })
        .filter_map(|v| v.transpose())
        .collect::<Result<Vec<_>, _>>()?;

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
