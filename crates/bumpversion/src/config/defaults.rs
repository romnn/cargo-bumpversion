// use crate::f_string::{PythonFormatString, Value};
// use once_cell::sync::Lazy;
// use regex::{Regex, RegexBuilder};

// const PARSE_VERSION_PATTERN: &str = r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)";
// pub static PARSE_VERSION_REGEX: Lazy<Regex> =
//     Lazy::new(|| RegexBuilder::new(PARSE_VERSION_PATTERN).build().unwrap());
//
// pub static SERIALIZE_VERSION_PATTERNS: Lazy<Vec<PythonFormatString>> = Lazy::new(|| {
//     vec![PythonFormatString(vec![
//         Value::Argument("major".to_string()),
//         Value::String(".".to_string()),
//         Value::Argument("minor".to_string()),
//         Value::String(".".to_string()),
//         Value::Argument("patch".to_string()),
//     ])]
// });
//
// pub const SEARCH: Lazy<super::regex::RegexTemplate> = Lazy::new(|| {
//     super::regex::RegexTemplate::Escaped(
//         [Value::Argument("current_version".to_string())]
//             .into_iter()
//             .collect(),
//     )
// });
// pub const REPLACE: &str = "{new_version}";

// pub static TAG_NAME: Lazy<PythonFormatString> = Lazy::new(|| {
//     [
//         Value::String(String::from("v")),
//         Value::Argument("new_version".to_string()),
//     ]
//     .into_iter()
//     .collect()
// });
//
// pub static TAG_MESSAGE: Lazy<PythonFormatString> = Lazy::new(|| {
//     PythonFormatString(vec![
//         Value::String("Bump version: ".to_string()),
//         Value::Argument("current_version".to_string()),
//         Value::String(" → ".to_string()),
//         Value::Argument("new_version".to_string()),
//     ])
// });

// pub const IGNORE_MISSING_VERSION: bool = false;
// pub const IGNORE_MISSING_FILES: bool = false;
// pub const CREATE_TAG: bool = false;
// pub const SIGN_TAGS: bool = false;
// pub const ALLOW_DIRTY: bool = false;
// pub const COMMIT: bool = false;
//
// pub static COMMIT_MESSAGE: Lazy<PythonFormatString> = Lazy::new(|| {
//     PythonFormatString(vec![
//         Value::String("Bump version: ".to_string()),
//         Value::Argument("current_version".to_string()),
//         Value::String(" → ".to_string()),
//         Value::Argument("new_version".to_string()),
//     ])
// });
