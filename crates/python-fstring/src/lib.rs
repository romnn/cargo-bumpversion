#![allow(warnings)]

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
