#![allow(warnings)]

pub mod diagnostics;
pub mod lines;
pub mod parse;
pub mod spanned;
pub mod value;

pub use parse::Error;
pub use spanned::{Span, Spanned};
pub use value::{from_reader, from_str, Section, SectionProxy, SectionProxyMut, Value};

#[cfg(test)]
pub mod tests {
    use crate::{
        diagnostics,
        value::{Options, Value},
        SectionProxy, Spanned,
    };
    use codespan_reporting::{diagnostic::Diagnostic, files, term};
    use std::sync::{Mutex, RwLock};

    static INIT: std::sync::Once = std::sync::Once::new();

    /// Initialize test
    ///
    /// This ensures `color_eyre` is setup once.
    pub fn init() {
        INIT.call_once(|| {
            color_eyre::install().ok();
        });
    }

    // this makes writing tests quick and concise but is confusing if included in the library
    impl From<&str> for Spanned<String> {
        fn from(value: &str) -> Self {
            Spanned::dummy(value.to_string())
        }
    }

    pub trait SectionProxyExt<'a> {
        fn items_vec(self) -> Vec<(&'a str, &'a str)>;
        fn keys_vec(self) -> Vec<&'a str>;
    }

    impl<'a> SectionProxyExt<'a> for SectionProxy<'a> {
        fn items_vec(self) -> Vec<(&'a str, &'a str)> {
            self.iter()
                .map(|(k, v)| (k.as_ref().as_str(), v.as_ref().as_str()))
                .collect::<Vec<_>>()
        }

        fn keys_vec(self) -> Vec<&'a str> {
            self.keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<&'a str>>()
        }
    }

    // this makes writing tests quick and concise but is confusing if included in the library
    impl From<String> for Spanned<String> {
        fn from(value: String) -> Self {
            Spanned::dummy(value)
        }
    }

    #[derive(Debug)]
    pub struct Printer {
        writer: Mutex<term::termcolor::Buffer>,
        diagnostic_config: term::Config,
        files: RwLock<files::SimpleFiles<String, String>>,
    }

    impl Default for Printer {
        fn default() -> Self {
            Self::new(term::termcolor::ColorChoice::Auto)
        }
    }

    impl Printer {
        #[must_use] pub fn new(color_choice: term::termcolor::ColorChoice) -> Self {
            use term::termcolor::WriteColor;
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

        pub fn add_source_file(&self, name: String, source: String) -> usize {
            let mut files = self.files.write().unwrap();
            files.add(name, source)
        }

        pub fn emit(&self, diagnostic: &Diagnostic<usize>) -> Result<(), files::Error> {
            term::emit(
                &mut *self.writer.lock().unwrap(),
                &self.diagnostic_config,
                &*self.files.read().unwrap(),
                diagnostic,
            )
        }

        /// Print written diagnostics to stderr.
        ///
        /// This is a workaround for <https://github.com/BurntSushi/termcolor/issues/51>.
        pub fn print(&self) {
            use std::io::Write;
            let mut writer = self.writer.lock().unwrap();
            writer.flush();
            eprintln!("{}", String::from_utf8_lossy(writer.as_slice()));
        }
    }

    pub fn parse(
        config: &str,
        options: Options,
        printer: &Printer,
    ) -> (Result<Value, super::Error>, usize, Vec<Diagnostic<usize>>) {
        let file_id = printer.add_source_file("config.ini".to_string(), config.to_string());
        let mut diagnostics = vec![];
        let config = crate::from_str(config, options, file_id, &mut diagnostics);
        if let Err(ref err) = config {
            diagnostics.extend(err.to_diagnostics(file_id));
        }
        for diagnostic in &diagnostics {
            printer.emit(diagnostic);
        }
        printer.print();
        (config, file_id, diagnostics)
    }
}
