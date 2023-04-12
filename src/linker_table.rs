use std::num::ParseIntError;

use crate::map::{c_name, hex, padded, Line};
use nom::{
    bytes::complete::tag,
    character::complete::char,
    combinator::map,
    error::{FromExternalError, ParseError},
    sequence::{pair, terminated},
    IResult, Parser,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Entry<S> {
    pub name: S,
    pub virt_addr: u32,
}

pub fn title<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Line<&'a str>, E> {
    map(tag("Linker generated symbols:"), |_| Line::LinkerTitle)(input)
}

pub fn entry<'a, E>(input: &'a str) -> IResult<&'a str, Entry<&'a str>, E>
where
    E: ParseError<&'a str>
        + FromExternalError<&'a str, ParseIntError>
        + FromExternalError<&'a str, &'static str>,
{
    map(
        pair(terminated(padded(25).and_then(c_name), char(' ')), hex(8)),
        |(name, virt_addr)| Entry { name, virt_addr },
    )(input)
}

#[cfg(test)]
mod tests {
    use super::{entry, title};
    use crate::{linker_table::Entry, utils::test_utils::assert_diff};
    use nom::{
        branch::alt,
        combinator::{all_consuming, map},
    };
    use nom_supreme::error::ErrorTree;

    #[test]
    fn test_linker_table() {
        use crate::map::Line;

        let input = "\
Linker generated symbols:\r\n\
\x20          _db_stack_addr 804f0c00\r\n\
\x20                  _ctors 00000000\r\n\
"
        .split_terminator("\r\n")
        .collect::<Vec<_>>();

        let expected: Vec<Line<&str>> = vec![
            Line::LinkerTitle,
            Line::LinkerEntry(Entry {
                name: "_db_stack_addr",
                virt_addr: 0x804f0c00,
            }),
            Line::LinkerEntry(Entry {
                name: "_ctors",
                virt_addr: 0,
            }),
        ];

        let mut parser = alt::<_, _, ErrorTree<&str>, _>((
            all_consuming(map(title, |_| Line::LinkerTitle)),
            all_consuming(map(entry, Line::LinkerEntry)),
        ));

        let (input_len, expected_len) = (&input.len(), &expected.len());

        // TODO: Factor out test boilerplate
        for (input, expected) in input.into_iter().zip(expected) {
            let actual = parser(input);
            match actual {
                Ok((_, actual)) => assert_diff(&expected, &actual),
                Err(err) => panic!("{err:#?}"),
            }
        }

        assert_eq!(input_len, expected_len);
    }
}
