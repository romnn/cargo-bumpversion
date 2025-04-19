use crate::{command::run_command, vcs::VersionControlSystem};
use async_process::Command;
use color_eyre::eyre;
use std::path::Path;
use tempfile::TempDir;

fn random_string_of_length(length: usize) -> String {
    use rand::{Rng, distr::Alphanumeric};
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

/// An ephemeral repository that wraps a VCS.
///
/// Used for testing purposes.
pub(crate) struct EphemeralRepository<VCS> {
    inner: VCS,
    dir: TempDir,
}

impl<VCS> EphemeralRepository<VCS> {
    pub(crate) fn path(&self) -> &Path {
        self.dir.path()
    }
}

impl<VCS> EphemeralRepository<VCS>
where
    VCS: VersionControlSystem,
{
    pub(crate) async fn new() -> eyre::Result<Self> {
        Self::with_name(&random_string_of_length(10)).await
    }

    pub(crate) async fn with_name(name: &str) -> eyre::Result<Self> {
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

    // async fn add(&self, files: &[impl AsRef<Path>]) -> eyre::Result<()> {
    //     let files = files
    //         .iter()
    //         .map(|f| f.as_ref().to_string_lossy().to_string());
    //     let mut cmd = Command::new("git");
    //     cmd.arg("add");
    //     cmd.args(files);
    //     cmd.current_dir(self.path());
    //     let _ = run_command(&mut cmd).await?;
    //     Ok(())
    // }
}

impl<VCS> std::ops::Deref for EphemeralRepository<VCS> {
    type Target = VCS;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
