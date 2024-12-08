use super::{Config, FileConfig, PartConfig};
use crate::config::pyproject_toml::{Error, ValueKind};
use crate::diagnostics::{DiagnosticExt, FileId, Span, Spanned};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use color_eyre::eyre;
use toml_span::Value;

// #[derive(thiserror::Error, Debug)]
// pub enum Error {
//     // #[error("{message}")]
//     // MissingKey {
//     //     key: String,
//     //     message: String,
//     //     span: Span,
//     // },
//     #[error("{message}")]
//     UnexpectedType {
//         message: String,
//         expected: Vec<ValueKind>,
//         span: Span,
//     },
//     // #[error("{source}")]
//     // Serde {
//     //     #[source]
//     //     source: serde_json::Error,
//     //     span: Span,
//     // },
//     #[error("{source}")]
//     Toml {
//         #[source]
//         source: toml_span::Error,
//     },
// }
//
// mod diagnostics {
//     use crate::config::pyproject_toml::ValueKind;
//     use crate::diagnostics::ToDiagnostics;
//     use codespan_reporting::diagnostic::{self, Diagnostic, Label};
//
//     impl ToDiagnostics for super::Error {
//         fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Vec<Diagnostic<F>> {
//             match self {
//                 // Self::MissingKey {
//                 //     message, key, span, ..
//                 // } => vec![Diagnostic::error()
//                 //     .with_message(format!("missing required key `{key}`"))
//                 //     .with_labels(vec![
//                 //         Label::secondary(file_id, span.clone()).with_message(message)
//                 //     ])],
//                 // Self::UnexpectedType {
//                 //     expected,
//                 //     // found,
//                 //     span,
//                 //     ..
//                 // } => {
//                 //     let expected = expected
//                 //         .iter()
//                 //         .map(|ty| format!("`{ty:?}`"))
//                 //         .collect::<Vec<_>>()
//                 //         .join(", or ");
//                 //     let note = unindent::unindent(&format!(
//                 //         "
//                 //         expected type {expected}
//                 //            found type `{:?}`
//                 //         ",
//                 //         ValueKind::String
//                 //     ));
//                 //     let diagnostic = Diagnostic::error()
//                 //         .with_message(self.to_string())
//                 //         .with_labels(vec![Label::primary(file_id, span.clone())
//                 //             .with_message(format!("expected {expected}"))])
//                 //         .with_notes(vec![note]);
//                 //     vec![diagnostic]
//                 // }
//                 // Self::Serde { source, span } => vec![Diagnostic::error()
//                 //     .with_message(self.to_string())
//                 //     .with_labels(vec![
//                 //         Label::primary(file_id, span.clone()).with_message(source.to_string())
//                 //     ])],
//                 Self::Toml { source } => vec![source.to_diagnostic(file_id)],
//             }
//         }
//     }
// }

impl Config {
    pub fn from_toml_value(
        config: Value,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let Some(config) = config
            .as_table()
            .and_then(|config| config.get("tool"))
            .and_then(|tool| tool.as_table())
            .and_then(|tool| tool.get("bumpversion"))
        // .map(|config| config.take())
        else {
            return Ok(None);
        };

        // let config = config.as_table().ok_or_else(||)

        // config
        //     .("current_version")
        //     .and_then(as_optional)
        //     .map(ini::Spanned::into_inner);
        Ok(None)
    }

    pub fn from_toml(
        config: &str,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let config = toml_span::parse(&config).map_err(|source| Error::Toml { source })?;
        Self::from_toml_value(config, file_id, strict, diagnostics)
    }
}

// #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
// pub struct BumpversionTomlFileConfig {
//     pub filename: String,
//     #[serde(flatten)]
//     pub config: super::Config,
// }
//
// #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
// pub struct BumpversionTomlTool {
//     pub files: Vec<BumpversionTomlFileConfig>,
// }
//
// #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
// pub struct SetupCfgTomlTools {
//     pub bumpversion: BumpversionTomlTool,
// }
//
// #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
// pub struct SetupCfgToml {
//     pub tool: SetupCfgTomlTools,
//     // bumpversion: Option<Config>,
// }
//
// pub type PyProjectToml = SetupCfgToml;
//
// impl SetupCfgToml {
//     pub fn from_str(config: &str) -> eyre::Result<Self> {
//         let config: SetupCfgToml = toml::from_str(&config)?;
//         Ok(config)
//     }
// }
//
// #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
// pub struct CargoToml {
//     pub tool: SetupCfgTomlTools,
// }
//
// impl CargoToml {
//     pub fn from_str(config: &str) -> eyre::Result<Self> {
//         let config: Self = toml::from_str(&config)?;
//         Ok(config)
//     }
// }

#[cfg(test)]
mod tests {
    use crate::config::pyproject_toml::tests::parse_toml;
    use crate::{
        config::{Config, FileConfig, PartConfig},
        diagnostics::{Printer, ToDiagnostics},
    };
    use codespan_reporting::diagnostic;
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use std::io::Read;
    use std::path::PathBuf;

    #[test]
    fn test_invalid_bumpversion_toml() -> eyre::Result<()> {
        crate::tests::init();

        // invalid (unlike ini files, quotation is required for values)
        let bumpversion_toml = indoc::indoc! {r#"
            [bumpversion]
            current_version = 0.1.8
            commit = True
            tag = True
            message = DO NOT BUMP VERSIONS WITH THIS FILE

            [bumpversion:glob:*.txt]
            [bumpversion:glob:**/*.txt]

            [bdist_wheel]
            universal = 1
        "#};

        let printer = Printer::default();
        let (config, file_id) = parse_toml(&bumpversion_toml, &printer);
        let err = config.unwrap_err();
        let error_diagnostics: Vec<codespan_reporting::diagnostic::Diagnostic<_>> =
            err.to_diagnostics(file_id);

        similar_asserts::assert_eq!(&err.to_string(), "expected newline, found a period");
        similar_asserts::assert_eq!(printer.lines(&error_diagnostics[0]).ok(), Some(vec![1]));
        Ok(())
    }

    #[test]
    fn test_valid_pyproject_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "1.0.0"
        "#};
        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;
        dbg!(config);

        let expected = Config {
            global: FileConfig {
                current_version: Some("1.0.0".to_string()),
                // commit: Some(true),
                // tag: Some(true),
                // commit_message: Some("DO NOT BUMP VERSIONS WITH THIS FILE".to_string()),
                ..FileConfig::default()
            },
            ..Config::default()
        };
        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;
        similar_asserts::assert_eq!(config, Some(expected));

        Ok(())
    }

    #[test]
    fn test_bumpversion_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [bumpversion]
            current_version = "0.1.8"
            commit = true
            tag = true
            message = "DO NOT BUMP VERSIONS WITH THIS FILE"

            [bumpversion:glob:*.txt]
            [bumpversion:glob:**/*.txt]

            [bumpversion:file:setup.py]
            search = 'version = "{current_version}"'
            replace = 'version = "{new_version}"'

            [bumpversion:file:favico/__init__.py]
            search = '__version__ = "{current_version}"'
            replace = '__version__ = "{new_version}"'

            [bumpversion:file_with_dotted_version:file1]
            search = 'dots: {current_version}'
            replace = 'dots: {new_version}'

            [bumpversion:file_with_dotted_version:file2]
            search = 'dashes: {current_version}'
            replace = 'dashes: {new_version}'
            parse = '(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)'
            serialize = '{major}-{minor}-{patch}'

            [bdist_wheel]
            universal = 1
        "#};

        let expected = Config {
            global: FileConfig {
                current_version: Some("0.1.8".to_string()),
                commit: Some(true),
                tag: Some(true),
                commit_message: Some("DO NOT BUMP VERSIONS WITH THIS FILE".to_string()),
                ..FileConfig::default()
            },
            files: vec![
                (
                    PathBuf::from("setup.py"),
                    FileConfig {
                        search: Some(r#"version = "{current_version}"#.to_string()),
                        replace: Some(r#"version = "{new_version}"#.to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("favico/__init__.py"),
                    FileConfig {
                        search: Some(r#"__version__ = "{current_version}"#.to_string()),
                        replace: Some(r#"__version__ = "{new_version}"#.to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("file1"),
                    FileConfig {
                        search: Some(r#"dots: "{current_version}"#.to_string()),
                        replace: Some(r#"dots: "{new_version}"#.to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("file2"),
                    FileConfig {
                        search: Some(r#"dashes: "{current_version}"#.to_string()),
                        replace: Some(r#"dashes: "{new_version}"#.to_string()),
                        parse: Some(r#"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)"#.to_string()),
                        serialize: vec![r#"{major}-{minor}-{patch}"#.to_string()],
                        ..FileConfig::default()
                    },
                ),
            ],
            parts: [].into_iter().collect(),
        };
        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;
        similar_asserts::assert_eq!(config, Some(expected));

        Ok(())
    }
}
