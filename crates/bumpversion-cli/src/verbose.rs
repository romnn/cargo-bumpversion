use bumpversion::logging::Verbosity;
use colored::Colorize;

pub(crate) struct Logger {
    dry_run: bool,
    verbosity: Verbosity,
}

impl Logger {
    pub fn new(verbosity: Verbosity) -> Self {
        Self {
            verbosity,
            dry_run: false,
        }
    }

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
            println!("{}", message);
        }
    }
}
