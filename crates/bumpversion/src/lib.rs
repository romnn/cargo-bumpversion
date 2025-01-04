#![forbid(unsafe_code)]
#![allow(warnings)]

pub mod command;
pub mod config;
pub mod context;
pub mod diagnostics;
pub mod f_string;
pub mod files;
pub mod hooks;
pub mod vcs;
pub mod version;

use crate::{
    files::FileMap,
    vcs::{git::GitRepository, TagAndRevision, VersionControlSystem},
};
use color_eyre::eyre;
use futures::stream::{StreamExt, TryStreamExt};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub enum Bump<'a> {
    Component(&'a str),
    NewVersion(&'a str),
}

/// Find config file in one of the default config file locations.
///
/// # Errors
/// When the config file cannot be read or parsed.
pub async fn find_config<W>(
    dir: &Path,
    printer: &diagnostics::Printer<W>,
) -> eyre::Result<Option<(PathBuf, config::Config)>>
where
    W: codespan_reporting::term::termcolor::WriteColor + Send + Sync + 'static,
{
    use diagnostics::ToDiagnostics;
    let config_files = config::config_file_locations(&dir);

    // let test: Vec<_> = futures::stream::iter(config_files).collect().await;

    // let mut config_files = futures::stream::iter(config_files.collect::<Vec<_>>())
    let mut config_files = futures::stream::iter(config_files)
        .then(|config_file| async move {
            let path = config_file.path();
            if !path.is_file() {
                return Ok(None);
            };
            let Ok(path) = path.canonicalize() else {
                return Ok(None);
            };
            let config = tokio::fs::read_to_string(&path).await?;
            let file_id = printer.add_source_file(&path, config.to_string());

            let test = tokio::task::spawn_blocking(move || {
                let mut diagnostics = vec![];
                let strict = true;

                let config_res = match &config_file {
                    config::ConfigFile::BumpversionToml(path)
                    | config::ConfigFile::PyProject(path) => {
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

                config_res.map(|c| c.map(|c| (path.clone(), c, diagnostics)))
            });

            test.await?
        })
        .filter_map(|res| async move { res.transpose() });

    futures::pin_mut!(config_files);

    Ok(config_files
        .next()
        .await
        .transpose()?
        .map(|(config_file_path, mut config, diagnostics)| {
            // emit diagnostics
            for diagnostic in &diagnostics {
                printer.emit(diagnostic);
            }

            // the order is important here
            config.merge_global_config();

            let defaults = config::GlobalConfig::default();
            config.apply_defaults(&defaults);

            (config_file_path, config)
        }))
}

/// Bump the desired version component to the next value or set the version to `new_version`.
pub async fn bump(
    bump: Bump<'_>,
    repo: &GitRepository,
    config: &config::Config,
    tag_and_revision: &TagAndRevision,
    file_map: FileMap,
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

    let parse_version_pattern = config
        .global
        .parse_version_pattern
        .as_deref()
        .unwrap_or(&config::defaults::PARSE_VERSION_REGEX);

    let version_spec = version::VersionSpec::from_components(components)?;

    let current_version = version::parse_version(
        current_version_serialized,
        &parse_version_pattern,
        &version_spec,
    )?;
    let current_version = current_version.ok_or_else(|| eyre::eyre!("current version is empty"))?;

    let working_dir = repo.path();
    hooks::run_setup_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&current_version),
        dry_run,
    )
    .await?;

    let next_version = match bump {
        Bump::Component(component) => {
            tracing::info!(
                component = component.to_string(),
                "attempting to increment version component"
            );
            current_version.bump(component).map(Some)
        }
        Bump::NewVersion(new_version) => {
            tracing::info!(new_version, "parse new version");
            version::parse_version(new_version, &parse_version_pattern, &version_spec)
        }
    }?;
    let next_version = next_version.ok_or_else(|| eyre::eyre!("next version is empty"))?;
    tracing::info!(next_version = next_version.to_string(), "next version");

    let ctx_without_new_version: HashMap<String, String> = context::get_context(
        Some(tag_and_revision),
        Some(&current_version),
        None,
        Some(current_version_serialized),
        None,
    )
    .collect();

    let serialize_version_patterns = config
        .global
        .serialize_version_patterns
        .as_deref()
        .unwrap_or_default();
    let next_version_serialized =
        next_version.serialize(serialize_version_patterns, &ctx_without_new_version)?;
    tracing::info!(version = next_version_serialized, "next version");

    if current_version_serialized == &next_version_serialized {
        tracing::info!(
            version = next_version_serialized,
            "next version matches current version"
        );
        return Ok(());
    }

    if dry_run {
        tracing::info!("dry run active, won't touch any files.");
    }

    let mut configured_files: IndexMap<PathBuf, Vec<config::FileChange>> =
        files::files_to_modify(config, file_map).collect();

    // filter the files that are not valid for this bump
    if let Bump::Component(version_component_to_bump) = bump {
        for changes in configured_files.values_mut() {
            changes.retain(|change| change.will_bump_component(version_component_to_bump));
            changes.retain(|change| !change.will_not_bump_component(version_component_to_bump));
        }
    }

    let ctx_with_new_version: HashMap<String, String> = context::get_context(
        Some(tag_and_revision),
        Some(&current_version),
        Some(&next_version),
        Some(current_version_serialized),
        Some(&next_version_serialized),
    )
    .collect();

    let configured_files = Arc::new(configured_files);

    futures::stream::iter(configured_files.iter())
        .map(|file| async move { file })
        .buffer_unordered(8)
        .then(|(path, change)| {
            let current_version = current_version.clone();
            let next_version = next_version.clone();
            let ctx_with_new_version = ctx_with_new_version.clone();
            async move {
                assert!(path.is_absolute());
                files::replace_version_in_file(
                    &path,
                    &change,
                    &current_version,
                    &next_version,
                    &ctx_with_new_version,
                    dry_run,
                )
                .await?;
                Ok::<_, eyre::Report>(())
            }
        })
        .try_collect()
        .await?;

    if let Some(config_file) = config_file {
        // check if config file is inside repo
        assert!(working_dir.is_absolute());
        assert!(config_file.is_absolute());

        if config_file.starts_with(working_dir) {
            match config_file
                .extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("cfg" | "ini") => {
                    config::ini::replace_version(
                        config_file,
                        config,
                        current_version_serialized,
                        &next_version_serialized,
                        dry_run,
                    )
                    .await
                }
                Some("toml") => {
                    config::pyproject_toml::replace_version(
                        config_file,
                        config,
                        current_version_serialized,
                        &next_version_serialized,
                        dry_run,
                    )
                    .await
                }
                other => Err(eyre::eyre!("unknown config file format {other:?}")),
            }?;
        } else {
            tracing::warn!("config file {config_file:?} is outside of the repo {working_dir:?} and will not be modified");
        }
    }

    hooks::run_pre_commit_hooks(
        config,
        working_dir,
        tag_and_revision,
        Some(&current_version),
        Some(&next_version),
        &next_version_serialized,
        dry_run,
    )
    .await?;

    let extra_args = config
        .global
        .commit_args
        .as_deref()
        .and_then(shlex::split)
        .unwrap_or_default();

    let mut files_to_commit: HashSet<&Path> =
        configured_files.keys().map(PathBuf::as_path).collect();
    if let Some(config_file) = config_file {
        files_to_commit.insert(config_file);
    }

    let commit = config.global.commit.unwrap_or(true);
    if commit {
        if dry_run {
            tracing::info!("would prepare commit");
        } else {
            tracing::info!("prepare commit");
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
            .unwrap_or(&config::defaults::COMMIT_MESSAGE);

        let commit_message = commit_message.format(&ctx_with_new_version, true)?;
        tracing::info!(msg = commit_message, "commit");

        if !dry_run {
            let env = std::env::vars().chain([
                ("HGENCODING".to_string(), "utf-8".to_string()),
                (
                    "BUMPVERSION_CURRENT_VERSION".to_string(),
                    current_version_serialized.clone(),
                ),
                (
                    "BUMPVERSION_NEW_VERSION".to_string(),
                    next_version_serialized.clone(),
                ),
            ]);
            repo.commit(commit_message.as_str(), extra_args.as_slice(), env)
                .await?;
        }
    }

    let tag = config.global.tag.unwrap_or(config::defaults::CREATE_TAG);
    if tag {
        let sign_tag = config
            .global
            .sign_tags
            .unwrap_or(config::defaults::SIGN_TAGS);

        let tag_name = config
            .global
            .tag_name
            .as_ref()
            .unwrap_or(&config::defaults::TAG_NAME)
            .format(&ctx_with_new_version, true)?;

        let tag_message = config
            .global
            .tag_message
            .as_ref()
            .unwrap_or(&config::defaults::TAG_MESSAGE)
            .format(&ctx_with_new_version, true)?;

        tracing::info!(msg = tag_message, name = tag_name, "tag");

        let existing_tags = repo.tags().await?;

        if existing_tags.contains(&tag_name) {
            tracing::warn!("tag {tag_name:?} already exists and will not be created");
        } else if dry_run {
            tracing::info!(msg = tag_message, sign = sign_tag, "would tag {tag_name:?}",);
        } else {
            repo.tag(tag_name.as_str(), Some(&tag_message), sign_tag)
                .await?;
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
    )
    .await?;

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use color_eyre::eyre;

    macro_rules! sim_assert_eq_sorted {
        ($left:expr, $right:expr $(,)?) => {
            $left.sort();
            $right.sort();
            similar_asserts::assert_eq!($left, $right);
        };
        ($left:expr, $right:expr, $($arg:tt)+) => {
            $left.sort();
            $right.sort();
            similar_asserts::assert_eq!($left, $right, $($arg)+);
        };
    }
    pub(crate) use sim_assert_eq_sorted;

    static INIT: std::sync::Once = std::sync::Once::new();

    /// Initialize test
    ///
    /// This ensures `color_eyre` is setup once.
    pub fn init() {
        INIT.call_once(|| {
            color_eyre::install().ok();
        });
    }
}
