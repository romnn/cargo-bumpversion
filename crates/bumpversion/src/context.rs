//! Helpers for constructing template contexts used in version serialization and tags.
//! Context construction for template rendering of version strings and tags.
//!
//! Builds a map of variables from environment, VCS info, and version data.
use crate::{vcs::TagAndRevision, version::Version};
use std::collections::HashMap;

/// A simple environment mapping of variable names to values.
/// A mapping of variable names to their string values.
pub type Env = HashMap<String, String>;

/// Return a dict of the environment with keys prefixed with `$`
fn prefixed_env() -> impl Iterator<Item = (String, String)> {
    std::env::vars().map(|(k, v)| (format!("${k}"), v))
}

/// The default context for rendering messages and tags
fn base_context(
    tag_and_revision: Option<&TagAndRevision>,
) -> impl Iterator<Item = (String, String)> {
    let tag = tag_and_revision
        .as_ref()
        .and_then(|v| v.tag.clone())
        .unwrap_or_default();
    let revision = tag_and_revision
        .as_ref()
        .and_then(|v| v.revision.clone())
        .unwrap_or_default();

    [
        ("now".to_string(), chrono::Local::now().to_rfc3339()),
        ("utcnow".to_string(), chrono::Utc::now().to_rfc3339()),
    ]
    .into_iter()
    .chain(prefixed_env())
    .chain([
        ("tool".to_string(), "git".to_string()),
        ("commit_sha".to_string(), tag.commit_sha),
        (
            "distance_to_latest_tag".to_string(),
            tag.distance_to_latest_tag.to_string(),
        ),
        ("current_version".to_string(), tag.current_version),
        ("current_tag".to_string(), tag.current_tag),
        ("branch_name".to_string(), revision.branch_name),
        ("short_branch_name".to_string(), revision.short_branch_name),
        (
            "repository_root".to_string(),
            revision.repository_root.to_string_lossy().to_string(),
        ),
        ("dirty".to_string(), tag.dirty.to_string()),
    ])
    .chain([
        ("#".to_string(), "#".to_string()),
        (";".to_string(), ";".to_string()),
    ])
}

/// Return the context for rendering messages and tags
pub fn get_context(
    tag_and_revision: Option<&TagAndRevision>,
    current_version: Option<&Version>,
    new_version: Option<&Version>,
    current_version_serialized: Option<&str>,
    new_version_serialized: Option<&str>,
) -> impl Iterator<Item = (String, String)> {
    base_context(tag_and_revision)
        .chain([
            (
                "current_version".to_string(),
                current_version_serialized.unwrap_or_default().to_string(),
            ),
            (
                "new_version".to_string(),
                new_version_serialized.unwrap_or_default().to_string(),
            ),
        ])
        .chain(
            current_version
                .map(|version| version.clone().into_iter())
                .unwrap_or_default()
                .map(|(part, value)| {
                    (
                        format!("current_{part}"),
                        value.value().unwrap_or_default().to_string(),
                    )
                }),
        )
        .chain(
            new_version
                .map(|version| version.clone().into_iter())
                .unwrap_or_default()
                .map(|(part, value)| {
                    (
                        format!("new_{part}"),
                        value.value().unwrap_or_default().to_string(),
                    )
                }),
        )
}
