use super::{Config, FileConfig, PartConfig};
use crate::diagnostics::{DiagnosticExt, FileId, Span, Spanned};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use color_eyre::eyre;
use indexmap::IndexMap;
use std::path::PathBuf;
use toml_span as toml;

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
        found: ValueKind,
        span: Span,
    },
    // #[error("{source}")]
    // Serde {
    //     #[source]
    //     source: serde_json::Error,
    //     span: Span,
    // },
    #[error("{source}")]
    Toml {
        #[source]
        source: toml_span::Error,
    },
}

mod diagnostics {
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
                        .with_labels(vec![Label::primary(file_id, span.clone())
                            .with_message(format!("expected {expected}"))])
                        .with_notes(vec![unindent::unindent(&format!(
                            "
                        expected type {expected}
                           found type `{found:?}`
                        "
                        ))]);
                    vec![diagnostic]
                }
                // Self::Serde { source, span } => vec![Diagnostic::error()
                //     .with_message(self.to_string())
                //     .with_labels(vec![
                //         Label::primary(file_id, span.clone()).with_message(source.to_string())
                //     ])],
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
pub fn as_string_array<'de>(value: &'de toml::Value<'de>) -> Result<Vec<String>, Error> {
    Ok(as_str_array(value)?
        .into_iter()
        .map(ToString::to_string)
        .collect())
}

#[inline]
pub fn as_str_array<'de>(value: &'de toml::Value<'de>) -> Result<Vec<&'de str>, Error> {
    match value.as_ref() {
        toml::value::ValueInner::String(value) => Ok(vec![&*value]),
        toml::value::ValueInner::Array(array) => {
            array.iter().map(as_str).collect::<Result<Vec<_>, _>>()
        }
        other => Err(Error::UnexpectedType {
            message: "expected a string or an array of strings".to_string(),
            expected: vec![ValueKind::Array, ValueKind::String],
            found: value.into(),
            span: value.span.into(),
        }),
    }
}

#[inline]
pub fn as_string<'de>(value: &'de toml::Value<'de>) -> Result<String, Error> {
    as_str(value).map(ToString::to_string)
}

#[inline]
pub fn as_str<'de>(value: &'de toml::Value<'de>) -> Result<&'de str, Error> {
    value.as_str().ok_or_else(|| Error::UnexpectedType {
        message: "expected a string".to_string(),
        expected: vec![ValueKind::String],
        found: value.into(),
        span: value.span.into(),
    })
}

#[inline]
pub fn as_bool<'de>(value: &'de toml::Value<'de>) -> Result<bool, Error> {
    value.as_bool().ok_or_else(|| Error::UnexpectedType {
        message: "expected a boolean".to_string(),
        expected: vec![ValueKind::String],
        found: value.into(),
        span: value.span.into(),
    })
}

pub(crate) fn parse_file<'de>(
    value: &'de toml::Value<'de>,
) -> Result<(PathBuf, FileConfig), Error> {
    let table = value.as_table().ok_or_else(|| Error::UnexpectedType {
        message: "file config must be a table".to_string(),
        expected: vec![ValueKind::Table],
        found: value.into(),
        span: value.span.into(),
    })?;
    let file_name = table
        .get("filename")
        .map(as_string)
        .transpose()?
        .ok_or_else(|| Error::MissingKey {
            key: "filename".to_string(),
            message: "file config is missing required key `filename`".to_string(),
            span: value.span.into(),
        })?;
    let file_config = parse_file_config(table)?;
    Ok((file_name.into(), file_config))
}

pub(crate) fn parse_part_config<'de>(
    value: &'de toml::value::Value<'de>,
) -> Result<PartConfig, Error> {
    let table = value.as_table().ok_or_else(|| Error::UnexpectedType {
        message: "part config must be a table".to_string(),
        expected: vec![ValueKind::Table],
        found: value.into(),
        span: value.span.into(),
    })?;
    let optional_value = table.get("optional_value").map(as_string).transpose()?;
    let values = table
        .get("values")
        .map(as_string_array)
        .transpose()?
        .unwrap_or_default();

    Ok(PartConfig {
        optional_value,
        values,
    })
}

pub(crate) fn parse_file_config<'de>(
    table: &'de toml::value::Table<'de>,
) -> Result<FileConfig, Error> {
    let current_version = table.get("current_version").map(as_string).transpose()?;

    let allow_dirty = table.get("allow_dirty").map(as_bool).transpose()?;
    let parse = table.get("parse").map(as_string).transpose()?;
    let serialize = table
        .get("serialize")
        .map(as_string_array)
        .transpose()?
        .unwrap_or_default();
    let search = table.get("search").map(as_string).transpose()?;
    let replace = table.get("replace").map(as_string).transpose()?;
    let regex = table.get("regex").map(as_bool).transpose()?;
    let no_configured_files = table.get("no_configured_files").map(as_bool).transpose()?;
    let ignore_missing_files = table.get("ignore_missing_files").map(as_bool).transpose()?;
    let ignore_missing_version = table
        .get("ignore_missing_version")
        .map(as_bool)
        .transpose()?;
    let dry_run = table.get("dry_run").map(as_bool).transpose()?;
    let commit = table.get("commit").map(as_bool).transpose()?;
    let tag = table.get("tag").map(as_bool).transpose()?;
    let sign_tag = table.get("sign_tag").map(as_bool).transpose()?;
    let tag_name = table.get("tag_name").map(as_string).transpose()?;
    let tag_message = table.get("tag_message").map(as_string).transpose()?;
    let commit_message = table
        .get("commit_message")
        .or(table.get("message"))
        .map(as_string)
        .transpose()?;
    let commit_args = table.get("commit_args").map(as_string).transpose()?;

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
    pub fn from_pyproject_value(
        config: toml::Value,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let Some((key, config)) = config
            .as_table()
            .and_then(|table| table.get("tool"))
            .and_then(|tool| tool.as_table())
            .and_then(|tool| tool.get_key_value("bumpversion"))
        else {
            return Ok(None);
        };

        let table = config.as_table().ok_or_else(|| Error::UnexpectedType {
            message: "bumpversion config must be a table".to_string(),
            expected: vec![ValueKind::Table],
            found: config.into(),
            span: config.span.into(),
        })?;

        if table.is_empty() {
            return Ok(None);
        }

        let global_file_config = parse_file_config(&table)?;

        let files = match table.get("files") {
            None => vec![],
            Some(value) => match value.as_ref() {
                toml::value::ValueInner::Array(array) => array
                    .iter()
                    .map(|value| parse_file(value))
                    .collect::<Result<Vec<(PathBuf, FileConfig)>, _>>()?,
                other => {
                    return Err(Error::UnexpectedType {
                        message: "files must be an array must be a table".to_string(),
                        expected: vec![ValueKind::Table],
                        found: value.into(),
                        span: value.span.into(),
                    })
                }
            },
        };

        let parts = match table.get("parts") {
            None => IndexMap::new(),
            Some(value) => match value.as_ref() {
                toml::value::ValueInner::Table(table) => table
                    .iter()
                    .map(|(key, value)| {
                        let part_config = parse_part_config(value)?;
                        Ok((key.name.to_string(), part_config))
                    })
                    .collect::<Result<Vec<(String, PartConfig)>, _>>()?
                    .into_iter()
                    .collect(),
                other => {
                    return Err(Error::UnexpectedType {
                        message: "parts must be a table".to_string(),
                        expected: vec![ValueKind::Table],
                        found: value.into(),
                        span: value.span.into(),
                    })
                }
            },
        };

        Ok(Some(Self {
            global: global_file_config,
            files,
            parts,
        }))
    }

    pub fn from_pyproject_toml(
        config: &str,
        file_id: FileId,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> Result<Option<Self>, Error> {
        let config = toml_span::parse(&config).map_err(|source| Error::Toml { source })?;
        Self::from_pyproject_value(config, file_id, strict, diagnostics)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        config::{Config, FileConfig, PartConfig},
        diagnostics::{Printer, ToDiagnostics},
    };
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use std::io::Read;
    use std::path::PathBuf;

    pub fn parse_toml(
        config: &str,
        printer: &Printer,
    ) -> (Result<Option<Config>, super::Error>, usize) {
        let mut diagnostics = vec![];
        let file_id = printer.add_source_file("bumpversion.toml".to_string(), config.to_string());
        let strict = true;
        let config = Config::from_pyproject_toml(config, file_id, strict, &mut diagnostics)
            .map_err(|err| {
                for diagnostic in err.to_diagnostics(file_id) {
                    printer.emit(&diagnostic);
                }
                err
            });
        if let Err(ref err) = config {
            for diagnostic in err.to_diagnostics(file_id) {
                printer.emit(&diagnostic);
            }
        }
        (config, file_id)
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

        let config = parse_toml(&pyproject_toml, &Printer::default()).0?;
        println!("config: {:#?}", config);

        let expected = Config {
            global: FileConfig {
                current_version: Some("1.2.3".to_string()),
                ..FileConfig::default()
            },
            files: [(
                PathBuf::from("config.ini"),
                FileConfig {
                    search: Some(
                        indoc::indoc! {r#"
                        [myproject]
                        version={current_version}"#}
                        .to_string(),
                    ),
                    replace: Some(
                        indoc::indoc! {r#"
                        [myproject]
                        version={new_version}"#}
                        .to_string(),
                    ),
                    ..FileConfig::default()
                },
            )]
            .into_iter()
            .collect(),
            parts: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
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

        let config = parse_toml(&pyproject_toml, &Printer::default()).0?;
        println!("config: {:#?}", config);

        let expected = Config {
            global: FileConfig {
                current_version: Some("0.28.1".to_string()),
                commit: Some(true),
                commit_args: Some("--no-verify".to_string()),
                tag: Some(true),
                tag_name: Some("{new_version}".to_string()),
                allow_dirty: Some(true),
                parse: Some("(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\.(?P<dev>post)\\d+\\.dev\\d+)?".to_string()),
                serialize: vec![
                    "{major}.{minor}.{patch}.{dev}{$PR_NUMBER}.dev{distance_to_latest_tag}".to_string(),
                    "{major}.{minor}.{patch}".to_string(),
                ],
                commit_message: Some("Version updated from {current_version} to {new_version}".to_string()),
                ..FileConfig::default()
            },
            files: [
                (
                    PathBuf::from("bumpversion/__init__.py"),
                    FileConfig::default()
                ),
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
                (
                    PathBuf::from("action.yml"),
                    FileConfig {
                        search: Some("bump-my-version=={current_version}".to_string()),
                        replace: Some("bump-my-version=={new_version}".to_string()),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("Dockerfile"),
                    FileConfig {
                        search: Some("created=\\d{{4}}-\\d{{2}}-\\d{{2}}T\\d{{2}}:\\d{{2}}:\\d{{2}}Z".to_string()),
                        replace: Some("created={utcnow:%Y-%m-%dT%H:%M:%SZ}".to_string()),
                        regex: Some(true),
                        ..FileConfig::default()
                    },
                ),
                (
                    PathBuf::from("Dockerfile"),
                    FileConfig::default(),
                ),
            ]
            .into_iter()
            .collect(),
            parts: [
                (
                    "dev".to_string(), 
                    PartConfig{
                        values: vec!["release".to_string(), "post".to_string()],
                        ..PartConfig::default()
                    }
                )
            ].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
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
            global: FileConfig {
                current_version: Some("0.10.5".to_string()),
                parse: Some(
                    r#"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\-(?P<release>[a-z]+))?"#
                        .to_string(),
                ),
                serialize: vec![
                    "{major}.{minor}.{patch}-{release}".to_string(),
                    "{major}.{minor}.{patch}".to_string(),
                ],
                ..FileConfig::default()
            },
            ..Config::default()
        };

        let config = parse_toml(&pyproject_toml, &Printer::default()).0?;
        similar_asserts::assert_eq!(config, Some(expected));

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
            global: FileConfig {
                current_version: Some("1.0.0".to_string()),
                commit: Some(true),
                tag: Some(true),
                // parse = "(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\-(?P<release>[a-z]+))?"
                parse: Some(
                    r#"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(\-(?P<release>[a-z]+))?"#
                        .to_string(),
                ),
                serialize: vec![
                    "{major}.{minor}.{patch}-{release}".to_string(),
                    "{major}.{minor}.{patch}".to_string(),
                ],
                ..FileConfig::default()
            },
            parts: [(
                "release".to_string(),
                PartConfig {
                    optional_value: Some("gamma".to_string()),
                    values: vec!["dev".to_string(), "gamma".to_string()],
                },
            )]
            .into_iter()
            .collect(),
            files: vec![
                (PathBuf::from("setup.py"), FileConfig::default()),
                (
                    PathBuf::from("bumpversion/__init__.py"),
                    FileConfig::default(),
                ),
                (
                    PathBuf::from("CHANGELOG.md"),
                    FileConfig {
                        search: Some(r#"**unreleased**"#.to_string()),
                        replace: Some(
                            indoc::indoc! {
                            r#"
                            **unreleased**
                            **v{new_version}**"#}
                            .to_string(),
                        ),
                        ..FileConfig::default()
                    },
                ),
            ]
            .into_iter()
            .collect(),
            ..Config::default()
        };

        let config = parse_toml(&pyproject_toml, &Printer::default()).0?;
        similar_asserts::assert_eq!(config, Some(expected));

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

        let config = parse_toml(&pyproject_toml, &Printer::default()).0?;
        similar_asserts::assert_eq!(config, None);
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

        let config = parse_toml(&pyproject_toml, &Printer::default()).0?;
        similar_asserts::assert_eq!(config, None);
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

        let config = parse_toml(&pyproject_toml, &Printer::default()).0?;

        let expected = Config {
            global: FileConfig {
                current_version: Some("1.0.0".to_string()),
                parse: Some(
                    indoc::indoc! {r#"
                        (?x)
                            (?P<major>[0-9]+)
                            \.(?P<minor>[0-9]+)
                            \.(?P<patch>[0-9]+)
                            (?:
                                -(?P<pre_label>alpha|beta|stable)
                                (?:-(?P<pre_n>[0-9]+))?
                            )?
                    "#}
                    .to_string(),
                ),
                serialize: vec![
                    "{major}.{minor}.{patch}-{pre_label}-{pre_n}".to_string(),
                    "{major}.{minor}.{patch}".to_string(),
                ],

                ..FileConfig::default()
            },
            files: [].into_iter().collect(),
            parts: [(
                "pre_label".to_string(),
                PartConfig {
                    optional_value: Some("stable".to_string()),
                    values: vec![
                        "alpha".to_string(),
                        "beta".to_string(),
                        "stable".to_string(),
                    ],
                },
            )]
            .into_iter()
            .collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }
}
