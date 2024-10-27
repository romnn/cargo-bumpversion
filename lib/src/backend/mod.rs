use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

#[cfg(feature = "native")]
pub mod native;

#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub dirty: bool,
    pub commit_sha: String,
    pub distance_to_latest_tag: usize,
    pub current_version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutput {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

impl From<Output> for CommandOutput {
    fn from(output: Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
            status: output.status,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "{} failed with code {}:\n\n--- Stdout:\n {}\n--- Stderr:\n {}",
        command,
        output.status.code().unwrap_or(1),
        output.stdout,
        output.stderr
    )]
    Failed {
        command: String,
        output: CommandOutput,
    },
}

fn check_exit_status(cmd: &Command, output: Output) -> Result<(), CommandError> {
    if output.status.success() {
        Ok(())
    } else {
        Err(CommandError::Failed {
            command: format!("{:?}", cmd),
            output: output.into(),
        })
    }
}

fn run_command(cmd: &mut Command) -> Result<CommandOutput, CommandError> {
    let output = cmd.output()?;
    check_exit_status(&cmd, output.clone())?;
    Ok(output.into())
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("utf decode error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("command failed: {0}")]
    CommandFailed(#[from] CommandError),
}

pub trait GitRepository {
    fn open<P: Into<PathBuf>>(path: P) -> Result<Self, Error>
    where
        Self: Sized;

    fn repo_dir(&self) -> &Path;

    fn add<P>(&self, files: &[P]) -> Result<(), Error>
    where
        P: AsRef<Path>;

    fn commit(&self, message: &str) -> Result<(), Error>;

    fn tag(&self, name: &str, message: Option<&str>, sign: bool) -> Result<(), Error>;

    fn dirty_files(&self) -> Result<Vec<PathBuf>, Error>;

    fn latest_tag_info(&self, pattern: Option<&str>) -> Result<Option<Tag>, Error>;

    fn run_command(&self, cmd: &mut Command) -> Result<CommandOutput, CommandError> {
        cmd.current_dir(self.repo_dir());
        run_command(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils;
    use color_eyre::eyre;
    use pretty_assertions::assert_eq;
    use regex::Regex;
    use std::fs;
    use std::io::Write;
    use std::process::Command;
    use tempdir::TempDir;

    #[inline]
    fn random_string_of_length(length: usize) -> String {
        use rand::{distributions::Alphanumeric, Rng};
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(length)
            .map(char::from)
            .collect()
    }

    #[inline]
    fn create_dirs<P: AsRef<Path>>(path: P) -> eyre::Result<()> {
        let path = path.as_ref();
        let dir = if path.extension().is_some() {
            path.parent()
                .ok_or(eyre::eyre!("no parent for {:?}", path))?
        } else {
            path
        };
        fs::create_dir_all(&dir)?;
        Ok(())
    }

    macro_rules! assert_eq_vec {
        ($left:expr, $right:expr $(,)?) => {
            $left.sort();
            $right.sort();
            assert_eq!($left, $right);
        };
        ($left:expr, $right:expr, $($arg:tt)+) => {
            $left.sort();
            $right.sort();
            assert_eq!($left, $right, $($arg)+);
        };
    }

    struct TempGitRepository<Repo> {
        inner: Repo,
        dir: TempDir,
    }

    impl<Repo> TempGitRepository<Repo>
    where
        Repo: GitRepository,
    {
        pub fn new() -> eyre::Result<Self> {
            Self::with_name(&random_string_of_length(10))
        }

        pub fn with_name(name: &str) -> eyre::Result<Self> {
            let dir = TempDir::new(name)?;
            Self::init(dir.path())?;
            let inner = Repo::open(dir.path())?;
            Ok(Self { inner, dir })
        }

        fn init<P: AsRef<Path>>(path: P) -> eyre::Result<()> {
            let path = path.as_ref();
            fs::create_dir_all(&path)?;
            let _ = run_command(Command::new("git").args(["init"]).current_dir(&path))?;
            Ok(())
        }

        fn add<P>(&self, files: &[P]) -> eyre::Result<()>
        where
            P: AsRef<Path>,
        {
            let files = files
                .iter()
                .map(|f| f.as_ref().to_string_lossy().to_string());
            let _ = self.run_command(Command::new("git").arg("add").args(files))?;
            Ok(())
        }
    }

    impl<Repo> std::ops::Deref for TempGitRepository<Repo> {
        type Target = Repo;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    #[test]
    fn test_create_empty_git_repo() -> eyre::Result<()> {
        let repo: TempGitRepository<native::NativeGitRepository> = TempGitRepository::new()?;
        let status = repo.run_command(Command::new("git").args(["status"]))?;
        assert!(utils::contains(&status.stdout, "No commits yet")?.is_some());
        Ok(())
    }

    #[test]
    fn test_tag() -> eyre::Result<()> {
        let repo: TempGitRepository<native::NativeGitRepository> = TempGitRepository::new()?;
        let tags = vec![
            None,
            Some(("tag1", Some("tag1 message"))),
            Some(("tag2", Some("tag2 message"))),
        ];
        // add a single file so we can commit and get a HEAD
        let initial_file = repo.repo_dir().join("README.md");
        fs::File::create(&initial_file)?.write_all(b"Hello, world!")?;

        repo.add(&[initial_file])?;
        repo.commit("initial commit")?;
        assert_eq!(repo.dirty_files()?.len(), 0);

        for (tag, previous) in tags[1..].iter().zip(&tags) {
            dbg!(previous);
            dbg!(tag);
            let latest = repo.latest_tag_info(None)?.map(|t| t.current_version);
            let previous = previous.map(|t| t.0.to_string());
            assert_eq!(&previous, &latest);
            if let Some((tag_name, tag_message)) = *tag {
                repo.tag(tag_name, tag_message, false)?;
            }
        }
        Ok(())
    }

    #[test]
    fn test_dirty_tree() -> eyre::Result<()> {
        let repo: TempGitRepository<native::NativeGitRepository> = TempGitRepository::new()?;
        assert_eq!(repo.dirty_files()?.len(), 0);

        // add some dirty files
        let mut dirty_files: Vec<PathBuf> = vec!["foo.txt", "dir/bar.txt"]
            .iter()
            .map(|f| repo.repo_dir().join(f))
            .collect();

        for dirty_file in dirty_files.iter() {
            create_dirs(&dirty_file);
            let mut file = fs::File::create(dirty_file)?;
            file.write_all(b"Hello, world!")?;
        }
        assert_eq!(repo.dirty_files()?.len(), 0);

        // track first file
        repo.add(&dirty_files[0..1]);
        assert_eq_vec!(repo.dirty_files()?, dirty_files[0..1]);

        // track all files
        repo.add(&dirty_files);
        assert_eq_vec!(repo.dirty_files()?, dirty_files);
        Ok(())
    }
}
