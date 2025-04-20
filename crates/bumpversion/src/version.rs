use crate::{
    config::version::{VersionComponentConfigs, VersionComponentSpec},
    f_string::PythonFormatString,
};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// Raw representation of parsed version segments.
///
/// Maps component names (e.g., "major", "minor") to their string values.
pub type RawVersion<'a> = HashMap<&'a str, &'a str>;

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct ValuesFunction<'a> {
//     values: &'a [String],
// }
//
// impl ValuesFunction<'_> {
//     /// Return the item after ``value`` in the list
//     fn bump(&self, value: &str) -> Result<&str> {
//         let current_idx = self.values.iter().position(|v| *v == value);
//         let current_idx =
//             current_idx.ok_or_else(|| eyre::eyre!("{value:?} must be one of {:?}", self.values))?;
//         let bumped_value = self.values.get(current_idx + 1).ok_or_else(|| {
//             eyre::eyre!(
//                 "the component has already the maximum value among {:?} and cannot be bumped.",
//                 self.values
//             )
//         })?;
//         Ok(bumped_value.as_str())
//     }
// }

// TODO: refactor this
/// Numeric parsing and bumping utilities for version components.
pub mod numeric {
    /// Regex matching the first numeric substring with optional prefix/suffix.
    ///
    /// Captures named groups: `prefix`, `number`, and `suffix`.
    pub static FIRST_NUMERIC_REGEX: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| {
            regex::RegexBuilder::new(r"(?P<prefix>[^-0-9]*)(?P<number>-?\d+)(?P<suffix>.*)")
                .build()
                .unwrap()
        });

    /// Errors encountered when extracting or bumping numeric components.
    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        /// No digit found in the version string.
        #[error("version {0:?} does not contain any digit")]
        MissingDigit(String),
        /// Prefix capture group missing from regex match.
        #[error("version {0:?} has no prefix")]
        MissingPrefix(String),
        /// Numeric capture group missing from regex match.
        #[error("version {0:?} has no number")]
        MissingNumber(String),
        /// Suffix capture group missing from regex match.
        #[error("version {0:?} has no suffix")]
        MissingSuffix(String),
        /// Failed to parse the numeric substring as an integer.
        #[error("{value:?} is not a valid number")]
        InvalidNumber {
            /// Source parse error.
            #[source]
            source: std::num::ParseIntError,
            /// The offending numeric text.
            value: String,
        },
        /// Numeric part out of valid range for the version.
        #[error("{value:?} of version {version:?} is not a valid number")]
        InvalidNumeric {
            /// Source parse error.
            #[source]
            source: std::num::ParseIntError,
            /// The full version string.
            version: String,
            /// The numeric substring.
            value: String,
        },
        /// Current numeric value is below the configured minimum.
        /// Current numeric value is below the configured minimum.
        #[error("{value:?} is lower than the first value {first_value:?} and cannot be bumped")]
        LessThanFirstValue {
            /// The minimum allowed value for the component.
            first_value: usize,
            /// The numeric value provided.
            value: usize,
        },
        /// Integer overflow or out-of-bounds during bump.
        /// Numeric bump would overflow or exceed bounds.
        #[error("version component {component:?} exceeds bounds and cannot be bumped")]
        OutOfBounds {
            /// The component value that could not be bumped.
            component: usize,
        },
        // #[error("{value:?} of version {version:?} is not a valid number")]
        // InvalidFirstfvalue {
        //     #[source]
        //     source: std::num::ParseIntError,
        //     version: String,
        //     value: String,
        // },
    }

    /// Function for bumping numeric version components.
    #[derive(Debug)]
    pub struct NumericFunction {
        /// Minimum value allowed for the numeric component.
        pub first_value: usize,
        /// Optional starting value for bumping when unspecified.
        pub optional_value: usize,
        // pub first_value: &'a str,
        // pub optional_value: &'a str,
        // pub first_value: String,
        // pub optional_value: String,
    }

    impl NumericFunction {
        /// Create a new `NumericFunction` with optional string defaults.
        ///
        /// # Arguments
        /// * `first_value` - Optional minimum value as string (defaults to 0).
        /// * `optional_value` - Optional starting bump value (defaults to `first_value`).
        ///
        /// # Errors
        /// Returns `Error::InvalidNumber` if provided defaults cannot be parsed.
        pub fn new(first_value: Option<&str>, optional_value: Option<&str>) -> Result<Self, Error> {
            // pub fn new(first_value: Option<usize>, optional_value: Option<usize>) -> Self {
            let first_value = first_value
                .map(|value| {
                    value.parse().map_err(|source| Error::InvalidNumber {
                        source,
                        value: value.to_string(),
                    })
                })
                .transpose()?
                .unwrap_or(0);

            let optional_value = optional_value
                .map(|value| {
                    value.parse().map_err(|source| Error::InvalidNumber {
                        source,
                        value: value.to_string(),
                    })
                })
                .transpose()?
                .unwrap_or(first_value);
            Ok(Self {
                first_value,
                optional_value,
            })
        }

        /// Increase the first numerical value by one
        /// Bump the first numeric component in `value` by one.
        ///
        /// Uses `FIRST_NUMERIC_REGEX` to locate a number, increments it, and reinserts.
        ///
        /// # Errors
        /// Returns `Error` variants for missing parts or overflow.
        pub fn bump(&self, value: &str) -> Result<String, Error> {
            let first_numeric = FIRST_NUMERIC_REGEX
                .captures(value)
                .ok_or_else(|| Error::MissingDigit(value.to_string()))?;

            let prefix_part = first_numeric
                .name("prefix")
                .ok_or_else(|| Error::MissingPrefix(value.to_string()))?;
            let numeric_part = first_numeric
                .name("number")
                .ok_or_else(|| Error::MissingNumber(value.to_string()))?;
            let suffix_part = first_numeric
                .name("suffix")
                .ok_or_else(|| Error::MissingSuffix(value.to_string()))?;

            let numeric_part: usize =
                numeric_part
                    .as_str()
                    .parse()
                    .map_err(|source| Error::InvalidNumeric {
                        source,
                        version: value.to_string(),
                        value: numeric_part.as_str().to_string(),
                    })?;

            // let first_value: usize = self.first_value.parse().map_err(|source| {
            //     // eyre::eyre!("first value {:?} is not a valid number", self.first_value)
            //     Error::InvalidNumeric {
            //         source,
            //         version: value.to_string(),
            //         value: self.first_value.to_string(),
            //     }
            // })?;

            if numeric_part < self.first_value {
                return Err(Error::LessThanFirstValue {
                    first_value: self.first_value,
                    value: numeric_part,
                });
            }

            let Some(bumped_numeric) = numeric_part.checked_add(1) else {
                return Err(Error::OutOfBounds {
                    component: numeric_part,
                });
            };
            Ok(format!(
                "{}{}{}",
                prefix_part.as_str(),
                bumped_numeric.to_string().as_str(),
                suffix_part.as_str()
            ))
        }
    }
}

/// A single version component, combining a value and its bump/reset specification.
///
/// Determines how the component is bumped and how dependents are reset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Component {
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

impl AsRef<str> for Component {
    fn as_ref(&self) -> &str {
        self.value().unwrap_or_default()
    }
}

/// Errors that can occur when bumping a version component.
#[derive(thiserror::Error, Debug)]
pub enum BumpError {
    /// Underlying numeric bump error (e.g., missing digits, overflow).
    #[error(transparent)]
    Numeric(#[from] numeric::Error),
    /// Specified component name does not exist in the version.
    #[error("invalid version component {0:?}")]
    InvalidComponent(String),
}

impl Component {
    /// Create a new `Component` with an optional initial `value` and its `spec`.
    ///
    /// # Arguments
    /// * `value` - Optional current component string value.
    /// * `spec` - Component configuration (first value, dependencies, bump rules).
    #[must_use]
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
            value: value.map(std::string::ToString::to_string),
            spec,
        }
    }

    /// Return the effective current value of this component.
    ///
    /// Falls back to the `spec.first_value` if no explicit value is set.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref().or(self.spec.first_value.as_deref())
    }

    /// Return a new `Component` initialized with its `spec.first_value`.
    ///
    /// Useful for resetting dependent components.
    #[must_use]
    pub fn first(&self) -> Self {
        Self {
            value: self.spec.first_value.clone(),
            ..self.clone()
        }
    }

    /// Bump this component according to its specification.
    ///
    /// For components with explicit value lists, uses those; otherwise numeric bump.
    ///
    /// # Errors
    /// Returns `BumpError::Numeric` or `BumpError::InvalidComponent` on failure.
    pub fn bump(&self) -> Result<Self, BumpError> {
        let value = if self.spec.values.is_empty() {
            // numeric
            let func = numeric::NumericFunction::new(
                self.spec.first_value.as_deref(),
                self.spec.optional_value.as_deref(),
            )?;
            func.bump(self.value.as_deref().unwrap_or("0"))
        } else {
            todo!("value bump function");
            // let func = ValuesFunction {
            //     values: self.spec.values.as_slice(),
            // };
            // let value = self
            //     .value
            //     .as_deref()
            //     .unwrap_or(self.spec.values[0].as_str());
            // func.bump(value).map(ToString::to_string)
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

/// Represents a parsed version, storing its components and bumping rules.
///
/// Holds component values and the `VersionSpec` that governs dependencies and resets.
#[derive(Debug, Clone)]
pub struct Version {
    components: IndexMap<String, Component>,
    spec: VersionSpec,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.components.iter().map(|(k, v)| (k, v.value())))
            .finish()
    }
}

impl IntoIterator for Version {
    type Item = (String, Component);
    type IntoIter = indexmap::map::IntoIter<String, Component>;
    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

impl<'a> IntoIterator for &'a Version {
    type Item = (&'a String, &'a Component);
    type IntoIter = indexmap::map::Iter<'a, String, Component>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Version {
    /// Parse a version string into a Version object.
    #[must_use]
    pub fn parse(value: &str, regex: &regex::Regex, version_spec: &VersionSpec) -> Option<Self> {
        let parsed = parse_raw_version(value, regex);
        if parsed.is_empty() {
            return None;
        }
        let version = version_spec.build(&parsed);
        Some(version)
    }

    /// Serialize the version using one of the given serialization patterns.
    ///
    /// Patterns that tried in order, except for incompatible patterns, which are attempted at last.
    ///
    /// # Errors
    /// If the version cannot be serialized using the first pattern.
    pub fn serialize<'a, K, V, S>(
        &self,
        serialize_version_patterns: impl IntoIterator<Item = &'a PythonFormatString>,
        ctx: &HashMap<K, V, S>,
    ) -> Result<String, SerializeError>
    where
        K: std::borrow::Borrow<str> + std::hash::Hash + Eq + std::fmt::Debug,
        V: AsRef<str> + std::fmt::Debug,
        S: std::hash::BuildHasher,
    {
        serialize_version(self, serialize_version_patterns, ctx)
    }

    /// Retrieve a component by its name.
    ///
    /// # Examples
    /// ```no_run
    /// let v: bumpversion::version::Version = todo!();
    /// if let Some(comp) = v.get("minor") {
    ///     println!("minor = {}", comp.value().unwrap());
    /// }
    /// ```
    pub fn get<Q>(&self, component: &Q) -> Option<&Component>
    where
        Q: ?Sized + std::hash::Hash + indexmap::Equivalent<String>,
    {
        self.components.get(component)
    }

    /// Iterate over the version components in order.
    #[must_use]
    pub fn iter(&self) -> indexmap::map::Iter<String, Component> {
        self.components.iter()
    }

    /// Iterate over names of non-optional components (those with explicit values).
    pub fn required_component_names(&self) -> impl Iterator<Item = &str> {
        self.iter()
            .filter(|(_, v)| v.value() != v.spec.optional_value.as_deref())
            .map(|(k, _)| k.as_str())
    }

    /// The version components that depend on the given component and are always incremented.
    fn always_incr_dependencies(&self) -> HashMap<&str, HashSet<&str>> {
        self.spec
            .components_to_always_increment
            .iter()
            .map(|comp_name| (comp_name.as_str(), self.spec.dependents(comp_name)))
            .collect()
    }

    /// The version components that always increment.
    fn increment_always_incr(&self) -> Result<HashMap<&str, Component>, BumpError> {
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
    fn always_increment(&self) -> Result<(HashMap<&str, Component>, HashSet<&str>), BumpError> {
        let values = self.increment_always_incr()?;
        let mut dependents = self.always_incr_dependencies();
        for (comp_name, value) in &values {
            if value == &self.components[*comp_name] {
                dependents.remove(comp_name);
            }
        }
        let unique_dependents: HashSet<&str> = dependents.values().flatten().copied().collect();
        Ok((values, unique_dependents))
    }

    /// Increase the value of the specified component.
    ///
    /// This will reset its dependents, and return a new `Version`.
    pub fn bump(&self, component: &str) -> Result<Self, BumpError> {
        if !self.components.contains_key(component) {
            return Err(BumpError::InvalidComponent(component.to_string()));
        }

        let mut new_components = self.components.clone();
        let (always_increment_values, mut components_to_reset) = self.always_increment()?;
        // dbg!(&always_increment_values, &components_to_reset);

        new_components.extend(
            always_increment_values
                .into_iter()
                .map(|(k, v)| (k.to_string(), v)),
        );

        let should_reset = components_to_reset.contains(component);
        if !should_reset {
            new_components.insert(component.to_string(), self.components[component].bump()?);
            let dependants = self.spec.dependents(component);
            components_to_reset.extend(dependants);
        }

        // dbg!(&new_components, &components_to_reset);

        for comp_name in components_to_reset {
            // dbg!(&comp_name);
            // dbg!(&self.components);
            let is_independent = self.components[comp_name].spec.independent == Some(true);
            if !is_independent {
                new_components.insert(comp_name.to_string(), self.components[comp_name].first());
                // *new_components.entry(comp_name.to_string()).or_default() =
                //     self.components[comp_name].first();
            }
        }

        Ok(Self {
            components: new_components,
            ..self.clone()
        })
    }
}

/// Specification of version components, dependencies, and auto-increment rules.
///
/// Defines the order of components, which depend on which, and which always increment.
#[derive(Debug, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct VersionSpec {
    components: VersionComponentConfigs,
    dependency_map: HashMap<String, Vec<String>>,
    components_to_always_increment: Vec<String>,
}

impl VersionSpec {
    /// Create a `VersionSpec` from component configurations.
    ///
    /// Builds the dependency map and identifies always-increment components.
    #[must_use]
    pub fn from_components(components: VersionComponentConfigs) -> Self {
        let mut dependency_map: HashMap<String, Vec<String>> = HashMap::new();
        // let mut previous_component: &String = components
        //     .keys()
        //     .next()
        //     .ok_or_else(|| eyre::eyre!("must have at least one component"))?;

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

        // for (comp_name, comp_config) in components.iter().skip(1) {
        for (previous_component, (comp_name, comp_config)) in
            components.keys().zip(components.iter().skip(1))
        {
            if comp_config.independent == Some(true) {
                continue;
            }
            if let Some(ref depends_on) = comp_config.depends_on {
                dependency_map
                    .entry(depends_on.clone())
                    .or_default()
                    .push(comp_name.clone());
            } else {
                dependency_map
                    .entry(previous_component.clone())
                    .or_default()
                    .push(comp_name.clone());
            }
            // previous_component = comp_name;
        }

        // dbg!(&components_to_always_increment);
        // dbg!(&dependency_map);

        Self {
            components,
            dependency_map,
            components_to_always_increment,
        }
    }

    /// Return the set of component names that transitively depend on `comp_name`.
    #[must_use]
    pub fn dependents(&self, comp_name: &str) -> HashSet<&str> {
        use std::collections::VecDeque;
        let mut stack: VecDeque<&String> = self
            .dependency_map
            .get(comp_name)
            .map(|deps| deps.iter())
            .unwrap_or_default()
            .collect();
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
                    stack.push_front(dep);
                }
            }
        }
        visited
    }

    /// Build a `Version` instance from raw parsed values.
    ///
    /// # Arguments
    /// * `raw_components` - Mapping from component names to parsed string values.
    #[must_use]
    pub fn build(&self, raw_components: &RawVersion) -> Version {
        let components = self
            .components
            .iter()
            .map(|(comp_name, comp_config)| {
                let comp_value = raw_components.get(comp_name.as_str()).copied();
                let component = Component::new(comp_value, comp_config.clone());
                (comp_name.to_string(), component)
            })
            .collect();
        Version {
            components,
            spec: self.clone(),
        }
    }
}

/// Errors that can occur when serializing a `Version`.
#[derive(thiserror::Error, Debug)]
pub enum SerializeError {
    /// No provided pattern could serialize the given version.
    #[error("version {version} has no valid formats")]
    NoValidFormat {
        /// The version that failed to serialize.
        version: Box<Version>,
        /// List of attempted (index, format pattern) pairs.
        formats: Vec<(usize, PythonFormatString)>,
    },
    /// A required argument for formatting was missing.
    #[error(transparent)]
    MissingArgument(#[from] crate::f_string::MissingArgumentError),
}

/// Attempts to serialize a version with the given serialization format.
///
/// - valid serialization patterns are those that are renderable with the given context
/// - formats that contain all required components are preferred
/// - the shortest valid serialization pattern is used
/// - if two patterns are equally short, the first one is used
/// - if no valid serialization pattern is found, an error is raised
fn serialize_version<'a, K, V, S>(
    version: &Version,
    serialize_patterns: impl IntoIterator<Item = &'a PythonFormatString>,
    ctx: &HashMap<K, V, S>,
) -> Result<String, SerializeError>
where
    K: std::borrow::Borrow<str> + std::hash::Hash + Eq + std::fmt::Debug,
    V: AsRef<str> + std::fmt::Debug,
    S: std::hash::BuildHasher,
{
    tracing::debug!(?version, "serializing");

    let ctx: HashMap<&str, &str> = ctx
        .iter()
        .map(|(k, v)| (k.borrow(), v.as_ref()))
        .chain(version.iter().map(|(k, v)| (k.as_str(), v.as_ref())))
        .collect();

    let required_component_names: HashSet<_> = version.required_component_names().collect();

    // TODO(roman): could also avoid allocation of indices when using stable sort?
    let mut patterns: Vec<(usize, &'a PythonFormatString)> =
        serialize_patterns.into_iter().enumerate().collect();

    patterns.sort_by_key(|(idx, pattern)| {
        let labels: HashSet<&str> = pattern.named_arguments().collect();
        let has_required_components = required_component_names.is_subset(&labels);
        let num_labels = labels.len();
        (std::cmp::Reverse(has_required_components), num_labels, *idx)
    });

    let (_, chosen_pattern) =
        patterns
            .first()
            .copied()
            .ok_or_else(|| SerializeError::NoValidFormat {
                version: Box::new(version.clone()),
                formats: patterns
                    .into_iter()
                    .map(|(idx, format_string)| (idx, format_string.clone()))
                    .collect(),
            })?;

    tracing::debug!(format = ?chosen_pattern, "serialization format");
    let serialized = chosen_pattern.format(&ctx, true)?;
    tracing::debug!(serialized, "serialized");

    Ok(serialized)
}

/// Parse a version string into a dictionary of the parts and values using a regular expression.
///
/// # Errors
/// If the `parse_pattern` is not a valid regular expression.
fn parse_raw_version<'a>(version: &'a str, pattern: &'a regex::Regex) -> RawVersion<'a> {
    if version.is_empty() {
        tracing::warn!("version string is empty");
        return RawVersion::default();
    }

    tracing::debug!(version, ?pattern, "parsing version");

    let Some(matches) = pattern.captures(version) else {
        tracing::debug!(?pattern, ?version, "pattern does not parse current version",);
        return RawVersion::default();
    };

    let parsed: RawVersion = pattern
        .capture_names()
        .flatten()
        .filter_map(|name| matches.name(name).map(|value| (name, value.as_str())))
        .collect();

    tracing::debug!(?parsed, "parsed version");

    parsed
}

// /// Parse a version string into a Version object.
// #[must_use]
// pub fn parse_version(
//     value: &str,
//     regex: &regex::Regex,
//     version_spec: &VersionSpec,
// ) -> Option<Version> {
//     let parsed = parse_raw_version(value, regex);
//     if parsed.is_empty() {
//         return None;
//     }
//     let version = version_spec.build(&parsed);
//     Some(version)
// }

#[cfg(test)]
mod tests {
    use color_eyre::eyre;
    use similar_asserts::assert_eq as sim_assert_eq;

    #[test]
    fn test_parse_raw_version() -> eyre::Result<()> {
        crate::tests::init();

        let parse_regex = regex::Regex::new(r"(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)")?;
        sim_assert_eq!(
            super::parse_raw_version("2.1.3", &parse_regex),
            [("major", "2"), ("minor", "1"), ("patch", "3")]
                .into_iter()
                .collect::<super::RawVersion>(),
        );
        Ok(())
    }
}
