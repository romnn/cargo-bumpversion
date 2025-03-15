#![forbid(unsafe_code)]

mod logging;
mod options;
mod verbose;

use bumpversion::{
    config,
    vcs::{TagAndRevision, VersionControlSystem, git::GitRepository},
};
use clap::Parser;
use color_eyre::eyre::{self, WrapErr};
use options::Invert;
use std::path::PathBuf;

fn fix_options(options: &mut options::Options) {
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

fn parse_positional_arguments(
    options: &mut options::Options,
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

async fn check_is_dirty(
    repo: &GitRepository,
    config: &config::FinalizedConfig,
) -> eyre::Result<()> {
    let dirty_files = repo.dirty_files().await?;
    if !config.global.allow_dirty && !dirty_files.is_empty() {
        eyre::bail!(
            "Working directory is not clean:\n\n{}",
            dirty_files
                .iter()
                .map(|file| file.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    Ok(())
}

fn global_cli_config(
    options: &options::Options,
) -> eyre::Result<bumpversion::config::GlobalConfig> {
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

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let start = std::time::Instant::now();
    color_eyre::install()?;

    let mut options = options::Options::parse();
    fix_options(&mut options);
    let color_choice = options.color_choice.unwrap_or(termcolor::ColorChoice::Auto);
    let use_color = logging::setup(options.log_level, color_choice)?;
    colored::control::set_override(use_color);

    let cwd = std::env::current_dir().wrap_err("could not determine current working dir")?;
    let dir = options.dir.as_deref().unwrap_or(&cwd).canonicalize()?;
    let repo = GitRepository::open(&dir)?;

    let printer = bumpversion::diagnostics::Printer::stderr(color_choice);

    let cli_overrides = global_cli_config(&options)?;
    let (config_file_path, mut config) = bumpversion::find_config(&dir, &cli_overrides, &printer)
        .await?
        .ok_or(eyre::eyre!("missing config file"))?;

    let components = config::version::version_component_configs(&config);
    let (bump, cli_files) = parse_positional_arguments(&mut options, &components)?;

    let TagAndRevision { tag, revision } = repo
        .latest_tag_and_revision(
            &config.global.tag_name,
            &config.global.parse_version_pattern,
        )
        .await?;

    tracing::debug!(?tag, "current");
    tracing::debug!(?revision, "current");

    let configured_version = &config.global.current_version;
    let actual_version = tag.as_ref().map(|tag| &tag.current_version).cloned();

    // if both versions are present, they should match
    if let Some((configured_version, actual_version)) =
        configured_version.as_ref().zip(actual_version.as_ref())
    {
        if configured_version != actual_version {
            tracing::warn!(
                "version {configured_version} from config does not match last tagged version ({actual_version})",
            );
        }
    }

    check_is_dirty(&repo, &config).await?;

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

    let bump = if let Some(new_version) = options.new_version.as_deref() {
        bumpversion::Bump::NewVersion(new_version)
    } else {
        let bump = bump
            .as_deref()
            .ok_or_else(|| eyre::eyre!("missing version component to bump"))?;
        bumpversion::Bump::Component(bump)
    };

    let verbosity: bumpversion::logging::Verbosity = if options.verbosity.quiet > 0 {
        bumpversion::logging::Verbosity::Off
    } else {
        options.verbosity.verbose.into()
    };

    let logger = verbose::Logger::new(verbosity).dry_run(config.global.dry_run);
    let manager = bumpversion::BumpVersion {
        repo,
        config,
        logger,
        tag_and_revision: TagAndRevision { tag, revision },
        file_map,
        components,
        config_file: Some(config_file_path),
    };
    manager.bump(bump).await?;

    tracing::info!(elapsed = ?start.elapsed(), "done");
    Ok(())
}
