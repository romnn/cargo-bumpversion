use super::{Config, FileConfig, PartConfig};
use crate::config::pyproject_toml::ValueKind;
use crate::diagnostics::{DiagnosticExt, FileId, Span, Spanned};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use color_eyre::eyre;
use indexmap::IndexMap;
use serde_ini_spanned as ini;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{message}")]
    MissingKey {
        key: String,
        message: String,
        span: Span,
    },
    #[error("{message}")]
    UnexpectedType {
        message: String,
        expected: Vec<ValueKind>,
        span: Span,
    },
    // #[error("{source}")]
    // Serde {
    //     #[source]
    //     source: serde_json::Error,
    //     span: Span,
    // },
    #[error("{source}")]
    Ini {
        #[source]
        source: ini::Error,
    },
}

mod diagnostics {
    use crate::config::pyproject_toml::ValueKind;
    use crate::diagnostics::ToDiagnostics;
    use codespan_reporting::diagnostic::{self, Diagnostic, Label};

    impl ToDiagnostics for super::Error {
        fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Vec<Diagnostic<F>> {
            match self {
                Self::MissingKey {
                    message, key, span, ..
                } => vec![Diagnostic::error()
                    .with_message(format!("missing required key `{key}`"))
                    .with_labels(vec![
                        Label::secondary(file_id, span.clone()).with_message(message)
                    ])],
                Self::UnexpectedType {
                    expected,
                    // found,
                    span,
                    ..
                } => {
                    let expected = expected
                        .iter()
                        .map(|ty| format!("`{ty:?}`"))
                        .collect::<Vec<_>>()
                        .join(", or ");
                    let note = unindent::unindent(&format!(
                        "
                        expected type {expected}
                           found type `{:?}`
                        ",
                        ValueKind::String
                    ));
                    let diagnostic = Diagnostic::error()
                        .with_message(self.to_string())
                        .with_labels(vec![Label::primary(file_id, span.clone())
                            .with_message(format!("expected {expected}"))])
                        .with_notes(vec![note]);
                    vec![diagnostic]
                }
                // // Self::Serde { source, span } => vec![Diagnostic::error()
                // //     .with_message(self.to_string())
                // //     .with_labels(vec![
                // //         Label::primary(file_id, span.clone()).with_message(source.to_string())
                // //     ])],
                Self::Ini { source } => source.to_diagnostics(file_id),
            }
        }
    }
}

#[inline]
pub fn as_bool(value: ini::Spanned<String>) -> Result<bool, Error> {
    match value.as_ref().trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(Error::UnexpectedType {
            message: "expected a boolean".to_string(),
            expected: vec![ValueKind::String],
            span: value.span.clone(),
        }),
    }
}

#[inline]
pub fn as_string_array(
    value: ini::Spanned<String>,
    allow_single_value: bool,
) -> Result<Vec<String>, Error> {
    let ini::Spanned { inner, span } = value;
    if inner.contains("\n") {
        Ok(inner.trim().split("\n").map(ToString::to_string).collect())
    } else if inner.contains(",") {
        Ok(inner.trim().split(",").map(ToString::to_string).collect())
    } else if allow_single_value {
        Ok(vec![inner])
    } else {
        Err(Error::UnexpectedType {
            message: "expected a list".to_string(),
            expected: vec![ValueKind::Array],
            span,
        })
    }

    // # derive the type of the variable
    // element_caster = str
    // for caster in (boolify, int, float, noneify, element_caster):
    //     with contextlib.suppress(ValueError):
    //         caster(str_list[0])  # type: ignore[operator]
    //         element_caster = caster  # type: ignore[assignment]
    //         break
    // # cast all elements
    // try:
    //     return [element_caster(x) for x in str_list]
    // except ValueError as e:
    //     raise TypeError("Autocasted list must be all same type") from e
    // match value.as_ref().trim().to_ascii_lowercase().as_str() {
    //     "true" => Ok(true),
    //     "false" => Ok(false),
    //     other => Err(Error::UnexpectedType {
    //         message: "expected a boolean".to_string(),
    //         expected: vec![ValueKind::String],
    //         span: value.span.clone(),
    //     }),
    // }
}

#[inline]
pub fn as_optional(value: ini::Spanned<String>) -> Option<ini::Spanned<String>> {
    if value.as_ref() == "None" {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn parse_part_config<'de>(
    mut value: ini::SectionProxyMut<'_>,
) -> Result<PartConfig, Error> {
    let optional_value = value
        .remove_option("optional_value")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);
    let values = value
        .remove_option("values")
        .map(|value| as_string_array(value, false))
        .transpose()?
        .unwrap_or_default();

    Ok(PartConfig {
        optional_value,
        values,
    })
}

// pub(crate) fn parse_file_config(mut value: ini::Section) -> Result<FileConfig, Error> {
pub(crate) fn parse_file_config(mut value: ini::SectionProxyMut<'_>) -> Result<FileConfig, Error> {
    let current_version = value
        .remove_option("current_version")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);

    let allow_dirty = value
        .remove_option("allow_dirty")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let parse = value.remove_option("parse").map(ini::Spanned::into_inner);
    let serialize = value
        .remove_option("serialize")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?
        .unwrap_or_default();
    let search = value
        .remove_option("search")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);
    let replace = value
        .remove_option("replace")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);
    let regex = value
        .remove_option("regex")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let no_configured_files = value
        .remove_option("no_configured_files")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let ignore_missing_files = value
        .remove_option("ignore_missing_files")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let ignore_missing_version = value
        .remove_option("ignore_missing_version")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let dry_run = value
        .remove_option("dry_run")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let commit = value
        .remove_option("commit")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let tag = value
        .remove_option("tag")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let sign_tag = value
        .remove_option("sign_tag")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let tag_name = value
        .remove_option("tag_name")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);
    let tag_message = value
        .remove_option("tag_message")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);
    let commit_message = value
        .remove_option("commit_message")
        .and_then(as_optional)
        .or(value.remove_option("message"))
        .map(ini::Spanned::into_inner);
    let commit_args = value
        .remove_option("commit_args")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);

    Ok(FileConfig {
        allow_dirty,
        current_version,
        parse,
        serialize,
        search,
        replace,
        regex,
        no_configured_files,
        ignore_missing_files,
        ignore_missing_version,
        dry_run,
        commit,
        tag,
        sign_tag,
        tag_name,
        tag_message,
        commit_message,
        commit_args,
    })
}

impl Config {
    pub fn from_ini_value(
        mut config: ini::Value,
        file_id: FileId,
        strict: bool,
        allow_unknown: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        if !allow_unknown {
            for (key, value) in config.defaults() {
                // emit warnings for ignored global values
                let diagnostic = Diagnostic::warning_or_error(strict)
                    .with_message("global config values have no effect")
                    .with_labels(vec![Label::primary(file_id, key.span.clone())
                        .with_message("this configuration will be ignored")]);
                diagnostics.push(diagnostic);
            }
        }

        let mut out = Self::default();
        let mut found = false;
        let section_names = config.section_names().cloned().collect::<Vec<_>>();
        for section_name in section_names {
            let section = config.section_mut(&section_name).unwrap();
            // let section_name = section.name.as_ref().trim();
            let span = section.span();
            // dbg!(&section_name);

            if !section_name.starts_with("bumpversion") {
                if !allow_unknown {
                    let diagnostic = Diagnostic::warning_or_error(strict)
                        .with_message(format!("unexpected section `{section_name}`"))
                        .with_labels(vec![Label::primary(file_id, span.clone()).with_message(
                            "sections that do not start with `bumpversion` are ignored",
                        )]);
                    diagnostics.push(diagnostic);
                }
                continue;
            }

            found = true;
            let section_parts = section_name.split(":").map(str::trim).collect::<Vec<_>>();

            match section_parts[..] {
                ["bumpversion"] => {
                    out.global = parse_file_config(section)?;
                }
                ["bumpversion", prefix, file_name] => {
                    if prefix.starts_with("file") {
                        let config = parse_file_config(section)?;
                        out.files.push((file_name.into(), config));
                    } else if prefix.starts_with("glob") {
                        let config = parse_file_config(section)?;
                        out.files.push((file_name.into(), config));
                    } else if prefix.starts_with("part") {
                        let config = parse_part_config(section)?;
                        out.parts.insert(file_name.into(), config);
                    } else if !allow_unknown {
                        let diagnostic = Diagnostic::warning_or_error(strict)
                            .with_message(format!("unknown config prefix `{prefix}`"))
                            .with_labels(vec![Label::primary(file_id, span.clone())
                                .with_message(format!(
                                    "config sections must start with `file`, `glob`, or `part`, got `{prefix}`",
                                ))]);
                        diagnostics.push(diagnostic);
                    }
                }
                _ => {
                    if !allow_unknown {
                        let diagnostic = Diagnostic::warning_or_error(strict)
                            .with_message(format!(
                                "invalid config section `{}`",
                                section_parts.join(":")
                            ))
                            .with_labels(vec![Label::primary(file_id, span.clone()).with_message(
                                format!("should be of the form `bumpversion:kind:file_name`",),
                            )]);
                        diagnostics.push(diagnostic);
                    }
                }
            };
        }

        if found {
            Ok(Some(out))
        } else {
            Ok(None)
        }
    }

    pub fn from_ini(
        config: &str,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let config = ini::from_str(&config).map_err(|source| Error::Ini { source })?;
        let allow_unknown = false;
        Self::from_ini_value(config, file_id, strict, allow_unknown, diagnostics)
    }

    pub fn from_setup_cfg_ini(
        config: &str,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let config = ini::from_str(&config).map_err(|source| Error::Ini { source })?;
        let allow_unknown = true;
        Self::from_ini_value(config, file_id, strict, allow_unknown, diagnostics)
    }
}

// def autocast_value(var: Any) -> Any:
//     """
//     Guess the string representation of the variable's type.
//
//     Args:
//         var: Value to autocast.
//
//     Returns:
//         The autocasted value.
//     """
//     if not isinstance(var, str):  # don't need to guess non-string types
//         return var
//
//     # guess string representation of var
//     for caster in (boolify, int, float, noneify, listify):
//         with contextlib.suppress(ValueError):
//             return caster(var)  # type: ignore[operator]
//
//     return var

#[cfg(test)]
mod tests {
    use crate::config::{Config, FileConfig, PartConfig};
    use crate::diagnostics::{Printer, ToDiagnostics};
    use codespan_reporting::diagnostic;
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use std::io::Read;
    use std::path::PathBuf;

    fn parse_ini(config: &str, printer: &Printer) -> (Result<Option<Config>, super::Error>, usize) {
        let mut diagnostics = vec![];
        let file_id = printer.add_source_file("bumpversion.cfg".to_string(), config.to_string());
        let strict = true;
        let config = Config::from_ini(config, file_id, strict, &mut diagnostics);
        if let Err(ref err) = config {
            for diagnostic in err.to_diagnostics(file_id) {
                printer.emit(&diagnostic);
            }
        }
        (config, file_id)
    }

    #[test]
    fn parse_cfg_ini_simple() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_cfg = indoc::indoc! {r#"
            [bumpversion:file:coolapp/__init__.py]

            [bumpversion:file(version heading):CHANGELOG.md]
            search = Unreleased

            [bumpversion:file(previous version):CHANGELOG.md]
            search = {current_version}...HEAD
            replace = {current_version}...{new_version}
        "#};

        let config = parse_ini(bumpversion_cfg, &Printer::default()).0?;

        let expected = Config {
            global: FileConfig::default(),
            files: vec![
                (PathBuf::from("coolapp/__init__.py"), FileConfig::default()),
                (
                    PathBuf::from("CHANGELOG.md"),
                    FileConfig {
                        search: Some("Unreleased".to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("CHANGELOG.md"),
                    FileConfig {
                        search: Some("{current_version}...HEAD".to_string()),
                        replace: Some("{current_version}...{new_version}".to_string()),
                        ..FileConfig::default()
                    },
                ),
            ],
            parts: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    #[test]
    fn parse_python_setup_cfg_ini() -> eyre::Result<()> {
        crate::tests::init();

        // note: in ini files, there are fewer conventions compared to TOML
        // for example, we can write 0.1.8 without quotes, just as treat "True" as boolean true
        let setup_cfg_ini = indoc::indoc! {r#"
            [bumpversion]
            current_version = 0.1.8
            commit = True
            tag = True
            message = DO NOT BUMP VERSIONS WITH THIS FILE

            [bumpversion:glob:*.txt]
            [bumpversion:glob:**/*.txt]

            [bumpversion:file:setup.py]
            search = version = "{current_version}"
            replace = version = "{new_version}"

            [bumpversion:file:favico/__init__.py]
            search = __version__ = "{current_version}"
            replace = __version__ = "{new_version}"

            [bumpversion:file_with_dotted_version:file1]
            search = dots: {current_version}
            replace = dots: {new_version}

            [bumpversion:file_with_dotted_version:file2]
            search = dashes: {current_version}
            replace = dashes: {new_version}
            parse = (?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)
            serialize = {major}-{minor}-{patch}

            [bdist_wheel]
            universal = 1

            [metadata]
            description-file = README.rst

            [flake8]
            exclude = docs
            ignore = E203, E266, E501, W503
            max-line-length = 88
            max-complexity = 18
            select = B,C,E,F,W,T4
        "#};

        let config = parse_ini(setup_cfg_ini, &Printer::default()).0?;

        let expected = Config {
            global: FileConfig {
                current_version: Some("0.1.8".to_string()),
                commit: Some(true),
                tag: Some(true),
                commit_message: Some("DO NOT BUMP VERSIONS WITH THIS FILE".to_string()),
                ..FileConfig::default()
            },
            files: vec![
                (PathBuf::from("*.txt"), FileConfig::default()),
                (PathBuf::from("**/*.txt"), FileConfig::default()),
                (
                    PathBuf::from("setup.py"),
                    FileConfig {
                        search: Some(r#"version = "{current_version}""#.to_string()),
                        replace: Some(r#"version = "{new_version}""#.to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("favico/__init__.py"),
                    FileConfig {
                        search: Some(r#"__version__ = "{current_version}""#.to_string()),
                        replace: Some(r#"__version__ = "{new_version}""#.to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("file1"),
                    FileConfig {
                        search: Some("dots: {current_version}".to_string()),
                        replace: Some("dots: {new_version}".to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("file2"),
                    FileConfig {
                        search: Some("dashes: {current_version}".to_string()),
                        replace: Some("dashes: {new_version}".to_string()),
                        parse: Some(r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)".to_string()),
                        serialize: vec!["{major}-{minor}-{patch}".to_string()],
                        ..FileConfig::default()
                    },
                ),
            ],
            parts: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    #[test]
    fn parse_cfg_ini_compat() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_cfg = indoc::indoc! {r#"
            [options.packages.find]
            exclude =
                example*
                tests*
                docs*
                build

            [bumpversion]
            commit = True
            tag = True
            current_version = 1.0.0
            parse = (?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\-(?P<release>[a-z]+))?
            serialize =
                {major}.{minor}.{patch}-{release}
                {major}.{minor}.{patch}

            [darglint]
            ignore = DAR402

            [bumpversion:file:setup.py]

            [bumpversion:file:bumpversion/__init__.py]

            [bumpversion:file:CHANGELOG.md]
            search = **unreleased**
            replace = **unreleased**
                **v{new_version}**

            [bumpversion:part:release]
            optional_value = gamma
            values =
                dev
                gamma
        "#};

        let config = parse_ini(bumpversion_cfg, &Printer::default()).0?;
        let expected = Config {
            global: FileConfig {
                commit: Some(true),
                tag: Some(true),
                current_version: Some("1.0.0".to_string()),
                parse: Some(
                    r#"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\-(?P<release>[a-z]+))?"#
                        .to_string(),
                ),
                serialize: vec![
                    r#"{major}.{minor}.{patch}-{release}"#.to_string(),
                    r#"{major}.{minor}.{patch}"#.to_string(),
                ],
                ..FileConfig::default()
            },
            files: vec![
                (PathBuf::from("setup.py"), FileConfig::default()),
                (
                    PathBuf::from("bumpversion/__init__.py"),
                    FileConfig::default(),
                ),
                (
                    PathBuf::from("CHANGELOG.md"),
                    FileConfig {
                        search: Some("**unreleased**".to_string()),
                        replace: Some(
                            indoc::indoc! {
                                r#"
                                **unreleased**
                                **v{new_version}**"#
                            }
                            .to_string(),
                        ),

                        ..FileConfig::default()
                    },
                ),
            ],
            parts: [(
                "release".to_string(),
                PartConfig {
                    optional_value: Some("gamma".to_string()),
                    values: vec!["dev".to_string(), "gamma".to_string()],
                },
            )]
            .into_iter()
            .collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }
}
