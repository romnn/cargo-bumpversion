//! Diagnostics utilities for reporting and emitting parsing errors with source spans.
//! Diagnostics utilities for reporting parsing and emitting errors with source spans.
//!
// Module provides traits and types for capturing and rendering diagnostics.
use codespan_reporting::{
    diagnostic::{Diagnostic, Severity},
    files, term,
};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

/// Identifier for a source file in the diagnostics registry.
pub type FileId = usize;
/// A byte-offset span in a source file.
pub type Span = std::ops::Range<usize>;

/// A diagnostics error.
/// Diagnostics error kinds.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Failed to lookup or access a registered source file.
    #[error("failed to lookup file")]
    FileLookup(#[from] codespan_reporting::files::Error),
}

/// Convert an item into a sequence of diagnostics associated with a file.
pub trait ToDiagnostics {
    /// Generate diagnostics for this item, tagged with `file_id`.
    fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Vec<Diagnostic<F>>;
}

/// Extension methods for `Diagnostic` to simplify severity checks.
pub trait DiagnosticExt {
    /// Returns `true` if the diagnostic severity is error or bug.
    fn is_error(&self) -> bool;
    /// Returns an error-level or warning-level diagnostic builder depending on `strict`.
    fn warning_or_error(strict: bool) -> Self;
}

impl<F> DiagnosticExt for Diagnostic<F> {
    fn is_error(&self) -> bool {
        match self.severity {
            Severity::Bug | Severity::Error => true,
            Severity::Warning | Severity::Note | Severity::Help => false,
        }
    }

    fn warning_or_error(strict: bool) -> Self {
        if strict {
            Self::error()
        } else {
            Self::warning()
        }
    }
}

/// Associates a value with its source span for error reporting.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    /// The inner value.
    pub inner: T,
    /// The byte-offset span where `inner` was found.
    pub span: Span,
}

impl std::ops::Deref for Spanned<&String> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl std::ops::Deref for Spanned<String> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> AsRef<T> for Spanned<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for Spanned<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Spanned<T> {
    pub fn new(span: impl Into<Span>, value: T) -> Self {
        Self {
            span: span.into(),
            inner: value,
        }
    }

    pub fn dummy(value: T) -> Self {
        Self {
            span: Span::default(),
            inner: value,
        }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> std::fmt::Display for Spanned<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}

impl PartialEq<Spanned<&str>> for Spanned<&String> {
    fn eq(&self, other: &Spanned<&str>) -> bool {
        self.inner == other.inner
    }
}

impl PartialEq<Spanned<&str>> for Spanned<String> {
    fn eq(&self, other: &Spanned<&str>) -> bool {
        self.inner == other.inner
    }
}

impl Spanned<String> {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.as_ref().as_str()
    }
}

impl<T> PartialEq for Spanned<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> PartialEq<T> for Spanned<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &T) -> bool {
        (&self.inner as &dyn PartialEq<T>).eq(other)
    }
}

impl<T> PartialEq<&T> for Spanned<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &&T) -> bool {
        (&self.inner as &dyn PartialEq<T>).eq(*other)
    }
}

impl<T> Ord for Spanned<T>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Ord::cmp(&self.inner, &other.inner)
    }
}

impl<T> PartialOrd for Spanned<T>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        PartialOrd::partial_cmp(&self.inner, &other.inner)
    }
}

impl<T> PartialOrd<T> for Spanned<T>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        PartialOrd::partial_cmp(&self.inner, other)
    }
}

impl<T> Eq for Spanned<T> where T: Eq {}

impl<T> std::hash::Hash for Spanned<T>
where
    T: std::hash::Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

/// A diagnostics printer that buffers messages for later emission.
pub type BufferedPrinter = Printer<term::termcolor::Buffer>;
/// A diagnostics printer that writes formatted messages to stderr.
pub type StderrPrinter = Printer<term::termcolor::StandardStream>;

/// Manages source files and emits formatted diagnostics to a writer.
///
/// Tracks registered source content and renders messages with spans.
/// Printer that tracks source files and emits formatted diagnostics.
pub struct Printer<W> {
    /// Underlying writer protected by a mutex for thread safety.
    writer: Mutex<W>,
    /// Configuration for diagnostic formatting (colors, styles).
    diagnostic_config: term::Config,
    /// In-memory registry of source files and their content.
    files: RwLock<files::SimpleFiles<String, String>>,
}

/// Convert an object into a diagnostics source name (e.g., file path or id).
pub trait ToSourceName {
    /// Transform to a string used as a source name in diagnostics.
    fn to_source_name(self) -> String;
}

impl ToSourceName for String {
    fn to_source_name(self) -> String {
        self
    }
}

impl ToSourceName for &Path {
    fn to_source_name(self) -> String {
        self.to_string_lossy().to_string()
    }
}

impl ToSourceName for &PathBuf {
    fn to_source_name(self) -> String {
        self.to_string_lossy().to_string()
    }
}

impl ToSourceName for PathBuf {
    fn to_source_name(self) -> String {
        self.to_string_lossy().to_string()
    }
}

impl Default for Printer<term::termcolor::StandardStream> {
    fn default() -> Self {
        Self::stderr(None)
    }
}

impl Default for Printer<term::termcolor::Buffer> {
    fn default() -> Self {
        Self::buffered()
    }
}

impl Printer<term::termcolor::Buffer> {
    #[must_use]
    pub fn buffered() -> Self {
        let writer = term::termcolor::Buffer::ansi();
        let diagnostic_config = term::Config {
            styles: term::Styles::with_blue(term::termcolor::Color::Blue),
            ..term::Config::default()
        };
        Self {
            writer: Mutex::new(writer),
            diagnostic_config,
            files: RwLock::new(files::SimpleFiles::new()),
        }
    }

    /// Print written diagnostics to stderr.
    ///
    /// This is a workaround for <https://github.com/BurntSushi/termcolor/issues/51>.
    pub fn print(&self) -> Result<(), std::io::Error> {
        use std::io::Write;
        let mut writer = self.writer.lock().unwrap();
        writer.flush()?;
        eprintln!("{}", String::from_utf8_lossy(writer.as_slice()));
        Ok(())
    }
}

impl Printer<term::termcolor::StandardStream> {
    #[must_use]
    pub fn stderr(color_choice: Option<term::termcolor::ColorChoice>) -> Self {
        let color_choice = color_choice.unwrap_or(term::termcolor::ColorChoice::Auto);
        let writer = term::termcolor::StandardStream::stderr(color_choice);

        let diagnostic_config = term::Config {
            styles: term::Styles::with_blue(term::termcolor::Color::Blue),
            ..term::Config::default()
        };
        Self {
            writer: Mutex::new(writer),
            diagnostic_config,
            files: RwLock::new(files::SimpleFiles::new()),
        }
    }
}

impl<W> Printer<W> {
    pub fn lines(
        &self,
        diagnostic: &Diagnostic<usize>,
    ) -> Result<Vec<usize>, codespan_reporting::files::Error> {
        use codespan_reporting::files::Files;
        diagnostic
            .labels
            .iter()
            .map(|label| {
                self.files
                    .read()
                    .unwrap()
                    .line_index(label.file_id, label.range.start)
            })
            .collect()
    }

    pub fn add_source_file(&self, name: impl ToSourceName, source: String) -> usize {
        let mut files = self.files.write().unwrap();
        files.add(name.to_source_name(), source)
    }
}

impl<W> Printer<W>
where
    W: term::termcolor::WriteColor,
{
    pub fn emit(&self, diagnostic: &Diagnostic<usize>) -> Result<(), files::Error> {
        term::emit(
            &mut *self.writer.lock().unwrap(),
            &self.diagnostic_config,
            &*self.files.read().unwrap(),
            diagnostic,
        )
    }
}
