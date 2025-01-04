#![forbid(unsafe_code)]
#![allow(warnings)]

mod logging;
mod options;

use bumpversion::{
    config::{self, DEFAULT_COMMIT_MESSAGE},
    context,
    diagnostics::{Printer, ToDiagnostics},
    files::FileMap,
    hooks,
    vcs::{git::GitRepository, TagAndRevision, VersionControlSystem},
    version, Bump,
};
use clap::Parser;
use color_eyre::eyre::{self, WrapErr};
use options::Invert;
use std::collections::{HashMap, HashSet};
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
    let repo = GitRepository::open(&dir)?;

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

    let defaults = config::GlobalConfig::default();
    config.apply_defaults(&defaults);

    // build list of parts
    let components = config::version_component_configs(&config)?;
    // dbg!(&parts);

    let mut cli_files = vec![];
    let mut bump = options.bump.take().map(Bump::from);
    if !options.args.is_empty() {
        if options.bump.is_none() {
            // first argument must be version component to bump
            let component = options.args.remove(0);
            if components.contains_key(&component) {
                bump = Some(Bump::Other(component.to_string()));

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

    let bump = bump.ok_or_else(|| eyre::eyre!("missing version component to bump"))?;
    dbg!(&bump);
    dbg!(&cli_files);

    let tag_name = config
        .global
        .tag_name
        .as_ref()
        .unwrap_or(&config::DEFAULT_TAG_NAME);

    let parse_version_pattern = config
        .global
        .parse_version_pattern
        .as_deref()
        .unwrap_or(config::DEFAULT_PARSE_VERSION_PATTERN);

    let TagAndRevision { tag, revision } =
        repo.latest_tag_and_revision(tag_name, parse_version_pattern)?;
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
    let file_map =
        bumpversion::files::resolve_files_from_config(&mut config, &components, Some(repo.path()))?;
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
        &repo,
        // &dir,
        options.new_version.as_deref(),
        &config,
        &TagAndRevision { tag, revision },
        // &files,
        &file_map,
        components,
        Some(config_file_path.as_path()),
        dry_run,
    )?;

    tracing::info!(elapsed = ?start.elapsed(), "done");
    Ok(())
}

/// Bump the version_part to the next value or set the version to new_version.
fn do_bump(
    version_component_to_bump: &Bump,
    repo: &GitRepository,
    new_version: Option<&str>,
    config: &config::Config,
    tag_and_revision: &TagAndRevision,
    file_map: &FileMap,
    components: config::VersionComponentConfigs,
    config_file: Option<&Path>,
    dry_run: bool,
) -> eyre::Result<()> {
    let current_version_serialized = config
        .global
        .current_version
        .as_ref()
        .ok_or_else(|| eyre::eyre!("missing current version"))?;

    tracing::debug!(
        version = current_version_serialized,
        "parsing current version"
    );

    // TODO: parse this as regex already
    let parse_version_pattern = config
        .global
        .parse_version_pattern
        .as_deref()
        .unwrap_or(config::DEFAULT_PARSE_VERSION_PATTERN);
    let parse_version_pattern = regex::RegexBuilder::new(&parse_version_pattern).build()?;

    let version_spec = version::compat::VersionSpec::from_components(components)?;

    // let version_config =
    //     version::compat::VersionConfig::from_config(&config.global, &parts.clone())?;

    let current_version = version::compat::parse_version(
        current_version_serialized,
        &parse_version_pattern,
        &version_spec,
    )?;
    // let current_version = version_config.parse(&*current_version_serialized)?;
    let current_version = current_version.ok_or_else(|| eyre::eyre!("current version is empty"))?;
    dbg!(&current_version);

    let working_dir = repo.path();
    hooks::run_setup_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&current_version),
        dry_run,
    )?;

    // let next_version = bumpversion::version::get_next_version(
    //     &current_version,
    //     &version_config,
    //     // config,
    //     version_component_to_bump,
    //     new_version,
    // )?;

    let next_version = if let Some(new_version) = new_version {
        tracing::info!(new_version, "parse new version");
        bumpversion::version::compat::parse_version(
            new_version,
            &parse_version_pattern,
            &version_spec,
        )
        // .map_err(|err| err)
    } else {
        tracing::info!(
            component = version_component_to_bump.to_string(),
            "attempting to increment version component"
        );
        current_version.bump(version_component_to_bump).map(Some)
        // .map_err(|err| err.into())
    }?;
    let next_version = next_version.ok_or_else(|| eyre::eyre!("next version is empty"))?;
    tracing::info!(next_version = next_version.to_string(), "next version");
    // dbg!(&next_version.to_string());

    let ctx_without_new_version: HashMap<String, String> = context::get_context(
        Some(tag_and_revision),
        Some(&current_version),
        None,
        Some(current_version_serialized),
        None,
    )
    .collect();

    let next_version_serialized = next_version.serialize(
        // &next_version,
        // version_config.serialize_version_patterns
        config
            .global
            .serialize_version_patterns
            .as_deref()
            .unwrap_or_default(),
        // ctx.iter().map()move |(k, v)| (k.as_str(), v.as_str())),
        &ctx_without_new_version, // ctx_without_new_version
                                  //     .iter()
                                  //     .map(|(k, v)| (k.as_str(), v.as_str())),
    )?;
    tracing::info!(version = next_version_serialized, "next version");

    if current_version_serialized == &next_version_serialized {
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
    let mut configured_files = bumpversion::files::resolve(&files_to_modify, None, None);

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

    let ctx_with_new_version: HashMap<String, String> = context::get_context(
        Some(tag_and_revision),
        Some(&current_version),
        Some(&next_version),
        Some(current_version_serialized),
        Some(&next_version_serialized),
    )
    .collect();

    // dbg!(&ctx_with_new_version);

    for file in configured_files.iter() {
        assert!(file.path.is_absolute());
        bumpversion::files::replace_version_in_file(
            &file.path,
            &file.file_change,
            &current_version,
            &next_version,
            &ctx_with_new_version,
            dry_run,
        )?;
    }

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
                    current_version_serialized,
                    &next_version_serialized,
                    dry_run,
                ),
                Some("toml") => bumpversion::config::pyproject_toml::replace_version(
                    config_file,
                    config,
                    current_version_serialized,
                    &next_version_serialized,
                    dry_run,
                ),
                other => Err(eyre::eyre!("unknown config file format {other:?}")),
            }?;
        } else {
            tracing::warn!("config file {config_file:?} is outside of the repo {working_dir:?} and will not be modified");
        }
    }

    // let new_version_serialized = bumpversion::version::compat::SerializedVersion {
    //     version: next_version_serialized.clone(),
    //     tag: tag_and_revision
    //         .tag
    //         .as_ref()
    //         .map(|tag| tag.current_tag.clone()),
    // };

    hooks::run_pre_commit_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&current_version),
        Some(&next_version),
        &next_version_serialized,
        dry_run,
    )?;

    let extra_args = config
        .global
        .commit_args
        .as_deref()
        .and_then(shlex::split)
        .unwrap_or_default();

    let mut files_to_commit: HashSet<&Path> = configured_files
        .iter()
        .map(|file| file.path.as_path())
        .collect();
    if let Some(config_file) = config_file {
        files_to_commit.insert(config_file);
    }

    let commit = config.global.commit.unwrap_or(true);
    if commit {
        if dry_run {
            tracing::info!("would prepare commit")
        } else {
            tracing::info!("prepare commit")
        }

        for path in files_to_commit {
            if dry_run {
                tracing::info!(?path, "would add changes");
            } else {
                tracing::info!(?path, "adding changes");
                repo.add(&[path]);
            }
        }

        let commit_message = config
            .global
            .commit_message
            .as_ref()
            .unwrap_or(&config::DEFAULT_COMMIT_MESSAGE);

        let commit_message = commit_message.format(&ctx_with_new_version, true)?;
        tracing::info!(msg = commit_message, "commit");

        if !dry_run {
            let env = std::env::vars().into_iter().chain(
                [
                    ("HGENCODING".to_string(), "utf-8".to_string()),
                    (
                        "BUMPVERSION_CURRENT_VERSION".to_string(),
                        current_version_serialized.clone(),
                    ),
                    (
                        "BUMPVERSION_NEW_VERSION".to_string(),
                        next_version_serialized.clone(),
                    ),
                ]
                .into_iter(),
            );
            repo.commit(commit_message.as_str(), extra_args.as_slice(), env)?;
        }
    }

    let tag = config.global.tag.unwrap_or(config::DEFAULT_CREATE_TAG);
    if tag {
        let sign_tag = config.global.sign_tags.unwrap_or(config::DEFAULT_SIGN_TAGS);

        let tag_name = config
            .global
            .tag_name
            .as_ref()
            .unwrap_or(&config::DEFAULT_TAG_NAME)
            .format(&ctx_with_new_version, true)?;

        let tag_message = config
            .global
            .tag_message
            .as_ref()
            .unwrap_or(&config::DEFAULT_TAG_MESSAGE)
            .format(&ctx_with_new_version, true)?;

        tracing::info!(msg = tag_message, name = tag_name, "tag");

        let existing_tags = repo.tags()?;

        if existing_tags.contains(&tag_name) {
            tracing::warn!("tag {tag_name:?} already exists and will not be created");
        } else {
            if dry_run {
                tracing::info!(msg = tag_message, sign = sign_tag, "would tag {tag_name:?}",);
            } else {
                repo.tag(tag_name.as_str(), Some(&tag_message), sign_tag)?;
            }
        }
    }

    hooks::run_post_commit_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&current_version),
        Some(&next_version),
        &next_version_serialized,
        dry_run,
    )?;

    Ok(())
}
