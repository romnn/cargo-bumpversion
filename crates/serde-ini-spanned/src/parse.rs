use crate::spanned::{Span, Spanned};
use aho_corasick::AhoCorasick;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use std::collections::HashMap;

pub const DEFAULT_ASSIGNMENT_DELIMITERS: [&str; 2] = ["=", ":"];
pub const DEFAULT_COMMENT_PREFIXES: [&str; 2] = [";", "#"];
pub const DEFAULT_INLINE_COMMENT_PREFIXES: [&str; 0] = [];

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Item {
    Section {
        name: String,
    },
    ContinuationValue {
        value: String,
    },
    Value {
        key: Spanned<String>,
        value: Spanned<String>,
    },
    Comment {
        text: String,
    },
}

#[derive(thiserror::Error, Debug, Clone, Eq, PartialEq)]
pub enum SyntaxError {
    SectionNotClosed {
        span: Span,
    },
    InvalidSectionName {
        span: Span,
    },
    EmptyOptionName {
        span: Span,
    },
    MissingAssignmentDelimiter {
        span: Span,
        assignment_delimiters: Vec<String>,
    },
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SectionNotClosed { .. } => write!(f, r"section was not closed: missing ']'"),
            Self::InvalidSectionName { .. } => write!(f, r"invalid section name: contains ']'"),
            Self::EmptyOptionName { .. } => write!(f, r"empty option name"),
            Self::MissingAssignmentDelimiter {
                assignment_delimiters,
                ..
            } => write!(
                f,
                r"variable assignment missing one of: {}",
                assignment_delimiters
                    .iter()
                    .map(|d| format!("`{d}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl SyntaxError {
    pub fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Diagnostic<F> {
        match self {
            Self::SectionNotClosed { span } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![
                    Label::primary(file_id, span.clone()).with_message("missing `]`")
                ]),
            Self::InvalidSectionName { span } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![Label::primary(file_id, span.clone())
                    .with_message("section must not contain `]`")]),
            Self::EmptyOptionName { span } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![Label::primary(file_id, span.clone())
                    .with_message("option name must not be empty")]),
            Self::MissingAssignmentDelimiter {
                span,
                assignment_delimiters,
            } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![Label::primary(file_id, span.clone()).with_message(
                    format!(
                        "missing one of: {}",
                        assignment_delimiters
                            .iter()
                            .map(|d| format!("`{d}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                )]),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error(transparent)]
    Pattern(#[from] aho_corasick::BuildError),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("syntax error: {0}")]
    Syntax(#[from] SyntaxError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

impl Error {
    pub fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Vec<Diagnostic<F>> {
        match self {
            Self::Io(_) | Self::Config(_) => vec![],
            Self::Syntax(err) => vec![err.to_diagnostics(file_id)],
        }
    }
}

pub(crate) trait Parse {
    fn parse_next(&mut self, state: &mut ParseState) -> Result<Option<Vec<Spanned<Item>>>, Error>;
}

pub(crate) fn trim_trailing_whitespace(value: &mut String, span: &mut Span) {
    let count = value
        .chars()
        .rev()
        .take_while(|c| c.is_whitespace())
        .count();
    span.end -= count;
    *value = value.split_at(value.len() - count).0.to_string();
}

#[must_use]
pub fn compact_span(line: &str, span: Span) -> Span {
    let Span { mut start, mut end } = span;
    debug_assert!(start <= end);

    start += line[start..]
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();

    debug_assert!(start <= end);

    end -= line[start..end]
        .chars()
        .rev()
        .take_while(|c| c.is_whitespace())
        .count();

    debug_assert!(start <= end);
    Span { start, end }
}

fn to_byte_span(line: &str, span: Span) -> Span {
    let start = line
        .char_indices()
        .nth(span.start)
        .map_or(span.start, |(offset, _)| offset);
    let end = line
        .char_indices()
        .nth(span.end)
        .map_or(span.end, |(offset, _)| offset);
    Span { start, end }
}

trait AddOffset {
    fn add_offset(self, offset: usize) -> Self;
}

impl AddOffset for Span {
    fn add_offset(self, offset: usize) -> Self {
        Self {
            start: self.start + offset,
            end: self.end + offset,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ParseState {
    current_section: HashMap<String, Vec<String>>,
    option_name: Option<String>,
    indent_level: usize,
}

/// INI parser config.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Config {
    /// Assignment delimiters that denote assignment.
    ///
    /// # Example
    /// When using "=" as an assignment delimiter, "a = 3" is a valid assignment.
    /// ```rust
    /// use serde_ini_spanned::{from_str, Options, ParseConfig, DerefInner};
    ///
    /// let ini = indoc::indoc!{r#"
    ///     [my-section]
    ///     a = 3
    /// "#};
    /// let options = Options{
    ///     strict: true,
    ///     parser_config: ParseConfig::default().with_assignment_delimiters(["="]),
    /// };
    /// let mut diagnostics = vec![];
    /// let config = from_str(ini, options, 0, &mut diagnostics)?;
    /// assert_eq!(config.get("my-section", "a").deref_inner(), Some("3"));
    /// # Ok::<_, color_eyre::eyre::Report>(())
    /// ```
    pub assignment_delimiters: Vec<&'static str>,

    /// Comment prefixes that denote full-line comments.
    ///
    /// # Example
    /// When using "#" as a comment prefix, "# a = 3" is a comment.
    /// Note that "a = 3 # test" will parse as "3 # test",
    /// unless "#" is also set in `inline_comment_prefixes`.
    pub comment_prefixes: Vec<&'static str>,

    /// Comment prefixes that denote inline comments.
    ///
    /// # Example
    /// When using "#" as an inline comment prefix, "a = 3 # test" will parse as a=3.
    pub inline_comment_prefixes: Vec<&'static str>,

    /// Allow empty lines in values.
    ///
    /// ```ini
    /// value = this value is
    ///
    ///     actually still here
    ///
    /// next_value = test
    /// ```
    pub allow_empty_lines_in_values: bool,

    /// Allow brackets in section name.
    pub allow_brackets_in_section_name: bool,
}

impl Config {
    pub fn with_assignment_delimiters(
        mut self,
        delimiters: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        self.assignment_delimiters = delimiters.into_iter().collect();
        self
    }

    pub fn with_comment_prefixes(
        mut self,
        prefixes: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        self.comment_prefixes = prefixes.into_iter().collect();
        self
    }

    pub fn with_inline_comment_prefixes(
        mut self,
        prefixes: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        self.inline_comment_prefixes = prefixes.into_iter().collect();
        self
    }

    #[must_use] pub fn empty_lines_in_values(mut self, enabled: bool) -> Self {
        self.allow_empty_lines_in_values = enabled;
        self
    }

    #[must_use] pub fn brackets_in_section_names(mut self, enabled: bool) -> Self {
        self.allow_brackets_in_section_name = enabled;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            assignment_delimiters: DEFAULT_ASSIGNMENT_DELIMITERS.to_vec(),
            comment_prefixes: DEFAULT_COMMENT_PREFIXES.to_vec(),
            inline_comment_prefixes: DEFAULT_INLINE_COMMENT_PREFIXES.to_vec(),
            allow_empty_lines_in_values: true,
            allow_brackets_in_section_name: true,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Parser<B> {
    config: Config,
    assignment_delimiters: AhoCorasick,
    comment_prefixes: AhoCorasick,
    inline_comment_prefixes: AhoCorasick,
    lines: crate::lines::Lines<B>,
}

impl<B> Parser<B> {
    pub fn new(buf: B, config: Config) -> Result<Self, ConfigError> {
        let assignment_delimiters = AhoCorasick::new(&config.assignment_delimiters)?;
        let comment_prefixes = AhoCorasick::new(&config.comment_prefixes)?;
        let inline_comment_prefixes = AhoCorasick::new(&config.inline_comment_prefixes)?;
        Ok(Self {
            assignment_delimiters,
            comment_prefixes,
            inline_comment_prefixes,
            lines: crate::lines::Lines::new(buf),
            config,
        })
    }
}

impl<B> Parse for Parser<B>
where
    B: std::io::BufRead,
{
    fn parse_next(&mut self, state: &mut ParseState) -> Result<Option<Vec<Spanned<Item>>>, Error> {
        let line = self.lines.next().transpose()?;
        let Some((offset, line)) = line else {
            return Ok(None);
        };
        let mut span = compact_span(&line, 0..line.len());
        let current_indent_level = span.start;

        dbg!(&line);

        let mut items: Vec<Spanned<Item>> = vec![];

        let comment_pos = self
            .comment_prefixes
            .find(&line[span.clone()])
            .map(|pos| pos.start());

        if line[span.clone()].starts_with('[') {
            if line[span.clone()].ends_with(']') {
                span.start += 1;
                span.end -= 1;
                let byte_span = to_byte_span(&line, span.clone()).add_offset(offset);
                if !self.config.allow_brackets_in_section_name && line[span.clone()].contains(']') {
                    return Err(Error::Syntax(SyntaxError::InvalidSectionName {
                        span: byte_span,
                    }));
                } else {
                    state.current_section.clear();
                    state.option_name = None;
                    println!("\t=> section: {}", &line[span.clone()]);

                    items.push(Spanned::new(
                        byte_span,
                        Item::Section {
                            name: line[span].into(),
                        },
                    ));
                }
            } else {
                let byte_span = to_byte_span(&line, span.clone()).add_offset(offset);
                return Err(Error::Syntax(SyntaxError::SectionNotClosed {
                    span: byte_span,
                }));
            }
        } else if let Some(0) = comment_pos {
            // comment
            span.start += 1;
            let byte_span = to_byte_span(&line, span.clone()).add_offset(offset);
            println!("\t=> comment: {line}");
            items.push(Spanned::new(
                byte_span,
                Item::Comment {
                    text: line[span].into(),
                },
            ));
        } else {
            // find position of assignment delimiter (e.g. '=')
            let assignment_delimiter_pos = self
                .assignment_delimiters
                .find(&line[span.clone()])
                .map(|pos| pos.start());

            // find position of inline comment
            let inline_comment_pos = self
                .inline_comment_prefixes
                .find(&line[span.clone()])
                .map(|pos| pos.start());

            if let Some(comment_pos) = inline_comment_pos {
                let comment_span = Span {
                    start: span.start + comment_pos + 1,
                    end: span.end,
                };
                let comment_span = compact_span(&line, comment_span);

                let value = &line[comment_span.clone()];
                println!("\t=> comment: {value}");
                let byte_span = to_byte_span(&line, comment_span).add_offset(offset);

                span.end = span.start + comment_pos;
                items.push(Spanned::new(
                    byte_span,
                    Item::Comment { text: value.into() },
                ));
            }

            let is_empty = line.chars().all(char::is_whitespace);

            // check if continue
            let mut is_continue = false;
            if let Some(ref _option_name) = state.option_name {
                // if true {
                //     dbg!(
                //         !state.current_section.is_empty(),
                //         assignment_delimiter_pos,
                //         current_indent_level,
                //         state.indent_level
                //     );
                // }
                is_continue =
                    !state.current_section.is_empty() && current_indent_level > state.indent_level;

                println!("\t=> continuation?: {is_continue}");

                if is_continue {
                    let continuation_span = compact_span(&line, span.clone());
                    let value = &line[continuation_span.clone()];
                    println!("\t=> continuation: {value}");

                    items.push(Spanned::new(
                        to_byte_span(&line, continuation_span.clone()).add_offset(offset),
                        Item::ContinuationValue {
                            value: value.into(),
                        },
                    ));
                }
            }

            if is_empty {
                if self.config.allow_empty_lines_in_values {
                    println!("\t=> empty (continuation)");
                    items.push(Spanned::new(
                        to_byte_span(&line, span.clone()).add_offset(offset),
                        Item::ContinuationValue { value: line },
                    ));
                } else {
                    // reset current option
                    state.option_name = None;
                }
            } else if !is_continue {
                let assignment_delimiter_pos = assignment_delimiter_pos.ok_or_else(|| {
                    Error::Syntax(SyntaxError::MissingAssignmentDelimiter {
                        span: to_byte_span(&line, span.clone()).add_offset(offset),
                        assignment_delimiters: self
                            .config
                            .assignment_delimiters
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                    })
                })?;

                let key_span = Span {
                    start: span.start,
                    end: span.start + assignment_delimiter_pos,
                };
                let key_span = compact_span(&line, key_span);
                let key = &line[key_span.clone()];

                let value_span = Span {
                    start: span.start + assignment_delimiter_pos + 1,
                    end: span.end,
                };
                let value_span = compact_span(&line, value_span);
                let value = &line[value_span.clone()];

                if key.is_empty() {
                    return Err(Error::Syntax(SyntaxError::EmptyOptionName {
                        span: to_byte_span(&line, key_span.clone()).add_offset(offset),
                    }));
                }

                state.indent_level = key_span.start;

                println!("\t=> key={key} value={value}");
                state.option_name = Some(key.to_string());
                state
                    .current_section
                    .insert(key.to_string(), vec![value.to_string()]);

                items.push(Spanned::new(
                    to_byte_span(&line, span).add_offset(offset),
                    Item::Value {
                        key: Spanned::new(
                            to_byte_span(&line, key_span).add_offset(offset),
                            key.into(),
                        ),
                        value: Spanned::new(
                            to_byte_span(&line, value_span).add_offset(offset),
                            value.into(),
                        ),
                    },
                ));
            }
        }
        Ok(Some(items))
    }
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
mod tests {
    use crate::{
        parse::{DEFAULT_ASSIGNMENT_DELIMITERS, DEFAULT_COMMENT_PREFIXES},
        spanned::{DerefInner, Spanned},
        tests::{parse, Printer, SectionProxyExt},
        value::{ClearSpans, NoSectionError, Options, RawSection, Section, Value},
        ParseConfig,
    };
    use color_eyre::eyre;
    use similar_asserts::assert_eq as sim_assert_eq;
    use unindent::unindent;

    #[test]
    fn compact_span() {
        let line = " this is a  test ";
        let span = super::compact_span(line, 0..line.len());
        sim_assert_eq!(&line[span], "this is a  test");

        let line = "this is a  test";
        let span = super::compact_span(line, 0..line.len());
        sim_assert_eq!(&line[span], "this is a  test");

        let line = "    ";
        let span = super::compact_span(line, 0..line.len());
        sim_assert_eq!(&line[span], "");

        let line = " \n\r   ";
        let span = super::compact_span(line, 0..line.len());
        sim_assert_eq!(&line[span], "");

        let line = "";
        let span = super::compact_span(line, 0..line.len());
        sim_assert_eq!(&line[span], "");

        let line = " ####      ";
        let span = super::compact_span(line, 3..line.len());
        sim_assert_eq!(&line[span], "##");

        let line = "####      ";
        let span = super::compact_span(line, 4..line.len());
        sim_assert_eq!(&line[span], "");
    }

    #[test]
    fn parse_simple_ini() -> eyre::Result<()> {
        crate::tests::init();

        let config = indoc::indoc! {r"
            [DEFAULT]
            key1 = value1
            pizzatime = yes

            cost = 9

            [topsecrets]
            nuclear launch codes = topsecret

            [github.com]
            User = QEDK
        "};

        let have = parse(config, Options::default(), &Printer::default()).0?;
        let mut expected = Value::with_defaults([].into_iter().collect());

        expected.add_section(
            "DEFAULT".into(),
            [
                ("key1".into(), "value1".into()),
                ("pizzatime".into(), "yes".into()),
                ("cost".into(), "9".into()),
            ],
        );

        expected.add_section(
            "topsecrets".into(),
            [("nuclear launch codes".into(), "topsecret".into())],
        );

        expected.add_section("github.com".into(), [("user".into(), "QEDK".into())]);

        sim_assert_eq!(have.clone().cleared_spans(), expected, "values match");

        // check that spans match
        sim_assert_eq!(
            &config[have.section("DEFAULT").unwrap().span().clone()],
            "DEFAULT"
        );

        let v = have.section("DEFAULT").unwrap();
        sim_assert_eq!(&config[v.key_span("key1").unwrap().clone()], "key1");
        sim_assert_eq!(&config[v["key1"].span.clone()], "value1");
        sim_assert_eq!(
            &config[v.key_span("pizzatime").unwrap().clone()],
            "pizzatime"
        );
        sim_assert_eq!(&config[v["pizzatime"].span.clone()], "yes");
        sim_assert_eq!(&config[v.key_span("cost").unwrap().clone()], "cost");
        sim_assert_eq!(&config[v["cost"].span.clone()], "9");

        let v = have.section("topsecrets").unwrap();
        sim_assert_eq!(
            &config[v.key_span("nuclear launch codes").unwrap().clone()],
            "nuclear launch codes"
        );
        sim_assert_eq!(&config[v["nuclear launch codes"].span.clone()], "topsecret");

        let v = have.section("github.com").unwrap();
        sim_assert_eq!(&config[v.key_span("User").unwrap().clone()], "User");
        sim_assert_eq!(&config[v["User"].span.clone()], "QEDK");

        Ok(())
    }

    fn check_configparser_compat_basic(have: &mut Value) -> eyre::Result<()> {
        let expected_section_names = [
            "Foo Bar",
            "Spacey Bar",
            "Spacey Bar From The Beginning",
            "Commented Bar",
            "Long Line",
            r"Section\with$weird%characters[\t",
            "Internationalized Stuff",
            "Spaces",
            "Types",
            "This One Has A ] In It",
        ];
        sim_assert_eq!(
            have.section_names()
                .map(std::convert::AsRef::as_ref)
                .collect::<Vec<_>>(),
            expected_section_names
        );

        let spacey_bar_beginning_expected: RawSection = [
            (Spanned::from("foo"), Spanned::from("bar3")),
            (Spanned::from("baz"), Spanned::from("qwe")),
        ]
        .into_iter()
        .collect();

        dbg!(&have.section("Spacey Bar From The Beginning"));
        sim_assert_eq!(
            &have.section("Spacey Bar From The Beginning").unwrap(),
            // .map(|value| value.clone().cleared_spans())
            // .as_ref(),
            // Some(&spacey_bar_beginning_expected),
            &spacey_bar_beginning_expected,
        );

        // test index trait
        dbg!(&have.section("Spacey Bar From The Beginning"));

        sim_assert_eq!(
            &have.section("Spacey Bar From The Beginning").unwrap(),
            // .clone()
            // .cleared_spans(),
            &spacey_bar_beginning_expected,
        );

        // L = cf.items()
        // L = sorted(list(L))
        // self.assertEqual(len(L), len(E))
        // for name, section in L:
        //     eq(name, section.name)
        // eq(cf.defaults(), cf[self.default_section])

        // API access
        // use std::ops::Deref;
        // let test: &str = have.get("Foo Bar", "foo").unwrap().deref();
        // let test: Option<Spanned<String>> = have.get("Foo Bar", "foo").cloned();
        // let test: Option<&str> = have.get("Foo Bar", "foo").cloned().as_deref();
        // let test: Option<&str> = have.get("Foo Bar", "foo").deref_inner();

        sim_assert_eq!(have.get("Foo Bar", "foo").deref_inner(), Some("bar1"));
        sim_assert_eq!(have.get("Spacey Bar", "foo").deref_inner(), Some("bar2"));
        sim_assert_eq!(
            have.get("Spacey Bar From The Beginning", "foo")
                .deref_inner(),
            Some("bar3")
        );
        sim_assert_eq!(
            have.get("Spacey Bar From The Beginning", "baz")
                .deref_inner(),
            Some("qwe")
        );
        sim_assert_eq!(have.get("Commented Bar", "foo").deref_inner(), Some("bar4"));
        sim_assert_eq!(have.get("Commented Bar", "baz").deref_inner(), Some("qwe"));
        sim_assert_eq!(
            have.get("Spaces", "key with spaces").deref_inner(),
            Some("value")
        );
        sim_assert_eq!(
            have.get("Spaces", "another with spaces").deref_inner(),
            Some("splat!")
        );
        sim_assert_eq!(
            have.get_int("Types", "int")?.map(Spanned::into_inner),
            Some(42)
        );
        sim_assert_eq!(have.get("Types", "int").deref_inner(), Some("42"));
        sim_assert_eq!(
            have.get_float("Types", "float")?.map(Spanned::into_inner),
            Some(0.44)
        );
        sim_assert_eq!(have.get("Types", "float").deref_inner(), Some("0.44"));
        sim_assert_eq!(
            have.get_bool("Types", "boolean")?.map(Spanned::into_inner),
            Some(false)
        );
        sim_assert_eq!(
            have.get("Types", "123").deref_inner(),
            Some("strange but acceptable")
        );
        sim_assert_eq!(
            have.get("This One Has A ] In It", "forks").deref_inner(),
            Some("spoons")
        );

        // test vars= and fallback=
        // sim_assert_eq!(have.get("Foo Bar", "foo", fallback="baz"), "bar1");
        // sim_assert_eq!(have.get("Foo Bar", "foo", vars={'foo': 'baz'}), "baz");
        // with self.assertRaises(configparser.NoSectionError):
        sim_assert_eq!(have.get("No Such Foo Bar", "foo"), None);
        // with self.assertRaises(configparser.NoOptionError):
        // cf.get('Foo Bar', 'no-such-foo')
        sim_assert_eq!(have.get("Foo Var", "no-such-foo"), None);

        // sim_assert_eq(cf.get('No Such Foo Bar', 'foo', fallback='baz'), 'baz')
        // eq(cf.get('Foo Bar', 'no-such-foo', fallback='baz'), 'baz')
        // eq(cf.get('Spacey Bar', 'foo', fallback=None), 'bar2')
        // eq(cf.get('No Such Spacey Bar', 'foo', fallback=None), None)
        // eq(cf.getint('Types', 'int', fallback=18), 42)
        // eq(cf.getint('Types', 'no-such-int', fallback=18), 18)
        // eq(cf.getint('Types', 'no-such-int', fallback="18"), "18") # sic!
        // with self.assertRaises(configparser.NoOptionError):
        sim_assert_eq!(have.get_int("Types", "no-such-int")?, None);
        // self.assertAlmostEqual(cf.getfloat('Types', 'float',
        //                                    fallback=0.0), 0.44)
        // self.assertAlmostEqual(cf.getfloat('Types', 'no-such-float',
        //                                    fallback=0.0), 0.0)
        // eq(cf.getfloat('Types', 'no-such-float', fallback="0.0"), "0.0") # sic!
        // with self.assertRaises(configparser.NoOptionError):
        // cf.getfloat('Types', 'no-such-float')
        sim_assert_eq!(have.get_float("Types", "no-such-float")?, None);
        // eq(cf.getboolean('Types', 'boolean', fallback=True), False)
        // eq(cf.getboolean('Types', 'no-such-boolean', fallback="yes"), "yes") # sic!
        // eq(cf.getboolean('Types', 'no-such-boolean', fallback=True), True)
        // with self.assertRaises(configparser.NoOptionError):
        // cf.getboolean('Types', 'no-such-boolean')
        sim_assert_eq!(have.get_bool("Types", "no-such-boolean")?, None);
        // eq(cf.getboolean('No Such Types', 'boolean', fallback=True), True)

        // mapping access
        sim_assert_eq!(&*have.section("Foo Bar").unwrap()["foo"], "bar1");
        sim_assert_eq!(&*have.section("Spacey Bar").unwrap()["foo"], "bar2");

        let section = &have.section("Spacey Bar From The Beginning").unwrap();
        // sim_assert_eq!(section.name, 'Spacey Bar From The Beginning')
        // self.assertIs(section.parser, cf)
        // with self.assertRaises(AttributeError):
        //     section.name = 'Name is read-only'
        // with self.assertRaises(AttributeError):
        //     section.parser = 'Parser is read-only'
        sim_assert_eq!(&*section["foo"], "bar3");
        sim_assert_eq!(&*section["baz"], "qwe");
        sim_assert_eq!(
            have.section("Commented Bar")
                .unwrap()
                .get("foo")
                .unwrap()
                .as_ref(),
            "bar4"
        );
        sim_assert_eq!(
            have.section("Commented Bar")
                .unwrap()
                .get("baz")
                .unwrap()
                .as_ref(),
            "qwe"
        );
        sim_assert_eq!(
            have.section("Spaces")
                .unwrap()
                .get("key with spaces")
                .unwrap()
                .as_ref(),
            "value"
        );
        sim_assert_eq!(
            have.section("Spaces")
                .unwrap()
                .get("another with spaces")
                .unwrap()
                .as_ref(),
            "splat!"
        );
        sim_assert_eq!(
            &*have.section("Long Line").unwrap()["foo"],
            "this line is much, much longer than my editor\nlikes it."
        );
        // if self.allow_no_value:
        //     eq(cf['NoValue']['option-without-value'], None)

        // test vars= and fallback=
        sim_assert_eq!(
            have.section("Foo Bar").unwrap().get("foo").deref_inner(),
            Some("bar1")
        );
        // eq(cf['Foo Bar'].get('foo', fallback='baz'), 'bar1')
        // eq(cf['Foo Bar'].get('foo', vars={'foo': 'baz'}), 'baz')

        sim_assert_eq!(
            have.section("Foo Bar").unwrap().get("foo").deref_inner(),
            Some("bar1")
        );

        // with self.assertRaises(KeyError):
        //     cf['No Such Foo Bar']['foo']
        sim_assert_eq!(
            std::panic::catch_unwind(|| have
                .section("No Such Foo Bar")
                .unwrap()
                .get("foo")
                .unwrap()
                .clone())
            .is_err(),
            true,
        );
        // with self.assertRaises(KeyError):
        //     cf['Foo Bar']['no-such-foo']
        sim_assert_eq!(
            std::panic::catch_unwind(|| have
                .section("Foo Bar")
                .unwrap()
                .get("no-such-foo")
                .unwrap()
                .clone())
            .is_err(),
            true,
        );
        // with self.assertRaises(KeyError):
        //     cf['No Such Foo Bar'].get('foo', fallback='baz')
        sim_assert_eq!(
            std::panic::catch_unwind(|| have
                .section("No Such Foo Bar")
                .unwrap()
                .get("foo")
                .unwrap()
                .clone())
            .is_err(),
            true,
        );
        // eq(cf['Foo Bar'].get('no-such-foo', 'baz'), 'baz')
        // eq(cf['Foo Bar'].get('no-such-foo', fallback='baz'), 'baz')
        // eq(cf['Foo Bar'].get('no-such-foo'), None)
        sim_assert_eq!(have.section("Foo Bar").unwrap().get("no-such-foo"), None);
        // eq(cf['Spacey Bar'].get('foo', None), 'bar2')
        sim_assert_eq!(
            have.section("Spacey Bar").unwrap().get("foo").deref_inner(),
            Some("bar2")
        );
        // eq(cf['Spacey Bar'].get('foo', fallback=None), 'bar2')
        // with self.assertRaises(KeyError):
        //     cf['No Such Spacey Bar'].get('foo', None)
        sim_assert_eq!(
            std::panic::catch_unwind(|| have.section("No Such Spacey Bar").unwrap()["foo"].clone())
                .is_err(),
            true,
        );
        sim_assert_eq!(
            std::panic::catch_unwind(|| have.section("No Such Spacey Bar").unwrap()["foo"].clone())
                .is_err(),
            true,
        );
        // eq(cf['Types'].getint('int', 18), 42)
        // eq(cf['Types'].getint('int', fallback=18), 42)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_int("int")?
                .map(Spanned::into_inner),
            Some(42)
        );

        // eq(cf['Types'].getint('no-such-int', 18), 18)
        // eq(cf['Types'].getint('no-such-int', fallback=18), 18)
        // eq(cf['Types'].getint('no-such-int', "18"), "18") # sic!
        // eq(cf['Types'].getint('no-such-int', fallback="18"), "18") # sic!

        // eq(cf['Types'].getint('no-such-int'), None)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_int("no-such-int")?
                .map(Spanned::into_inner),
            None,
        );
        // self.assertAlmostEqual(cf['Types'].getfloat('float', 0.0), 0.44)
        // self.assertAlmostEqual(cf['Types'].getfloat('float', fallback=0.0), 0.44)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_float("float")?
                .map(Spanned::into_inner),
            Some(0.44),
        );
        // self.assertAlmostEqual(cf['Types'].getfloat('no-such-float', 0.0), 0.0)
        // self.assertAlmostEqual(cf['Types'].getfloat('no-such-float', fallback=0.0), 0.0)
        // eq(cf['Types'].getfloat('no-such-float', "0.0"), "0.0") # sic!
        // eq(cf['Types'].getfloat('no-such-float', fallback="0.0"), "0.0") # sic!

        // eq(cf['Types'].getfloat('no-such-float'), None)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_float("no-such-float")?
                .map(Spanned::into_inner),
            None,
        );
        // eq(cf['Types'].getboolean('boolean', True), False)
        // eq(cf['Types'].getboolean('boolean', fallback=True), False)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_bool("boolean")?
                .map(Spanned::into_inner),
            Some(false),
        );
        // eq(cf['Types'].getboolean('no-such-boolean', "yes"), "yes") # sic!
        // eq(cf['Types'].getboolean('no-such-boolean', fallback="yes"), "yes") # sic!
        // eq(cf['Types'].getboolean('no-such-boolean', True), True)
        // eq(cf['Types'].getboolean('no-such-boolean', fallback=True), True)

        // eq(cf['Types'].getboolean('no-such-boolean'), None)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_bool("no-such-boolean")?
                .map(Spanned::into_inner),
            None,
        );

        // Make sure the right things happen for remove_section() and
        // remove_option(); added to include check for SourceForge bug #123324.

        have.defaults_mut()
            .set(Spanned::from("this_value"), Spanned::from("1"));
        have.defaults_mut()
            .set(Spanned::from("that_value"), Spanned::from("2"));

        // API access
        assert!(have.remove_section("Spaces").is_some());
        sim_assert_eq!(have.has_option("Spaces", "key with spaces"), false);

        sim_assert_eq!(have.remove_section("Spaces"), None);
        // self.assertFalse(cf.remove_section(self.default_section))
        assert!(
            have.remove_option("Foo Bar", "foo").is_some(),
            "remove_option() failed to report existence of option"
        );
        // self.assertFalse(cf.has_option('Foo Bar', 'foo'),
        //             "remove_option() failed to remove option")
        sim_assert_eq!(
            have.has_option("Foo Bar", "foo"),
            false,
            "remove_option() failed to report existence of option"
        );

        // self.assertFalse(cf.remove_option('Foo Bar', 'foo'),
        //    "remove_option() failed to report non-existence of option that was removed")
        assert!(
            have.remove_option("Foo Bar", "foo").is_none(),
            "remove_option() failed to report non-existence of option that was removed"
        );

        // self.assertTrue(cf.has_option('Foo Bar', 'this_value'))
        assert!(have.has_option("Foo Bar", "this_value"));

        // self.assertFalse(cf.remove_option('Foo Bar', 'this_value'))
        assert!(have.remove_option("Foo Bar", "this_value").is_none());

        // self.assertTrue(cf.remove_option(self.default_section, 'this_value'))
        assert!(have.defaults_mut().remove_option("this_value").is_some());

        // self.assertFalse(cf.has_option('Foo Bar', 'this_value'))
        sim_assert_eq!(have.has_option("Foo Bar", "this_value"), false);
        // self.assertFalse(cf.remove_option(self.default_section, 'this_value'))
        assert!(have.defaults_mut().remove_option("this_value").is_none());

        // with self.assertRaises(configparser.NoSectionError) as cm:
        //     cf.remove_option('No Such Section', 'foo')
        assert!(have.remove_option("No Such Section", "foo").is_none());
        // self.assertEqual(cm.exception.args, ('No Such Section',))
        //
        // eq(cf.get('Long Line', 'foo'),
        //    'this line is much, much longer than my editor\nlikes it.')
        sim_assert_eq!(
            have.get("Long Line", "foo").deref_inner(),
            Some("this line is much, much longer than my editor\nlikes it."),
        );

        // mapping access
        have.remove_section("Types");
        sim_assert_eq!(have.has_section("Types"), false);

        // with self.assertRaises(KeyError):
        //     del cf['Types']
        sim_assert_eq!(have.remove_section("Types"), None);

        // with self.assertRaises(ValueError):
        //     del cf[self.default_section]

        // del cf['Spacey Bar']['foo']
        assert!(have.remove_option("Spacey Bar", "foo").is_some());

        // self.assertFalse('foo' in cf['Spacey Bar'])
        sim_assert_eq!(have.section("Spacey Bar").unwrap().has_option("foo"), false);

        // with self.assertRaises(KeyError):
        //     del cf['Spacey Bar']['foo']
        sim_assert_eq!(
            have.section_mut("Spacey Bar").unwrap().remove_option("foo"),
            None
        );

        // self.assertTrue('that_value' in cf['Spacey Bar'])
        sim_assert_eq!(
            have.section("Spacey Bar").unwrap().has_option("that_value"),
            true
        );

        // with self.assertRaises(KeyError):
        //     del cf['Spacey Bar']['that_value']
        sim_assert_eq!(
            have.section_mut("Spacey Bar")
                .unwrap()
                .remove_option("that_value"),
            None
        );

        // del cf[self.default_section]['that_value']
        // self.assertFalse('that_value' in cf['Spacey Bar'])
        // with self.assertRaises(KeyError):
        //     del cf[self.default_section]['that_value']
        // with self.assertRaises(KeyError):
        //     del cf['No Such Section']['foo']
        Ok(())
    }

    #[test]
    fn configparser_compat_case_sensitivity() -> eyre::Result<()> {
        crate::tests::init();

        let mut config = Value::default();
        config.add_section(Spanned::from("A"), []);
        config.add_section(Spanned::from("a"), []);
        config.add_section(Spanned::from("B"), []);

        sim_assert_eq!(config.section_names().collect::<Vec<_>>(), ["A", "a", "B"]);

        config.set("a", Spanned::from("B"), Spanned::from("value"))?;
        sim_assert_eq!(config.options("a").collect::<Vec<_>>(), ["b"]);
        sim_assert_eq!(
            config.get("a", "b").deref_inner(),
            Some("value"),
            "could not locate option, expecting case-insensitive option names"
        );

        // with self.assertRaises(configparser.NoSectionError):
        //     # section names are case-sensitive
        //     cf.set("b", "A", "value")

        sim_assert_eq!(
            config
                .set("b", Spanned::from("A"), Spanned::from("value"))
                .unwrap_err()
                .to_string(),
            r#"missing section: "b""#,
        );

        sim_assert_eq!(config.has_option("a", "b"), true);
        sim_assert_eq!(config.has_option("b", "b"), false);

        config.set("A", Spanned::from("A-B"), Spanned::from("A-B value"))?;

        for option in ["a-b", "A-b", "a-B", "A-B"] {
            // dbg!(config.get())
            sim_assert_eq!(
                config.has_option("A", option),
                true,
                "has_option() returned false for option which should exist",
            );
        }

        sim_assert_eq!(config.options("A").collect::<Vec<_>>(), ["a-b"]);
        sim_assert_eq!(config.options("a").collect::<Vec<_>>(), ["b"]);

        config.remove_option("a", "B");
        sim_assert_eq!(config.options("a").collect::<Vec<_>>(), [] as [&str; 0]);

        // SF bug #432369:
        let config = unindent::unindent(&format!(
            "
            [MySection]
            Option{} first line   
            \tsecond line   
            ",
            DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let config = parse(&config, Options::default(), &Printer::default()).0?;

        sim_assert_eq!(config.options("MySection").collect::<Vec<_>>(), ["option"]);
        sim_assert_eq!(
            config.get("MySection", "Option").deref_inner(),
            Some("first line\nsecond line")
        );

        // SF bug #561822:
        let config = unindent::unindent(&format!(
            r"
            [section]
            nekey{}nevalue\n
            ",
            DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let config = parse(&config, Options::default(), &Printer::default()).0?;

        // cf = self.fromstring("[section]\n"
        //                      "nekey{}nevalue\n".format(self.delimiters[0]),
        //                      defaults={"key":"value"})
        // self.assertTrue(cf.has_option("section", "Key"))
        // TODO(roman): this was true but we do not implement defaults
        sim_assert_eq!(config.has_option("section", "Key"), false);
        Ok(())
    }

    #[test]
    fn configparser_compat_case_insensitivity_mapping_access() -> eyre::Result<()> {
        crate::tests::init();

        let mut config = Value::default();
        config.add_section(Spanned::from("A"), []);
        config.add_section(
            Spanned::from("a"),
            [(Spanned::from("B"), Spanned::from("value"))],
        );
        config.add_section(Spanned::from("B"), []);

        sim_assert_eq!(config.section_names().collect::<Vec<_>>(), ["A", "a", "B"]);

        sim_assert_eq!(
            config.section("a").unwrap().keys().collect::<Vec<_>>(),
            ["b"]
        );

        sim_assert_eq!(
            &config.section("a").unwrap()["b"],
            "value",
            "could not locate option, expecting case-insensitive option names"
        );

        // with self.assertRaises(KeyError):
        //     # section names are case-sensitive
        //     cf["b"]["A"] = "value"

        sim_assert_eq!(
            std::panic::catch_unwind(|| config.section("b").unwrap()["A"].clone()).is_err(),
            true,
        );

        sim_assert_eq!(config.section("a").unwrap().has_option("b"), true);

        config
            .section_mut("A")
            .unwrap()
            .set(Spanned::from("A-B"), Spanned::from("A-B value"));

        for option in ["a-b", "A-b", "a-B", "A-B"] {
            sim_assert_eq!(
                config.get("A", option).is_some(),
                true,
                "has_option() returned false for option which should exist"
            );
        }

        sim_assert_eq!(
            config.section("A").unwrap().keys().collect::<Vec<_>>(),
            ["a-b"]
        );
        sim_assert_eq!(
            config.section("a").unwrap().keys().collect::<Vec<_>>(),
            ["b"]
        );
        config.remove_option("a", "B");

        sim_assert_eq!(
            config.section("a").unwrap().keys().collect::<Vec<_>>(),
            [] as [&str; 0]
        );

        // SF bug #432369:
        let config = format!(
            "[MySection]\nOption{} first line   \n\tsecond line   \n",
            DEFAULT_ASSIGNMENT_DELIMITERS[0],
        );
        let config = parse(&config, Options::default(), &Printer::default()).0?;

        sim_assert_eq!(
            config
                .section("MySection")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["option"]
        );
        sim_assert_eq!(
            config.section("MySection").unwrap()["Option"]
                .as_ref()
                .as_str(),
            "first line\nsecond line",
        );

        // SF bug #561822:
        // let config = format!(
        //     "[MySection]\nOption{} first line   \n\tsecond line   \n",
        //     DEFAULT_ASSIGNMENT_DELIMITERS[0],
        // );
        // let mut config = parse(&config, &Printer::default()).0?;

        // cf = self.fromstring("[section]\n"
        //                      "nekey{}nevalue\n".format(self.delimiters[0]),
        //                      defaults={"key":"value"})
        // self.assertTrue("Key" in cf["section"])
        Ok(())
    }

    #[test]
    fn configparser_compat_default_case_sensitivity() -> eyre::Result<()> {
        crate::tests::init();

        let config = Value::with_defaults(
            [(Spanned::from("foo"), Spanned::from("Bar"))]
                .into_iter()
                .collect(),
        );

        dbg!(&config);

        sim_assert_eq!(
            config.defaults().get("Foo").deref_inner(),
            Some("Bar"),
            "could not locate option, expecting case-insensitive option names",
        );

        let config = Value::with_defaults(
            [(Spanned::from("Foo"), Spanned::from("Bar"))]
                .into_iter()
                .collect(),
        );

        sim_assert_eq!(
            config.defaults().get("Foo").deref_inner(),
            Some("Bar"),
            "could not locate option, expecting case-insensitive defaults",
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_parse_errors() -> eyre::Result<()> {
        crate::tests::init();

        let config = format!(
            "[Foo]\n{}val-without-opt-name\n",
            DEFAULT_ASSIGNMENT_DELIMITERS[0]
        );
        let config = parse(&config, Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: empty option name")
        );

        let config = format!(
            "[Foo]\n{}val-without-opt-name\n",
            DEFAULT_ASSIGNMENT_DELIMITERS[1]
        );
        let config = parse(&config, Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: empty option name")
        );

        let config = "No Section!\n"; // python configparser raises `MissingSectionHeaderError`
        let config = parse(config, Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: variable assignment missing one of: `=`, `:`")
        );
        // self.assertEqual(e.args, ('<???>', 1, "No Section!\n"))

        let config = "[Foo]\n  wrong-indent\n";
        let config = parse(config, Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: variable assignment missing one of: `=`, `:`")
        );

        // # read_file on a real file
        // tricky = support.findfile("cfgparser.3", subdir="configdata")
        // if self.delimiters[0] == '=':
        //     error = configparser.ParsingError
        //     expected = (tricky,)
        // else:
        //     error = configparser.MissingSectionHeaderError
        //     expected = (tricky, 1,
        //                 '  # INI with as many tricky parts as possible\n')
        // with open(tricky, encoding='utf-8') as f:
        //     e = self.parse_error(cf, error, f)
        // self.assertEqual(e.args, expected)

        Ok(())
    }

    #[test]
    fn configparser_compat_query_errors() -> eyre::Result<()> {
        crate::tests::init();

        let mut config = Value::default();
        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            [] as [&str; 0],
            "new ConfigParser should have no defined sections"
        );
        sim_assert_eq!(
            config.has_section("Foo"),
            false,
            "new ConfigParser should have no acknowledged sections"
        );

        // with self.assertRaises(configparser.NoSectionError):
        sim_assert_eq!(config.options("Foo").collect::<Vec<_>>(), [] as [&str; 0]);

        // with self.assertRaises(configparser.NoSectionError):
        //     cf.set("foo", "bar", "value")
        sim_assert_eq!(
            config
                .set("foo", Spanned::from("bar"), Spanned::from("value"))
                .err(),
            Some(NoSectionError("foo".to_string()))
        );

        config.add_section(Spanned::from("foo"), []);
        sim_assert_eq!(config.get("foo", "bar"), None);

        sim_assert_eq!(
            config
                .set("foo", Spanned::from("bar"), Spanned::from("value"))
                .err(),
            None,
        );

        Ok(())
    }

    #[test]
    fn configparser_compat_boolean() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent::unindent(&format!(
            "
            [BOOLTEST]\n
            T1{equals}1\n
            T2{equals}TRUE\n
            T3{equals}True\n
            T4{equals}oN\n
            T5{equals}yes\n
            F1{equals}0\n
            F2{equals}FALSE\n
            F3{equals}False\n
            F4{equals}oFF\n
            F5{equals}nO\n
            E1{equals}2\n
            E2{equals}foo\n
            E3{equals}-1\n
            E4{equals}0.1\n
            E5{equals}FALSE AND MORE",
            equals = DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let config = parse(&config, Options::default(), &Printer::default()).0?;

        for x in 1..5 {
            sim_assert_eq!(
                config
                    .get_bool("BOOLTEST", &format!("t{x}"))?
                    .map(Spanned::into_inner),
                Some(true)
            );
            sim_assert_eq!(
                config
                    .get_bool("BOOLTEST", &format!("f{x}"))?
                    .map(Spanned::into_inner),
                Some(false)
            );
            assert!(config
                .get_bool("BOOLTEST", &format!("e{x}"))
                .unwrap_err()
                .to_string()
                .starts_with("invalid boolean: "));
        }
        Ok(())
    }

    #[test]
    fn configparser_compat_weird_errors() -> eyre::Result<()> {
        crate::tests::init();

        let mut config = Value::default();
        config.add_section(Spanned::from("Foo"), []);

        // unlike configparser, we do not raise `DuplicateSectionError`,
        // however, the user can manually detect when a key is present more than once
        sim_assert_eq!(
            config.add_section(Spanned::from("Foo"), []),
            Some(Section::from_iter([]).with_name(Spanned::from("Foo"))),
        );

        // our implementation is very relaxed in that we collect all the options from all the
        // occurrences of the same of section
        let config = unindent(&format!(
            "
            [Foo]
            will this be added{equals}True
            [Bar]
            what about this{equals}True
            [Foo]
            oops{equals}this won't
            ",
            equals = DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let config = parse(&config, Options::default(), &Printer::default()).0?;
        let mut expected = Value::default();
        expected.add_section(
            Spanned::from("Foo"),
            [
                (Spanned::from("will this be added"), Spanned::from("True")),
                (Spanned::from("oops"), Spanned::from("this won't")),
            ],
        );
        expected.add_section(
            Spanned::from("Bar"),
            [(Spanned::from("what about this"), Spanned::from("True"))],
        );

        sim_assert_eq!(config.cleared_spans(), expected);
        Ok(())
    }

    #[test]
    fn configparser_compat_get_after_duplicate_option_error() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent(&format!(
            "
            [Foo]
            x{equals}1
            y{equals}2
            y{equals}3
            ",
            equals = DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let options = Options {
            strict: true,
            ..Options::default()
        };
        let config = parse(&config, options, &Printer::default()).0?;
        sim_assert_eq!(config.get("Foo", "x").deref_inner(), Some("1"));
        sim_assert_eq!(config.get("Foo", "y").deref_inner(), Some("2"));
        Ok(())
    }

    #[test]
    fn configparser_compat_set_string_types() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent(&format!(
            "
            [sect]
            option1{equals}foo
            ",
            equals = DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let mut config = parse(&config, Options::default(), &Printer::default()).0?;

        // check that we don't get an exception when setting values in
        // an existing section using strings:

        config.set("sect", "option1".into(), "splat".into())?;
        config.set("sect", "option1".into(), "splat".to_string().into())?;
        config.set("sect", "option2".into(), "splat".into())?;
        config.set("sect", "option2".into(), "splat".to_string().into())?;
        config.set("sect", "option1".into(), "splat".into())?;
        config.set("sect", "option2".into(), "splat".into())?;
        Ok(())
    }

    #[test]
    fn configparser_compat_check_items_config() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent(&format!(
            r"
            default {delim0} <default>

            [section]
            name {delim0} %(value)s
            key{delim1} |%(name)s|
            getdefault{delim1} |%(default)s|
            ",
            delim0 = DEFAULT_ASSIGNMENT_DELIMITERS[0],
            delim1 = DEFAULT_ASSIGNMENT_DELIMITERS[1],
        ));
        let config = parse(&config, Options::default(), &Printer::default()).0?;
        sim_assert_eq!(
            config.section("section").unwrap().items_vec(),
            vec![
                ("default", "<default>"),
                ("name", "%(value)s"),
                ("key", "|%(name)s|"),
                ("getdefault", "|%(default)s|"),
            ]
        );
        sim_assert_eq!(config.section("no such section"), None);
        Ok(())
    }

    #[test]
    fn configparser_compat_popitem() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent(&format!(
            r"
            [section1]
            name1 {delim0} value1
            [section2]
            name2 {delim0} value2
            [section3]
            name3 {delim0} value3
            ",
            delim0 = DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let mut config = parse(&config, Options::default(), &Printer::default()).0?;

        sim_assert_eq!(
            config.pop().map(|section| section.name).as_deref(),
            Some("section1")
        );
        sim_assert_eq!(
            config.pop().map(|section| section.name).as_deref(),
            Some("section2")
        );
        sim_assert_eq!(
            config.pop().map(|section| section.name).as_deref(),
            Some("section3")
        );
        sim_assert_eq!(config.pop(), None);
        Ok(())
    }

    #[test]
    fn configparser_compat_clear() -> eyre::Result<()> {
        crate::tests::init();

        let mut config = Value::default();
        config.defaults_mut().set("foo".into(), "Bar".into());
        sim_assert_eq!(
            config.defaults().get("Foo").deref_inner(),
            Some("Bar"),
            "could not locate option, expecting case-insensitive option names"
        );

        config.add_section(
            "zing".into(),
            [
                ("option1".into(), "value1".into()),
                ("option2".into(), "value2".into()),
            ],
        );

        sim_assert_eq!(config.section_names().collect::<Vec<_>>(), vec!["zing"]);
        sim_assert_eq!(
            config
                .section("zing")
                .map(super::super::tests::SectionProxyExt::keys_vec),
            Some(vec!["option1", "option2", "foo"]),
        );

        config.clear();
        sim_assert_eq!(
            config.section_names().collect::<Vec<&Spanned<String>>>(),
            vec![] as Vec<&Spanned<String>>
        );
        sim_assert_eq!(
            config
                .defaults()
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<&str>>(),
            vec!["foo"]
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_setitem() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent(&format!(
            r"
            nameD {equals} valueD
            [section1]
            name1 {equals} value1
            [section2]
            name2 {equals} value2
            [section3]
            name3 {equals} value3
            ",
            equals = DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let mut config = parse(&config, Options::default(), &Printer::default()).0?;

        sim_assert_eq!(
            config.section("section1").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name1", "named"])
        );
        sim_assert_eq!(
            config.section("section2").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name2", "named"])
        );
        sim_assert_eq!(
            config.section("section3").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name3", "named"])
        );
        sim_assert_eq!(
            config
                .section("section1")
                .and_then(|section| section.get("name1"))
                .deref_inner(),
            Some("value1")
        );
        sim_assert_eq!(
            config
                .section("section2")
                .and_then(|section| section.get("name2"))
                .deref_inner(),
            Some("value2")
        );
        sim_assert_eq!(
            config
                .section("section3")
                .and_then(|section| section.get("name3"))
                .deref_inner(),
            Some("value3")
        );
        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );
        config.add_section("section2".into(), [("name22".into(), "value22".into())]);
        sim_assert_eq!(
            config.section("section2").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name22", "named"]),
        );
        sim_assert_eq!(
            config
                .section("section2")
                .and_then(|section| section.get("name22"))
                .deref_inner(),
            Some("value22")
        );
        assert!(!config.section("section2").unwrap().has_option("name2"));
        sim_assert_eq!(config.section("section2").unwrap().get("name2"), None);

        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );
        config.add_section("section3".into(), []);
        sim_assert_eq!(
            config.section("section3").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["named"])
        );

        assert!(!config.section("section3").unwrap().has_option("name3"));
        sim_assert_eq!(config.section("section3").unwrap().get("name3"), None);

        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );
        // For bpo-32108, assigning default_section to itself.
        *config.defaults_mut() = config.defaults().clone();
        assert_ne!(
            config.defaults().keys().collect::<Vec<&Spanned<String>>>(),
            vec![] as Vec<&Spanned<String>>
        );
        *config.defaults_mut() = Section::default();

        sim_assert_eq!(
            config.defaults().keys().collect::<Vec<_>>(),
            vec![] as Vec<&Spanned<String>>
        );
        sim_assert_eq!(
            config
                .section("section1")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["name1"]
        );
        sim_assert_eq!(
            config
                .section("section2")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["name22"]
        );
        sim_assert_eq!(
            config
                .section("section3")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec![] as Vec<&Spanned<String>>
        );
        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );

        // For bpo-32108, assigning section to itself.
        // *config.section_mut("section2").unwrap().as_mut() =
        //     config.section("section2").unwrap().clone();
        let section2: Section = config.section("section2").unwrap().as_ref().clone();
        config
            .section_mut("section2")
            .unwrap()
            .replace_with(section2);
        sim_assert_eq!(
            config
                .section("section2")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["name22"]
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_invalid_multiline_value() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent(&format!(
            "\
            [DEFAULT]
            test {equals} test
            invalid\
            ",
            equals = DEFAULT_ASSIGNMENT_DELIMITERS[0],
        ));
        let res = parse(&config, Options::default(), &Printer::default()).0;
        let err = res.err().map(|err| err.to_string());
        sim_assert_eq!(
            err.as_deref(),
            Some("syntax error: variable assignment missing one of: `=`, `:`")
        );

        // sim_assert_eq!(
        //     config.section("DEFAULT").unwrap().get("test").deref_inner(),
        //     Some("test")
        // );
        // sim_assert_eq!(
        //     config.section("DEFAULT").unwrap().get("test").deref_inner(),
        //     Some("test")
        // );
        Ok(())
    }

    #[test]
    fn configparser_compat_defaults_keyword() -> eyre::Result<()> {
        crate::tests::init();

        // bpo-23835 fix for ConfigParser
        let mut config = Value::default();
        config.defaults_mut().set("1".into(), "2.4".into());

        sim_assert_eq!(config.defaults().get("1").deref_inner(), Some("2.4"));
        sim_assert_eq!(
            config
                .defaults()
                .get_float("1")?
                .as_ref()
                .map(std::convert::AsRef::as_ref)
                .copied(),
            Some(2.4)
        );

        let mut config = Value::default();
        config.defaults_mut().set("A".into(), "5.2".into());
        sim_assert_eq!(config.defaults().get("a").deref_inner(), Some("5.2"));
        sim_assert_eq!(
            config
                .defaults()
                .get_float("a")?
                .as_ref()
                .map(std::convert::AsRef::as_ref)
                .copied(),
            Some(5.2)
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_no_interpolation_matches_ini() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent(
            r"
            [numbers]
            one = 1
            two = %(one)s * 2
            three = ${common:one} * 3

            [hexen]
            sixteen = ${numbers:two} * 8
            ",
        );
        let config = parse(&config, Options::default(), &Printer::default()).0?;

        sim_assert_eq!(config.get("numbers", "one").deref_inner(), Some("1"));
        sim_assert_eq!(
            config.get("numbers", "two").deref_inner(),
            Some("%(one)s * 2")
        );
        sim_assert_eq!(
            config.get("numbers", "three").deref_inner(),
            Some("${common:one} * 3")
        );
        sim_assert_eq!(
            config.get("hexen", "sixteen").deref_inner(),
            Some("${numbers:two} * 8")
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_empty_case() -> eyre::Result<()> {
        crate::tests::init();

        let config = parse("", Options::default(), &Printer::default()).0?;
        sim_assert_eq!(config, Value::default());
        assert!(config.is_empty());
        Ok(())
    }

    #[test]
    fn configparser_compat_dominating_multiline_values() -> eyre::Result<()> {
        crate::tests::init();

        let wonderful_spam =
            "I'm having spam spam spam spam spam spam spam beaked beans spam spam spam and spam!"
                .replace(' ', "\n\t");

        // we're reading from file because this is where the code changed
        // during performance updates in Python 3.2
        let mut config = Value::default();
        for i in 0..100 {
            config.add_section(
                format!("section{i}").into(),
                (0..10)
                    .map(|j| {
                        (
                            format!("lovely_spam{j}").into(),
                            wonderful_spam.clone().into(),
                        )
                    })
                    .collect::<Section>(),
            );
        }
        let have = config.get("section8", "lovely_spam4");
        let want = &wonderful_spam;
        sim_assert_eq!(have.deref_inner(), Some(want.as_str()));

        let mut config = String::new();
        for i in 0..2 {
            config += &format!("[section{i}]\n");
            for j in 0..5 {
                config += &format!("lovely_spam{j} = {wonderful_spam}\n");
            }
        }
        let config = parse(&config, Options::default(), &Printer::default()).0?;
        let have = config.get("section1", "lovely_spam4");
        let want = wonderful_spam.replace("\n\t", "\n");
        sim_assert_eq!(have.deref_inner(), Some(want.as_str()));
        Ok(())
    }

    #[ignore = "allow non-string values"]
    #[test]
    fn configparser_compat_set_nonstring_types() -> eyre::Result<()> {
        crate::tests::init();

        let mut config = Value::default();
        config.add_section("non-string".into(), []);
        config.set("non-string", "int".into(), "1".into())?;
        todo!("support for different value types similar to serde_json");
        // config.set("non-string", "list", vec![0, 1, 1, 2, 3, 5, 8, 13]);
        // // config.set("non-string", "dict", {'pi': 3.14159});
        // sim_assert_eq!(config.get("non-string", "int"), Some(1));
        // sim_assert_eq!(config.get("non-string", "list"), Some(vec![0, 1, 1, 2, 3, 5, 8, 13]));
        // // sim_assert_eq!(config.get("non-string", "dict"), {'pi': 3.14159});
        // config.add_section(123);
        // config.set(123, "this is sick", True);
        // sim_assert_eq!(config.get(123, "this is sick"), True);
        // Ok(())
    }

    #[test]
    fn configparser_compat_parse_cfgparser_1() -> eyre::Result<()> {
        crate::tests::init();
        let config = include_str!("../test-data/cfgparser.1.ini");
        let config = parse(config, Options::default(), &Printer::default()).0?;
        println!("{}", &config);
        Ok(())
    }

    #[test]
    fn configparser_compat_parse_cfgparser_2() -> eyre::Result<()> {
        crate::tests::init();
        let config = include_str!("../test-data/cfgparser.2.ini");
        // let config = include_str!("../test-data/cfgparser.0.ini");
        let options = Options {
            parser_config: ParseConfig {
                comment_prefixes: vec![";", "#", "----", "//"],
                inline_comment_prefixes: vec!["//"],
                allow_empty_lines_in_values: false,
                ..ParseConfig::default()
            },
            ..Options::default()
        };
        let config = parse(config, options, &Printer::default()).0?;
        println!("{}", &config);

        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec![
                "global",
                "homes",
                "printers",
                "print$",
                "pdf-generator",
                "tmp",
                "Agustin",
            ]
        );
        sim_assert_eq!(
            config.get("global", "workgroup").deref_inner(),
            Some("MDKGROUP")
        );
        sim_assert_eq!(
            config
                .get_int("global", "max log size")?
                .as_ref()
                .map(std::convert::AsRef::as_ref)
                .copied(),
            Some(50)
        );
        sim_assert_eq!(
            config.get("global", "hosts allow").deref_inner(),
            Some("127.")
        );
        sim_assert_eq!(
            config.get("tmp", "echo command").deref_inner(),
            Some("cat %s; rm %s")
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_parse_cfgparser_3() -> eyre::Result<()> {
        crate::tests::init();

        let config = include_str!("../test-data/cfgparser.3.ini");
        // let config = include_str!("../test-data/cfgparser.0.ini");
        let options = Options {
            parser_config: ParseConfig {
                comment_prefixes: vec![";", "#"],
                inline_comment_prefixes: vec!["#"],
                allow_empty_lines_in_values: true,
                ..ParseConfig::default()
            },
            ..Options::default()
        };
        let config = parse(config, options, &Printer::default()).0?;
        println!("{}", &config);

        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec![
                "DEFAULT",
                "strange",
                "corruption",
                "yeah, sections can be indented as well",
                "another one!",
                "no values here",
                "tricky interpolation",
                "more interpolation",
            ]
        );
        sim_assert_eq!(
            config.section("DEFAULT").unwrap().items_vec(),
            vec![("go", "%(interpolate)s")]
        );
        sim_assert_eq!(
            config.section("strange").unwrap().items_vec(),
            vec![
                ("values", "that are indented"),
                (
                    "other",
                    indoc::indoc! {
                        "that do continue
                          in
                          other
                          lines",
                    }
                ),
            ]
        );
        let multiline_expected = indoc::indoc! {"\
            that is


            actually still here


            and holds all these weird newlines


            nor the indentation"};

        sim_assert_eq!(multiline_expected, "that is\n\n\nactually still here\n\n\nand holds all these weird newlines\n\n\nnor the indentation");
        sim_assert_eq!(
            config.section("corruption").unwrap().items_vec(),
            vec![
                ("value", multiline_expected),
                ("another value", ""),
                // ("yet another", "")
            ]
        );
        sim_assert_eq!(
            config
                .section("yeah, sections can be indented as well")
                .unwrap()
                .items_vec(),
            vec![
                ("and that does not mean", "anything"),
                ("are they subsections", "False"),
                ("if you want subsections", "use XML"),
                ("lets use some unicode", "片仮名"), // note: lowercased key
            ]
        );
        sim_assert_eq!(
            config.section("another one!").unwrap().items_vec(),
            vec![
                ("even if values are indented like this", "seriously"),
                ("yes, this still applies to", r#"section "another one!""#),
                (
                    "this too",
                    indoc::indoc! {
                        r#"are there people with configurations broken as this?
                        beware, this is going to be a continuation
                        of the value for
                        key "this too"
                        even if it has a = character
                        this is still the continuation
                        your editor probably highlights it wrong
                        but that's life"#,
                    }
                ),
                ("interpolate", "anything will do"),
            ]
        );
        sim_assert_eq!(
            config.section("no values here").unwrap().items_vec(),
            vec![],
        );
        sim_assert_eq!(
            config.section("tricky interpolation").unwrap().items_vec(),
            vec![("interpolate", "do this"), ("lets", "%(go)s"),],
        );
        sim_assert_eq!(
            config.section("more interpolation").unwrap().items_vec(),
            vec![("interpolate", "go shopping"), ("lets", "%(go)s"),],
        );

        Ok(())
    }

    /// Basic configparser compat test
    ///
    /// adapted from: <https://github.com/python/cpython/blob/3.13/Lib/test/test_configparser.py#L294>
    #[test]
    fn configparser_compat_basic() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent::unindent(&format!(
            r"
            [Foo Bar]
            foo{d0}bar1
            [Spacey Bar]
            foo {d0} bar2
            [Spacey Bar From The Beginning]
              foo {d0} bar3
              baz {d0} qwe
            [Commented Bar]
            foo{d1} bar4 {c1} comment
            baz{d0}qwe {c0}another one
            [Long Line]
            foo{d1} this line is much, much longer than my editor
               likes it.
            [Section\with$weird%characters[\t]
            [Internationalized Stuff]
            foo[bg]{d1} Bulgarian
            foo{d0}Default
            foo[en]{d0}English
            foo[de]{d0}Deutsch
            [Spaces]
            key with spaces {d1} value
            another with spaces {d0} splat!
            [Types]
            int {d1} 42
            float {d0} 0.44
            boolean {d0} NO
            123 {d1} strange but acceptable
            [This One Has A ] In It]
              forks {d0} spoons
            ",
            d0 = DEFAULT_ASSIGNMENT_DELIMITERS[0],
            d1 = DEFAULT_ASSIGNMENT_DELIMITERS[1],
            c0 = DEFAULT_COMMENT_PREFIXES[0],
            c1 = DEFAULT_COMMENT_PREFIXES[1],
        ));

        let options = Options {
            parser_config: ParseConfig {
                inline_comment_prefixes: vec!["#", ";"],
                ..ParseConfig::default()
            },
            ..Options::default()
        };
        let mut have = parse(&config, options, &Printer::default()).0?;
        check_configparser_compat_basic(&mut have)?;
        // let have = super::value::from_str(&config).map_?;
        // let expected = Value {
        //     sections: [
        //         (
        //             Spanned::from("DEFAULT".to_string()),
        //             [
        //                 (
        //                     Spanned::from("key1".to_string()),
        //                     Spanned::from("value1".to_string()),
        //                 ),
        //                 (
        //                     Spanned::from("pizzatime".to_string()),
        //                     Spanned::from("yes".to_string()),
        //                 ),
        //                 (
        //                     Spanned::from("cost".to_string()),
        //                     Spanned::from("9".to_string()),
        //                 ),
        //             ]
        //             .into_iter()
        //             .collect(),
        //         ),
        //         (
        //             Spanned::from("topsecrets".to_string()),
        //             [(
        //                 Spanned::from("nuclear launch codes".to_string()),
        //                 Spanned::from("topsecret".to_string()),
        //             )]
        //             .into_iter()
        //             .collect(),
        //         ),
        //         (
        //             Spanned::from("github.com".to_string()),
        //             [(
        //                 Spanned::from("User".to_string()),
        //                 Spanned::from("QEDK".to_string()),
        //             )]
        //             .into_iter()
        //             .collect(),
        //         ),
        //     ]
        //     .into_iter()
        //     .collect(),
        //     global: [].into_iter().collect(),
        // };

        // if self.allow_no_value:
        //     config_string += (
        //         "[NoValue]\n"
        //         "option-without-value\n"
        //         )
        // cf = self.fromstring(config_string)
        // self.basic_test(cf)
        // if self.strict:
        //     with self.assertRaises(configparser.DuplicateOptionError):
        //         cf.read_string(textwrap.dedent("""\
        //             [Duplicate Options Here]
        //             option {0[0]} with a value
        //             option {0[1]} with another value
        //         """.format(self.delimiters)))
        //     with self.assertRaises(configparser.DuplicateSectionError):
        //         cf.read_string(textwrap.dedent("""\
        //             [And Now For Something]
        //             completely different {0[0]} True
        //             [And Now For Something]
        //             the larch {0[1]} 1
        //         """.format(self.delimiters)))
        // else:
        //     cf.read_string(textwrap.dedent("""\
        //         [Duplicate Options Here]
        //         option {0[0]} with a value
        //         option {0[1]} with another value
        //     """.format(self.delimiters)))
        //
        //     cf.read_string(textwrap.dedent("""\
        //         [And Now For Something]
        //         completely different {0[0]} True
        //         [And Now For Something]
        //         the larch {0[1]} 1
        //     """.format(self.delimiters)))
        Ok(())
    }

    #[test]
    fn parse_ini_multi_line_continuation() -> eyre::Result<()> {
        crate::tests::init();

        let config = indoc::indoc! {r"
            [options.packages.find]
            exclude =
                example*
                tests*
                docs*
                build

            [bumpversion:file:CHANGELOG.md]
            replace = **unreleased**
                **v{new_version}**

            [bumpversion:part:release]
            optional_value = gamma
            values =
                dev
                gamma
        "};

        let have = parse(config, Options::default(), &Printer::default()).0?;
        dbg!(&have);
        let mut expected = Value::with_defaults([].into_iter().collect());
        expected.add_section(
            Spanned::from("options.packages.find"),
            [(
                Spanned::from("exclude"),
                Spanned::from("\nexample*\ntests*\ndocs*\nbuild"),
            )],
        );
        expected.add_section(
            Spanned::from("bumpversion:file:CHANGELOG.md"),
            [(
                Spanned::from("replace"),
                Spanned::from("**unreleased**\n**v{new_version}**"),
            )],
        );
        expected.add_section(
            Spanned::from("bumpversion:part:release"),
            [
                (Spanned::from("optional_value"), Spanned::from("gamma")),
                (Spanned::from("values"), Spanned::from("\ndev\ngamma")),
            ],
        );

        sim_assert_eq!(have.clone().cleared_spans(), expected);
        Ok(())
    }
}
