use crate::{config, context, Bump};
use color_eyre::eyre;

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

/// Bump the version_part to the next value.
pub fn get_next_version(
    current_version: &compat::Version,
    version_config: &compat::VersionConfig,
    // config: &config::Config,
    version_component_to_bump: &Bump,
    new_version: Option<&str>,
) -> eyre::Result<Option<compat::Version>> {
    let next_version = if let Some(new_version) = new_version {
        tracing::info!(new_version, "attempting to set new version");
        version_config.parse(new_version).map_err(|err| err)
    } else {
        tracing::info!(
            component = version_component_to_bump.to_string(),
            "attempting to increment version component"
        );
        current_version
            .bump(version_component_to_bump)
            .map(Some)
            .map_err(|err| err.into())
    }?;

    tracing::info!(?next_version, "next version");
    Ok(next_version)
}

pub mod compat {
    use crate::{
        config::{self, VersionComponentSpec},
        context, Bump,
    };
    use color_eyre::eyre::{self, WrapErr};
    use std::collections::{HashMap, HashSet};

    pub type RawVersion<'a> = HashMap<&'a str, &'a str>;

    // pub trait BumpComponent {
    //     // type Error: std::error::Error;
    //     type Error;
    //
    //     fn bump(&self, value: Option<&str>) -> Result<String, Self::Error>;
    // }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValuesFunction<'a> {
        // values: Vec<String>,
        values: &'a [String],
    }

    // impl BumpComponent for ValuesFunction {
    impl<'a> ValuesFunction<'a> {
        // type Error = eyre::Report;

        /// Return the item after ``value`` in the list
        // fn bump(&self, value: Option<&str>) -> Result<String, Self::Error> {
        fn bump(&self, value: &str) -> eyre::Result<&str> {
            // let value = value.ok_or_else(|| eyre::eyre!("missing value"))?;
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
            // .cloned()
            // try:
            //     return self._values[self._values.index(value) + 1]
            // except IndexError as e:
            //     raise ValueError(
            //         f"The part has already the maximum value among {self._values} and cannot be bumped."
            //     ) from e
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

    // def __init__(self, optional_value: Union[str, int, None] = None, first_value: Union[str, int, None] = None):
    //     if first_value is not None and not self.FIRST_NUMERIC.search(str(first_value)):
    //         raise ValueError(f"The given first value {first_value} does not contain any digit")
    //
    //     self.first_value = str(first_value or 0)
    //     self.optional_value = str(optional_value or self.first_value)
    //     self.independent = False
    //     self.always_increment = False
    //
    // def bump(self, value: Union[str, int]) -> str:
    //     """Increase the first numerical value by one."""
    //     match = self.FIRST_NUMERIC.search(str(value))
    //     if not match:
    //         raise ValueError(f"The given value {value} does not contain any digit")
    //
    //     part_prefix, part_numeric, part_suffix = match.groups()
    //
    //     if int(part_numeric) < int(self.first_value):
    //         raise ValueError(
    //             f"The given value {value} is lower than the first value {self.first_value} and cannot be bumped."
    //         )
    //
    //     bumped_numeric = int(part_numeric) + 1
    //
    //     return "".join([part_prefix, str(bumped_numeric), part_suffix])

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

    // TODO: rename part config to version component spec
    impl config::VersionComponentSpec {
        /// Generate a version component from the configuration
        pub fn build_component(&self, value: Option<&str>) -> VersionComponent {
            VersionComponent::new(value, self.clone())
        }
    }

    #[derive(Debug, Clone)]
    pub struct Version {
        components: HashMap<String, VersionComponent>,
        spec: VersionSpec,
    }

    #[derive(thiserror::Error, Debug)]
    pub enum BumpError {
        #[error("invalid version component {0:?}")]
        InvalidComponent(String),
    }

    impl Version {
        // pub fn empty() -> Self {
        //     Self {
        //         components: HashMap::default(),
        //         spec: VersionSpec::default(),
        //     }
        // }

        // Return the values of the parts
        pub fn into_iter(self) -> std::collections::hash_map::IntoIter<String, VersionComponent> {
            self.components.into_iter()
        }

        // Return the values of the parts
        pub fn iter(&self) -> std::collections::hash_map::Iter<String, VersionComponent> {
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
        components: config::Parts,
        dependency_map: HashMap<String, Vec<String>>,
        components_to_always_increment: Vec<String>,
    }

    impl VersionSpec {
        pub fn from_parts(components: &config::Parts) -> eyre::Result<Self> {
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

            dbg!(&components_to_always_increment);
            dbg!(&dependency_map);

            Ok(Self {
                components: components.clone(),
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
        pub fn build(&self, raw_components: &RawVersion, raw_version: String) -> Version {
            dbg!(&self.components);
            let components = self
                .components
                .iter()
                // .filter_map(|(comp_name, comp_config)| {
                .map(|(comp_name, comp_config)| {
                    let comp_value = raw_components.get(comp_name.as_str()).copied();
                    let component = comp_config.build_component(comp_value);
                    (comp_name.to_string(), component)
                    // let comp_value = raw_components.get(comp_name.as_str());
                    // comp_value.map(|comp_value| {
                    //     let component = comp_config.build_component(comp_value.to_string());
                    //     (comp_name.to_string(), component)
                    // })
                })
                .collect();
            Version {
                components,
                spec: self.clone(),
            }
        }
    }

    #[derive(Debug)]
    pub struct VersionConfig {
        /// Regex parsing the version string
        pub parse_version_regex: regex::Regex,
        /// How to serialize back to a version
        pub serialize_version_patterns: Option<Vec<String>>,
        /// Template for complete string to search
        pub search: Option<String>,
        /// Template for complete string to replace
        pub replace: Option<String>,
        // /// Template for complete string to replace
        // pub parts: config::Parts,
        pub version_spec: VersionSpec,
    }

    pub fn format_string_arguments<'a>(
        pattern: &'a str,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        // use gobble (https://docs.rs/gobble/latest/gobble/) ?
        // use fancy-regex with lookaround?
        // use nom?

        [].into_iter()
        // todo!("use nom");

        // use parse_format::{Argument, ParseMode, Parser, Piece, Position};
        // let parser = Parser::new(pattern, None, None, false, ParseMode::Format);
        // parser.into_iter().filter_map(|piece| match piece {
        //     Piece::String(s) => {
        //         dbg!(&s);
        //         None
        //     }
        //     Piece::NextArgument(Argument {
        //         position: Position::ArgumentNamed(arg),
        //         ..
        //     }) => Some(arg),
        //     Piece::NextArgument(arg) => {
        //         dbg!(&arg);
        //         None
        //     }
        // })
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SerializedVersion {
        pub version: String,
        pub tag: Option<String>,
    }

    /// Attempts to serialize a version with the given serialization format.
    ///
    /// - valid serialization patterns are those that are renderable with the given context
    /// - formats that contain all required components are preferred
    /// - the shortest valid serialization pattern is used
    /// - if two patterns are equally short, the first one is used
    /// - if no valid serialization pattern is found, an error is raised
    fn serialize_version<'a>(
        version: &'a Version,
        serialize_patterns: &[String],
        // ctx: &context::Env,
        ctx: impl Iterator<Item = (&'a str, &'a str)>,
        // ctx: impl Iterator<Item = (String, String)>,
        // ctx: &HashMap<&str, &str>,
    ) -> eyre::Result<String> {
        use crate::f_string::PythonFormatString;

        tracing::debug!(?version, "serializing");

        let ctx: HashMap<&str, &str> = ctx
            // .iter()
            // .map(|(k, v)| (*k, *v))
            // .map(|(k, v)| (k.as_str(), v.as_str()))
            .chain(version.iter().map(|(k, v)| (k.as_str(), v.as_ref())))
            .collect();
        // let ctx: HashMap<&str, &VersionComponent> =
        //     version.iter().map(|(k, v)| (k.as_str(), v)).collect();
        let required_component_names: HashSet<_> = version.required_component_names().collect();
        // local_context_keys = set(local_context.keys())

        // /// Return a list of labels for the given serialize_format
        // fn labels_for_format(serialize_format: &str) -> Vec<String> {
        //     // return [item[1] for item in string.Formatter().parse(serialize_format) if item[1]]
        //     vec![]
        // }
        dbg!(&serialize_patterns);

        // #[derive(Debug)]
        // struct Pattern<'a> {
        //     idx: usize,
        //     pattern: &'a str,
        //     // labels: Vec<String>,
        //     labels: HashSet<&'a str>,
        // }

        // let patterns = serialize_patterns.iter().enumerate().map(|(idx, pattern)| {

        let patterns = serialize_patterns
            .iter()
            .enumerate()
            .map(|(idx, pattern)| {
                // let labels: HashSet<_> = format_string_arguments(pattern).collect();
                PythonFormatString::try_from(pattern.as_str()).map(|f| (idx, f))
                // let fstring
                // Pattern {
                //     idx,
                //     pattern,
                //     labels,
                // }
            })
            .collect::<Result<Vec<(usize, PythonFormatString)>, _>>()?;

        dbg!(&patterns);
        // dbg!(patterns.clone().collect::<Vec<_>>());

        let mut valid_patterns: Vec<_> = patterns
            .iter()
            .filter(|(_, pattern)| ctx.len() >= pattern.named_arguments().count())
            .collect();

        valid_patterns.sort_by_key(|(idx, pattern)| {
            use std::cmp::Reverse;
            let labels: HashSet<&str> = pattern.named_arguments().collect();
            let has_required_components = required_component_names.is_subset(&labels);
            let num_labels = labels.len();
            (Reverse(has_required_components), num_labels, idx)
        });
        // sorted_patterns = multisort(
        // list(valid_patterns), (("has_required_components", True), ("num_labels", False), ("order", False))
        // )

        dbg!(&valid_patterns);
        // dbg!(valid_patterns.clone().collect::<Vec<_>>());

        // for (index, pattern) in serialize_patterns.iter().enumerate() {
        //     let args: Vec<_> = format_string_arguments(pattern).collect();
        //     //     let labels = labels_for_format(pattern);
        //     //     dbg!(&labels);
        //     //     // patterns.append(
        //     //     //     {
        //     //     //         "pattern": pattern,
        //     //     //         "labels": labels,
        //     //     //         "order": index,
        //     //     //         "num_labels": len(labels),
        //     //     //         "renderable": local_context_keys >= labels,
        //     //     //         "has_required_components": required_component_labels <= labels,
        //     //     //     }
        //     //     // )
        // }

        let (_, chosen_pattern) = valid_patterns.first().ok_or_else(|| {
            eyre::eyre!(
            "could not find a valid serialization format in {serialize_patterns:?} for {version:?}"
        )
        })?;
        // if valid_patterns.is_empty() {
        //     eyre::bail!(
        //         "could not find a valid serialization format in {serialize_patterns:?} for {version:?}"
        //     );
        // }

        // "test".to_string().as_str()
        // let test: &str = "test".to_string().as_ref();
        tracing::debug!(format = ?chosen_pattern, "serialization format");
        let serialized = chosen_pattern.format(&ctx, true)?;
        tracing::debug!(serialized, "serialized");

        Ok("todo".to_string())
    }

    /// Parse a version string into a dictionary of the parts and values using a regular expression.
    ///
    /// # Errors
    /// If the parse_pattern is not a valid regular expression.
    // fn parse_version<'a>(version: &'a str, parse_pattern: &str) -> eyre::Result<RawVersion<'a>> {
    fn parse_version<'a>(
        version: &'a str,
        parse_pattern: &'a regex::Regex,
    ) -> eyre::Result<RawVersion<'a>> {
        // A dictionary of version part labels and their values, or an empty dictionary if the version string doesn't match.

        if version.is_empty() {
            tracing::warn!("version string is empty");
            return Ok(RawVersion::default());
        }
        // else if parse_pattern.is_empty() {
        //     tracing::warn!("parse pattern is empty");
        //     return Ok(RawVersion::default());
        // }

        tracing::debug!(version, ?parse_pattern, "parsing version");

        // let pattern = regex::RegexBuilder::new(parse_pattern).build()?;

        let Some(matches) = parse_pattern.captures(version) else {
            tracing::debug!(
                ?parse_pattern,
                ?version,
                "pattern does not parse current version",
            );
            return Ok(RawVersion::default());
        };

        let parsed: RawVersion = parse_pattern
            .capture_names()
            // .cloned()
            .filter_map(|name| name)
            .filter_map(|name| matches.name(name).map(|value| (name, value.as_str())))
            // .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();

        tracing::debug!(?parsed, "parsed version");

        Ok(parsed)
    }

    impl VersionConfig {
        pub fn from_config(
            config: &config::FileConfig,
            parts: &config::Parts,
            // ) -> Result<Self, regex::Error> {
        ) -> eyre::Result<Self> {
            let parse_version_regex = regex::RegexBuilder::new(
                config
                    .parse_version_pattern
                    .as_deref()
                    .unwrap_or(config::DEFAULT_PARSE_VERSION_PATTERN),
            )
            .build()?;

            let version_spec = VersionSpec::from_parts(parts)?;
            Ok(Self {
                // parse_pattern: config.parse_pattern.clone(),
                parse_version_regex,
                serialize_version_patterns: config.serialize_version_patterns.clone(),
                search: config.search.clone(),
                replace: config.replace.clone(),
                version_spec,
                // parts,
            })
        }

        /// Serialize a version to a string.
        pub fn serialize<'a>(
            &self,
            version: &'a Version,
            // ctx: &HashMap<&str, &str>,
            ctx: impl Iterator<Item = (&'a str, &'a str)>,
            // ctx: impl Iterator<Item = (String, String)>,
        ) -> eyre::Result<String> {
            serialize_version(
                version,
                self.serialize_version_patterns
                    .as_deref()
                    .unwrap_or_default(),
                ctx,
            )
        }

        /// Parse a version string into a Version object.
        pub fn parse(
            &self,
            raw_version: &str,
            // allow_empty: bool,
        ) -> eyre::Result<Option<Version>> {
            let parsed = parse_version(raw_version, &self.parse_version_regex)?;
            // dbg!(&parsed);

            if parsed.is_empty() {
                return Ok(None);
            }

            // if !allow_empty && parsed.is_empty() {
            //     eyre::bail!("Unable to parse version {version} using {}", self.parse_regex);
            // } else if
            //
            let version = self.version_spec.build(&parsed, raw_version.to_string());
            Ok(Some(version))
        }
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
