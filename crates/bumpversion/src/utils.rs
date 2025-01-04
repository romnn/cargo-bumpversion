use color_eyre::eyre;
use std::path::Path;

#[inline]
pub fn contains<'a>(
    s: &'a str,
    pattern: &'a str,
) -> Result<Option<regex::Match<'a>>, regex::Error> {
    let re = regex::Regex::new(pattern)?;
    Ok(re.find(s))
}

pub(crate) fn create_dirs<P: AsRef<Path>>(path: P) -> eyre::Result<()> {
    let path = path.as_ref();
    let dir = if path.extension().is_some() {
        path.parent()
            .ok_or(eyre::eyre!("no parent for {:?}", path))?
    } else {
        path
    };
    std::fs::create_dir_all(dir)?;
    Ok(())
}
