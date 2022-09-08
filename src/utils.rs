#[inline]
pub fn contains<'a>(
    s: &'a str,
    pattern: &'a str,
) -> Result<Option<regex::Match<'a>>, regex::Error> {
    let re = regex::Regex::new(pattern)?;
    Ok(re.find(s))
    // .ok_or(anyhow::anyhow!("{} does not contain {}", s, pattern));
    // Ok(())
}
