use crate::map::{
    hex, padded, section_name, DebugSectionName, Line, SectionName,
};
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::char,
    combinator::{eof, map, recognize},
    error::{FromExternalError, ParseError},
    multi::count,
    sequence::{preceded, terminated, tuple},
    IResult, Parser,
};
use std::num::ParseIntError;

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum Data<S> {
    Main {
        name: SectionName<S>,
        virt_addr: u32,
    },
    Debug {
        name: DebugSectionName,
    },
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Entry<S> {
    pub data: Data<S>,
    pub size: u32,
    pub file_addr: u32,
}

pub fn title<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(tag("Memory map:"))(input)
}

pub fn columns0<'a, E>(input: &'a str) -> IResult<&'a str, Line<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(
        tuple((
            count(char(' '), 19),
            tag("Starting"),
            char(' '),
            tag("Size"),
            count(char(' '), 5),
            tag("File"),
        )),
        |_| Line::MemoryColumns0,
    )(input)
}

pub fn columns1<'a, E>(input: &'a str) -> IResult<&'a str, Line<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(
        tuple((
            count(char(' '), 19),
            tag("address"),
            count(char(' '), 11),
            tag("Offset"),
        )),
        |_| Line::MemoryColumns1,
    )(input)
}

pub fn entry<'a, E>(input: &'a str) -> IResult<&'a str, Entry<&'a str>, E>
where
    E: ParseError<&'a str>
        + FromExternalError<&'a str, ParseIntError>
        + FromExternalError<&'a str, &'static str>,
{
    map(
        tuple((
            terminated(padded(17).and_then(section_name), count(char(' '), 2)),
            terminated(hex(8), char(' ')),
            terminated(hex(8), char(' ')),
            hex(8),
        )),
        |(name, virt_addr, size, file_addr)| Entry {
            data: Data::Main { name, virt_addr },
            size,
            file_addr,
        },
    )(input)
}

fn debug_section_name<'a, E>(
    input: &'a str,
) -> IResult<&'a str, DebugSectionName, E>
where
    E: ParseError<&'a str>,
{
    use DebugSectionName::*;

    preceded(
        char('.'),
        alt((
            map(tag("line"), |_| Line),
            preceded(
                tag("debug"),
                alt((
                    map(eof, |_| Main),
                    preceded(
                        char('_'),
                        alt((
                            map(tag("abbrev"), |_| Abbrev),
                            map(tag("aranges"), |_| Aranges),
                            map(tag("info"), |_| Info),
                            map(tag("line"), |_| Info),
                            map(tag("sfnames"), |_| SfNames),
                            map(tag("srcinfo"), |_| SrcInfo),
                            map(tag("str"), |_| Str),
                        )),
                    ),
                )),
            ),
        )),
    )(input)
}

pub fn debug_entry<'a, E>(
    input: &'a str,
) -> IResult<&'a str, Entry<&'a str>, E>
where
    E: ParseError<&'a str>
        + FromExternalError<&'a str, ParseIntError>
        + FromExternalError<&'a str, &'static str>,
{
    map(
        tuple((
            terminated(
                padded(17).and_then(debug_section_name),
                count(char(' '), 11),
            ),
            terminated(hex(6), char(' ')),
            hex(8),
        )),
        |(name, size, file_addr)| Entry {
            data: Data::Debug { name },
            size,
            file_addr,
        },
    )(input)
}

#[cfg(test)]
mod tests {
    use super::{title, Data, Entry};
    use crate::{
        map::{DebugSectionName, SectionName},
        memory_table::{columns0, columns1, debug_entry, entry},
        utils::test_utils::assert_diff,
    };
    use nom::{
        branch::alt,
        combinator::{all_consuming, map},
    };
    use nom_supreme::error::ErrorTree;

    #[test]
    fn test_memory_map() {
        use crate::map::Line;

        let input = "\
Memory map:\r\n\
\x20                  Starting Size     File\r\n\
\x20                  address           Offset\r\n\
\x20           .init  80003100 000023a8 000001c0\r\n\
\x20          _extab  800054c0 000006a8 00002580\r\n\
\x20     _extabindex  80005b80 00000a1c 00002c40\r\n\
\x20  .debug_srcinfo           000000 00000000\r\n\
\x20  .debug_sfnames           000000 00000000\r\n\
\x20          .debug           000000 00000000\r\n\
\x20           .line           000000 00000000\r\n\
"
        .split_terminator("\r\n")
        .collect::<Vec<_>>();

        let expected: Vec<Line<&str>> = vec![
            Line::MemoryTitle,
            Line::MemoryColumns0,
            Line::MemoryColumns1,
            Line::MemoryEntry(Entry {
                data: Data::Main {
                    name: SectionName::Init,
                    virt_addr: 0x80003100,
                },
                size: 0x23a8,
                file_addr: 0x1c0,
            }),
            Line::MemoryEntry(Entry {
                data: Data::Main {
                    name: SectionName::ExTab,
                    virt_addr: 0x800054c0,
                },
                size: 0x6a8,
                file_addr: 0x2580,
            }),
            Line::MemoryEntry(Entry {
                data: Data::Main {
                    name: SectionName::ExTabIndex,
                    virt_addr: 0x80005b80,
                },
                size: 0xa1c,
                file_addr: 0x2c40,
            }),
            Line::MemoryEntry(Entry {
                data: Data::Debug {
                    name: DebugSectionName::SrcInfo,
                },
                size: 0,
                file_addr: 0,
            }),
            Line::MemoryEntry(Entry {
                data: Data::Debug {
                    name: DebugSectionName::SfNames,
                },
                size: 0,
                file_addr: 0,
            }),
            Line::MemoryEntry(Entry {
                data: Data::Debug {
                    name: DebugSectionName::Main,
                },
                size: 0,
                file_addr: 0,
            }),
            Line::MemoryEntry(Entry {
                data: Data::Debug {
                    name: DebugSectionName::Line,
                },
                size: 0,
                file_addr: 0,
            }),
        ];

        let mut parser = alt::<_, _, ErrorTree<&str>, _>((
            all_consuming(map(title, |_| Line::MemoryTitle)),
            all_consuming(map(columns0, |_| Line::MemoryColumns0)),
            all_consuming(map(columns1, |_| Line::MemoryColumns1)),
            all_consuming(map(alt((entry, debug_entry)), Line::MemoryEntry)),
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
