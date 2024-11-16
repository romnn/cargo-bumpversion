#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub status: std::process::ExitStatus,
}

impl From<std::process::Output> for Output {
    fn from(output: std::process::Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
            status: output.status,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "{} failed with code {}:\n\n--- Stdout:\n {}\n--- Stderr:\n {}",
        command,
        output.status.code().unwrap_or(1),
        output.stdout,
        output.stderr
    )]
    Failed { command: String, output: Output },
}

pub fn check_exit_status(
    cmd: &std::process::Command,
    output: std::process::Output,
) -> Result<(), Error> {
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Failed {
            command: format!("{:?}", cmd),
            output: output.into(),
        })
    }
}

pub fn run_command(cmd: &mut std::process::Command) -> Result<Output, Error> {
    let output = cmd.output()?;
    check_exit_status(&cmd, output.clone())?;
    Ok(output.into())
}
