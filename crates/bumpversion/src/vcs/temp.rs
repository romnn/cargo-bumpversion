use crate::{command::run_command, vcs::VersionControlSystem};
use async_process::Command;
use color_eyre::eyre;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

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
    pub async fn new() -> eyre::Result<Self> {
        Self::with_name(&random_string_of_length(10)).await
    }

    pub async fn with_name(name: &str) -> eyre::Result<Self> {
        let dir = TempDir::with_prefix(name)?;
        Self::init(dir.path()).await?;
        let inner = VCS::open(dir.path())?;
        Ok(Self { inner, dir })
    }

    async fn init(path: &Path) -> eyre::Result<()> {
        tokio::fs::create_dir_all(path).await?;
        let mut cmd = Command::new("git");
        cmd.args(["init"]);
        cmd.current_dir(path);
        let _ = run_command(&mut cmd).await?;
        Ok(())
    }

    async fn add(&self, files: &[impl AsRef<Path>]) -> eyre::Result<()> {
        let files = files
            .iter()
            .map(|f| f.as_ref().to_string_lossy().to_string());
        let mut cmd = Command::new("git");
        cmd.arg("add");
        cmd.args(files);
        cmd.current_dir(self.path());
        let _ = run_command(&mut cmd).await?;
        Ok(())
    }
}

impl<Repo> std::ops::Deref for GitRepository<Repo> {
    type Target = Repo;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
