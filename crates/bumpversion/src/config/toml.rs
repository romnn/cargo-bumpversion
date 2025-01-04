#[cfg(test)]
mod tests {
    use crate::{
        config::{
            pyproject_toml::tests::parse_toml, Config, FileConfig, GlobalConfig, InputFile,
            RegexTemplate, VersionComponentSpec,
        },
        diagnostics::{Printer, ToDiagnostics},
        f_string::{PythonFormatString, Value},
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
        let bumpversion_toml = indoc::indoc! {r"
            [bumpversion]
            current_version = 0.1.8
            commit = True
            tag = True
            message = DO NOT BUMP VERSIONS WITH THIS FILE

            [bumpversion:glob:*.txt]
            [bumpversion:glob:**/*.txt]

            [bdist_wheel]
            universal = 1
        "};

        let printer = Printer::default();
        let (config, file_id, diagnostics) = parse_toml(bumpversion_toml, &printer);
        let err = config.unwrap_err();
        similar_asserts::assert_eq!(&err.to_string(), "expected newline, found a period");
        similar_asserts::assert_eq!(printer.lines(&diagnostics[0]).ok(), Some(vec![1]));
        Ok(())
    }

    #[test]
    fn test_bumpversion_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "0.1.8"
            commit = true
            tag = true
            message = "DO NOT BUMP VERSIONS WITH THIS FILE"

            # NOTE: also sections with colons are not allowed

            [bdist_wheel]
            universal = 1
        "#};

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("0.1.8".to_string()),
                commit: Some(true),
                tag: Some(true),
                // commit_message: Some("DO NOT BUMP VERSIONS WITH THIS FILE".to_string()),
                commit_message: Some(PythonFormatString(vec![Value::String(
                    "DO NOT BUMP VERSIONS WITH THIS FILE".to_string(),
                )])),
                ..GlobalConfig::empty()
            },
            files: vec![],
            components: [].into_iter().collect(),
        };
        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;
        similar_asserts::assert_eq!(config, Some(expected));

        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/basic_cfg.toml>
    #[test]
    fn parse_compat_basic_cfg_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
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

        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;

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

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/file_config_overrides.toml>
    #[test]
    fn parse_compat_file_config_overrides() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "0.0.1"
            ignore_missing_version = true
            regex = true

            [[tool.bumpversion.files]]
            filename = "should_contain_defaults.txt"

            [[tool.bumpversion.files]]
            filename = "should_override_search.txt"
            search = "**unreleased**"

            [[tool.bumpversion.files]]
            filename = "should_override_replace.txt"
            replace = "**unreleased**"

            [[tool.bumpversion.files]]
            filename = "should_override_parse.txt"
            parse = "version(?P<major>\\d+)"

            [[tool.bumpversion.files]]
            filename = "should_override_serialize.txt"
            serialize = ["{major}"]

            [[tool.bumpversion.files]]
            filename = "should_override_ignore_missing.txt"
            ignore_missing_version = false

            [[tool.bumpversion.files]]
            filename = "should_override_regex.txt"
            search = "not a regex"
            regex = false
        "#};

        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;

        let expected = Config {
            global: GlobalConfig {
                ignore_missing_version: Some(true),
                // regex: Some(true),
                current_version: Some("0.0.1".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![
                (
                    InputFile::Path("should_contain_defaults.txt".into()),
                    FileConfig::empty(),
                ),
                (
                    InputFile::Path("should_override_search.txt".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Regex(
                            [Value::String("**unreleased**".to_string())]
                                .into_iter()
                                .collect(),
                        )),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("should_override_replace.txt".into()),
                    FileConfig {
                        replace: Some("**unreleased**".to_string()),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("should_override_parse.txt".into()),
                    FileConfig {
                        parse_version_pattern: Some(
                            regex::Regex::new("version(?P<major>\\d+)")?.into(),
                        ),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("should_override_serialize.txt".into()),
                    FileConfig {
                        serialize_version_patterns: Some(vec!["{major}".to_string()]),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("should_override_ignore_missing.txt".into()),
                    FileConfig {
                        ignore_missing_version: Some(false),
                        ..FileConfig::empty()
                    },
                ),
                (
                    InputFile::Path("should_override_regex.txt".into()),
                    FileConfig {
                        // regex: Some(false),
                        search: Some(RegexTemplate::Escaped(
                            vec![Value::String("not a regex".to_string())]
                                .into_iter()
                                .collect(),
                        )),
                        ..FileConfig::empty()
                    },
                ),
            ],
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/partial_version_strings.toml>
    #[test]
    fn parse_compat_partial_version_strings_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [project]
            name = "sample-repo"
            version = "0.0.2"
            description = ""
            authors = [
                {name = "Someone", email = "someone@example.com"},
            ]
            dependencies = []
            requires-python = ">=3.11"
            readme = "README.md"
            license = {text = "MIT"}

            [build-system]
            requires = ["setuptools>=61", "wheel"]
            build-backend = "setuptools.build_meta"

            [tool.pdm.dev-dependencies]
            lint = [
                "ruff==0.0.292", # Comments should be saved
            ]
            build = [
                "bump-my-version>=0.12.0",
            ]

            [tool.bumpversion]
            commit = false
            tag = false
            current_version = "0.0.2"

            [tool.othertool]
            bake_cookies = true
            ignore-words-list = "sugar, salt, flour"
        "#};

        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;

        let expected = Config {
            global: GlobalConfig {
                commit: Some(false),
                tag: Some(false),
                current_version: Some("0.0.2".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![].into_iter().collect(),
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/pep440.toml>
    #[test]
    fn parse_compat_pep440_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            allow_dirty = false
            commit = false
            message = "Bump version: {current_version} → {new_version}"
            commit_args = ""
            tag = false
            sign_tags = false
            tag_name = "v{new_version}"
            tag_message = "Bump version: {current_version} → {new_version}"
            current_version = "1.0.0"
            parse = """(?x)
            (?:
                (?P<major>[0-9]+)
                (?:
                    \\.(?P<minor>[0-9]+)
                    (?:
                        \\.(?P<patch>[0-9]+)
                    )?
                )?
                (?P<prerelease>
                    [-_\\.]?
                    (?P<pre_label>a|b|rc)
                    [-_\\.]?
                    (?P<pre_n>[0-9]+)?
                )?
                (?P<postrelease>
                    (?:
                        [-_\\.]?
                        (?P<post_label>post|rev|r)
                        [-_\\.]?
                        (?P<post_n>[0-9]+)?
                    )
                )?
                (?P<dev>
                    [-_\\.]?
                    (?P<dev_label>dev)
                    [-_\\.]?
                    (?P<dev_n>[0-9]+)?
                )?
            )
            (?:\\+(?P<local>[a-z0-9]+(?:[-_\\.][a-z0-9]+)*))?
            """
            serialize = [
                "{major}.{minor}.{patch}.{dev_label}{distance_to_latest_tag}+{short_branch_name}",
            #    "{major}.{minor}.{patch}{pre_label}{pre_n}",
            #    "{major}.{minor}.{patch}+{branch_name}",
                "{major}.{minor}.{patch}",
            ]
            search = "{current_version}"
            replace = "{new_version}"

            [tool.bumpversion.parts.pre_label]
            values = ["final", "a", "b", "rc"]

            [tool.bumpversion.parts.pre_n]
            first_value = 1

            [tool.bumpversion.parts.post_label]
            values = ["final", "post"]

            [tool.bumpversion.parts.post_n]
            first_value = 1


            [tool.bumpversion.parts.dev_label]
            values = ["final", "dev"]
            independent = true

            [tool.bumpversion.parts.dev_n]
            first_value = 1

            [tool.bumpversion.parts.local]
            independent = true
        "#};

        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;

        let expected = Config {
            global: GlobalConfig {
                allow_dirty: Some(false),
                commit: Some(false),
                commit_message: Some(PythonFormatString(vec![
                    Value::String("Bump version: ".to_string()),
                    Value::Argument("current_version".to_string()),
                    Value::String(" → ".to_string()),
                    Value::Argument("new_version".to_string()),
                ])),
                commit_args: Some(String::new()),
                tag: Some(false),
                sign_tags: Some(false),
                tag_name: Some(PythonFormatString(vec![
                    Value::String("v".to_string()),
                    Value::Argument("new_version".to_string()),
                ])),
                tag_message: Some(PythonFormatString(vec![
                    Value::String("Bump version: ".to_string()),
                    Value::Argument("current_version".to_string()),
                    Value::String(" → ".to_string()),
                    Value::Argument("new_version".to_string()),
                ])),
                current_version: Some("1.0.0".to_string()),
                parse_version_pattern: Some(regex::Regex::new(
                    indoc::indoc! {r"(?x)
                    (?:
                        (?P<major>[0-9]+)
                        (?:
                            \.(?P<minor>[0-9]+)
                            (?:
                                \.(?P<patch>[0-9]+)
                            )?
                        )?
                        (?P<prerelease>
                            [-_\.]?
                            (?P<pre_label>a|b|rc)
                            [-_\.]?
                            (?P<pre_n>[0-9]+)?
                        )?
                        (?P<postrelease>
                            (?:
                                [-_\.]?
                                (?P<post_label>post|rev|r)
                                [-_\.]?
                                (?P<post_n>[0-9]+)?
                            )
                        )?
                        (?P<dev>
                            [-_\.]?
                            (?P<dev_label>dev)
                            [-_\.]?
                            (?P<dev_n>[0-9]+)?
                        )?
                    )
                    (?:\+(?P<local>[a-z0-9]+(?:[-_\.][a-z0-9]+)*))?
                    ",
                    }
                )?.into()
                ),
                serialize_version_patterns: Some(vec![
                    "{major}.{minor}.{patch}.{dev_label}{distance_to_latest_tag}+{short_branch_name}".to_string(),
                    "{major}.{minor}.{patch}".to_string(),
                ]),
                search: Some(RegexTemplate::Escaped([
                    Value::Argument("current_version".to_string()),
                ].into_iter().collect())),
                replace: Some("{new_version}".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![].into_iter().collect(),
            components: [
                ("pre_label".to_string(), VersionComponentSpec{
            values: vec!["final".to_string(), "a".to_string(), "b".to_string(), "rc".to_string()],
                    ..VersionComponentSpec::default()
                }),
                ("pre_n".to_string(), VersionComponentSpec{
                    // first_value: Some(1),
                    ..VersionComponentSpec::default()
                }),
                ("post_label".to_string(), VersionComponentSpec{
                     values: vec!["final".to_string(), "post".to_string()],
                    ..VersionComponentSpec::default()
                }),
                ("post_n".to_string(), VersionComponentSpec{
                     // first_value: Some(1),
                    ..VersionComponentSpec::default()
                }),
                ("dev_label".to_string(), VersionComponentSpec{
                     // first_value: Some(1),
                     values: vec!["final".to_string(), "dev".to_string()],
                     independent: Some(true),
                    ..VersionComponentSpec::default()
                }),
                ("dev_n".to_string(), VersionComponentSpec{
                     // first_value: Some(1),
                    ..VersionComponentSpec::default()
                }),
                ("local".to_string(), VersionComponentSpec{
                    independent: Some(true),
                    ..VersionComponentSpec::default()
                }),
            ].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/regex_test_config.toml>
    #[test]
    fn parse_compat_regex_test_config_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "4.7.1"
            regex = true

            [[tool.bumpversion.files]]
            filename = "./citation.cff"
            search = "date-released: \\d{{4}}-\\d{{2}}-\\d{{2}}"
            replace = "date-released: {utcnow:%Y-%m-%d}"
        "#};

        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;

        let expected = Config {
            global: GlobalConfig {
                // regex: Some(true),
                current_version: Some("4.7.1".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![(
                InputFile::Path("./citation.cff".into()),
                FileConfig {
                    search: Some(RegexTemplate::Regex(
                        [Value::String(
                            r#"date-released: \d{4}-\d{2}-\d{2}"#.to_string(),
                        )]
                        .into_iter()
                        .collect(),
                    )),
                    replace: Some("date-released: {utcnow:%Y-%m-%d}".to_string()),
                    ..FileConfig::empty()
                },
            )],
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/regex_with_caret_config.toml>
    #[test]
    fn parse_compat_regex_with_caret_config_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = "1.0.0"
            regex = true

            [[tool.bumpversion.files]]
            filename = "thingy.yaml"
            search = "^version: {current_version}"
            replace = "version: {new_version}"
        "#};

        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;

        let expected = Config {
            global: GlobalConfig {
                // regex: Some(true),
                current_version: Some("1.0.0".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![(
                InputFile::Path("thingy.yaml".into()),
                FileConfig {
                    search: Some(RegexTemplate::Regex(
                        [
                            Value::String("^version: ".to_string()),
                            Value::Argument("current_version".to_string()),
                        ]
                        .into_iter()
                        .collect(),
                    )),
                    replace: Some("version: {new_version}".to_string()),
                    ..FileConfig::empty()
                },
            )],
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }

    /// Taken from <https://github.com/callowayproject/bump-my-version/blob/master/tests/fixtures/replace-date-config.toml>
    #[test]
    fn parse_compat_replace_date_config_toml() -> eyre::Result<()> {
        crate::tests::init();

        let bumpversion_toml = indoc::indoc! {r#"
            [tool.bumpversion]
            current_version = '1.2.3'

            [[tool.bumpversion.files]]
            filename = 'VERSION'
            search = "__date__ = '\\d{{4}}-\\d{{2}}-\\d{{2}}'"
            replace = "__date__ = '{now:%Y-%m-%d}'"
            regex = true

            [[tool.bumpversion.files]]
            filename = 'VERSION'
        "#};

        let config = parse_toml(bumpversion_toml, &Printer::default()).0?;

        let expected = Config {
            global: GlobalConfig {
                current_version: Some("1.2.3".to_string()),
                ..GlobalConfig::empty()
            },
            files: vec![
                (
                    InputFile::Path("VERSION".into()),
                    FileConfig {
                        search: Some(RegexTemplate::Regex(
                            [Value::String(r"__date__ = '\d{4}-\d{2}-\d{2}'".to_string())]
                                .into_iter()
                                .collect(),
                        )),
                        replace: Some("__date__ = '{now:%Y-%m-%d}'".to_string()),
                        // regex: Some(true),
                        ..FileConfig::empty()
                    },
                ),
                (InputFile::Path("VERSION".into()), FileConfig::empty()),
            ],
            components: [].into_iter().collect(),
        };
        similar_asserts::assert_eq!(config, Some(expected));
        Ok(())
    }
}
