use crate::parse::Item;
use std::io::Write;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineEnding {
    Linefeed,
    CrLf,
}

impl Default for LineEnding {
    fn default() -> Self {
        LineEnding::CrLf
    }
}

impl std::fmt::Display for LineEnding {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                LineEnding::Linefeed => "\n",
                LineEnding::CrLf => "\r\n",
            }
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Writer<W> {
    write: W,
    line_ending: LineEnding,
}

impl<W> Writer<W> {
    pub fn new(write: W, line_ending: LineEnding) -> Self {
        Writer { write, line_ending }
    }

    pub fn into_inner(self) -> W {
        self.write
    }
}

impl<W: Write> Writer<W> {
    pub fn write(&mut self, item: &Item) -> std::io::Result<()> {
        match *item {
            Item::Section { ref name } => write!(&mut self.write, "[{}]{}", name, self.line_ending),
            Item::Value { ref key, ref value } => {
                write!(&mut self.write, "{}={}{}", key, value, self.line_ending)
            }
            Item::Comment { ref text } => write!(&mut self.write, ";{}{}", text, self.line_ending),
            Item::Empty => write!(&mut self.write, "{}", self.line_ending),
        }
    }
}
