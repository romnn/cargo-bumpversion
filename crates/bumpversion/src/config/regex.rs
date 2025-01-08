use crate::f_string::{MissingArgumentError, PythonFormatString};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Regex(pub regex::Regex);

impl std::ops::Deref for Regex {
    type Target = regex::Regex;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> TryFrom<&'a str> for Regex {
    type Error = regex::Error;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        regex::RegexBuilder::new(value).build().map(Self)
    }
}

impl std::fmt::Display for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Ord for Regex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        std::cmp::Ord::cmp(self.0.as_str(), other.0.as_str())
    }
}

impl PartialOrd for Regex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(std::cmp::Ord::cmp(&self, &other))
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(self.0.as_str(), other.0.as_str())
    }
}

impl Eq for Regex {}

impl std::hash::Hash for Regex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl From<regex::Regex> for Regex {
    fn from(value: regex::Regex) -> Self {
        Self(value)
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum RegexTemplateError {
    #[error(transparent)]
    MissingArgument(#[from] MissingArgumentError),
    #[error(transparent)]
    Regex(#[from] regex::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegexTemplate {
    Regex(PythonFormatString),
    Escaped(PythonFormatString),
}

impl std::fmt::Display for RegexTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref().to_string())
    }
}

impl AsRef<PythonFormatString> for RegexTemplate {
    fn as_ref(&self) -> &PythonFormatString {
        match self {
            Self::Regex(s) | Self::Escaped(s) => &s,
        }
    }
}

impl RegexTemplate {
    #[must_use]
    pub fn is_regex(&self) -> bool {
        matches!(self, Self::Regex(_))
    }

    #[must_use]
    pub fn is_escaped(&self) -> bool {
        matches!(self, Self::Escaped(_))
    }

    pub fn format<K, V>(
        &self,
        values: &HashMap<K, V>,
        strict: bool,
    ) -> Result<regex::Regex, RegexTemplateError>
    where
        K: std::borrow::Borrow<str>,
        K: std::hash::Hash + Eq,
        V: AsRef<str>,
    {
        let raw_pattern = match self {
            Self::Regex(format_string) => {
                let escaped_values: HashMap<&str, String> = values
                    .iter()
                    .map(|(k, v)| (k.borrow(), regex::escape(v.as_ref())))
                    .collect();

                format_string.format(&escaped_values, strict)?
            }
            Self::Escaped(format_string) => {
                let raw_pattern = format_string.format(values, strict)?;
                regex::escape(&raw_pattern)
            }
        };
        let pattern = regex::RegexBuilder::new(&raw_pattern)
            .multi_line(true)
            .build()?;
        Ok(pattern)
    }
}
