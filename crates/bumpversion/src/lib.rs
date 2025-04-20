//! # Bumpversion
//!
//! A library for bumping version numbers in projects based on configuration files
//! and version control metadata.
//!
//! # Examples
//!
//! ```no_run
//! use bumpversion::{
//!   BumpVersion,
//!   Bump,
//!   diagnostics::Printer,
//!   vcs::{TagAndRevision, VersionControlSystem, git::GitRepository},
//!   logging,
//!   config,
//! };
//! use std::path::PathBuf;
//!
//! # async fn example() -> color_eyre::eyre::Result<()> {
//! // Locate and parse the configuration
//! let repo_path: PathBuf = ".".into();
//! let printer = Printer::stderr(None);
//!
//! // find and parse configuration
//! let (config_file, mut config) = bumpversion::find_config(
//!   &repo_path,
//!   &Default::default(),
//!   &printer,
//! ).await?.unwrap();
//!
//! // open version control system
//! let repo = GitRepository::open(&repo_path)?;
//! let components = config::version::version_component_configs(&config);
//!
//! // build resolved file map
//! let file_map = bumpversion::files::resolve_files_from_config(
//!   &mut config,
//!   &components,
//!   Some(repo.path()),
//! )?;
//!
//! let TagAndRevision { tag, revision } = repo.latest_tag_and_revision(
//!   &config.global.tag_name,
//!   &config.global.parse_version_pattern,
//! )
//! .await?;
//!
//! let logger = logging::TracingLogger::new(logging::Verbosity::High);
//! let manager = BumpVersion {
//!   repo,
//!   config,
//!   logger,
//!   tag_and_revision: TagAndRevision { tag, revision },
//!   file_map,
//!   components,
//!   config_file: Some(config_file),
//! };
//! manager.bump(Bump::Component("patch")).await?;
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]
// #![warn(missing_docs)]

pub mod command;
pub mod config;
pub mod context;
pub mod diagnostics;
pub mod f_string;
pub mod files;
pub mod hooks;
pub mod logging;
pub mod vcs;
pub mod version;

use crate::{
    files::FileMap,
    vcs::{TagAndRevision, VersionControlSystem},
};
use colored::{Color, Colorize};
use files::IoError;
use futures::stream::{StreamExt, TryStreamExt};
use indexmap::IndexMap;
use logging::{LogExt, Verbosity};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Specifies which version bump to perform.
///
/// Variants:
/// - `Component(name)`: increment the named component (e.g., "major", "minor", "patch").
/// - `NewVersion(version)`: set the version to the given value.
pub enum Bump<'a> {
    /// Increment the named version component (e.g., "major", "minor", "patch").
    Component(&'a str),
    /// Set the version to the specified new version string.
    NewVersion(&'a str),
}

/// Find config file in one of the default config file locations.
///
/// # Errors
/// When the config file cannot be read or parsed.
pub async fn find_config<W>(
    dir: &Path,
    config_overrides: &config::GlobalConfig,
    printer: &diagnostics::Printer<W>,
) -> Result<Option<(config::ConfigFile, config::FinalizedConfig)>, config::Error>
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
            }
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
                        todo!("cargo");
                        // Ok(None)
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
            use crate::config::MergeWith;

            // emit diagnostics
            for diagnostic in &diagnostics {
                printer.emit(diagnostic).map_err(diagnostics::Error::from)?;
            }

            let mut global_config = config_overrides.clone();
            global_config.merge_with(&config.global);
            config.global = global_config;

            Ok::<_, config::Error>((config_file, config.finalize()))
        })
        .transpose()
}

/// Errors that can occur when performing a version bump.
///
/// This includes missing versions, hook failures, serialization errors,
/// file replacement failures, and version control errors.
#[derive(thiserror::Error, Debug)]
pub enum BumpError<VCS>
where
    VCS: VersionControlSystem,
{
    /// Current version was not found in configuration or tags.
    #[error("missing current version")]
    MissingCurrentVersion,
    /// Parsed version string was empty or invalid.
    #[error("version is empty")]
    EmptyVersion,
    /// A configured hook (setup, pre-commit, or post-commit) failed.
    #[error("failed to run hook")]
    Hook(#[from] crate::hooks::Error),
    /// An error occurred while bumping the version component.
    #[error("failed to bump version")]
    Bump(#[from] crate::version::BumpError),
    /// Failed to serialize the new version string.
    #[error("failed to serialize version")]
    Serialize(#[from] crate::version::SerializeError),
    /// Error replacing version in project files.
    #[error("failed to replace version")]
    ReplaceVersion(#[from] crate::files::ReplaceVersionError),
    /// A required template argument was missing.
    #[error(transparent)]
    MissingArgument(#[from] f_string::MissingArgumentError),
    /// Underlying version control system error.
    #[error(transparent)]
    VCS(VCS::Error),
}

/// Manager for performing version bumps in a repository.
///
/// Holds the VCS interface, configuration, and file mappings needed to
/// update version numbers, run hooks, and commit/tag changes.
#[derive(Debug)]
pub struct BumpVersion<VCS, L> {
    /// Interface to the version control system (e.g., Git).
    pub repo: VCS,
    /// Finalized bumpversion configuration.
    pub config: config::FinalizedConfig,
    /// Logger for outputting messages and diffs.
    pub logger: L,
    /// Metadata about the current tag and repository revision.
    pub tag_and_revision: TagAndRevision,
    /// Mapping of files to their associated version change instructions.
    pub file_map: FileMap,
    /// Component definitions for parsing and serializing versions.
    pub components: config::version::VersionComponentConfigs,
    /// Path to the configuration file in use, if any.
    pub config_file: Option<config::ConfigFile>,
}

pub async fn apply_modifications<'a, VCS, S>(
    configured_files: &'a IndexMap<PathBuf, Vec<config::change::FileChange>>,
    current_version: &version::Version,
    new_version: &version::Version,
    ctx_with_new_version: &HashMap<String, String, S>,
    dry_run: bool,
) -> Result<Vec<(&'a PathBuf, Option<files::Modification>)>, BumpError<VCS>>
where
    VCS: VersionControlSystem,
    S: std::hash::BuildHasher + Clone,
{
    let mut modifications: Vec<(&PathBuf, Option<files::Modification>)> =
        futures::stream::iter(configured_files.iter())
            .map(|file| async move { file })
            .buffer_unordered(8)
            .then(|(path, change)| {
                let current_version = current_version.clone();
                let new_version = new_version.clone();
                let ctx_with_new_version = ctx_with_new_version.clone();
                async move {
                    debug_assert!(path.is_absolute());
                    let modification = files::replace_version_in_file(
                        path,
                        change,
                        &current_version,
                        &new_version,
                        &ctx_with_new_version,
                        dry_run,
                    )
                    .await?;
                    Ok::<_, BumpError<VCS>>((path, modification))
                }
            })
            .try_collect()
            .await?;

    modifications.sort_by_key(|(path, _)| (*path).clone());
    Ok(modifications)
}

impl<VCS, L> BumpVersion<VCS, L>
where
    VCS: VersionControlSystem,
    L: logging::Log,
{
    async fn apply_version_bump(
        &self,
        bump: Bump<'_>,
        current_version: version::Version,
        current_version_serialized: &str,
        new_version: version::Version,
        new_version_serialized: String,
    ) -> Result<(), BumpError<VCS>> {
        if current_version_serialized == new_version_serialized {
            tracing::info!(
                version = new_version_serialized,
                "next version matches current version"
            );
            return Ok(());
        }

        if self.config.global.dry_run {
            tracing::info!("dry run active, won't touch any files.");
        }

        let mut configured_files: IndexMap<PathBuf, Vec<config::change::FileChange>> =
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
            Some(&new_version),
            Some(current_version_serialized),
            Some(&new_version_serialized),
        )
        .collect();

        let configured_files = Arc::new(configured_files);

        let modifications = apply_modifications(
            &configured_files,
            &current_version,
            &new_version,
            &ctx_with_new_version,
            self.config.global.dry_run,
        )
        .await?;

        for (path, modification) in modifications {
            self.logger.log(Verbosity::Low, "");
            self.logger.log_modification(path, modification);
        }

        if let Some(ref config_file) = self.config_file {
            let modification = self
                .update_config_file(config_file, &ctx_with_new_version)
                .await?;
            self.logger.log(Verbosity::Low, "");
            self.logger
                .log_modification(config_file.path(), modification);
        }

        self.run_pre_commit_hooks(
            Some(&current_version),
            Some(&new_version),
            &new_version_serialized,
        )
        .await?;

        let additional_files: Vec<_> = self
            .config
            .global
            .additional_files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    self.repo.path().join(path)
                }
            })
            .collect();

        // TODO: warn for files that dirty but not in either configured or additional files
        self.commit_changes(
            &configured_files,
            &additional_files,
            current_version_serialized.to_string(),
            new_version_serialized.clone(),
            &ctx_with_new_version,
        )
        .await?;

        self.run_post_commit_hooks(
            Some(&current_version),
            Some(&new_version),
            &new_version_serialized,
        )
        .await?;

        Ok(())
    }

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

        let parse_version_pattern = &self.config.global.parse_version_pattern;
        let version_spec = version::VersionSpec::from_components(self.components.clone());
        let current_version = version::Version::parse(
            current_version_serialized,
            parse_version_pattern,
            &version_spec,
        );
        // let current_version = self.current_version()?;
        let current_version = current_version.ok_or_else(|| BumpError::EmptyVersion)?;

        self.logger
            .log(Verbosity::Low, &format!("{}", "[current version]".blue()));
        self.logger.log(
            Verbosity::Low,
            &format!("\t{}", current_version_serialized.yellow().bold()),
        );
        self.logger.log(
            Verbosity::Medium,
            &format!(
                "\t{}",
                crate::logging::format_version(&current_version, Color::Cyan)
            ),
        );

        self.run_setup_hooks(Some(&current_version)).await?;

        let new_version = match bump {
            Bump::Component(comp_name) => {
                tracing::info!(
                    component = comp_name.to_string(),
                    "attempting to increment version component"
                );
                current_version.bump(comp_name).map_err(Into::into)
            }
            Bump::NewVersion(new_version) => {
                tracing::info!(new_version, "parse new version");
                version::Version::parse(new_version, parse_version_pattern, &version_spec)
                    .ok_or_else(|| BumpError::EmptyVersion)
            }
        }?;

        tracing::info!(new_version = new_version.to_string(), "next version");

        let ctx_without_new_version: HashMap<String, String> = context::get_context(
            Some(&self.tag_and_revision),
            Some(&current_version),
            None,
            Some(current_version_serialized),
            None,
        )
        .collect();

        let serialize_version_patterns = &self.config.global.serialize_version_patterns;
        let new_version_serialized =
            new_version.serialize(serialize_version_patterns, &ctx_without_new_version)?;
        tracing::info!(version = new_version_serialized, "next version");

        self.logger
            .log(Verbosity::Low, &format!("{}", "[new version]".blue()));
        self.logger.log(
            Verbosity::Low,
            &format!("\t{}", new_version_serialized.yellow().bold()),
        );
        self.logger.log(
            Verbosity::Medium,
            &format!(
                "\t{}",
                crate::logging::format_version(&new_version, Color::Cyan)
            ),
        );

        self.apply_version_bump(
            bump,
            current_version,
            current_version_serialized,
            new_version,
            new_version_serialized,
        )
        .await
    }

    /// Update the version string in the bumpversion configuration file.
    ///
    /// Detects the file format (INI or TOML), applies version replacement using the provided
    /// template context, and writes the file if modified (unless dry-run is enabled).
    /// Supports:
    /// - `.bumpversion.cfg` and `setup.cfg` (INI)
    /// - `.bumpversion.toml` and `pyproject.toml` (TOML)
    ///
    /// # Arguments
    /// * `config_file` - The configuration file variant indicating path and format.
    /// * `ctx` - Mapping of template variables for version substitution.
    ///
    /// # Returns
    /// * `Ok(Some(modification))` when the file was updated and a diff is available.
    /// * `Ok(None)` when no change was needed or the config file is outside the repository.
    ///
    /// # Errors
    /// Returns `BumpError::ReplaceVersion` if the version replacement fails due to
    /// I/O, serialization, or template errors.
    pub async fn update_config_file<K, V>(
        &self,
        config_file: &config::ConfigFile,
        ctx: &HashMap<K, V>,
    ) -> Result<Option<files::Modification>, BumpError<VCS>>
    where
        K: std::borrow::Borrow<str> + std::hash::Hash + Eq + std::fmt::Debug,
        V: AsRef<str> + std::fmt::Debug,
    {
        let config_path = config_file.path();

        // check if config file is inside repo
        let working_dir = self.repo.path();
        debug_assert!(working_dir.is_absolute());
        debug_assert!(config_path.is_absolute());

        if config_path.starts_with(working_dir) {
            let modification = match config_file {
                config::ConfigFile::SetupCfg(_) | config::ConfigFile::BumpversionCfg(_) => {
                    config::ini::replace_version(
                        config_path,
                        &self.config,
                        ctx,
                        self.config.global.dry_run,
                    )
                    .await
                    .map_err(files::ReplaceVersionError::from)
                }
                config::ConfigFile::PyProject(_) | config::ConfigFile::BumpversionToml(_) => {
                    config::toml::replace_version(
                        config_path,
                        &self.config,
                        ctx,
                        self.config.global.dry_run,
                    )
                    .await
                }
                config::ConfigFile::CargoToml(_) => {
                    todo!("cargo support")
                }
            }?;

            Ok(modification)
        } else {
            tracing::warn!(
                "config file {config_file:?} is outside of the repo {working_dir:?} and will not be modified"
            );
            Ok(None)
        }
    }

    /// Stage and commit versioned files, and optionally create a VCS tag.
    ///
    /// This will:
    /// 1. Collect files modified according to `configured_files` and any `additional_files`.
    /// 2. Stage these files along with the configuration file if present.
    /// 3. Commit with the configured commit message and arguments.
    /// 4. If tagging is enabled in config, create a new tag with the configured name and message.
    ///
    /// # Arguments
    /// * `configured_files` - Map of file paths to their version change instructions.
    /// * `additional_files` - Extra file paths to include in the commit (e.g., config file).
    /// * `current_version_serialized` - Previous version string before bump.
    /// * `next_version_serialized` - New version string after bump.
    /// * `ctx` - Context map used to format commit and tag templates.
    ///
    /// # Errors
    /// Returns a [`BumpError<VCS>`] if staging, committing, or tagging fails,
    /// or if message/tag formatting encounters an error.
    pub async fn commit_changes(
        &self,
        configured_files: &IndexMap<PathBuf, Vec<config::change::FileChange>>,
        additional_files: &[impl AsRef<Path>],
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

        let mut files_to_commit: HashSet<&Path> = configured_files
            .keys()
            .map(PathBuf::as_path)
            .chain(additional_files.iter().map(std::convert::AsRef::as_ref))
            .collect();

        if let Some(ref config_file) = self.config_file {
            // add config file
            files_to_commit.insert(config_file.path());
        }

        if self.config.global.commit {
            self.logger
                .log(Verbosity::Low, &format!("{}", "[commit]".magenta()));

            for path in &files_to_commit {
                self.logger.log(
                    Verbosity::Low,
                    &format!("\t{} {}", "   add".dimmed(), path.to_string_lossy().cyan()),
                );
            }
            if !self.config.global.dry_run {
                self.repo
                    .add(&files_to_commit)
                    .await
                    .map_err(BumpError::VCS)?;
            }

            let commit_message = self.config.global.commit_message.format(ctx, true)?;
            tracing::info!(msg = commit_message, "commit");

            self.logger.log(
                Verbosity::Low,
                &format!("\t{} {}", "commit".dimmed(), commit_message.cyan()),
            );

            if !self.config.global.dry_run {
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

        if self.config.global.tag {
            let sign_tag = self.config.global.sign_tags;
            let tag_name = self.config.global.tag_name.format(ctx, true)?;
            let tag_message = self.config.global.tag_message.format(ctx, true)?;

            tracing::info!(msg = tag_message, name = tag_name, "tag");

            let existing_tags = self.repo.tags().await.map_err(BumpError::VCS)?;

            self.logger
                .log(Verbosity::Low, &format!("{}", "[tag]".magenta()));

            if existing_tags.contains(&tag_name) {
                self.logger.log(
                    Verbosity::Low,
                    &format!(
                        "\t{}",
                        format!("tag {tag_name} already exists and will not be created").dimmed()
                    ),
                );
                tracing::warn!("tag {tag_name:?} already exists and will not be created");
            } else {
                self.logger.log(
                    Verbosity::Low,
                    &format!("\t{}{}", "tag = ".dimmed(), tag_name.yellow()),
                );
                self.logger.log(
                    Verbosity::Low,
                    &format!("\t{}{}", "message = ".dimmed(), tag_message.yellow()),
                );
                self.logger.log(
                    Verbosity::Low,
                    &format!("\t{}{}", "sign = ".dimmed(), sign_tag.to_string().yellow()),
                );
                if !self.config.global.dry_run {
                    self.repo
                        .tag(tag_name.as_str(), Some(&tag_message), sign_tag)
                        .await
                        .map_err(BumpError::VCS)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    //! Crate-level testing utilities.

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
