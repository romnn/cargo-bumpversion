use color_eyre::eyre;
pub use parser::{Error, OwnedValue, Value};
use std::collections::HashMap;

mod parser {
    use color_eyre::eyre;
    use winnow::ascii::{alpha1, alphanumeric1, digit0, digit1, escaped_transform};
    use winnow::combinator::{alt, cut_err, delimited, eof, opt, permutation, repeat, seq};
    use winnow::error::{ErrMode, ErrorKind, InputError, ParserError};
    use winnow::prelude::*;
    use winnow::stream::AsChar;
    use winnow::token::{any, none_of, one_of, take_while};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OwnedValue {
        String(String),
        Argument(String),
    }

    impl std::fmt::Display for OwnedValue {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::String(s) => write!(f, "{s}"),
                Self::Argument(arg) => write!(f, r#"{{arg}}"#),
            }
        }
    }

    impl OwnedValue {
        pub fn as_argument(&self) -> Option<&str> {
            match self {
                Self::Argument(arg) => Some(arg),
                _ => None,
            }
        }

        pub fn is_argument(&self) -> bool {
            matches!(self, Self::Argument(_))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Value<'a> {
        String(String),
        Argument(&'a str),
    }

    impl<'a> Value<'a> {
        pub fn as_argument(&self) -> Option<&str> {
            match self {
                Self::Argument(arg) => Some(arg),
                _ => None,
            }
        }

        pub fn is_argument(&self) -> bool {
            matches!(self, Self::Argument(_))
        }
    }

    impl<'a> From<Value<'a>> for OwnedValue {
        fn from(value: Value<'a>) -> Self {
            match value {
                Value::String(s) => Self::String(s.to_string()),
                Value::Argument(s) => Self::Argument(s.to_string()),
            }
        }
    }

    // impl<'a> ToOwned for Value<'a> {
    //     type Owned = OwnedValue;
    //     fn to_owned(&self) -> Self::Owned {
    //         match self {
    //             Self::String(s) => OwnedValue::String(s.to_string()),
    //             Self::Argument(s) => OwnedValue::Argument(s.to_string()),
    //         }
    //     }
    // }

    fn any_except_curly_bracket0<'a>(s: &mut &'a str) -> PResult<&'a str, InputError<&'a str>> {
        take_while(0.., |c| c != '{' && c != '}').parse_next(s)
    }

    fn any_except_curly_bracket1<'a>(s: &mut &'a str) -> PResult<&'a str, InputError<&'a str>> {
        take_while(1.., |c| c != '{' && c != '}').parse_next(s)
    }

    fn text_including_escaped_brackets<'a>(
        s: &mut &'a str,
    ) -> PResult<Value<'a>, InputError<&'a str>> {
        repeat(
            1..,
            alt((any_except_curly_bracket1, "{{".value("{"), "}}".value("}"))),
        )
        .fold(String::new, |mut string, c| {
            string.push_str(&c);
            string
        })
        .map(Value::String)
        .context("text_including_escaped_brackets")
        .parse_next(s)
    }

    fn non_escaped_bracket_argument<'a>(
        s: &mut &'a str,
    ) -> PResult<Value<'a>, InputError<&'a str>> {
        delimited("{", any_except_curly_bracket0, "}")
            .map(|inner| Value::Argument(inner))
            .context("non_escaped_bracket_argument")
            .parse_next(s)
    }

    fn text_or_argument<'a>(s: &mut &'a str) -> PResult<Value<'a>, InputError<&'a str>> {
        alt((
            text_including_escaped_brackets,
            non_escaped_bracket_argument,
        ))
        .context("text_or_argument")
        .parse_next(s)
    }

    #[derive(thiserror::Error, Debug, PartialEq, Eq)]
    #[error("invalid format: {format_string:?}")]
    pub struct Error {
        pub format_string: String,
    }

    pub fn parse_format_arguments(value: &str) -> Result<Vec<Value>, Error> {
        let test = repeat(0.., text_or_argument)
            .parse(value)
            .map_err(|_| Error {
                format_string: value.to_string(),
            })?;
        return Ok(test);
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use color_eyre::eyre;
        use similar_asserts::assert_eq as sim_assert_eq;
        use winnow::{ascii::alphanumeric1, error::ParseError, token::any};

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
                Ok(Value::String(" hello world".to_string()))
            );

            sim_assert_eq!(
                text_including_escaped_brackets.parse(" hello {{ world }}"),
                Ok(Value::String(" hello { world }".to_string()))
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
                Ok(Value::String(" hello { world }".to_string()))
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
pub struct OwnedPythonFormatString(pub Vec<parser::OwnedValue>);

impl std::fmt::Display for OwnedPythonFormatString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for value in self.0.iter() {
            write!(f, "{value}")?
            // match value {
            //     parser::OwnedValue::String(s) => write!(f, "{s}")?,
            //     parser::OwnedValue::Argument(arg) => write!(f, r#"{{arg}}"#)?,
            // }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PythonFormatString<'a>(pub Vec<parser::Value<'a>>);

impl<'a> IntoIterator for PythonFormatString<'a> {
    type Item = parser::Value<'a>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> AsRef<Vec<parser::Value<'a>>> for PythonFormatString<'a> {
    fn as_ref(&self) -> &Vec<parser::Value<'a>> {
        &self.0
    }
}

impl<'a> TryFrom<&'a str> for PythonFormatString<'a> {
    type Error = parser::Error;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let arguments = parser::parse_format_arguments(value)?;
        Ok(Self(arguments))
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq, PartialOrd, Hash)]
#[error("missing argument {0:?}")]
pub struct MissingArgumentError(String);

impl<'a> PythonFormatString<'a> {
    pub fn parse(value: &'a str) -> Result<Self, parser::Error> {
        Self::try_from(value)
    }

    pub fn iter(&'a self) -> std::slice::Iter<'a, parser::Value<'a>> {
        self.0.iter()
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
                parser::Value::Argument(arg) => match values.get(*arg).map(|s| s.as_ref()) {
                    Some(value) => Ok(value),
                    None if strict => Err(MissingArgumentError(arg.to_string())),
                    None => Ok(""),
                },
                parser::Value::String(s) => Ok(s.as_str()),
            }?;
            acc.push_str(value);
            Ok(acc)
        })
    }

    pub fn named_arguments(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|value| value.as_argument())
    }
}

impl OwnedPythonFormatString {
    pub fn parse(value: &str) -> Result<Self, parser::Error> {
        Ok(Self(
            PythonFormatString::parse(value)?
                .into_iter()
                .map(Into::into)
                .collect(),
        ))
    }

    pub fn iter(&self) -> std::slice::Iter<'_, parser::OwnedValue> {
        self.0.iter()
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
                parser::OwnedValue::Argument(arg) => match values.get(&arg).map(|s| s.as_ref()) {
                    Some(value) => Ok(value),
                    None if strict => Err(MissingArgumentError(arg.to_string())),
                    None => Ok(""),
                },
                parser::OwnedValue::String(s) => Ok(s.as_str()),
            }?;
            acc.push_str(value);
            Ok(acc)
        })
    }

    pub fn named_arguments(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|value| value.as_argument())
    }
}

#[cfg(test)]
mod tests {
    use super::{parser::Value, PythonFormatString};
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
                Value::Argument("value"),
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
                &Value::Argument("value"),
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
                Value::Argument("$value1"),
                Value::String(", and ".to_string()),
                Value::Argument("another1"),
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
                Value::Argument("$value1"),
                Value::String(", and ".to_string()),
                Value::Argument("another1"),
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
                Value::Argument("$value1"),
                Value::String(", and ".to_string()),
                Value::Argument("another1"),
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
}
