use crate::{
    backend::{self, GitBackend},
    command::run_command,
    utils, Tag,
};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempdir::TempDir;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("utf decode error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("command failed: {0}")]
    CommandFailed(#[from] crate::command::Error),
}

pub struct GitRepository {
    repo_dir: PathBuf,
}

// impl GitRepository {
//     pub fn native<P: Into<PathBuf>>(path: P) -> Result<Self, Error> {
//         Self::open(path)
//     }
// }

impl backend::GitBackend for GitRepository {
    type Error = Error;

    fn open<P: Into<PathBuf>>(path: P) -> Result<Self, Error> {
        Ok(Self {
            repo_dir: path.into(),
        })
    }

    fn repo_dir(&self) -> &Path {
        &self.repo_dir
    }

    fn commit(&self, message: &str) -> Result<(), Error> {
        use std::io::Write;
        let tmp = TempDir::new("")?;
        let tmp_file_path = tmp.path().join("commit-message.txt");
        let mut tmp_file = std::fs::File::create(&tmp_file_path)?;
        tmp_file.write_all(message.as_bytes())?;

        let commit_output = run_command(
            Command::new("git")
                .arg("commit")
                .arg("-F")
                .arg(tmp_file_path.to_string_lossy().to_string())
                // need extra args?
                .env("HGENCODING", "utf-8")
                .current_dir(&self.repo_dir),
        )?;
        dbg!(&commit_output);
        Ok(())
    }

    fn add<P>(&self, files: &[P]) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        let files = files
            .iter()
            .map(|f| f.as_ref().to_string_lossy().to_string());
        let mut cmd = Command::new("git");
        cmd.arg("add")
            .arg("--update")
            .args(files)
            .current_dir(&self.repo_dir);
        let add_output = run_command(&mut cmd)?;
        dbg!(&add_output);
        Ok(())
    }

    fn dirty_files(&self) -> Result<Vec<PathBuf>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["status", "-u", "--porcelain"])
            .current_dir(&self.repo_dir);
        let status_output = run_command(&mut cmd)?;
        dbg!(&status_output);
        let dirty = status_output
            .stdout
            .lines()
            .filter_map(|f| f.trim().split_once(' '))
            .filter(|(status, f)| status.trim() != "??")
            .map(|(status, f)| self.repo_dir().join(f.trim()))
            .collect();
        Ok(dirty)
    }

    fn tag(&self, name: &str, message: Option<&str>, sign: bool) -> Result<(), Error> {
        let mut cmd = Command::new("git");
        cmd.args(["tag", name]).current_dir(&self.repo_dir);
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

    fn latest_tag_info(&self, pattern: Option<&str>) -> Result<Option<Tag>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["update-index", "--refresh"])
            .current_dir(&self.repo_dir);
        let _ = run_command(&mut cmd)?;

        let mut cmd = Command::new("git");
        cmd.args(["describe", "--dirty", "--tags", "--long", "--abbrev=40"])
            .current_dir(&self.repo_dir);
        if let Some(pattern) = pattern {
            cmd.arg("--match=v*");
        }
        match run_command(&mut cmd) {
            Ok(tag_info) => {
                let mut tag_parts: Vec<&str> = tag_info.stdout.split("-").collect();
                dbg!(&tag_parts);

                let mut dirty = false;
                if let Some(t) = tag_parts.last() {
                    if t.trim() == "dirty" {
                        dirty = true;
                        tag_parts.pop().unwrap();
                    }
                }

                let commit_sha = tag_parts.pop().unwrap().trim_left_matches("g").to_string();
                let distance_to_latest_tag = tag_parts.pop().unwrap().parse::<usize>().unwrap();
                let current_version = tag_parts.join("-").trim_left_matches("v").to_string();
                Ok(Some(Tag {
                    dirty,
                    commit_sha,
                    distance_to_latest_tag,
                    current_version,
                }))
            }
            Err(err) => {
                if let crate::command::Error::Failed { ref output, .. } = err {
                    // TODO: make this a static regex
                    if utils::contains(&output.stderr, "No names found, cannot describe anything")
                        .map(|m| m.is_some())
                        .unwrap_or(false)
                    {
                        return Ok(None);
                    }
                }
                Err(err.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        backend::{native, temp, GitBackend},
        command::run_command,
        tests::assert_eq_vec,
        utils,
    };
    use color_eyre::eyre;
    use regex::Regex;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use tempdir::TempDir;

    #[test]
    fn test_create_empty_git_repo() -> eyre::Result<()> {
        let repo: temp::GitRepository<native::GitRepository> = temp::GitRepository::new()?;
        let status = run_command(
            Command::new("git")
                .args(["status"])
                .current_dir(repo.repo_dir()),
        )?;
        assert!(utils::contains(&status.stdout, "No commits yet")?.is_some());
        Ok(())
    }

    #[test]
    fn test_tag() -> eyre::Result<()> {
        let repo: temp::GitRepository<native::GitRepository> = temp::GitRepository::new()?;
        let tags = vec![
            None,
            Some(("tag1", Some("tag1 message"))),
            Some(("tag2", Some("tag2 message"))),
        ];
        // add a single file so we can commit and get a HEAD
        let initial_file = repo.repo_dir().join("README.md");
        std::fs::File::create(&initial_file)?.write_all(b"Hello, world!")?;

        repo.add(&[initial_file])?;
        repo.commit("initial commit")?;
        similar_asserts::assert_eq!(repo.dirty_files()?.len(), 0);

        for (tag, previous) in tags[1..].iter().zip(&tags) {
            dbg!(previous);
            dbg!(tag);
            let latest = repo.latest_tag_info(None)?.map(|t| t.current_version);
            let previous = previous.map(|t| t.0.to_string());
            similar_asserts::assert_eq!(&previous, &latest);
            if let Some((tag_name, tag_message)) = *tag {
                repo.tag(tag_name, tag_message, false)?;
            }
        }
        Ok(())
    }

    #[test]
    fn test_dirty_tree() -> eyre::Result<()> {
        let repo: temp::GitRepository<native::GitRepository> = temp::GitRepository::new()?;
        similar_asserts::assert_eq!(repo.dirty_files()?.len(), 0);

        // add some dirty files
        let mut dirty_files: Vec<PathBuf> = vec!["foo.txt", "dir/bar.txt"]
            .iter()
            .map(|f| repo.repo_dir().join(f))
            .collect();

        for dirty_file in dirty_files.iter() {
            crate::utils::create_dirs(&dirty_file);
            let mut file = std::fs::File::create(dirty_file)?;
            file.write_all(b"Hello, world!")?;
        }
        similar_asserts::assert_eq!(repo.dirty_files()?.len(), 0);

        // track first file
        repo.add(&dirty_files[0..1]);
        assert_eq_vec!(repo.dirty_files()?, dirty_files[0..1]);

        // track all files
        repo.add(&dirty_files);
        assert_eq_vec!(repo.dirty_files()?, dirty_files);
        Ok(())
    }
}
