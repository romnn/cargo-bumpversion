// use configparser::ini::Ini;
// use derive_builder::Builder;
use color_eyre::eyre;
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid integer: {0}")]
    BadInt(#[from] std::num::TryFromIntError),

    #[error("{0}")]
    Unknown(String),
}

// const CONFIG_FILE_SEARCH_ORDER: &[&str; 4] = &[
//     ".bumpversion.cfg",
//     ".bumpversion.toml",
//     "setup.cfg",
//     "pyproject.toml",
// ];

// /// Check if config file is valid
// pub fn is_valid(file: Option<Path>) -> true {
// }

// pub enum ConfigFile<'a> {
// pub enum ConfigFile {
//     BumpversionToml(&'a Path),
//     SetupCfg(&'a Path),
//     PyProject(&'a Path),
//     CargoToml(&'a Path),
// }

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigFile {
    BumpversionToml(PathBuf),
    SetupCfg(PathBuf),
    PyProject(PathBuf),
    CargoToml(PathBuf),
}

impl ConfigFile {
    pub fn path(&self) -> &Path {
        match self {
            Self::BumpversionToml(path) => path.as_ref(),
            Self::SetupCfg(path) => path.as_ref(),
            Self::PyProject(path) => path.as_ref(),
            Self::CargoToml(path) => path.as_ref(),
        }
    }
}

pub fn config_file_locations(dir: &Path) -> impl Iterator<Item = ConfigFile> + use<'_> {
    [
        ConfigFile::BumpversionToml(dir.join(".bumpversion.toml")),
        ConfigFile::BumpversionToml(dir.join(".bumpversion.cfg")),
        ConfigFile::PyProject(dir.join("pyproject.toml")),
        ConfigFile::SetupCfg(dir.join("setup.cfg")),
    ]
    .into_iter()
}

impl From<String> for ParseError {
    fn from(message: String) -> Self {
        Self::Unknown(message)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse config file: {0}")]
    Parse(#[from] ParseError),
}

fn deserialize_python_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    // let s: &str = serde::de::Deserialize::deserialize(deserializer)?;
    let s: Option<String> = serde::de::Deserialize::deserialize(deserializer).ok();
    let Some(s) = s else {
        return Ok(None);
    };
    match s.as_str() {
        "" => Ok(None),
        "True" | "true" => Ok(Some(true)),
        "False" | "false" => Ok(Some(false)),
        _ => Err(serde::de::Error::unknown_variant(
            &s,
            &["True", "true", "False", "false"],
        )),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
pub struct Config {
    /// don't abort if working directory is dirty
    allow_dirty: Option<bool>,
    /// version that needs to be updated
    current_version: Option<String>,
    /// regex parsing the version string
    parse: Option<String>,
    /// how to serialize back to a version
    serialize: Option<String>,
    /// template for complete string to search
    search: Option<String>,
    /// template for complete string to replace
    replace: Option<String>,
    /// treat the search parameter as a regular expression
    regex: Option<bool>,
    /// only replace the version in files specified on the command line, ignoring the files from the configuration file
    no_configured_files: Option<bool>,
    /// ignore any missing files when searching and replacing in files
    ignore_missing_files: Option<bool>,
    /// ignore any missing version when searching and replacing in files
    ignore_missing_version: Option<bool>,
    /// don't write any files, just pretend
    dry_run: Option<bool>,
    /// commit to version control
    #[serde(deserialize_with = "deserialize_python_bool", default)]
    commit: Option<bool>,
    /// create a tag in version control
    #[serde(deserialize_with = "deserialize_python_bool", default)]
    tag: Option<bool>,
    /// sign tags if created
    sign_tag: Option<bool>,
    /// tag name (only works with --tag)
    tag_name: Option<String>,
    /// tag message
    tag_message: Option<String>,
    /// commit message
    #[serde(rename = "message")]
    commit_message: Option<String>,
    /// extra arguments to commit command
    commit_args: Option<String>,
}

pub mod toml {
    use color_eyre::eyre;

    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    pub struct BumpversionTomlFileConfig {
        pub filename: String,
        #[serde(flatten)]
        pub config: super::Config,
    }

    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    pub struct BumpversionTomlTool {
        pub files: Vec<BumpversionTomlFileConfig>,
    }

    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    pub struct SetupCfgTomlTools {
        pub bumpversion: BumpversionTomlTool,
    }

    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    pub struct SetupCfgToml {
        pub tool: SetupCfgTomlTools,
        // bumpversion: Option<Config>,
    }

    pub type PyProjectToml = SetupCfgToml;

    impl SetupCfgToml {
        pub fn from_str(config: &str) -> eyre::Result<Self> {
            let config: SetupCfgToml = toml::from_str(&config)?;
            Ok(config)
        }
    }

    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    pub struct CargoToml {
        pub tool: SetupCfgTomlTools,
        // bumpversion: Option<Config>,
    }

    impl CargoToml {
        pub fn from_str(config: &str) -> eyre::Result<Self> {
            let config: Self = toml::from_str(&config)?;
            Ok(config)
        }
    }
}

pub mod ini {
    use color_eyre::eyre;
    use indexmap::IndexMap;

    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    pub struct SetupCfgINI {
        pub bumpversion: Option<super::Config>,
        #[serde(flatten)]
        pub per_target: IndexMap<String, super::Config>,
        // #[serde(with = "tuple_vec_map", default)]
        // per_target: Vec<(String, Config)>,
    }

    impl SetupCfgINI {
        fn filter(&mut self) {
            self.per_target
                .retain(|key, value| key.starts_with("bumpversion:"));

            // self.per_target = self
            //     .per_target
            //     .filter(|key, value| key.starts_with("bumpversion:"))
            //     .map(|key, value| (key, value))
            //     .collect();
            // for (mut key, value) in self.per_target.iter_mut() {
            //     if let Some(new_key) = key.strip_prefix("bumpversion:") {
            //         *key = new_key.to_string()
            //     }
            // }
        }

        pub fn from_reader(reader: impl std::io::BufRead) -> eyre::Result<Self> {
            let reader = std::io::BufReader::new(reader);
            let mut config: SetupCfgINI = serde_ini::from_read(reader)?;
            config.filter();
            // filter
            Ok(config)
        }

        pub fn from_str(config: &str) -> eyre::Result<Self> {
            let mut config: SetupCfgINI = serde_ini::from_str(&config)?;
            config.filter();
            Ok(config)
        }
    }
    //     def boolify(s: str) -> bool:
    //     """Convert a string to a boolean."""
    //     if s in {"True", "true"}:
    //         return True
    //     if s in {"False", "false"}:
    //         return False
    //     raise ValueError("Not Boolean Value!")
    //
    //
    // def noneify(s: str) -> None:
    //     """Convert a string to None."""
    //     if s == "None":
    //         return None
    //     raise ValueError("Not None Value!")
    //
    //
    // def listify(s: str) -> list:
    //     """
    //     Convert a string representation of a list into list of homogeneous basic types.
    //
    //     Type of elements in list is determined via first element. Successive elements are
    //     cast to that type.
    //
    //     Args:
    //         s: String representation of a list.
    //
    //     Raises:
    //         ValueError: If string does not represent a list.
    //         TypeError: If string does not represent a list of homogeneous basic types.
    //
    //     Returns:
    //         List of homogeneous basic types.
    //     """
    //     if "\n" in s:
    //         str_list = s.strip().split("\n")
    //     elif "," in s:
    //         str_list = s.strip().split(",")
    //     else:
    //         raise ValueError("Not a List")
    //
    //     # derive the type of the variable
    //     element_caster = str
    //     for caster in (boolify, int, float, noneify, element_caster):
    //         with contextlib.suppress(ValueError):
    //             caster(str_list[0])  # type: ignore[operator]
    //             element_caster = caster  # type: ignore[assignment]
    //             break
    //     # cast all elements
    //     try:
    //         return [element_caster(x) for x in str_list]
    //     except ValueError as e:
    //         raise TypeError("Autocasted list must be all same type") from e
    //
    //
    // def autocast_value(var: Any) -> Any:
    //     """
    //     Guess the string representation of the variable's type.
    //
    //     Args:
    //         var: Value to autocast.
    //
    //     Returns:
    //         The autocasted value.
    //     """
    //     if not isinstance(var, str):  # don't need to guess non-string types
    //         return var
    //
    //     # guess string representation of var
    //     for caster in (boolify, int, float, noneify, listify):
    //         with contextlib.suppress(ValueError):
    //             return caster(var)  # type: ignore[operator]
    //
    //     return var
}

// impl Default for Config {
//     fn default() -> Self {
//         Self {
//             // verbosity: 0,
//             // list: false,
//             allow_dirty: false,
//             current_version: None,
//             parse: r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)".into(),
//             serialize: "{major}.{minor}.{patch}".into(),
//             search: "{current_version}".into(),
//             replace: "{new_version}".into(),
//             // no_configured_files: false,
//             dry_run: false,
//             commit: false,
//             no_commit: true,
//             tag: false,
//             no_tag: true,
//             sign_tag: false,
//             no_sign_tag: true,
//             tag_name: "v{new_version}".into(),
//             tag_message: "bump: {current_version} → {new_version}".into(),
//             commit_message: "bump: {current_version} → {new_version}".into(),
//             commit_args: None,
//         }
//     }
// }

// impl Config {
//     pub fn from_ini<S: Into<String>>(content: S) -> Result<Self, ParseError> {
//         let mut config = Self::default();
//         config.load_ini(content)?;
//         Ok(config)
//     }
//
//     pub fn load_ini<S: Into<String>>(&mut self, content: S) -> Result<(), ParseError> {
//         use std::cmp::max;
//
//         // ident, block, stmt, expr, pat, ty, lifetime, literal, path, meta, tt, item, vis
//         macro_rules! get {
//             ($config:ident, $getter:ident, $component:literal, $field:ident) => {
//                 if let Some(val) = $config.$getter("bumpversion", stringify!($field))? {
//                     self.$field = val;
//                 }
//             };
//         }
//
//         let mut config = Ini::new();
//         let _ = config.read(content.into()).unwrap();
//
//         if let Some(verbosity) = config.getuint("bumpversion", "verbosity")? {
//             self.verbosity = max(self.verbosity, verbosity.try_into()?);
//         }
//         // get!(config, getbool, "bumpversion", list);
//         // get!(config, getbool, "bumpversion", allow_dirty);
//         // get!(config, get, "bumpversion", current_version);
//         // get!(config, get, "bumpversion", parse);
//         // get!(config, get, "bumpversion", serialize);
//         // get!(config, getbool, "bumpversion", dry_run);
//         // get!(config, getbool, "bumpversion", commit);
//         // get!(config, getbool, "bumpversion", no_commit);
//         // get!(config, getbool, "bumpversion", tag);
//         // get!(config, getbool, "bumpversion", no_tag);
//         // get!(config, getbool, "bumpversion", sign_tag);
//         // get!(config, getbool, "bumpversion", no_sign_tag);
//         // get!(config, get, "bumpversion", tag_name);
//         // get!(config, get, "bumpversion", tag_message);
//         // get!(config, get, "bumpversion", commit_message);
//
//         // verbosity
//         // list
//         // allow_dirty
//         // current_version
//         // parse
//         // serialize
//         // dry_run
//         // commit
//         // no_commit
//         // tag
//         // no_tag
//         // sign_tag
//         // no_sign_tag
//         // tag_name
//         // tag_message
//         // commit_message
//         // }
//         for section in config.sections() {
//             println!("section: {}", section);
//             // search
//             // replace
//             // parse
//             // serialize
//         }
//
//         // config
//         //     .load_ini(&mut config_file)
//         //     .map_err(|message| Error::ConfigFile { message })?;
//         // .map_err(?;
//         // println!("config: {:?}", config);
//         // if let Some(main_section) = config_file.get("bumpversion") {
//         // println!("bumpversion: {:?}", main_section);
//         Ok(())
//     }
//
//     pub fn from_reader<R: io::Read + io::Seek>(mut reader: R) -> Result<Self, ParseError> {
//         let mut buffered = io::BufReader::new(reader);
//         let mut buf = String::new();
//         buffered.read_to_string(&mut buf)?;
//         Self::from_ini(buf)
//     }
//
//     pub fn open<P: Into<PathBuf>>(path: Option<P>) -> Result<Option<Self>, Error> {
//         let path = match path {
//             Some(p) => Some(p.into()),
//             None => {
//                 let cwd = std::env::current_dir()?;
//                 let bumpversion_cfg = cwd.join(".bumpversion.cfg");
//                 let setup_cfg = cwd.join("setup.cfg");
//                 if bumpversion_cfg.is_file() {
//                     Some(bumpversion_cfg)
//                 } else if setup_cfg.is_file() {
//                     Some(setup_cfg)
//                 } else {
//                     None
//                 }
//             }
//         };
//         match path {
//             Some(path) => {
//                 let config = Self::from_reader(fs::File::open(path)?)?;
//                 Ok(Some(config))
//             }
//             None => Ok(None),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use std::io::Read;

    #[test]
    fn parse_python_pyproject_toml() -> eyre::Result<()> {
        use super::toml::{
            BumpversionTomlFileConfig, BumpversionTomlTool, SetupCfgToml, SetupCfgTomlTools,
        };
        use super::Config;

        let pyproject_toml = r#"
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
"#;

        let config = SetupCfgToml::from_str(pyproject_toml)?;
        println!("config: {:#?}", config);

        let expected = SetupCfgToml {
            tool: SetupCfgTomlTools {
                bumpversion: BumpversionTomlTool {
                    files: vec![BumpversionTomlFileConfig {
                        filename: "config.ini".to_string(),
                        config: Config {
                            search: Some("[myproject]\nversion={current_version}".to_string()),
                            replace: Some("[myproject]\nversion={new_version}".to_string()),
                            ..Config::default()
                        },
                    }],
                },
            },
        };

        similar_asserts::assert_eq!(config, expected);
        Ok(())
    }

    #[test]
    fn parse_python_setup_cfg() -> eyre::Result<()> {
        use super::ini::SetupCfgINI;
        use super::Config;

        // note: in ini files, there are fewer conventions compared to TOML
        // for example, we can write 0.1.8 without quotes, just as treat "True" as boolean true
        let setup_cfg_ini = r#"
[bumpversion]
current_version = 0.1.8
commit = True
tag = True
message = DO NOT BUMP VERSIONS WITH THIS FILE

[bumpversion:glob:*.txt]
[bumpversion:glob:**/*.txt]

[bumpversion:file:setup.py]
search = version = "{current_version}"
replace = version = "{new_version}"

[bumpversion:file:favico/__init__.py]
search = __version__ = "{current_version}"
replace = __version__ = "{new_version}"

[bumpversion:file_with_dotted_version:file1]
search = dots: {current_version}
replace = dots: {new_version}

[bumpversion:file_with_dotted_version:file2]
search = dashes: {current_version}
replace = dashes: {new_version}
parse = (?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)
serialize = {major}-{minor}-{patch}

[bdist_wheel]
universal = 1

[metadata]
description-file = README.rst

[flake8]
exclude = docs
ignore = E203, E266, E501, W503
max-line-length = 88
max-complexity = 18
select = B,C,E,F,W,T4
        "#;

        let config = SetupCfgINI::from_str(setup_cfg_ini)?;
        println!("config: {:#?}", config);

        let expected = SetupCfgINI {
            bumpversion: Some(Config {
                current_version: Some("0.1.8".to_string()),
                commit: Some(true),
                tag: Some(true),
                commit_message: Some("DO NOT BUMP VERSIONS WITH THIS FILE".to_string()),
                ..Config::default()
            }),
            per_target: IndexMap::from_iter([
                ("bumpversion:glob:*.txt".to_string(), Config::default()),
                ("bumpversion:glob:**/*.txt".to_string(), Config::default()),
                (
                    "bumpversion:file:setup.py".to_string(),
                    Config {
                        search: Some(r#"version = "{current_version}""#.to_string()),
                        replace: Some(r#"version = "{new_version}""#.to_string()),
                        ..Config::default()
                    },
                ),
                (
                    "bumpversion:file:favico/__init__.py".to_string(),
                    Config {
                        search: Some(r#"__version__ = "{current_version}""#.to_string()),
                        replace: Some(r#"__version__ = "{new_version}""#.to_string()),
                        ..Config::default()
                    },
                ),
                (
                    "bumpversion:file_with_dotted_version:file1".to_string(),
                    Config {
                        search: Some("dots: {current_version}".to_string()),
                        replace: Some("dots: {new_version}".to_string()),
                        ..Config::default()
                    },
                ),
                (
                    "bumpversion:file_with_dotted_version:file2".to_string(),
                    Config {
                        search: Some("dashes: {current_version}".to_string()),
                        replace: Some("dashes: {new_version}".to_string()),
                        parse: Some(r"(?P<major>\d+)-(?P<minor>\d+)-(?P<patch>\d+)".to_string()),
                        serialize: Some("{major}-{minor}-{patch}".to_string()),
                        ..Config::default()
                    },
                ),
            ]),
        };

        similar_asserts::assert_eq!(config, expected);
        Ok(())
    }
}
