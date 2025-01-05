use bumpversion::Verbosity;

pub(crate) struct Logger {
    verbosity: Verbosity,
}

impl Logger {
    pub fn new(verbosity: Verbosity) -> Self {
        Self { verbosity }
    }
}

impl bumpversion::Log for Logger {
    fn log(&self, verbosity: Verbosity, message: &str) {
        if verbosity <= self.verbosity {
            println!("{}", message);
        }
    }
}
