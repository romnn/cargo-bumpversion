use color_eyre::eyre;
use python_fstring::parser_winnow::{parse_format_arguments, Error, Value};
use std::collections::HashMap;

// pub type Error = rustpython_parser::ParseError;
// pub type Error = rustpython_parser::ParseError;

#[derive(Debug)]
pub struct PythonFormatString<'a>(Vec<Value<'a>>);
// pub struct PythonFormatString(rustpython_parser::ast::ExprJoinedStr);

impl<'a> TryFrom<&'a str> for PythonFormatString<'a> {
    type Error = Error;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        // let double_quotes = format!(r#"f"{value}""#);
        // let single_quotes = format!(r#"f'{value}'"#);
        // Self::parse(&double_quotes).or(Self::parse(&single_quotes))
        let arguments = parse_format_arguments(value)?;
        Ok(Self(arguments))
    }
}

impl<'a> PythonFormatString<'a> {
    // pub fn parse(value: &'a str) -> Result<Self, Error> {
    //     use rustpython_parser::{ast, Parse};
    //     let parsed = ast::ExprJoinedStr::parse(value, "")?;
    //     Ok(Self(parsed))
    // }

    pub fn format<K, V>(&self, values: &HashMap<K, V>, strict: bool) -> eyre::Result<String>
    where
        K: std::borrow::Borrow<str>,
        K: std::hash::Hash + Eq,
        V: AsRef<str>,
    {
        self.0.iter().try_fold(String::new(), |mut acc, value| {
            let value = match value {
                Value::Argument(arg) => values.get(*arg).map(|s| s.as_ref()).unwrap_or_default(),
                Value::String(s) => s.as_str(),
                _ => "",
            };
            acc.push_str(value);
            Ok(acc)
        })
    }

    pub fn named_arguments(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|value| value.as_argument())
    }
}
