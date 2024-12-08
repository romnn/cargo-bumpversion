use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::hash::Hash;

pub trait Deser {
    type Error;

    // fn parse<S: AsRef<str>>(&self, version: S) -> Result<SemVer, Self::Error>;
    fn parse(&self, version: impl AsRef<str>) -> Result<SemVer, Self::Error>;
    fn serialize(&self, version: impl Borrow<SemVer>) -> Result<String, Self::Error>;
}

// pub mod traits {
//     use std::borrow::Borrow;
//
//     pub trait VersionParser {
//         type Error;
//         type Version;
//
//         fn parse<S: AsRef<str>>(&self, version: S) -> Result<Self::Version, Self::Error>;
//         fn serialize<V: Borrow<Self::Version>>(&self, version: V) -> Result<String, Self::Error>;
//     }
//
//     pub trait Version {
//         type Error;
//     }
// }

#[derive(thiserror::Error, Debug)]
pub enum NamedParameterError {
    #[error("failed to build named parameter regex for `{parameter}`: {source}")]
    Regex {
        parameter: String,
        source: regex::Error,
    },
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("version must have format `{format}`, found `{found}`")]
    BadFormat { format: String, found: String },

    #[error("missing {0} version component")]
    MissingComponent(String),

    #[error("failed to parse version component {component}: {source}")]
    ParseComponent {
        component: String,
        source: Box<dyn std::error::Error + Sync + Send + 'static>,
    },

    #[error("bad named parameter: {0}")]
    BadNamedParameter(#[from] NamedParameterError),
}

pub fn name_parameter<'a, F, P, V>(
    format: &'a F,
    param: P,
    value: V,
) -> Result<Cow<'a, str>, NamedParameterError>
where
    F: AsRef<str>,
    P: AsRef<str>,
    V: std::string::ToString,
{
    let re = [r"\{\s*", param.as_ref(), r"\s*\}"].join("");
    // println!("named regex `{}` = `{}`", param.as_ref(), &re);
    let re = regex::Regex::new(&re).map_err(|err| NamedParameterError::Regex {
        parameter: param.as_ref().to_string(),
        source: err,
    })?;
    Ok(re.replace(format.as_ref(), &value.to_string()))
}

macro_rules! parse_component {
    ($captures:expr, $component:expr) => {
        $captures
            .name($component)
            .ok_or(Error::MissingComponent($component.into()))?
            .as_str()
            .parse()
            .map_err(|err| Error::ParseComponent {
                component: $component.into(),
                source: Box::new(err),
            })
    };
}

macro_rules! named_format {
    ($fmt:expr, $($field:tt = $value:expr),* $(,)?) => {{
        let mut res: Result<Cow<'_, str>, NamedParameterError> = Ok($fmt.into());
        $(
            // dbg!($field, $value);
            match res.as_mut() {
                Ok(fmt) => {
                    match name_parameter(fmt, $field, $value) {
                        // unmodified: skip copy
                        Ok(Cow::Borrowed(old)) => {},
                        // modified: make a copy
                        Ok(Cow::Owned(new)) => {
                            *fmt = new.into()
                        },
                        Err(err) => {
                            res = Err(err);
                        },
                    }
                }
                Err(e) => {},
            }
        )*
        res
    }}
}

// #[derive(Debug, Default, PartialEq)]
// struct Version<K, V>
// where
//     K: Eq + Hash,
//     V: PartialEq,
// {
//     inner: HashMap<K, V>,
// }
//
// impl<K, V> Version<K, V>
// where
//     K: Eq + Hash,
//     V: PartialEq,
// {
//     pub fn new<IK, IV>(inner: HashMap<IK, IV>) -> Self
//     where
//         IK: Into<K>,
//         IV: Into<V>,
//     {
//         inner.into()
//     }
// }
// impl<K, V, IK, IV> From<HashMap<IK, IV>> for Version<K, V>
// where
//     K: Eq + Hash,
//     V: PartialEq,
//     IK: Into<K>,
//     IV: Into<V>,
// {
//     fn from(inner: HashMap<IK, IV>) -> Self {
//         let inner = inner.into_iter().map(|(k, v)| (k.into(), v.into()));
//         let inner = HashMap::from_iter(inner);
//         Self { inner }
//     }
// }
//
// impl<K, V> traits::Version for Version<K, V>
// where
//     K: Eq + Hash,
//     V: PartialEq,
// {
//     type Error = Error;
// }
//
// impl<K, V> std::ops::Deref for Version<K, V>
// where
//     K: Eq + Hash,
//     V: PartialEq,
// {
//     type Target = HashMap<K, V>;
//
//     fn deref(&self) -> &Self::Target {
//         &self.inner
//     }
// }

// #[derive(Debug)]
// struct Parser {
//     parse_regex: regex::Regex,
//     serialize_format: String,
// }
//
// impl Deser for Parser {
//     type Error = Error;
//     // type Version = Version<String, String>;
//
//     // fn parse<S: AsRef<str>>(&self, version: S) -> Result<SemVer, Self::Error> {
//     fn parse(&self, version: impl AsRef<str>) -> Result<SemVer, Self::Error> {
//         let version = version.as_ref();
//         let mut inner = HashMap::new();
//         let caps = self.parse_regex.captures(version).ok_or(Error::BadFormat {
//             format: self.parse_regex.to_string(),
//             found: version.to_string(),
//         })?;
//         for cap in self.parse_regex.capture_names() {
//             if let Some(cap) = cap {
//                 let part: String = parse_component!(caps, cap)?;
//                 inner.insert(cap.to_string(), part);
//             }
//         }
//         Ok(SemVer { inner })
//     }
//
//     // fn serialize<V: Borrow<SemVer>>(&self, version: V) -> Result<String, Self::Error> {
//     fn serialize(&self, version: impl Borrow<SemVer>) -> Result<String, Self::Error> {
//         let v = version.borrow();
//         let mut serialized = self.serialize_format.clone();
//         for (param, value) in v.iter() {
//             serialized = named_format!(&serialized, param = value)?.to_string();
//         }
//         Ok(serialized.to_string())
//     }
// }

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct SemVer {
    pub major: usize,
    pub minor: usize,
    pub patch: usize,
}

impl SemVer {
    pub fn new(major: usize, minor: usize, patch: usize) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

// impl traits::Version for SemVer {
//     type Error = Error;
// }

#[derive(Debug)]
struct SemVerParser<'a> {
    parse_regex: &'a regex::Regex,
    serialize_format: String,
}

// static test: &lazy_regex::Lazy<regex::Regex> = lazy_regex::regex!(r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)");

impl<'a> Default for SemVerParser<'a> {
    fn default() -> Self {
        let parse_regex = lazy_regex::regex!(r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)");
        // static parse_regex: &lazy_regex::Lazy<regex::Regex> = lazy_regex::regex!(r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)");
        Self {
            parse_regex: &*parse_regex,
            serialize_format: "{major}.{minor}.{patch}".to_string(),
        }
    }
}

impl<'a> Deser for SemVerParser<'a> {
    type Error = Error;

    fn parse(&self, version: impl AsRef<str>) -> Result<SemVer, Self::Error> {
        let version = version.as_ref();
        let caps = self.parse_regex.captures(version).ok_or(Error::BadFormat {
            format: self.parse_regex.to_string(),
            found: version.to_string(),
        })?;
        Ok(SemVer {
            major: parse_component!(caps, "major")?,
            minor: parse_component!(caps, "minor")?,
            patch: parse_component!(caps, "patch")?,
        })
    }

    fn serialize(&self, version: impl Borrow<SemVer>) -> Result<String, Self::Error> {
        let v = version.borrow();
        let serialized = named_format!(
            &self.serialize_format,
            "major" = v.major,
            "minor" = v.minor,
            "patch" = v.patch
        )?;
        Ok(serialized.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{Deser, SemVer, SemVerParser};
    use color_eyre::eyre;
    use std::borrow::Borrow;

    #[test]
    fn test_semver_version() -> eyre::Result<()> {
        crate::tests::init();

        let version = SemVer::new(1, 3, 2);
        let parser = SemVerParser::default();
        let serialized = parser.serialize(&version)?;
        let deserialized = parser.parse(&serialized)?;
        similar_asserts::assert_eq!(version, deserialized);
        Ok(())
    }

    #[test]
    fn test_generic_version() -> eyre::Result<()> {
        crate::tests::init();

        let version = SemVer::new(1, 3, 2);
        let parse_regex = regex::Regex::new(r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)")?;
        let parser = SemVerParser {
            parse_regex: &parse_regex,
            serialize_format: "{major}.{minor}.{patch}".into(),
        };
        let serialized = parser.serialize(&version)?;
        let deserialized = parser.parse(&serialized)?;
        similar_asserts::assert_eq!(version, deserialized);
        Ok(())
    }
}
