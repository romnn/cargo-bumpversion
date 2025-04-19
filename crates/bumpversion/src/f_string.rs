//! Parsing support for Python-style format strings used in version templates.
//!
//! Provides utilities to split format strings into literal text and argument placeholders,
//! and to unescape double curly braces.
pub use parser::ParseError;
use std::collections::HashMap;

/// A segment of a format string: either literal text or a placeholder.
///
/// `Value::String` holds literal content, while `Value::Argument` represents `{name}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    String(String),
    Argument(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, "{s}"),
            Self::Argument(arg) => write!(f, "{{{arg}}}"),
        }
    }
}

impl Value {
    /// If this is an argument placeholder, return its name, otherwise `None`.
    ///
    /// # Examples
    /// ```
    /// use bumpversion::f_string::Value;
    /// assert_eq!(Value::Argument("x".to_string()).as_argument(), Some("x"));
    /// assert_eq!(Value::String("x".to_string()).as_argument(), None);
    /// ```
    #[must_use]
    pub fn as_argument(&self) -> Option<&str> {
        match self {
            Self::Argument(arg) => Some(arg),
            _ => None,
        }
    }

    /// Returns `true` if this value is a placeholder (`Argument`).
    ///
    /// # Examples
    /// ```
    /// use bumpversion::f_string::Value;
    /// assert!(Value::Argument("y".to_string()).is_argument());
    /// assert!(!Value::String("y".to_string()).is_argument());
    /// ```
    #[must_use]
    pub fn is_argument(&self) -> bool {
        matches!(self, Self::Argument(_))
    }
}

impl<'a> From<parser::Value<'a>> for Value {
    fn from(value: parser::Value<'a>) -> Self {
        match value {
            parser::Value::String(s) => Self::String(s.to_string()),
            parser::Value::Argument(s) => Self::Argument(s.to_string()),
        }
    }
}

pub mod parser {
    //! Internal module implementing the parser for format strings.
    //!
    //! Users should call `escape_double_curly_braces` or `parse_format_arguments`.
    use winnow::combinator::{alt, delimited, repeat};
    use winnow::error::InputError;
    use winnow::prelude::*;

    use winnow::token::take_while;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Value<'a> {
        String(String),
        Argument(&'a str),
    }

    fn any_except_curly_bracket0<'a>(s: &mut &'a str) -> ModalResult<&'a str, InputError<&'a str>> {
        take_while(0.., |c| c != '{' && c != '}')
            .context("any_except_curly_bracket0")
            .parse_next(s)
    }

    fn any_except_curly_bracket1<'a>(s: &mut &'a str) -> ModalResult<&'a str, InputError<&'a str>> {
        take_while(1.., |c| c != '{' && c != '}')
            .context("any_except_curly_bracket1")
            .parse_next(s)
    }

    fn text_including_escaped_brackets<'a>(
        s: &mut &'a str,
    ) -> ModalResult<String, InputError<&'a str>> {
        repeat(
            1..,
            alt((any_except_curly_bracket1, "{{".value("{"), "}}".value("}"))),
        )
        .fold(String::new, |mut string, c| {
            string.push_str(c);
            string
        })
        .context("text_including_escaped_brackets")
        .parse_next(s)
    }

    fn non_escaped_bracket_argument<'a>(
        s: &mut &'a str,
    ) -> ModalResult<Value<'a>, InputError<&'a str>> {
        delimited("{", any_except_curly_bracket0, "}")
            .map(Value::Argument)
            .context("non_escaped_bracket_argument")
            .parse_next(s)
    }

    fn text_or_argument<'a>(s: &mut &'a str) -> ModalResult<Value<'a>, InputError<&'a str>> {
        alt((
            text_including_escaped_brackets.map(Value::String),
            non_escaped_bracket_argument,
        ))
        .context("text_or_argument")
        .parse_next(s)
    }

    #[derive(thiserror::Error, Debug, PartialEq, Eq)]
    #[error("invalid format: {format_string:?}")]
    pub struct ParseError {
        pub format_string: String,
    }

    /// Unescape doubled braces (`{{` -> `{`, `}}` -> `}`) in `value`.
    ///
    /// # Errors
    /// Returns `ParseError` if the input is not valid.
    pub fn escape_double_curly_braces(value: &str) -> Result<String, ParseError> {
        let test = text_including_escaped_brackets
            .parse(value)
            .map_err(|_| ParseError {
                format_string: value.to_string(),
            })?;
        Ok(test)
    }

    /// Parse a format string into a sequence of `Value` segments.
    ///
    /// # Examples
    /// ```no_run
    /// use bumpversion::f_string::parser::parse_format_arguments;
    /// let parts = parse_format_arguments("v{major}.{minor}.{patch}")?;
    /// # Ok::<(), bumpversion::f_string::ParseError>(())
    /// ```
    pub fn parse_format_arguments(value: &str) -> Result<Vec<Value>, ParseError> {
        let test = repeat(0.., text_or_argument)
            .parse(value)
            .map_err(|_| ParseError {
                format_string: value.to_string(),
            })?;
        Ok(test)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use color_eyre::eyre;
        use similar_asserts::assert_eq as sim_assert_eq;

        #[test]
        fn parses_complex_arguments() -> eyre::Result<()> {
            crate::tests::init();

            sim_assert_eq!(
                parse_format_arguments("this is a {test} value")?,
                vec![
                    Value::String("this is a ".to_string()),
                    Value::Argument("test"),
                    Value::String(" value".to_string()),
                ]
            );

            sim_assert_eq!(
                parse_format_arguments("{jane!s}")?,
                vec![Value::Argument("jane!s")]
            );

            sim_assert_eq!(
                parse_format_arguments("Magic wand: {bag['wand']:^10}")?,
                vec![
                    Value::String("Magic wand: ".to_string()),
                    Value::Argument("bag['wand']:^10"),
                ]
            );
            Ok(())
        }

        #[test]
        fn parses_version_pattern() {
            sim_assert_eq!(
                parse_format_arguments(
                    "{major}.{minor}.{patch}.{dev}{$PR_NUMBER}.dev{distance_to_latest_tag}"
                ),
                Ok(vec![
                    Value::Argument("major"),
                    Value::String(".".to_string()),
                    Value::Argument("minor"),
                    Value::String(".".to_string()),
                    Value::Argument("patch"),
                    Value::String(".".to_string()),
                    Value::Argument("dev"),
                    Value::Argument("$PR_NUMBER"),
                    Value::String(".dev".to_string()),
                    Value::Argument("distance_to_latest_tag"),
                ])
            );
        }

        #[test]
        fn escapes_double_curly_brackets() {
            sim_assert_eq!(
                text_including_escaped_brackets.parse(" hello world"),
                Ok(" hello world".to_string())
            );

            sim_assert_eq!(
                text_including_escaped_brackets.parse(" hello {{ world }}"),
                Ok(" hello { world }".to_string())
            );

            sim_assert_eq!(
                non_escaped_bracket_argument.parse("{test}"),
                Ok(Value::Argument("test"))
            );

            sim_assert_eq!(
                repeat(1.., text_or_argument).parse("this is a {test} for parsing {arguments}"),
                Ok(vec![
                    Value::String("this is a ".to_string()),
                    Value::Argument("test"),
                    Value::String(" for parsing ".to_string()),
                    Value::Argument("arguments"),
                ])
            );

            sim_assert_eq!(
                parse_format_arguments("this }} {{ is a "),
                Ok(vec![Value::String("this } { is a ".to_string())])
            );

            sim_assert_eq!(
                non_escaped_bracket_argument.parse("{}"),
                Ok(Value::Argument(""))
            );

            sim_assert_eq!(
                text_including_escaped_brackets.parse(" hello {{ world }}"),
                Ok(" hello { world }".to_string())
            );

            sim_assert_eq!(
                parse_format_arguments("this }} {{ is a {test}"),
                Ok(vec![
                    Value::String("this } { is a ".to_string()),
                    Value::Argument("test"),
                ])
            );

            sim_assert_eq!(
                parse_format_arguments("this }} {{ is a {test} for parsing {arguments}"),
                Ok(vec![
                    Value::String("this } { is a ".to_string()),
                    Value::Argument("test"),
                    Value::String(" for parsing ".to_string()),
                    Value::Argument("arguments"),
                ])
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PythonFormatString(pub Vec<Value>);

impl std::fmt::Display for PythonFormatString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for value in &self.0 {
            write!(f, "{value}")?;
        }
        Ok(())
    }
}

impl FromIterator<Value> for PythonFormatString {
    fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl AsRef<Vec<Value>> for PythonFormatString {
    fn as_ref(&self) -> &Vec<Value> {
        &self.0
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq, PartialOrd, Hash)]
#[error("missing argument {0:?}")]
pub struct MissingArgumentError(String);

impl PythonFormatString {
    pub fn parse(value: &str) -> Result<Self, parser::ParseError> {
        let arguments = parser::parse_format_arguments(value)?;
        Ok(Self(arguments.into_iter().map(Into::into).collect()))
    }

    pub fn format<K, V>(
        &self,
        values: &HashMap<K, V>,
        strict: bool,
    ) -> Result<String, MissingArgumentError>
    where
        K: std::borrow::Borrow<str>,
        K: std::hash::Hash + Eq,
        V: AsRef<str>,
    {
        self.0.iter().try_fold(String::new(), |mut acc, value| {
            let value = match value {
                Value::Argument(arg) => {
                    let as_timestamp = || {
                        // try to parse as timestamp of format "utcnow:%Y-%m-%dT%H:%M:%SZ"
                        arg.split_once(':').and_then(|(arg, format)| {
                            values.get(arg).and_then(|value| {
                                let timestamp =
                                    chrono::DateTime::parse_from_rfc3339(value.as_ref()).ok()?;
                                Some(timestamp.format(format).to_string())
                            })
                        })
                    };
                    let value = values
                        .get(arg)
                        .map(|value| value.as_ref().to_string())
                        .or_else(as_timestamp);

                    match value {
                        Some(value) => Ok(value),
                        None if strict => Err(MissingArgumentError(arg.to_string())),
                        None => Ok(String::new()),
                    }
                }
                Value::String(s) => Ok(s.clone()),
            }?;
            acc.push_str(&value);
            Ok(acc)
        })
    }

    pub fn named_arguments(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|value| value.as_argument())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Value> {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{PythonFormatString, Value};
    use color_eyre::eyre;
    use similar_asserts::assert_eq as sim_assert_eq;
    use std::collections::HashMap;

    #[test]
    fn parse_f_string_simple() -> eyre::Result<()> {
        crate::tests::init();
        let fstring = PythonFormatString::parse("this is a formatted {value}!")?;
        sim_assert_eq!(
            fstring.as_ref().as_slice(),
            [
                Value::String("this is a formatted ".to_string()),
                Value::Argument("value".to_string()),
                Value::String("!".to_string()),
            ]
        );

        let strict = true;
        sim_assert_eq!(
            fstring
                .format(
                    &[("value", "text"), ("other", "not used")]
                        .into_iter()
                        .collect::<HashMap<&str, &str>>(),
                    strict
                )
                .as_deref(),
            Ok("this is a formatted text!")
        );
        Ok(())
    }

    #[test]
    fn parse_f_string_iter() -> eyre::Result<()> {
        crate::tests::init();
        let fstring = PythonFormatString::parse("this is a formatted {value}!")?;
        sim_assert_eq!(
            fstring.iter().collect::<Vec<_>>(),
            vec![
                &Value::String("this is a formatted ".to_string()),
                &Value::Argument("value".to_string()),
                &Value::String("!".to_string()),
            ]
        );
        Ok(())
    }

    #[test]
    fn parse_f_string_with_dollar_sign_argument() -> eyre::Result<()> {
        crate::tests::init();
        let fstring = PythonFormatString::parse("this is a formatted {$value1}, and {another1}!")?;
        sim_assert_eq!(
            fstring.as_ref().as_slice(),
            [
                Value::String("this is a formatted ".to_string()),
                Value::Argument("$value1".to_string()),
                Value::String(", and ".to_string()),
                Value::Argument("another1".to_string()),
                Value::String("!".to_string()),
            ]
        );

        let strict = true;
        sim_assert_eq!(
            fstring
                .format(
                    &[
                        ("$value1", "text"),
                        ("another1", "more"),
                        ("other", "unused")
                    ]
                    .into_iter()
                    .collect::<HashMap<&str, &str>>(),
                    strict
                )
                .as_deref(),
            Ok("this is a formatted text, and more!")
        );
        Ok(())
    }

    #[test]
    fn parse_f_string_with_missing_argument() -> eyre::Result<()> {
        crate::tests::init();
        let fstring = PythonFormatString::parse("this is a formatted {$value1}, and {another1}!")?;
        sim_assert_eq!(
            fstring.as_ref().as_slice(),
            [
                Value::String("this is a formatted ".to_string()),
                Value::Argument("$value1".to_string()),
                Value::String(", and ".to_string()),
                Value::Argument("another1".to_string()),
                Value::String("!".to_string()),
            ]
        );

        let strict = false;
        sim_assert_eq!(
            fstring
                .format(
                    &[
                        // ("$value1", "text"), // missing
                        ("another1", "more"),
                        ("other", "unused")
                    ]
                    .into_iter()
                    .collect::<HashMap<&str, &str>>(),
                    strict
                )
                .as_deref(),
            Ok("this is a formatted , and more!")
        );
        Ok(())
    }

    #[test]
    fn parse_f_string_with_missing_argument_strict() -> eyre::Result<()> {
        crate::tests::init();
        let fstring = PythonFormatString::parse("this is a formatted {$value1}, and {another1}!")?;
        sim_assert_eq!(
            fstring.as_ref().as_slice(),
            [
                Value::String("this is a formatted ".to_string()),
                Value::Argument("$value1".to_string()),
                Value::String(", and ".to_string()),
                Value::Argument("another1".to_string()),
                Value::String("!".to_string()),
            ]
        );

        let strict = true;
        sim_assert_eq!(
            fstring.format(
                &[
                    // ("$value1", "text"), // missing
                    ("another1", "more"),
                    ("other", "unused")
                ]
                .into_iter()
                .collect::<HashMap<&str, &str>>(),
                strict
            ),
            Err(super::MissingArgumentError("$value1".to_string())),
        );
        Ok(())
    }

    #[test]
    fn f_string_display() -> eyre::Result<()> {
        crate::tests::init();
        let raw_fstring = "this is a formatted {$value1}, and {another1}!";
        let fstring = PythonFormatString::parse(raw_fstring)?;
        sim_assert_eq!(&fstring.to_string(), raw_fstring);
        Ok(())
    }
}
