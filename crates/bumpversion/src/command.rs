//! Utilities for running and checking external commands.
use async_process::{Command, ExitStatus};

/// The captured output of a child process.
///
/// Contains `stdout`, `stderr`, and the exit `status`.
/// Captured output of a child process execution.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// Standard output of the command.
    pub stdout: String,
    /// Standard error of the command.
    pub stderr: String,
    /// Exit status of the process.
    pub status: ExitStatus,
}

impl From<async_process::Output> for Output {
    fn from(output: async_process::Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
            status: output.status,
        }
    }
}

/// Errors that can occur when running an external process.
/// Errors that can occur when running external commands.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// I/O error while spawning or capturing the process.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    // TODO: into eyre here!
    /// The process exited with a non-zero status code.
    #[error(
        "`{}` failed with code {}:\n\n--- Stdout:\n {}\n--- Stderr:\n {}",
        command,
        output.status.code().unwrap_or(1),
        output.stdout,
        output.stderr
    )]
    Failed { 
        /// Debug representation of the command that was run.
        command: String, 
        /// Captured output including status, stdout, stderr.
        output: Output 
    },
}

/// Check that the process exited successfully, returning an error otherwise.
///
/// # Errors
/// Returns `Error::Failed` if the exit status indicates failure.
/// Check that a process exited successfully, returning an error otherwise.
///
/// # Errors
/// Returns `Error::Failed` if the exit status indicates failure.
pub fn check_exit_status(cmd: &Command, output: &async_process::Output) -> Result<(), Error> {
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Failed {
            command: format!("{cmd:?}"),
            output: output.clone().into(),
        })
    }
}

/// Execute the given command, capturing output and checking exit status.
///
/// # Errors
/// Returns `Error::Io` for I/O errors or `Error::Failed` if the process exits with non-zero status.
/// Execute the given command, capturing stdout/stderr and checking exit code.
///
/// # Errors
/// Returns `Error::Io` for I/O failures or `Error::Failed` for non-zero exits.
pub async fn run_command(cmd: &mut Command) -> Result<Output, Error> {
    let output = cmd.output().await?;
    check_exit_status(cmd, &output)?;
    Ok(output.into())
}
