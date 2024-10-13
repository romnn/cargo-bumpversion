use configparser::ini::Ini;
use std::fs;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use derive_builder::Builder;

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("failed to read config: {0}")]
    Io(#[from] io::Error),

    #[error("invalid integer: {0}")]
    BadInt(#[from] std::num::TryFromIntError),

    #[error("{0}")]
    Unknown(String),
    // Unknown { message: String },
}

impl From<String> for ParseError {
    fn from(message: String) -> Self {
        Self::Unknown(message) //  { message }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to read config file: {0}")]
    Io(#[from] io::Error),

    #[error("failed to parse config file: {0}")]
    Parse(#[from] ParseError),
}

#[derive(Builder, Clone, Debug)]
#[builder(setter(into))]
pub struct Config {
    /// print verbose logging
    verbosity: u8,
    /// list machine readable information
    list: bool,
    /// don't abort if working directory is dirty
    allow_dirty: bool,
    /// version that needs to be updated
    current_version: Option<String>,
    /// regex parsing the version string
    parse: String,
    /// how to serialize back to a version
    serialize: String,
    /// template for complete string to search
    search: String,
    /// template for complete string to replace
    replace: String,

    /// only replace the version in files specified on the command line, ignoring the files from the configuration file
    // no_configured_files: bool,

    /// don't write any files, just pretend
    dry_run: bool,
    /// commit to version control
    commit: bool,
    /// do not commit to version control
    no_commit: bool,
    /// create a tag in version control
    tag: bool,
    /// do not create a tag in version control
    no_tag: bool,
    /// sign tags if created
    sign_tag: bool,
    /// do not sign tags if created
    no_sign_tag: bool,
    /// tag name (only works with --tag)
    tag_name: String,
    /// tag message
    tag_message: String,
    /// commit message
    commit_message: String,
    /// extra arguments to commit command
    commit_args: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            verbosity: 0,
            list: false,
            allow_dirty: false,
            current_version: None,
            parse: r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)".into(),
            serialize: "{major}.{minor}.{patch}".into(),
            search: "{current_version}".into(),
            replace: "{new_version}".into(),
            // no_configured_files: false,
            dry_run: false,
            commit: false,
            no_commit: true,
            tag: false,
            no_tag: true,
            sign_tag: false,
            no_sign_tag: true,
            tag_name: "v{new_version}".into(),
            tag_message: "bump: {current_version} → {new_version}".into(),
            commit_message: "bump: {current_version} → {new_version}".into(),
            commit_args: None,
        }
    }
}

impl Config {
    pub fn from_ini<S: Into<String>>(content: S) -> Result<Self, ParseError> {
        let mut config = Self::default();
        config.load_ini(content)?;
        Ok(config)
    }

    pub fn load_ini<S: Into<String>>(&mut self, content: S) -> Result<(), ParseError> {
        use std::cmp::max;

        // ident, block, stmt, expr, pat, ty, lifetime, literal, path, meta, tt, item, vis
        macro_rules! get {
            ($config:ident, $getter:ident, $component:literal, $field:ident) => {
                if let Some(val) = $config.$getter("bumpversion", stringify!($field))? {
                    self.$field = val;
                }
            };
        }

        let mut config = Ini::new();
        let _ = config.read(content.into()).unwrap();

        if let Some(verbosity) = config.getuint("bumpversion", "verbosity")? {
            self.verbosity = max(self.verbosity, verbosity.try_into()?);
        }
        // get!(config, getbool, "bumpversion", list);
        // get!(config, getbool, "bumpversion", allow_dirty);
        // get!(config, get, "bumpversion", current_version);
        // get!(config, get, "bumpversion", parse);
        // get!(config, get, "bumpversion", serialize);
        // get!(config, getbool, "bumpversion", dry_run);
        // get!(config, getbool, "bumpversion", commit);
        // get!(config, getbool, "bumpversion", no_commit);
        // get!(config, getbool, "bumpversion", tag);
        // get!(config, getbool, "bumpversion", no_tag);
        // get!(config, getbool, "bumpversion", sign_tag);
        // get!(config, getbool, "bumpversion", no_sign_tag);
        // get!(config, get, "bumpversion", tag_name);
        // get!(config, get, "bumpversion", tag_message);
        // get!(config, get, "bumpversion", commit_message);

        // verbosity
        // list
        // allow_dirty
        // current_version
        // parse
        // serialize
        // dry_run
        // commit
        // no_commit
        // tag
        // no_tag
        // sign_tag
        // no_sign_tag
        // tag_name
        // tag_message
        // commit_message
        // }
        for section in config.sections() {
            println!("section: {}", section);
            // search
            // replace
            // parse
            // serialize
        }

        // config
        //     .load_ini(&mut config_file)
        //     .map_err(|message| Error::ConfigFile { message })?;
        // .map_err(?;
        // println!("config: {:?}", config);
        // if let Some(main_section) = config_file.get("bumpversion") {
        // println!("bumpversion: {:?}", main_section);
        Ok(())
    }

    pub fn from_reader<R: io::Read + io::Seek>(mut reader: R) -> Result<Self, ParseError> {
        let mut buffered = io::BufReader::new(reader);
        let mut buf = String::new();
        buffered.read_to_string(&mut buf)?;
        Self::from_ini(buf)
    }

    pub fn open<P: Into<PathBuf>>(path: Option<P>) -> Result<Option<Self>, Error> {
        let path = match path {
            Some(p) => Some(p.into()),
            None => {
                let cwd = std::env::current_dir()?;
                let bumpversion_cfg = cwd.join(".bumpversion.cfg");
                let setup_cfg = cwd.join("setup.cfg");
                if bumpversion_cfg.is_file() {
                    Some(bumpversion_cfg)
                } else if setup_cfg.is_file() {
                    Some(setup_cfg)
                } else {
                    None
                }
            }
        };
        match path {
            Some(path) => {
                let config = Self::from_reader(fs::File::open(path)?)?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::io::Read;

    #[test]
    fn test_parse_python_setup_cfg_with_bumpversion() -> Result<()> {
        let setup_cfg = r#"
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

[bumpversion:file_with_dotted_version:file2]
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

[isort]
multi_line_output = 3
include_trailing_comma = True
force_grid_wrap = 0
use_parentheses = True
line_length = 88

[mypy]
files = favico,tests
ignore_missing_imports = true
disallow_subclassing_any = true
disallow_any_generics = true
disallow_untyped_calls = true
disallow_untyped_defs = true
disallow_incomplete_defs = true
check_untyped_defs = true
no_implicit_optional = true
warn_redundant_casts = true
warn_return_any = true
warn_unused_ignores = true
no_warn_unused_configs = true
warn_unused_configs = true
disallow_untyped_decorators = true

[tool:pytest]
addopts = -n auto
testpaths = tests/
        "#;
        let config = Config::from_ini(setup_cfg)?;
        println!("config: {:?}", config);
        todo!();
        Ok(())
    }

    #[test]
    fn test_parse_complex_python_setup_cfg() -> Result<()> {
        let setup_cfg = r#"
[metadata]
name = my_package
version = attr: my_package.VERSION
description = My package description
long_description = file: README.rst, CHANGELOG.rst, LICENSE.rst
keywords = one, two
license = BSD 3-Clause License
classifiers =
    Framework :: Django
    Programming Language :: Python :: 3

[options]
zip_safe = False
include_package_data = True
packages = find:
install_requires =
    requests
    importlib-metadata; python_version<"3.8"

[options.package_data]
* = *.txt, *.rst
hello = *.msg

[options.entry_points]
console_scripts =
    executable-name = my_package.module:function

[options.extras_require]
pdf = ReportLab>=1.2; RXP
rest = docutils>=0.3; pack ==1.1, ==1.3

[options.packages.find]
exclude =
    examples*
    tools*
    docs*
    my_package.tests*
        "#;
        let _ = Config::from_ini(setup_cfg)?;
        Ok(())
    }
}
