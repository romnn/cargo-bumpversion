#![allow(warnings)]

// pub mod de;
pub mod lines;
pub mod parse;
pub mod spanned;
pub mod value;
// pub mod ser;
// pub mod write;

pub use parse::Error;
pub use spanned::{Span, Spanned};
pub use value::{from_reader, from_str, Section, SectionProxy, SectionProxyMut, Value};

// pub use de::{from_bufread, from_read, from_str, Deserializer};
// pub use parse::{Item, Parser};
// pub use ser::{to_string, to_vec, to_writer, Serializer};
// pub use write::{LineEnding, Writer};

#[cfg(test)]
pub mod tests {
    use crate::value::Value;
    use codespan_reporting::{diagnostic::Diagnostic, files, term};
    use std::sync::RwLock;

    static INIT: std::sync::Once = std::sync::Once::new();

    /// Initialize test
    ///
    /// This ensures color_eyre is setup once.
    pub fn init() {
        INIT.call_once(|| {
            color_eyre::install().ok();
        });
    }

    #[derive(Debug)]
    pub struct Printer {
        writer: term::termcolor::StandardStream,
        diagnostic_config: term::Config,
        files: RwLock<files::SimpleFiles<String, String>>,
    }

    impl Default for Printer {
        fn default() -> Self {
            Self::new(term::termcolor::ColorChoice::Auto)
        }
    }

    impl Printer {
        pub fn new(color_choice: term::termcolor::ColorChoice) -> Self {
            let writer = term::termcolor::StandardStream::stderr(color_choice);
            let diagnostic_config = term::Config {
                styles: term::Styles::with_blue(term::termcolor::Color::Blue),
                ..term::Config::default()
            };
            Self {
                writer,
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
                &mut self.writer.lock(),
                &self.diagnostic_config,
                &*self.files.read().unwrap(),
                diagnostic,
            )
        }
    }

    pub fn parse(config: &str, printer: &Printer) -> (Result<Value, super::Error>, usize) {
        let file_id = printer.add_source_file("config.ini".to_string(), config.to_string());
        // let strict = true;
        let config = crate::from_str(config);
        // , file_id, strict, &mut diagnostics).map_err(|err| {
        //     for diagnostic in err.to_diagnostics(file_id) {
        //         printer.emit(&diagnostic);
        //     }
        //     err
        // });
        if let Err(ref err) = config {
            for diagnostic in err.to_diagnostics(file_id) {
                printer.emit(&diagnostic);
            }
        }
        (config, file_id)
    }
}
