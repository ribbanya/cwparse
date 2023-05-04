use crate::map::{
    hex, identifier, origin, padded, section_name, Identifier, Line, Origin,
    SectionName,
};
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1},
    combinator::{map, map_res, opt},
    error::{FromExternalError, ParseError},
    multi::count,
    sequence::{delimited, pair, terminated, tuple},
    IResult, Parser,
};
use std::num::ParseIntError;

#[derive(Debug, Eq, PartialEq)]
pub enum Data<S: Eq + PartialEq> {
    Parent { size: u32, align: u8 },
    Child { parent: Identifier<S> },
}

#[derive(Debug, Eq, PartialEq)]
pub struct Addrs {
    section: u32,
    virual: u32,
    file: Option<u32>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Symbol<S: Eq + PartialEq> {
    pub addrs: Option<Addrs>,
    pub data: Data<S>,
    pub id: Identifier<S>,
    pub origin: Origin<S>,
}

pub fn title<'a, E>(
    input: &'a str,
) -> IResult<&'a str, SectionName<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    terminated(section_name, tag(" section layout"))(input)
}

pub fn columns0<'a, E>(input: &'a str) -> IResult<&'a str, Line<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(
        tuple((
            count(char(' '), 2),
            tag("Starting"),
            count(char(' '), 8),
            tag("Virtual"),
        )),
        |_| Line::SectionColumns0,
    )(input)
}

pub fn columns1<'a, E>(input: &'a str) -> IResult<&'a str, Line<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(
        tuple((
            count(char(' '), 2),
            tag("address"),
            count(char(' '), 2),
            tag("Size"),
            count(char(' '), 3),
            tag("address"),
        )),
        |_| Line::SectionColumns1,
    )(input)
}

pub fn separator<'a, E>(input: &'a str) -> IResult<&'a str, Line<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(pair(count(char(' '), 2), count(char('-'), 23)), |_| {
        Line::SectionSeparator
    })(input)
}

pub fn symbol<'a, E>(input: &'a str) -> IResult<&'a str, Symbol<&'a str>, E>
where
    E: ParseError<&'a str>
        + FromExternalError<&'a str, ParseIntError>
        + FromExternalError<&'a str, &'static str>,
{
    map(
        tuple((
            delimited(count(char(' '), 2), hex(8), char(' ')),
            terminated(alt((parent, child)), alt((tag("\x20\t"), tag("\t")))),
            origin,
        )),
        |(addr, (virt_addr, file_addr, data, id), origin)| Symbol {
            addr,
            virt_addr,
            file_addr,
            data,
            id,
            origin,
        },
    )(input)
}

fn align<'a, E>(input: &'a str) -> IResult<&'a str, u8, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
    E: ParseError<&'a str> + FromExternalError<&'a str, &'static str>,
{
    map_res(padded(2).and_then(digit1), |n| u8::from_str_radix(n, 10))(input)
}

fn unused<'a, E>(
    input: &'a str,
) -> IResult<&'a str, (Option<u32>, Data<&'a str>, Identifier<&'a str>), E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map(
        tuple((
            terminated(count(char('0'), 6), char(' ')),
            terminated(hex(8), char(' ')),
            opt(terminated(hex(8), char(' '))),
            terminated(identifier, char(' ')),
            parent_identifier,
        )),
        |(_, virt_addr, file_addr, id, parent)| {
            (virt_addr, file_addr, Data::Child { parent }, id)
        },
    )(input)
}

fn child<'a, E>(
    input: &'a str,
) -> IResult<&'a str, (u32, Option<u32>, Data<&'a str>, Identifier<&'a str>), E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map(
        tuple((
            terminated(count(char('0'), 6), char(' ')),
            terminated(hex(8), char(' ')),
            opt(terminated(hex(8), char(' '))),
            terminated(identifier, char(' ')),
            parent_identifier,
        )),
        |(_, virt_addr, file_addr, id, parent)| {
            (virt_addr, file_addr, Data::Child { parent }, id)
        },
    )(input)
}

fn parent_identifier<'a, E>(
    input: &'a str,
) -> IResult<&'a str, Identifier<&'a str>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    delimited(tag("(entry of "), identifier, char(')'))(input)
}

fn parent<'a, E>(
    input: &'a str,
) -> IResult<&'a str, (u32, Option<u32>, Data<&'a str>, Identifier<&'a str>), E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
    E: ParseError<&'a str> + FromExternalError<&'a str, &'static str>,
{
    map(
        tuple((
            terminated(hex(6), char(' ')),
            terminated(hex(8), char(' ')),
            opt(terminated(hex(8), char(' '))),
            terminated(align, char(' ')),
            identifier,
        )),
        |(size, virt_addr, file_addr, align, id)| {
            (virt_addr, file_addr, Data::Parent { size, align }, id)
        },
    )(input)
}

#[cfg(test)]
mod tests {
    use super::{columns0, columns1, separator, symbol, title, Data, Symbol};
    use crate::{
        map::{Identifier, Origin, SectionName},
        utils::test_utils::assert_diff,
    };
    use nom::{
        branch::alt,
        combinator::{all_consuming, map},
    };
    use nom_supreme::error::ErrorTree;

    #[test]
    fn test_section_table() {
        use crate::map::Line;

        let input = "\
.init section layout\r\n\
\x20 Starting        Virtual\r\n\
\x20 address  Size   address\r\n\
\x20 -----------------------\r\n\
\x20 00000000 0001cc 80003100  1 .init\x20\t__start.o \r\n\
\x20 00000000 0000f0 80003100  4 __start\x20\t__start.o \r\n\
\x20 00000250 000000 80003350 __fill_mem (entry of memset) \t__mem.o \r\n\
\x20 00031b94 00009c 800ec754 000e8954  4 OnRemoval__23AControllerRemovedStateFv\tAControllerRemovedState.o \r\n
\x20 UNUSED   000004 ........ ........    OSVReport os.a OSError.o \r\n
"
        .split_terminator("\r\n")
        .collect::<Vec<_>>();

        let expected: Vec<Line<&str>> = vec![
            Line::SectionTitle(SectionName::Init),
            Line::SectionColumns0,
            Line::SectionColumns1,
            Line::SectionSeparator,
            Line::SectionSymbol(Symbol {
                addr: 0,
                data: Data::Parent {
                    size: 0x1cc,
                    align: 1,
                },
                virt_addr: 0x80003100,
                file_addr: None,
                id: Identifier::Section {
                    name: SectionName::Init,
                    idx: None,
                },
                origin: Origin {
                    obj: "__start.o",
                    src: None,
                    asm: false,
                },
            }),
            Line::SectionSymbol(Symbol {
                addr: 0,
                data: Data::Parent {
                    size: 0xf0,
                    align: 4,
                },
                virt_addr: 0x80003100,
                file_addr: None,
                id: Identifier::Named {
                    name: "__start",
                    instance: None,
                },
                origin: Origin {
                    obj: "__start.o",
                    src: None,
                    asm: false,
                },
            }),
            Line::SectionSymbol(Symbol {
                addr: 0x250,
                data: Data::Child {
                    parent: Identifier::Named {
                        name: "memset",
                        instance: None,
                    },
                },
                virt_addr: 0x80003350,
                file_addr: None,
                id: Identifier::Named {
                    name: "__fill_mem",
                    instance: None,
                },
                origin: Origin {
                    obj: "__mem.o",
                    src: None,
                    asm: false,
                },
            }),
            // 00031b94 00009c 800ec754 000e8954  4 OnRemoval__23AControllerRemovedStateFv	AControllerRemovedState.o
            Line::SectionSymbol(Symbol {
                addr: 0x31b94,
                data: Data::Parent {
                    size: 0x9c,
                    align: 4,
                },
                virt_addr: 0x800ec754,
                file_addr: Some(0xe8954),
                id: Identifier::Named {
                    name: "OnRemoval__23AControllerRemovedStateFv",
                    instance: None,
                },
                origin: Origin {
                    obj: "AControllerRemovedState.o",
                    src: None,
                    asm: false,
                },
            }),
        ];

        let mut parser = alt::<_, _, ErrorTree<&str>, _>((
            all_consuming(map(title, Line::SectionTitle)),
            all_consuming(map(columns0, |_| Line::SectionColumns0)),
            all_consuming(map(columns1, |_| Line::SectionColumns1)),
            all_consuming(map(separator, |_| Line::SectionSeparator)),
            all_consuming(map(symbol, Line::SectionSymbol)),
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
