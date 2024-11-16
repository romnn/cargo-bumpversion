use crate::{backend::GitBackend, command::run_command, utils};
use color_eyre::eyre;
use regex::Regex;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tempdir::TempDir;

fn random_string_of_length(length: usize) -> String {
    use rand::{distributions::Alphanumeric, Rng};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub struct GitRepository<R> {
    inner: R,
    dir: TempDir,
}

impl<R> GitRepository<R>
where
    R: GitBackend,
{
    pub fn new() -> eyre::Result<Self> {
        Self::with_name(&random_string_of_length(10))
    }

    pub fn with_name(name: &str) -> eyre::Result<Self> {
        let dir = TempDir::new(name)?;
        Self::init(dir.path())?;
        let inner = R::open(dir.path())?;
        Ok(Self { inner, dir })
    }

    fn init<P: AsRef<Path>>(path: P) -> eyre::Result<()> {
        let path = path.as_ref();
        std::fs::create_dir_all(&path)?;
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
        let _ = run_command(
            Command::new("git")
                .arg("add")
                .args(files)
                .current_dir(self.repo_dir()),
        )?;
        Ok(())
    }
}

impl<Repo> std::ops::Deref for GitRepository<Repo> {
    type Target = Repo;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
