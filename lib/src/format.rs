// /// Parse error
// pub struct ParseError {
//     pub description: String,
//     pub note: Option<String>,
//     pub label: String,
//     // pub span: InnerSpan,
//     pub span: std::ops::Range<usize>,
//     // pub secondary_label: Option<(string::String, InnerSpan)>,
//     // pub suggestion: Suggestion,
// }
//
// type InnerSpan = std::ops::Range<usize>;
//
// #[derive(Copy, Clone)]
// struct InnerOffset(usize);
//
// impl InnerOffset {
//     fn to(self, end: InnerOffset) -> InnerSpan {
//         InnerSpan {
//             start: self.0,
//             end: end.0,
//         }
//         // InnerSpan::new(self.0, end.0)
//     }
// }
//
// // pub enum Suggestion {
// //     None,
// //     /// Replace inline argument with positional argument:
// //     /// `format!("{foo.bar}")` -> `format!("{}", foo.bar)`
// //     UsePositional,
// //     /// Remove `r#` from identifier:
// //     /// `format!("{r#foo}")` -> `format!("{foo}")`
// //     RemoveRawIdent(InnerSpan),
// // }
//
// /// The parser structure for interpreting the input format string. This is
// /// modeled as an iterator over `Piece` structures to form a stream of tokens
// /// being output.
// ///
// /// This is a recursive-descent parser for the sake of simplicity, and if
// /// necessary there's probably lots of room for improvement performance-wise.
// pub struct Parser<'a> {
//     // mode: ParseMode,
//     input: &'a str,
//     cur: std::iter::Peekable<std::str::CharIndices<'a>>,
//     /// Error messages accumulated during parsing
//     pub errors: Vec<ParseError>,
//     // /// Current position of implicit positional argument pointer
//     // pub curarg: usize,
//     // /// `Some(raw count)` when the string is "raw", used to position spans correctly
//     // style: Option<usize>,
//     // /// Start and end byte offset of every successfully parsed argument
//     // pub arg_places: Vec<InnerSpan>,
//     // /// Characters whose length has been changed from their in-code representation
//     // width_map: Vec<InnerWidthMapping>,
//     /// Span of the last opening brace seen, used for error reporting
//     last_opening_brace: Option<InnerSpan>,
//     // /// Whether the source string is comes from `println!` as opposed to `format!` or `print!`
//     // append_newline: bool,
//     // /// Whether this formatting string was written directly in the source. This controls whether we
//     // /// can use spans to refer into it and give better error messages.
//     // /// N.B: This does _not_ control whether implicit argument captures can be used.
//     // pub is_source_literal: bool,
//     /// Start position of the current line.
//     cur_line_start: usize,
//     // /// Start and end byte offset of every line of the format string. Excludes
//     // /// newline characters and leading whitespace.
//     // pub line_spans: Vec<InnerSpan>,
// }
//
// enum Piece<'a> {
//     String(&'a str),
//     Argument(&'a str),
// }
//
// impl<'a> Iterator for Parser<'a> {
//     type Item = Piece<'a>;
//
//     fn next(&mut self) -> Option<Piece<'a>> {
//         if let Some(&(pos, c)) = self.cur.peek() {
//             match c {
//                 '{' => {
//                     let curr_last_brace = self.last_opening_brace;
//                     let byte_pos = self.to_span_index(pos);
//                     let lbrace_end = InnerOffset(byte_pos.0 + self.to_span_width(pos));
//                     self.last_opening_brace = Some(byte_pos.to(lbrace_end));
//                     self.cur.next();
//                     if self.consume('{') {
//                         self.last_opening_brace = curr_last_brace;
//
//                         Some(Piece::String(self.string(pos + 1)))
//                     } else {
//                         let arg = self.argument(lbrace_end);
//                         if let Some(rbrace_pos) = self.consume_closing_brace(&arg) {
//                             // if self.is_source_literal {
//                             //     let lbrace_byte_pos = self.to_span_index(pos);
//                             //     let rbrace_byte_pos = self.to_span_index(rbrace_pos);
//                             //
//                             //     let width = self.to_span_width(rbrace_pos);
//                             //
//                             //     self.arg_places.push(
//                             //         lbrace_byte_pos.to(InnerOffset(rbrace_byte_pos.0 + width)),
//                             //     );
//                             // }
//                         } else if let Some(&(_, maybe)) = self.cur.peek() {
//                             todo!();
//                             // match maybe {
//                             //     '?' => self.suggest_format_debug(),
//                             //     '<' | '^' | '>' => self.suggest_format_align(maybe),
//                             //     _ => self.suggest_positional_arg_instead_of_captured_arg(arg),
//                             // }
//                         }
//                         Some(Piece::Argument("TODO"))
//                     }
//                 }
//                 '}' => {
//                     self.cur.next();
//                     if self.consume('}') {
//                         Some(Piece::String(self.string(pos + 1)))
//                     } else {
//                         let err_pos = self.to_span_index(pos);
//                         // self.err_with_note(
//                         //     "unmatched `}` found",
//                         //     "unmatched `}`",
//                         //     "if you intended to print `}`, you can escape it using `}}`",
//                         //     err_pos.to(err_pos),
//                         // );
//                         None
//                     }
//                 }
//                 _ => Some(Piece::String(self.string(pos))),
//             }
//         } else {
//             // if self.is_source_literal {
//             //     let span = self.span(self.cur_line_start, self.input.len());
//             //     if self.line_spans.last() != Some(&span) {
//             //         self.line_spans.push(span);
//             //     }
//             // }
//             None
//         }
//     }
// }
//
// impl<'a> Parser<'a> {
//     /// Creates a new parser for the given format string
//     pub fn new(
//         s: &'a str,
//         // style: Option<usize>,
//         // snippet: Option<string::String>,
//         // append_newline: bool,
//         // mode: ParseMode,
//     ) -> Parser<'a> {
//         let input_string_kind = find_width_map_from_snippet(s, snippet, style);
//         let (width_map, is_source_literal) = match input_string_kind {
//             InputStringKind::Literal { width_mappings } => (width_mappings, true),
//             InputStringKind::NotALiteral => (Vec::new(), false),
//         };
//
//         Parser {
//             // mode,
//             input: s,
//             cur: s.char_indices().peekable(),
//             errors: vec![],
//             // curarg: 0,
//             // style,
//             // arg_places: vec![],
//             // width_map,
//             last_opening_brace: None,
//             // append_newline,
//             // is_source_literal,
//             // cur_line_start: 0,
//             // line_spans: vec![],
//         }
//     }
//
//     /// Optionally consumes the specified character. If the character is not at
//     /// the current position, then the current iterator isn't moved and `false` is
//     /// returned, otherwise the character is consumed and `true` is returned.
//     fn consume(&mut self, c: char) -> bool {
//         self.consume_pos(c).is_some()
//     }
//
//     /// Optionally consumes the specified character. If the character is not at
//     /// the current position, then the current iterator isn't moved and `None` is
//     /// returned, otherwise the character is consumed and the current position is
//     /// returned.
//     fn consume_pos(&mut self, c: char) -> Option<usize> {
//         if let Some(&(pos, maybe)) = self.cur.peek() {
//             if c == maybe {
//                 self.cur.next();
//                 return Some(pos);
//             }
//         }
//         None
//     }
//
//     fn remap_pos(&self, mut pos: usize) -> InnerOffset {
//         for width in &self.width_map {
//             if pos > width.position {
//                 pos += width.before - width.after;
//             } else if pos == width.position && width.after == 0 {
//                 pos += width.before;
//             } else {
//                 break;
//             }
//         }
//
//         InnerOffset(pos)
//     }
//
//     fn to_span_index(&self, pos: usize) -> InnerOffset {
//         // This handles the raw string case, the raw argument is the number of #
//         // in r###"..."### (we need to add one because of the `r`).
//         let raw = self.style.map_or(0, |raw| raw + 1);
//         let pos = self.remap_pos(pos);
//         InnerOffset(raw + pos.0 + 1)
//     }
//
//     fn to_span_width(&self, pos: usize) -> usize {
//         let pos = self.remap_pos(pos);
//         match self.width_map.iter().find(|w| w.position == pos.0) {
//             Some(w) => w.before,
//             None => 1,
//         }
//     }
//
//     /// Consumes all whitespace characters until the first non-whitespace character
//     fn ws(&mut self) {
//         while let Some(&(_, c)) = self.cur.peek() {
//             if c.is_whitespace() {
//                 self.cur.next();
//             } else {
//                 break;
//             }
//         }
//     }
//
//     /// Forces consumption of the specified character. If the character is not
//     /// found, an error is emitted.
//     fn consume_closing_brace(&mut self, arg: &Argument<'_>) -> Option<usize> {
//         self.ws();
//
//         let pos;
//         let description;
//
//         if let Some(&(peek_pos, maybe)) = self.cur.peek() {
//             if maybe == '}' {
//                 self.cur.next();
//                 return Some(peek_pos);
//             }
//
//             pos = peek_pos;
//             description = format!("expected `}}`, found `{}`", maybe.escape_debug());
//         } else {
//             description = "expected `}` but string was terminated".to_owned();
//             // point at closing `"`
//             pos = self.input.len() - if self.append_newline { 1 } else { 0 };
//         }
//
//         let pos = self.to_span_index(pos);
//
//         let label = "expected `}`".to_owned();
//         let (note, secondary_label) = if arg.format.fill == Some('}') {
//             (
//                 Some("the character `}` is interpreted as a fill character because of the `:` that precedes it".to_owned()),
//                 arg.format.fill_span.map(|sp| ("this is not interpreted as a formatting closing brace".to_owned(), sp)),
//             )
//         } else {
//             (
//                 Some("if you intended to print `{`, you can escape it using `{{`".to_owned()),
//                 self.last_opening_brace
//                     .map(|sp| ("because of this opening brace".to_owned(), sp)),
//             )
//         };
//
//         self.errors.push(ParseError {
//             description,
//             note,
//             label,
//             span: pos.to(pos),
//             secondary_label,
//             suggestion: Suggestion::None,
//         });
//
//         None
//     }
//
//     /// Parses all of a string which is to be considered a "raw literal" in a
//     /// format string. This is everything outside of the braces.
//     fn string(&mut self, start: usize) -> &'a str {
//         // we may not consume the character, peek the iterator
//         while let Some(&(pos, c)) = self.cur.peek() {
//             match c {
//                 '{' | '}' => {
//                     return &self.input[start..pos];
//                 }
//                 // '\n' if self.is_source_literal => {
//                 //     self.line_spans.push(self.span(self.cur_line_start, pos));
//                 //     self.cur_line_start = pos + 1;
//                 //     self.cur.next();
//                 // }
//                 _ => {
//                     // if self.is_source_literal && pos == self.cur_line_start && c.is_whitespace() {
//                     //     self.cur_line_start = pos + c.len_utf8();
//                     // }
//                     self.cur.next();
//                 }
//             }
//         }
//         &self.input[start..self.input.len()]
//     }
//
//     /// Parses an `Argument` structure, or what's contained within braces inside the format string.
//     fn argument(&mut self, start: InnerOffset) -> Argument<'a> {
//         let pos = self.position();
//
//         let end = self
//             .cur
//             .clone()
//             .find(|(_, ch)| !ch.is_whitespace())
//             .map_or(start, |(end, _)| self.to_span_index(end));
//         let position_span = start.to(end);
//
//         let format = match self.mode {
//             ParseMode::Format => self.format(),
//             ParseMode::InlineAsm => self.inline_asm(),
//         };
//
//         // Resolve position after parsing format spec.
//         let pos = match pos {
//             Some(position) => position,
//             None => {
//                 let i = self.curarg;
//                 self.curarg += 1;
//                 ArgumentImplicitlyIs(i)
//             }
//         };
//
//         Argument {
//             position: pos,
//             position_span,
//             format,
//         }
//     }
// }
