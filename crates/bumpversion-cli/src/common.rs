use crate::options;
use bumpversion::{
    config,
    vcs::{TagAndRevision, VersionControlSystem, git::GitRepository},
};
use color_eyre::eyre::{self, WrapErr};

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

pub async fn bumpversion(mut options: options::Options) -> eyre::Result<()> {
    let start = std::time::Instant::now();

    let color_choice = options.color_choice.unwrap_or(termcolor::ColorChoice::Auto);
    let use_color = crate::logging::setup(options.log_level, color_choice)?;
    colored::control::set_override(use_color);

    let cwd = std::env::current_dir().wrap_err("could not determine current working dir")?;
    let dir = options.dir.as_deref().unwrap_or(&cwd).canonicalize()?;
    let repo = GitRepository::open(&dir)?;

    let printer = bumpversion::diagnostics::Printer::stderr(color_choice);

    let cli_overrides = options::global_cli_config(&options)?;
    let (config_file_path, mut config) = bumpversion::find_config(&dir, &cli_overrides, &printer)
        .await?
        .ok_or(eyre::eyre!("missing config file"))?;

    let components = config::version::version_component_configs(&config);
    let (bump, cli_files) = options::parse_positional_arguments(&mut options, &components)?;

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

    let logger = crate::verbose::Logger::new(verbosity).dry_run(config.global.dry_run);
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
