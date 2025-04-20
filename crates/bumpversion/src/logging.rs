//! Logging utilities for formatting version and change output based on verbosity.
//!
//! Provides traits and helpers for displaying modifications and hooks.
use crate::version::Version;
use colored::{Color, Colorize};

/// Controls level of detail emitted by loggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Verbosity {
    /// No logs will be emitted.
    Off = 0,
    /// Minimal output, e.g., current/new version and file modifications.
    Low = 1,
    /// Show diffs and more detailed logs.
    Medium = 2,
    /// Verbose debug-level output.
    High = 3,
}

impl From<u8> for Verbosity {
    fn from(value: u8) -> Self {
        match value {
            0 => Verbosity::Off,
            1 => Verbosity::Low,
            2 => Verbosity::Medium,
            _ => Verbosity::High,
        }
    }
}

/// A no-op logger implementation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoOpLogger {}

impl Log for NoOpLogger {
    fn log(&self, _: Verbosity, _: &str) {}
}

/// A `tracing` based logger implementation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TracingLogger {
    /// The maximum verbosity.
    ///
    /// Only messages with lower or equal verbosity will be logged.
    verbosity: Verbosity,
}

impl TracingLogger {
    #[must_use]
    pub fn new(verbosity: Verbosity) -> Self {
        Self { verbosity }
    }
}

impl Log for TracingLogger {
    fn log(&self, verbosity: Verbosity, message: &str) {
        if verbosity > self.verbosity {
            return;
        }
        tracing::info!("{message}");
    }
}

/// Abstraction for logger implementations.
///
/// Provides a method to emit log messages at a given `Verbosity` level.
pub trait Log {
    /// Log a message if `verbosity` is within the configured level.
    fn log(&self, verbosity: Verbosity, message: &str);
}

/// Extension methods on `Log` for common bumpversion log patterns.
pub trait LogExt {
    /// Log a file modification, including search/replace details and diff if any.
    fn log_modification(
        &self,
        path: &std::path::Path,
        modification: Option<crate::files::Modification>,
    );

    /// Log configured hooks with their names (e.g., 'setup', 'pre-commit').
    fn log_hooks(&self, hook_name: &str, hooks: &[String]);
}

impl<T> LogExt for T
where
    T: Log,
{
    fn log_modification(
        &self,
        path: &std::path::Path,
        modification: Option<crate::files::Modification>,
    ) {
        self.log(
            Verbosity::Low,
            &format!("{}", format!("[{}]", path.to_string_lossy()).magenta()),
        );

        let Some(modification) = modification else {
            self.log(Verbosity::Low, "\tnot modified");
            return;
        };
        let (search_color, replace_color) = (Color::Red, Color::Green);

        let diff = modification.diff(None);
        for replacement in modification.replacements {
            let search = unescape(&replacement.search);
            let replace = unescape(&replacement.replace);
            self.log(
                Verbosity::Low,
                &format!(
                    "\treplacing `{}` ({}) with `{}` ({})",
                    replacement.search_pattern.color(search_color),
                    search.color(search_color).dimmed(),
                    replacement.replace_pattern.color(replace_color),
                    replace.color(replace_color).dimmed(),
                ),
            );
        }
        if let Some(diff) = diff {
            self.log(Verbosity::Low, "");
            for line in diff.lines() {
                let mut line = format!("\t{line}");
                line.push_str("\x1b[0;0m"); // reset all styles at end of line
                self.log(Verbosity::Low, &line);
            }
        } else {
            self.log(Verbosity::Low, &format!("{}", "\tno changes".dimmed()));
        }
    }

    fn log_hooks(&self, name: &str, hooks: &[String]) {
        self.log(
            Verbosity::Low,
            &format!("{}", format!("[{name}]").magenta()),
        );
        if hooks.is_empty() {
            self.log(
                Verbosity::Low,
                &format!("\t{}", format!("no {name} hooks defined").dimmed()),
            );
        }
        for hook in hooks {
            self.log(
                Verbosity::Low,
                &format!("\t{} {}", "running".dimmed(), hook),
            );
        }
    }
}

pub(crate) fn unescape(value: &str) -> String {
    let mut n = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(c) = chars.next() {
                    n.push(c);
                }
            }
            c => n.push(c),
        }
    }
    n
}

pub(crate) fn format_version(version: &Version, color: Color) -> String {
    version
        .iter()
        .map(|(comp_name, value)| {
            format!(
                "{}={}",
                comp_name.color(color),
                value.value().unwrap_or("?")
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}
