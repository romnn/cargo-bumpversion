use crate::{
    config::{Config, FileChange, FileConfig, InputFile, VersionComponentConfigs},
    f_string::OwnedPythonFormatString,
    version::compat::Version,
};
use color_eyre::eyre::{self, Context};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A file to modify in a configured way
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfiguredFile {
    pub path: PathBuf,
    pub file_change: FileChange,
    // pub version_config: VersionConfig,
}

impl ConfiguredFile {
    pub fn new(
        path: PathBuf,
        file_change: FileChange,
        // version_config: &VersionConfig,
        search: Option<&str>,
        replace: Option<&str>,
    ) -> Self {
        // let replacement: Option<&str> = [
        //     replace,
        //     file_change.replace.as_deref(),
        //     version_config.replace.as_deref(),
        // ]
        // .into_iter()
        // .filter_map(|v| v)
        // .next();

        // let mut merged_file_change = FileChange {
        //     search: search.map(ToString::to_string),
        //     replace: replace.map(ToString::to_string),
        //     ..FileChange::default()
        // };
        // merged_file_change.merge_with(&file_change);
        // merged_file_change.merge_with(&FileChange {
        //     // TODO: should file change also store a regex?
        //     parse_pattern: Some(version_config.parse_version_regex.as_str().to_string()),
        //     serialize_patterns: version_config.serialize_version_patterns.clone(),
        //     search: version_config.search.clone(),
        //     replace: version_config.replace.clone(),
        //     ..FileChange::default() // TODO: empty?
        // });
        // merged_file_change.merge_with(&FileChange::defaults());

        let search = search.unwrap_or(file_change.search.as_str());
        let replace = replace.unwrap_or(file_change.replace.as_str());

        // let file_change = FileChange{
        //     parse_pattern: file_change.parse_pattern.or(version_config.parse_regex),
        //     serialize_patterns: file_change.serialize_patterns.or(version_config.serialize_patterns),
        //     search: search.or(file_change.search).or(version_config.search),
        //     replace: replacement,
        //     regex: file_change.regex.or(Some(false)),
        //     ignore_missing_version: file_change.ignore_missing_version.unwrap_or(false),
        //     ignore_missing_file: file_change.ignore_missing_file.unwrap_or(false),
        //     filename: file_change.filename,
        //     glob: file_change.glob,
        //     key_path: file_change.key_path,
        //     // include_bumps=file_change.include_bumps,
        //     // exclude_bumps=file_change.exclude_bumps,
        //     ..FileChange::default()
        // };

        Self {
            path,
            file_change,
            // version_config: version_config.clone(),
        }
    }
    // file_change: FileChange,
    //     version_config: VersionConfig,
    //     search: Optional[str] = None,
    //     replace: Optional[str] = None,
    // ) -> None:
    //     replacements = [replace, file_change.replace, version_config.replace]
    //     replacement = next((r for r in replacements if r is not None), "")
    //     self.file_change = FileChange(
    //         parse=file_change.parse or version_config.parse_regex.pattern,
    //         serialize=file_change.serialize or version_config.serialize_formats,
    //         search=search or file_change.search or version_config.search,
    //         replace=replacement,
    //         regex=file_change.regex or False,
    //         ignore_missing_version=file_change.ignore_missing_version or False,
    //         ignore_missing_file=file_change.ignore_missing_file or False,
    //         filename=file_change.filename,
    //         glob=file_change.glob,
    //         key_path=file_change.key_path,
    //         include_bumps=file_change.include_bumps,
    //         exclude_bumps=file_change.exclude_bumps,
    //     )
    //     self.version_config = VersionConfig(
    //         self.file_change.parse,
    //         self.file_change.serialize,
    //         self.file_change.search,
    //         self.file_change.replace,
    //         version_config.part_configs,
    //     )
    //     self._newlines: Optional[str] = None
}

/// Does the search pattern match any part of the contents?
fn contains_pattern(contents: &str, search_pattern: &regex::Regex) -> bool {
    let matches = search_pattern.captures_iter(contents);
    let Some(m) = matches.into_iter().next() else {
        return false;
    };
    let Some(m) = m.iter().next().flatten() else {
        return false;
    };
    let line_num = contents[..m.start()].chars().filter(|c| *c == '\n').count() + 1;
    tracing::info!(
        "found {:?} at line {}: {:?}",
        search_pattern.as_str(),
        line_num,
        m.as_str(),
    );
    true
}

/// Replace version in file
pub fn replace_version<'a, K, V>(
    before: &'a str,
    file_change: &'a FileChange,
    current_version: &'a Version,
    new_version: &'a Version,
    ctx: &'a HashMap<K, V>,
    // dry_run: bool,
) -> eyre::Result<std::borrow::Cow<'a, str>>
where
    K: std::borrow::Borrow<str> + std::hash::Hash + Eq + std::fmt::Debug,
    V: AsRef<str> + std::fmt::Debug,
{
    // we need to update the version because each file may serialize versions differently
    let current_version_serialized =
        current_version.serialize(&file_change.serialize_version_patterns, &ctx)?;
    let new_version_serialized =
        new_version.serialize(&file_change.serialize_version_patterns, &ctx)?;

    let ctx: HashMap<&str, &str> = ctx
        .into_iter()
        .map(|(k, v)| (k.borrow(), v.as_ref()))
        .chain([
            ("current_version", current_version_serialized.as_str()),
            ("new_version", new_version_serialized.as_str()),
        ])
        .collect();

    let search_regex = file_change.search_pattern(&ctx)?;
    let replace = OwnedPythonFormatString::parse(&file_change.replace)?;
    let replacement = replace
        .format(&ctx, true)
        .wrap_err_with(|| eyre::eyre!("invalid replace format string"))?;

    // TODO(roman): i don't think we need to check if the change pattern is present?
    // // does the file contain the change pattern?
    // let contains_change_pattern: bool = {
    //     if contains_pattern(&before, &search_regex) {
    //         return Ok(true);
    //     }
    //
    //     // The `search` pattern did not match, but the original supplied
    //     // version number (representing the same version component values) might
    //     // match instead. This is probably the case if environment variables are used.
    //     let file_uses_global_search_pattern = file_change.search == version_config.search;
    //
    //     let pattern = regex::RegexBuilder::new(regex::escape(version.original)).build()?;
    //
    //     if file_uses_global_search_pattern && contains_pattern(&before, &pattern) {
    //         // The original version is present, and we're not looking for something
    //         // more specific -> this is accepted as a match
    //         return Ok(true);
    //     }
    //
    //     Ok(false)
    // }?;
    //
    // if !contains_change_pattern {
    //     if file_change.ignore_missing_version {
    //         tracing::warn!("did not find {:?} in file {path:?}", search_regex.as_str());
    //     } else {
    //         eyre::bail!("did not find {:?} in file {path:?}", search_regex.as_str());
    //     }
    //     return Ok(());
    // }

    let after = search_regex.replace_all(&before, replacement);
    Ok(after)
}

/// Replace version in file
pub fn replace_version_in_file<K, V>(
    path: &Path,
    file_change: &FileChange,
    current_version: &Version,
    new_version: &Version,
    ctx: &HashMap<K, V>,
    dry_run: bool,
) -> eyre::Result<()>
where
    K: std::borrow::Borrow<str> + std::hash::Hash + Eq + std::fmt::Debug,
    V: AsRef<str> + std::fmt::Debug,
{
    tracing::info!(
        file = ?path,
        search = ?file_change.search,
        replace = ?file_change.replace,
        "update file",
    );

    if !path.is_file() {
        if file_change.ignore_missing_file {
            tracing::info!(?path, "file not found");
            return Ok(());
        }
        eyre::bail!("file not found {:?}", path);
    }

    let before = std::fs::read_to_string(path)?;
    let after = replace_version(&before, file_change, current_version, new_version, ctx)?;
    if before == after {
        tracing::warn!(?path, "no change after version replacement");
        // TODO(roman): can we also not do this?
        // && current_version.original {
        // og_context = deepcopy(context)
        // og_context["current_version"] = current_version.original
        // search_for_og, _ = self.file_change.get_search_pattern(og_context)
        // file_content_after = search_for_og.sub(replace_with, file_content_before)
        // return Ok(());
    };

    // log_changes(self.file_change.filename, file_content_before, file_content_after, dry_run)

    let label_existing = format!("{path:?} (before)");
    let label_new = format!("{path:?} (after)");
    let diff = similar_asserts::SimpleDiff::from_str(&before, &after, &label_existing, &label_new);

    if dry_run {
        println!("{diff}");
    } else {
        todo!("write");
        use std::io::Write;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(false)
            .truncate(true)
            .open(path)?;
        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(after.as_bytes())?;
        writer.flush()?;
    }
    Ok(())
}

/// Resolve the files, searching and replacing values according to the FileConfig
pub fn resolve<'a>(
    files: &'a [(PathBuf, &'a FileChange)],
    // version_config: &VersionConfig,
    search: Option<&str>,
    replace: Option<&str>,
) -> Vec<ConfiguredFile> {
    files
        .into_iter()
        .map(|(file, config)| {
            ConfiguredFile::new(
                file.to_path_buf(),
                (*config).clone(),
                // version_config,
                search,
                replace,
            )
        })
        .collect()
}

pub struct InputPath {
    path: PathBuf,
    is_glob: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error(transparent)]
    Pattern(#[from] glob::PatternError),
    #[error(transparent)]
    Glob(#[from] glob::GlobError),
}

#[derive(thiserror::Error, Debug)]
#[error("io error for {path:?}")]
pub struct IoError {
    #[source]
    source: std::io::Error,
    path: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Glob(#[from] GlobError),
    #[error(transparent)]
    Io(#[from] IoError),
}

/// Return a list of file configurations that match the glob pattern
fn resolve_glob_files(
    pattern: &str,
    exclude_patterns: &[String],
) -> Result<Vec<PathBuf>, GlobError> {
    let options = glob::MatchOptions {
        case_sensitive: false,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };
    let included: HashSet<PathBuf> = glob::glob_with(pattern, options)?
        .map(|entry| entry.map(|path| path.to_path_buf()))
        .collect::<Result<_, _>>()?;

    let excluded: HashSet<PathBuf> = exclude_patterns
        .iter()
        .map(|pattern| glob::glob_with(pattern, options))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(|pattern| pattern.into_iter())
        .map(|entry| entry.map(|path| path.to_path_buf()))
        .collect::<Result<_, _>>()?;

    Ok(included.difference(&excluded).cloned().collect())
}

pub type FileMap = IndexMap<PathBuf, Vec<FileChange>>;

/// Return a map of filenames to file configs, expanding any globs
pub fn resolve_files_from_config<'a>(
    config: &mut Config,
    parts: &VersionComponentConfigs,
    base_dir: Option<&Path>,
) -> Result<FileMap, Error> {
    let files = config.files.drain(..);
    let new_files: Vec<_> = files
        .into_iter()
        .map(|(file, file_config)| {
            let new_files = match file {
                InputFile::GlobPattern {
                    pattern,
                    exclude_patterns,
                } => resolve_glob_files(&pattern, exclude_patterns.as_deref().unwrap_or_default()),
                InputFile::Path(path) => Ok(vec![path.clone()]),
            }?;

            let global = config.global.clone();
            let file_change = FileChange::new(file_config, parts);
            Ok(new_files
                .into_iter()
                .map(|file| {
                    if file.is_absolute() {
                        Ok(file)
                    } else if let Some(base_dir) = base_dir {
                        let file = base_dir.join(&file);
                        file.canonicalize()
                            .map_err(|source| IoError { source, path: file })
                    } else {
                        Ok(file)
                    }
                })
                .map(move |file| file.map(|file| (file, file_change.clone()))))
        })
        .collect::<Result<_, Error>>()?;

    let new_files = new_files
        .into_iter()
        .flat_map(|new_files| new_files)
        .try_fold(
            IndexMap::<PathBuf, Vec<FileChange>>::new(),
            |mut acc, res| {
                let (file, config) = res?;
                acc.entry(file).or_default().push(config);
                Ok::<_, Error>(acc)
            },
        )?;
    Ok(new_files)
}

/// Return a list of files to modify
pub fn files_to_modify<'a>(
    config: &'a Config,
    // file_map: &'a FileMap<'a>,
    file_map: &'a FileMap,
    // ) -> impl Iterator<Item = (&'a PathBuf, &'a Vec<&'a FileChange>)> {
) -> impl Iterator<Item = (&'a PathBuf, &'a Vec<FileChange>)> {
    let excluded_paths_from_config: HashSet<&'a PathBuf> = config
        .global
        .excluded_paths
        .as_deref()
        .unwrap_or_default()
        .iter()
        .collect();

    let included_paths_from_config: HashSet<&'a PathBuf> = config
        .global
        .included_paths
        .as_deref()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let included_files: HashSet<&'a PathBuf> = file_map
        .keys()
        .collect::<HashSet<&'a PathBuf>>()
        .difference(&excluded_paths_from_config)
        .copied()
        .collect();

    let included_files: HashSet<&'a PathBuf> = included_paths_from_config
        .union(&included_files)
        .copied()
        .collect();

    included_files
        .into_iter()
        .cloned()
        .filter_map(move |file| file_map.get_key_value(&file))
}
