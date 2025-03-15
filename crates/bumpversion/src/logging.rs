use crate::version::Version;
use colored::{Color, Colorize};

/// Logging verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Verbosity {
    Off = 0,
    Low = 1,
    Medium = 2,
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

/// Logging implementation to use.
pub trait Log {
    fn log(&self, verbosity: Verbosity, message: &str);
}

pub trait LogExt {
    fn log_modification(
        &self,
        path: &std::path::Path,
        modification: Option<crate::files::Modification>,
    );

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
            let search = unescape(replacement.search);
            let replace = unescape(replacement.replace);
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

pub(crate) fn unescape(value: String) -> String {
    let mut n = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => if let Some(c) = chars.next() { n.push(c) },
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
