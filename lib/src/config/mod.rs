pub mod ini;
pub mod pyproject_toml;
// pub mod toml;

use crate::diagnostics::{DiagnosticExt, FileId, Printer, Span, Spanned};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use color_eyre::eyre;
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid integer: {0}")]
    BadInt(#[from] std::num::TryFromIntError),

    #[error("{0}")]
    Unknown(String),
}

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
pub struct FileConfig {
    /// don't abort if working directory is dirty
    allow_dirty: Option<bool>,
    /// version that needs to be updated
    current_version: Option<String>,
    /// regex parsing the version string
    parse: Option<String>,
    /// how to serialize back to a version
    serialize: Vec<String>,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
pub struct PartConfig {
    optional_value: Option<String>,
    values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    pub global: FileConfig,
    // pub files: IndexMap<PathBuf, FileConfig>,
    pub files: Vec<(PathBuf, FileConfig)>,
    pub parts: IndexMap<String, PartConfig>,
    // pub parts: Vec<String, PartConfig>,
}

impl Config {
    pub fn parse(
        path: impl AsRef<Path>,
        printer: &Printer,
        strict: bool,
        diagnostics: &mut Vec<Diagnostic<FileId>>,
    ) -> eyre::Result<Option<Self>> {
        let path = path.as_ref();
        let config = std::fs::read_to_string(path)?;
        let file_id = printer.add_source_file(path.to_string_lossy().to_string(), config.clone());

        match path.extension().and_then(|ext| ext.to_str()) {
            Some("cfg") => {
                tracing::warn!("the .cfg file format is deprecated. Please use .toml instead");
                let options = serde_ini_spanned::value::Options {
                    strict,
                    ..serde_ini_spanned::value::Options::default()
                };
                Self::from_ini(&config, &options, file_id, strict, diagnostics).map_err(Into::into)
            }
            None | Some("toml") => {
                Self::from_pyproject_toml(&config, file_id, strict, diagnostics).map_err(Into::into)
            }
            Some(other) => {
                eyre::bail!("unkown config file format: {other:?}");
            }
        }
    }
}

// [tool.bumpversion]
// current_version = "0.28.1"
// commit = true
// commit_args = "--no-verify"
// tag = true
// tag_name = "{new_version}"
// allow_dirty = true
// parse = "(?P<major>\\d+)\\.(?P<minor>\\d+)\\.(?P<patch>\\d+)(\\.(?P<dev>post)\\d+\\.dev\\d+)?"
// serialize = [
//     "{major}.{minor}.{patch}.{dev}{$PR_NUMBER}.dev{distance_to_latest_tag}",
//     "{major}.{minor}.{patch}"
// ]
// message = "Version updated from {current_version} to {new_version}"
//
// [tool.bumpversion.parts.dev]
// values = ["release", "post"]
//
// [[tool.bumpversion.files]]
// filename = "bumpversion/__init__.py"
//
// [[tool.bumpversion.files]]
// filename = "CHANGELOG.md"
// search = "Unreleased"
//
// [[tool.bumpversion.files]]
// filename = "CHANGELOG.md"
// search = "{current_version}...HEAD"
// replace = "{current_version}...{new_version}"
//
// [[tool.bumpversion.files]]
// filename = "action.yml"
// search = "bump-my-version=={current_version}"
// replace = "bump-my-version=={new_version}"
//
// [[tool.bumpversion.files]]
// filename = "Dockerfile"
// search = "created=\\d{{4}}-\\d{{2}}-\\d{{2}}T\\d{{2}}:\\d{{2}}:\\d{{2}}Z"
// replace = "created={utcnow:%Y-%m-%dT%H:%M:%SZ}"
// regex = true
//
// [[tool.bumpversion.files]]
// filename = "Dockerfile"

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
