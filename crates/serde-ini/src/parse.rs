#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum Item {
    Empty,
    Section { name: String },
    Value { key: String, value: String },
    Comment { text: String },
}

#[derive(thiserror::Error, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum SyntaxError {
    #[error("section was not closed: missing ']'")]
    SectionNotClosed,
    #[error("invalid section name: contains ']'")]
    InvalidSectionName,
    #[error("variable assignment missing '='")]
    MissingEquals,
}

#[derive(thiserror::Error, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum Error<E>
where
    E: std::fmt::Display,
{
    #[error("{0}")]
    Inner(E),
    #[error("INI syntax error")]
    Syntax(#[from] SyntaxError),
}

pub struct Parser<T> {
    input: T,
}

impl<T> Parser<T> {
    pub fn new(input: T) -> Self {
        Parser { input }
    }

    pub fn into_inner(self) -> T {
        self.input
    }
}

impl<'a> Parser<OkIter<std::str::Lines<'a>>> {
    pub fn from_str(s: &'a str) -> Self {
        Self::new(OkIter(s.lines()))
    }
}

impl<R: std::io::BufRead> Parser<std::io::Lines<R>> {
    pub fn from_bufread(r: R) -> Self {
        Self::new(r.lines())
    }
}

impl<R: std::io::Read> Parser<std::io::Lines<std::io::BufReader<R>>> {
    pub fn from_read(r: R) -> Self {
        Self::from_bufread(std::io::BufReader::new(r))
    }
}

impl<T> Parser<T> {
    fn parse_next<E, S>(line: Option<S>) -> Result<Option<Item>, Error<E>>
    where
        E: std::fmt::Display,
        S: AsRef<str>,
    {
        let line = match line {
            Some(line) => line,
            None => return Ok(None),
        };
        let line = line.as_ref();

        if line.starts_with('[') {
            if line.ends_with(']') {
                let line = &line[1..line.len() - 1];
                if line.contains(']') {
                    Err(Error::Syntax(SyntaxError::InvalidSectionName))
                } else {
                    Ok(Some(Item::Section { name: line.into() }))
                }
            } else {
                Err(Error::Syntax(SyntaxError::SectionNotClosed))
            }
        } else if line.starts_with(';') || line.starts_with('#') {
            Ok(Some(Item::Comment { text: line.into() }))
        } else {
            // println!("line: {line}");
            let mut line = line.splitn(2, '=');
            println!("line: {:?}", line.clone().into_iter().collect::<Vec<_>>());
            if let Some(key) = line.next() {
                let key = key.trim();
                if let Some(value) = line.next() {
                    Ok(Some(Item::Value {
                        key: key.into(),
                        value: value.trim().into(),
                    }))
                } else if key.is_empty() {
                    Ok(Some(Item::Empty))
                } else {
                    Err(Error::Syntax(SyntaxError::MissingEquals))
                }
            } else {
                unreachable!()
            }
        }
    }
}

impl<E, S, T> Iterator for Parser<T>
where
    E: std::fmt::Display,
    S: AsRef<str>,
    T: Iterator<Item = Result<S, E>>,
{
    type Item = Result<Item, Error<E>>;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.input.next().transpose().map_err(Error::Inner);
        value.and_then(|l| Self::parse_next(l)).transpose()
    }
}

pub struct OkIter<I>(pub I);

impl<T, I: Iterator<Item = T>> Iterator for OkIter<I> {
    type Item = Result<T, std::convert::Infallible>;

    fn next(&mut self) -> Option<Self::Item> {
        (self.0).next().map(Ok)
    }
}
