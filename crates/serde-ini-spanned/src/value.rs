use crate::spanned::{Span, Spanned};
use crate::{
    parse::{Config, Item, Parse, ParseState, Parser},
    Error,
};
// use crate::{lines::Lines, Config, Error, Item, Parse, Parser};
use indexmap::{Equivalent, IndexMap};
use std::hash::Hash;

#[derive(thiserror::Error, Debug)]
#[error("invalid boolean: {0:?}")]
pub struct InvalidBooleanError(pub String);

/// Return a boolean value translating from other types if necessary
///
/// adopted from https://github.com/python/cpython/blob/main/Lib/configparser.py#L634
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

// pub trait Cast {
//     fn get_int(&self, key: &str) -> Result<Option<Spanned<i32>>, std::num::ParseIntError>;
//
//     fn get_float(&self, key: &str) -> Result<Option<Spanned<f64>>, std::num::ParseFloatError>;
//
//     fn get_bool(&self, key: &str) -> Result<Option<Spanned<bool>>, InvalidBooleanError>;
// }

pub type RawSection = IndexMap<Spanned<String>, Spanned<String>>;

// #[derive(Debug, Clone, Default, PartialEq, Eq)]
// pub struct Sections(IndexMap<Spanned<String>, Section>);

// impl Sections {
//     pub fn drain<R>(&mut self, range: R) -> indexmap::map::Drain<'_, Spanned<String>, Section>
//     where
//         R: std::ops::RangeBounds<usize>,
//     {
//         self.0.drain(range)
//     }
//
//     pub fn keys(&self) -> indexmap::map::Keys<'_, Spanned<String>, Section> {
//         self.0.keys()
//     }
//
//     pub fn entry(
//         &mut self,
//         key: Spanned<String>,
//     ) -> indexmap::map::Entry<'_, Spanned<String>, Section> {
//         self.0.entry(key)
//     }
//
//     pub fn remove<Q>(&mut self, key: &Q) -> Option<Section>
//     where
//         Q: ?Sized + Hash + Equivalent<Spanned<String>>,
//     {
//         self.0.shift_remove(key)
//     }
//
//     pub fn get<Q>(&self, key: &Q) -> Option<&Section>
//     where
//         Q: ?Sized + Hash + Equivalent<Spanned<String>>,
//     {
//         self.0.get(key)
//     }
//
//     pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Section>
//     where
//         Q: ?Sized + Hash + Equivalent<Spanned<String>>,
//     {
//         self.0.get_mut(key)
//     }
// }

// impl std::ops::IndexMut<&str> for Sections {
//     fn index_mut(&mut self, index: &str) -> &mut Self::Output {
//         self.get_mut(index).unwrap()
//     }
// }
//
// impl std::ops::Index<&str> for Sections {
//     type Output = Section;
//
//     fn index(&self, index: &str) -> &Self::Output {
//         self.get(index).unwrap()
//     }
// }

// impl FromIterator<(Spanned<String>, Section)> for Sections {
//     fn from_iter<T: IntoIterator<Item = (Spanned<String>, Section)>>(iter: T) -> Self {
//         Sections(iter.into_iter().collect())
//     }
// }
//
// impl IntoIterator for Sections {
//     type Item = (Spanned<String>, Section);
//     type IntoIter = indexmap::map::IntoIter<Spanned<String>, Section>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         self.0.into_iter()
//     }
// }

// impl<'a> IntoIterator for &'a Sections {
//     type Item = (&'a Spanned<String>, &'a Section);
//     type IntoIter = indexmap::map::Iter<'a, Spanned<String>, Section>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         self.0.iter()
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    inner: RawSection,
    name: Spanned<String>,
}

impl Default for Section {
    fn default() -> Self {
        Self {
            inner: IndexMap::default(),
            name: Spanned::dummy("".to_string()),
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

impl Section {
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

    pub fn set(&mut self, key: Spanned<String>, value: Spanned<String>) -> Option<Spanned<String>> {
        self.inner.insert(key, value)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>>,
    {
        self.inner.get_mut(key)
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>>,
    {
        self.inner.get(key)
    }

    pub fn has_option<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>>,
    {
        self.inner.contains_key(key)
    }

    pub fn remove_option<Q>(&mut self, key: &Q) -> Option<Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>>,
    {
        self.inner.shift_remove(key)
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
    // sections: Sections,
    defaults: Section,
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

// impl std::ops::IndexMut<&str> for Value {
//     fn index_mut(&mut self, index: &str) -> &mut Self::Output {
//         &mut self.sections[index]
//     }
// }

#[derive(Clone, Copy)]
pub struct SectionProxy<'a> {
    pub name: &'a Spanned<String>,
    section: &'a RawSection,
    // section: &'a str,
    // sections: &'a Sections,
    defaults: Option<&'a Section>,
    // defaults: Option<&'a RawSection>,
}

impl<'a> std::fmt::Debug for SectionProxy<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.section, f)
    }
}

impl<'a> PartialEq<RawSection> for SectionProxy<'a> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(self.section, other)
    }
}

impl<'a> PartialEq<RawSection> for &'a SectionProxy<'a> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(self.section, other)
    }
}

impl<'a> std::ops::Index<&str> for SectionProxy<'a> {
    type Output = Spanned<String>;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

pub struct SectionProxyMut<'a> {
    pub name: &'a Spanned<String>,
    section: &'a mut RawSection,
    // section: &'a str,
    // sections: &'a mut Sections,
    defaults: Option<&'a mut Section>,
    // defaults: Option<&'a mut RawSection>,
}

impl<'a> std::fmt::Debug for SectionProxyMut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.section, f)
    }
}

impl<'a> PartialEq<RawSection> for SectionProxyMut<'a> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(self.section, other)
    }
}

impl<'a> PartialEq<RawSection> for &'a SectionProxyMut<'a> {
    fn eq(&self, other: &RawSection) -> bool {
        std::cmp::PartialEq::eq(self.section, other)
    }
}

// impl ClearSpans for SectionProxyMut {
//     fn clear_spans(&mut self) {
//
//     }
// }

impl<'a> std::ops::Index<&str> for SectionProxyMut<'a> {
    type Output = Spanned<String>;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<'a> std::ops::IndexMut<&str> for SectionProxyMut<'a> {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

impl<'a> SectionProxyMut<'a> {
    pub fn section_mut(&mut self) -> &mut RawSection {
        // &mut self.sections[self.section]
        &mut self.section
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

    pub fn set(&mut self, key: Spanned<String>, value: Spanned<String>) -> Option<Spanned<String>> {
        self.section_mut().insert(key, value)
        // self.section.insert(key, value)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>>,
    {
        self.section_mut().get_mut(key)
        // self.section.get_mut(key)
    }

    pub fn get_mut_owned<Q>(mut self, key: &'a Q) -> Option<&mut Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>>,
    {
        self.section.get_mut(key)
        // self.section.get_mut(key)
    }

    pub fn remove_option<Q>(&mut self, key: &Q) -> Option<Spanned<String>>
    where
        Q: ?Sized + Hash + Equivalent<Spanned<String>>,
    {
        self.section_mut().shift_remove(key)
        // self.section.shift_remove(key)
    }
}

macro_rules! impl_section_proxy {
    ($name:ident) => {
        impl<'a> $name<'a> {
            pub fn span(&self) -> &Span {
                &self.name.span
            }

            pub fn section(&self) -> &RawSection {
                &self.section
                // &self.sections[self.section]
            }

            // pub fn get_test2<'b: 'a, Q>(&'b self, key: &'a Q) -> Option<&'a Spanned<String>>
            // where
            //     Q: ?Sized + Hash + Equivalent<Spanned<String>>,
            // {
            //     self.section.get(key)
            // }

            // pub fn get<'b>(&'b self, key: Spanned<String>) -> Option<&'a Spanned<String>> {
            //     self.section.get(&key)
            // }

            // pub fn get<'b, Q>(self, key: &'b Q) -> Option<&Spanned<String>>
            // where
            //     'a: 'b,
            //     Q: ?Sized + Hash + Equivalent<Spanned<String>>,
            // {
            //     self.section().inner.get(key)
            // }

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
            pub fn get<'b, Q>(&self, key: &'b Q) -> Option<&Spanned<String>>
            where
                'a: 'b,
                Q: ?Sized + Hash + Equivalent<Spanned<String>>,
            {
                // try:
                //     d = self._unify_values(section, vars)
                // except NoSectionError:
                //     if fallback is _UNSET:
                //         raise
                //     else:
                //         return fallback
                // option = self.optionxform(option)
                // try:
                //     value = d[option]
                // except KeyError:
                //     if fallback is _UNSET:
                //         raise NoOptionError(option, section)
                //     else:
                //         return fallback
                //
                // if raw or value is None:
                //     return value
                // else:
                //     return self._interpolation.before_get(self, section, option, value, d)

                self.section().get(key)
            }

            pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&Spanned<String>, &Spanned<String>)>
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>>,
            {
                self.section().get_key_value(key)
            }

            pub fn key_span<Q>(&self, key: &Q) -> Option<&Span>
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>>,
            {
                self.section().get_key_value(key).map(|(k, _)| &k.span)
            }

            pub fn get_owned<Q>(self, key: &'a Q) -> Option<&Spanned<String>>
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>>,
            {
                self.section.get(key)
            }

            pub fn has_option<Q>(&self, key: &Q) -> bool
            where
                Q: ?Sized + Hash + Equivalent<Spanned<String>>,
            {
                // self.section().contains_key(key)
                // self.section(section)
                self.section
                    .get(key)
                    .or(self
                        .defaults
                        .as_ref()
                        .and_then(|defaults| defaults.get(key)))
                    .is_some()
                // python configparser also checks the default section
                // .and_then(|section| {
                // section.get_owned(option)
                // let SectionProxy {
                //     section, defaults, ..
                // } = section;
                // section.get_owned(option).or(self.defaults.get(option))
                // .and_then(|defaults| defaults.inner.get(option)))
                // })
                // .is_some()

                // self.section.contains_key(key)
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

// impl<'a> AsRef<SectionProxy<'a>> for SectionProxyMut<'a> {
//     fn as_ref(&self) -> &SectionProxy<'a> {
//
//     }
// }
// impl<'a> std::ops::Deref for SectionProxyMut<'a> {
//     type Target = SectionProxy<'a>;
//     fn deref(&self) -> &Self::Target {
//         SectionProxy {
//             name: &*self.name,
//             section: &*self.section,
//             defaults: &*self.defaults,
//         }
//     }
// }

// impl<'a> SectionProxy<'a> {
//     pub fn span(&self) -> &Span {
//         &self.name.span
//     }
//
//     pub fn get<Q>(&self, key: &Q) -> Option<&Spanned<String>>
//     where
//         Q: ?Sized + Hash + Equivalent<Spanned<String>>,
//     {
//         self.section.get(key)
//     }
//
//     pub fn has_option<Q>(&self, key: &Q) -> bool
//     where
//         Q: ?Sized + Hash + Equivalent<Spanned<String>>,
//     {
//         self.section.contains_key(key)
//     }
//
//     pub fn get_int(&self, key: &str) -> Result<Option<Spanned<i32>>, std::num::ParseIntError> {
//         self.get(key)
//             .map(|value| {
//                 value
//                     .as_ref()
//                     .parse()
//                     .map(|int| Spanned::new(value.span.clone(), int))
//             })
//             .transpose()
//     }
//
//     pub fn get_float(&self, key: &str) -> Result<Option<Spanned<f64>>, std::num::ParseFloatError> {
//         self.get(key)
//             .map(|value| {
//                 value
//                     .as_ref()
//                     .parse()
//                     .map(|float| Spanned::new(value.span.clone(), float))
//             })
//             .transpose()
//     }
//
//     pub fn get_bool(&self, key: &str) -> Result<Option<Spanned<bool>>, InvalidBooleanError> {
//         self.get(key)
//             .map(|value| {
//                 convert_to_boolean(value.as_ref())
//                     .map(|boolean| Spanned::new(value.span.clone(), boolean))
//             })
//             .transpose()
//     }
// }

impl<'a> std::fmt::Display for SectionProxy<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Section({})", self.name.as_ref())
    }
}

// impl<'a> std::ops::Index<&str> for Value {
//     type Output = SectionProxy<'a>;
//
//     fn index(&'a self, index: &str) -> &'a Self::Output {
//         &SectionProxy {
//             section: &self.sections[index],
//             defaults: &self.defaults,
//         }
//     }
// }

impl Value {
    pub fn new(mut sections: Sections, defaults: Section) -> Self {
        for (name, section) in sections.iter_mut() {
            section.name = name.clone();
        }
        Self { sections, defaults }
    }

    pub fn remove_section(&mut self, name: &str) -> Option<Section> {
        self.sections.shift_remove(name)
    }

    pub fn remove_option(&mut self, section: &str, option: &str) -> Option<Spanned<String>> {
        self.section_mut(section)
            .and_then(|mut section| section.remove_option(option))
    }

    pub fn defaults(&self) -> &Section {
        &self.defaults
    }

    pub fn defaults_mut(&mut self) -> &mut Section {
        &mut self.defaults
    }

    // pub fn defaults(&self) -> SectionProxy<'_> {
    //     SectionProxy {
    //         name: &self.defaults.name,
    //         section_name: &self.defaults.inner,
    //         sections: &self.defaults.inner,
    //         // section: &self.defaults.inner,
    //         defaults: None,
    //     }
    // }
    //
    // pub fn defaults_mut(&mut self) -> SectionProxyMut<'_> {
    //     SectionProxyMut {
    //         name: &self.defaults.name,
    //         section: &mut self.defaults.inner,
    //         defaults: None,
    //     }
    // }

    pub fn has_section(&self, section: &str) -> bool {
        self.section(section).is_some()
    }

    pub fn section_names(&self) -> impl Iterator<Item = &Spanned<String>> {
        self.sections.keys().into_iter()
    }

    // pub fn sections(&self) -> impl Iterator<Item = SectionProxy<'_>> {
    //     self.sections.iter().map(|(_, section)| SectionProxy {
    //         name: &section.name,
    //         // sections: &self.sections.inner,
    //         // section: name,
    //         section: &section.inner,
    //         defaults: Some(&self.defaults),
    //         // defaults: Some(&self.defaults.inner),
    //     })
    // }

    // pub fn sections_mut<'a>(&mut self) -> impl Iterator<Item = SectionProxyMut<'_>> + use<'_> {
    //     let test = &mut self.defaults;
    //     let sections = &mut self.sections;
    //     sections
    //         .values_mut()
    //         // .chain(std::iter::repeat())
    //         .map(|section| SectionProxyMut {
    //             name: &section.name,
    //             // sections: &self.sections.inner,
    //             // section: name,
    //             section: &mut section.inner,
    //             defaults: Some(test),
    //             // defaults: Some(&self.defaults.inner),
    //         })
    // }

    pub fn section(&self, name: &str) -> Option<SectionProxy<'_>> {
        // SectionProxy {
        //     name: &section.name,
        //     sections: &self.sections.inner,
        //     section: name,
        //     defaults: Some(&self.defaults),
        //     // defaults: Some(&self.defaults.inner),
        // }
        self.sections
            // .0
            .get(name)
            // .get_key_value(name)
            .map(|section| SectionProxy {
                name: &section.name,
                // sections: &self.sections.inner,
                // section: name,
                section: &section.inner,
                defaults: Some(&self.defaults),
                // defaults: Some(&self.defaults.inner),
            })
    }

    pub fn section_mut(&mut self, name: &str) -> Option<SectionProxyMut<'_>> {
        self.sections
            // .0
            .get_mut(name)
            .map(|section| SectionProxyMut {
                name: &section.name,
                // section: &mut section.inner,
                section: &mut section.inner,
                defaults: Some(&mut self.defaults),
            })
    }

    // Check for the existence of a given option in a given section.
    //
    // If the specified `section` is None or an empty string, DEFAULT is
    // assumed. If the specified `section` does not exist, returns False."""
    pub fn has_option(&self, section: &str, option: &str) -> bool {
        // if not section or section == self.default_section:
        //     option = self.optionxform(option)
        //     return option in self._defaults
        // elif section not in self._sections:
        //     return False
        // else:
        //     option = self.optionxform(option)
        //     return (option in self._sections[section]
        //             or option in self._defaults)
        self.section(section)
            .map(|section| section.has_option(option))
            .unwrap_or(false)
        // self.has_option(section, option)
        // self.section(section)
        //     // python configparser also checks the default section
        //     .and_then(|section| {
        //         // section.get_owned(option)
        //         // let SectionProxy {
        //         //     section, defaults, ..
        //         // } = section;
        //         section.get_owned(option).or(self.defaults.get(option))
        //         // .and_then(|defaults| defaults.inner.get(option)))
        //     })
        //     .is_some()
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

    pub fn get<'a>(&'a self, section: &str, option: &'a str) -> Option<&'a Spanned<String>> {
        // let section: SectionProxy<'a> = self.section(section)?;
        // section.ge(option)
        self.section(section)
            .and_then(|section| section.get_owned(option))
    }

    pub fn get_mut<'a>(
        &'a mut self,
        section: &str,
        option: &'a str,
    ) -> Option<&'a mut Spanned<String>> {
        self.section_mut(section)
            .and_then(move |mut section| section.get_mut_owned(option))
    }
}

pub fn from_reader(reader: impl std::io::BufRead) -> Result<Value, Error> {
    let mut parser = Parser::new(reader, Config::default());
    let mut out = Value::default();
    let mut current_section: Option<Spanned<String>> = None;
    let mut current_option: Option<Spanned<String>> = None;
    let mut state = ParseState::default();

    while let Some(item) = parser.parse_next(&mut state).transpose() {
        let item = item?;
        match item {
            Spanned {
                inner: Item::Empty | Item::Comment { .. },
                ..
            } => {
                current_option = None;
            }
            Spanned {
                inner: Item::Section { name },
                span,
            } => {
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
                let section = match current_section {
                    Some(ref name) => &mut out.sections.entry(name.clone()).or_default(),
                    None => &mut out.defaults,
                };
                if let Some(mut current_value) =
                    current_option.as_ref().and_then(|op| section.get_mut(op))
                {
                    current_value.inner += "\n";
                    current_value.inner += &value;
                    current_value.span.end = span.end;
                }
            }
            Spanned {
                inner: Item::Value { key, value },
                ..
            } => {
                let section = match current_section {
                    Some(ref name) => &mut out.sections.entry(name.clone()).or_default(),
                    None => &mut out.defaults,
                };
                current_option = Some(key.clone());
                section.set(key, value);
            }
        }
    }
    Ok(out)
}

pub fn from_str(value: &str) -> Result<Value, Error> {
    let cursor = std::io::Cursor::new(value);
    let reader = std::io::BufReader::new(cursor);
    from_reader(reader)
}
