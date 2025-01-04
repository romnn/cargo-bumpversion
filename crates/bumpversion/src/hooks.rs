use crate::command::{self, Error as CommandError, Output};
use crate::{
    config::{self, Config},
    vcs::{RevisionInfo, TagAndRevision},
    version::Version,
};
use async_process::{Command, Stdio};
use color_eyre::eyre;
use std::collections::HashMap;
use std::path::Path;

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
            commit_sha: String::new(),
            distance_to_latest_tag: 0,
            current_tag: String::new(),
            current_version: String::new(),
        });
    let revision = revision.clone().unwrap_or(RevisionInfo {
        branch_name: String::new(),
        short_branch_name: String::new(),
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

/// Provide the environment dictionary for `new_version` serialized and tag name.
fn new_version_env<'a>(
    // new_version: &SerializedVersion,
    new_version_serialized: &str,
    tag: Option<&str>,
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
            new_version_serialized.to_string(),
        ),
        (
            format!("{ENV_PREFIX}NEW_VERSION_TAG"),
            tag.unwrap_or_default().to_string(),
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
    new_version_serialized: &str,
    // tag: Option<&str>,
) -> impl Iterator<Item = (String, String)> + use<'a> {
    let tag = tag_and_revision
        .tag
        .as_ref()
        .map(|tag| tag.current_tag.as_str());
    std::env::vars()
        .chain(base_env(config))
        .chain(vcs_env(tag_and_revision))
        .chain(version_env(current_version, "CURRENT_"))
        .chain(version_env(new_version, "NEW_"))
        .chain(new_version_env(new_version_serialized, tag))
}

/// Run the pre-commit hooks
pub async fn run_pre_commit_hooks(
    config: &Config,
    working_dir: &Path,
    tag_and_revision: &TagAndRevision,
    current_version: Option<&Version>,
    new_version: Option<&Version>,
    new_version_serialized: &str,
    dry_run: bool,
) -> eyre::Result<()> {
    let tag = tag_and_revision
        .tag
        .as_ref()
        .map(|tag| tag.current_tag.as_str());

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

    run_hooks(pre_commit_hooks, working_dir, env, dry_run).await
}

/// Run the post-commit hooks
pub async fn run_post_commit_hooks(
    config: &Config,
    working_dir: &Path,
    tag_and_revision: &TagAndRevision,
    current_version: Option<&Version>,
    new_version: Option<&Version>,
    new_version_serialized: &str,
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

    run_hooks(post_commit_hooks, working_dir, env, dry_run).await
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error("failed to split shell script {0:?}")]
    Shell(String),
}

/// Runs command-line programs using the shell
async fn run_hook(
    script: &str,
    working_dir: &Path,
    env: &HashMap<String, String>,
) -> Result<Output, Error> {
    let args = shlex::split(script).ok_or_else(|| Error::Shell(script.to_string()))?;
    let mut cmd = Command::new("sh");
    cmd.args(["-c".to_string()].into_iter().chain(args));
    cmd.envs(env);
    cmd.current_dir(working_dir);
    let output = command::run_command(&mut cmd).await?;
    Ok(output)
}

/// Run a list of command-line programs using the shell.
async fn run_hooks(
    hooks: &[String],
    working_dir: &Path,
    env: impl Iterator<Item = (String, String)>,
    dry_run: bool,
) -> eyre::Result<()> {
    let env = env.collect();
    for script in hooks {
        if dry_run {
            tracing::info!(?script, "would run hook");
            continue;
        }
        tracing::info!(?script, "running");
        match run_hook(script, working_dir, &env).await {
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
pub async fn run_setup_hooks(
    config: &Config,
    working_dir: &Path,
    tag_and_revision: &TagAndRevision,
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
    run_hooks(setup_hooks, working_dir, env, dry_run).await
}

#[cfg(test)]
mod tests {
    // def assert_os_environ_items_included(result_env: dict) -> None:
    //     """Assert that the OS environment variables are in the result."""
    //     for var, value in os.environ.items():
    //         assert var in result_env
    //         assert result_env[var] == value
    //
    //
    // def assert_scm_info_included(result_env: dict):
    //     """Assert the SCM information is included in the result."""
    //     assert f"{PREFIX}COMMIT_SHA" in result_env
    //     assert f"{PREFIX}DISTANCE_TO_LATEST_TAG" in result_env
    //     assert f"{PREFIX}IS_DIRTY" in result_env
    //     assert f"{PREFIX}BRANCH_NAME" in result_env
    //     assert f"{PREFIX}SHORT_BRANCH_NAME" in result_env
    //     assert f"{PREFIX}CURRENT_VERSION" in result_env
    //     assert f"{PREFIX}CURRENT_TAG" in result_env
    //
    //
    // def assert_current_version_info_included(result_env: dict):
    //     """Assert the current version information is included in the result."""
    //     assert f"{PREFIX}CURRENT_MAJOR" in result_env
    //     assert f"{PREFIX}CURRENT_MINOR" in result_env
    //     assert f"{PREFIX}CURRENT_PATCH" in result_env
    //
    //
    // def assert_new_version_info_included(result_env: dict):
    //     """Assert the new version information is included in the result."""
    //     assert f"{PREFIX}NEW_MAJOR" in result_env
    //     assert f"{PREFIX}NEW_MINOR" in result_env
    //     assert f"{PREFIX}NEW_PATCH" in result_env
    //     assert f"{PREFIX}NEW_VERSION" in result_env
    //     assert f"{PREFIX}NEW_VERSION_TAG" in result_env
    //
    //
    // def test_scm_env_returns_correct_info(git_repo: Path):
    //     """Should return information about the latest tag."""
    //     readme = git_repo.joinpath("readme.md")
    //     readme.touch()
    //     tag_prefix = "v"
    //     overrides = {"current_version": "0.1.0", "commit": True, "tag": True, "tag_name": f"{tag_prefix}{{new_version}}"}
    //
    //     with inside_dir(git_repo):
    //         # Add a file and tag
    //         subprocess.run(["git", "add", "readme.md"])
    //         subprocess.run(["git", "commit", "-m", "first"])
    //         subprocess.run(["git", "tag", f"{tag_prefix}0.1.0"])
    //         conf, _, _ = get_config_data(overrides)
    //
    //     result = scm_env(conf)
    //     assert result[f"{PREFIX}BRANCH_NAME"] == "master"
    //     assert len(result[f"{PREFIX}COMMIT_SHA"]) == 40
    //     assert result[f"{PREFIX}CURRENT_TAG"] == "v0.1.0"
    //     assert result[f"{PREFIX}CURRENT_VERSION"] == "0.1.0"
    //     assert result[f"{PREFIX}DISTANCE_TO_LATEST_TAG"] == "0"
    //     assert result[f"{PREFIX}IS_DIRTY"] == "False"
    //     assert result[f"{PREFIX}SHORT_BRANCH_NAME"] == "master"
    //
    //
    // class MockDatetime(datetime.datetime):
    //     @classmethod
    //     def now(cls, tz=None):
    //         return cls(2022, 2, 1, 17) if tz else cls(2022, 2, 1, 12)
    //
    //
    // class TestBaseEnv:
    //     """Tests for base_env function."""
    //
    //     def test_includes_now_and_utcnow(self, mocker):
    //         """The output includes NOW and UTCNOW."""
    //         mocker.patch("datetime.datetime", new=MockDatetime)
    //         config, _, _ = get_config_data({"current_version": "0.1.0"})
    //         result_env = base_env(config)
    //
    //         assert f"{PREFIX}NOW" in result_env
    //         assert f"{PREFIX}UTCNOW" in result_env
    //         assert result_env[f"{PREFIX}NOW"] == "2022-02-01T12:00:00"
    //         assert result_env[f"{PREFIX}UTCNOW"] == "2022-02-01T17:00:00"
    //
    //     def test_includes_os_environ(self):
    //         """The output includes the current process' environment."""
    //         config, _, _ = get_config_data({"current_version": "0.1.0"})
    //         result_env = base_env(config)
    //
    //         assert_os_environ_items_included(result_env)
    //
    //     def test_includes_scm_info(self):
    //         """The output includes SCM information."""
    //         config, _, _ = get_config_data({"current_version": "0.1.0"})
    //         result_env = base_env(config)
    //
    //         assert_scm_info_included(result_env)
    //
    //

    /// The `version_env` for a version should include all its parts"""
    #[test]
    fn test_current_version_env_includes_correct_info() {
        // config, _, current_version = get_config_data(
        //     {"current_version": "0.1.0", "parse": r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)"}
        // )
        // let current_version = Version::from_components([("")]);
        // let env = super::version_env(Some(current_version), "CURRENT_")

        // assert result[f"{PREFIX}CURRENT_MAJOR"] == "0"
        // assert result[f"{PREFIX}CURRENT_MINOR"] == "1"
        // assert result[f"{PREFIX}CURRENT_PATCH"] == "0"
    }

    // def test_new_version_env_includes_correct_info():
    //     """The new_version_env should return the serialized version and tag name."""
    //
    //     config, _, current_version = get_config_data(
    //         {"current_version": "0.1.0", "parse": r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)"}
    //     )
    //     new_version = current_version.bump("minor")
    //     result = new_version_env(config, current_version, new_version)
    //
    //     assert result[f"{PREFIX}NEW_VERSION"] == "0.2.0"
    //     assert result[f"{PREFIX}NEW_VERSION_TAG"] == "v0.2.0"
    //
    //
    // def test_get_setup_hook_env_includes_correct_info():
    //     """The setup hook environment should contain specific information."""
    //     config, _, current_version = get_config_data({"current_version": "0.1.0"})
    //     result_env = get_setup_hook_env(config, current_version)
    //
    //     assert_os_environ_items_included(result_env)
    //     assert_scm_info_included(result_env)
    //     assert_current_version_info_included(result_env)
    //
    //
    // def test_get_pre_commit_hook_env_includes_correct_info():
    //     """The pre-commit hook environment should contain specific information."""
    //     config, _, current_version = get_config_data({"current_version": "0.1.0"})
    //     new_version = current_version.bump("minor")
    //     result_env = get_pre_commit_hook_env(config, current_version, new_version)
    //
    //     assert_os_environ_items_included(result_env)
    //     assert_scm_info_included(result_env)
    //     assert_current_version_info_included(result_env)
    //     assert_new_version_info_included(result_env)
}
