use crate::{config, context, version, Bump};
use color_eyre::eyre;

// /// Bump the version_part to the next value.
// pub fn get_next_version(
//     current_version: &compat::Version,
//     // version_config: &compat::VersionConfig,
//     version_component_to_bump: &Bump,
//     new_version: Option<&str>,
// ) -> eyre::Result<Option<compat::Version>> {
//     let next_version = if let Some(new_version) = new_version {
//         tracing::info!(new_version, "attempting to set new version");
//         version::compat::parse_version(new_version).map_err(|err| err)
//     } else {
//         tracing::info!(
//             component = version_component_to_bump.to_string(),
//             "attempting to increment version component"
//         );
//         current_version
//             .bump(version_component_to_bump)
//             .map(Some)
//             .map_err(|err| err.into())
//     }?;
//
//     Ok(next_version)
// }

pub mod compat {
    use crate::{
        config::{self, VersionComponentConfigs, VersionComponentSpec},
        context,
        f_string::OwnedPythonFormatString,
        Bump,
    };
    use color_eyre::eyre::{self, WrapErr};
    use indexmap::IndexMap;
    use std::collections::{HashMap, HashSet};

    pub type RawVersion<'a> = HashMap<&'a str, &'a str>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValuesFunction<'a> {
        values: &'a [String],
    }

    impl<'a> ValuesFunction<'a> {
        /// Return the item after ``value`` in the list
        fn bump(&self, value: &str) -> eyre::Result<&str> {
            let current_idx = self.values.iter().position(|v| *v == value);
            let current_idx = current_idx
                .ok_or_else(|| eyre::eyre!("{value:?} must be one of {:?}", self.values))?;
            let bumped_value = self.values.get(current_idx + 1).ok_or_else(|| {
                eyre::eyre!(
                    "the component has already the maximum value among {:?} and cannot be bumped.",
                    self.values
                )
            })?;
            Ok(bumped_value.as_str())
        }
    }

    pub static FIRST_NUMERIC_REGEX: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| {
            regex::RegexBuilder::new(r#"(?P<prefix>[^-0-9]*)(?P<number>-?\d+)(?P<suffix>.*)"#)
                .build()
                .unwrap()
        });

    #[derive(Debug)]
    pub struct NumericFunction<'a> {
        pub first_value: &'a str,
        pub optional_value: &'a str,
        // pub first_value: String,
        // pub optional_value: String,
    }

    impl<'a> NumericFunction<'a> {
        pub fn new(first_value: Option<&'a str>, optional_value: Option<&'a str>) -> Self {
            let first_value = first_value.unwrap_or("0"); // .to_string();
            let optional_value = optional_value.unwrap_or(&first_value); // .to_string();
            Self {
                first_value,
                optional_value,
            }
        }

        /// Increase the first numerical value by one
        pub fn bump(&self, value: &str) -> eyre::Result<String> {
            let first_numeric = FIRST_NUMERIC_REGEX.captures(value).ok_or_else(|| {
                eyre::eyre!("the given value {value:?} does not contain any digit")
            })?;

            let prefix_part = first_numeric
                .name("prefix")
                .ok_or_else(|| eyre::eyre!("{value:?} has no prefix"))?;
            let numeric_part = first_numeric
                .name("number")
                .ok_or_else(|| eyre::eyre!("{value:?} has no number"))?;
            let suffix_part = first_numeric
                .name("suffix")
                .ok_or_else(|| eyre::eyre!("{value:?} has no suffix"))?;

            let numeric_part: usize = numeric_part.as_str().parse().wrap_err_with(|| {
                eyre::eyre!(
                    "numeric part {numeric_part:?} of value {value:?} is not a valid number"
                )
            })?;

            let first_value: usize = self.first_value.parse().wrap_err_with(|| {
                eyre::eyre!("first value {:?} is not a valid number", self.first_value)
            })?;

            if numeric_part < first_value {
                eyre::bail!(
                    "{value:?} is lower than the first value {first_value:?} and cannot be bumped"
                );
            }

            let bumped_numeric = numeric_part.checked_add(1).ok_or_else(|| {
                eyre::eyre!("cannot imcrement numeric version number {numeric_part:?}")
            })?;
            Ok(format!(
                "{}{}{}",
                prefix_part.as_str(),
                bumped_numeric.to_string().as_str(),
                suffix_part.as_str()
            ))
        }
    }

    /// Represent part of a version number.
    ///
    /// Determines how the component behaves when increased or reset
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct VersionComponent {
        // value: String,
        value: Option<String>,
        spec: VersionComponentSpec,
        // todo: add spec here too?
        // pub func: Box<dyn BumpComponent<Error = >>, // avoid this and just dispatch in `bump()`?

        // pub func: ValuesFunction,
        // pub spec: VersionComponentSpec,

        // values: Optional[list] = None,
        // optional_value: Optional[str] = None,
        // first_value: Union[str, int, None] = None,
        // independent: bool = False,
        // always_increment: bool = False,
        // calver_format: Optional[str] = None,
        // source: Optional[str] = None,
        // value: Union[str, int, None] = None,
    }

    // self.func = ValuesFunction(str_values, str_optional_value, str_first_value)

    impl AsRef<str> for VersionComponent {
        fn as_ref(&self) -> &str {
            self.value().unwrap_or_default()
        }
    }

    impl VersionComponent {
        pub fn new(value: Option<&str>, spec: VersionComponentSpec) -> Self {
            // let func = ValuesFunction {
            //     values: spec.values.clone(),
            // };
            // if !spec.values.is_empty() {
            //     // let str_values = [str(v) for v in values]
            //     // let str_optional_value = str(optional_value) if spec.optional_value is not None else None
            //     // let str_first_value = str(first_value) if first_value is not None else None
            //     self.func = ValuesFunction(str_values, str_optional_value, str_first_value)
            // else if spec.calver_format:
            //     self.func = CalVerFunction(calver_format)
            //     self._value = self._value or self.func.first_value
            // else:
            //     self.func = NumericFunction(optional_value, first_value or "0")
            Self {
                value: value.map(|v| v.to_string()),
                spec,
            }
        }

        pub fn value(&self) -> Option<&str> {
            self.value.as_deref().or(self.spec.first_value.as_deref())
        }

        /// Return the component with with first value
        pub fn first(&self) -> Self {
            Self {
                value: self.spec.first_value.clone(),
                ..self.clone()
            }
        }

        /// Return a part with bumped value.
        pub fn bump(&self) -> eyre::Result<Self> {
            let value = if !self.spec.values.is_empty() {
                // let value = self.func.bump(Some(&self.value)).unwrap();
                let func = ValuesFunction {
                    values: self.spec.values.as_slice(),
                };
                let value = self
                    .value
                    .as_deref()
                    .unwrap_or(self.spec.values[0].as_str());
                func.bump(value).map(ToString::to_string)
            } else {
                // numeric
                let func = NumericFunction::new(
                    self.spec.first_value.as_deref(),
                    self.spec.optional_value.as_deref(),
                );
                func.bump(self.value.as_deref().unwrap_or("0"))
            }?;
            Ok(Self {
                value: Some(value),
                ..self.clone()
            })
        }
    }

    // impl config::VersionComponentSpec {
    //     /// Generate a version component from the configuration
    //     pub fn build_component(&self, value: Option<&str>) -> VersionComponent {
    //         VersionComponent::new(value, self.clone())
    //     }
    // }

    #[derive(thiserror::Error, Debug)]
    pub enum BumpError {
        #[error("invalid version component {0:?}")]
        InvalidComponent(String),
    }

    #[derive(Debug, Clone)]
    pub struct Version {
        components: IndexMap<String, VersionComponent>,
        spec: VersionSpec,
    }

    impl std::fmt::Display for Version {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_map()
                .entries(self.components.iter().map(|(k, v)| (k, v.value())))
                .finish()
        }
    }

    impl Version {
        pub fn from_components(
            components: impl IntoIterator<Item = (String, VersionComponent)>,
        ) -> Self {
            let components = components.into_iter().collect();
            // let spec = VersionSpec::from_components(&components).expect("TODO");
            let spec = VersionSpec::default();
            Self { components, spec }
        }

        /// Serialize a version to a string.
        pub fn serialize<'a, S, K, V>(
            &'a self,
            serialize_version_patterns: impl IntoIterator<Item = S>,
            ctx: &HashMap<K, V>,
        ) -> eyre::Result<String>
        where
            K: std::borrow::Borrow<str> + std::hash::Hash + Eq + std::fmt::Debug,
            V: AsRef<str> + std::fmt::Debug,
            S: AsRef<str> + std::fmt::Debug,
        {
            serialize_version(
                self,
                serialize_version_patterns,
                // .as_deref()
                // .unwrap_or_default(),
                ctx,
            )
        }

        // Return the values of the parts
        pub fn into_iter(self) -> indexmap::map::IntoIter<String, VersionComponent> {
            self.components.into_iter()
        }

        // Return the values of the parts
        pub fn iter(&self) -> indexmap::map::Iter<String, VersionComponent> {
            self.components.iter()
        }

        // Return the names of the parts that are required
        pub fn required_component_names(&self) -> impl Iterator<Item = &str> {
            self.iter()
                .filter(|(k, v)| v.value() != v.spec.optional_value.as_deref())
                .map(|(k, _)| k.as_str())
        }

        /// Return the components that always increment and depend on the given component
        fn always_incr_dependencies(&self) -> HashMap<&str, HashSet<&str>> {
            self.spec
                .components_to_always_increment
                .iter()
                .map(|comp_name| (comp_name.as_str(), self.spec.dependents(comp_name)))
                .collect()
        }

        /// Increase the values of the components that always increment
        fn increment_always_incr(&self) -> eyre::Result<HashMap<&str, VersionComponent>> {
            let components = self
                .spec
                .components_to_always_increment
                .iter()
                .map(|comp_name| {
                    self.components[comp_name]
                        .bump()
                        .map(|bumped_comp| (comp_name.as_str(), bumped_comp))
                })
                .collect::<Result<_, _>>()?;
            Ok(components)
        }

        /// Return the components that always increment and their dependents
        fn always_increment(
            &self,
        ) -> eyre::Result<(HashMap<&str, VersionComponent>, HashSet<&str>)> {
            let values = self.increment_always_incr()?;
            let mut dependents = self.always_incr_dependencies();
            for (comp_name, value) in values.iter() {
                if value == &self.components[*comp_name] {
                    dependents.remove(comp_name);
                }
            }
            let unique_dependents: HashSet<&str> =
                dependents.values().flat_map(|v| v).copied().collect();
            Ok((values, unique_dependents))
        }

        /// Increase the value of the specified component.
        ///
        /// This will reset its dependents, and return a new `Version`.
        pub fn bump(&self, component: &Bump) -> eyre::Result<Self> {
            let component_name = component.name();
            if !self.components.contains_key(component_name) {
                return Err(BumpError::InvalidComponent(component_name.to_string()).into());
            }

            let mut new_components = self.components.clone();
            let (always_increment_values, mut components_to_reset) = self.always_increment()?;
            dbg!(&always_increment_values, &components_to_reset);

            new_components.extend(
                always_increment_values
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v)),
            );

            let should_reset = components_to_reset.contains(component_name);
            if !should_reset {
                new_components.insert(
                    component_name.to_string(),
                    self.components[component_name].bump()?,
                );
                let dependants = self.spec.dependents(component_name);
                components_to_reset.extend(dependants);
            }

            dbg!(&new_components, &components_to_reset);

            for comp_name in components_to_reset {
                dbg!(&comp_name);
                dbg!(&self.components);
                let is_independent = self.components[comp_name].spec.independent == Some(true);
                if !is_independent {
                    *new_components.get_mut(comp_name).unwrap() =
                        self.components[comp_name].first();
                }
            }

            Ok(Self {
                components: new_components,
                ..self.clone()
            })
        }
    }

    /// The specification of a version's components and their relationships
    #[derive(Debug, Clone, Default)]
    pub struct VersionSpec {
        components: config::VersionComponentConfigs,
        dependency_map: HashMap<String, Vec<String>>,
        components_to_always_increment: Vec<String>,
    }

    impl VersionSpec {
        pub fn from_components(components: config::VersionComponentConfigs) -> eyre::Result<Self> {
            let mut dependency_map: HashMap<String, Vec<String>> = HashMap::new();
            let mut previous_component: &String = components
                .keys()
                .next()
                .ok_or_else(|| eyre::eyre!("must have at least one component"))?;

            let components_to_always_increment: Vec<String> = components
                .iter()
                .filter_map(|(comp_name, comp_config)| {
                    if comp_config.always_increment {
                        Some(comp_name)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect();

            for (comp_name, comp_config) in components.iter().skip(1) {
                if comp_config.independent == Some(true) {
                    continue;
                }
                if let Some(ref depends_on) = comp_config.depends_on {
                    dependency_map
                        .entry(depends_on.clone())
                        .or_default()
                        .push(comp_name.clone())
                } else {
                    dependency_map
                        .entry(previous_component.clone())
                        .or_default()
                        .push(comp_name.clone())
                }
                previous_component = comp_name;
            }

            // dbg!(&components_to_always_increment);
            // dbg!(&dependency_map);

            Ok(Self {
                components,
                dependency_map,
                components_to_always_increment,
            })
        }

        /// Return the components that depend on the given component.
        pub fn dependents(&self, comp_name: &str) -> HashSet<&str> {
            use std::collections::VecDeque;
            let mut stack = VecDeque::from_iter(
                self.dependency_map
                    .get(comp_name)
                    .map(|deps| deps.iter())
                    .unwrap_or_default(),
            );
            let mut visited: HashSet<&str> = HashSet::new();

            while let Some(e) = stack.pop_front() {
                if !visited.contains(e.as_str()) {
                    visited.insert(e);
                    for dep in self
                        .dependency_map
                        .get(e)
                        .map(|deps| deps.iter())
                        .unwrap_or_default()
                    {
                        stack.push_front(dep)
                    }
                }
            }
            visited
        }

        /// Generate a version from the given values
        pub fn build(&self, raw_components: &RawVersion) -> Version {
            let components = self
                .components
                .iter()
                .map(|(comp_name, comp_config)| {
                    let comp_value = raw_components.get(comp_name.as_str()).copied();
                    let component = VersionComponent::new(comp_value, comp_config.clone());
                    (comp_name.to_string(), component)
                })
                .collect();
            Version {
                components,
                spec: self.clone(),
            }
        }
    }

    // #[derive(Debug)]
    // pub struct VersionConfig {
    //     /// Regex parsing the version string
    //     pub parse_version_regex: regex::Regex,
    //     /// How to serialize back to a version
    //     pub serialize_version_patterns: Option<Vec<String>>,
    //     /// Template for complete string to search
    //     pub search: Option<String>,
    //     /// Template for complete string to replace
    //     pub replace: Option<String>,
    //     // /// Template for complete string to replace
    //     // pub parts: config::Parts,
    //     pub version_spec: VersionSpec,
    // }
    //
    // #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    // pub struct SerializedVersion {
    //     pub version: String,
    //     pub tag: Option<String>,
    // }

    /// Attempts to serialize a version with the given serialization format.
    ///
    /// - valid serialization patterns are those that are renderable with the given context
    /// - formats that contain all required components are preferred
    /// - the shortest valid serialization pattern is used
    /// - if two patterns are equally short, the first one is used
    /// - if no valid serialization pattern is found, an error is raised
    fn serialize_version<'a, S, K, V>(
        version: &'a Version,
        // serialize_patterns: &[String],
        serialize_patterns: impl IntoIterator<Item = S>,
        // ctx: impl IntoIterator<Item = (&'a str, &'a str)>,
        // ctx: impl IntoIterator<Item = (&'a str, &'a str)>,
        ctx: &HashMap<K, V>,
    ) -> eyre::Result<String>
    where
        K: std::borrow::Borrow<str> + std::hash::Hash + Eq + std::fmt::Debug,
        V: AsRef<str> + std::fmt::Debug,
        S: AsRef<str> + std::fmt::Debug,
    {
        tracing::debug!(?version, "serializing");

        let ctx: HashMap<&str, &str> = ctx
            .into_iter()
            .map(|(k, v)| (k.borrow(), v.as_ref()))
            .chain(version.iter().map(|(k, v)| (k.as_str(), v.as_ref())))
            .collect();

        let required_component_names: HashSet<_> = version.required_component_names().collect();
        // local_context_keys = set(local_context.keys())

        // dbg!(&serialize_patterns);

        let mut patterns: Vec<(usize, OwnedPythonFormatString)> = serialize_patterns
            .into_iter()
            .enumerate()
            .map(|(idx, pattern)| {
                OwnedPythonFormatString::parse(pattern.as_ref()).map(|f| (idx, f))
            })
            .collect::<Result<_, _>>()?;

        // dbg!(&patterns);

        // let mut valid_patterns: Vec<_> = patterns
        //     .iter()
        //     .filter(|(_, pattern)| ctx.len() >= pattern.named_arguments().count())
        //     .collect();

        patterns.sort_by_key(|(idx, pattern)| {
            let labels: HashSet<&str> = pattern.named_arguments().collect();
            let has_required_components = required_component_names.is_subset(&labels);
            let num_labels = labels.len();
            (std::cmp::Reverse(has_required_components), num_labels, *idx)
        });

        // dbg!(&patterns);

        let (_, chosen_pattern) = patterns.first().ok_or_else(|| {
            eyre::eyre!(
                "could not find a valid serialization format in {patterns:?} for {version:?}"
            )
        })?;

        // dbg!(&ctx);

        tracing::debug!(format = ?chosen_pattern, "serialization format");
        let serialized = chosen_pattern.format(&ctx, true)?;
        tracing::debug!(serialized, "serialized");

        Ok(serialized)
    }

    /// Parse a version string into a dictionary of the parts and values using a regular expression.
    ///
    /// # Errors
    /// If the parse_pattern is not a valid regular expression.
    fn parse_raw_version<'a>(
        version: &'a str,
        pattern: &'a regex::Regex,
    ) -> eyre::Result<RawVersion<'a>> {
        if version.is_empty() {
            tracing::warn!("version string is empty");
            return Ok(RawVersion::default());
        }

        tracing::debug!(version, ?pattern, "parsing version");

        let Some(matches) = pattern.captures(version) else {
            tracing::debug!(?pattern, ?version, "pattern does not parse current version",);
            return Ok(RawVersion::default());
        };

        let parsed: RawVersion = pattern
            .capture_names()
            .filter_map(|name| name)
            .filter_map(|name| matches.name(name).map(|value| (name, value.as_str())))
            .collect();

        tracing::debug!(?parsed, "parsed version");

        Ok(parsed)
    }

    // impl VersionConfig {
    //     pub fn from_config(
    //         config: &config::GlobalConfig,
    //         parts: &config::VersionComponentConfigs,
    //     ) -> eyre::Result<Self> {
    //         let parse_version_regex = regex::RegexBuilder::new(
    //             config
    //                 .parse_version_pattern
    //                 .as_deref()
    //                 .unwrap_or(config::DEFAULT_PARSE_VERSION_PATTERN),
    //         )
    //         .build()?;
    //
    //         let version_spec = VersionSpec::from_components(parts)?;
    //         Ok(Self {
    //             // parse_pattern: config.parse_pattern.clone(),
    //             parse_version_regex,
    //             serialize_version_patterns: config.serialize_version_patterns.clone(),
    //             search: config.search.clone(),
    //             replace: config.replace.clone(),
    //             version_spec,
    //         })
    //     }

    /// Parse a version string into a Version object.
    pub fn parse_version(
        value: &str,
        regex: &regex::Regex,
        version_spec: &VersionSpec,
    ) -> eyre::Result<Option<Version>> {
        let parsed = parse_raw_version(value, &regex)?;
        if parsed.is_empty() {
            return Ok(None);
        }
        let version = version_spec.build(&parsed);
        Ok(Some(version))
    }
}

mod semver {
    use std::borrow::{Borrow, Cow};

    pub trait Deser {
        type Error;

        fn parse(&self, version: impl AsRef<str>) -> Result<Version, Self::Error>;
        fn serialize(&self, version: impl Borrow<Version>) -> Result<String, Self::Error>;
    }

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

    #[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
    struct Version {
        pub major: usize,
        pub minor: usize,
        pub patch: usize,
    }

    impl Version {
        pub fn new(major: usize, minor: usize, patch: usize) -> Self {
            Self {
                major,
                minor,
                patch,
            }
        }
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

    #[derive(Debug)]
    struct Parser<'a> {
        parse_regex: &'a regex::Regex,
        serialize_format: String,
    }

    static DEFAULT_PARSE_PATTERN_REGEX: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| {
            regex::RegexBuilder::new(r#"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)"#)
                .build()
                .unwrap()
        });

    impl<'a> Default for Parser<'a> {
        fn default() -> Self {
            Self {
                parse_regex: &*DEFAULT_PARSE_PATTERN_REGEX,
                serialize_format: "{major}.{minor}.{patch}".to_string(),
            }
        }
    }

    impl<'a> Deser for Parser<'a> {
        type Error = Error;

        fn parse(&self, version: impl AsRef<str>) -> Result<Version, Self::Error> {
            let version = version.as_ref();
            let caps = self.parse_regex.captures(version).ok_or(Error::BadFormat {
                format: self.parse_regex.to_string(),
                found: version.to_string(),
            })?;
            Ok(Version {
                major: parse_component!(caps, "major")?,
                minor: parse_component!(caps, "minor")?,
                patch: parse_component!(caps, "patch")?,
            })
        }

        fn serialize(&self, version: impl Borrow<Version>) -> Result<String, Self::Error> {
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
}

#[cfg(test)]
mod tests {
    use super::semver;
    use color_eyre::eyre;
    use std::borrow::Borrow;

    // #[test]
    // fn test_semver_version() -> eyre::Result<()> {
    //     crate::tests::init();
    //
    //     let version = SemVer::new(1, 3, 2);
    //     let parser = SemVerParser::default();
    //     let serialized = parser.serialize(&version)?;
    //     let deserialized = parser.parse(&serialized)?;
    //     similar_asserts::assert_eq!(version, deserialized);
    //     Ok(())
    // }
    //
    // #[test]
    // fn test_generic_version() -> eyre::Result<()> {
    //     crate::tests::init();
    //
    //     let version = SemVer::new(1, 3, 2);
    //     let parse_regex = regex::Regex::new(r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)")?;
    //     let parser = SemVerParser {
    //         parse_regex: &parse_regex,
    //         serialize_format: "{major}.{minor}.{patch}".into(),
    //     };
    //     let serialized = parser.serialize(&version)?;
    //     let deserialized = parser.parse(&serialized)?;
    //     similar_asserts::assert_eq!(version, deserialized);
    //     Ok(())
    // }
}
