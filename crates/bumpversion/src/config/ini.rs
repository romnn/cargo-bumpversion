use crate::{
    config::{
        self, pyproject_toml::ValueKind, Config, FileConfig, GlobalConfig, InputFile,
        RegexTemplate, VersionComponentSpec,
    },
    diagnostics::{DiagnosticExt, FileId, Span, Spanned},
    f_string::PythonFormatString,
};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use color_eyre::eyre;
use indexmap::IndexMap;
use serde_ini_spanned as ini;
use std::path::{Path, PathBuf};

pub use ini::value::Options;

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
    #[error("{message}")]
    InvalidFormatString {
        #[source]
        source: crate::f_string::Error,
        message: String,
        span: Span,
    },
    #[error("{message}")]
    InvalidRegex {
        #[source]
        source: regex::Error,
        message: String,
        span: Span,
    },
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
                Self::InvalidRegex {
                    source,
                    message,
                    span,
                    ..
                } => vec![Diagnostic::error()
                    .with_message("invalid regular expression".to_string())
                    .with_labels(vec![
                        Label::primary(file_id, span.clone()).with_message(source.to_string()),
                        Label::secondary(file_id, span.clone()).with_message(message),
                    ])],
                Self::InvalidFormatString {
                    source,
                    message,
                    span,
                    ..
                } => vec![Diagnostic::error()
                    .with_message("invalid format string".to_string())
                    .with_labels(vec![
                        Label::primary(file_id, span.clone()).with_message(source.to_string()),
                        Label::secondary(file_id, span.clone()).with_message(message),
                    ])],
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
pub fn as_format_string(value: ini::Spanned<String>) -> Result<PythonFormatString, Error> {
    let ini::Spanned { inner, span } = value;
    PythonFormatString::parse(&inner).map_err(|source| Error::InvalidFormatString {
        source,
        message: "invalid format string".to_string(),
        span,
    })
}

#[inline]
pub fn as_regex(value: ini::Spanned<String>) -> Result<config::Regex, Error> {
    let ini::Spanned { inner, span } = value;
    let inner = inner.replace("\\\\", "\\");
    let inner = crate::f_string::parser::escape_double_curly_braces(&inner).unwrap_or(inner);
    regex::Regex::new(&inner)
        .map(Into::into)
        .map_err(|source| Error::InvalidRegex {
            source,
            message: format!("invalid regular expression: {inner:?}"),
            span,
        })
}

#[inline]
pub fn as_string_array(
    value: ini::Spanned<String>,
    allow_single_value: bool,
) -> Result<Vec<String>, Error> {
    let ini::Spanned { inner, span } = value;
    if inner.contains('\n') {
        Ok(inner.trim().split('\n').map(ToString::to_string).collect())
    } else if inner.contains(',') {
        Ok(inner.trim().split(',').map(ToString::to_string).collect())
    } else if allow_single_value {
        Ok(vec![inner])
    } else {
        Err(Error::UnexpectedType {
            message: "expected a list".to_string(),
            expected: vec![ValueKind::Array],
            span,
        })
    }
}

#[inline]
#[must_use]
pub fn as_optional(value: ini::Spanned<String>) -> Option<ini::Spanned<String>> {
    if value.as_ref() == "None" {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn parse_part_config<'de>(
    mut value: ini::SectionProxyMut<'_>,
) -> Result<VersionComponentSpec, Error> {
    let independent = value
        .remove_option("independent")
        .map(as_bool)
        .transpose()?;

    let optional_value = value
        .remove_option("optional_value")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);
    let values = value
        .remove_option("values")
        .map(|value| as_string_array(value, false))
        .transpose()?
        .unwrap_or_default();

    Ok(VersionComponentSpec {
        independent,
        optional_value,
        values,
        ..VersionComponentSpec::default()
    })
}

fn parse_search_pattern(
    value: &mut ini::SectionProxyMut<'_>,
    is_regex: Option<bool>,
) -> Result<(Option<bool>, Option<RegexTemplate>), Error> {
    let search_is_regex_compat = value
        .remove_option("regex")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?
        .or(is_regex);
    let search = value
        .remove_option("search")
        .and_then(as_optional)
        .map(|search| {
            if search_is_regex_compat == Some(true) {
                let format_string = as_format_string(search)?;
                Ok(RegexTemplate::Regex(format_string))
            } else {
                let format_string = as_format_string(search)?;
                Ok(RegexTemplate::Escaped(format_string))
            }
        })
        .transpose()?;
    Ok((search_is_regex_compat, search))
}

pub(crate) fn parse_global_config(
    mut value: ini::SectionProxyMut<'_>,
) -> Result<(Option<bool>, GlobalConfig), Error> {
    let current_version = value
        .remove_option("current_version")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);

    let (search_is_regex_compat, search) = parse_search_pattern(&mut value, None)?;

    let allow_dirty = value
        .remove_option("allow_dirty")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let parse_version_pattern = value.remove_option("parse").map(as_regex).transpose()?;
    let serialize_version_patterns = value
        .remove_option("serialize")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?;

    let replace = value
        .remove_option("replace")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);

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
    let sign_tags = value
        .remove_option("sign_tag")
        .or(value.remove_option("sign_tags"))
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let tag_name = value
        .remove_option("tag_name")
        .and_then(as_optional)
        .map(as_format_string)
        .transpose()?;
    let tag_message = value
        .remove_option("tag_message")
        .and_then(as_optional)
        .map(as_format_string)
        .transpose()?;
    let commit_message = value
        .remove_option("commit_message")
        .and_then(as_optional)
        .or(value.remove_option("message"))
        .map(as_format_string)
        .transpose()?;
    let commit_args = value
        .remove_option("commit_args")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);

    // extra stuff
    let setup_hooks = value
        .remove_option("setup_hooks")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?;
    let pre_commit_hooks = value
        .remove_option("pre_commit_hooks")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?;
    let post_commit_hooks = value
        .remove_option("post_commit_hooks")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?;
    let included_paths = value
        .remove_option("included_paths")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?
        .map(|values| values.into_iter().map(PathBuf::from).collect());
    let excluded_paths = value
        .remove_option("excluded_paths")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?
        .map(|values| values.into_iter().map(PathBuf::from).collect());

    Ok((
        search_is_regex_compat,
        GlobalConfig {
            allow_dirty,
            current_version,
            parse_version_pattern,
            serialize_version_patterns,
            search,
            replace,
            no_configured_files,
            ignore_missing_files,
            ignore_missing_version,
            dry_run,
            commit,
            tag,
            sign_tags,
            tag_name,
            tag_message,
            commit_message,
            commit_args,
            // extra stuff
            setup_hooks,
            pre_commit_hooks,
            post_commit_hooks,
            included_paths,
            excluded_paths,
        },
    ))
}

pub(crate) fn parse_file_config(
    mut value: ini::SectionProxyMut<'_>,
    search_is_regex_compat: Option<bool>,
) -> Result<FileConfig, Error> {
    let (_, search) = parse_search_pattern(&mut value, search_is_regex_compat)?;
    let parse_version_pattern = value.remove_option("parse").map(as_regex).transpose()?;
    let serialize_version_patterns = value
        .remove_option("serialize")
        .and_then(as_optional)
        .map(|value| as_string_array(value, true))
        .transpose()?;

    let replace = value
        .remove_option("replace")
        .and_then(as_optional)
        .map(ini::Spanned::into_inner);

    let ignore_missing_file = value
        .remove_option("ignore_missing_files")
        .or(value.remove_option("ignore_missing_file"))
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;
    let ignore_missing_version = value
        .remove_option("ignore_missing_version")
        .and_then(as_optional)
        .map(as_bool)
        .transpose()?;

    Ok(FileConfig {
        parse_version_pattern,
        serialize_version_patterns,
        search,
        replace,
        ignore_missing_file,
        ignore_missing_version,
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
        let mut search_is_regex_compat = None;

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
            let section_parts = section_name.split(':').map(str::trim).collect::<Vec<_>>();

            match section_parts[..] {
                ["bumpversion"] => {
                    (search_is_regex_compat, out.global) = parse_global_config(section)?;
                }
                ["bumpversion", prefix, value] => {
                    if prefix.starts_with("file") {
                        let config = parse_file_config(section, search_is_regex_compat)?;
                        out.files.push((InputFile::Path(value.into()), config));
                    } else if prefix.starts_with("glob") {
                        let config = parse_file_config(section, search_is_regex_compat)?;
                        out.files.push((
                            InputFile::GlobPattern {
                                pattern: value.into(),
                                exclude_patterns: None,
                            },
                            config,
                        ));
                    } else if prefix.starts_with("part") {
                        let config = parse_part_config(section)?;
                        out.components.insert(value.into(), config);
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
                                "should be of the form `bumpversion:kind:file_name`".to_string(),
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
        options: Options,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let config = ini::from_str(config, options, file_id, diagnostics)
            .map_err(|source| Error::Ini { source })?;
        let allow_unknown = false;
        Self::from_ini_value(config, file_id, strict, allow_unknown, diagnostics)
    }

    pub fn from_setup_cfg_ini(
        config: &str,
        options: Options,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let config = ini::from_str(config, options, file_id, diagnostics)
            .map_err(|source| Error::Ini { source })?;
        let allow_unknown = true;
        Self::from_ini_value(config, file_id, strict, allow_unknown, diagnostics)
    }
}

static CONFIG_CURRENT_VERSION_REGEX: once_cell::sync::Lazy<regex::Regex> =
    once_cell::sync::Lazy::new(|| {
        regex::RegexBuilder::new(r"(?P<section_prefix>\\[bumpversion]\n[^[]*current_version\\s*=\\s*)(?P<version>{current_version})").multi_line(true).build().unwrap()
    });

/// Update the `current_version` key in the configuration file.
///
/// Instead of parsing and re-writing the config file with new information,
/// it will use a regular expression to just replace the `current_version` value.
/// The idea is it will avoid unintentional changes (like formatting) to the
/// config file.
pub async fn replace_version(
    path: &Path,
    _config: &Config,
    current_version: &str,
    new_version: &str,
    dry_run: bool,
) -> eyre::Result<bool> {
    let existing_config = tokio::fs::read_to_string(path).await?;
    // let extension = path.extension().and_then(|ext| ext.to_str());
    let matches = CONFIG_CURRENT_VERSION_REGEX.find_iter(&existing_config);
    // let new_config = if extension == Some("cfg") && matches.count() > 0 {
    let new_config = if matches.count() > 0 {
        let replacement = format!(r#"\g<section_prefix>{new_version}"#);
        CONFIG_CURRENT_VERSION_REGEX.replace_all(&existing_config, replacement)
    } else {
        tracing::info!("could not find current version ({current_version}) in {path:?}");
        return Ok(false);
    };

    if dry_run {
        tracing::info!("Would write to config file {path:?}");
    } else {
        tracing::info!("Writing to config file {path:?}");
    }

    let label_existing = format!("{path:?} (before)");
    let label_new = format!("{path:?} (after)");
    let diff = similar_asserts::SimpleDiff::from_str(
        &existing_config,
        &new_config,
        &label_existing,
        &label_new,
    );

    if dry_run {
        println!("{diff}");
    } else {
        use tokio::io::AsyncWriteExt;
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(false)
            .truncate(true)
            .open(path)
            .await?;
        let mut writer = tokio::io::BufWriter::new(file);
        writer.write_all(new_config.as_bytes()).await?;
        writer.flush().await?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{
            Config, FileConfig, GlobalConfig, InputFile, RegexTemplate, VersionComponentSpec,
        },
        diagnostics::{BufferedPrinter, ToDiagnostics},
        f_string::{PythonFormatString, Value},
    };
    use codespan_reporting::diagnostic::Diagnostic;
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use serde_ini_spanned::{self as ini, value::Options};
    use std::io::Read;
    use std::path::PathBuf;

    fn parse_ini(
        config: &str,
        options: Options,
        printer: &BufferedPrinter,
    ) -> (
        Result<Option<Config>, super::Error>,
        usize,
        Vec<Diagnostic<usize>>,
    ) {
        let mut diagnostics = vec![];
        let file_id = printer.add_source_file("bumpversion.cfg".to_string(), config.to_string());
        let strict = true;
        let config = Config::from_ini(config, options, file_id, strict, &mut diagnostics);
        if let Err(ref err) = config {
            diagnostics.extend(err.to_diagnostics(file_id));
        }
        for diagnostic in &diagnostics {
            printer.emit(diagnostic);
        }
        printer.print();
        (config, file_id, diagnostics)
    }

    #[test]
    fn parse_cfg_ini_simple() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_cfg = indoc::indoc! {r"
            [bumpversion:file:coolapp/__init__.py]

            [bumpversion:file(version heading):CHANGELOG.md]
            search = Unreleased

            [bumpversion:file(previous version):CHANGELOG.md]
            search = {current_version}...HEAD
            replace = {current_version}...{new_version}
        "};

        let config = parse_ini(
            bumpversion_cfg,
            Options::default(),
            &BufferedPrinter::default(),
        )
        .0?;

        let expected = Config {
            global: GlobalConfig::empty(),
            files: vec![
                (
                    InputFile::Path("coolapp/__init__.py".into()),
                    FileConfig::empty(),
                ),
                (
                    InputFile::Path("CHANGELOG.md".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            [Value::String("Unreleased".to_string())]
                                .into_iter()
                                .collect(),
                        )),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("CHANGELOG.md".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            [
                                Value::Argument("current_version".to_string()),
                                Value::String("...HEAD".to_string()),
                            ]
                            .into_iter()
                            .collect(),
                        )),
                        replace: Some("{current_version}...{new_version}".to_string()),
                        ..FileConfig::empty()
                    },
                ),
            ],
            components: [].into_iter().collect(),
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

        let config = parse_ini(
            setup_cfg_ini,
            Options::default(),
            &BufferedPrinter::default(),
        )
        .0?;

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("0.1.8".to_string()),
                commit: Some(true),
                tag: Some(true),
                commit_message: Some(PythonFormatString(vec![Value::String(
                    "DO NOT BUMP VERSIONS WITH THIS FILE".to_string(),
                )])),
                ..GlobalConfig::empty()
            },
            files: vec![
                (InputFile::glob("*.txt"), FileConfig::empty()),
                (InputFile::glob("**/*.txt"), FileConfig::empty()),
                (
                    InputFile::Path("setup.py".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            [
                                Value::String(r#"version = ""#.to_string()),
                                Value::Argument("current_version".to_string()),
                                Value::String(r#"""#.to_string()),
                            ]
                            .into_iter()
                            .collect(),
                        )),
                        replace: Some(r#"version = "{new_version}""#.to_string()),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("favico/__init__.py".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            vec![
                                Value::String(r#"__version__ = ""#.to_string()),
                                Value::Argument("current_version".to_string()),
                                Value::String(r#"""#.to_string()),
                            ]
                            .into_iter()
                            .collect(),
                        )),
                        replace: Some(r#"__version__ = "{new_version}""#.to_string()),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("file1".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            [
                                Value::String("dots: ".to_string()),
                                Value::Argument("current_version".to_string()),
                            ]
                            .into_iter()
                            .collect(),
                        )),
                        replace: Some("dots: {new_version}".to_string()),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("file2".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            [
                                Value::String("dashes: ".to_string()),
                                Value::Argument("current_version".to_string()),
                            ]
                            .into_iter()
                            .collect(),
                        )),
                        replace: Some("dashes: {new_version}".to_string()),
                        parse_version_pattern: Some(
                            regex::Regex::new(r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)")?
                                .into(),
                        ),
                        serialize_version_patterns: Some(vec![
                            "{major}-{minor}-{patch}".to_string()
                        ]),
                        ..FileConfig::empty()
                    },
                ),
            ],
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/basic_cfg.cfg>
    #[test]
    fn parse_compat_basic_cfg_cfg() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_cfg = indoc::indoc! {r"
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
        "};

        let config = parse_ini(
            bumpversion_cfg,
            Options::default(),
            &BufferedPrinter::default(),
        )
        .0?;
        let expected = Config {
            global: GlobalConfig {
                commit: Some(true),
                tag: Some(true),
                current_version: Some("1.0.0".to_string()),
                parse_version_pattern: Some(
                    regex::Regex::new(
                        r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\-(?P<release>[a-z]+))?",
                    )?
                    .into(),
                ),
                serialize_version_patterns: Some(vec![
                    r"{major}.{minor}.{patch}-{release}".to_string(),
                    r"{major}.{minor}.{patch}".to_string(),
                ]),
                ..GlobalConfig::empty()
            },
            files: vec![
                (InputFile::Path("setup.py".into()), FileConfig::empty()),
                (
                    InputFile::Path("bumpversion/__init__.py".into()),
                    FileConfig::empty(),
                ),
                (
                    InputFile::Path("CHANGELOG.md".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            [Value::String("**unreleased**".to_string())]
                                .into_iter()
                                .collect(),
                        )),
                        replace: Some(
                            indoc::indoc! {
                                r"
                                **unreleased**
                                **v{new_version}**"
                            }
                            .to_string(),
                        ),

                        ..FileConfig::empty()
                    },
                ),
            ],
            components: [(
                "release".to_string(),
                VersionComponentSpec {
                    optional_value: Some("gamma".to_string()),
                    values: vec!["dev".to_string(), "gamma".to_string()],
                    ..VersionComponentSpec::default()
                },
            )]
            .into_iter()
            .collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/legacy_multiline_search.cfg>
    #[test]
    fn parse_compat_legacy_multiline_search_cfg() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_cfg = indoc::indoc! {r"
            [bumpversion]
            current_version = 1.0.0

            [bumpversion:file:MULTILINE_SEARCH.md]
            search = **unreleased**
                **v{current_version}**
            replace = **unreleased**
                **v{new_version}**
        "};

        let config = parse_ini(
            bumpversion_cfg,
            Options::default(),
            &BufferedPrinter::default(),
        )
        .0?;
        let expected = Config {
            global: GlobalConfig {
                current_version: Some("1.0.0".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![(
                InputFile::Path("MULTILINE_SEARCH.md".into()),
                FileConfig {
                    search: Some(RegexTemplate::Escaped(
                        [
                            Value::String("**unreleased**\n**v".to_string()),
                            Value::Argument("current_version".to_string()),
                            Value::String("**".to_string()),
                        ]
                        .into_iter()
                        .collect(),
                    )),
                    replace: Some("**unreleased**\n**v{new_version}**".to_string()),
                    ..FileConfig::empty()
                },
            )],
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/legacy_multiline_search_comma.cfg>
    #[test]
    fn parse_compat_legacy_multiline_search_comma_cfg() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_cfg = indoc::indoc! {r"
            [bumpversion]
            current_version = 1.0.0

            [bumpversion:file:MULTILINE_SEARCH.md]
            search = **unreleased**,
                **v{current_version}**,
            replace = **unreleased**,
                **v{new_version}**,
        "};

        let config = parse_ini(
            bumpversion_cfg,
            Options::default(),
            &BufferedPrinter::default(),
        )
        .0?;
        let expected = Config {
            global: GlobalConfig {
                current_version: Some("1.0.0".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![(
                InputFile::Path("MULTILINE_SEARCH.md".into()),
                FileConfig {
                    search: Some(RegexTemplate::Escaped(
                        [
                            Value::String("**unreleased**,\n**v".to_string()),
                            Value::Argument("current_version".to_string()),
                            Value::String("**,".to_string()),
                        ]
                        .into_iter()
                        .collect(),
                    )),
                    replace: Some("**unreleased**,\n**v{new_version}**,".to_string()),
                    ..FileConfig::empty()
                },
            )],
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }
}
