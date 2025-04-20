use crate::{
    config::{
        self, Config, InputFile, file::FileConfig, global::GlobalConfig, regex::RegexTemplate,
        version::VersionComponentSpec,
    },
    diagnostics::{FileId, Span},
    f_string::PythonFormatString,
};
use codespan_reporting::diagnostic::Diagnostic;
use indexmap::IndexMap;
use std::path::PathBuf;
use toml_span as toml;

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("{message}")]
    InvalidConfiguration { message: String, span: Span },
    #[error("{message}")]
    MissingKey {
        key: String,
        message: String,
        span: Span,
    },
    #[error("{message}")]
    MissingOneOf {
        keys: Vec<String>,
        message: String,
        span: Span,
    },
    #[error("{message}")]
    UnexpectedType {
        message: String,
        expected: Vec<ValueKind>,
        found: ValueKind,
        span: Span,
    },
    #[error("{message}")]
    InvalidFormatString {
        #[source]
        source: crate::f_string::ParseError,
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
    Toml {
        #[source]
        source: toml_span::Error,
    },
}

mod diagnostics {
    use crate::diagnostics::ToDiagnostics;
    use codespan_reporting::diagnostic::{Diagnostic, Label};

    impl ToDiagnostics for super::ParseError {
        fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Vec<Diagnostic<F>> {
            match self {
                Self::InvalidFormatString {
                    source,
                    message,
                    span,
                    ..
                } => vec![
                    Diagnostic::error()
                        .with_message("invalid format string".to_string())
                        .with_labels(vec![
                            Label::primary(file_id, span.clone()).with_message(source.to_string()),
                            Label::secondary(file_id, span.clone()).with_message(message),
                        ]),
                ],
                Self::InvalidRegex {
                    source,
                    message,
                    span,
                    ..
                } => vec![
                    Diagnostic::error()
                        .with_message("invalid regular expression".to_string())
                        .with_labels(vec![
                            Label::primary(file_id, span.clone()).with_message(source.to_string()),
                            Label::secondary(file_id, span.clone()).with_message(message),
                        ]),
                ],

                Self::InvalidConfiguration { message, span, .. } => vec![
                    Diagnostic::error()
                        .with_message("invalid configuration".to_string())
                        .with_labels(vec![
                            Label::secondary(file_id, span.clone()).with_message(message),
                        ]),
                ],
                Self::MissingKey {
                    message, key, span, ..
                } => vec![
                    Diagnostic::error()
                        .with_message(format!("missing required key `{key}`"))
                        .with_labels(vec![
                            Label::secondary(file_id, span.clone()).with_message(message),
                        ]),
                ],
                Self::MissingOneOf {
                    message,
                    keys,
                    span,
                    ..
                } => vec![
                    Diagnostic::error()
                        .with_message(format!(
                            "missing one of {}",
                            keys.iter()
                                .map(|key| format!("`{key}`"))
                                .collect::<Vec<_>>()
                                .join(" or ")
                        ))
                        .with_labels(vec![
                            Label::secondary(file_id, span.clone()).with_message(message),
                        ]),
                ],
                Self::UnexpectedType {
                    expected,
                    found,
                    span,
                    ..
                } => {
                    let expected = expected
                        .iter()
                        .map(|ty| format!("`{ty:?}`"))
                        .collect::<Vec<_>>()
                        .join(", or ");
                    let diagnostic = Diagnostic::error()
                        .with_message(self.to_string())
                        .with_labels(vec![
                            Label::primary(file_id, span.clone())
                                .with_message(format!("expected {expected}")),
                        ])
                        .with_notes(vec![unindent::unindent(&format!(
                            "
                        expected type {expected}
                           found type `{found:?}`
                        "
                        ))]);
                    vec![diagnostic]
                }
                Self::Toml { source } => {
                    vec![source.to_diagnostic(file_id)]
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueKind {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Table,
}

impl<'de> From<&toml_span::Value<'de>> for ValueKind {
    fn from(value: &toml_span::Value<'de>) -> Self {
        value.as_ref().into()
    }
}

impl<'de> From<&toml_span::value::ValueInner<'de>> for ValueKind {
    fn from(value: &toml_span::value::ValueInner<'de>) -> Self {
        use toml_span::value::ValueInner;
        match value {
            ValueInner::String(..) => ValueKind::String,
            ValueInner::Integer(..) => ValueKind::Integer,
            ValueInner::Float(..) => ValueKind::Float,
            ValueInner::Boolean(..) => ValueKind::Boolean,
            ValueInner::Array(..) => ValueKind::Array,
            ValueInner::Table(..) => ValueKind::Table,
        }
    }
}

#[inline]
pub fn as_string_array<'de>(value: &'de toml::Value<'de>) -> Result<Vec<String>, ParseError> {
    Ok(as_str_array(value)?
        .into_iter()
        .map(ToString::to_string)
        .collect())
}

#[inline]
#[must_use]
pub fn as_array<'de>(value: &'de toml::Value<'de>) -> Vec<&'de toml::Value<'de>> {
    match value.as_ref() {
        toml::value::ValueInner::Array(array) => array.iter().collect(),
        _ => vec![value],
    }
}

#[inline]
pub fn as_str_array<'de>(value: &'de toml::Value<'de>) -> Result<Vec<&'de str>, ParseError> {
    as_array(value)
        .into_iter()
        .map(|value| as_str(value))
        .collect::<Result<_, _>>()
}

#[inline]
pub fn as_format_string<'de>(
    value: &'de toml::Value<'de>,
) -> Result<PythonFormatString, ParseError> {
    as_str(value).and_then(|s| {
        PythonFormatString::parse(s).map_err(|source| ParseError::InvalidFormatString {
            source,
            message: "invalid format string".to_string(),
            span: value.span.into(),
        })
    })
}

#[inline]
pub fn as_regex<'de>(value: &'de toml::Value<'de>) -> Result<config::regex::Regex, ParseError> {
    as_str(value).and_then(|s| {
        // let s = s.replace("\\\\", "\\");
        // let s = crate::f_string::parser::escape_double_curly_braces(&s).unwrap_or(s);
        regex::Regex::new(s)
            .map(Into::into)
            .map_err(|source| ParseError::InvalidRegex {
                source,
                message: format!("invalid regular expression: {s:?}"),
                span: value.span.into(),
            })
    })
}

#[inline]
pub fn as_string<'de>(value: &'de toml::Value<'de>) -> Result<String, ParseError> {
    as_str(value).map(ToString::to_string)
}

#[inline]
pub fn as_str<'de>(value: &'de toml::Value<'de>) -> Result<&'de str, ParseError> {
    value.as_str().ok_or_else(|| ParseError::UnexpectedType {
        message: "expected a string".to_string(),
        expected: vec![ValueKind::String],
        found: value.into(),
        span: value.span.into(),
    })
}

#[inline]
pub fn as_bool<'de>(value: &'de toml::Value<'de>) -> Result<bool, ParseError> {
    value.as_bool().ok_or_else(|| ParseError::UnexpectedType {
        message: "expected a boolean".to_string(),
        expected: vec![ValueKind::String],
        found: value.into(),
        span: value.span.into(),
    })
}

pub(crate) fn parse_file<'de>(
    value: &'de toml::Value<'de>,
    search_is_regex: Option<bool>,
) -> Result<(InputFile, FileConfig), ParseError> {
    let table = value.as_table().ok_or_else(|| ParseError::UnexpectedType {
        message: "file config must be a table".to_string(),
        expected: vec![ValueKind::Table],
        found: value.into(),
        span: value.span.into(),
    })?;
    let file_name = table.get("filename").map(as_string).transpose()?;
    let glob_pattern = table.get("glob").map(as_string).transpose()?;

    let input_file = match (file_name, glob_pattern) {
        (Some(_), Some(_)) => Err(ParseError::InvalidConfiguration {
            message: "file config must specify exactly one of `filename` and `glob`".to_string(),
            span: value.span.into(),
        }),
        (None, None) => Err(ParseError::MissingOneOf {
            keys: vec!["filename".to_string(), "glob".to_string()],
            message: "file config must specify either `filename` or `glob`".to_string(),
            span: value.span.into(),
        }),
        (Some(file_name), None) => Ok(InputFile::Path(file_name.into())),
        (None, Some(glob_pattern)) => {
            let exclude_patterns = table.get("glob_exclude").map(as_string_array).transpose()?;
            Ok(InputFile::GlobPattern {
                pattern: glob_pattern,
                exclude_patterns,
            })
        }
    }?;

    let file_config = parse_file_config(table, search_is_regex)?;
    Ok((input_file, file_config))
}

pub(crate) fn parse_part_config<'de>(
    value: &'de toml::value::Value<'de>,
) -> Result<VersionComponentSpec, ParseError> {
    let table = value.as_table().ok_or_else(|| ParseError::UnexpectedType {
        message: "part config must be a table".to_string(),
        expected: vec![ValueKind::Table],
        found: value.into(),
        span: value.span.into(),
    })?;
    let independent = table.get("independent").map(as_bool).transpose()?;
    let optional_value = table.get("optional_value").map(as_string).transpose()?;
    let values = table
        .get("values")
        .map(as_string_array)
        .transpose()?
        .unwrap_or_default();

    Ok(VersionComponentSpec {
        independent,
        optional_value,
        values,
        ..VersionComponentSpec::default()
    })
}

fn parse_search_pattern<'de>(
    table: &'de toml::value::Table<'de>,
    is_regex: Option<bool>,
) -> Result<(Option<bool>, Option<RegexTemplate>), ParseError> {
    let search_is_regex_compat = table.get("regex").map(as_bool).transpose()?.or(is_regex);
    let search = table
        .get("search")
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

pub(crate) fn parse_global_config<'de>(
    table: &'de toml::value::Table<'de>,
) -> Result<(Option<bool>, GlobalConfig), ParseError> {
    let current_version = table.get("current_version").map(as_string).transpose()?;

    let (is_regex, search) = parse_search_pattern(table, None)?;

    let allow_dirty = table.get("allow_dirty").map(as_bool).transpose()?;
    let parse_version_pattern = table.get("parse").map(as_regex).transpose()?;
    let serialize_version_patterns = table
        .get("serialize")
        .map(as_array)
        .map(|patterns| {
            patterns
                .into_iter()
                .map(as_format_string)
                .collect::<Result<_, _>>()
        })
        .transpose()?;

    let replace = table.get("replace").map(as_string).transpose()?;
    let no_configured_files = table.get("no_configured_files").map(as_bool).transpose()?;
    let ignore_missing_files = table.get("ignore_missing_files").map(as_bool).transpose()?;
    let ignore_missing_version = table
        .get("ignore_missing_version")
        .map(as_bool)
        .transpose()?;
    let dry_run = table.get("dry_run").map(as_bool).transpose()?;
    let commit = table.get("commit").map(as_bool).transpose()?;
    let tag = table.get("tag").map(as_bool).transpose()?;
    let sign_tags = table
        .get("sign_tag")
        .or(table.get("sign_tags"))
        .map(as_bool)
        .transpose()?;
    let tag_name = table.get("tag_name").map(as_format_string).transpose()?;
    let tag_message = table.get("tag_message").map(as_format_string).transpose()?;
    let commit_message = table
        .get("commit_message")
        .or(table.get("message"))
        .map(as_format_string)
        .transpose()?;
    let commit_args = table.get("commit_args").map(as_string).transpose()?;

    // extra stuff
    let setup_hooks = table.get("setup_hooks").map(as_string_array).transpose()?;
    let pre_commit_hooks = table
        .get("pre_commit_hooks")
        .map(as_string_array)
        .transpose()?;
    let post_commit_hooks = table
        .get("post_commit_hooks")
        .map(as_string_array)
        .transpose()?;
    let included_paths = table
        .get("included_paths")
        .map(as_string_array)
        .transpose()?
        .map(|values| values.into_iter().map(PathBuf::from).collect());
    let excluded_paths = table
        .get("excluded_paths")
        .map(as_string_array)
        .transpose()?
        .map(|values| values.into_iter().map(PathBuf::from).collect());
    let additional_files = table
        .get("additional_files")
        .map(as_string_array)
        .transpose()?
        .map(|values| values.into_iter().map(PathBuf::from).collect());

    Ok((
        is_regex,
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
            additional_files,
        },
    ))
}

pub(crate) fn parse_file_config<'de>(
    table: &'de toml::value::Table<'de>,
    search_is_regex: Option<bool>,
) -> Result<FileConfig, ParseError> {
    let (_, search) = parse_search_pattern(table, search_is_regex)?;
    let parse_version_pattern = table.get("parse").map(as_regex).transpose()?;
    let serialize_version_patterns = table
        .get("serialize")
        .map(as_array)
        .map(|patterns| {
            patterns
                .into_iter()
                .map(as_format_string)
                .collect::<Result<_, _>>()
        })
        .transpose()?;
    let replace = table.get("replace").map(as_string).transpose()?;
    let ignore_missing_file = table
        .get("ignore_missing_files")
        .or(table.get("ignore_missing_file"))
        .map(as_bool)
        .transpose()?;
    let ignore_missing_version = table
        .get("ignore_missing_version")
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
    pub fn from_pyproject_value(
        config: &toml::Value,
        _file_id: FileId,
        _strict: bool,
        _diagnostics: &mut [Diagnostic<FileId>],
    ) -> Result<Option<Self>, ParseError> {
        let Some((_, config)) = config
            .as_table()
            .and_then(|table| table.get("tool"))
            .and_then(|tool| tool.as_table())
            .and_then(|tool| tool.get_key_value("bumpversion"))
        else {
            return Ok(None);
        };

        let table = config
            .as_table()
            .ok_or_else(|| ParseError::UnexpectedType {
                message: "bumpversion config must be a table".to_string(),
                expected: vec![ValueKind::Table],
                found: config.into(),
                span: config.span.into(),
            })?;

        if table.is_empty() {
            return Ok(None);
        }

        let (is_regex_compat, global_file_config) = parse_global_config(table)?;

        let files = match table.get("files") {
            None => vec![],
            Some(value) => match value.as_ref() {
                toml::value::ValueInner::Array(array) => array
                    .iter()
                    .map(|value| parse_file(value, is_regex_compat))
                    .collect::<Result<Vec<(InputFile, FileConfig)>, _>>()?,
                _ => {
                    return Err(ParseError::UnexpectedType {
                        message: "files must be an array must be a table".to_string(),
                        expected: vec![ValueKind::Table],
                        found: value.into(),
                        span: value.span.into(),
                    });
                }
            },
        };

        let components = match table.get("parts") {
            None => IndexMap::new(),
            Some(value) => match value.as_ref() {
                toml::value::ValueInner::Table(table) => table
                    .iter()
                    .map(|(key, value)| {
                        let part_config = parse_part_config(value)?;
                        Ok((key.name.to_string(), part_config))
                    })
                    .collect::<Result<Vec<(String, VersionComponentSpec)>, _>>()?
                    .into_iter()
                    .collect(),
                _ => {
                    return Err(ParseError::UnexpectedType {
                        message: "parts must be a table".to_string(),
                        expected: vec![ValueKind::Table],
                        found: value.into(),
                        span: value.span.into(),
                    });
                }
            },
        };

        Ok(Some(Self {
            global: global_file_config,
            files,
            components,
        }))
    }

    pub fn from_pyproject_toml(
        config: &str,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut [Diagnostic<FileId>],
    ) -> Result<Option<Self>, ParseError> {
        let config = toml_span::parse(config).map_err(|source| ParseError::Toml { source })?;
        Self::from_pyproject_value(&config, file_id, strict, diagnostics)
    }
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
pub mod tests {
    use crate::{
        config::{
            self, Config, InputFile, change::FileChange, file::FileConfig, global::GlobalConfig,
            regex::RegexTemplate, version::VersionComponentSpec,
        },
        diagnostics::{BufferedPrinter, ToDiagnostics},
        f_string::{PythonFormatString, Value},
    };
    use codespan_reporting::diagnostic::Diagnostic;
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use similar_asserts::assert_eq as sim_assert_eq;

    use std::path::PathBuf;

    pub(crate) fn parse_toml(
        config: &str,
        printer: &BufferedPrinter,
    ) -> (
        Result<Option<Config>, super::ParseError>,
        usize,
        Vec<Diagnostic<usize>>,
    ) {
        let mut diagnostics = vec![];
        let file_id = printer.add_source_file("bumpversion.toml".to_string(), config.to_string());
        let strict = true;
        let config = Config::from_pyproject_toml(config, file_id, strict, &mut diagnostics);
        if let Err(ref err) = config {
            diagnostics.extend(err.to_diagnostics(file_id));
        }
        dbg!(&diagnostics);
        for diagnostic in &diagnostics {
            printer.emit(diagnostic).expect("emit diagnostics");
        }
        printer.print().expect("print diagnostics");
        (config, file_id, diagnostics)
    }

    #[test]
    fn parse_pyproject_toml_simple() -> eyre::Result<()> {
        crate::tests::init();

        let pyproject_toml = indoc::indoc! {r#"
            [tool.poetry]
            name = "ai2"
            version = "0.1.0"
            description = ""
            authors = ["roman <roman@luup-systems.com>"]

            [tool.poetry.dependencies]
            python = "^3.10"
            luup = {path = "../../packages/python/proto", develop = true}

            [tool.poetry.group.dev.dependencies]
            pytest = "^8.3.3"
            mypy = "^1.11.2"
            ruff = "^0.6.9"

            [tool.bumpversion]
            current_version = "1.2.3"

            [[tool.bumpversion.files]]
            filename = "config.ini"

            search = """
            [myproject]
            version={current_version}"""

            replace = """
            [myproject]
            version={new_version}"""
        "#};

        let config = parse_toml(pyproject_toml, &BufferedPrinter::default()).0?;
        println!("config: {config:#?}");

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("1.2.3".to_string()),
                ..GlobalConfig::empty()
            },
            files: [(
                InputFile::Path("config.ini".into()),
                FileConfig {
                    search: Some(RegexTemplate::Escaped(
                        [
                            Value::String("[myproject]\nversion=".to_string()),
                            Value::Argument("current_version".to_string()),
                        ]
                        .into_iter()
                        .collect(),
                    )),
                    replace: Some(
                        indoc::indoc! {r"
                        [myproject]
                        version={new_version}"}
                        .to_string(),
                    ),
                    ..FileConfig::empty()
                },
            )]
            .into_iter()
            .collect(),
            components: [].into_iter().collect(),
        };
        sim_assert_eq!(config, Some(expected));
        Ok(())
    }

    #[test]
    fn parse_pyproject_toml_complex() -> eyre::Result<()> {
        crate::tests::init();

        let pyproject_toml = indoc::indoc! {r#"
            [build-system]
            requires = ["hatchling"]
            build-backend = "hatchling.build"

            [project]
            name = "bump-my-version"
            description = "Version bump your Python project"
            authors = [
                { name = "Corey Oordt", email = "coreyoordt@gmail.com" }
            ]
            classifiers = [
                "Development Status :: 5 - Production/Stable",
                "Environment :: Console",
                "Intended Audience :: Developers",
                "License :: OSI Approved :: MIT License",
                "Operating System :: OS Independent",
                "Programming Language :: Python",
                "Programming Language :: Python :: 3 :: Only",
                "Programming Language :: Python :: 3.8",
                "Programming Language :: Python :: 3.9",
                "Programming Language :: Python :: 3.10",
                "Programming Language :: Python :: 3.11",
                "Programming Language :: Python :: 3.12",
                "Programming Language :: Python :: Implementation :: PyPy",
                "Topic :: Software Development :: Build Tools",
                "Topic :: Software Development :: Version Control",
                "Topic :: System :: Software Distribution",
            ]
            readme = "README.md"
            requires-python = ">=3.8"
            license = { file = "LICENSE" }
            keywords = ["bumpversion", "version", "release"]
            dynamic = ["version"]
            dependencies = [
                "click",
                "pydantic>=2.0.0",
                "pydantic-settings",
                "questionary",
                "rich-click",
                "rich",
                "tomlkit",
                "wcmatch>=8.5.1",
            ]

            [project.scripts]
            bump-my-version = "bumpversion.cli:cli"


            [project.urls]
            homepage = "https://github.com/callowayproject/bump-my-version"
            repository = "https://github.com/callowayproject/bump-my-version.git"
            documentation = "https://callowayproject.github.io/bump-my-version/"

            [project.optional-dependencies]
            dev = [
                "git-fame>=1.12.2",
                "generate-changelog>=0.7.6",
                "pip-tools",
                "pre-commit",
            ]
            docs = [
                "black",
                "markdown-customblocks",
                "mdx-truly-sane-lists",
                "mkdocs",
                "mkdocs-click",
                "mkdocs-drawio",
                "mkdocs-gen-files",
                "mkdocs-git-authors-plugin",
                "mkdocs-git-committers-plugin",
                "mkdocs-git-revision-date-localized-plugin>=1.2.6",
                "mkdocs-include-markdown-plugin",
                "mkdocs-literate-nav",
                "mkdocs-material",
                "mkdocstrings[python]",
                "python-frontmatter",
            ]
            test = [
                "coverage",
                "freezegun",
                "pre-commit",
                "pytest-cov",
                "pytest",
                "pytest-mock",
                "pytest-sugar",
            ]

            [tool.hatch.version]
            path = "bumpversion/__init__.py"

            [tool.hatch.build.targets.wheel]
            packages = ["bumpversion"]


            [tool.coverage.run]
            branch = true
            omit = ["**/test_*.py", "**/__main__.py", "**/aliases.py"]

            [tool.coverage.report]
            omit = [
                "*site-packages*",
                "*tests*",
                "*.tox*",
            ]
            show_missing = true
            exclude_lines = [
                "raise NotImplementedError",
                "pragma: no-coverage",
                "pragma: no-cov",
            ]

            [tool.coverage.html]
            directory = "test-reports/htmlcov"

            [tool.coverage.xml]
            output = "test-reports/coverage.xml"

            [tool.pytest.ini_options]
            norecursedirs = [
                ".*",
                "build",
                "dist",
                "{arch}",
                "*.egg",
                "venv",
                "requirements*",
                "lib",
            ]
            python_files = "test*.py"
            addopts = [
                "--cov=bumpversion",
                "--cov-branch",
                "--cov-report=term",
                "--cov-report=html",
            ]

            [tool.interrogate]
            ignore-init-method = true
            ignore-init-module = false
            ignore-magic = true
            ignore-semiprivate = false
            ignore-private = false
            ignore-property-decorators = false
            ignore-module = false
            ignore-nested-functions = true
            ignore-nested-classes = true
            ignore-setters = false
            fail-under = 95
            exclude = ["setup.py", "docs", "build"]
            ignore-regex = ["^get$", "^mock_.*", ".*BaseClass.*"]
            verbose = 0
            quiet = false
            whitelist-regex = []
            color = true

            [tool.black]
            line-length = 119

            [tool.ruff]
            exclude = [
                ".bzr",
                ".direnv",
                ".eggs",
                ".git",
                ".hg",
                ".mypy_cache",
                ".nox",
                ".pants.d",
                ".pytype",
                ".ruff_cache",
                ".svn",
                ".tox",
                ".venv",
                "__pypackages__",
                "_build",
                "buck-out",
                "build",
                "dist",
                "node_modules",
                "venv",
            ]

            # Same as Black.
            line-length = 119

            [tool.ruff.lint]
            preview = true
            select = [
                "E", # pycodestyle errors
                "W", # pycodestyle warnings
                "F", # pyflakes
                "I", # isort
                "N", # PEP8 naming
                "B", # flake8-bugbear
                "BLE", # flake8-blind except
                "D", # pydocstyle
                # "DOC", # pydoclint
                "S", # flakeu-bandit
                "RUF", # Ruff-specific rules
                "NPY", # NumPy-specific rules
                "PD", # Pandas-vet
                "PGH", # PyGrep hooks
                "ANN", # flake8-annotations
                "C90", # McCabe complexity
                "PLC", # Pylint conventions
                "PLE", # Pylint errors
                "PLW", # Pylint warnings
                "TCH", # Flake8 type-checking
            ]
            ignore = [
                "ANN002", # missing-type-args
                "ANN003", # missing-type-kwargs
                "ANN101", # missing-type-self
                "ANN102", # missing-type-cls
                "ANN204", # missing-return-type-special-method
                "ANN401", # any-type
                "S101", # assert
                "S104", # hardcoded-bind-all-interfaces
                "S404", # suspicious-subprocess-import
                "S602", # subprocess-popen-with-shell-equals-true
                "D105", # undocumented-magic-method
                "D106", # undocumented-public-nested-class
                "D107", # undocumented-public-init
                "D200", # fits-on-one-line
                "D212", # multi-line-summary-first-line
                "PD011", # pandas-use-of-dot-values
                "PLC0415", # import-outside-toplevel
                "PLW1641", # eq-without-hash
            ]

            fixable = ["ALL"]
            unfixable = []

            # Allow unused variables when underscore-prefixed.
            dummy-variable-rgx = "^(_+|(_+[a-zA-Z0-9_]*[a-zA-Z0-9]+?))$"

            typing-modules = ["typing", "types", "typing_extensions", "mypy", "mypy_extensions"]

            [tool.ruff.lint.per-file-ignores]
            "tests/*" = ["S101", "PLR0913", "PLR0915", "PGH003", "ANN001", "ANN202", "ANN201", "PLR0912", "TRY301", "PLW0603", "PLR2004", "ANN101", "S106", "TRY201", "ANN003", "ANN002", "S105", "TRY003"]

            [tool.ruff.lint.mccabe]
            # Unlike Flake8, default to a complexity level of 10.
            max-complexity = 10

            [tool.ruff.lint.isort]
            order-by-type = true

            [tool.ruff.lint.pydocstyle]
            convention = "google"

            [tool.ruff.lint.flake8-annotations]
            allow-star-arg-any = true
            mypy-init-return = true
            suppress-dummy-args = true
            suppress-none-returning = true

            [tool.bumpversion]
            current_version = "0.28.1"
            commit = true
            commit_args = "--no-verify"
            tag = true
            tag_name = "{new_version}"
            allow_dirty = true
            parse = "(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\.(?P<dev>post)\\d+\\.dev\\d+)?"
            serialize = [
                "{major}.{minor}.{patch}.{dev}{$PR_NUMBER}.dev{distance_to_latest_tag}",
                "{major}.{minor}.{patch}"
            ]
            message = "Version updated from {current_version} to {new_version}"

            [tool.bumpversion.parts.dev]
            values = ["release", "post"]

            [[tool.bumpversion.files]]
            filename = "bumpversion/__init__.py"

            [[tool.bumpversion.files]]
            filename = "CHANGELOG.md"
            search = "Unreleased"

            [[tool.bumpversion.files]]
            filename = "CHANGELOG.md"
            search = "{current_version}...HEAD"
            replace = "{current_version}...{new_version}"

            [[tool.bumpversion.files]]
            filename = "action.yml"
            search = "bump-my-version=={current_version}"
            replace = "bump-my-version=={new_version}"

            [[tool.bumpversion.files]]
            filename = "Dockerfile"
            search = "created=\\d{{4}}-\\d{{2}}-\\d{{2}}T\\d{{2}}:\\d{{2}}:\\d{{2}}Z"
            replace = "created={utcnow:%Y-%m-%dT%H:%M:%SZ}"
            regex = true

            [[tool.bumpversion.files]]
            filename = "Dockerfile"

            [tool.pydoclint]
            style = "google"
            exclude = '\.git|tests'
            require-return-section-when-returning-nothing = false
            arg-type-hints-in-docstring = false
            check-return-types = false
            quiet = true
            check-class-attributes = false
        "#};

        let config = parse_toml(pyproject_toml, &BufferedPrinter::default()).0?;
        println!("config: {config:#?}");

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("0.28.1".to_string()),
                commit: Some(true),
                commit_args: Some("--no-verify".to_string()),
                tag: Some(true),
                tag_name: Some(PythonFormatString(vec![Value::Argument("new_version".to_string())])),
                allow_dirty: Some(true),
                parse_version_pattern: Some(regex::Regex::new("(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\.(?P<dev>post)\\d+\\.dev\\d+)?")?.into()),
                serialize_version_patterns: Some(vec![
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("dev".to_string()),
                        Value::Argument("$PR_NUMBER".to_string()),
                        Value::String(".dev".to_string()),
                        Value::Argument("distance_to_latest_tag".to_string()),
                    ].into_iter().collect(),
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                    ].into_iter().collect(),
                ]),
                commit_message: Some(PythonFormatString(vec![
                    Value::String("Version updated from ".to_string()),
                    Value::Argument("current_version".to_string()),
                    Value::String(" to ".to_string()),
                    Value::Argument("new_version".to_string()),
                ])),
                ..GlobalConfig::empty()
            },
            files: [
                (
                    InputFile::Path("bumpversion/__init__.py".into()),
                    FileConfig::empty()
                ),
                (
                    InputFile::Path("CHANGELOG.md".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped([
                            Value::String("Unreleased".to_string()),
                        ].into_iter().collect())),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("CHANGELOG.md".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped([
                            Value::Argument("current_version".to_string()),
                            Value::String("...HEAD".to_string()),
                        ].into_iter().collect())),
                        replace: Some("{current_version}...{new_version}".to_string()),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("action.yml".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped([
                            Value::String("bump-my-version==".to_string()),
                            Value::Argument("current_version".to_string()),
                        ].into_iter().collect())),
                        replace: Some("bump-my-version=={new_version}".to_string()),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("Dockerfile".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Regex([
                            Value::String(r"created=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z".to_string())
                        ].into_iter().collect())),
                        replace: Some("created={utcnow:%Y-%m-%dT%H:%M:%SZ}".to_string()),
                        // is_regex: Some(true),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("Dockerfile".into()),
                    FileConfig::empty(),
                ),
            ]
            .into_iter()
            .collect(),
            components: [
                (
                    "dev".to_string(), 
                    VersionComponentSpec{
                        values: vec!["release".to_string(), "post".to_string()],
                        ..VersionComponentSpec::default()
                    }
                )
            ].into_iter().collect(),
        };
        sim_assert_eq!(config, Some(expected));
        Ok(())
    }

    #[test]
    fn parse_pyproject_toml_compat() -> eyre::Result<()> {
        crate::tests::init();

        let pyproject_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "0.10.5"
            parse = "(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\-(?P<release>[a-z]+))?"
            serialize = [
                "{major}.{minor}.{patch}-{release}",
                "{major}.{minor}.{patch}"
            ]
        "#};
        let expected = Config {
            global: GlobalConfig {
                current_version: Some("0.10.5".to_string()),
                parse_version_pattern: Some(
                    regex::Regex::new(
                        r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\-(?P<release>[a-z]+))?",
                    )?
                    .into(),
                ),
                serialize_version_patterns: Some(vec![
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                        Value::String("-".to_string()),
                        Value::Argument("release".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ]),
                ..GlobalConfig::empty()
            },
            ..Config::default()
        };

        let config = parse_toml(pyproject_toml, &BufferedPrinter::default()).0?;
        sim_assert_eq!(config, Some(expected));

        let pyproject_toml = indoc::indoc! {r#"
            [tool.pytest.ini_options]
            norecursedirs = [
                ".*",
                "build",
                "dist",
                "{arch}",
                "*.egg",
                "venv",
                "requirements*",
                "lib",
            ]
            python_files = "test*.py"
            addopts = [
                "--cov=bumpversion",
                "--cov-branch",
                "--cov-report=term",
                "--cov-report=html",
            ]

            [tool.bumpversion]
            commit = true
            tag = true
            current_version = "1.0.0"
            parse = "(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\-(?P<release>[a-z]+))?"
            serialize = [
                "{major}.{minor}.{patch}-{release}",
                "{major}.{minor}.{patch}"
            ]
            [[tool.bumpversion.files]]
            filename = "setup.py"

            [[tool.bumpversion.files]]
            filename = "bumpversion/__init__.py"

            [[tool.bumpversion.files]]
            filename = "CHANGELOG.md"
            search = "**unreleased**"
            replace = """**unreleased**
            **v{new_version}**"""

            [tool.bumpversion.parts.release]
            optional_value = "gamma"
            values =[
                "dev",
                "gamma",
            ]

            [tool.othertool]
            bake_cookies = true
            ignore-words-list = "sugar, salt, flour"
        "#};

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("1.0.0".to_string()),
                commit: Some(true),
                tag: Some(true),
                parse_version_pattern: Some(
                    regex::Regex::new(
                        r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\-(?P<release>[a-z]+))?",
                    )?
                    .into(),
                ),
                serialize_version_patterns: Some(vec![
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                        Value::String("-".to_string()),
                        Value::Argument("release".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ]),
                ..GlobalConfig::empty()
            },
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
                            [Value::String(r"**unreleased**".to_string())]
                                .into_iter()
                                .collect(),
                        )),
                        replace: Some(
                            indoc::indoc! {
                            r"
                            **unreleased**
                            **v{new_version}**"}
                            .to_string(),
                        ),
                        ..FileConfig::empty()
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };

        let config = parse_toml(pyproject_toml, &BufferedPrinter::default()).0?;
        sim_assert_eq!(config, Some(expected));

        Ok(())
    }

    #[test]
    fn parse_pyproject_toml_without_config() -> eyre::Result<()> {
        crate::tests::init();

        let pyproject_toml = indoc::indoc! {r#"
            [build-system]
            requires = ["hatchling"]
            build-backend = "hatchling.build"

            [project]
            name = "bump-my-version"
            description = "Version bump your Python project"
            authors = [
                { name = "Corey Oordt", email = "coreyoordt@gmail.com" }
            ]
            classifiers = [
                "Development Status :: 5 - Production/Stable",
                "Environment :: Console",
                "Intended Audience :: Developers",
                "License :: OSI Approved :: MIT License",
                "Operating System :: OS Independent",
                "Programming Language :: Python",
                "Programming Language :: Python :: 3 :: Only",
                "Programming Language :: Python :: 3.8",
                "Programming Language :: Python :: 3.9",
                "Programming Language :: Python :: 3.10",
                "Programming Language :: Python :: 3.11",
                "Programming Language :: Python :: 3.12",
                "Programming Language :: Python :: Implementation :: PyPy",
                "Topic :: Software Development :: Build Tools",
                "Topic :: Software Development :: Version Control",
                "Topic :: System :: Software Distribution",
            ]
            readme = "README.md"
            requires-python = ">=3.8"
            license = { file = "LICENSE" }
            keywords = ["bumpversion", "version", "release"]
            dynamic = ["version"]
            dependencies = [
                "click",
                "pydantic>=2.0.0",
                "pydantic-settings",
                "questionary",
                "rich-click",
                "rich",
                "tomlkit",
                "wcmatch>=8.5.1",
            ]

            [project.scripts]
            bump-my-version = "bumpversion.cli:cli"
        "#};

        let config = parse_toml(pyproject_toml, &BufferedPrinter::default()).0?;
        sim_assert_eq!(config, None);
        Ok(())
    }

    #[test]
    fn parse_pyproject_toml_with_empty_config() -> eyre::Result<()> {
        crate::tests::init();

        let pyproject_toml = indoc::indoc! {r#"
            [build-system]
            requires = ["hatchling"]
            build-backend = "hatchling.build"

            [project]
            name = "bump-my-version"
            description = "Version bump your Python project"
            authors = [
                { name = "Corey Oordt", email = "coreyoordt@gmail.com" }
            ]
            classifiers = [
                "Development Status :: 5 - Production/Stable",
                "Environment :: Console",
                "Intended Audience :: Developers",
                "License :: OSI Approved :: MIT License",
                "Operating System :: OS Independent",
                "Programming Language :: Python",
                "Programming Language :: Python :: 3 :: Only",
                "Programming Language :: Python :: 3.8",
                "Programming Language :: Python :: 3.9",
                "Programming Language :: Python :: 3.10",
                "Programming Language :: Python :: 3.11",
                "Programming Language :: Python :: 3.12",
                "Programming Language :: Python :: Implementation :: PyPy",
                "Topic :: Software Development :: Build Tools",
                "Topic :: Software Development :: Version Control",
                "Topic :: System :: Software Distribution",
            ]
            readme = "README.md"
            requires-python = ">=3.8"
            license = { file = "LICENSE" }
            keywords = ["bumpversion", "version", "release"]
            dynamic = ["version"]
            dependencies = [
                "click",
                "pydantic>=2.0.0",
                "pydantic-settings",
                "questionary",
                "rich-click",
                "rich",
                "tomlkit",
                "wcmatch>=8.5.1",
            ]

            [tool.bumpversion]

            [project.scripts]
            bump-my-version = "bumpversion.cli:cli"
        "#};

        let config = parse_toml(pyproject_toml, &BufferedPrinter::default()).0?;
        sim_assert_eq!(config, None);
        Ok(())
    }

    #[test]
    fn test_valid_pyproject_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "1.0.0"
        "#};
        let config = parse_toml(bumpversion_toml, &BufferedPrinter::default()).0?;
        dbg!(config);

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("1.0.0".to_string()),
                ..GlobalConfig::empty()
            },
            ..Config::default()
        };
        let config = parse_toml(bumpversion_toml, &BufferedPrinter::default()).0?;
        sim_assert_eq!(config, Some(expected));

        Ok(())
    }

    #[test]
    fn parse_pyproject_toml_with_part_config() -> eyre::Result<()> {
        crate::tests::init();

        let pyproject_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "1.0.0"
            parse = """(?x)
                (?P<major>[0-9]+)
                \\.(?P<minor>[0-9]+)
                \\.(?P<patch>[0-9]+)
                (?:
                    -(?P<pre_label>alpha|beta|stable)
                    (?:-(?P<pre_n>[0-9]+))?
                )?
            """
            serialize = [
                "{major}.{minor}.{patch}-{pre_label}-{pre_n}",
                "{major}.{minor}.{patch}",
            ]

            [tool.bumpversion.parts.pre_label]
            optional_value = "stable"
            values =[
                "alpha",
                "beta",
                "stable",
            ]
        "#};

        let config = parse_toml(pyproject_toml, &BufferedPrinter::default()).0?;

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("1.0.0".to_string()),
                parse_version_pattern: Some(
                    regex::Regex::new(indoc::indoc! {r"
                        (?x)
                            (?P<major>[0-9]+)
                            \.(?P<minor>[0-9]+)
                            \.(?P<patch>[0-9]+)
                            (?:
                                -(?P<pre_label>alpha|beta|stable)
                                (?:-(?P<pre_n>[0-9]+))?
                            )?
                    "})?
                    .into(),
                ),
                serialize_version_patterns: Some(vec![
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                        Value::String("-".to_string()),
                        Value::Argument("pre_label".to_string()),
                        Value::String("-".to_string()),
                        Value::Argument("pre_n".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                    [
                        Value::Argument("major".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("minor".to_string()),
                        Value::String(".".to_string()),
                        Value::Argument("patch".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ]),
                ..GlobalConfig::empty()
            },
            files: [].into_iter().collect(),
            components: [(
                "pre_label".to_string(),
                VersionComponentSpec {
                    optional_value: Some("stable".to_string()),
                    values: vec![
                        "alpha".to_string(),
                        "beta".to_string(),
                        "stable".to_string(),
                    ],
                    ..VersionComponentSpec::default()
                },
            )]
            .into_iter()
            .collect(),
        };
        sim_assert_eq!(config, Some(expected));
        Ok(())
    }

    #[test]
    fn parse_pyproject_toml_of_bump_my_version() -> eyre::Result<()> {
        use crate::config::MergeWith;

        crate::tests::init();
        let pyproject_toml = include_str!("../../test-data/bump-my-version.pyproject.toml");
        let mut config = parse_toml(pyproject_toml, &BufferedPrinter::default())
            .0?
            .unwrap();

        let parse_regex: config::regex::Regex = regex::Regex::new(
            r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\.(?P<dev>post)\d+\.dev\d+)?",
        )?
        .into();
        let serialize = vec![
            [
                Value::Argument("major".to_string()),
                Value::String(".".to_string()),
                Value::Argument("minor".to_string()),
                Value::String(".".to_string()),
                Value::Argument("patch".to_string()),
                Value::String(".".to_string()),
                Value::Argument("dev".to_string()),
                Value::Argument("$PR_NUMBER".to_string()),
                Value::String(".dev".to_string()),
                Value::Argument("distance_to_latest_tag".to_string()),
            ]
            .into_iter()
            .collect(),
            [
                Value::Argument("major".to_string()),
                Value::String(".".to_string()),
                Value::Argument("minor".to_string()),
                Value::String(".".to_string()),
                Value::Argument("patch".to_string()),
            ]
            .into_iter()
            .collect(),
        ];
        let expected = Config {
            global: GlobalConfig {
                current_version: Some("0.29.0".to_string()),
                commit: Some(true),
                commit_args: Some("--no-verify".to_string()),
                tag: Some(true),
                tag_name: Some(PythonFormatString(vec![Value::Argument(
                    "new_version".to_string(),
                )])),
                allow_dirty: Some(true),
                parse_version_pattern: Some(parse_regex.clone()),
                serialize_version_patterns: Some(serialize.clone()),
                commit_message: Some(PythonFormatString(vec![
                    Value::String("Version updated from ".to_string()),
                    Value::Argument("current_version".to_string()),
                    Value::String(" to ".to_string()),
                    Value::Argument("new_version".to_string()),
                ])),
                pre_commit_hooks: Some(vec![
                    "uv sync --upgrade".to_string(),
                    "git add uv.lock".to_string(),
                ]),
                ..GlobalConfig::empty()
            },
            files: [
                (
                    InputFile::Path("bumpversion/__init__.py".into()),
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
                (
                    InputFile::Path("action.yml".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Escaped(
                            [
                                Value::String("bump-my-version==".to_string()),
                                Value::Argument("current_version".to_string()),
                            ]
                            .into_iter()
                            .collect(),
                        )),
                        replace: Some("bump-my-version=={new_version}".to_string()),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("Dockerfile".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Regex(
                            [Value::String(
                                r"created=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z".to_string(),
                            )]
                            .into_iter()
                            .collect(),
                        )),
                        replace: Some(r"created={utcnow:%Y-%m-%dT%H:%M:%SZ}".to_string()),
                        // regex: Some(true),
                        ..FileConfig::empty()
                    },
                ),
                (InputFile::Path("Dockerfile".into()), FileConfig::empty()),
            ]
            .into_iter()
            .collect(),
            components: [(
                "dev".to_string(),
                VersionComponentSpec {
                    values: vec!["release".to_string(), "post".to_string()],
                    ..VersionComponentSpec::default()
                },
            )]
            .into_iter()
            .collect(),
        };
        sim_assert_eq!(&config, &expected);

        // the order is important here
        config.global.merge_with(&GlobalConfig::default());
        config.merge_file_configs_with_global_config();
        // config.apply_defaults(&GlobalConfig::default());

        sim_assert_eq!(
            &config.global,
            &GlobalConfig {
                allow_dirty: Some(true),
                tag: Some(true),
                sign_tags: Some(false),
                search: Some(RegexTemplate::Escaped(
                    [Value::Argument("current_version".to_string()),]
                        .into_iter()
                        .collect()
                )),
                replace: Some("{new_version}".to_string()),
                tag_name: Some(PythonFormatString(vec![Value::Argument(
                    "new_version".to_string()
                )])),
                commit: Some(true),
                commit_args: Some("--no-verify".to_string()),
                current_version: Some("0.29.0".to_string()),
                ignore_missing_files: Some(false),
                ignore_missing_version: Some(false),
                commit_message: Some(PythonFormatString(vec![
                    Value::String("Version updated from ".to_string()),
                    Value::Argument("current_version".to_string()),
                    Value::String(" to ".to_string()),
                    Value::Argument("new_version".to_string()),
                ])),
                tag_message: Some(PythonFormatString(vec![
                    Value::String("Bump version: ".to_string()),
                    Value::Argument("current_version".to_string()),
                    Value::String(" → ".to_string()),
                    Value::Argument("new_version".to_string()),
                ])),
                parse_version_pattern: Some(parse_regex.clone()),
                serialize_version_patterns: Some(serialize.clone()),
                pre_commit_hooks: Some(vec![
                    "uv sync --upgrade".to_string(),
                    "git add uv.lock".to_string()
                ]),
                // overrides from default global config
                setup_hooks: Some(vec![]),
                post_commit_hooks: Some(vec![]),
                dry_run: Some(false),
                no_configured_files: Some(false),
                ..GlobalConfig::empty()
            },
        );

        let mut config = config.finalize();
        let component_configs = config::version::version_component_configs(&config);
        sim_assert_eq!(
            &component_configs,
            &[
                (
                    "major".to_string(),
                    VersionComponentSpec {
                        values: vec![],
                        optional_value: None,
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
                (
                    "minor".to_string(),
                    VersionComponentSpec {
                        values: vec![],
                        optional_value: None,
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
                (
                    "patch".to_string(),
                    VersionComponentSpec {
                        values: vec![],
                        optional_value: None,
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
                (
                    "dev".to_string(),
                    VersionComponentSpec {
                        values: vec!["release".to_string(), "post".to_string()],
                        optional_value: None,
                        independent: Some(false),
                        ..VersionComponentSpec::default()
                    }
                ),
            ]
            .into_iter()
            .collect::<IndexMap<_, _>>(),
        );

        let file_map =
            crate::files::resolve_files_from_config(&mut config, &component_configs, None)?;
        let include_bumps = vec![
            "major".to_string(),
            "minor".to_string(),
            "patch".to_string(),
            "dev".to_string(),
        ];

        sim_assert_eq!(
            file_map.into_iter().collect::<Vec<_>>(),
            vec![
                (
                    PathBuf::from("bumpversion/__init__.py"),
                    vec![FileChange {
                        parse_version_pattern: parse_regex.clone(),
                        serialize_version_patterns: serialize.clone(),
                        search: RegexTemplate::Escaped(
                            [Value::Argument("current_version".to_string()),]
                                .into_iter()
                                .collect()
                        ),
                        replace: "{new_version}".to_string(),
                        ignore_missing_version: false,
                        ignore_missing_file: false,
                        include_bumps: Some(include_bumps.clone()),
                        exclude_bumps: None,
                    }]
                ),
                (
                    PathBuf::from("CHANGELOG.md"),
                    vec![
                        FileChange {
                            parse_version_pattern: parse_regex.clone(),
                            serialize_version_patterns: serialize.clone(),
                            search: RegexTemplate::Escaped(
                                [Value::String("Unreleased".to_string()),]
                                    .into_iter()
                                    .collect()
                            ),
                            replace: "{new_version}".to_string(),
                            ignore_missing_version: false,
                            ignore_missing_file: false,
                            include_bumps: Some(include_bumps.clone()),
                            exclude_bumps: None,
                        },
                        FileChange {
                            parse_version_pattern: parse_regex.clone(),
                            serialize_version_patterns: serialize.clone(),
                            search: RegexTemplate::Escaped(
                                [
                                    Value::Argument("current_version".to_string()),
                                    Value::String("...HEAD".to_string())
                                ]
                                .into_iter()
                                .collect()
                            ),
                            replace: "{current_version}...{new_version}".to_string(),
                            ignore_missing_version: false,
                            ignore_missing_file: false,
                            include_bumps: Some(include_bumps.clone()),
                            exclude_bumps: None,
                        },
                    ],
                ),
                (
                    PathBuf::from("action.yml"),
                    vec![FileChange {
                        parse_version_pattern: parse_regex.clone(),
                        serialize_version_patterns: serialize.clone(),
                        search: RegexTemplate::Escaped(
                            [
                                Value::String("bump-my-version==".to_string()),
                                Value::Argument("current_version".to_string())
                            ]
                            .into_iter()
                            .collect()
                        ),
                        replace: "bump-my-version=={new_version}".to_string(),
                        ignore_missing_version: false,
                        ignore_missing_file: false,
                        include_bumps: Some(include_bumps.clone()),
                        exclude_bumps: None,
                    },],
                ),
                (
                    PathBuf::from("Dockerfile"),
                    vec![
                        FileChange {
                            parse_version_pattern: parse_regex.clone(),
                            serialize_version_patterns: serialize.clone(),
                            search: RegexTemplate::Regex(
                                [Value::String(
                                    r"created=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z".to_string()
                                ),]
                                .into_iter()
                                .collect()
                            ),
                            replace: r"created={utcnow:%Y-%m-%dT%H:%M:%SZ}".to_string(),
                            ignore_missing_version: false,
                            ignore_missing_file: false,
                            include_bumps: Some(include_bumps.clone()),
                            exclude_bumps: None,
                        },
                        FileChange {
                            parse_version_pattern: parse_regex.clone(),
                            serialize_version_patterns: serialize.clone(),
                            search: RegexTemplate::Escaped(
                                [Value::Argument("current_version".to_string()),]
                                    .into_iter()
                                    .collect()
                            ),
                            replace: "{new_version}".to_string(),
                            ignore_missing_version: false,
                            ignore_missing_file: false,
                            include_bumps: Some(include_bumps.clone()),
                            exclude_bumps: None,
                        },
                    ]
                ),
            ]
        );

        Ok(())
    }
}
