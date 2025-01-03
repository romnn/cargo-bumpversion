use crate::{command::run_command, utils, vcs::VersionControlSystem};
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

pub struct GitRepository<VCS> {
    inner: VCS,
    dir: TempDir,
}

impl<VCS> GitRepository<VCS>
where
    VCS: VersionControlSystem,
{
    pub fn new() -> eyre::Result<Self> {
        Self::with_name(&random_string_of_length(10))
    }

    pub fn with_name(name: &str) -> eyre::Result<Self> {
        let dir = TempDir::new(name)?;
        Self::init(dir.path())?;
        let inner = VCS::open(dir.path())?;
        Ok(Self { inner, dir })
    }

    fn init(path: &Path) -> eyre::Result<()> {
        std::fs::create_dir_all(path)?;
        let _ = run_command(Command::new("git").args(["init"]).current_dir(&path))?;
        Ok(())
    }

    fn add(&self, files: &[impl AsRef<Path>]) -> eyre::Result<()> {
        let files = files
            .iter()
            .map(|f| f.as_ref().to_string_lossy().to_string());
        let _ = run_command(
            Command::new("git")
                .arg("add")
                .args(files)
                .current_dir(self.path()),
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
