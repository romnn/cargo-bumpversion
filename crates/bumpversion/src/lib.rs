#![forbid(unsafe_code)]
#![forbid(unsafe_code)]

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
    vcs::{TagAndRevision, VersionControlSystem},
};
use files::IoError;
use futures::stream::{StreamExt, TryStreamExt};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub enum Bump<'a> {
    Component(&'a str),
    NewVersion(&'a str),
}

/// Logging verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Verbosity {
    Off = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

impl From<u8> for Verbosity {
    fn from(value: u8) -> Self {
        match value {
            0 => Verbosity::Off,
            1 => Verbosity::Low,
            2 => Verbosity::Medium,
            _ => Verbosity::High,
        }
    }
}

/// Logging implementation to use.
pub trait Log {
    fn log(&self, verbosity: Verbosity, message: &str);
}

/// Find config file in one of the default config file locations.
///
/// # Errors
/// When the config file cannot be read or parsed.
pub async fn find_config<W>(
    dir: &Path,
    printer: &diagnostics::Printer<W>,
) -> Result<Option<(config::ConfigFile, config::Config)>, config::Error>
where
    W: codespan_reporting::term::termcolor::WriteColor + Send + Sync + 'static,
{
    use diagnostics::ToDiagnostics;
    let config_files = config::config_file_locations(dir);

    let config_files = futures::stream::iter(config_files)
        .then(|config_file| async move {
            let path = config_file.path();
            if !path.is_file() {
                return Ok(None);
            };
            let Ok(path) = path.canonicalize() else {
                return Ok(None);
            };
            let config = tokio::fs::read_to_string(&path)
                .await
                .map_err(|source| IoError::new(source, &path))
                .map_err(config::Error::from)?;

            let file_id = printer.add_source_file(&path, config.to_string());

            let parse_config_task = tokio::task::spawn_blocking(move || {
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
                        res.map_err(|source| config::Error::Toml {
                            source,
                            path: path.clone(),
                        })
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
                        res.map_err(|source| config::Error::Ini {
                            source,
                            path: path.clone(),
                        })
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
                        res.map_err(|source| config::Error::Ini {
                            source,
                            path: path.clone(),
                        })
                    }
                    config::ConfigFile::CargoToml(_) => {
                        // TODO: cargo
                        Ok(None)
                    }
                };

                config_res.map(|c| c.map(|c| (config_file.clone(), c, diagnostics)))
            });

            parse_config_task.await?
        })
        .filter_map(|res| async move { res.transpose() });

    futures::pin_mut!(config_files);

    config_files
        .next()
        .await
        .transpose()?
        .map(|(config_file, mut config, diagnostics)| {
            // emit diagnostics
            for diagnostic in &diagnostics {
                printer.emit(diagnostic).map_err(diagnostics::Error::from)?;
            }

            // the order is important here
            config.merge_global_config();

            let defaults = config::GlobalConfig::default();
            config.apply_defaults(&defaults);

            Ok::<_, config::Error>((config_file, config))
        })
        .transpose()
}

#[derive(thiserror::Error, Debug)]
pub enum BumpError<VCS>
where
    VCS: VersionControlSystem,
{
    #[error("missing current version")]
    MissingCurrentVersion,
    #[error("version is empty")]
    EmptyVersion,
    #[error("failed to run hook")]
    Hook(#[from] crate::hooks::Error),
    #[error("failed to bump version")]
    Bump(#[from] crate::version::BumpError),
    #[error("failed to serialize version")]
    Serialize(#[from] crate::version::SerializeError),
    #[error("failed to replace version")]
    ReplaceVersion(#[from] crate::files::ReplaceVersionError),
    #[error(transparent)]
    MissingArgument(#[from] f_string::MissingArgumentError),
    #[error(transparent)]
    VCS(VCS::Error),
}

/// Bumpversion manager
#[derive(Debug)]
pub struct BumpVersion<VCS, L> {
    pub repo: VCS,
    pub config: config::Config,
    pub logger: L,
    pub tag_and_revision: TagAndRevision,
    pub file_map: FileMap,
    pub components: config::VersionComponentConfigs,
    pub config_file: Option<config::ConfigFile>,
    pub dry_run: bool,
}

impl<VCS, L> BumpVersion<VCS, L>
where
    VCS: VersionControlSystem,
    L: Log,
{
    /// Bump the desired version component to the next value or set the version to `new_version`.
    ///
    /// # Errors
    /// - When the no current version is present.
    /// - When the current or next version are empty.
    /// - When one of the user-provided setup, pre, or post-commit hooks fails.
    /// - When the current version component cannot be bumped.
    /// - When the next version cannot be serialized.
    /// - When a version in a file cannot be replaced.
    pub async fn bump(&self, bump: Bump<'_>) -> Result<(), BumpError<VCS>> {
        let current_version_serialized = self
            .config
            .global
            .current_version
            .as_ref()
            .ok_or_else(|| BumpError::MissingCurrentVersion)?;

        tracing::debug!(
            version = current_version_serialized,
            "parsing current version"
        );

        let parse_version_pattern = self
            .config
            .global
            .parse_version_pattern
            .as_deref()
            .unwrap_or(&config::defaults::PARSE_VERSION_REGEX);

        let version_spec = version::VersionSpec::from_components(self.components.clone());

        let current_version = version::parse_version(
            current_version_serialized,
            parse_version_pattern,
            &version_spec,
        );
        let current_version = current_version.ok_or_else(|| BumpError::EmptyVersion)?;

        let working_dir = self.repo.path();
        hooks::run_setup_hooks(
            &self.config,
            working_dir,
            &self.tag_and_revision,
            Some(&current_version),
            self.dry_run,
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
                Ok(version::parse_version(
                    new_version,
                    parse_version_pattern,
                    &version_spec,
                ))
            }
        }?;
        let next_version = next_version.ok_or_else(|| BumpError::EmptyVersion)?;
        tracing::info!(next_version = next_version.to_string(), "next version");

        let ctx_without_new_version: HashMap<String, String> = context::get_context(
            Some(&self.tag_and_revision),
            Some(&current_version),
            None,
            Some(current_version_serialized),
            None,
        )
        .collect();

        let serialize_version_patterns = self
            .config
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

        if self.dry_run {
            tracing::info!("dry run active, won't touch any files.");
        }

        let mut configured_files: IndexMap<PathBuf, Vec<config::FileChange>> =
            files::files_to_modify(&self.config, self.file_map.clone()).collect();

        // filter the files that are not valid for this bump
        if let Bump::Component(version_component_to_bump) = bump {
            for changes in configured_files.values_mut() {
                changes.retain(|change| change.will_bump_component(version_component_to_bump));
                changes.retain(|change| !change.will_not_bump_component(version_component_to_bump));
            }
        }

        let ctx_with_new_version: HashMap<String, String> = context::get_context(
            Some(&self.tag_and_revision),
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
                    debug_assert!(path.is_absolute());
                    files::replace_version_in_file(
                        path,
                        change,
                        &current_version,
                        &next_version,
                        &ctx_with_new_version,
                        self.dry_run,
                    )
                    .await?;
                    Ok::<_, BumpError<VCS>>(())
                }
            })
            .try_collect::<()>()
            .await?;

        if let Some(ref config_file) = self.config_file {
            let config_path = config_file.path();
            // check if config file is inside repo
            debug_assert!(working_dir.is_absolute());
            debug_assert!(config_path.is_absolute());

            if config_path.starts_with(working_dir) {
                match config_file {
                    config::ConfigFile::SetupCfg(_) | config::ConfigFile::BumpversionCfg(_) => {
                        config::ini::replace_version(
                            config_path,
                            &self.config,
                            current_version_serialized,
                            &next_version_serialized,
                            self.dry_run,
                        )
                        .await
                        .map_err(files::ReplaceVersionError::from)
                    }
                    config::ConfigFile::PyProject(_) | config::ConfigFile::BumpversionToml(_) => {
                        config::pyproject_toml::replace_version(
                            config_path,
                            &self.config,
                            current_version_serialized,
                            &next_version_serialized,
                            self.dry_run,
                        )
                        .await
                        .map_err(|err| match err {
                            config::pyproject_toml::ReplaceVersionError::Io(err) => {
                                files::ReplaceVersionError::from(err)
                            }
                            config::pyproject_toml::ReplaceVersionError::Toml(err) => {
                                files::ReplaceVersionError::from(err)
                            }
                        })
                    }
                    config::ConfigFile::CargoToml(_) => {
                        todo!("cargo support")
                    }
                }?;
            } else {
                tracing::warn!("config file {config_file:?} is outside of the repo {working_dir:?} and will not be modified");
            }
        }

        hooks::run_pre_commit_hooks(
            &self.config,
            working_dir,
            &self.tag_and_revision,
            Some(&current_version),
            Some(&next_version),
            &next_version_serialized,
            self.dry_run,
        )
        .await?;

        self.commit_changes(
            &configured_files,
            current_version_serialized.clone(),
            next_version_serialized.clone(),
            &ctx_with_new_version,
        )
        .await?;

        hooks::run_post_commit_hooks(
            &self.config,
            working_dir,
            &self.tag_and_revision,
            Some(&current_version),
            Some(&next_version),
            &next_version_serialized,
            self.dry_run,
        )
        .await?;

        Ok(())
    }

    pub async fn commit_changes(
        &self,
        configured_files: &IndexMap<PathBuf, Vec<config::FileChange>>,
        current_version_serialized: String,
        next_version_serialized: String,
        ctx: &HashMap<String, String>,
    ) -> Result<(), BumpError<VCS>> {
        let extra_args = self
            .config
            .global
            .commit_args
            .as_deref()
            .and_then(shlex::split)
            .unwrap_or_default();

        let mut files_to_commit: HashSet<&Path> =
            configured_files.keys().map(PathBuf::as_path).collect();
        if let Some(ref config_file) = self.config_file {
            files_to_commit.insert(config_file.path());
        }

        let commit = self.config.global.commit.unwrap_or(true);
        if commit {
            if self.dry_run {
                tracing::info!("would prepare commit");
            } else {
                tracing::info!("prepare commit");
            }

            for path in files_to_commit {
                if self.dry_run {
                    tracing::info!(?path, "would add changes");
                } else {
                    tracing::info!(?path, "adding changes");
                    self.repo.add(&[path]).await.map_err(BumpError::VCS)?;
                }
            }

            let commit_message = self
                .config
                .global
                .commit_message
                .as_ref()
                .unwrap_or(&config::defaults::COMMIT_MESSAGE);

            let commit_message = commit_message.format(ctx, true)?;
            tracing::info!(msg = commit_message, "commit");

            if !self.dry_run {
                let env = std::env::vars().chain([
                    ("HGENCODING".to_string(), "utf-8".to_string()),
                    (
                        "BUMPVERSION_CURRENT_VERSION".to_string(),
                        current_version_serialized,
                    ),
                    (
                        "BUMPVERSION_NEW_VERSION".to_string(),
                        next_version_serialized,
                    ),
                ]);
                self.repo
                    .commit(commit_message.as_str(), extra_args.as_slice(), env)
                    .await
                    .map_err(BumpError::VCS)?;
            }
        }

        let tag = self
            .config
            .global
            .tag
            .unwrap_or(config::defaults::CREATE_TAG);
        if tag {
            let sign_tag = self
                .config
                .global
                .sign_tags
                .unwrap_or(config::defaults::SIGN_TAGS);

            let tag_name = self
                .config
                .global
                .tag_name
                .as_ref()
                .unwrap_or(&config::defaults::TAG_NAME)
                .format(ctx, true)?;

            let tag_message = self
                .config
                .global
                .tag_message
                .as_ref()
                .unwrap_or(&config::defaults::TAG_MESSAGE)
                .format(ctx, true)?;

            tracing::info!(msg = tag_message, name = tag_name, "tag");

            let existing_tags = self.repo.tags().await.map_err(BumpError::VCS)?;

            if existing_tags.contains(&tag_name) {
                tracing::warn!("tag {tag_name:?} already exists and will not be created");
            } else if self.dry_run {
                tracing::info!(msg = tag_message, sign = sign_tag, "would tag {tag_name:?}",);
            } else {
                self.repo
                    .tag(tag_name.as_str(), Some(&tag_message), sign_tag)
                    .await
                    .map_err(BumpError::VCS)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use similar_asserts::assert_eq as sim_assert_eq;

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
    pub(crate) fn init() {
        INIT.call_once(|| {
            color_eyre::install().ok();
        });
    }

    #[test]
    fn test_verbosity_ord() {
        use super::Verbosity;

        let mut verbosities = [Verbosity::Medium, Verbosity::Low, Verbosity::High];
        verbosities.sort();
        sim_assert_eq!(
            verbosities,
            [Verbosity::Low, Verbosity::Medium, Verbosity::High]
        );
    }
}
