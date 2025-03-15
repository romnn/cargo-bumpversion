use crate::diagnostics::DiagnosticExt;
use crate::spanned::{Span, Spanned};
use crate::{
    Error, ParseConfig,
    parse::{Item, Parse, ParseState, Parser},
};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use indexmap::{Equivalent, IndexMap};
use std::hash::Hash;

#[derive(thiserror::Error, Debug)]
#[error("invalid boolean: {0:?}")]
pub struct InvalidBooleanError(pub String);

/// Return a boolean value translating from other types if necessary
///
/// adopted from <https://github.com/python/cpython/blob/main/Lib/configparser.py#L634>
pub fn convert_to_boolean(value: &str) -> Result<bool, InvalidBooleanError> {
    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "1" | "yes" | "true" | "on" => Ok(true),
        "0" | "no" | "false" | "off" => Ok(false),
        _ => Err(InvalidBooleanError(value)),
    }
}

pub trait ClearSpans {
    fn clear_spans(&mut self);

    fn cleared_spans(mut self) -> Self
    where
        Self: Sized,
    {
        self.clear_spans();
        self
    }
}

pub type RawSection = IndexMap<Spanned<String>, Spanned<String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    inner: RawSection,
    pub name: Spanned<String>,
}

impl Default for Section {
    fn default() -> Self {
        Self {
            inner: IndexMap::default(),
            name: Spanned::dummy(String::new()),
        }
    }
}

impl ClearSpans for Section {
    fn clear_spans(&mut self) {
        self.name.span = Span::default();
        self.inner = self
            .inner
            .drain(..)
            .map(|(k, v)| {
                (
                    Spanned::dummy(k.into_inner()),
                    Spanned::dummy(v.into_inner()),
                )
            })
            .collect();
    }
}

struct DisplayRepr<T>(T);

impl<T> std::fmt::Debug for DisplayRepr<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl<T> std::fmt::Display for DisplayRepr<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::fmt::Display for Section {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.inner.iter().map(|(k, v)| (k.as_ref(), v.as_ref())))
            .finish()
    }
}

impl Section {
    #[must_use]
    pub fn with_name(mut self, name: Spanned<String>) -> Self {
        self.name = name;
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[must_use]
    pub fn lowercase_keys(self) -> Self {
        self.into_iter()
            .map(|(mut k, v)| {
                k.inner = k.inner.to_lowercase();
                (k, v)
            })
            .collect()
    }

    #[must_use]
    pub fn span(&self) -> &Span {
        &self.name.span
    }

    pub fn drain<R>(
        &mut self,
        range: R,
    ) -> indexmap::map::Drain<'_, Spanned<String>, Spanned<String>>
    where
        R: std::ops::RangeBounds<usize>,
    {
        self.inner.drain(range)
    }

    #[must_use]
    pub fn iter(
        &self,
    ) -> indexmap::map::Iter<'_, Spanned<std::string::String>, Spanned<std::string::String>> {
        self.inner.iter()
    }

    #[must_use]
    pub fn options(&self) -> indexmap::map::Keys<'_, Spanned<String>, Spanned<String>> {
        self.keys()
    }

    #[must_use]
    pub fn keys(&self) -> indexmap::map::Keys<'_, Spanned<String>, Spanned<String>> {
        self.inner.keys()
    }

    pub fn set(
        &mut self,
        mut key: Spanned<String>,
        value: Spanned<String>,
    ) -> Option<Spanned<String>> {
        key.inner = key.inner.to_lowercase();
        self.inner.insert(key, value)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key = key.lowercase();
        self.inner.get_mut(&key)
    }

    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&Spanned<String>, &Spanned<String>)>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key = key.lowercase();
        self.inner.get_key_value(&key)
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key = key.lowercase();
        self.inner.get(&key)
    }

    pub fn has_option<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key = key.lowercase();
        self.inner.contains_key(&key)
    }

    pub fn remove_option<Q>(&mut self, key: &Q) -> Option<Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key = key.lowercase();
        self.inner.shift_remove(&key)
    }

    pub fn get_int(&self, key: &str) -> Result<Option<Spanned<i32>>, std::num::ParseIntError> {
        self.get(key)
            .map(|value| {
                value
                    .as_ref()
                    .parse()
                    .map(|int| Spanned::new(value.span.clone(), int))
            })
            .transpose()
    }

    pub fn get_float(&self, key: &str) -> Result<Option<Spanned<f64>>, std::num::ParseFloatError> {
        self.get(key)
            .map(|value| {
                value
                    .as_ref()
                    .parse()
                    .map(|float| Spanned::new(value.span.clone(), float))
            })
            .transpose()
    }

    pub fn get_bool(&self, key: &str) -> Result<Option<Spanned<bool>>, InvalidBooleanError> {
        self.get(key)
            .map(|value| {
                convert_to_boolean(value.as_ref())
                    .map(|boolean| Spanned::new(value.span.clone(), boolean))
            })
            .transpose()
    }
}

impl std::ops::IndexMut<&str> for Section {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

impl std::ops::Index<&str> for Section {
    type Output = Spanned<String>;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl From<Vec<(Spanned<String>, Spanned<String>)>> for Section {
    fn from(value: Vec<(Spanned<String>, Spanned<String>)>) -> Self {
        value.into_iter().collect()
    }
}

impl<const N: usize> From<[(Spanned<String>, Spanned<String>); N]> for Section {
    fn from(value: [(Spanned<String>, Spanned<String>); N]) -> Self {
        value.into_iter().collect()
    }
}

impl FromIterator<(Spanned<String>, Spanned<String>)> for Section {
    fn from_iter<T: IntoIterator<Item = (Spanned<String>, Spanned<String>)>>(iter: T) -> Self {
        Section {
            inner: iter.into_iter().collect(),
            ..Self::default()
        }
    }
}

impl IntoIterator for Section {
    type Item = (Spanned<String>, Spanned<String>);
    type IntoIter = indexmap::map::IntoIter<Spanned<String>, Spanned<String>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a> IntoIterator for &'a Section {
    type Item = (&'a Spanned<String>, &'a Spanned<String>);
    type IntoIter = indexmap::map::Iter<'a, Spanned<String>, Spanned<String>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

pub type Sections = IndexMap<Spanned<String>, Section>;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct Value {
    sections: Sections,
    defaults: Section,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(
                self.sections
                    .iter()
                    .map(|(k, v)| (k.as_ref(), DisplayRepr(v))),
            )
            .entries(
                self.defaults
                    .iter()
                    .map(|(k, v)| (k.as_ref(), DisplayRepr(v))),
            )
            .finish()
    }
}

impl ClearSpans for Value {
    fn clear_spans(&mut self) {
        self.sections = self
            .sections
            .drain(..)
            .map(|(k, v)| (Spanned::dummy(k.into_inner()), v.cleared_spans()))
            .collect();
        self.defaults.clear_spans();
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct SectionProxy<'a> {
    pub section: &'a Section,
    defaults: Option<&'a Section>,
}

impl AsRef<Section> for SectionProxy<'_> {
    fn as_ref(&self) -> &Section {
        self.section
    }
}

impl std::fmt::Debug for SectionProxy<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.section, f)
    }
}

impl PartialEq<RawSection> for SectionProxy<'_> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(&self.section.inner, other)
    }
}

impl<'a> PartialEq<RawSection> for &'a SectionProxy<'a> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(&self.section.inner, other)
    }
}

impl std::ops::Index<&str> for SectionProxy<'_> {
    type Output = Spanned<String>;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

pub struct SectionProxyMut<'a> {
    section: &'a mut Section,
    defaults: Option<&'a Section>,
}

impl std::fmt::Debug for SectionProxyMut<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.section, f)
    }
}

impl PartialEq<RawSection> for SectionProxyMut<'_> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(&self.section.inner, other)
    }
}

impl<'a> PartialEq<RawSection> for &'a SectionProxyMut<'a> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(&self.section.inner, other)
    }
}

impl std::ops::Index<&str> for SectionProxyMut<'_> {
    type Output = Spanned<String>;

    fn index(&self, index: &str) -> &Self::Output {
        self.get_by_ref(index).unwrap()
    }
}

impl std::ops::IndexMut<&str> for SectionProxyMut<'_> {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

impl<'a> SectionProxyMut<'a> {
    pub fn section_mut(&mut self) -> &mut RawSection {
        &mut self.section.inner
    }

    pub fn replace_with(&mut self, section: Section) {
        *self.section = section;
    }

    pub fn drain<R>(
        &mut self,
        range: R,
    ) -> indexmap::map::Drain<'_, Spanned<String>, Spanned<String>>
    where
        R: std::ops::RangeBounds<usize>,
    {
        self.section_mut().drain(range)
    }

    pub fn set(
        &mut self,
        mut key: Spanned<String>,
        value: Spanned<String>,
    ) -> Option<Spanned<String>> {
        key.inner = key.inner.to_lowercase();
        self.section_mut().insert(key, value)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key: String = key.lowercase();
        self.section_mut().get_mut(&key)
    }

    pub fn get_mut_owned<Q>(self, key: &'a Q) -> Option<&'a mut Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key: String = key.lowercase();
        self.section.get_mut(&key)
    }

    pub fn remove_option<Q>(&mut self, key: &Q) -> Option<Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
    {
        let key: String = key.lowercase();
        self.section_mut().shift_remove(&key)
    }
}

pub trait Lowercase {
    fn lowercase(&self) -> String;
}

impl Lowercase for &str {
    fn lowercase(&self) -> String {
        self.to_lowercase()
    }
}

impl Lowercase for str {
    fn lowercase(&self) -> String {
        self.to_lowercase()
    }
}

impl Lowercase for String {
    fn lowercase(&self) -> String {
        self.to_lowercase()
    }
}

impl Lowercase for Spanned<String> {
    fn lowercase(&self) -> String {
        self.as_ref().to_lowercase()
    }
}

impl Lowercase for &Spanned<String> {
    fn lowercase(&self) -> String {
        self.as_ref().to_lowercase()
    }
}

pub static EMPTY_SECTION: std::sync::LazyLock<Section> = std::sync::LazyLock::new(Section::default);

pub type Keys<'a> = std::iter::Chain<
    indexmap::map::Keys<'a, Spanned<String>, Spanned<String>>,
    indexmap::map::Keys<'a, Spanned<String>, Spanned<String>>,
>;

pub type OwnedKeys<'a> = std::iter::Chain<
    indexmap::map::Keys<'a, Spanned<String>, Spanned<String>>,
    indexmap::map::Keys<'a, Spanned<String>, Spanned<String>>,
>;

macro_rules! impl_section_proxy {
    ($name:ident) => {
        impl<'a> $name<'a> {
            #[must_use]
            pub fn span(&self) -> &Span {
                &self.section.name.span
            }

            #[must_use]
            pub fn section(&self) -> &RawSection {
                &self.section.inner
            }

            #[deprecated]
            pub fn options_by_ref(&self) -> Keys<'_> {
                self.keys_by_ref()
            }

            pub fn options(self) -> Keys<'a> {
                self.keys()
            }

            pub fn iter(
                self,
            ) -> std::iter::Chain<
                indexmap::map::Iter<'a, Spanned<std::string::String>, Spanned<std::string::String>>,
                indexmap::map::Iter<'a, Spanned<std::string::String>, Spanned<std::string::String>>,
            > {
                // TODO(remove the unwrap here)
                self.defaults.unwrap().iter().chain(self.section.iter())
            }

            pub fn keys(self) -> OwnedKeys<'a> {
                // let empty_section: &Section = &*EMPTY_SECTION;
                let section_keys: indexmap::map::Keys<'a, Spanned<String>, Spanned<String>> =
                    self.section.keys();
                let default_section_keys: indexmap::map::Keys<
                    'a,
                    Spanned<String>,
                    Spanned<String>,
                > = self.defaults.unwrap().inner.keys();

                section_keys.chain(default_section_keys)
            }

            pub fn keys_by_ref(&self) -> Keys<'_> {
                let empty_section: &Section = &*EMPTY_SECTION;
                let default_section: Option<&Section> = self.defaults.as_deref();

                self.section
                    .keys()
                    .chain(default_section.unwrap_or(empty_section).inner.keys())
            }

            pub fn get<Q>(self, key: &Q) -> Option<&'a Spanned<String>>
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
            {
                let key: String = key.lowercase();
                self.section.get(&key)
            }

            /// Get an option value for a given section.
            ///
            /// If `vars` is provided, it must be a dictionary. The option is looked up
            /// in `vars` (if provided), `section`, and in `DEFAULTSECT` in that order.
            /// If the key is not found and `fallback` is provided, it is used as
            /// a fallback value. `None` can be provided as a `fallback` value.
            ///
            /// If interpolation is enabled and the optional argument `raw` is False,
            /// all interpolations are expanded in the return values.
            ///
            /// Arguments `raw`, `vars`, and `fallback` are keyword only.
            ///
            /// The section DEFAULT is special.
            pub fn get_by_ref<'b, Q>(&self, key: &'b Q) -> Option<&Spanned<String>>
            where
                'a: 'b,
                Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
            {
                let key: String = key.lowercase();
                self.section().get(&key)
            }

            pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&Spanned<String>, &Spanned<String>)>
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
            {
                let key: String = key.lowercase();
                self.section.get_key_value(&key)
            }

            pub fn key_span<Q>(&self, key: &Q) -> Option<&Span>
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
            {
                let key: String = key.lowercase();
                self.section.get_key_value(&key).map(|(k, _)| &k.span)
            }

            pub fn get_owned<Q>(self, key: &'a Q) -> Option<&'a Spanned<String>>
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
            {
                let key: String = key.lowercase();
                self.section.get(&key)
            }

            pub fn has_option<Q>(&self, key: &Q) -> bool
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>> + Lowercase,
            {
                let key: String = key.lowercase();
                self.section
                    .get(key.as_str())
                    .or(self
                        .defaults
                        .as_ref()
                        .and_then(|defaults| defaults.get(key.as_str())))
                    .is_some()
            }

            pub fn get_int(
                self,
                key: &str,
            ) -> Result<Option<Spanned<i32>>, std::num::ParseIntError> {
                self.get(key)
                    .map(|value| {
                        value
                            .as_ref()
                            .parse()
                            .map(|int| Spanned::new(value.span.clone(), int))
                    })
                    .transpose()
            }

            pub fn get_float(
                self,
                key: &str,
            ) -> Result<Option<Spanned<f64>>, std::num::ParseFloatError> {
                self.get(key)
                    .map(|value| {
                        value
                            .as_ref()
                            .parse()
                            .map(|float| Spanned::new(value.span.clone(), float))
                    })
                    .transpose()
            }

            pub fn get_bool(self, key: &str) -> Result<Option<Spanned<bool>>, InvalidBooleanError> {
                self.get(key)
                    .map(|value| {
                        convert_to_boolean(value.as_ref())
                            .map(|boolean| Spanned::new(value.span.clone(), boolean))
                    })
                    .transpose()
            }
        }
    };
}

impl_section_proxy!(SectionProxy);
impl_section_proxy!(SectionProxyMut);

impl std::fmt::Display for SectionProxy<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Section({})", self.section.name.as_ref())
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("missing section: {0:?}")]
pub struct NoSectionError(pub String);

impl Value {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && self.defaults.is_empty()
    }

    pub fn replace_section(&mut self, key: Spanned<String>, section: Section) -> Section {
        let old_section = self.sections.entry(key).or_default();
        std::mem::replace(old_section, section)
    }

    #[must_use]
    pub fn with_defaults(defaults: Section) -> Self {
        Self {
            sections: Sections::default(),
            defaults: defaults.lowercase_keys(),
        }
    }

    pub fn add_section(
        &mut self,
        name: Spanned<String>,
        section: impl Into<Section>,
    ) -> Option<Section> {
        let section: Section = section.into();
        let mut section = section.lowercase_keys();
        section.name = name.clone();
        self.sections.insert(name, section)
    }

    pub fn remove_section(&mut self, name: &str) -> Option<Section> {
        self.sections.shift_remove(name)
    }

    pub fn remove_option(&mut self, section: &str, option: &str) -> Option<Spanned<String>> {
        self.section_mut(section)
            .and_then(|mut section| section.remove_option(option))
    }

    #[must_use]
    pub fn defaults(&self) -> &Section {
        &self.defaults
    }

    pub fn defaults_mut(&mut self) -> &mut Section {
        &mut self.defaults
    }

    #[must_use]
    pub fn has_section(&self, section: &str) -> bool {
        self.section(section).is_some()
    }

    pub fn section_names(&self) -> impl Iterator<Item = &Spanned<String>> {
        self.sections.keys()
    }

    pub fn clear(&mut self) {
        self.sections.clear();
    }

    /// Remove a section from the value.
    ///
    /// The default section is never returned because it cannot be removed.
    pub fn pop(&mut self) -> Option<Section> {
        let first_section_name = self.sections.keys().next()?.clone();
        self.remove_section(&first_section_name)
    }

    #[must_use]
    pub fn section<'a>(&'a self, name: &str) -> Option<SectionProxy<'a>> {
        self.sections.get(name).map(|section| SectionProxy {
            section,
            defaults: Some(&self.defaults),
        })
    }

    pub fn section_mut(&mut self, name: &str) -> Option<SectionProxyMut<'_>> {
        self.sections.get_mut(name).map(|section| SectionProxyMut {
            section,
            defaults: Some(&mut self.defaults),
        })
    }

    // Check for the existence of a given option in a given section.
    //
    // If the specified `section` is None or an empty string, DEFAULT is
    // assumed. If the specified `section` does not exist, returns False."""
    #[must_use]
    pub fn has_option(&self, section: &str, option: &str) -> bool {
        self.section(section)
            .is_some_and(|section| section.has_option(option))
    }

    pub fn options<'a>(&'a self, section: &str) -> Keys<'a> {
        self.section(section)
            .map(SectionProxy::options)
            .unwrap_or_default()
    }

    pub fn set(
        &mut self,
        section: &str,
        option: Spanned<String>,
        value: Spanned<String>,
    ) -> Result<Option<Spanned<String>>, NoSectionError> {
        let mut section = self
            .section_mut(section)
            .ok_or(NoSectionError(section.to_string()))?;
        Ok(section.set(option, value))
    }

    #[must_use]
    pub fn get<'a>(&'a self, section: &str, option: &'a str) -> Option<&'a Spanned<String>> {
        self.section(section)
            .and_then(|section| section.get_owned(option))
    }

    pub fn get_mut<'a>(
        &'a mut self,
        section: &str,
        option: &'a str,
    ) -> Option<&'a mut Spanned<String>> {
        self.section_mut(section)
            .and_then(move |section| section.get_mut_owned(option))
    }
    pub fn get_int(
        &self,
        section: &str,
        option: &str,
    ) -> Result<Option<Spanned<i32>>, std::num::ParseIntError> {
        self.section(section)
            .and_then(|section| section.get_int(option).transpose())
            .transpose()
    }

    pub fn get_float(
        &self,
        section: &str,
        option: &str,
    ) -> Result<Option<Spanned<f64>>, std::num::ParseFloatError> {
        self.section(section)
            .and_then(|section| section.get_float(option).transpose())
            .transpose()
    }

    pub fn get_bool(
        &self,
        section: &str,
        option: &str,
    ) -> Result<Option<Spanned<bool>>, InvalidBooleanError> {
        self.section(section)
            .and_then(|section| section.get_bool(option).transpose())
            .transpose()
    }
}

fn get_section<'a>(
    current_section: &Option<Spanned<String>>,
    out: &'a mut Value,
) -> &'a mut Section {
    match current_section {
        Some(name) => out.sections.entry(name.clone()).or_default(),
        None => &mut out.defaults,
    }
}

fn finalize_continuation_value(current_option: &Option<Spanned<String>>, section: &mut Section) {
    if let Some(current_value) = current_option.as_ref().and_then(|op| section.get_mut(op)) {
        // finalize previous
        crate::parse::trim_trailing_whitespace(&mut current_value.inner, &mut current_value.span);
    }
}

/// Parse a `serde_ini_spanned::Value` from a buffered reader.
///
/// # Errors
/// - When custom delimiters configured in `Options` are not valid.
/// - When an IO error is encountered while reading.
/// - When the reader contains invalid INI syntax.
pub fn from_reader<F: PartialEq + Copy>(
    reader: impl std::io::BufRead,
    options: Options,
    file_id: F,
    diagnostics: &mut Vec<Diagnostic<F>>,
) -> Result<Value, Error> {
    let mut parser = Parser::new(reader, options.parser_config)?;
    let mut out = Value::default();
    let mut current_section: Option<Spanned<String>> = None;
    let mut current_option: Option<Spanned<String>> = None;
    let mut state = ParseState::default();

    while let Some(items) = parser.parse_next(&mut state).transpose() {
        let items = items?;
        for item in items {
            match item {
                Spanned {
                    inner: Item::Comment { .. },
                    ..
                } => {
                    // ignore
                }
                Spanned {
                    inner: Item::Section { name },
                    span,
                } => {
                    // finalize previous
                    let section = get_section(&current_section, &mut out);
                    finalize_continuation_value(&current_option, section);

                    // start new section
                    let section_name = Spanned::new(span, name);
                    let section = out.sections.entry(section_name.clone()).or_default();
                    section.name = section_name.clone();
                    current_section = Some(section_name);
                    current_option = None;
                }
                Spanned {
                    inner: Item::ContinuationValue { value },
                    span,
                } => {
                    let section = get_section(&current_section, &mut out);
                    if let Some(current_value) =
                        current_option.as_ref().and_then(|op| section.get_mut(op))
                    {
                        current_value.inner += "\n";
                        current_value.inner += &value;
                        current_value.span.end = span.end;
                    }
                }
                Spanned {
                    inner: Item::Value { mut key, value },
                    ..
                } => {
                    let section = get_section(&current_section, &mut out);
                    finalize_continuation_value(&current_option, section);

                    key.inner = key.inner.to_lowercase();
                    current_option = Some(key.clone());
                    let existing = section.get_key_value(&key);
                    if let Some((previous_key, _previous_value)) = existing {
                        let diagnostic = Diagnostic::warning_or_error(options.strict)
                            .with_message(format!("duplicate option `{key}`"))
                            .with_labels(vec![
                                Label::primary(file_id, key.span.clone())
                                    .with_message(format!("second use of option `{key}`")),
                                Label::secondary(file_id, previous_key.span.clone())
                                    .with_message(format!("first use of option `{previous_key}`")),
                            ]);
                        diagnostics.push(diagnostic);
                    }

                    if !(options.strict && existing.is_some()) {
                        section.set(key, value);
                    }
                }
            }
        }
    }

    let section = get_section(&current_section, &mut out);
    finalize_continuation_value(&current_option, section);
    Ok(out)
}

/// Options for parsing INI.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Options {
    /// Enable strict mode
    ///
    /// In strict mode, warnings are treated as errors.
    /// For example, duplicate options will result in an error.  
    pub strict: bool,
    pub parser_config: ParseConfig,
}

/// Parse a `serde_ini_spanned::Value` from a buffered reader.
///
/// # Errors
/// - When custom delimiters configured in `Options` are not valid.
/// - When the reader contains invalid INI syntax.
pub fn from_str<F: PartialEq + Copy>(
    value: &str,
    options: Options,
    file_id: F,
    diagnostics: &mut Vec<Diagnostic<F>>,
) -> Result<Value, Error> {
    let cursor = std::io::Cursor::new(value);
    let reader = std::io::BufReader::new(cursor);
    from_reader(reader, options, file_id, diagnostics)
}
