#![forbid(unsafe_code)]
#![allow(warnings)]

mod logging;
mod options;
mod verbose;

use bumpversion::{
    config, context,
    diagnostics::{Printer, ToDiagnostics},
    files::FileMap,
    hooks,
    vcs::{git::GitRepository, TagAndRevision, VersionControlSystem},
    version,
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

#[tokio::main]
async fn main() -> eyre::Result<()> {
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

    let printer = bumpversion::diagnostics::Printer::stderr(color_choice);

    let (config_file_path, mut config) = bumpversion::find_config(&dir, &printer)
        .await?
        .ok_or(eyre::eyre!("missing config file"))?;

    // build list of parts
    let components = crate::config::version_component_configs(&config)?;

    let mut cli_files = vec![];
    let mut bump: Option<String> = options
        .bump
        .as_ref()
        .map(AsRef::as_ref)
        .map(ToString::to_string);
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

    let tag_name = config
        .global
        .tag_name
        .as_ref()
        .unwrap_or(&config::defaults::TAG_NAME);

    let parse_version_pattern = config
        .global
        .parse_version_pattern
        .as_deref()
        .unwrap_or(&config::defaults::PARSE_VERSION_REGEX);

    let TagAndRevision { tag, revision } = repo
        .latest_tag_and_revision(tag_name, parse_version_pattern)
        .await?;

    tracing::debug!(?tag, "current");
    tracing::debug!(?revision, "current");

    let allow_dirty = options
        .allow_dirty
        .or(options.no_allow_dirty.invert())
        .or(config.global.allow_dirty)
        .unwrap_or(false);

    let dry_run = options.dry_run.or(config.global.dry_run).unwrap_or(false);

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
        .ok_or(eyre::eyre!("Unable to determine the current version"))?;

    let dirty_files = repo.dirty_files().await?;
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

    // build resolved file map
    let file_map =
        bumpversion::files::resolve_files_from_config(&mut config, &components, Some(repo.path()))?;

    if options.no_configured_files == Some(true) {
        config.global.excluded_paths = Some(file_map.keys().cloned().collect());
    }

    if !cli_files.is_empty() {
        // file_map.extend(cli_files);
        // config.add_files(files);
        config.global.included_paths = Some(cli_files);
    }

    let bump = match options.new_version.as_deref() {
        Some(new_version) => bumpversion::Bump::NewVersion(new_version),
        None => {
            let bump = bump
                .as_deref()
                .ok_or_else(|| eyre::eyre!("missing version component to bump"))?;
            bumpversion::Bump::Component(&bump)
        }
    };

    bumpversion::bump(
        bump,
        &repo,
        &config,
        &TagAndRevision { tag, revision },
        &file_map,
        components,
        Some(config_file_path.as_path()),
        dry_run,
    )
    .await?;

    tracing::info!(elapsed = ?start.elapsed(), "done");
    Ok(())
}
