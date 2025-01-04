#![forbid(unsafe_code)]
#![allow(warnings)]

pub mod command;
pub mod config;
pub mod context;
pub mod diagnostics;
pub mod error;
pub mod f_string;
pub mod files;
pub mod hooks;
pub mod utils;
pub mod vcs;
pub mod version;

use crate::{
    files::FileMap,
    vcs::{git::GitRepository, TagAndRevision, VersionControlSystem},
};
use color_eyre::eyre;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub enum Bump<'a> {
    Component(&'a str),
    NewVersion(&'a str),
}

/// Bump the desired version component to the next value or set the version to `new_version`.
pub fn bump(
    bump: Bump<'_>,
    // version_component_to_bump: &str,
    // new_version_override: Option<&str>,
    repo: &GitRepository,
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
    let parse_version_pattern = regex::RegexBuilder::new(parse_version_pattern).build()?;

    let version_spec = version::VersionSpec::from_components(components)?;

    let current_version = version::parse_version(
        current_version_serialized,
        &parse_version_pattern,
        &version_spec,
    )?;
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

    let next_version_serialized = next_version.serialize(
        config
            .global
            .serialize_version_patterns
            .as_deref()
            .unwrap_or_default(),
        &ctx_without_new_version,
    )?;
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

    // let mut files_to_modify: Vec<(PathBuf, &config::FileChange)> =
    // let mut files_to_modify: IndexMap<PathBuf, &config::FileChange> =
    //     files::files_to_modify(&config, &file_map)
    //         // .flat_map(|(file, configs)| {
    //         //     configs
    //         //         .into_iter()
    //         //         // .copied()
    //         //         .map(|config| (file.clone(), config))
    //         // })
    //         .collect();
    // files_to_modify.sort();

    // dbg!(&files_to_modify);
    // let mut configured_files = files::resolve(&files_to_modify, None, None);
    let mut configured_files: IndexMap<&PathBuf, &Vec<config::FileChange>> =
        files::files_to_modify(config, file_map)
            // .into_iter()
            // .map(|(file, changes)| {
            //     files::ConfiguredFile::new(
            //         file.to_path_buf(),
            //         (*config).clone(),
            //         // version_config,
            //         search,
            //         replace,
            //     )
            // })
            .collect();

    // filter the files that are not valid for this bump
    if let Bump::Component(version_component_to_bump) = bump {
        // TODO: use iter_mut() and retain on the changes...
        // configured_files.retain(|_, change| change.will_bump_component(version_component_to_bump));
        // configured_files
        //     .retain(|_, change| !change.will_not_bump_component(version_component_to_bump));
    }
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

    for (path, change) in &configured_files {
        assert!(path.is_absolute());
        files::replace_version_in_file(
            path,
            change,
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
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("cfg" | "ini") => config::ini::replace_version(
                    config_file,
                    config,
                    current_version_serialized,
                    &next_version_serialized,
                    dry_run,
                ),
                Some("toml") => config::pyproject_toml::replace_version(
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
        .keys()
        .copied()
        .map(PathBuf::as_path)
        .collect();
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
            .unwrap_or(&config::DEFAULT_COMMIT_MESSAGE);

        let commit_message = commit_message.format(&ctx_with_new_version, true)?;
        tracing::info!(msg = commit_message, "commit");

        if !dry_run {
            let env = std::env::vars().chain(
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
                ],
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
        } else if dry_run {
            tracing::info!(msg = tag_message, sign = sign_tag, "would tag {tag_name:?}",);
        } else {
            repo.tag(tag_name.as_str(), Some(&tag_message), sign_tag)?;
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
