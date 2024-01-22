use crate::map::{c_name, identifier, origin, Identifier, Origin};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{char, digit1},
    combinator::{map, map_res},
    error::{FromExternalError, ParseError},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};
use std::num::ParseIntError;

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum Type {
    None,
    Section,
    Object,
    Function,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum Scope {
    Global,
    Local,
    Weak,
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum Data<'a> {
    Linker(&'a str),
    Object(Identifier<'a>, Specifier<'a>),
    DuplicateIdentifier(Identifier<'a>),
    DuplicateSpecifier(Specifier<'a>),
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Node<'a> {
    pub depth: u32,
    pub data: Data<'a>,
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Specifier<'a> {
    pub r#type: Type,
    pub scope: Scope,
    pub origin: Origin<'a>,
}

pub fn title<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    preceded(tag("Link map of "), c_name)(input)
}

pub fn node<'a, E>(input: &'a str) -> IResult<&'a str, Node<'a>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map(
        pair(depth, alt((linker_data, object_data, duplicate))),
        |(depth, data)| Node { depth, data },
    )(input)
}

fn r#type<'a, E>(input: &'a str) -> IResult<&'a str, Type, E>
where
    E: ParseError<&'a str>,
{
    use Type::*;

    alt((
        map(tag("section"), |_| Section),
        map(tag("object"), |_| Object),
        map(tag("func"), |_| Function),
        map(tag("notype"), |_| None),
    ))(input)
}

fn scope<'a, E>(input: &'a str) -> IResult<&'a str, Scope, E>
where
    E: ParseError<&'a str>,
{
    use Scope::*;

    alt((
        map(tag("global"), |_| Global),
        map(tag("local"), |_| Local),
        map(tag("weak"), |_| Weak),
    ))(input)
}

fn linker_data<'a, E>(input: &'a str) -> IResult<&'a str, Data<'a>, E>
where
    E: ParseError<&'a str>,
{
    map(
        terminated(c_name, tag(" found as linker generated symbol")),
        Data::Linker,
    )(input)
}

fn object_data<'a, E>(input: &'a str) -> IResult<&'a str, Data<'a>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map(
        tuple((terminated(identifier, char(' ')), specifier)),
        |(id, loc)| Data::Object(id, loc),
    )(input)
}

fn duplicate<'a, E>(input: &'a str) -> IResult<&'a str, Data<'a>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    preceded(
        tag(">>> "),
        alt((
            map(
                preceded(tag("UNREFERENCED DUPLICATE "), identifier),
                Data::DuplicateIdentifier,
            ),
            map(specifier, Data::DuplicateSpecifier),
        )),
    )(input)
}

fn specifier<'a, E>(input: &'a str) -> IResult<&'a str, Specifier<'a>, E>
where
    E: ParseError<&'a str>,
{
    map(
        pair(
            terminated(
                delimited(
                    char('('),
                    separated_pair(r#type, char(','), scope),
                    char(')'),
                ),
                char(' '),
            ),
            preceded(tag("found in "), origin),
        ),
        |((r#type, scope), origin)| Specifier {
            r#type,
            scope,
            origin,
        },
    )(input)
}

fn depth<'a, E>(input: &'a str) -> IResult<&'a str, u32, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map_res(
        delimited(take_while(|c| c == ' '), digit1, tag("] ")),
        str::parse::<u32>,
    )(input)
}

#[cfg(test)]
mod tests {
    use super::{node, title, Node, Specifier};
    use crate::{
        map::{Origin, SectionName},
        utils::test_utils::assert_diff,
    };
    use nom::{
        branch::alt,
        combinator::{eof, map},
        sequence::terminated,
    };
    use nom_supreme::error::ErrorTree;

    #[test]
    fn test_tree() {
        use crate::{
            map::Line,
            tree::{Data, Identifier, Scope, Type},
        };

        let input = "\
Link map of __start\r\n\
\x20 1] __start (func,weak) found in os.a __start.c\r\n\
\x20 1] __start (func,global) found in __start.c.o \r\n\
\x20  2] __init_registers (func,local) found in __start.c.o \r\n\
\x20   3] _stack_addr found as linker generated symbol\r\n\
\x20    4] ...data.0 (notype,local) found in OSCache.c.o \r\n\
\x20 19] finfo$221 (notype,global) found in bss.o \r\n\
\x20        8] extab_0 (notype,global) found in c++_exception_data.s.o \r\n\
\x20      6] vprintf (notype,global) found in MSL_C.PPCEABI.bare.H.a printf.o (asm)\r\n\
\x20                16] >>> UNREFERENCED DUPLICATE __dt__15CMemoryInStreamFv\r\n\
\x20                16] >>> (func,weak) found in Kyoto_CW1.a CMemoryInStream.cpp\r\n\
"
        .split_terminator("\r\n")
        .collect::<Vec<_>>();

        let expected = vec![
            Line::TreeTitle("__start"),
            Line::TreeNode(Node {
                depth: 1,
                data: Data::Object(
                    Identifier::Named {
                        name: "__start",
                        instance: None,
                    },
                    Specifier {
                        r#type: Type::Function,
                        scope: Scope::Weak,
                        origin: Origin {
                            obj: "os.a",
                            src: Some("__start.c"),
                            asm: false,
                        },
                    },
                ),
            }),
            Line::TreeNode(Node {
                depth: 1,
                data: Data::Object(
                    Identifier::Named {
                        name: "__start",
                        instance: None,
                    },
                    Specifier {
                        r#type: Type::Function,
                        scope: Scope::Global,
                        origin: Origin {
                            obj: "__start.c.o",
                            src: None,
                            asm: false,
                        },
                    },
                ),
            }),
            Line::TreeNode(Node {
                depth: 2,
                data: Data::Object(
                    Identifier::Named {
                        name: "__init_registers",
                        instance: None,
                    },
                    Specifier {
                        r#type: Type::Function,
                        scope: Scope::Local,
                        origin: Origin {
                            obj: "__start.c.o",
                            src: None,
                            asm: false,
                        },
                    },
                ),
            }),
            Line::TreeNode(Node {
                depth: 3,
                data: Data::Linker("_stack_addr"),
            }),
            Line::TreeNode(Node {
                depth: 4,
                data: Data::Object(
                    Identifier::Section {
                        name: SectionName::Data,
                        idx: Some(0),
                    },
                    Specifier {
                        r#type: Type::None,
                        scope: Scope::Local,
                        origin: Origin {
                            obj: "OSCache.c.o",
                            src: None,
                            asm: false,
                        },
                    },
                ),
            }),
            Line::TreeNode(Node {
                depth: 19,
                data: Data::Object(
                    Identifier::Named {
                        name: "finfo",
                        instance: Some(221),
                    },
                    Specifier {
                        r#type: Type::None,
                        scope: Scope::Global,
                        origin: Origin {
                            obj: "bss.o",
                            src: None,
                            asm: false,
                        },
                    },
                ),
            }),
            Line::TreeNode(Node {
                depth: 8,
                data: Data::Object(
                    Identifier::Named {
                        name: "extab_0",
                        instance: None,
                    },
                    Specifier {
                        r#type: Type::None,
                        scope: Scope::Global,
                        origin: Origin {
                            obj: "c++_exception_data.s.o",
                            src: None,
                            asm: false,
                        },
                    },
                ),
            }),
            Line::TreeNode(Node {
                depth: 6,
                data: Data::Object(
                    Identifier::Named {
                        name: "vprintf",
                        instance: None,
                    },
                    Specifier {
                        r#type: Type::None,
                        scope: Scope::Global,
                        origin: Origin {
                            obj: "MSL_C.PPCEABI.bare.H.a",
                            src: Some("printf.o"),
                            asm: true,
                        },
                    },
                ),
            }),
            Line::TreeNode(Node {
                depth: 16,
                data: Data::DuplicateIdentifier(Identifier::Named {
                    name: "__dt__15CMemoryInStreamFv",
                    instance: None,
                }),
            }),
            Line::TreeNode(Node {
                depth: 16,
                data: Data::DuplicateSpecifier(Specifier {
                    r#type: Type::Function,
                    scope: Scope::Weak,
                    origin: Origin {
                        obj: "Kyoto_CW1.a",
                        src: Some("CMemoryInStream.cpp"),
                        asm: false,
                    },
                }),
            }),
        ];

        let mut parser = terminated::<_, _, _, ErrorTree<&str>, _, _>(
            alt((map(title, Line::TreeTitle), map(node, Line::TreeNode))),
            eof,
        );

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
