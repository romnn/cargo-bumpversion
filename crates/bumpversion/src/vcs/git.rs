use crate::{
    command::run_command,
    config::DEFAULT_TAG_NAME,
    f_string::{OwnedPythonFormatString, OwnedValue},
    utils,
    vcs::{RevisionInfo, TagAndRevision, TagInfo, VersionControlSystem},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("command failed: {0}")]
    CommandFailed(#[from] crate::command::Error),

    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("invalid tag: {0}")]
    InvalidTag(#[from] InvalidTagError),

    #[error("failed to template {format_string}")]
    MissingArgument {
        #[source]
        source: crate::f_string::MissingArgumentError,
        format_string: OwnedPythonFormatString,
    },
}

#[derive(thiserror::Error, Debug)]
pub enum InvalidTagError {
    #[error("tag {0:?} is missing commit SHA")]
    MissingCommitSha(String),
    #[error("tag {0:?} is missing distance to latest tag")]
    MissingDistanceToLatestTag(String),
    #[error("invalid distance to latest tag for {tag:?}")]
    InvalidDistanceToLatestTag {
        #[source]
        source: std::num::ParseIntError,
        tag: String,
    },
    #[error("tag {0:?} is missing current tag")]
    MissingCurrentTag(String),
    #[error("tag {0:?} is missing version")]
    MissingVersion(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitRepository {
    path: PathBuf,
}

static FLAG_PATTERN: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
    regex::RegexBuilder::new(r"^(\(\?[aiLmsux]+\))")
        .build()
        .unwrap()
});

/// Extract the regex flags from the regex pattern.
///
/// # Returns
/// The tuple `(pattern_without flags, flags)`.
fn extract_regex_flags(pattern: &str) -> (&str, &str) {
    let bits: Vec<_> = FLAG_PATTERN.split(pattern).collect();
    dbg!(&bits);
    if bits.len() < 2 {
        (pattern, "")
    } else {
        (bits[1], bits[0])
    }
}

// pub static NEW_VERSION_PATTERN: once_cell::sync::Lazy<aho_corasick::AhoCorasick> =
//     once_cell::sync::Lazy::new(|| aho_corasick::AhoCorasick::new(["{new_version}"]).unwrap());

/// Return the version from a tag
pub fn get_version_from_tag<'a>(
    tag: &'a str,
    tag_name: &OwnedPythonFormatString,
    // tag_name: &str,
    parse_pattern: &str,
) -> Result<Option<&'a str>, Error> {
    let version_pattern = parse_pattern.replace("\\\\", "\\");
    let (version_pattern, regex_flags) = extract_regex_flags(&version_pattern);
    let OwnedPythonFormatString(values) = tag_name;
    let (prefix, suffix) = values
        .iter()
        .position(|value| value == &OwnedValue::Argument("new_version".to_string()))
        .map(|idx| {
            let prefix = &values[..idx];
            let suffix = &values[idx..];
            (prefix, suffix)
        })
        .unwrap_or_default();
    // let (prefix, suffix) = NEW_VERSION_PATTERN
    //     .find(tag_name)
    //     .map(|m| {
    //         let prefix = &tag_name[..m.start()];
    //         let suffix = &tag_name[m.end()..];
    //         (prefix, suffix)
    //     })
    //     .unwrap_or_default();

    let prefix = prefix.iter().fold(String::new(), |mut acc, value| {
        acc.push_str(&value.to_string());
        acc
    });
    let suffix = suffix.iter().fold(String::new(), |mut acc, value| {
        acc.push_str(&value.to_string());
        acc
    });

    let pattern = format!(
        "{regex_flags}{}(?P<current_version>{version_pattern}){}",
        regex::escape(&prefix),
        regex::escape(&suffix),
    );
    let tag_regex = regex::RegexBuilder::new(&pattern).build()?;
    let version = tag_regex
        .captures_iter(tag)
        .filter_map(|m| m.name("current_version"))
        .map(|m| m.as_str())
        .next();
    Ok(version)
}

pub static BRANCH_NAME_REGEX: once_cell::sync::Lazy<regex::Regex> =
    once_cell::sync::Lazy::new(|| {
        regex::RegexBuilder::new(r"([^a-zA-Z0-9]*)")
            .build()
            .unwrap()
    });

impl GitRepository {
    /// Returns a dictionary containing revision information.
    fn revision_info(&self) -> Result<Option<RevisionInfo>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["rev-parse", "--show-toplevel", "--abbrev-ref", "HEAD"])
            .current_dir(&self.path);

        let res = run_command(&mut cmd)?;
        let mut lines = res.stdout.lines().map(str::trim);
        let Some(repository_root) = lines.next().map(PathBuf::from) else {
            return Ok(None);
        };
        let Some(branch_name) = lines.next() else {
            return Ok(None);
        };
        let short_branch_name: String = BRANCH_NAME_REGEX
            .replace_all(branch_name, "")
            .to_lowercase()
            .chars()
            .take(20)
            .collect();

        Ok(Some(RevisionInfo {
            branch_name: branch_name.to_string(),
            short_branch_name,
            repository_root,
        }))
    }

    /// Get the commit info for the repo.
    ///
    /// The `tag_name` is the tag name format used to locate the latest tag.
    /// The `parse_pattern` is a regular expression pattern used to parse the version from the tag.
    fn latest_tag_info(
        &self,
        tag_name: &OwnedPythonFormatString,
        parse_pattern: &str,
    ) -> Result<Option<TagInfo>, Error> {
        let tag_pattern = tag_name
            .format(&[("new_version", "*")].into_iter().collect(), true)
            .map_err(|source| Error::MissingArgument {
                source,
                format_string: tag_name.clone(),
            })?;
        // let tag_pattern = tag_name.replace("{new_version}", "*");

        // get info about the latest tag in git
        let match_tag_pattern_flag = format!("--match={tag_pattern}");
        let mut cmd = Command::new("git");
        cmd.args([
            "describe",
            "--dirty",
            "--tags",
            "--long",
            "--abbrev=40",
            &match_tag_pattern_flag,
        ])
        .current_dir(&self.path);

        match run_command(&mut cmd) {
            Ok(tag_info) => {
                let raw_tag = tag_info.stdout;
                let mut tag_parts: Vec<&str> = raw_tag.split('-').collect();
                dbg!(&tag_parts);

                let dirty = tag_parts
                    .last()
                    .is_some_and(|t| t.trim().eq_ignore_ascii_case("dirty"));
                if dirty {
                    let _ = tag_parts.pop();
                }

                let commit_sha = tag_parts
                    .pop()
                    .ok_or_else(|| InvalidTagError::MissingCommitSha(raw_tag.clone()))?
                    .trim_left_matches("g")
                    .to_string();

                let distance_to_latest_tag = tag_parts
                    .pop()
                    .ok_or_else(|| InvalidTagError::MissingDistanceToLatestTag(raw_tag.clone()))?
                    .parse::<usize>()
                    .map_err(|source| InvalidTagError::InvalidDistanceToLatestTag {
                        source,
                        tag: raw_tag.clone(),
                    })?;
                let current_tag = tag_parts.join("-");
                let version = get_version_from_tag(&current_tag, tag_name, parse_pattern)?;
                let current_numeric_version = current_tag.trim_left_matches("v").to_string();
                let current_version = version
                    .unwrap_or(current_numeric_version.as_str())
                    .to_string();

                tracing::debug!(
                    dirty,
                    commit_sha,
                    distance_to_latest_tag,
                    current_tag,
                    version,
                    current_numeric_version,
                    current_version
                );

                Ok(Some(TagInfo {
                    dirty,
                    commit_sha,
                    distance_to_latest_tag,
                    current_tag,
                    current_version,
                }))
            }
            Err(err) => {
                if let crate::command::Error::Failed { ref output, .. } = err {
                    if output
                        .stderr
                        .contains("No names found, cannot describe anything")
                    {
                        return Ok(None);
                    }
                }
                Err(err.into())
            }
        }
    }
}

impl VersionControlSystem for GitRepository {
    type Error = Error;

    fn open(path: impl Into<PathBuf>) -> Result<Self, Error> {
        Ok(Self { path: path.into() })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    // envs<I, K, V>(&mut self, vars: I) -> &mut Command
    // where
    //     I: IntoIterator<Item = (K, V)>,
    //     K: AsRef<OsStr>,
    //     V: AsRef<OsStr>,

    fn commit<A, E, AS, EK, EV>(
        &self,
        message: &str,
        extra_args: A,
        // extra_args: Option<impl IntoIterator<Item = S>>,
        // env: &HashMap<&str, &str>,
        env: E,
        // env: &HashMap<&str, &str>,
    ) -> Result<(), Error>
    where
        A: IntoIterator<Item = AS>,
        E: IntoIterator<Item = (EK, EV)>,
        AS: AsRef<std::ffi::OsStr>,
        EK: AsRef<std::ffi::OsStr>,
        EV: AsRef<std::ffi::OsStr>,
    {
        use std::io::Write;

        let tmp = tempfile::TempDir::new()?;
        let tmp_file_path = tmp.path().join("commit-message.txt");
        let mut tmp_file = std::fs::File::create(&tmp_file_path)?;
        tmp_file.write_all(message.as_bytes())?;

        let mut cmd = Command::new("git");
        cmd.arg("commit");
        cmd.arg("-F");
        cmd.arg(tmp_file_path.to_string_lossy().to_string());
        // if let Some(extra_args) = extra_args {
        cmd.args(extra_args);
        cmd.envs(env);
        // }
        cmd.current_dir(&self.path);
        let commit_output = run_command(&mut cmd)?;
        dbg!(&commit_output);
        Ok(())
    }

    fn add(&self, files: &[impl AsRef<Path>]) -> Result<(), Error> {
        let files = files
            .iter()
            .map(|f| f.as_ref().to_string_lossy().to_string());
        let mut cmd = Command::new("git");
        cmd.arg("add")
            .arg("--update")
            .args(files)
            .current_dir(&self.path);
        let add_output = run_command(&mut cmd)?;
        dbg!(&add_output);
        Ok(())
    }

    fn dirty_files(&self) -> Result<Vec<PathBuf>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["status", "-u", "--porcelain"])
            .current_dir(&self.path);

        let status_output = run_command(&mut cmd)?;
        let dirty = status_output
            .stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| !line.starts_with("??"))
            .filter_map(|line| line.split_once(' '))
            .map(|(_, file)| self.path().join(file))
            .collect();
        Ok(dirty)
    }

    fn tag(&self, name: &str, message: Option<&str>, sign: bool) -> Result<(), Error> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.path);
        cmd.args(["tag", name]);
        if sign {
            cmd.arg("--sign");
        }
        if let Some(message) = message {
            cmd.args(["--message", message]);
        }
        let tag_output = run_command(&mut cmd)?;
        dbg!(&tag_output);
        Ok(())
    }

    fn tags(&self) -> Result<Vec<String>, Error> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.path);
        cmd.args(["tag", "--list"]);
        let output = run_command(&mut cmd)?;
        Ok(output
            .stdout
            .lines()
            .map(|line| line.trim().to_string())
            .collect())
    }

    fn latest_tag_and_revision(
        &self,
        tag_name: &OwnedPythonFormatString,
        parse_pattern: &str,
    ) -> Result<TagAndRevision, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["update-index", "--refresh", "-q"])
            .current_dir(&self.path);
        if let Err(err) = run_command(&mut cmd) {
            tracing::debug!("failed to update git index: {err}");
        }

        let tag = self.latest_tag_info(tag_name, parse_pattern)?;
        let revision = self.revision_info().ok().flatten();

        Ok(TagAndRevision { tag, revision })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::run_command,
        tests::sim_assert_eq_sorted,
        utils,
        vcs::{git, temp, VersionControlSystem},
    };
    use color_eyre::eyre;
    use regex::Regex;
    use std::collections::HashMap;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use tempfile::TempDir;

    #[ignore = "wip"]
    #[test]
    fn test_create_empty_git_repo() -> eyre::Result<()> {
        let repo: temp::GitRepository<git::GitRepository> = temp::GitRepository::new()?;
        let status = run_command(
            Command::new("git")
                .args(["status"])
                .current_dir(repo.path()),
        )?;
        assert!(utils::contains(&status.stdout, "No commits yet")?.is_some());
        Ok(())
    }

    #[ignore = "wip"]
    #[test]
    fn test_tag() -> eyre::Result<()> {
        let repo: temp::GitRepository<git::GitRepository> = temp::GitRepository::new()?;
        let tags = vec![
            None,
            Some(("tag1", Some("tag1 message"))),
            Some(("tag2", Some("tag2 message"))),
        ];
        // add a single file so we can commit and get a HEAD
        let initial_file = repo.path().join("README.md");
        std::fs::File::create(&initial_file)?.write_all(b"Hello, world!")?;

        repo.add(&[initial_file])?;
        repo.commit::<_, _, &str, &str, &str>("initial commit", [], [])?;
        similar_asserts::assert_eq!(repo.dirty_files()?.len(), 0);

        for (tag, previous) in tags[1..].iter().zip(&tags) {
            dbg!(previous);
            dbg!(tag);
            // let latest = repo.latest_tag_info(None)?.map(|t| t.current_version);
            // let previous = previous.map(|t| t.0.to_string());
            // similar_asserts::assert_eq!(&previous, &latest);
            // if let Some((tag_name, tag_message)) = *tag {
            //     repo.tag(tag_name, tag_message, false)?;
            // }
        }
        Ok(())
    }

    #[ignore = "wip"]
    #[test]
    fn test_dirty_tree() -> eyre::Result<()> {
        let repo: temp::GitRepository<git::GitRepository> = temp::GitRepository::new()?;
        similar_asserts::assert_eq!(repo.dirty_files()?.len(), 0);

        // add some dirty files
        let mut dirty_files: Vec<PathBuf> = ["foo.txt", "dir/bar.txt"]
            .iter()
            .map(|f| repo.path().join(f))
            .collect();

        for dirty_file in &dirty_files {
            crate::utils::create_dirs(dirty_file);
            let mut file = std::fs::File::create(dirty_file)?;
            file.write_all(b"Hello, world!")?;
        }
        similar_asserts::assert_eq!(repo.dirty_files()?.len(), 0);

        // track first file
        repo.add(&dirty_files[0..1]);
        sim_assert_eq_sorted!(repo.dirty_files()?, dirty_files[0..1]);

        // track all files
        repo.add(&dirty_files);
        sim_assert_eq_sorted!(repo.dirty_files()?, dirty_files);
        Ok(())
    }
}
