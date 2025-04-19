//! Console logger for bumpversion CLI, implementing the `Log` trait.
//!
//! Prints messages to stdout, with optional dry-run prefix.
use bumpversion::logging::Verbosity;
use colored::Colorize;

/// Logger for CLI output, respects verbosity and dry-run mode.
pub(crate) struct Logger {
    /// If true, prefix messages indicating no file changes.
    dry_run: bool,
    /// Current verbosity level threshold.
    verbosity: Verbosity,
}

impl Logger {
    /// Create a new `Logger` with the given verbosity.
    pub fn new(verbosity: Verbosity) -> Self {
        Self {
            verbosity,
            dry_run: false,
        }
    }

    /// Enable or disable dry-run mode, which prefixes output.
    pub fn dry_run(mut self, enabled: bool) -> Self {
        self.dry_run = enabled;
        self
    }
}

impl bumpversion::logging::Log for Logger {
    fn log(&self, verbosity: Verbosity, message: &str) {
        if verbosity > self.verbosity {
            return;
        }
        if self.dry_run {
            println!("{}{}", " [DRY-RUN] ".blue(), message);
        } else {
            println!("{message}");
        }
    }
}
