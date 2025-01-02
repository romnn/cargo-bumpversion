#![forbid(unsafe_code)]
#![allow(warnings)]

mod logging;
mod options;

use bumpversion::{
    backend::{native, TagAndRevision, VersionControlSystem},
    config, context,
    diagnostics::{Printer, ToDiagnostics},
    files::FileMap,
    hooks, Bump,
};
use clap::Parser;
use color_eyre::eyre::{self, WrapErr};
use options::Invert;
use std::path::{Path, PathBuf};

fn fix_options(options: &mut options::Options) {
    // HACK(roman):
    //
    // For some reason, clap v4 may set `options.allow_dirty = Some(false)` when using
    // `clap::ArgAction::SetTrue` and the flag is not specified.
    //
    // It's fine to check for these cases, since `clap::ArgAction::SetTrue` does not allow
    // users to set `--allow-dirty=false`.
    if options.allow_dirty != Some(true) {
        options.allow_dirty = None;
    }
    if options.no_allow_dirty != Some(true) {
        options.no_allow_dirty = None;
    }
}

fn main() -> eyre::Result<()> {
    if std::env::var("RUST_SPANTRACE").is_err() {
        std::env::set_var("RUST_SPANTRACE", "0");
    }

    let start = std::time::Instant::now();
    color_eyre::install()?;

    let mut options = options::Options::parse();
    fix_options(&mut options);
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
    let dir = options.dir.unwrap_or(cwd).canonicalize()?;
    let repo = native::GitRepository::open(&dir)?;

    // find config file
    let config_files = config::config_file_locations(&dir);
    // dbg!(config::config_file_locations(&dir).collect::<Vec<_>>());

    let mut config_files = config_files
        .map(|config_file| {
            let path = config_file.path();
            if !path.is_file() {
                return Ok(None);
            };
            let Ok(path) = path.canonicalize() else {
                return Ok(None);
            };
            let config = std::fs::read_to_string(&path)?;

            let mut diagnostics = vec![];
            let printer = Printer::stderr(color_choice);
            let file_id = printer.add_source_file(&path, config.to_string());
            let strict = true;

            let config_res = match &config_file {
                config::ConfigFile::BumpversionToml(path) | config::ConfigFile::PyProject(path) => {
                    let res = config::Config::from_pyproject_toml(
                        &config,
                        file_id,
                        strict,
                        &mut diagnostics,
                    );
                    if let Err(ref err) = res {
                        diagnostics.extend(err.to_diagnostics(file_id));
                    }
                    res.map_err(eyre::Report::from)
                }
                config::ConfigFile::BumpversionCfg(path) => {
                    let options = config::ini::Options::default();
                    let res = config::Config::from_ini(
                        &config,
                        options,
                        file_id,
                        strict,
                        &mut diagnostics,
                    );
                    if let Err(ref err) = res {
                        diagnostics.extend(err.to_diagnostics(file_id));
                    }
                    res.map_err(eyre::Report::from)
                }
                config::ConfigFile::SetupCfg(path) => {
                    let options = config::ini::Options::default();
                    let res = config::Config::from_setup_cfg_ini(
                        &config,
                        options,
                        file_id,
                        strict,
                        &mut diagnostics,
                    );
                    if let Err(ref err) = res {
                        diagnostics.extend(err.to_diagnostics(file_id));
                    }
                    res.map_err(eyre::Report::from)
                }
                config::ConfigFile::CargoToml(_) => {
                    // TODO: cargo
                    Ok(None)
                }
            };
            // emit diagnostics
            for diagnostic in diagnostics.iter() {
                printer.emit(diagnostic);
            }
            config_res.map(|c| c.map(|c| (path.to_path_buf(), c)))
            // let config = config_res?;
            // Ok::<Option<config::Config>, eyre::Report>(config)
        })
        .filter_map(|v| v.transpose());

    let config_files: Vec<_> = config_files.collect();
    dbg!(&config_files);

    let (config_file_path, mut config) = config_files
        .into_iter()
        .next()
        .transpose()?
        .ok_or(eyre::eyre!("missing config file"))?;

    // the order is important here
    config.merge_global_config();

    let defaults = config::FileConfig::default();
    config.apply_defaults(&defaults);

    // build list of parts
    let parts = config::get_all_part_configs(&config)?;
    // dbg!(&parts);

    let mut cli_files = vec![];
    let mut bump = options.bump.take().map(Bump::from);
    if !options.args.is_empty() {
        if options.bump.is_none() {
            // first argument must be version component to bump
            let component = options.args.remove(0);
            if parts.contains_key(&component) {
                bump = Some(Bump::Other(component.to_string()));

                // remaining arguments are files
                cli_files.extend(options.args.drain(..).map(PathBuf::from));
            } else {
                eyre::bail!(
                    "first argument must be one of the version components {:?}",
                    parts.keys().collect::<Vec<_>>()
                )
            }
        } else {
            // assume all arguments are files to run on
            cli_files.extend(options.args.drain(..).map(PathBuf::from));
        }
    }

    let bump = bump.ok_or_else(|| eyre::eyre!("missing version component to bump"))?;
    dbg!(&bump);
    dbg!(&cli_files);

    let TagAndRevision { tag, revision } = repo.latest_tag_and_revision(
        config
            .global
            .tag_name
            .as_deref()
            .unwrap_or(config::DEFAULT_TAG_NAME),
        config
            .global
            .parse_version_pattern
            .as_deref()
            .unwrap_or(config::DEFAULT_PARSE_VERSION_PATTERN),
    )?;
    tracing::debug!(?tag, "current");
    tracing::debug!(?revision, "current");

    dbg!(
        &options.allow_dirty,
        &options.no_allow_dirty.invert(),
        &config.global.allow_dirty
    );
    let allow_dirty = options
        .allow_dirty
        .or(options.no_allow_dirty.invert())
        .or(config.global.allow_dirty)
        .unwrap_or(false);
    dbg!(allow_dirty);

    let dry_run = options.dry_run.or(config.global.dry_run).unwrap_or(false);
    dbg!(dry_run);

    let configured_version = options
        .current_version
        .as_ref()
        .or(config.global.current_version.as_ref())
        .cloned();
    let actual_version = tag.as_ref().map(|tag| &tag.current_version).cloned();

    // if both versions are present, they should match
    match (&configured_version, &actual_version) {
        (Some(configured_version), Some(actual_version))
            if configured_version != actual_version =>
        {
            tracing::warn!(
                "Specified version ({configured_version}) does not match last tagged version ({actual_version})",
            );
        }
        _ => {}
    };
    let current_version: String = configured_version
        .or(actual_version)
        .ok_or(eyre::eyre!("Unable to determine the current version."))?;
    // dbg!(&current_version);

    let dirty_files = repo.dirty_files()?;
    // dbg!(&dirty_files);
    if !allow_dirty && !dirty_files.is_empty() {
        eyre::bail!(
            "Working directory is not clean:\n\n{}",
            dirty_files
                .iter()
                .map(|file| file.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    // let files = config::get_all_file_configs(&config, &parts);
    // dbg!(&files);

    // build resolved file map
    let file_map = bumpversion::files::resolve_files_from_config(&mut config, &parts)?;
    dbg!(&file_map);

    if options.no_configured_files == Some(true) {
        config.global.excluded_paths = Some(file_map.keys().cloned().collect());
    }

    if !cli_files.is_empty() {
        // file_map.extend(cli_files);
        // config.add_files(files);
        config.global.included_paths = Some(cli_files);
    }

    do_bump(
        &bump,
        &dir,
        options.new_version.as_deref(),
        &config,
        &TagAndRevision { tag, revision },
        // &files,
        &file_map,
        &parts,
        Some(config_file_path.as_path()),
        dry_run,
    )?;

    tracing::info!(elapsed = ?start.elapsed(), "done");
    Ok(())
}

/// Bump the version_part to the next value or set the version to new_version.
fn do_bump(
    version_component_to_bump: &Bump,
    working_dir: &Path,
    new_version: Option<&str>,
    config: &config::Config,
    tag_and_revision: &TagAndRevision,
    // parts: &indexmap::IndexMap<String, String>,
    // parts: &indexmap::IndexMap<String, String>,
    // files: &config::Files,
    // files: &Vec<(config::InputFile, config::FileChange)>,
    // file_map: &FileMap<'_>,
    file_map: &FileMap,
    parts: &config::Parts,
    config_file: Option<&Path>,
    dry_run: bool,
) -> eyre::Result<()> {
    let current_version = config
        .global
        .current_version
        .as_ref()
        .ok_or_else(|| eyre::eyre!("missing current version"))?;

    tracing::debug!(version = current_version, "parsing current version");

    let version_config =
        bumpversion::version::compat::VersionConfig::from_config(&config.global, &parts.clone())?;

    let version = version_config.parse(&*current_version)?;
    let version = version.ok_or_else(|| eyre::eyre!("empty version"))?;
    dbg!(&version);
    hooks::run_setup_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&version),
        dry_run,
    )?;

    use std::collections::HashMap;
    let ctx: HashMap<String, String> = context::get_context(config, None, None, None).collect();

    let next_version = bumpversion::version::get_next_version(
        &version,
        &version_config,
        // config,
        version_component_to_bump,
        new_version,
    )?;
    // TODO: why can the version be none?
    let next_version = next_version.ok_or_else(|| eyre::eyre!("next version is None"))?;
    dbg!(&next_version);
    let next_version_serialized = version_config.serialize(
        &next_version,
        // ctx.iter().map()move |(k, v)| (k.as_str(), v.as_str())),
        ctx.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    )?;
    tracing::info!(version = next_version_serialized, "next version");

    if current_version == &next_version_serialized {
        tracing::info!(
            version = next_version_serialized,
            "next version matches current version"
        );
        return Ok(());
    }

    // if dry_run {
    //     tracing::info!("dry run active, won't touch any files.");
    // }

    let mut files_to_modify: Vec<(PathBuf, &config::FileChange)> =
        bumpversion::files::files_to_modify(&config, &file_map)
            .flat_map(|(file, configs)| {
                configs
                    .into_iter()
                    // .copied()
                    .map(|config| (file.clone(), config))
            })
            .collect();
    files_to_modify.sort();

    dbg!(&files_to_modify);
    let mut configured_files =
        bumpversion::files::resolve(&files_to_modify, &version_config, None, None);

    // filter the files that are not valid for this bump
    configured_files.retain(|file| {
        file.file_change
            .will_bump_component(version_component_to_bump.as_str())
    });
    configured_files.retain(|file| {
        !file
            .file_change
            .will_not_bump_component(version_component_to_bump.as_str())
    });
    dbg!(&configured_files);

    // let ctx: bumpversion::context::Env = bumpversion::context::get_context(
    let ctx = context::get_context(
        config,
        Some(tag_and_revision),
        Some(&version),
        Some(&next_version),
    );
    // .collect();
    // dbg!(ctx);

    // modify_files(configured_files, version, next_version, ctx, dry_run)
    if let Some(config_file) = config_file {
        // check if config file is inside repo
        assert!(working_dir.is_absolute());
        assert!(config_file.is_absolute());
        if config_file.starts_with(working_dir) {
            match config_file
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .as_deref()
            {
                Some("cfg" | "ini") => bumpversion::config::ini::replace_version(
                    config_file,
                    config,
                    // config.current_version,
                    // next_version_str,
                    current_version,
                    &next_version_serialized,
                    dry_run,
                ),
                Some("toml") => bumpversion::config::pyproject_toml::replace_version(
                    config_file,
                    config,
                    current_version,
                    &next_version_serialized,
                    // config,
                    // version,
                    // next_version,
                    // ctx,
                    dry_run,
                ),
                other => Err(eyre::eyre!("unknown config file format {other:?}")),
            }?;
        } else {
            tracing::warn!("config file {config_file:?} is outside of the repo {working_dir:?} and will not be modified");
        }
    }

    let ctx = context::get_context(
        config,
        Some(tag_and_revision),
        Some(&version),
        Some(&next_version),
    );
    // ctx["new_version"] = next_version_str
    //

    let new_version_serialized = bumpversion::version::compat::SerializedVersion {
        version: next_version_serialized.clone(),
        tag: tag_and_revision
            .tag
            .as_ref()
            .map(|tag| tag.current_tag.clone()),
    };
    hooks::run_pre_commit_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&version),
        Some(&next_version),
        &new_version_serialized,
        dry_run,
    )?;
    //
    // commit_and_tag(config, config_file, configured_files, ctx, dry_run)
    //
    hooks::run_post_commit_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&version),
        Some(&next_version),
        &new_version_serialized,
        dry_run,
    )?;
    //
    Ok(())
}

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
