use crate::command::{Error as CommandError, Output};
use crate::{
    config::{self, Config},
    vcs::{RevisionInfo, TagAndRevision},
    version::compat::{SerializedVersion, Version},
};
use color_eyre::eyre;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

// type Env = HashMap<String, String>;

pub const ENV_PREFIX: &str = "BVHOOK_";

/// Provide the base environment variables
fn base_env(config: &Config) -> impl Iterator<Item = (String, String)> {
    vec![
        (
            format!("{ENV_PREFIX}NOW"),
            chrono::Local::now().to_rfc3339(),
        ),
        (
            format!("{ENV_PREFIX}UTCNOW"),
            chrono::Utc::now().to_rfc3339(),
        ),
    ]
    .into_iter()
}

/// Provide the VCS environment variables.
fn vcs_env(tag_and_revision: &TagAndRevision) -> impl Iterator<Item = (String, String)> {
    let TagAndRevision { tag, revision } = tag_and_revision;
    let tag = tag
        // .and_then(|t| t.tag)
        .clone()
        .unwrap_or(crate::vcs::TagInfo {
            dirty: false,
            commit_sha: "".to_string(),
            distance_to_latest_tag: 0,
            current_tag: "".to_string(),
            current_version: "".to_string(),
        });
    let revision = revision.clone().unwrap_or(RevisionInfo {
        branch_name: "".to_string(),
        short_branch_name: "".to_string(),
        repository_root: std::path::PathBuf::default(),
    });
    vec![
        (format!("{ENV_PREFIX}COMMIT_SHA"), tag.commit_sha),
        (
            format!("{ENV_PREFIX}DISTANCE_TO_LATEST_TAG"),
            tag.distance_to_latest_tag.to_string(),
        ),
        (format!("{ENV_PREFIX}IS_DIRTY"), tag.dirty.to_string()),
        (format!("{ENV_PREFIX}CURRENT_VERSION"), tag.current_version),
        (format!("{ENV_PREFIX}CURRENT_TAG"), tag.current_tag),
        (format!("{ENV_PREFIX}BRANCH_NAME"), revision.branch_name),
        (
            format!("{ENV_PREFIX}SHORT_BRANCH_NAME"),
            revision.short_branch_name,
        ),
    ]
    .into_iter()
}

/// Provide the environment variables for each version component with a prefix
fn version_env<'a>(
    version: Option<&'a Version>,
    version_prefix: &'a str,
) -> impl Iterator<Item = (String, String)> + use<'a> {
    let iter = version.map(|version| version.iter()).unwrap_or_default();
    iter.map(move |(comp_name, comp)| {
        (
            format!("{ENV_PREFIX}{version_prefix}{}", comp_name.to_uppercase()),
            comp.value().unwrap_or_default().to_string(),
        )
    })
}

/// Provide the environment dictionary for new_version serialized and tag name.
fn new_version_env<'a>(
    new_version: &SerializedVersion,
    // version: &'a str,
    // tag: &'a str,
) -> impl Iterator<Item = (String, String)> + use<'a> {
    // ctx = get_context(config, current_version, new_version)
    // new_version_string = config.version_config.serialize(new_version, ctx)
    // ctx["new_version"] = new_version_string
    // new_version_tag = config.tag_name.format(**ctx)
    // return {f"{PREFIX}NEW_VERSION": new_version_string, f"{PREFIX}NEW_VERSION_TAG": new_version_tag}
    // return {f"{PREFIX}NEW_VERSION": new_version_string, f"{PREFIX}NEW_VERSION_TAG": new_version_tag}
    vec![
        (
            format!("{ENV_PREFIX}NEW_VERSION"),
            new_version.version.to_string(),
        ),
        (
            format!("{ENV_PREFIX}NEW_VERSION_TAG"),
            new_version.tag.as_deref().unwrap_or_default().to_string(),
        ),
    ]
    .into_iter()
}

/// Provide the environment dictionary for `setup_hook`s.
fn setup_hook_env<'a>(
    config: &'a Config,
    tag_and_revision: &'a TagAndRevision,
    // current_version: &Version,
    current_version: Option<&'a Version>,
) -> impl Iterator<Item = (String, String)> + use<'a> {
    // ) -> HashMap<String, String> {
    std::env::vars()
        .chain(base_env(config))
        .chain(vcs_env(tag_and_revision))
        .chain(version_env(current_version, "CURRENT_"))
    // .collect()
}

// /// Provide the environment dictionary for `pre_commit_hook`s
// fn pre_commit_hook_env<'a>(
//     config: &'a Config,
//     // tag_and_revision: Option<&'a TagAndRevision>,
//     tag_and_revision: &'a TagAndRevision,
//     current_version: Option<&'a Version>,
//     new_version: Option<&'a Version>,
//     new_version_serialized: &SerializedVersion,
// ) -> impl Iterator<Item = (String, String)> + use<'a> {
//     std::env::vars()
//         .chain(base_env(config))
//         .chain(vcs_env(tag_and_revision))
//         .chain(version_env(current_version, "CURRENT_"))
//         .chain(version_env(new_version, "NEW_"))
//         .chain(new_version_env(new_version_serialized))
// }

/// Provide the environment dictionary for `pre_commit_hook` and `post_commit_hook`s
fn pre_and_post_commit_hook_env<'a>(
    config: &'a Config,
    // tag_and_revision: Option<&'a TagAndRevision>,
    tag_and_revision: &'a TagAndRevision,
    current_version: Option<&'a Version>,
    new_version: Option<&'a Version>,
    new_version_serialized: &SerializedVersion,
) -> impl Iterator<Item = (String, String)> + use<'a> {
    std::env::vars()
        .chain(base_env(config))
        .chain(vcs_env(tag_and_revision))
        .chain(version_env(current_version, "CURRENT_"))
        .chain(version_env(new_version, "NEW_"))
        .chain(new_version_env(new_version_serialized))
}

/// Run the pre-commit hooks
pub fn run_pre_commit_hooks(
    config: &Config,
    working_dir: &Path,
    tag_and_revision: &TagAndRevision,
    current_version: Option<&Version>,
    new_version: Option<&Version>,
    new_version_serialized: &SerializedVersion,
    dry_run: bool,
) -> eyre::Result<()> {
    let env = pre_and_post_commit_hook_env(
        config,
        tag_and_revision,
        current_version,
        new_version,
        new_version_serialized,
    );

    let pre_commit_hooks = config
        .global
        .pre_commit_hooks
        .as_deref()
        .unwrap_or_default();

    if pre_commit_hooks.is_empty() {
        tracing::info!("no pre commit hooks defined");
        return Ok(());
    } else if dry_run {
        tracing::info!("would run {} pre commit hooks", pre_commit_hooks.len());
        return Ok(());
    } else {
        tracing::info!("running pre commit hooks");
    }

    run_hooks(pre_commit_hooks, working_dir, env, dry_run)
}

/// Run the post-commit hooks
pub fn run_post_commit_hooks(
    config: &Config,
    working_dir: &Path,
    tag_and_revision: &TagAndRevision,
    current_version: Option<&Version>,
    new_version: Option<&Version>,
    new_version_serialized: &SerializedVersion,
    dry_run: bool,
) -> eyre::Result<()> {
    let env = pre_and_post_commit_hook_env(
        config,
        tag_and_revision,
        current_version,
        new_version,
        new_version_serialized,
    );

    let post_commit_hooks = config
        .global
        .post_commit_hooks
        .as_deref()
        .unwrap_or_default();

    if post_commit_hooks.is_empty() {
        tracing::info!("no post commit hooks defined");
        return Ok(());
    } else if dry_run {
        tracing::info!("would run {} post commit hooks", post_commit_hooks.len());
        return Ok(());
    } else {
        tracing::info!("running post commit hooks");
    }

    run_hooks(post_commit_hooks, working_dir, env, dry_run)
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error("failed to split shell script {0:?}")]
    Shell(String),
}

/// Runs command-line programs using the shell
// fn run_hook(script: &str, ) -> Result<Output, Error> {
fn run_hook(
    script: &str,
    working_dir: &Path,
    // env: &Env,
    env: &HashMap<String, String>,
    // env: &impl Iterator<Item = (String, String)>,
) -> Result<Output, Error> {
    // return subprocess.run(
    //     script, env=environment, encoding="utf-8", shell=True, text=True, capture_output=True, check=False
    // )

    let args = shlex::split(script).ok_or_else(|| Error::Shell(script.to_string()))?;
    let mut cmd = Command::new("sh");
    cmd.args(["-c".to_string()].into_iter().chain(args.into_iter()));
    cmd.envs(env);
    cmd.current_dir(working_dir);
    let output = crate::command::run_command(&mut cmd)?;
    Ok(output)
}

/// Run a list of command-line programs using the shell.
fn run_hooks(
    hooks: &[String],
    working_dir: &Path,
    env: impl Iterator<Item = (String, String)>,
    // env: impl Iterator<Item = (String, String)>,
    // env: &Env,
    dry_run: bool,
) -> eyre::Result<()> {
    // let env: Env = env.collect();
    let env = env.collect();
    for script in hooks {
        if dry_run {
            tracing::info!(?script, "would run hook");
            continue;
        }
        tracing::info!(?script, "running");
        match run_hook(script, working_dir, &env) {
            Ok(output) => {
                tracing::debug!(code = output.status.code(), "hook completed");
                tracing::debug!(output.stdout);
                tracing::debug!(output.stderr);
            }
            Err(err) => {
                if let Error::Command(CommandError::Failed { ref output, .. }) = err {
                    tracing::warn!(output.stdout);
                    tracing::warn!(output.stderr);
                };
                return Err(err.into());
            }
        };
    }
    Ok(())
}

/// Run the setup hooks
pub fn run_setup_hooks(
    config: &Config,
    working_dir: &Path,
    tag_and_revision: &TagAndRevision,
    // current_version: &Version,
    current_version: Option<&Version>,
    dry_run: bool,
) -> eyre::Result<()> {
    let env = setup_hook_env(config, tag_and_revision, current_version);
    let setup_hooks = config.global.setup_hooks.as_deref().unwrap_or_default();
    if setup_hooks.is_empty() {
        tracing::trace!("no setup hooks defined");
        return Ok(());
    } else if dry_run {
        tracing::info!("would run {} setup hooks", setup_hooks.len());
    } else {
        tracing::info!("running setup hooks");
    }
    run_hooks(setup_hooks, working_dir, env, dry_run)
}
