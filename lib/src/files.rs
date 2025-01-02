use crate::{
    config::{Config, FileChange, FileConfig, InputFile, Parts},
    version::compat::VersionConfig,
};
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
        version_config: &VersionConfig,
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

        let mut merged_file_change = FileChange {
            search: search.map(ToString::to_string),
            replace: replace.map(ToString::to_string),
            ..FileChange::default()
        };
        merged_file_change.merge_with(&file_change);
        merged_file_change.merge_with(&FileChange {
            // TODO: should file change also store a regex?
            parse_pattern: Some(version_config.parse_version_regex.as_str().to_string()),
            serialize_patterns: version_config.serialize_version_patterns.clone(),
            search: version_config.search.clone(),
            replace: version_config.replace.clone(),
            ..FileChange::default() // TODO: empty?
        });
        merged_file_change.merge_with(&FileChange::defaults());

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

/// Resolve the files, searching and replacing values according to the FileConfig
pub fn resolve<'a>(
    // files: Vec<FileChange>,
    // files: impl IntoIterator<Item = &'a (&'a PathBuf, &'a Vec<&'a FileChange>)>,
    files: &'a [(PathBuf, &'a FileChange)],
    version_config: &VersionConfig,
    search: Option<&str>,
    replace: Option<&str>,
) -> Vec<ConfiguredFile> {
    files
        .into_iter()
        .map(|(file, config)| {
            ConfiguredFile::new(
                file.to_path_buf(),
                (*config).clone(),
                version_config,
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

// pub type FileMap<'a> = HashMap<PathBuf, Vec<&'a FileChange>>;
pub type FileMap = HashMap<PathBuf, Vec<FileChange>>;

/// Return a map of filenames to file configs, expanding any globs
pub fn resolve_files_from_config<'a>(
    config: &mut Config,
    parts: &Parts,
    // files: impl IntoIterator<Item = &'a (InputFile, FileChange)>,
    // ) -> Result<FileMap<'a>, GlobError> {
) -> Result<FileMap, GlobError> {
    let files = config.files.drain(..);
    // .cloned()
    // .map(|(input_file, file_config)| {
    //     let global = config.global.clone();
    //     let file_change = ;
    //     (input_file, file_change)
    // })
    // .collect();

    let new_files = files
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
            // (input_file, file_change)
            Ok(new_files
                .into_iter()
                .map(move |file| (file, file_change.clone())))
        })
        .collect::<Result<Vec<_>, GlobError>>()?;

    let new_files = new_files.into_iter().flat_map(|new_files| new_files).fold(
        HashMap::<PathBuf, Vec<FileChange>>::new(),
        |mut acc, (file, config)| {
            acc.entry(file).or_default().push(config);
            acc
        },
    );
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
