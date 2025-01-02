#![allow(warnings)]

pub mod parser_winnow {
    use color_eyre::eyre;
    use nom::character::complete::anychar;
    use winnow::ascii::{alpha1, alphanumeric1, digit0, digit1, escaped_transform};
    use winnow::combinator::{alt, cut_err, delimited, eof, opt, permutation, repeat, seq};
    use winnow::error::{ErrMode, ErrorKind, InputError, ParserError};
    use winnow::prelude::*;
    use winnow::stream::AsChar;
    use winnow::token::{any, none_of, one_of, take_while};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Value<'a> {
        String(String),
        // Argument(&'a str),
        Argument(&'a str),
        NewArgument(Argument<'a>),
    }

    impl<'a> Value<'a> {
        pub fn as_argument(&self) -> Option<&str> {
            match self {
                Self::Argument(arg) => Some(arg),
                _ => None,
            }
        }

        pub fn is_argument(&self) -> bool {
            matches!(self, Value::Argument(_) | Value::NewArgument(_))
        }
    }

    fn complete_escaped_transform_internal<I, Error, F, G, Output>(
        input: &mut I,
        normal: &mut F,
        control_char: char,
        transform: &mut G,
    ) -> PResult<Output, Error>
    where
        I: winnow::stream::StreamIsPartial,
        I: winnow::stream::Stream,
        I: winnow::stream::Compare<char>,
        Output: winnow::stream::Accumulate<<I as winnow::stream::Stream>::Slice>,
        F: Parser<I, <I as winnow::stream::Stream>::Slice, Error>,
        G: Parser<I, <I as winnow::stream::Stream>::Slice, Error>,
        Error: ParserError<I>,
    {
        let mut res = Output::initial(Some(input.eof_offset()));
        let mut matched = false;

        while input.eof_offset() > 0 {
            let current_len = input.eof_offset();
            // dbg!(&current_len);

            match opt(normal.by_ref()).parse_next(input)? {
                // match repeat(1.., normal.by_ref()).parse_next(input)? {
                Some(o) => {
                    // dbg!(&o);
                    res.accumulate(o);
                    matched = true;
                    if input.eof_offset() == current_len {
                        // return Ok(res);
                        break;
                    }
                }
                None => {
                    if opt(control_char).parse_next(input)?.is_some() {
                        // if control_char.parse_next(input)?.is_some() {
                        let o = transform.parse_next(input)?;
                        // dbg!(&o);
                        res.accumulate(o);
                        matched = true;
                    } else {
                        // return Ok(res);
                        break;
                    }
                }
            }
        }

        dbg!(&input);
        dbg!(&matched);
        // Ok(res)
        if matched {
            Ok(res)
        } else {
            Err(ErrMode::Backtrack(Error::from_error_kind(
                input,
                ErrorKind::Token,
            )))
        }
        // dbg!(&input.eof_offset());
    }

    fn any_except_curly_bracket0<'a>(s: &mut &'a str) -> PResult<&'a str, InputError<&'a str>> {
        take_while(0.., |c| c != '{' && c != '}').parse_next(s)
    }

    fn any_except_curly_bracket1<'a>(s: &mut &'a str) -> PResult<&'a str, InputError<&'a str>> {
        take_while(1.., |c| c != '{' && c != '}').parse_next(s)
    }

    pub fn consuming_escaped_transform<Input, Error, Normal, Escape, Output>(
        mut normal: Normal,
        control_char: char,
        mut escape: Escape,
    ) -> impl Parser<Input, Output, Error>
    where
        Input: winnow::stream::StreamIsPartial
            + winnow::stream::Stream
            + winnow::stream::Compare<char>,
        Output: winnow::stream::Accumulate<<Input as winnow::stream::Stream>::Slice>,
        Normal: Parser<Input, <Input as winnow::stream::Stream>::Slice, Error>,
        Escape: Parser<Input, <Input as winnow::stream::Stream>::Slice, Error>,
        Error: ParserError<Input>,
    {
        // winnow::combinator::trace("escaped_transform", move |input: &mut Input| {
        move |input: &mut Input| {
            complete_escaped_transform_internal(input, &mut normal, control_char, &mut escape)
        }
    }

    fn escaped_open_bracket<'a>(
        s: &mut &'a str,
        // ) -> PResult<(&'a str, String), InputError<&'a str>> {
    ) -> PResult<String, InputError<&'a str>> {
        consuming_escaped_transform(any_except_curly_bracket1, '{', "{")
            .context("escaped_open_bracket")
            // .parse_peek(s)
            .parse_next(s)
        // consuming_escaped_transform(any_except_curly_bracket1, '{', one_of("{{".value("{")).parse_next(s)
        // complete_escaped_transform_internal(s, any_except_curly_bracket1, '{', "{".value("{"))
        // move |input: &mut &mut &'a str| {
        //     complete_escaped_transform_internal(s, any_except_curly_bracket1, '{', "{".value("{"))
        // }()
        // escaped_transform(any_except_curly_bracket1, '{', "{".value("{")).parse_next(s)
    }

    fn escaped_closed_bracket<'a>(
        s: &mut &'a str,
        // ) -> PResult<(&'a str, String), InputError<&'a str>> {
    ) -> PResult<String, InputError<&'a str>> {
        // consuming_escaped_transform(any_except_curly_bracket1, '}', "}}".value("}")).parse_next(s)
        consuming_escaped_transform(any_except_curly_bracket1, '}', "}")
            .context("escaped_closed_bracket")
            // .parse_peek(s)
            .parse_next(s)
    }

    // pub fn text_including_escaped_brackets<'a>(s: &mut &'a str) -> IResult<&'a str, Value<'a>> {
    pub fn text_including_escaped_brackets<'a>(
        s: &mut &'a str,
    ) -> PResult<Value<'a>, InputError<&'a str>> {
        repeat(
            1..,
            alt((
                // any_except_curly_bracket1.map(ToString::to_string),
                any_except_curly_bracket1,
                "{{".value("{"),
                "}}".value("}"),
                // escaped_open_bracket,
                // escaped_closed_bracket,
            )),
        )
        .fold(String::new, |mut string, c| {
            string.push_str(&c);
            string
        })
        .map(Value::String)
        .context("text_including_escaped_brackets")
        .parse_next(s)

        // repeat(
        //     1..,
        //     alt((
        //         (escaped_open_bracket.map(Some), opt(escaped_closed_bracket)),
        //         (opt(escaped_closed_bracket), escaped_open_bracket.map(Some)),
        //     )),
        // )
        // .fold(String::new, |mut string, (a, b)| {
        //     string.push_str(&a.unwrap_or_default());
        //     string.push_str(&b.unwrap_or_default());
        //     string
        // })
        // .map(Value::String)
        // .context("text_including_escaped_brackets")
        // .parse_next(s)
    }

    pub fn take_until_unbalanced<'a>(
        opening_bracket: char,
        closing_bracket: char,
    ) -> impl FnMut(&mut &'a str) -> PResult<&'a str, InputError<&'a str>> {
        // ) -> impl FnMut(&mut &'a str) -> IResult<&'a str, &'a str> {
        move |i: &mut &'a str| {
            let mut index = 0;
            let mut bracket_counter = 0;
            while let Some(n) = &i[index..].find(&[opening_bracket, closing_bracket, '\\'][..]) {
                index += n;
                let mut it = i[index..].chars();
                match it.next() {
                    Some(c) if c == '\\' => {
                        // Skip the escape char `\`.
                        index += '\\'.len_utf8();
                        // Skip also the following char.
                        if let Some(c) = it.next() {
                            index += c.len_utf8();
                        }
                    }
                    Some(c) if c == opening_bracket => {
                        bracket_counter += 1;
                        index += opening_bracket.len_utf8();
                    }
                    Some(c) if c == closing_bracket => {
                        // Closing bracket.
                        bracket_counter -= 1;
                        index += closing_bracket.len_utf8();
                    }
                    // Can not happen.
                    _ => unreachable!(),
                };
                // We found the unmatched closing bracket.
                if bracket_counter == -1 {
                    // We do not consume it.
                    index -= closing_bracket.len_utf8();
                    // return Ok((&i[index..], &i[0..index]));
                    return Ok(&i[0..index]);
                };
            }

            if bracket_counter == 0 {
                Ok(i)
                // Ok(("", i))
            } else {
                Err(ErrMode::Backtrack(InputError::from_error_kind(
                    i,
                    ErrorKind::Many,
                )))
                // Err(nom::Err::Error(Error::from_error_kind(
                //     i,
                //     ErrorKind::TakeUntil,
                // )))
            }
        }
    }

    // pub fn text_or_argument(s: &str) -> IResult<&str, (&str, &str, &str)> {
    // pub fn non_escaped_bracket_argument<'a>(s: &mut &'a str) -> PResult<&'a str, Value<'a>> {

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Conversion {
        Auto,
        String,
        Repr,
    }

    #[derive(thiserror::Error, Debug)]
    #[error("invalid conversion {0:?}")]
    pub struct InvalidConversionError(char);

    impl TryFrom<char> for Conversion {
        type Error = InvalidConversionError;
        fn try_from(value: char) -> Result<Self, Self::Error> {
            match value {
                'a' => Ok(Self::Auto),
                'r' => Ok(Self::Repr),
                's' => Ok(Self::String),
                other => Err(InvalidConversionError(other)),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Alignment {
        /// The "<" alignment
        Less,
        /// The ">" alignment
        Greater,
        /// The "=" alignment
        Equal,
        /// The "^" alignment
        Caret,
    }

    #[derive(thiserror::Error, Debug)]
    #[error("invalid alignment {0:?}")]
    pub struct InvalidAlignmentError(char);

    impl TryFrom<char> for Alignment {
        type Error = InvalidAlignmentError;
        fn try_from(value: char) -> Result<Self, Self::Error> {
            match value {
                '<' => Ok(Self::Less),
                '>' => Ok(Self::Greater),
                '=' => Ok(Self::Equal),
                '^' => Ok(Self::Caret),
                other => Err(InvalidAlignmentError(other)),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Sign {
        /// The "+" sign
        Plus,
        /// The "-" sign
        Minus,
        /// The " " sign
        Empty,
    }

    #[derive(thiserror::Error, Debug)]
    #[error("invalid sign {0:?}")]
    pub struct InvalidSignError(char);

    impl TryFrom<char> for Sign {
        type Error = InvalidSignError;
        fn try_from(value: char) -> Result<Self, Self::Error> {
            match value {
                '+' => Ok(Self::Plus),
                '-' => Ok(Self::Minus),
                ' ' => Ok(Self::Empty),
                other => Err(InvalidSignError(other)),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Grouping {
        /// The "_" grouping
        Underscore,
        /// The "," grouping
        Comma,
    }

    #[derive(thiserror::Error, Debug)]
    #[error("invalid grouping {0:?}")]
    pub struct InvalidGroupingError(char);

    impl TryFrom<char> for Grouping {
        type Error = InvalidSignError;
        fn try_from(value: char) -> Result<Self, Self::Error> {
            match value {
                '_' => Ok(Self::Underscore),
                ',' => Ok(Self::Comma),
                other => Err(InvalidSignError(other)),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Kind {
        b,
        c,
        d,
        e,
        E,
        f,
        F,
        g,
        G,
        n,
        o,
        s,
        x,
        X,
        Percent,
    }

    #[derive(thiserror::Error, Debug)]
    #[error("invalid type {0:?}")]
    pub struct InvalidKindError(char);

    impl TryFrom<char> for Kind {
        type Error = InvalidKindError;
        fn try_from(value: char) -> Result<Self, Self::Error> {
            match value {
                'b' => Ok(Self::b),
                'c' => Ok(Self::c),
                'd' => Ok(Self::d),
                'e' => Ok(Self::e),
                'E' => Ok(Self::E),
                'f' => Ok(Self::f),
                'F' => Ok(Self::F),
                'g' => Ok(Self::g),
                'G' => Ok(Self::G),
                'n' => Ok(Self::n),
                'o' => Ok(Self::o),
                's' => Ok(Self::s),
                'x' => Ok(Self::x),
                'X' => Ok(Self::X),
                '%' => Ok(Self::Percent),
                other => Err(InvalidKindError(other)),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Encoding {
        /// The "z" grouping
        pub z: bool,
        /// The "#" encoding
        pub hash: bool,
        /// The "0" encoding
        pub zero: bool,
    }

    // #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    // pub enum Encoding {
    //     /// The "z" grouping
    //     Z,
    //     /// The "#" encoding
    //     Hash,
    //     /// The "0" encoding
    //     Zero,
    // }
    //
    // #[derive(thiserror::Error, Debug)]
    // #[error("invalid encoding {0:?}")]
    // pub struct InvalidEncodingError(char);
    //
    // impl TryFrom<char> for Encoding {
    //     type Error = InvalidEncodingError;
    //     fn try_from(value: char) -> Result<Self, Self::Error> {
    //         match value {
    //             'z' => Ok(Self::Z),
    //             '#' => Ok(Self::Hash),
    //             '0' => Ok(Self::Zero),
    //             other => Err(InvalidEncodingError(other)),
    //         }
    //     }
    // }

    #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FormatSpec {
        pub fill: Option<char>,
        pub alignment: Option<Alignment>,
        pub sign: Option<Sign>,
        // pub encoding: Option<Encoding>,
        pub encoding: Encoding,
        pub width: Option<usize>,
        pub grouping: Option<Grouping>,
        pub precision: Option<usize>,
        pub kind: Option<Kind>,
    }

    #[inline(always)]
    pub fn nonzero_digit1<Input, Error>(
        input: &mut Input,
    ) -> PResult<<Input as winnow::stream::Stream>::Slice, Error>
    where
        Input: winnow::stream::StreamIsPartial + winnow::stream::Stream,
        <Input as winnow::stream::Stream>::Token: AsChar + Copy,
        Error: ParserError<Input>,
    {
        winnow::combinator::trace(
            "nonzero_digit0",
            take_while(1.., |c| {
                winnow::stream::AsChar::is_dec_digit(c) && winnow::stream::AsChar::as_char(c) != '0'
            }),
        )
        .parse_next(input)
    }

    pub fn format_spec<'a>(s: &mut &'a str) -> PResult<FormatSpec, InputError<&'a str>> {
        // format_spec     ::=  [[fill]align][sign]["z"]["#"]["0"][width][grouping_option]["." precision][type]
        // fill            ::=  <any character>
        // align           ::=  "<" | ">" | "=" | "^"
        // sign            ::=  "+" | "-" | " "
        // width           ::=  digit+
        // grouping_option ::=  "_" | ","
        // precision       ::=  digit+
        // type            ::=  "b" | "c" | "d" | "e" | "E" | "f" | "F" | "g" | "G" | "n" | "o" | "s" | "x" | "X" | "%"

        // let fill = any.context("fill");
        let fill = take_while(0..=1, |c| true).context("fill");
        let align = || alt(('<', '>', '=', '^')).context("align");
        let sign = alt(('+', '-', ' '));
        let width = (nonzero_digit1, digit0).context("width");
        let grouping_option = alt(('_', ','));
        let precision = digit1;
        let kind = alt((
            'b', 'c', 'd', 'e', 'E', 'f', 'F', 'g', 'G', 'n', 'o', 's', 'x', 'X', '%',
        ));
        (
            opt(alt(((fill, align()), ("", align())))),
            // opt((opt(fill), align)),
            // opt(fill),
            // fill,
            // opt(align),
            opt(sign),
            opt('z'),
            opt('#'),
            opt('0'),
            opt(width),
            opt(grouping_option),
            opt(('.', precision)),
            opt(kind),
        )
            .map(
                |(
                    fill_align,
                    // fill,
                    // align,
                    sign,
                    z,
                    hash,
                    zero,
                    width,
                    grouping_option,
                    precision,
                    kind,
                )| {
                    // let (fill, align) = fill_align;
                    // let fill: &str = fill;
                    // let fill = fill_align.and_then(|(fill, _): (Option<&str>, char)| {
                    //     fill.and_then(|fill| {
                    //         if fill.is_empty() {
                    //             None
                    //         } else {
                    //             fill.chars().next()
                    //         }
                    //     })
                    // });
                    dbg!(fill_align);
                    let fill = fill_align.and_then(|(fill, _): (&str, char)| {
                        // fill.and_then(|fill| {
                        dbg!(&fill);
                        if fill.is_empty() {
                            None
                        } else {
                            fill.chars().next()
                        }
                        // })
                    });

                    let encoding = Encoding {
                        z: z.is_some(),
                        hash: hash.is_some(),
                        zero: zero.is_some(),
                    };

                    // let fill = if fill.is_empty() {
                    //     None
                    // } else {
                    //     fill.chars().next()
                    // };

                    // let alignment = align
                    //     // .map(|(_, align): (Option<&str>, char)| Alignment::try_from(align))
                    //     .map(|align: char| Alignment::try_from(align))
                    //     .transpose()
                    //     .unwrap()

                    let alignment = fill_align
                        .map(|(_, align): (&str, char)| {
                            Alignment::try_from(align)
                            // Alignment::try_from(align.chars().next().unwrap()).unwrap()
                        })
                        // .map(|align: char| Alignment::try_from(align))
                        .transpose()
                        .unwrap();

                    let width = width
                        .map(|(start, cont): (&str, &str)| format!("{start}{cont}").parse())
                        .transpose()
                        .unwrap();

                    FormatSpec {
                        fill,
                        alignment,
                        sign: sign.map(|sign| Sign::try_from(sign)).transpose().unwrap(),
                        encoding,
                        width,
                        grouping: grouping_option
                            .map(|grouping| Grouping::try_from(grouping))
                            .transpose()
                            .unwrap(),
                        precision: precision
                            .map(|(_, precision): (_, &str)| precision.parse())
                            .transpose()
                            .unwrap(),
                        kind: kind
                            .map(|kind: char| Kind::try_from(kind))
                            .transpose()
                            .unwrap(),
                        // ..FormatSpec::default()
                    }
                },
            )
            .context("argument_format_spec")
            .parse_next(s)
    }

    pub fn argument_format_spec<'a>(s: &mut &'a str) -> PResult<FormatSpec, InputError<&'a str>> {
        (':', format_spec)
            .map(|(_, spec)| spec)
            .context("argument_format_spec")
            .parse_next(s)
    }

    pub fn argument_conversion<'a>(s: &mut &'a str) -> PResult<Conversion, InputError<&'a str>> {
        ('!', alt(('r', 's', 'a')))
            .map(|(_, a)| Conversion::try_from(a).unwrap())
            .context("argument_conversion")
            .parse_next(s)
    }

    pub fn python_identifier<'a>(s: &mut &'a str) -> PResult<String, InputError<&'a str>> {
        // identifier   ::=  xid_start xid_continue*
        // id_start     ::=  <all characters in general categories Lu, Ll, Lt, Lm, Lo, Nl, the underscore, and characters with the Other_ID_Start property>
        // id_continue  ::=  <all characters in id_start, plus characters in the categories Mn, Mc, Nd, Pc and others with the Other_ID_Continue property>
        // xid_start    ::=  <all characters in id_start whose NFKC normalization is in "id_start xid_continue*">
        // xid_continue ::=  <all characters in id_continue whose NFKC normalization is in "id_continue*">
        //
        // Lu - uppercase letters

        // Ll - lowercase letters
        // Lt - titlecase letters
        // Lm - modifier letters
        // Lo - other letters
        // Nl - letter numbers
        // Mn - nonspacing marks
        // Mc - spacing combining marks
        // Nd - decimal numbers
        // Pc - connector punctuations
        // Other_ID_Start - explicit list of characters in PropList.txt to support backwards compatibility
        // Other_ID_Continue - likewise

        // The first character of an identifier must be a letter or an underscore.
        // Rule 2	The rest of the identifier can be composed of letters, digits, and underscores.

        // let xid_start = alt((alpha1, '_'));
        // let xid_continue = alt((alphanumeric1, '_'));
        let xid_start = one_of(|c| winnow::stream::AsChar::is_alpha(c));
        let xid_continue = take_while(1.., |c| winnow::stream::AsChar::is_alphanum(c) || c == '_');

        (xid_start, xid_continue)
            .map(|(start, cont): (char, &str)| {
                format!("{start}{cont}")
                // let value
                // start.to_string().push_str(&*cont)
            })
            .context("python_identifier")
            .parse_next(s)
    }

    // pub struct AttributeName(String);
    //
    // pub struct ElementIndex(String);

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ArgumentAccessor<'a> {
        AttributeName(String),
        ElementIndex(&'a str),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ArgumentField<'a> {
        pub name: String,
        pub accessors: Vec<ArgumentAccessor<'a>>,
    }

    pub fn argument_field_name<'a>(
        s: &mut &'a str,
    ) -> PResult<ArgumentField<'a>, InputError<&'a str>> {
        // replacement_field ::=  "{" [field_name] ["!" conversion] [":" format_spec] "}"
        // field_name        ::= arg_name ("." attribute_name | "[" element_index "]")*
        // arg_name          ::=  [identifier | digit+]
        // attribute_name    ::=  identifier
        // element_index     ::=  digit+ | index_string
        // index_string      ::=  <any source character except "]"> +
        // let identifier = python_iden;
        let arg_name = alt((python_identifier, digit1.map(ToString::to_string)));
        let attribute_name =
            ('.', python_identifier).map(|(_, name)| ArgumentAccessor::AttributeName(name));
        // let index_string = take_while(1.., |c| c != ']');
        let element_index = delimited(
            '[',
            take_while(1.., |c| winnow::stream::AsChar::is_dec_digit(c) || c != ']')
                .map(ArgumentAccessor::ElementIndex),
            // alt((digit1, index_string)),
            ']',
        );

        (
            arg_name,
            repeat(0.., alt((attribute_name, element_index))), //     .fold(Vec::new, |mut acc, v| {
                                                               //     acc.push()
                                                               //     acc
                                                               // })
        )
            .map(
                |(name, accessors): (String, Vec<ArgumentAccessor>)| ArgumentField {
                    name,
                    accessors,
                },
            )
            .context("argument_field_name")
            .parse_next(s)
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Argument<'a> {
        // name: &'a str,
        name: ArgumentField<'a>,
        conversion: Option<Conversion>,
        format_spec: Option<FormatSpec>,
    }

    pub fn argument_inner<'a>(s: &mut &'a str) -> PResult<Argument<'a>, InputError<&'a str>> {
        (
            argument_field_name,
            opt(argument_conversion),
            opt(argument_format_spec),
        )
            .map(|(name, conversion, format_spec)| Argument {
                name,
                conversion,
                format_spec,
            })
            .context("argument_inner")
            .parse_next(s)
    }

    pub fn non_escaped_bracket_argument<'a>(
        s: &mut &'a str,
    ) -> PResult<Value<'a>, InputError<&'a str>> {
        delimited(
            "{",
            // argument_inner,
            any_except_curly_bracket0,
            // take_until_unbalanced('{', '}'), // .map(Value::Argument),
            "}",
        )
        // .map(Value::NewArgument)
        .map(|inner| Value::Argument(inner))
        .context("non_escaped_bracket_argument")
        .parse_next(s)
        // (
        //     "{",
        //     take_until_unbalanced('{', '}'), // .map(Value::Argument),
        //     "}",
        // )
        //     .map(|(_, inner, _)| Value::Argument(inner))
        //     .parse_next(s)
        // .map(|(_, inner, _)| Value::Argument(inner))
        // .map(|(_, inner)| Value::Argument(inner))
        // .map(|(_, _, _)| Value::Argument(inner))
        // .parse_next(s)
        // todo!()
        // map(
        //     delimited(("{", take_until_unbalanced('{', '}'), "}")),
        //     |(_, inner, _)| Value::Argument(inner),
        // )(s)
    }

    pub fn text_or_argument<'a>(s: &mut &'a str) -> PResult<Value<'a>, InputError<&'a str>> {
        // repeat(
        //     1..,
        alt((
            text_including_escaped_brackets,
            non_escaped_bracket_argument,
            // eof.value(Value::String("".to_string())),
            // cut_err(non_escaped_bracket_argument),
            // cut_err(text_including_escaped_brackets),
        ))
        // )
        .context("text_or_argument")
        .parse_next(s)
    }

    #[derive(thiserror::Error, Debug, PartialEq, Eq)]
    #[error("invalid format: {format_string:?}")]
    pub struct Error {
        pub format_string: String,
    }

    pub fn parse_format_arguments(value: &str) -> Result<Vec<Value>, Error> {
        let test = repeat(0.., text_or_argument)
            .parse(value)
            .map_err(|_| Error {
                format_string: value.to_string(),
            })?;
        // .map_err(|err| eyre::eyre!(err.to_string()))?;
        return Ok(test);
        // let mut rest = value;
        // let mut values = vec![];
        // let mut i = 0;
        // while let Ok(value) = text_or_argument(&mut rest) {
        //     dbg!(value);
        //     i += 1;
        //     if i >= 10 {
        //         break;
        //     }
        // }
        // Ok(values)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use color_eyre::eyre;
        use nom::character::complete::multispace1;
        use pest::ParseResult;
        use similar_asserts::assert_eq as sim_assert_eq;
        use winnow::{ascii::alphanumeric1, error::ParseError, token::any};

        #[derive(Debug, Eq, PartialEq)]
        pub(crate) struct Color {
            pub(crate) red: u8,
            pub(crate) green: u8,
            pub(crate) blue: u8,
        }

        fn hex_color(input: &mut &str) -> PResult<Color> {
            seq!(Color {
                _: '#',
                red: hex_primary,
                green: hex_primary,
                blue: hex_primary
            })
            .parse_next(input)
        }

        fn hex_primary(input: &mut &str) -> PResult<u8> {
            take_while(2, |c: char| c.is_ascii_hexdigit())
                .try_map(|input| u8::from_str_radix(input, 16))
                .parse_next(input)
        }

        #[test]
        fn parse_fstring_simple() -> eyre::Result<()> {
            crate::tests::init();

            sim_assert_eq!(
                parse_format_arguments("this is a {test} value")?,
                vec![
                    Value::String("this is a ".to_string()),
                    Value::NewArgument(Argument {
                        name: ArgumentField {
                            name: "test".to_string(),
                            accessors: vec![]
                        },
                        conversion: None,
                        format_spec: None,
                    }),
                    Value::String(" value".to_string()),
                ]
            );

            sim_assert_eq!(
                parse_format_arguments("{jane!s}")?,
                vec![
                    // Value::Argument("jane!s"),
                    Value::NewArgument(Argument {
                        name: ArgumentField {
                            name: "jane".to_string(),
                            accessors: vec![]
                        },
                        conversion: Some(Conversion::String),
                        format_spec: None,
                    }),
                ]
            );

            sim_assert_eq!(
                parse_format_arguments("Magic wand: {bag['wand']:^10}")?,
                vec![
                    Value::String("Magic wand: ".to_string()),
                    // Value::Argument("bag['wand']:^10"),
                    Value::NewArgument(Argument {
                        name: ArgumentField {
                            name: "bag".to_string(),
                            accessors: vec![]
                        },
                        conversion: Some(Conversion::String),
                        format_spec: None,
                    }),
                ]
            );
            Ok(())
        }

        // #[test]
        // fn python_parser() -> eyre::Result<()> {
        //     crate::tests::init();
        //
        //     use rustpython_parser::{ast, Parse};
        //     let source = r#"f"Hello, {person['name']}! You're {person['age']} years old.""#;
        //     let statements = ast::Suite::parse(source, "")?;
        //     dbg!(&statements);
        //     // let expr = ast::Expr::parse(source)?;
        //     // let expr = ast::ExprFormattedValue::parse(source, "")?;
        //     let source = r#"Hello, {person['name']}! You're {person['age']} years old."#;
        //     let source = format!(r#"f"{source}""#);
        //     dbg!(&source);
        //     let expr = ast::ExprJoinedStr::parse(&source, "")?;
        //     dbg!(&expr);
        //     Ok(())
        // }

        #[test]
        fn parses_final() {
            // crate::tests::init();

            sim_assert_eq!(
                parse_format_arguments(
                    "{major}.{minor}.{patch}.{dev}{$PR_NUMBER}.dev{distance_to_latest_tag}"
                ),
                Ok(vec![
                    // Value::NewArgument(Argument {
                    //     name: ArgumentField {
                    //         name: "major".to_string(),
                    //         accessors: vec![]
                    //     },
                    //     conversion: None,
                    //     format_spec: None,
                    // }),
                    // Value::NewArgument(Argument {
                    //     name: ArgumentField {
                    //         name: "minor".to_string(),
                    //         accessors: vec![]
                    //     },
                    //     conversion: None,
                    //     format_spec: None,
                    // }),
                    Value::Argument("major"),
                    Value::String(".".to_string()),
                    Value::Argument("minor"),
                    Value::String(".".to_string()),
                    Value::Argument("patch"),
                    Value::String(".".to_string()),
                    Value::Argument("dev"),
                    Value::Argument("$PR_NUMBER"),
                    Value::String(".dev".to_string()),
                    Value::Argument("distance_to_latest_tag"),
                ])
            );
            // sim_assert_eq!(
            //     format_spec.parse(),
            //     Ok(FormatSpec {
            //         alignment: Some(Alignment::Caret),
            //         width: Some(10),
            //         ..FormatSpec::default()
            //     })
            // );
            // Ok(())
        }

        #[test]
        fn parses_format_spec() {
            // centerred with width
            sim_assert_eq!(
                format_spec.parse("^10"),
                Ok(FormatSpec {
                    alignment: Some(Alignment::Caret),
                    width: Some(10),
                    ..FormatSpec::default()
                })
            );

            // two decimal digits
            sim_assert_eq!(
                format_spec.parse(".2f"),
                Ok(FormatSpec {
                    precision: Some(2),
                    kind: Some(Kind::f),
                    ..FormatSpec::default()
                })
            );
            // centered with fill and width
            sim_assert_eq!(
                format_spec.parse("=^30"),
                Ok(FormatSpec {
                    fill: Some('='),
                    alignment: Some(Alignment::Caret),
                    width: Some(30),
                    ..FormatSpec::default()
                })
            );
            // comma as thousand separators
            sim_assert_eq!(
                format_spec.parse(","),
                Ok(FormatSpec {
                    grouping: Some(Grouping::Comma),
                    ..FormatSpec::default()
                })
            );
            /// underscore as thousand separators
            sim_assert_eq!(
                format_spec.parse("_"),
                Ok(FormatSpec {
                    grouping: Some(Grouping::Underscore),
                    ..FormatSpec::default()
                })
            );
            // comma as thousand separators and two decimals
            sim_assert_eq!(
                format_spec.parse(",.2f"),
                Ok(FormatSpec {
                    grouping: Some(Grouping::Comma),
                    precision: Some(2),
                    kind: Some(Kind::f),
                    ..FormatSpec::default()
                })
            );

            // date: 02
            sim_assert_eq!(
                format_spec.parse("02"),
                Ok(FormatSpec {
                    // fill: Some('0'),
                    fill: None,
                    encoding: Encoding {
                        zero: true,
                        ..Encoding::default()
                    },
                    width: Some(2),
                    ..FormatSpec::default()
                })
            );

            // date: %m/%d/%Y
            // sim_assert_eq!(
            //     format_spec.parse("%m/%d/%Y"),
            //     Ok(FormatSpec {
            //         fill: Some('='),
            //         alignment: Some(Alignment::Caret),
            //         width: Some(30),
            //         ..FormatSpec::default()
            //     })
            // );
        }

        #[test]
        fn parses_invalid_identifiers() -> eyre::Result<()> {
            crate::tests::init();

            if false {
                sim_assert_eq!(
                    hex_color.parse("#fcba03"),
                    Ok(Color {
                        red: 252,
                        green: 186,
                        blue: 3
                    })
                );

                sim_assert_eq!(
                    // repeat(0.., alphanumeric1::<&str, InputError<&str>>)
                    repeat(
                        0..,
                        // alt((alphanumeric1::<&str, InputError<&str>>, multispace1))
                        // alt((alphanumeric1::<&str, &str>, multispace1::<&str, &str>))
                        any::<_, InputError<&str>>,
                    )
                    .fold(String::new, |mut string, c| {
                        string.push(c);
                        string
                    })
                    .parse("helo world"),
                    // .map_err(|err| err.to_string()),
                    Ok("helo world".to_string()),
                    // Ok(vec!["helo world"]),
                    // Ok("helo world") as Result<&str, ParseError<&str, &str>>
                );

                sim_assert_eq!(
                    text_including_escaped_brackets.parse(" helo world"),
                    Ok(Value::String(" helo world".to_string()))
                );

                sim_assert_eq!(
                    text_including_escaped_brackets.parse(" helo {{ world }}"),
                    Ok(Value::String(" helo { world }".to_string()))
                );

                sim_assert_eq!(
                    non_escaped_bracket_argument.parse("{test}"),
                    Ok(Value::Argument("test"))
                );
                // .map_err(|err| eyre::eyre!(err.to_string()))?);

                sim_assert_eq!(
                    repeat(1.., text_or_argument).parse("this is a {test} for parsing {arguments}"),
                    Ok(vec![
                        // Value::String(" helo { world }".to_string()),
                        Value::String("this is a ".to_string()),
                        Value::Argument("test"),
                        Value::String(" for parsing ".to_string()),
                        Value::Argument("arguments"),
                    ])
                );

                sim_assert_eq!(
                    parse_format_arguments("this }} {{ is a ")?,
                    vec![
                        Value::String("this } { is a ".to_string()),
                        // Value::Argument("test"),
                        // Value::String(" for parsing ".to_string()),
                        // Value::Argument("arguments"),
                    ]
                );

                sim_assert_eq!(
                    non_escaped_bracket_argument.parse("{}"),
                    Ok(Value::Argument(""))
                );
            }

            sim_assert_eq!(
                text_including_escaped_brackets.parse(" helo {{ world }}"),
                Ok(Value::String(" helo { world }".to_string()))
            );

            sim_assert_eq!(
                parse_format_arguments("this }} {{ is a {test}")?,
                vec![
                    Value::String("this } { is a ".to_string()),
                    Value::Argument("test"),
                ]
            );

            sim_assert_eq!(
                parse_format_arguments("this }} {{ is a {test} for parsing {arguments}")?,
                vec![
                    Value::String("this } { is a ".to_string()),
                    Value::Argument("test"),
                    Value::String(" for parsing ".to_string()),
                    Value::Argument("arguments"),
                ]
            );

            Ok(())
        }

        #[test]
        fn test_take_until_unmatched() {
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "abc"),
                Ok("abc") // Ok(("", "abc"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "url)abc"),
                Ok("url") // Ok((")abc", "url"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "url)abc\\"),
                Ok("url") // Ok((")abc\\", "url"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "u()rl)abc"),
                Ok("u()rl") // Ok((")abc", "u()rl"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "u(())rl)abc"),
                Ok("u(())rl") // Ok((")abc", "u(())rl"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "u\\(())rl)abc"),
                Ok("u\\(()") // Ok((")rl)abc", "u\\(()"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "u(()\\)rl)abc"),
                Ok("u(()\\)rl)abc") // Ok(("", "u(()\\)rl)abc"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "u(())r()l)abc"),
                Ok("u(())r()l") // Ok((")abc", "u(())r()l"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "u(())r()labc"),
                Ok("u(())r()labc") // Ok(("", "u(())r()labc"))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut r#"u\((\))r()labc"#),
                Ok(r#"u\((\))r()labc"#) // Ok(("", r#"u\((\))r()labc"#))
            );
            sim_assert_eq!(
                take_until_unbalanced('(', ')').parse_next(&mut "u(())r(labc"),
                Err(ErrMode::Backtrack(
                    winnow::error::InputError::from_error_kind(&"u(())r(labc", ErrorKind::Many)
                ))
            );
            sim_assert_eq!(
                take_until_unbalanced('€', 'ü').parse_next(&mut "€uü€€üürlüabc"),
                Ok("€uü€€üürl") // Ok(("üabc", "€uü€€üürl"))
            );
        }
    }
}

pub mod parser_nom {
    use color_eyre::eyre;
    use nom::{
        branch::alt,
        bytes::complete::{escaped, escaped_transform, is_not, tag, take_while},
        character::complete::{alpha0, alpha1, alphanumeric0, alphanumeric1, none_of, one_of},
        combinator::{eof, map, value},
        complete::*,
        error::{Error, ErrorKind, ParseError},
        multi::{fold_many1, many0, many1},
        sequence::tuple,
        Finish, IResult,
    };

    // /// # Basic usage
    // /// ```
    // /// use nom::bytes::complete::tag;
    // /// use nom::sequence::delimited;
    // /// use parse_hyperlinks::take_until_unbalanced;
    // ///
    // /// let mut parser = delimited(tag("<"), take_until_unbalanced('<', '>'), tag(">"));
    // /// assert_eq!(parser("<<inside>inside>abc"), Ok(("abc", "<inside>inside")));
    // /// ```
    // /// It skips nested brackets until it finds an extra unbalanced closing bracket. Escaped brackets
    // /// like `\<` and `\>` are not considered as brackets and are not counted. This function is
    // /// very similar to `nom::bytes::complete::take_until(">")`, except it also takes nested brackets.

    /// A parser similar to `nom::bytes::complete::take_until()`, except that this
    /// one does not stop at balanced opening and closing tags. It is designed to
    /// work inside the `nom::sequence::delimited()` parser.
    ///
    pub fn take_until_unbalanced(
        opening_bracket: char,
        closing_bracket: char,
    ) -> impl Fn(&str) -> IResult<&str, &str> {
        move |i: &str| {
            let mut index = 0;
            let mut bracket_counter = 0;
            while let Some(n) = &i[index..].find(&[opening_bracket, closing_bracket, '\\'][..]) {
                index += n;
                let mut it = i[index..].chars();
                match it.next() {
                    Some(c) if c == '\\' => {
                        // Skip the escape char `\`.
                        index += '\\'.len_utf8();
                        // Skip also the following char.
                        if let Some(c) = it.next() {
                            index += c.len_utf8();
                        }
                    }
                    Some(c) if c == opening_bracket => {
                        bracket_counter += 1;
                        index += opening_bracket.len_utf8();
                    }
                    Some(c) if c == closing_bracket => {
                        // Closing bracket.
                        bracket_counter -= 1;
                        index += closing_bracket.len_utf8();
                    }
                    // Can not happen.
                    _ => unreachable!(),
                };
                // We found the unmatched closing bracket.
                if bracket_counter == -1 {
                    // We do not consume it.
                    index -= closing_bracket.len_utf8();
                    return Ok((&i[index..], &i[0..index]));
                };
            }

            if bracket_counter == 0 {
                Ok(("", i))
            } else {
                Err(nom::Err::Error(Error::from_error_kind(
                    i,
                    ErrorKind::TakeUntil,
                )))
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Value<'a> {
        // String(&'a str),
        String(String),
        Argument(&'a str),
    }

    // pub fn escaped_or_non_bracket0(s: &str) -> impl Fn(&str) -> IResult<&str, &str> {
    // pub fn escaped_or_non_bracket0(s: &str) -> IResult<&str, &str> {
    pub fn text_including_escaped_brackets<'a>(s: &'a str) -> IResult<&'a str, Value<'a>> {
        // escaped(alphanumeric0, '{', one_of(r"{"))(s)
        // TODO: use escaped transform here...
        // map(escaped(none_of("{"), '{', one_of(r"{")), Value::String)(s)
        let escaped_open_bracket = escaped_transform(none_of("{}"), '{', one_of(r"{"));
        let escaped_closed_bracket = escaped_transform(none_of("{}"), '}', one_of(r"}"));
        map(
            // fold_many1(
            // many1(
            alt((
                escaped_open_bracket,
                escaped_closed_bracket,
                // map(eof, ToString::to_string),
                // alt((escaped_open_bracket, map(tag(""), ToString::to_string))),
                // alt((escaped_closed_bracket, map(tag(""), ToString::to_string))),
            )),
            // String::new(),
            // |values| values.join(""),
            // |values| values.join(""),
            // ),
            // |test| Value::String(test.join("")),
            |test| Value::String(test),
            // |(a, b)| Value::String(a),
        )(s)

        // map(
        //     // fold_many1(
        //     // many1(
        //     alt((
        //         many1(escaped_open_bracket),
        //         many1(escaped_closed_bracket),
        //         // map(eof, ToString::to_string),
        //         // alt((escaped_open_bracket, map(tag(""), ToString::to_string))),
        //         // alt((escaped_closed_bracket, map(tag(""), ToString::to_string))),
        //     )),
        //     // String::new(),
        //     // |values| values.join(""),
        //     // |values| values.join(""),
        //     // ),
        //     |test| Value::String(test.join("")),
        //     // |test| Value::String(test),
        //     // |(a, b)| Value::String(a),
        // )(s)
    }

    // pub fn text_or_argument(s: &str) -> IResult<&str, (&str, &str, &str)> {
    pub fn non_escaped_bracket_argument(s: &str) -> IResult<&str, Value> {
        map(
            // tuple((tag("{"), take_while(none_of("{}")), tag("}"))),
            // tuple((tag("{"), none_of("{}")), tag("}"))),
            tuple((tag("{"), take_until_unbalanced('{', '}'), tag("}"))),
            |(_, inner, _)| Value::Argument(inner),
        )(s)
    }

    pub fn text_or_argument(s: &str) -> IResult<&str, Value> {
        alt((
            text_including_escaped_brackets,
            non_escaped_bracket_argument,
        ))(s)
    }

    pub fn parse_format_arguments(value: &str) -> eyre::Result<Vec<Value>> {
        let mut rest = value;
        let mut values = vec![];
        while let Ok((remaining, value)) = text_or_argument(rest) {
            values.push(value);
            rest = remaining;
            if rest.is_empty() {
                break;
            }
        }
        Ok(values)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_take_until_unmatched() {
            assert_eq!(take_until_unbalanced('(', ')')("abc"), Ok(("", "abc")));
            assert_eq!(
                take_until_unbalanced('(', ')')("url)abc"),
                Ok((")abc", "url"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("url)abc\\"),
                Ok((")abc\\", "url"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("u()rl)abc"),
                Ok((")abc", "u()rl"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("u(())rl)abc"),
                Ok((")abc", "u(())rl"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("u\\(())rl)abc"),
                Ok((")rl)abc", "u\\(()"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("u(()\\)rl)abc"),
                Ok(("", "u(()\\)rl)abc"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("u(())r()l)abc"),
                Ok((")abc", "u(())r()l"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("u(())r()labc"),
                Ok(("", "u(())r()labc"))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')(r#"u\((\))r()labc"#),
                Ok(("", r#"u\((\))r()labc"#))
            );
            assert_eq!(
                take_until_unbalanced('(', ')')("u(())r(labc"),
                Err(nom::Err::Error(nom::error::Error::new(
                    "u(())r(labc",
                    ErrorKind::TakeUntil
                )))
            );
            assert_eq!(
                take_until_unbalanced('€', 'ü')("€uü€€üürlüabc"),
                Ok(("üabc", "€uü€€üürl"))
            );
        }

        #[test]
        fn parses_escaped_open_bracket() -> eyre::Result<()> {
            // let mut escaped_or_non_bracket = tuple((
            //     // tag("{"),
            //     // escaped(none_of(r"{{"), '{', one_of(r"{")),
            //     escaped(alphanumeric0, '{', one_of(r"{")),
            //     // escaped_transform(none_of(r"{{"), '{', one_of(r"{")),
            //     // tag("/"),
            // ));
            // let test: IResult<&str, (&str,)> = escaped_or_non_bracket(r"{{test");
            // let test: IResult<&str, (&str,)> = escaped_or_non_bracket(r"{{test");

            // sim_assert_eq!(test, Ok(("{", ("test",))));
            // sim_assert_eq!(escaped_or_non_bracket0(r"{{test"), Ok(("{", ("test",))));

            // working?
            // sim_assert_eq!(escaped_or_non_bracket0(r"test"), Ok(("", "test")));
            // sim_assert_eq!(escaped_or_non_bracket0(r"{{test"), Ok(("", "{{test")));
            // sim_assert_eq!(
            //     escaped_or_non_bracket0(r"test{{test"),
            //     Ok(("", "test{{test"))
            // );
            // dbg!(text_including_escaped_brackets(r"test {{ test "));
            // dbg!(text_including_escaped_brackets(r"test {{ {{ test "));

            // dbg!(text_including_escaped_brackets("").finish());
            // dbg!(escaped_transform(
            //     alpha1,
            //     '\\',
            //     alt((
            //         value("\\", tag("\\")),
            //         value("\"", tag("\"")),
            //         value("\n", tag("n")),
            //     ))
            // )("")
            // .finish() as Result<(&str, String), nom::error::Error<_>>);
            // as IResult<&str, String>);

            fn escaped_open_bracket<'a>(s: &'a str) -> IResult<&'a str, String> {
                escaped_transform(none_of("{}"), '{', one_of(r"{"))(s)
            }

            fn escaped_closed_bracket<'a>(s: &'a str) -> IResult<&'a str, String> {
                escaped_transform(
                    none_of("{}"),
                    '}',
                    alt((
                        value("}", tag("}")),
                        // value("\\", tag("\\")),
                        // value("\"", tag("\"")),
                        // value("\n", tag("n")),
                    )), // one_of(r"}"),
                )(s)
            }

            // dbg!(escaped_open_bracket("}} done").finish());
            // dbg!(escaped_closed_bracket("}} done").finish());
            // dbg!(escaped_closed_bracket("").finish());
            // dbg!(many1(escaped_open_bracket)("}} done").finish());
            // dbg!(many1(escaped_closed_bracket)("}} done").finish());
            // dbg!(alt((escaped_open_bracket, escaped_closed_bracket))("}} done").finish());
            // dbg!(alt((many1(escaped_open_bracket), many1(escaped_closed_bracket)))("}} done").finish());

            let rest = r"test }} {{ {{ test ";

            if false {
                let mut it =
                    nom::combinator::iterator(rest, many1(text_including_escaped_brackets));
                let parsed = it.collect::<Vec<_>>();
                // let parsed = it.map(|v| (v, v.len())).collect::<HashMap<_, _>>();
                let res: IResult<_, _> = it.finish();
                dbg!(&parsed);
                dbg!(&res);

                // dbg!(text_including_escaped_brackets(rest).finish());
                // dbg!(
                //     // text_including_escaped_brackets(rest).finish() as Result<(&str, Value), _>
                //     nom::sequence::terminated(many0(text_including_escaped_brackets), eof)(rest)
                //         .finish() as Result<(&str, Vec<Value>), _> // text_including_escaped_brackets(rest).finish() as Result<(&str, Vec<Value>), _>
                //                                                    // text_including_escaped_brackets(rest).finish() as Result<(&str, Value), _> // text_including_escaped_brackets(rest).finish() as Result<(&str, Vec<Value>), _>
                // );
                // as IResult<&str, Vec<Value>>

                // let rest = r"test }} {{ {{ test ";
                let rest = r"test {value} {{ hello }} ";
                // dbg!(text_including_escaped_brackets(rest).finish());
                dbg!(many1(alt((
                    text_including_escaped_brackets,
                    non_escaped_bracket_argument,
                    value(Value::String("".to_string()), eof),
                    // map(eof, || Value::String("")),
                )))(rest)
                .finish() as Result<(&str, Vec<Value>), _>);
                // as IResult<&str, Vec<Value>>
            }

            // dbg!(text_including_escaped_brackets(rest)
            //     .err()
            //     .unwrap()
            //     .to_string());

            //             map(eof, ToString::to_string),

            let (rest, _) = dbg!(text_including_escaped_brackets(rest))?;
            let (rest, _) = dbg!(text_including_escaped_brackets(rest))?;
            let (rest, _) = dbg!(text_including_escaped_brackets(rest))?;
            let (rest, _) = dbg!(text_including_escaped_brackets(rest))?;

            fn parse<'a>(s: &'a str) -> IResult<&'a str, Value<'a>> {
                alt((
                    text_including_escaped_brackets,
                    non_escaped_bracket_argument,
                ))(s)
            }

            let value = r"test {value} {{ hello }} ";
            let mut rest = value;
            let mut values = vec![];
            while let Ok((remaining, value)) = parse(rest) {
                // dbg!(value);
                values.push(value);
                rest = remaining;
                if rest.is_empty() {
                    break;
                }
            }
            dbg!(&values);
            // let rest = va

            // let (rest, _) = dbg!(parse(rest))?;
            // let (rest, _) = dbg!(parse(rest))?;
            // let (rest, _) = dbg!(parse(rest))?;
            // let (rest, _) = dbg!(parse(rest))?;
            // dbg!(text_including_escaped_brackets(value));
            // dbg!(map(
            //     escaped_transform(none_of("{}"), '}', one_of(r"}")),
            //     Value::String
            // )(r"test }} {{ {{ test ") as IResult<&str, Value>);
            // dbg!(non_escaped_bracket_argument(r"{value}"));
            // dbg!(non_escaped_bracket_argument(r"{{value}"));
            // dbg!(non_escaped_bracket_argument(r"{{{value}}}"));

            // dbg!(text_or_argument(r"test {{ test {value} rest"));
            // sim_assert_eq!(
            //     text_or_argument(r"test {{ test {value} rest"),
            //     // Ok(("{value} rest", "test {{ test "))
            //     Ok(("{value} rest", "test {{ test "))
            // );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::take_until_unbalanced;
    use super::parser_nom;
    use color_eyre::eyre;
    // use nom::{
    //     branch::alt,
    //     bytes::complete::{escaped, escaped_transform, is_not, tag, take_while},
    //     character::complete::{alpha0, alpha1, alphanumeric0, alphanumeric1, none_of, one_of},
    //     combinator::{eof, map, value},
    //     complete::*,
    //     error::ErrorKind,
    //     multi::{fold_many1, many0, many1},
    //     sequence::tuple,
    //     Finish, IResult,
    // };
    use similar_asserts::assert_eq as sim_assert_eq;

    static INIT: std::sync::Once = std::sync::Once::new();

    /// Initialize test
    ///
    /// This ensures `color_eyre` is setup once.
    pub fn init() {
        INIT.call_once(|| {
            color_eyre::install().ok();
        });
    }

    // fn parse_format_arguments(value: &str) -> eyre::Result<()> {
    //     // let test: IResult<&str, &str> = is_not("{}")(value);
    //     // let bracket_open = tuple((
    //     //     tag("/"),
    //     //     escaped_transform(none_of(r"\/"), '\\', one_of(r"/")),
    //     //     tag("/"),
    //     // ));
    //
    //     let mut escaped_or_non_bracket = tuple((
    //         // tag("{"),
    //         escaped(none_of(r"{{"), '{', one_of(r"{")),
    //         // escaped_transform(none_of(r"{{"), '{', one_of(r"{")),
    //         // tag("/"),
    //     ));
    //     let test: IResult<&str, (&str,)> = escaped_or_non_bracket(r"{{test");
    //     // let test: IResult<&str, (&str,)> = escaped_or_non_bracket(r"{{test");
    //
    //     sim_assert_eq!(test?, ("{", ("test",)));
    //     // sim_assert_eq!(test?, ("{", ("test", "{".to_string())));
    //     // let test: IResult<&str, &str> = bracket_open(value);
    //     // dbg!(&test);
    //     Ok(())
    // }

    #[test]
    fn parses_invalid_identifiers() -> eyre::Result<()> {
        crate::tests::init();

        let value = "this is a {test} for parsing {arguments}";
        let args = parser_nom::parse_format_arguments(&value)?;
        sim_assert_eq!(
            args,
            vec![
                parser_nom::Value::String("this is a ".to_string()),
                parser_nom::Value::Argument("test"),
                parser_nom::Value::String(" for parsing ".to_string()),
                parser_nom::Value::Argument("arguments"),
            ]
        );
        Ok(())
    }
}
