use indexmap::IndexMap;

pub type VersionComponentConfigs = IndexMap<String, VersionComponentSpec>;

/// Configuration of a version component.
///
/// This is used to read in the configuration from the bumpversion config file.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionComponentSpec {
    /// Is the component independent of the other components?
    pub independent: Option<bool>,

    /// The value that is optional to include in the version.
    ///
    /// - Defaults to first value in values or 0 in the case of numeric.
    /// - Empty string means nothing is optional.
    /// - `CalVer` components ignore this."""
    pub optional_value: Option<String>,

    /// The possible values for the component.
    ///
    /// If it and `calver_format` is None, the component is numeric.
    pub values: Vec<String>,

    /// The first value to increment from
    pub first_value: Option<String>,

    /// Should the component always increment, even if it is not necessary?
    pub always_increment: bool,

    /// The format string for a `CalVer` component
    pub calver_format: Option<String>,

    /// The name of the component this component depends on
    pub depends_on: Option<String>,
}

/// Make sure all version components are included
#[must_use] pub fn version_component_configs(config: &super::FinalizedConfig) -> VersionComponentConfigs {
    let parsing_groups = config
        .global
        .parse_version_pattern
        .capture_names()
        .flatten();
    let component_configs: VersionComponentConfigs = parsing_groups
        .map(|label| {
            use super::MergeWith;
            let is_independent = label.starts_with('$');
            let mut spec = match config.components.get(label) {
                Some(part) => part.clone(),
                None => VersionComponentSpec::default(),
            };
            spec.independent.merge_with(Some(&is_independent));
            (label.to_string(), spec)
        })
        .collect();
    component_configs
}
