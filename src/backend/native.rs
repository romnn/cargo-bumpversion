use super::{CommandError, Error, GitRepository, Tag};
use crate::utils;
use std::fs;
use std::io::BufRead;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempdir::TempDir;

pub struct NativeGitRepository {
    repo_dir: PathBuf,
}

impl GitRepository for NativeGitRepository {
    fn open<P: Into<PathBuf>>(path: P) -> Result<Self, Error> {
        Ok(Self {
            repo_dir: path.into(),
        })
    }

    fn repo_dir(&self) -> &Path {
        &self.repo_dir
    }

    fn commit(&self, message: &str) -> Result<(), Error> {
        let tmp = TempDir::new("")?;
        let tmp_file_path = tmp.path().join("commit-message.txt");
        let mut tmp_file = fs::File::create(&tmp_file_path)?;
        tmp_file.write_all(message.as_bytes())?;

        let commit_output = self.run_command(
            Command::new("git")
                .arg("commit")
                .arg("-F")
                .arg(tmp_file_path.to_string_lossy().to_string())
                // need extra args?
                .env("HGENCODING", "utf-8"),
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
        cmd.arg("add").arg("--update").args(files);
        let add_output = self.run_command(&mut cmd)?;
        dbg!(&add_output);
        Ok(())
    }

    fn dirty_files(&self) -> Result<Vec<PathBuf>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["status", "-u", "--porcelain"]);
        let status_output = self.run_command(&mut cmd)?;
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
        cmd.args(["tag", name]);
        if sign {
            cmd.arg("--sign");
        }
        if let Some(message) = message {
            cmd.args(["--message", message]);
        }
        let tag_output = self.run_command(&mut cmd)?;
        dbg!(&tag_output);
        Ok(())
    }

    fn latest_tag_info(&self, pattern: Option<&str>) -> Result<Option<Tag>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["update-index", "--refresh"]);
        let _ = self.run_command(&mut cmd)?;

        let mut cmd = Command::new("git");
        cmd.args(["describe", "--dirty", "--tags", "--long", "--abbrev=40"]);
        if let Some(pattern) = pattern {
            cmd.arg("--match=v*");
        }
        match self.run_command(&mut cmd) {
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
                if let CommandError::Failed { ref output, .. } = err {
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
