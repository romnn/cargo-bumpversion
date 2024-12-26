use crate::spanned::{Span, Spanned};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_DELIMITERS: [char; 2] = ['=', ':'];
pub const DEFAULT_COMMENT_PREFIXES: [char; 2] = [';', '#'];

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Item {
    Empty,
    Section {
        name: String,
    },
    ContinuationValue {
        value: String,
    },
    Value {
        key: Spanned<String>,
        value: Spanned<String>,
    },
    Comment {
        text: String,
    },
}

#[derive(thiserror::Error, Debug, Clone, Eq, PartialEq)]
pub enum SyntaxError {
    SectionNotClosed {
        span: Span,
    },
    InvalidSectionName {
        span: Span,
    },
    EmptyOptionName {
        span: Span,
    },
    MissingAssignmentDelimiter {
        span: Span,
        assignment_delimiters: Vec<char>,
    },
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SectionNotClosed { span } => write!(f, r"section was not closed: missing ']'"),
            Self::InvalidSectionName { span } => write!(f, r"invalid section name: contains ']'"),
            Self::EmptyOptionName { span } => write!(f, r"empty option name"),
            Self::MissingAssignmentDelimiter {
                span,
                assignment_delimiters,
            } => write!(
                f,
                r"variable assignment missing one of: {}",
                assignment_delimiters
                    .into_iter()
                    .map(|d| format!("`{d}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl SyntaxError {
    pub fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Diagnostic<F> {
        match self {
            Self::SectionNotClosed { span } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![
                    Label::primary(file_id, span.clone()).with_message("missing `]`")
                ]),
            Self::InvalidSectionName { span } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![Label::primary(file_id, span.clone())
                    .with_message("section must not contain `]`")]),
            Self::EmptyOptionName { span } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![Label::primary(file_id, span.clone())
                    .with_message("option name must not be empty")]),
            Self::MissingAssignmentDelimiter {
                span,
                assignment_delimiters,
            } => Diagnostic::error()
                .with_message(self.to_string())
                .with_labels(vec![Label::primary(file_id, span.clone()).with_message(
                    format!(
                        "missing one of: {}",
                        assignment_delimiters
                            .iter()
                            .map(|d| format!("`{d}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                )]),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("syntax error: {0}")]
    Syntax(#[from] SyntaxError),
}

impl Error {
    pub fn to_diagnostics<F: Copy + PartialEq>(&self, file_id: F) -> Vec<Diagnostic<F>> {
        match self {
            Self::Io(_) => vec![],
            Self::Syntax(err) => vec![err.to_diagnostics(file_id)],
        }
    }
}

// pub jtruct Parser<T> {
//     input: T,
// }
//
// impl<T> Parser<T> {
//     pub fn new(input: T) -> Self {
//         Parser { input }
//     }
//
//     pub fn into_inner(self) -> T {
//         self.input
//     }
// }

// impl<'a> Parser<OkIter<std::str::Lines<'a>>> {
//     pub fn from_str(s: &'a str) -> Self {
//         Self::new(OkIter(s.lines()))
//     }
// }

// impl<R: std::io::BufRead> Parser<std::io::Lines<R>> {
//     pub fn from_bufread(r: R) -> Self {
//         Self::new(r.lines())
//     }
// }
//
// impl<R: std::io::Read> Parser<std::io::Lines<std::io::BufReader<R>>> {
//     pub fn from_read(r: R) -> Self {
//         Self::from_bufread(std::io::BufReader::new(r))
//     }
// }

// #[derive(Debug)]
// pub struct Parser<B>(lines::Lines<B>);

pub trait Parse {
    fn parse_next(&mut self, state: &mut ParseState) -> Result<Option<Spanned<Item>>, Error>;
}

// impl<B> std::iter::Iterator for Parser<B>
// where
//     B: std::io::BufRead,
// {

// fn compact_span(line: &str, span: Span) -> Span {
//     let Span { start, end } = span;
//     // start += line[start..end].char_indices().iter().take_while(|c| c.is_whitespace()).count();
//     let start = line
//         .char_indices()
//         .skip(start)
//         .find_map(|(offset, c)| {
//             if !c.is_whitespace() {
//                 Some(offset)
//             } else {
//                 None
//             }
//         })
//         .unwrap_or(start);
//     let end = line
//         .char_indices()
//         .skip(end)
//         .find_map(|(offset, c)| {
//             if !c.is_whitespace() {
//                 Some(offset)
//             } else {
//                 None
//             }
//         })
//         .unwrap_or(end);
//     // start = line[start..end].chars().iter().take_while(|c| c.is_whitespace()).count();
//     // while start < end {
//     Span { start, end }
// }

fn compact_span(line: &str, span: Span) -> Span {
    let Span { mut start, mut end } = span;
    // let start = line
    //     .chars()
    //     .enumerate()
    //     .skip(start)
    //     .find_map(|(pos, c)| if !c.is_whitespace() { Some(pos) } else { None })
    //     .unwrap_or(start);
    start += line[start..]
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();
    end -= line[start..end]
        .chars()
        .rev()
        .take_while(|c| c.is_whitespace())
        .count();
    // dbg!(&start, &end);
    // .enumerate()
    // .skip(start)
    // .find_map(|(pos, c)| if !c.is_whitespace() { Some(pos) } else { None })
    // .unwrap_or(start);

    // let end = line
    //     .chars()
    //     .enumerate()
    //     .skip(end)
    //     .find_map(|(pos, c)| {
    //         dbg!(&c);
    //         if !c.is_whitespace() {
    //             Some(pos)
    //         } else {
    //             None
    //         }
    //     })
    //     .unwrap_or(end);
    Span { start, end }
}

fn to_byte_span(line: &str, span: Span) -> Span {
    let start = line
        .char_indices()
        .nth(span.start)
        .map(|(offset, _)| offset)
        .unwrap_or(span.start);
    let end = line
        .char_indices()
        .nth(span.end)
        .map(|(offset, _)| offset)
        .unwrap_or(span.end);
    Span { start, end }
}

trait AddOffset {
    fn add_offset(self, offset: usize) -> Self;
}

impl AddOffset for Span {
    fn add_offset(self, offset: usize) -> Self {
        Self {
            start: self.start + offset,
            end: self.end + offset,
        }
    }
}

// impl<B> lines::Lines<B>
// where
//     B: std::io::BufRead,
// {
//     pub fn parse_next(&mut self) -> Result<Option<Spanned<Item>>, Error> {
//         let line = self.next().transpose()?;
//         let Some((offset, line)) = line else {
//             return Ok(None);
//         };
//         let mut span = compact_span(&line, 0..line.len());
//         // let mut span: Span = 0..line.len();
//
//         // let line = <Self as lines::Lines<std::io::BufRead>>::next(self)?;
//         // let line = line.trim();
//         dbg!(&line[span.clone()]);
//         // TODO: trim in place?
//         if line[span.clone()].starts_with('[') {
//             if line[span.clone()].ends_with(']') {
//                 span.start += 1;
//                 span.end -= 1;
//                 // let line = &line[1..line.len() - 1];
//                 // let line = &line[span];
//                 let byte_span = to_byte_span(&line, span.clone()).add_offset(offset);
//                 if line[span.clone()].contains(']') {
//                     Err(Error::Syntax(SyntaxError::InvalidSectionName {
//                         span: byte_span,
//                     }))
//                 } else {
//                     Ok(Some(Spanned::new(
//                         byte_span,
//                         Item::Section {
//                             name: line[span].into(),
//                         },
//                     )))
//                 }
//             } else {
//                 let byte_span = to_byte_span(&line, span.clone()).add_offset(offset);
//                 Err(Error::Syntax(SyntaxError::SectionNotClosed {
//                     span: byte_span,
//                 }))
//             }
//         } else if line[span.clone()].starts_with(';') || line[span.clone()].starts_with('#') {
//             span.start += 1;
//             let byte_span = to_byte_span(&line, span).add_offset(offset);
//             Ok(Some(Spanned::new(
//                 byte_span,
//                 Item::Comment { text: line.into() },
//             )))
//         } else if line[span.clone()].is_empty() {
//             let byte_span = to_byte_span(&line, span).add_offset(offset);
//             Ok(Some(Spanned::new(byte_span, Item::Empty)))
//         } else {
//             // find position of assignment delimiter
//             let equal_pos = line[span.clone()].chars().enumerate().find_map(|(idx, c)| {
//                 // if c == '=' {
//                 if '=' {
//                     Some(idx)
//                 } else {
//                     None
//                 }
//             });
//             let equal_pos = equal_pos.ok_or_else(|| {
//                 Error::Syntax(SyntaxError::MissingAssignmentDelimiter {
//                     span: to_byte_span(&line, span.clone()).add_offset(offset),
//                     assignment_delimiters: vec![],
//                 })
//             })?;
//             // if let Some(equal_pos) = equal_pos {
//
//             // dbg!(&line, &span, &equal_pos);
//
//             let key_span = Span {
//                 start: span.start,
//                 end: span.start + equal_pos,
//             };
//             let key_span = compact_span(&line, key_span);
//             // dbg!(&key_span);
//             let key = &line[key_span.clone()];
//             // dbg!(&key);
//
//             let value_span = Span {
//                 start: span.start + equal_pos + 1,
//                 end: span.end,
//             };
//             // dbg!(&value_span);
//             let value_span = compact_span(&line, value_span);
//             let value = &line[value_span.clone()];
//             // dbg!(&value);
//             Ok(Some(Spanned::new(
//                 to_byte_span(&line, span).add_offset(offset),
//                 Item::Value {
//                     key: Spanned::new(to_byte_span(&line, key_span).add_offset(offset), key.into()),
//                     value: Spanned::new(
//                         to_byte_span(&line, value_span).add_offset(offset),
//                         value.into(),
//                     ),
//                 },
//             )))
//             // let mut line = line.splitn(2, '=');
//             // if let Some(key) = line.next() {
//             //     let key = key.trim();
//             //     if let Some(value) = line.next() {
//             //         Ok(Some(Item::Value {
//             //             key: key.into(),
//             //             value: value.trim().into(),
//             //         }))
//             //     } else if key.is_empty() {
//             //         Ok(Some(Item::Empty))
//             //     } else {
//             //         Err(Error::Syntax(SyntaxError::MissingEquals))
//             //     }
//             // } else {
//             //     unreachable!()
//             // }
//             // } else {
//             //     let byte_span = to_byte_span(&line, span).add_offset(offset);
//             //     Ok(Some(Spanned::new(byte_span, Item::Empty)))
//             // }
//         }
//     }
// }

// impl<T> Parser<T> {
//     // fn parse_next<E>(line: Option<impl AsRef<str>>) -> Result<Option<Item>, Error<E>>
//     fn parse_next(line: Option<impl AsRef<str>>) -> Result<Option<Item>, Error>
// // where
//     //     E: std::fmt::Display,
//     {
//         let line = match line {
//             Some(line) => line,
//             None => return Ok(None),
//         };
//         let line = line.as_ref();
//
//         if line.starts_with('[') {
//             if line.ends_with(']') {
//                 let line = &line[1..line.len() - 1];
//                 if line.contains(']') {
//                     Err(Error::Syntax(SyntaxError::InvalidSectionName))
//                 } else {
//                     Ok(Some(Item::Section { name: line.into() }))
//                 }
//             } else {
//                 Err(Error::Syntax(SyntaxError::SectionNotClosed))
//             }
//         } else if line.starts_with(';') || line.starts_with('#') {
//             Ok(Some(Item::Comment { text: line.into() }))
//         } else {
//             // println!("line: {line}");
//             let mut line = line.splitn(2, '=');
//             // println!("line: {:?}", line.clone().into_iter().collect::<Vec<_>>());
//             if let Some(key) = line.next() {
//                 let key = key.trim();
//                 if let Some(value) = line.next() {
//                     Ok(Some(Item::Value {
//                         key: key.into(),
//                         value: value.trim().into(),
//                     }))
//                 } else if key.is_empty() {
//                     Ok(Some(Item::Empty))
//                 } else {
//                     Err(Error::Syntax(SyntaxError::MissingEquals))
//                 }
//             } else {
//                 unreachable!()
//             }
//         }
//     }
// }

// impl<E, S, T> Iterator for Parser<T>
// impl<S, T> Iterator for Parser<T>
// where
//     // E: std::fmt::Display,
//     S: AsRef<str>,
//     T: Iterator<Item = Result<S, Error>>,
// {
//     // type Item = Result<Item, Error<E>>;
//     type Item = Result<Item, Error>;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         let value = self.input.next().transpose(); // .map_err(Error::Inner);
//         value.and_then(|l| Self::parse_next(l)).transpose()
//     }
// }
//
// pub struct OkIter<I>(pub I);
//
// impl<T, I: Iterator<Item = T>> Iterator for OkIter<I> {
//     type Item = Result<T, std::convert::Infallible>;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         (self.0).next().map(Ok)
//     }
// }

#[derive(Debug)]
pub struct ParseState {
    // elements: HashSet<String>,
    current_section: HashMap<String, Vec<String>>,
    option_name: Option<String>,
    indent_level: usize,
    // current_indent_level: usize,
    // cursect : dict[str, str] | None = None
    // sectname : str | None = None
    // optname : str | None = None
    // lineno : int = 0
    // indent_level : int = 0
    // errors : list[ParsingError]

    // def __init__(self):
    //     self.elements_added = set()
    //     self.errors = list()
}

impl Default for ParseState {
    fn default() -> Self {
        Self {
            current_section: Default::default(),
            option_name: None,
            indent_level: 0,
        }
    }
}

// pub fn read(state: &mut ReadState) -> eyre::Result<()> {
//     Ok(())
// }

#[derive(Debug)]
pub struct Config {
    assignment_delimiters: Vec<char>,
    comment_prefixes: Vec<char>,
    allow_brackets_in_section_name: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            assignment_delimiters: vec!['=', ':'],
            comment_prefixes: vec!['#', ';'],
            allow_brackets_in_section_name: true,
        }
    }
}

#[derive(Debug)]
pub struct Parser<B> {
    config: Config,
    lines: crate::lines::Lines<B>,
}

impl<B> Parser<B> {
    pub fn new(buf: B, config: Config) -> Self {
        Self {
            lines: crate::lines::Lines::new(buf),
            config,
        }
    }
}

impl<B> Parse for Parser<B>
where
    B: std::io::BufRead,
{
    fn parse_next(&mut self, state: &mut ParseState) -> Result<Option<Spanned<Item>>, Error> {
        let line = self.lines.next().transpose()?;
        let Some((offset, line)) = line else {
            return Ok(None);
        };
        let mut span = compact_span(&line, 0..line.len());
        let current_indent_level = span.start;

        // st.cur_indent_level = first_nonspace.start() if first_nonspace else 0

        // check for prefix of line
        // prefixes = types.SimpleNamespace(
        //     full=tuple(comment_prefixes or ()), # ('#', ';')
        //     inline=tuple(inline_comment_prefixes or ()), # ()
        // )

        // dbg!(&state.option_name);
        // dbg!(&line[span.clone()]);

        if line[span.clone()].starts_with('[') {
            if line[span.clone()].ends_with(']') {
                span.start += 1;
                span.end -= 1;
                // let line = &line[1..line.len() - 1];
                // let line = &line[span];
                let byte_span = to_byte_span(&line, span.clone()).add_offset(offset);
                if !self.config.allow_brackets_in_section_name && line[span.clone()].contains(']') {
                    Err(Error::Syntax(SyntaxError::InvalidSectionName {
                        span: byte_span,
                    }))
                } else {
                    state.current_section.clear();
                    state.option_name = None;
                    Ok(Some(Spanned::new(
                        byte_span,
                        Item::Section {
                            name: line[span].into(),
                        },
                    )))
                }
            } else {
                let byte_span = to_byte_span(&line, span.clone()).add_offset(offset);
                Err(Error::Syntax(SyntaxError::SectionNotClosed {
                    span: byte_span,
                }))
            }
        } else if line[span.clone()].starts_with(';') || line[span.clone()].starts_with('#') {
            // comment
            // # empty line marks end of value
            // st.indent_level = sys.maxsize
            span.start += 1;
            let byte_span = to_byte_span(&line, span).add_offset(offset);
            Ok(Some(Spanned::new(
                byte_span,
                Item::Comment { text: line.into() },
            )))
        } else if line[span.clone()].is_empty() {
            state.option_name = None;
            let byte_span = to_byte_span(&line, span).add_offset(offset);
            Ok(Some(Spanned::new(byte_span, Item::Empty)))
        } else {
            // find position of assignment delimiter (e.g. '=')
            let assignment_delimiter_pos =
                line[span.clone()].chars().enumerate().find_map(|(idx, c)| {
                    if self.config.assignment_delimiters.iter().any(|d| *d == c) {
                        Some(idx)
                    } else {
                        None
                    }
                });

            // find position of comment (e.g. '#')
            let comment_pos = line[span.clone()].chars().enumerate().find_map(|(idx, c)| {
                if self.config.comment_prefixes.iter().any(|d| *d == c) {
                    Some(idx)
                } else {
                    None
                }
            });

            // check if continue
            if let Some(ref option_name) = state.option_name {
                // continuation line?
                let is_continue = !state.current_section.is_empty()
                    && assignment_delimiter_pos.is_none()
                    && current_indent_level > state.indent_level;

                println!(
                    "section={} option={} continuation={}",
                    "", // state.current_section.len(),
                    option_name,
                    is_continue
                );

                if is_continue {
                    // let Some(mut previous_value) = state.current_section.get_mut(option_name) else {
                    //     // raise MultilineContinuationError(fpname, st.lineno, line)
                    //     panic!("multi line continuation error");
                    // };
                    // value.push(line[span.clone()].to_string());

                    // let value_span = compact_span(&line, value_span);
                    // let value = &line[span.clone()];
                    // dbg!(&value);
                    // Ok(Some(Spanned::new(
                    //     to_byte_span(&line, span).add_offset(offset),
                    //     Item::Value {
                    //         key: Spanned::new(
                    //             to_byte_span(&line, key_span).add_offset(offset),
                    //             key.into(),
                    //         ),
                    //         value: Spanned::new(
                    //             to_byte_span(&line, value_span).add_offset(offset),
                    //             value.into(),
                    //         ),
                    //     },
                    // )))

                    let value = &line[span.clone()];

                    return Ok(Some(Spanned::new(
                        to_byte_span(&line, span).add_offset(offset),
                        Item::ContinuationValue {
                            // key: Spanned::new(to_byte_span(&line, key_span).add_offset(offset), key.into()),
                            value: value.into(),
                            // value: Spanned::new(
                            //     to_byte_span(&line, span).add_offset(offset),
                            //     value.into(),
                            // ),
                        },
                    )));
                    // let byte_span = to_byte_span(&line, span).add_offset(offset);
                    // return Ok(Some(Spanned::new(byte_span, Item::Empty)));
                }
            }

            let assignment_delimiter_pos = assignment_delimiter_pos.ok_or_else(|| {
                Error::Syntax(SyntaxError::MissingAssignmentDelimiter {
                    span: to_byte_span(&line, span.clone()).add_offset(offset),
                    assignment_delimiters: self.config.assignment_delimiters.clone(),
                })
            })?;
            // if let Some(equal_pos) = equal_pos {

            // dbg!(&line, &span, &equal_pos);

            let key_span = Span {
                start: span.start,
                end: span.start + assignment_delimiter_pos,
            };
            let key_span = compact_span(&line, key_span);
            // dbg!(&key_span);
            let key = &line[key_span.clone()];
            // dbg!(&key);

            let value_span = Span {
                start: span.start + assignment_delimiter_pos + 1,
                end: span.end.min(comment_pos.unwrap_or(usize::MAX)),
            };
            // dbg!(&value_span);
            let value_span = compact_span(&line, value_span);
            let value = &line[value_span.clone()];

            if key.is_empty() {
                return Err(Error::Syntax(SyntaxError::EmptyOptionName {
                    span: to_byte_span(&line, key_span.clone()).add_offset(offset),
                }));
            }

            state.option_name = Some(key.to_string());
            state
                .current_section
                .insert(key.to_string(), vec![value.to_string()]);

            Ok(Some(Spanned::new(
                to_byte_span(&line, span).add_offset(offset),
                Item::Value {
                    key: Spanned::new(to_byte_span(&line, key_span).add_offset(offset), key.into()),
                    value: Spanned::new(
                        to_byte_span(&line, value_span).add_offset(offset),
                        value.into(),
                    ),
                },
            )))
            // let mut line = line.splitn(2, '=');
            // if let Some(key) = line.next() {
            //     let key = key.trim();
            //     if let Some(value) = line.next() {
            //         Ok(Some(Item::Value {
            //             key: key.into(),
            //             value: value.trim().into(),
            //         }))
            //     } else if key.is_empty() {
            //         Ok(Some(Item::Empty))
            //     } else {
            //         Err(Error::Syntax(SyntaxError::MissingEquals))
            //     }
            // } else {
            //     unreachable!()
            // }
            // } else {
            //     let byte_span = to_byte_span(&line, span).add_offset(offset);
            //     Ok(Some(Spanned::new(byte_span, Item::Empty)))
            // }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::string::ParseError;

    use crate::parse::{DEFAULT_COMMENT_PREFIXES, DEFAULT_DELIMITERS};
    use crate::spanned::{DerefInner, Spanned};
    use crate::tests::{parse, Printer};
    use crate::value::{ClearSpans, NoSectionError, Options, RawSection, Section, Value};
    // use codespan_reporting::{diagnostic::Diagnostic, files, term};
    use color_eyre::eyre;
    use indexmap::map::Keys;
    use serde::de::Error;
    use unindent::unindent;
    // use std::sync::RwLock;

    // macro_rules! get_key {
    //     ($map:expr, $key:expr $(,)?) => {
    //         $map.get_key_value($key).unwrap().0
    //     };
    // }
    //
    // macro_rules! get_value {
    //     ($map:expr, $key:expr $(,)?) => {
    //         $map.get_key_value($key).unwrap().1
    //     };
    // }

    #[test]
    fn compact_span() {
        let line = " this is a  test ";
        let span = super::compact_span(line, 0..line.len());
        similar_asserts::assert_eq!(&line[span], "this is a  test");

        let line = "this is a  test";
        let span = super::compact_span(line, 0..line.len());
        similar_asserts::assert_eq!(&line[span], "this is a  test");

        let line = "    ";
        let span = super::compact_span(line, 0..line.len());
        similar_asserts::assert_eq!(&line[span], "");

        let line = " \n\r   ";
        let span = super::compact_span(line, 0..line.len());
        similar_asserts::assert_eq!(&line[span], "");

        let line = "";
        let span = super::compact_span(line, 0..line.len());
        similar_asserts::assert_eq!(&line[span], "");

        let line = " ####      ";
        let span = super::compact_span(line, 3..line.len());
        similar_asserts::assert_eq!(&line[span], "##");

        let line = "####      ";
        let span = super::compact_span(line, 4..line.len());
        similar_asserts::assert_eq!(&line[span], "");
    }

    #[test]
    fn parse_simple_ini() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = indoc::indoc! {r#"
            [DEFAULT]
            key1 = value1
            pizzatime = yes

            cost = 9

            [topsecrets]
            nuclear launch codes = topsecret

            [github.com]
            User = QEDK
        "#};

        let have = parse(config, &Options::default(), &Printer::default()).0?;
        let mut expected = Value::with_defaults([].into_iter().collect());

        expected.add_section(
            Spanned::from("DEFAULT"),
            [
                (Spanned::from("key1"), Spanned::from("value1")),
                (Spanned::from("pizzatime"), Spanned::from("yes")),
                (Spanned::from("cost"), Spanned::from("9")),
            ],
        );

        expected.add_section(
            Spanned::from("topsecrets"),
            [(
                Spanned::from("nuclear launch codes"),
                Spanned::from("topsecret"),
            )],
        );

        expected.add_section(
            Spanned::from("github.com"),
            [(Spanned::from("user"), Spanned::from("QEDK"))],
        );

        sim_assert_eq!(have.clone().cleared_spans(), expected, "values match");

        // check that spans match
        sim_assert_eq!(
            &config[have.section("DEFAULT").unwrap().span().clone()],
            "DEFAULT"
        );

        let v = have.section("DEFAULT").unwrap();
        sim_assert_eq!(&config[v.key_span("key1").unwrap().clone()], "key1");
        sim_assert_eq!(&config[v["key1"].span.clone()], "value1");
        sim_assert_eq!(
            &config[v.key_span("pizzatime").unwrap().clone()],
            "pizzatime"
        );
        sim_assert_eq!(&config[v["pizzatime"].span.clone()], "yes");
        sim_assert_eq!(&config[v.key_span("cost").unwrap().clone()], "cost");
        sim_assert_eq!(&config[v["cost"].span.clone()], "9");

        let v = have.section("topsecrets").unwrap();
        sim_assert_eq!(
            &config[v.key_span("nuclear launch codes").unwrap().clone()],
            "nuclear launch codes"
        );
        sim_assert_eq!(&config[v["nuclear launch codes"].span.clone()], "topsecret");

        let v = have.section("github.com").unwrap();
        sim_assert_eq!(&config[v.key_span("User").unwrap().clone()], "User");
        sim_assert_eq!(&config[v["User"].span.clone()], "QEDK");

        Ok(())
    }

    fn check_configparser_compat_basic(have: &mut Value) -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        let expected_section_names = [
            "Foo Bar",
            "Spacey Bar",
            "Spacey Bar From The Beginning",
            "Commented Bar",
            "Long Line",
            r"Section\with$weird%characters[\t",
            "Internationalized Stuff",
            "Spaces",
            "Types",
            "This One Has A ] In It",
        ];
        sim_assert_eq!(
            have.section_names()
                .map(|name| name.as_ref())
                .collect::<Vec<_>>(),
            expected_section_names
        );

        let spacey_bar_beginning_expected: RawSection = [
            (Spanned::from("foo"), Spanned::from("bar3")),
            (Spanned::from("baz"), Spanned::from("qwe")),
        ]
        .into_iter()
        .collect();

        dbg!(&have.section("Spacey Bar From The Beginning"));
        sim_assert_eq!(
            &have.section("Spacey Bar From The Beginning").unwrap(),
            // .map(|value| value.clone().cleared_spans())
            // .as_ref(),
            // Some(&spacey_bar_beginning_expected),
            &spacey_bar_beginning_expected,
        );

        // test index trait
        dbg!(&have.section("Spacey Bar From The Beginning"));

        sim_assert_eq!(
            &have.section("Spacey Bar From The Beginning").unwrap(),
            // .clone()
            // .cleared_spans(),
            &spacey_bar_beginning_expected,
        );

        // L = cf.items()
        // L = sorted(list(L))
        // self.assertEqual(len(L), len(E))
        // for name, section in L:
        //     eq(name, section.name)
        // eq(cf.defaults(), cf[self.default_section])

        // API access
        // use std::ops::Deref;
        // let test: &str = have.get("Foo Bar", "foo").unwrap().deref();
        // let test: Option<Spanned<String>> = have.get("Foo Bar", "foo").cloned();
        // let test: Option<&str> = have.get("Foo Bar", "foo").cloned().as_deref();
        // let test: Option<&str> = have.get("Foo Bar", "foo").deref_inner();

        sim_assert_eq!(have.get("Foo Bar", "foo").deref_inner(), Some("bar1"));
        sim_assert_eq!(have.get("Spacey Bar", "foo").deref_inner(), Some("bar2"));
        sim_assert_eq!(
            have.get("Spacey Bar From The Beginning", "foo")
                .deref_inner(),
            Some("bar3")
        );
        sim_assert_eq!(
            have.get("Spacey Bar From The Beginning", "baz")
                .deref_inner(),
            Some("qwe")
        );
        sim_assert_eq!(have.get("Commented Bar", "foo").deref_inner(), Some("bar4"));
        sim_assert_eq!(have.get("Commented Bar", "baz").deref_inner(), Some("qwe"));
        sim_assert_eq!(
            have.get("Spaces", "key with spaces").deref_inner(),
            Some("value")
        );
        sim_assert_eq!(
            have.get("Spaces", "another with spaces").deref_inner(),
            Some("splat!")
        );
        sim_assert_eq!(
            have.get_int("Types", "int")?.map(Spanned::into_inner),
            Some(42)
        );
        sim_assert_eq!(have.get("Types", "int").deref_inner(), Some("42"));
        sim_assert_eq!(
            have.get_float("Types", "float")?.map(Spanned::into_inner),
            Some(0.44)
        );
        sim_assert_eq!(have.get("Types", "float").deref_inner(), Some("0.44"));
        sim_assert_eq!(
            have.get_bool("Types", "boolean")?.map(Spanned::into_inner),
            Some(false)
        );
        sim_assert_eq!(
            have.get("Types", "123").deref_inner(),
            Some("strange but acceptable")
        );
        sim_assert_eq!(
            have.get("This One Has A ] In It", "forks").deref_inner(),
            Some("spoons")
        );

        // test vars= and fallback=
        // sim_assert_eq!(have.get("Foo Bar", "foo", fallback="baz"), "bar1");
        // sim_assert_eq!(have.get("Foo Bar", "foo", vars={'foo': 'baz'}), "baz");
        // with self.assertRaises(configparser.NoSectionError):
        sim_assert_eq!(have.get("No Such Foo Bar", "foo"), None);
        // with self.assertRaises(configparser.NoOptionError):
        // cf.get('Foo Bar', 'no-such-foo')
        sim_assert_eq!(have.get("Foo Var", "no-such-foo"), None);

        // sim_assert_eq(cf.get('No Such Foo Bar', 'foo', fallback='baz'), 'baz')
        // eq(cf.get('Foo Bar', 'no-such-foo', fallback='baz'), 'baz')
        // eq(cf.get('Spacey Bar', 'foo', fallback=None), 'bar2')
        // eq(cf.get('No Such Spacey Bar', 'foo', fallback=None), None)
        // eq(cf.getint('Types', 'int', fallback=18), 42)
        // eq(cf.getint('Types', 'no-such-int', fallback=18), 18)
        // eq(cf.getint('Types', 'no-such-int', fallback="18"), "18") # sic!
        // with self.assertRaises(configparser.NoOptionError):
        sim_assert_eq!(have.get_int("Types", "no-such-int")?, None);
        // self.assertAlmostEqual(cf.getfloat('Types', 'float',
        //                                    fallback=0.0), 0.44)
        // self.assertAlmostEqual(cf.getfloat('Types', 'no-such-float',
        //                                    fallback=0.0), 0.0)
        // eq(cf.getfloat('Types', 'no-such-float', fallback="0.0"), "0.0") # sic!
        // with self.assertRaises(configparser.NoOptionError):
        // cf.getfloat('Types', 'no-such-float')
        sim_assert_eq!(have.get_float("Types", "no-such-float")?, None);
        // eq(cf.getboolean('Types', 'boolean', fallback=True), False)
        // eq(cf.getboolean('Types', 'no-such-boolean', fallback="yes"), "yes") # sic!
        // eq(cf.getboolean('Types', 'no-such-boolean', fallback=True), True)
        // with self.assertRaises(configparser.NoOptionError):
        // cf.getboolean('Types', 'no-such-boolean')
        sim_assert_eq!(have.get_bool("Types", "no-such-boolean")?, None);
        // eq(cf.getboolean('No Such Types', 'boolean', fallback=True), True)

        // mapping access
        sim_assert_eq!(&*have.section("Foo Bar").unwrap()["foo"], "bar1");
        sim_assert_eq!(&*have.section("Spacey Bar").unwrap()["foo"], "bar2");

        let section = &have.section("Spacey Bar From The Beginning").unwrap();
        // sim_assert_eq!(section.name, 'Spacey Bar From The Beginning')
        // self.assertIs(section.parser, cf)
        // with self.assertRaises(AttributeError):
        //     section.name = 'Name is read-only'
        // with self.assertRaises(AttributeError):
        //     section.parser = 'Parser is read-only'
        sim_assert_eq!(&*section["foo"], "bar3");
        sim_assert_eq!(&*section["baz"], "qwe");
        sim_assert_eq!(
            &*have
                .section("Commented Bar")
                .unwrap()
                .get("foo")
                .unwrap()
                .as_ref(),
            "bar4"
        );
        sim_assert_eq!(
            &*have
                .section("Commented Bar")
                .unwrap()
                .get("baz")
                .unwrap()
                .as_ref(),
            "qwe"
        );
        sim_assert_eq!(
            &*have
                .section("Spaces")
                .unwrap()
                .get("key with spaces")
                .unwrap()
                .as_ref(),
            "value"
        );
        sim_assert_eq!(
            &*have
                .section("Spaces")
                .unwrap()
                .get("another with spaces")
                .unwrap()
                .as_ref(),
            "splat!"
        );
        sim_assert_eq!(
            &*have.section("Long Line").unwrap()["foo"],
            "this line is much, much longer than my editor\nlikes it."
        );
        // if self.allow_no_value:
        //     eq(cf['NoValue']['option-without-value'], None)

        // test vars= and fallback=
        sim_assert_eq!(
            have.section("Foo Bar").unwrap().get("foo").deref_inner(),
            Some("bar1")
        );
        // eq(cf['Foo Bar'].get('foo', fallback='baz'), 'bar1')
        // eq(cf['Foo Bar'].get('foo', vars={'foo': 'baz'}), 'baz')

        sim_assert_eq!(
            have.section("Foo Bar").unwrap().get("foo").deref_inner(),
            Some("bar1")
        );

        // with self.assertRaises(KeyError):
        //     cf['No Such Foo Bar']['foo']
        sim_assert_eq!(
            std::panic::catch_unwind(|| have
                .section("No Such Foo Bar")
                .unwrap()
                .get("foo")
                .unwrap()
                .clone())
            .is_err(),
            true,
        );
        // with self.assertRaises(KeyError):
        //     cf['Foo Bar']['no-such-foo']
        sim_assert_eq!(
            std::panic::catch_unwind(|| have
                .section("Foo Bar")
                .unwrap()
                .get("no-such-foo")
                .unwrap()
                .clone())
            .is_err(),
            true,
        );
        // with self.assertRaises(KeyError):
        //     cf['No Such Foo Bar'].get('foo', fallback='baz')
        sim_assert_eq!(
            std::panic::catch_unwind(|| have
                .section("No Such Foo Bar")
                .unwrap()
                .get("foo")
                .unwrap()
                .clone())
            .is_err(),
            true,
        );
        // eq(cf['Foo Bar'].get('no-such-foo', 'baz'), 'baz')
        // eq(cf['Foo Bar'].get('no-such-foo', fallback='baz'), 'baz')
        // eq(cf['Foo Bar'].get('no-such-foo'), None)
        sim_assert_eq!(have.section("Foo Bar").unwrap().get("no-such-foo"), None);
        // eq(cf['Spacey Bar'].get('foo', None), 'bar2')
        sim_assert_eq!(
            have.section("Spacey Bar").unwrap().get("foo").deref_inner(),
            Some("bar2")
        );
        // eq(cf['Spacey Bar'].get('foo', fallback=None), 'bar2')
        // with self.assertRaises(KeyError):
        //     cf['No Such Spacey Bar'].get('foo', None)
        sim_assert_eq!(
            std::panic::catch_unwind(|| have.section("No Such Spacey Bar").unwrap()["foo"].clone())
                .is_err(),
            true,
        );
        sim_assert_eq!(
            std::panic::catch_unwind(|| have.section("No Such Spacey Bar").unwrap()["foo"].clone())
                .is_err(),
            true,
        );
        // eq(cf['Types'].getint('int', 18), 42)
        // eq(cf['Types'].getint('int', fallback=18), 42)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_int("int")?
                .map(Spanned::into_inner),
            Some(42)
        );

        // eq(cf['Types'].getint('no-such-int', 18), 18)
        // eq(cf['Types'].getint('no-such-int', fallback=18), 18)
        // eq(cf['Types'].getint('no-such-int', "18"), "18") # sic!
        // eq(cf['Types'].getint('no-such-int', fallback="18"), "18") # sic!

        // eq(cf['Types'].getint('no-such-int'), None)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_int("no-such-int")?
                .map(Spanned::into_inner),
            None,
        );
        // self.assertAlmostEqual(cf['Types'].getfloat('float', 0.0), 0.44)
        // self.assertAlmostEqual(cf['Types'].getfloat('float', fallback=0.0), 0.44)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_float("float")?
                .map(Spanned::into_inner),
            Some(0.44),
        );
        // self.assertAlmostEqual(cf['Types'].getfloat('no-such-float', 0.0), 0.0)
        // self.assertAlmostEqual(cf['Types'].getfloat('no-such-float', fallback=0.0), 0.0)
        // eq(cf['Types'].getfloat('no-such-float', "0.0"), "0.0") # sic!
        // eq(cf['Types'].getfloat('no-such-float', fallback="0.0"), "0.0") # sic!

        // eq(cf['Types'].getfloat('no-such-float'), None)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_float("no-such-float")?
                .map(Spanned::into_inner),
            None,
        );
        // eq(cf['Types'].getboolean('boolean', True), False)
        // eq(cf['Types'].getboolean('boolean', fallback=True), False)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_bool("boolean")?
                .map(Spanned::into_inner),
            Some(false),
        );
        // eq(cf['Types'].getboolean('no-such-boolean', "yes"), "yes") # sic!
        // eq(cf['Types'].getboolean('no-such-boolean', fallback="yes"), "yes") # sic!
        // eq(cf['Types'].getboolean('no-such-boolean', True), True)
        // eq(cf['Types'].getboolean('no-such-boolean', fallback=True), True)

        // eq(cf['Types'].getboolean('no-such-boolean'), None)
        sim_assert_eq!(
            have.section("Types")
                .unwrap()
                .get_bool("no-such-boolean")?
                .map(Spanned::into_inner),
            None,
        );

        // Make sure the right things happen for remove_section() and
        // remove_option(); added to include check for SourceForge bug #123324.

        have.defaults_mut()
            .set(Spanned::from("this_value"), Spanned::from("1"));
        have.defaults_mut()
            .set(Spanned::from("that_value"), Spanned::from("2"));

        // API access
        assert!(have.remove_section("Spaces").is_some());
        sim_assert_eq!(have.has_option("Spaces", "key with spaces"), false);

        sim_assert_eq!(have.remove_section("Spaces"), None);
        // self.assertFalse(cf.remove_section(self.default_section))
        assert!(
            have.remove_option("Foo Bar", "foo").is_some(),
            "remove_option() failed to report existence of option"
        );
        // self.assertFalse(cf.has_option('Foo Bar', 'foo'),
        //             "remove_option() failed to remove option")
        sim_assert_eq!(
            have.has_option("Foo Bar", "foo"),
            false,
            "remove_option() failed to report existence of option"
        );

        // self.assertFalse(cf.remove_option('Foo Bar', 'foo'),
        //    "remove_option() failed to report non-existence of option that was removed")
        assert!(
            have.remove_option("Foo Bar", "foo").is_none(),
            "remove_option() failed to report non-existence of option that was removed"
        );

        // self.assertTrue(cf.has_option('Foo Bar', 'this_value'))
        assert_eq!(have.has_option("Foo Bar", "this_value"), true);

        // self.assertFalse(cf.remove_option('Foo Bar', 'this_value'))
        assert!(have.remove_option("Foo Bar", "this_value").is_none());

        // self.assertTrue(cf.remove_option(self.default_section, 'this_value'))
        assert!(have.defaults_mut().remove_option("this_value").is_some());

        // self.assertFalse(cf.has_option('Foo Bar', 'this_value'))
        sim_assert_eq!(have.has_option("Foo Bar", "this_value"), false);
        // self.assertFalse(cf.remove_option(self.default_section, 'this_value'))
        assert!(have.defaults_mut().remove_option("this_value").is_none());

        // with self.assertRaises(configparser.NoSectionError) as cm:
        //     cf.remove_option('No Such Section', 'foo')
        assert!(have.remove_option("No Such Section", "foo").is_none());
        // self.assertEqual(cm.exception.args, ('No Such Section',))
        //
        // eq(cf.get('Long Line', 'foo'),
        //    'this line is much, much longer than my editor\nlikes it.')
        sim_assert_eq!(
            have.get("Long Line", "foo").deref_inner(),
            Some("this line is much, much longer than my editor\nlikes it."),
        );

        // mapping access
        have.remove_section("Types");
        sim_assert_eq!(have.has_section("Types"), false);

        // with self.assertRaises(KeyError):
        //     del cf['Types']
        sim_assert_eq!(have.remove_section("Types"), None);

        // with self.assertRaises(ValueError):
        //     del cf[self.default_section]

        // del cf['Spacey Bar']['foo']
        assert!(have.remove_option("Spacey Bar", "foo").is_some());

        // self.assertFalse('foo' in cf['Spacey Bar'])
        sim_assert_eq!(have.section("Spacey Bar").unwrap().has_option("foo"), false);

        // with self.assertRaises(KeyError):
        //     del cf['Spacey Bar']['foo']
        sim_assert_eq!(
            have.section_mut("Spacey Bar").unwrap().remove_option("foo"),
            None
        );

        // self.assertTrue('that_value' in cf['Spacey Bar'])
        sim_assert_eq!(
            have.section("Spacey Bar").unwrap().has_option("that_value"),
            true
        );

        // with self.assertRaises(KeyError):
        //     del cf['Spacey Bar']['that_value']
        sim_assert_eq!(
            have.section_mut("Spacey Bar")
                .unwrap()
                .remove_option("that_value"),
            None
        );

        // del cf[self.default_section]['that_value']
        // self.assertFalse('that_value' in cf['Spacey Bar'])
        // with self.assertRaises(KeyError):
        //     del cf[self.default_section]['that_value']
        // with self.assertRaises(KeyError):
        //     del cf['No Such Section']['foo']
        Ok(())
    }

    #[test]
    fn configparser_compat_case_sensitivity() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let mut config = Value::default();
        config.add_section(Spanned::from("A"), []);
        config.add_section(Spanned::from("a"), []);
        config.add_section(Spanned::from("B"), []);

        sim_assert_eq!(
            config
                .section_names()
                // .map(|name| name.as_ref().as_str())
                .collect::<Vec<_>>(),
            ["A", "a", "B"]
        );

        config.set("a", Spanned::from("B"), Spanned::from("value"))?;
        sim_assert_eq!(
            config
                .options("a")
                // .map(|option| option.as_ref().as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
        sim_assert_eq!(
            config.get("a", "b").deref_inner(),
            Some("value"),
            "could not locate option, expecting case-insensitive option names"
        );

        // with self.assertRaises(configparser.NoSectionError):
        //     # section names are case-sensitive
        //     cf.set("b", "A", "value")

        sim_assert_eq!(
            config
                .set("b", Spanned::from("A"), Spanned::from("value"))
                .unwrap_err()
                .to_string(),
            r#"missing section: "b""#,
        );

        sim_assert_eq!(config.has_option("a", "b"), true);
        sim_assert_eq!(config.has_option("b", "b"), false);

        config.set("A", Spanned::from("A-B"), Spanned::from("A-B value"))?;

        for option in ["a-b", "A-b", "a-B", "A-B"] {
            // dbg!(config.get())
            sim_assert_eq!(
                config.has_option("A", option),
                true,
                "has_option() returned false for option which should exist",
            );
        }

        sim_assert_eq!(
            config
                .options("A")
                // .map(|option| option.as_ref().as_str())
                .collect::<Vec<_>>(),
            ["a-b"]
        );
        sim_assert_eq!(
            config
                .options("a")
                // .map(|option| option.as_ref().as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );

        config.remove_option("a", "B");
        sim_assert_eq!(
            config
                .options("a")
                // .map(|option| option.as_ref().as_str())
                .collect::<Vec<_>>(),
            [] as [&str; 0]
        );

        // SF bug #432369:
        let config = unindent::unindent(&format!(
            "
            [MySection]
            Option{} first line   
            \tsecond line   
            ",
            DEFAULT_DELIMITERS[0],
        ));
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;

        sim_assert_eq!(
            config
                .options("MySection")
                // .map(|option| option.as_ref().as_str())
                .collect::<Vec<_>>(),
            ["option"]
        );
        sim_assert_eq!(
            config.get("MySection", "Option").deref_inner(),
            Some("first line\nsecond line")
        );

        // SF bug #561822:
        let config = unindent::unindent(&format!(
            r#"
            [section]
            nekey{}nevalue\n
            "#,
            DEFAULT_DELIMITERS[0],
        ));
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;

        // cf = self.fromstring("[section]\n"
        //                      "nekey{}nevalue\n".format(self.delimiters[0]),
        //                      defaults={"key":"value"})
        // self.assertTrue(cf.has_option("section", "Key"))
        // TODO(roman): this was true but we do not implement defaults
        sim_assert_eq!(config.has_option("section", "Key"), false);
        Ok(())
    }

    #[test]
    fn configparser_compat_case_insensitivity_mapping_access() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let mut config = Value::default();
        config.add_section(Spanned::from("A"), []);
        config.add_section(
            Spanned::from("a"),
            [(Spanned::from("B"), Spanned::from("value"))],
        );
        config.add_section(Spanned::from("B"), []);

        sim_assert_eq!(
            config
                .section_names()
                // .map(|name| name.as_ref().as_str())
                .collect::<Vec<_>>(),
            ["A", "a", "B"]
        );

        sim_assert_eq!(
            config
                .section("a")
                .unwrap()
                .keys()
                // .map(|name| name.as_ref().as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );

        sim_assert_eq!(
            config.section("a").unwrap()["b"].as_ref().as_str(),
            "value",
            "could not locate option, expecting case-insensitive option names"
        );

        // with self.assertRaises(KeyError):
        //     # section names are case-sensitive
        //     cf["b"]["A"] = "value"

        sim_assert_eq!(
            std::panic::catch_unwind(|| config.section("b").unwrap()["A"].clone()).is_err(),
            true,
        );

        sim_assert_eq!(config.section("a").unwrap().has_option("b"), true);

        config
            .section_mut("A")
            .unwrap()
            .set(Spanned::from("A-B"), Spanned::from("A-B value"));

        for option in ["a-b", "A-b", "a-B", "A-B"] {
            sim_assert_eq!(
                config.get("A", option).is_some(),
                true,
                "has_option() returned false for option which should exist"
            )
        }

        sim_assert_eq!(
            config.section("A").unwrap().keys().collect::<Vec<_>>(),
            ["a-b"]
        );
        sim_assert_eq!(
            config.section("a").unwrap().keys().collect::<Vec<_>>(),
            ["b"]
        );
        config.remove_option("a", "B");

        sim_assert_eq!(
            config.section("a").unwrap().keys().collect::<Vec<_>>(),
            [] as [&str; 0]
        );

        // SF bug #432369:
        let config = format!(
            "[MySection]\nOption{} first line   \n\tsecond line   \n",
            DEFAULT_DELIMITERS[0],
        );
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;

        sim_assert_eq!(
            config
                .section("MySection")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["option"]
        );
        sim_assert_eq!(
            config.section("MySection").unwrap()["Option"]
                .as_ref()
                .as_str(),
            "first line\nsecond line",
        );

        // SF bug #561822:
        // let config = format!(
        //     "[MySection]\nOption{} first line   \n\tsecond line   \n",
        //     DEFAULT_DELIMITERS[0],
        // );
        // let mut config = parse(&config, &Printer::default()).0?;

        // cf = self.fromstring("[section]\n"
        //                      "nekey{}nevalue\n".format(self.delimiters[0]),
        //                      defaults={"key":"value"})
        // self.assertTrue("Key" in cf["section"])
        Ok(())
    }

    #[test]
    fn configparser_compat_default_case_sensitivity() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let mut config = Value::with_defaults(
            [(Spanned::from("foo"), Spanned::from("Bar"))]
                .into_iter()
                .collect(),
        );

        dbg!(&config);

        sim_assert_eq!(
            config.defaults().get("Foo").deref_inner(),
            Some("Bar"),
            "could not locate option, expecting case-insensitive option names",
        );

        let config = Value::with_defaults(
            [(Spanned::from("Foo"), Spanned::from("Bar"))]
                .into_iter()
                .collect(),
        );

        sim_assert_eq!(
            config.defaults().get("Foo").deref_inner(),
            Some("Bar"),
            "could not locate option, expecting case-insensitive defaults",
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_parse_errors() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = format!("[Foo]\n{}val-without-opt-name\n", DEFAULT_DELIMITERS[0]);
        let config = parse(&config, &Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: empty option name")
        );

        let config = format!("[Foo]\n{}val-without-opt-name\n", DEFAULT_DELIMITERS[1]);
        let config = parse(&config, &Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: empty option name")
        );

        let config = "No Section!\n"; // python configparser raises `MissingSectionHeaderError`
        let config = parse(config, &Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: variable assignment missing one of: `=`, `:`")
        );
        // self.assertEqual(e.args, ('<???>', 1, "No Section!\n"))

        let config = "[Foo]\n  wrong-indent\n";
        let config = parse(config, &Options::default(), &Printer::default()).0;
        sim_assert_eq!(
            config.err().map(|err| err.to_string()).as_deref(),
            Some("syntax error: variable assignment missing one of: `=`, `:`")
        );

        // # read_file on a real file
        // tricky = support.findfile("cfgparser.3", subdir="configdata")
        // if self.delimiters[0] == '=':
        //     error = configparser.ParsingError
        //     expected = (tricky,)
        // else:
        //     error = configparser.MissingSectionHeaderError
        //     expected = (tricky, 1,
        //                 '  # INI with as many tricky parts as possible\n')
        // with open(tricky, encoding='utf-8') as f:
        //     e = self.parse_error(cf, error, f)
        // self.assertEqual(e.args, expected)

        Ok(())
    }

    #[test]
    fn configparser_compat_query_errors() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let mut config = Value::default();
        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            [] as [&str; 0],
            "new ConfigParser should have no defined sections"
        );
        sim_assert_eq!(
            config.has_section("Foo"),
            false,
            "new ConfigParser should have no acknowledged sections"
        );

        // with self.assertRaises(configparser.NoSectionError):
        sim_assert_eq!(config.options("Foo").collect::<Vec<_>>(), [] as [&str; 0]);

        // with self.assertRaises(configparser.NoSectionError):
        //     cf.set("foo", "bar", "value")
        sim_assert_eq!(
            config
                .set("foo", Spanned::from("bar"), Spanned::from("value"))
                .err(),
            Some(NoSectionError("foo".to_string()))
        );

        config.add_section(Spanned::from("foo"), []);
        sim_assert_eq!(config.get("foo", "bar"), None);

        sim_assert_eq!(
            config
                .set("foo", Spanned::from("bar"), Spanned::from("value"))
                .err(),
            None,
        );

        Ok(())
    }

    #[test]
    fn configparser_compat_boolean() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = unindent::unindent(&format!(
            "
            [BOOLTEST]\n
            T1{equals}1\n
            T2{equals}TRUE\n
            T3{equals}True\n
            T4{equals}oN\n
            T5{equals}yes\n
            F1{equals}0\n
            F2{equals}FALSE\n
            F3{equals}False\n
            F4{equals}oFF\n
            F5{equals}nO\n
            E1{equals}2\n
            E2{equals}foo\n
            E3{equals}-1\n
            E4{equals}0.1\n
            E5{equals}FALSE AND MORE",
            equals = DEFAULT_DELIMITERS[0],
        ));
        let config = parse(&config, &Options::default(), &Printer::default()).0?;

        for x in 1..5 {
            sim_assert_eq!(
                config
                    .get_bool("BOOLTEST", &format!("t{}", x))?
                    .map(Spanned::into_inner),
                Some(true)
            );
            sim_assert_eq!(
                config
                    .get_bool("BOOLTEST", &format!("f{}", x))?
                    .map(Spanned::into_inner),
                Some(false)
            );
            assert!(config
                .get_bool("BOOLTEST", &format!("e{}", x))
                .unwrap_err()
                .to_string()
                .starts_with("invalid boolean: "));
        }
        Ok(())
    }

    #[test]
    fn configparser_compat_weird_errors() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let mut config = Value::default();
        config.add_section(Spanned::from("Foo"), []);

        // unlike configparser, we do not raise `DuplicateSectionError`,
        // however, the user can manuelly detect when a key is present more than once
        sim_assert_eq!(
            config.add_section(Spanned::from("Foo"), []),
            Some(Section::from_iter([]).with_name(Spanned::from("Foo"))),
        );

        // our implementation is very relaxed in that we collect all the options from all the
        // occurences of the same of section
        let config = unindent(&format!(
            "
            [Foo]
            will this be added{equals}True
            [Bar]
            what about this{equals}True
            [Foo]
            oops{equals}this won't
            ",
            equals = DEFAULT_DELIMITERS[0],
        ));
        let config = parse(&config, &Options::default(), &Printer::default()).0?;
        let mut expected = Value::default();
        expected.add_section(
            Spanned::from("Foo"),
            [
                (Spanned::from("will this be added"), Spanned::from("True")),
                (Spanned::from("oops"), Spanned::from("this won't")),
            ],
        );
        expected.add_section(
            Spanned::from("Bar"),
            [(Spanned::from("what about this"), Spanned::from("True"))],
        );

        sim_assert_eq!(config.cleared_spans(), expected);
        Ok(())
    }

    #[test]
    fn configparser_compat_get_after_duplicate_option_error() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = unindent(&format!(
            "
            [Foo]
            x{equals}1
            y{equals}2
            y{equals}3
            ",
            equals = DEFAULT_DELIMITERS[0],
        ));
        let options = Options {
            strict: true,
            ..Options::default()
        };
        let config = parse(&config, &options, &Printer::default()).0?;
        let mut expected = Value::default();
        sim_assert_eq!(config.get("Foo", "x").deref_inner(), Some("1"));
        sim_assert_eq!(config.get("Foo", "y").deref_inner(), Some("2"));
        Ok(())
    }

    #[test]
    fn configparser_compat_set_string_types() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = unindent(&format!(
            "
            [sect]
            option1{equals}foo
            ",
            equals = DEFAULT_DELIMITERS[0],
        ));
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;

        // check that we don't get an exception when setting values in
        // an existing section using strings:

        config.set("sect", "option1".into(), "splat".into());
        config.set("sect", "option1".into(), "splat".to_string().into());
        config.set("sect", "option2".into(), "splat".into());
        config.set("sect", "option2".into(), "splat".to_string().into());
        config.set("sect", "option1".into(), "splat".into());
        config.set("sect", "option2".into(), "splat".into());
        Ok(())
    }

    #[test]
    fn configparser_compat_check_items_config() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = unindent(&format!(
            r#"
            default {delim0} <default>

            [section]
            name {delim0} %(value)s
            key{delim1} |%(name)s|
            getdefault{delim1} |%(default)s|
            "#,
            delim0 = DEFAULT_DELIMITERS[0],
            delim1 = DEFAULT_DELIMITERS[1],
        ));
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;
        let mut items = config
            .section("section")
            .unwrap()
            .iter()
            .map(|(k, v)| (k.as_ref().as_str(), v.as_ref().as_str()))
            .collect::<Vec<_>>();
        sim_assert_eq!(
            items,
            vec![
                ("default", "<default>"),
                ("name", "%(value)s"),
                ("key", "|%(name)s|"),
                ("getdefault", "|%(default)s|"),
            ]
        );
        // self.assertEqual(L, expected);
        sim_assert_eq!(config.section("no such section"), None);
        Ok(())
    }

    #[test]
    fn configparser_compat_popitem() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = unindent(&format!(
            r#"
            [section1]
            name1 {delim0} value1
            [section2]
            name2 {delim0} value2
            [section3]
            name3 {delim0} value3
            "#,
            delim0 = DEFAULT_DELIMITERS[0],
        ));
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;

        sim_assert_eq!(
            config
                .pop()
                .map(|section| section.name)
                .as_ref()
                .deref_inner(),
            Some("section1")
        );
        sim_assert_eq!(
            config
                .pop()
                .map(|section| section.name)
                .as_ref()
                .deref_inner(),
            Some("section2")
        );
        sim_assert_eq!(
            config
                .pop()
                .map(|section| section.name)
                .as_ref()
                .deref_inner(),
            Some("section3")
        );
        sim_assert_eq!(config.pop(), None);
        Ok(())
    }

    #[test]
    fn configparser_compat_clear() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let mut config = Value::default();
        config.defaults_mut().set("foo".into(), "Bar".into());
        sim_assert_eq!(
            config.defaults().get("Foo").deref_inner(),
            Some("Bar"),
            "could not locate option, expecting case-insensitive option names"
        );

        config.add_section(
            "zing".into(),
            [
                ("option1".into(), "value1".into()),
                ("option2".into(), "value2".into()),
            ],
        );

        sim_assert_eq!(config.section_names().collect::<Vec<_>>(), vec!["zing"]);
        sim_assert_eq!(
            config.section("zing").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<&str>>()),
            Some(vec!["option1", "option2", "foo"]),
        );

        config.clear();
        sim_assert_eq!(
            config.section_names().collect::<Vec<&Spanned<String>>>(),
            vec![] as Vec<&Spanned<String>>
        );
        sim_assert_eq!(
            config
                .defaults()
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<&str>>(),
            vec!["foo"]
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_setitem() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = unindent(&format!(
            r#"
            nameD {equals} valueD
            [section1]
            name1 {equals} value1
            [section2]
            name2 {equals} value2
            [section3]
            name3 {equals} value3
            "#,
            equals = DEFAULT_DELIMITERS[0],
        ));
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;

        sim_assert_eq!(
            config.section("section1").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name1", "named"])
        );
        sim_assert_eq!(
            config.section("section2").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name2", "named"])
        );
        sim_assert_eq!(
            config.section("section3").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name3", "named"])
        );
        sim_assert_eq!(
            config
                .section("section1")
                .and_then(|section| section.get("name1"))
                .deref_inner(),
            Some("value1")
        );
        sim_assert_eq!(
            config
                .section("section2")
                .and_then(|section| section.get("name2"))
                .deref_inner(),
            Some("value2")
        );
        sim_assert_eq!(
            config
                .section("section3")
                .and_then(|section| section.get("name3"))
                .deref_inner(),
            Some("value3")
        );
        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );
        config.add_section("section2".into(), [("name22".into(), "value22".into())]);
        sim_assert_eq!(
            config.section("section2").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["name22", "named"]),
        );
        sim_assert_eq!(
            config
                .section("section2")
                .and_then(|section| section.get("name22"))
                .deref_inner(),
            Some("value22")
        );
        assert!(!config.section("section2").unwrap().has_option("name2"));
        sim_assert_eq!(config.section("section2").unwrap().get("name2"), None);

        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );
        config.add_section("section3".into(), []);
        sim_assert_eq!(
            config.section("section3").map(|section| section
                .keys()
                .map(|k| k.as_ref().as_str())
                .collect::<Vec<_>>()),
            Some(vec!["named"])
        );

        assert!(!config.section("section3").unwrap().has_option("name3"));
        sim_assert_eq!(config.section("section3").unwrap().get("name3"), None);

        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );
        // For bpo-32108, assigning default_section to itself.
        *config.defaults_mut() = config.defaults().clone();
        assert_ne!(
            config.defaults().keys().collect::<Vec<&Spanned<String>>>(),
            vec![] as Vec<&Spanned<String>>
        );
        *config.defaults_mut() = Section::default();

        sim_assert_eq!(
            config.defaults().keys().collect::<Vec<_>>(),
            vec![] as Vec<&Spanned<String>>
        );
        sim_assert_eq!(
            config
                .section("section1")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["name1"]
        );
        sim_assert_eq!(
            config
                .section("section2")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["name22"]
        );
        sim_assert_eq!(
            config
                .section("section3")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec![] as Vec<&Spanned<String>>
        );
        sim_assert_eq!(
            config.section_names().collect::<Vec<_>>(),
            vec!["section1", "section2", "section3"]
        );

        // For bpo-32108, assigning section to itself.
        // *config.section_mut("section2").unwrap().as_mut() =
        //     config.section("section2").unwrap().clone();
        let section2: Section = config.section("section2").unwrap().as_ref().clone();
        config
            .section_mut("section2")
            .unwrap()
            .replace_with(section2);
        sim_assert_eq!(
            config
                .section("section2")
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["name22"]
        );
        Ok(())
    }

    #[test]
    fn configparser_compat_invalid_multiline_value() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();

        let config = unindent(&format!(
            "\
            [DEFAULT]
            test {equals} test
            invalid\
            ",
            equals = DEFAULT_DELIMITERS[0],
        ));
        let res = parse(&config, &Options::default(), &Printer::default()).0;
        let err = res.err().map(|err| err.to_string());
        sim_assert_eq!(
            err.as_deref(),
            Some("syntax error: variable assignment missing one of: `=`, `:`")
        );

        // sim_assert_eq!(
        //     config.section("DEFAULT").unwrap().get("test").deref_inner(),
        //     Some("test")
        // );
        // sim_assert_eq!(
        //     config.section("DEFAULT").unwrap().get("test").deref_inner(),
        //     Some("test")
        // );
        Ok(())
    }

    #[test]
    fn configparser_compat_parse_cfgparser_1() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();
        let config = include_str!("../test-data/cfgparser.1.ini");
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;
        dbg!(&config);
        Ok(())
    }

    #[test]
    fn configparser_compat_parse_cfgparser_2() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();
        let config = include_str!("../test-data/cfgparser.2.ini");
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;
        dbg!(&config);
        Ok(())
    }

    #[test]
    fn configparser_compat_parse_cfgparser_3() -> eyre::Result<()> {
        use similar_asserts::assert_eq as sim_assert_eq;
        crate::tests::init();
        let config = include_str!("../test-data/cfgparser.3.ini");
        let mut config = parse(&config, &Options::default(), &Printer::default()).0?;
        dbg!(&config);
        Ok(())
    }

    /// Basic configparser compat test
    ///
    /// adapted from: https://github.com/python/cpython/blob/3.13/Lib/test/test_configparser.py#L294
    #[test]
    fn configparser_compat_basic() -> eyre::Result<()> {
        crate::tests::init();

        let config = unindent::unindent(&format!(
            r#"
            [Foo Bar]
            foo{d0}bar1
            [Spacey Bar]
            foo {d0} bar2
            [Spacey Bar From The Beginning]
              foo {d0} bar3
              baz {d0} qwe
            [Commented Bar]
            foo{d1} bar4 {c1} comment
            baz{d0}qwe {c0}another one
            [Long Line]
            foo{d1} this line is much, much longer than my editor
               likes it.
            [Section\with$weird%characters[\t]
            [Internationalized Stuff]
            foo[bg]{d1} Bulgarian
            foo{d0}Default
            foo[en]{d0}English
            foo[de]{d0}Deutsch
            [Spaces]
            key with spaces {d1} value
            another with spaces {d0} splat!
            [Types]
            int {d1} 42
            float {d0} 0.44
            boolean {d0} NO
            123 {d1} strange but acceptable
            [This One Has A ] In It]
              forks {d0} spoons
            "#,
            d0 = DEFAULT_DELIMITERS[0],
            d1 = DEFAULT_DELIMITERS[1],
            c0 = DEFAULT_COMMENT_PREFIXES[0],
            c1 = DEFAULT_COMMENT_PREFIXES[1],
        ));

        let mut have = parse(&config, &Options::default(), &Printer::default()).0?;
        check_configparser_compat_basic(&mut have)?;
        // let have = super::value::from_str(&config).map_?;
        // let expected = Value {
        //     sections: [
        //         (
        //             Spanned::from("DEFAULT".to_string()),
        //             [
        //                 (
        //                     Spanned::from("key1".to_string()),
        //                     Spanned::from("value1".to_string()),
        //                 ),
        //                 (
        //                     Spanned::from("pizzatime".to_string()),
        //                     Spanned::from("yes".to_string()),
        //                 ),
        //                 (
        //                     Spanned::from("cost".to_string()),
        //                     Spanned::from("9".to_string()),
        //                 ),
        //             ]
        //             .into_iter()
        //             .collect(),
        //         ),
        //         (
        //             Spanned::from("topsecrets".to_string()),
        //             [(
        //                 Spanned::from("nuclear launch codes".to_string()),
        //                 Spanned::from("topsecret".to_string()),
        //             )]
        //             .into_iter()
        //             .collect(),
        //         ),
        //         (
        //             Spanned::from("github.com".to_string()),
        //             [(
        //                 Spanned::from("User".to_string()),
        //                 Spanned::from("QEDK".to_string()),
        //             )]
        //             .into_iter()
        //             .collect(),
        //         ),
        //     ]
        //     .into_iter()
        //     .collect(),
        //     global: [].into_iter().collect(),
        // };

        // if self.allow_no_value:
        //     config_string += (
        //         "[NoValue]\n"
        //         "option-without-value\n"
        //         )
        // cf = self.fromstring(config_string)
        // self.basic_test(cf)
        // if self.strict:
        //     with self.assertRaises(configparser.DuplicateOptionError):
        //         cf.read_string(textwrap.dedent("""\
        //             [Duplicate Options Here]
        //             option {0[0]} with a value
        //             option {0[1]} with another value
        //         """.format(self.delimiters)))
        //     with self.assertRaises(configparser.DuplicateSectionError):
        //         cf.read_string(textwrap.dedent("""\
        //             [And Now For Something]
        //             completely different {0[0]} True
        //             [And Now For Something]
        //             the larch {0[1]} 1
        //         """.format(self.delimiters)))
        // else:
        //     cf.read_string(textwrap.dedent("""\
        //         [Duplicate Options Here]
        //         option {0[0]} with a value
        //         option {0[1]} with another value
        //     """.format(self.delimiters)))
        //
        //     cf.read_string(textwrap.dedent("""\
        //         [And Now For Something]
        //         completely different {0[0]} True
        //         [And Now For Something]
        //         the larch {0[1]} 1
        //     """.format(self.delimiters)))
        Ok(())
    }

    #[test]
    fn parse_ini_multi_line_continuation() -> eyre::Result<()> {
        crate::tests::init();

        let config = indoc::indoc! {r#"
            [options.packages.find]
            exclude =
                example*
                tests*
                docs*
                build

            [bumpversion:file:CHANGELOG.md]
            replace = **unreleased**
                **v{new_version}**

            [bumpversion:part:release]
            optional_value = gamma
            values =
                dev
                gamma
        "#};

        let have = parse(config, &Options::default(), &Printer::default()).0?;
        dbg!(&have);
        let mut expected = Value::with_defaults([].into_iter().collect());
        expected.add_section(
            Spanned::from("options.packages.find"),
            [(
                Spanned::from("exclude"),
                Spanned::from("\nexample*\ntests*\ndocs*\nbuild"),
            )],
        );
        expected.add_section(
            Spanned::from("bumpversion:file:CHANGELOG.md"),
            [(
                Spanned::from("replace"),
                Spanned::from("**unreleased**\n**v{new_version}**"),
            )],
        );
        expected.add_section(
            Spanned::from("bumpversion:part:release"),
            [
                (Spanned::from("optional_value"), Spanned::from("gamma")),
                (Spanned::from("values"), Spanned::from("\ndev\ngamma")),
            ],
        );

        similar_asserts::assert_eq!(have.clone().cleared_spans(), expected);
        Ok(())
    }

    //     Line = functools.partial(_Line, prefixes=self._prefixes)
    //     for st.lineno, line in enumerate(map(Line, fp), start=1):
    //         if not line.clean:
    //             if self._empty_lines_in_values:
    //                 # add empty line to the value, but only if there was no
    //                 # comment on the line
    //                 if (not line.has_comments and
    //                     st.cursect is not None and
    //                     st.optname and
    //                     st.cursect[st.optname] is not None):
    //                     st.cursect[st.optname].append('') # newlines added at join
    //             else:
    //                 # empty line marks end of value
    //                 st.indent_level = sys.maxsize
    //             continue
    //
    //         first_nonspace = self.NONSPACECRE.search(line)
    //         st.cur_indent_level = first_nonspace.start() if first_nonspace else 0
    //
    //         if self._handle_continuation_line(st, line, fpname):
    //             continue
    //
    //         self._handle_rest(st, line, fpname)
    //
    //     return st.errors

    // def _read(self, fp, fpname):
    //     """Parse a sectioned configuration file.
    //
    //     Each section in a configuration file contains a header, indicated by
    //     a name in square brackets (`[]`), plus key/value options, indicated by
    //     `name` and `value` delimited with a specific substring (`=` or `:` by
    //     default).
    //
    //     Values can span multiple lines, as long as they are indented deeper
    //     than the first line of the value. Depending on the parser's mode, blank
    //     lines may be treated as parts of multiline values or ignored.
    //
    //     Configuration files may include comments, prefixed by specific
    //     characters (`#` and `;` by default). Comments may appear on their own
    //     in an otherwise empty line or may be entered in lines holding values or
    //     section names. Please note that comments get stripped off when reading configuration files.
    //     """
    //
    //     try:
    //         ParsingError._raise_all(self._read_inner(fp, fpname))
    //     finally:
    //         self._join_multiline_values()
    //
    // def _read_inner(self, fp, fpname):
    //     st = _ReadState()
    //
    //     Line = functools.partial(_Line, prefixes=self._prefixes)
    //     for st.lineno, line in enumerate(map(Line, fp), start=1):
    //         if not line.clean:
    //             if self._empty_lines_in_values:
    //                 # add empty line to the value, but only if there was no
    //                 # comment on the line
    //                 if (not line.has_comments and
    //                     st.cursect is not None and
    //                     st.optname and
    //                     st.cursect[st.optname] is not None):
    //                     st.cursect[st.optname].append('') # newlines added at join
    //             else:
    //                 # empty line marks end of value
    //                 st.indent_level = sys.maxsize
    //             continue
    //
    //         first_nonspace = self.NONSPACECRE.search(line)
    //         st.cur_indent_level = first_nonspace.start() if first_nonspace else 0
    //
    //         if self._handle_continuation_line(st, line, fpname):
    //             continue
    //
    //         self._handle_rest(st, line, fpname)
    //
    //     return st.errors
    //
    // def _handle_continuation_line(self, st, line, fpname):
    //     # continuation line?
    //     is_continue = (st.cursect is not None and st.optname and
    //         st.cur_indent_level > st.indent_level)
    //     if is_continue:
    //         if st.cursect[st.optname] is None:
    //             raise MultilineContinuationError(fpname, st.lineno, line)
    //         st.cursect[st.optname].append(line.clean)
    //     return is_continue
    //
    // def _handle_rest(self, st, line, fpname):
    //     # a section header or option header?
    //     if self._allow_unnamed_section and st.cursect is None:
    //         st.sectname = UNNAMED_SECTION
    //         st.cursect = self._dict()
    //         self._sections[st.sectname] = st.cursect
    //         self._proxies[st.sectname] = SectionProxy(self, st.sectname)
    //         st.elements_added.add(st.sectname)
    //
    //     st.indent_level = st.cur_indent_level
    //     # is it a section header?
    //     mo = self.SECTCRE.match(line.clean)
    //
    //     if not mo and st.cursect is None:
    //         raise MissingSectionHeaderError(fpname, st.lineno, line)
    //
    //     self._handle_header(st, mo, fpname) if mo else self._handle_option(st, line, fpname)
    //
    // def _handle_header(self, st, mo, fpname):
    //     st.sectname = mo.group('header')
    //     if st.sectname in self._sections:
    //         if self._strict and st.sectname in st.elements_added:
    //             raise DuplicateSectionError(st.sectname, fpname,
    //                                         st.lineno)
    //         st.cursect = self._sections[st.sectname]
    //         st.elements_added.add(st.sectname)
    //     elif st.sectname == self.default_section:
    //         st.cursect = self._defaults
    //     else:
    //         st.cursect = self._dict()
    //         self._sections[st.sectname] = st.cursect
    //         self._proxies[st.sectname] = SectionProxy(self, st.sectname)
    //         st.elements_added.add(st.sectname)
    //     # So sections can't start with a continuation line
    //     st.optname = None
    //
    // def _handle_option(self, st, line, fpname):
    //     # an option line?
    //     st.indent_level = st.cur_indent_level
    //
    //     mo = self._optcre.match(line.clean)
    //     if not mo:
    //         # a non-fatal parsing error occurred. set up the
    //         # exception but keep going. the exception will be
    //         # raised at the end of the file and will contain a
    //         # list of all bogus lines
    //         st.errors.append(ParsingError(fpname, st.lineno, line))
    //         return
    //
    //     st.optname, vi, optval = mo.group('option', 'vi', 'value')
    //     if not st.optname:
    //         st.errors.append(ParsingError(fpname, st.lineno, line))
    //     st.optname = self.optionxform(st.optname.rstrip())
    //     if (self._strict and
    //         (st.sectname, st.optname) in st.elements_added):
    //         raise DuplicateOptionError(st.sectname, st.optname,
    //                                 fpname, st.lineno)
    //     st.elements_added.add((st.sectname, st.optname))
    //     # This check is fine because the OPTCRE cannot
    //     # match if it would set optval to None
    //     if optval is not None:
    //         optval = optval.strip()
    //         st.cursect[st.optname] = [optval]
    //     else:
    //         # valueless option handling
    //         st.cursect[st.optname] = None
    //
    // def _join_multiline_values(self):
    //     defaults = self.default_section, self._defaults
    //     all_sections = itertools.chain((defaults,),
    //                                    self._sections.items())
    //     for section, options in all_sections:
    //         for name, val in options.items():
    //             if isinstance(val, list):
    //                 val = '\n'.join(val).rstrip()
    //             options[name] = self._interpolation.before_read(self,
    //                                                             section,
    //                                                             name, val)
}
