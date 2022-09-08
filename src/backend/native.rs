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

        let output = self.run_command(
            Command::new("git")
                .arg("commit")
                .arg("-F")
                .arg(tmp_file_path.to_string_lossy().to_string())
                // .args(files)
                .env("HGENCODING", "utf-8"),
        )?;
        println!("{:?}", output);
        Ok(())
    }

    fn add<P>(&self, files: &[P]) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        let files = files
            .iter()
            .map(|f| f.as_ref().to_string_lossy().to_string());
        let _ = self.run_command(Command::new("git").arg("add").arg("--update").args(files))?;
        Ok(())
    }

    fn dirty_files(&self) -> Result<Vec<PathBuf>, Error> {
        let status = self.run_command(Command::new("git").args(["status", "-u", "--porcelain"]))?;
        let dirty = status
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
        let output = self.run_command(&mut cmd)?;
        println!("{:?} output {:?}", cmd, &output);
        Ok(())
    }

    fn latest_tag_info(&self, pattern: Option<&str>) -> Result<Option<Tag>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(["update-index", "--refresh"]);
        let _ = self.run_command(&mut cmd)?;
        // .current_dir(&self.repo_path()),

        // let _ = Command::new("git")
        //     .arg("update-index")
        //     .arg("--refresh")
        //     .output();
        let mut cmd = Command::new("git");
        cmd.args(["describe", "--dirty", "--tags", "--long", "--abbrev=40"]);
        if let Some(pattern) = pattern {
            cmd.arg("--match=v*");
        }
        println!("get tags");
        match self.run_command(&mut cmd) {
            Ok(tag_info) => {
                let mut tag_out: Vec<&str> = tag_info.stdout.split("-").collect();
                println!("tags: {:?}", &tag_out);

                let mut dirty = false;
                if let Some(t) = tag_out.last() {
                    if t.trim() == "dirty" {
                        dirty = true;
                        tag_out.pop().unwrap();
                    }
                }
                // let dirty = if tag_out
                //     .last()
                //     .map(|t| *t)
                //     .map(str::trim)
                //     .map(|t| t == "dirty")
                //     .unwrap_or(false)
                // {
                //     true
                // } else {
                //     false
                // };

                // let mut Tag = Tag::default();
                // info["commit_sha"] = describe_out.pop().lstrip("g")
                // info["distance_to_latest_tag"] = int(describe_out.pop())
                // info["current_version"] = "-".join(describe_out).lstrip("v")

                let commit_sha = tag_out.pop().unwrap().trim_left_matches("g").to_string();
                let distance_to_latest_tag = tag_out.pop().unwrap().parse::<usize>().unwrap();
                let current_version = tag_out.join("-").trim_left_matches("v").to_string();
                Ok(Some(Tag {
                    dirty,
                    commit_sha,
                    distance_to_latest_tag,
                    current_version,
                }))
            }
            // Err(CommandError::Io(err)) => {
            Err(err) => {
                println!("get tag error {:?}", &err);
                if let CommandError::Failed { ref output, .. } = err {
                    if utils::contains(&output.stderr, "No names found, cannot describe anything")
                        .map(|m| m.is_some())
                        .unwrap_or(false)
                    {
                        return Ok(None);
                    }
                    // else {
                    //     Err(err.into())
                }
                Err(err.into())
            }
        }

        // let tag_info = Command::new("git")
        //     .arg("describe")
        //     .arg("--dirty")
        //     .arg("--tags")
        //     .arg("--long")
        //     .arg("--abbrev=40")
        //     .arg("--match=v*")
        //     .stderr(Stdio::piped())
        //     .output()?;
        // assert!(tag_info.status.success());
        // let tag_info = std::str::from_utf8(&tag_info.stdout)?;
        // let tags = String::from_utf8_lossy(&tag_info.stdout).split("-");

        // return info
        // let dirty = status
        //     .lines()
        //     .map(|f| f.trim())
        //     .filter(|f| !f.starts_with("??"))
        //     .map(|f| self.repo_dir.join(f))
        //     .collect();
        // Ok(tag)
    }
}
