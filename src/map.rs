use crate::{
    linker_table, memory_table, section_table, tree, windows::filename,
};
use nom::{
    branch::alt,
    bytes::complete::{
        is_a, tag, take, take_while, take_while1, take_while_m_n,
    },
    character::complete::{alpha1, alphanumeric1, char, digit1},
    combinator::{all_consuming, eof, map, map_res, opt, recognize},
    error::{FromExternalError, ParseError},
    multi::many0_count,
    sequence::{pair, preceded, separated_pair, tuple},
    AsChar, IResult, Parser,
};
use std::num::ParseIntError;

#[derive(Debug, Eq, PartialEq)]
pub enum Line<S: Eq + PartialEq> {
    Empty,
    TreeTitle(S),
    TreeNode(tree::Node<S>),
    SectionTitle(SectionName<S>),
    SectionColumns0,
    SectionColumns1,
    SectionSeparator,
    SectionSymbol(section_table::Symbol<S>),
    MemoryTitle,
    MemoryColumns0,
    MemoryColumns1,
    MemoryEntry(memory_table::Entry<S>),
    LinkerTitle,
    LinkerEntry(linker_table::Entry<S>),
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum Identifier<S: Eq + PartialEq> {
    Relative {
        idx: u32,
    },
    StringBase {
        idx: u8,
    },
    Named {
        name: S,
        instance: Option<u32>,
    },
    Mangled {
        name: S,
    },
    Section {
        name: SectionName<S>,
        idx: Option<u8>,
    },
    DotL {
        name: S,
    },
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum SectionName<S> {
    Bss,
    Ctors,
    Data,
    Dtors,
    ExTab,
    ExTabIndex,
    Init,
    RoData,
    SBss,
    SBss2,
    SData,
    SData2,
    Text,
    Unknown(S),
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum DebugSectionName {
    Main,
    Line,
    Abbrev,
    Aranges,
    Info,
    SfNames,
    SrcInfo,
    Str,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Origin<S: Eq + PartialEq> {
    pub obj: S,
    pub src: Option<S>,
    pub asm: bool,
}

pub fn line<'a, E>(input: &'a str) -> IResult<&'a str, Line<&'a str>, E>
where
    E: ParseError<&'a str>
        + FromExternalError<&'a str, ParseIntError>
        + FromExternalError<&'a str, &'static str>,
{
    use Line::*;

    let input = input.trim_end_matches("\r\n");

    alt((
        map(eof, |_| Empty),
        all_consuming(map(tree::title, TreeTitle)),
        all_consuming(map(tree::node, TreeNode)),
        all_consuming(map(section_table::title, SectionTitle)),
        all_consuming(map(section_table::columns0, |_| SectionColumns0)),
        all_consuming(map(section_table::columns1, |_| SectionColumns1)),
        all_consuming(map(section_table::separator, |_| SectionSeparator)),
        all_consuming(map(section_table::symbol, SectionSymbol)),
        all_consuming(map(memory_table::title, |_| MemoryTitle)),
        all_consuming(map(memory_table::columns0, |_| MemoryColumns0)),
        all_consuming(map(memory_table::columns1, |_| MemoryColumns1)),
        all_consuming(map(memory_table::entry, MemoryEntry)),
        all_consuming(map(memory_table::debug_entry, MemoryEntry)),
        all_consuming(map(linker_table::title, |_| LinkerTitle)),
        all_consuming(map(linker_table::entry, LinkerEntry)),
    ))(input)
}

// TODO: Custom error type
pub(crate) fn padded<'a, E>(
    len: usize,
) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, &'static str>,
{
    move |input: &str| {
        let (input, pad) = take_while(|c| c == ' ')(input)?;
        let len = len.checked_sub(pad.len()).ok_or(nom::Err::Error(
            E::from_external_error(
                input,
                nom::error::ErrorKind::LengthValue,
                "Padding length is too large",
            ),
        ))?;

        take(len).parse(input)
    }
}

pub(crate) fn hex<'a, E>(
    count: usize,
) -> impl FnMut(&'a str) -> IResult<&'a str, u32, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let mut parser = map_res(
        take_while_m_n(count, count, |c: char| c.is_hex_digit()),
        |x| u32::from_str_radix(x, 16),
    );
    move |input: &'a str| parser(input)
}

pub(crate) fn c_name<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ))(input)
}

pub(crate) fn cpp_name<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(pair(
        alt((alpha1, is_a("_@"))),
        many0_count(alt((alphanumeric1, is_a("_@$<>,-")))),
    ))(input)
}

fn relative<'a, E>(input: &'a str) -> IResult<&'a str, u32, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map_res(preceded(char('@'), digit1), str::parse::<u32>)(input)
}

fn section_symbol<'a, E>(
    input: &'a str,
) -> IResult<&'a str, (SectionName<&'a str>, u8), E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map(
        tuple((
            tag(".."),
            section_name,
            char('.'),
            map_res(digit1, str::parse::<u8>),
        )),
        |(_, section_name, _, idx)| (section_name, idx),
    )(input)
}

fn instance<'a, E>(input: &'a str) -> IResult<&'a str, u32, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map_res(preceded(char('$'), digit1), str::parse::<u32>)(input)
}

pub(crate) fn identifier<'a, E>(
    input: &'a str,
) -> IResult<&'a str, Identifier<&'a str>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    use Identifier::*;

    take_while1(|c: char| {
        matches!(c, '0'..='9' | 'a'..='z' | 'A'..='Z' | '<' | '>'
                    | ',' | '_' | '$' | '@' | '.' | '-')
    })
    .and_then(alt((
        all_consuming(map(preceded(tag(".L"), c_name), |name| DotL { name })),
        all_consuming(map(relative, |idx| Relative { idx })),
        all_consuming(map(section_symbol, |(name, idx)| Section {
            name,
            idx: Some(idx),
        })),
        all_consuming(map(section_name, |name| Section { name, idx: None })),
        all_consuming(map(pair(c_name, opt(instance)), |(name, instance)| {
            Named { name, instance }
        })),
        all_consuming(map(cpp_name, |name| Mangled { name })),
        all_consuming(string_base),
    )))
    .parse(input)
}

pub(crate) fn section_name<'a, E>(
    input: &'a str,
) -> IResult<&'a str, SectionName<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    use SectionName::*;

    alt((
        extabindex,
        extab,
        preceded(
            char('.'),
            alt((
                map(tag("bss"), |_| Bss),
                map(tag("ctors"), |_| Ctors),
                map(tag("data"), |_| Data),
                map(tag("dtors"), |_| Dtors),
                map(tag("init"), |_| Init),
                map(tag("rodata"), |_| RoData),
                map(tag("sbss2"), |_| SBss2),
                map(tag("sbss"), |_| SBss),
                map(tag("sdata2"), |_| SData2),
                map(tag("sdata"), |_| SData),
                map(tag("text"), |_| Text),
            )),
        ),
        map(
            preceded(
                char('.'),
                take_while1(|c| {
                    matches!(c, '0'..='9' | 'a'..='z' | 'A'..='Z'
                                | '_' | '.')
                }),
            ),
            Unknown,
        ),
    ))(input)
}

pub(crate) fn origin<'a, E>(
    input: &'a str,
) -> IResult<&'a str, Origin<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(
        separated_pair(
            filename,
            char(' '),
            alt((
                map(
                    pair(filename, map(opt(tag(" (asm)")), |o| o.is_some())),
                    |(src, asm)| (Some(src), asm),
                ),
                map(tag(""), |_| (None, false)),
            )),
        ),
        |(obj, (src, asm))| Origin { obj, src, asm },
    )(input)
}

fn extab<'a, E>(input: &'a str) -> IResult<&'a str, SectionName<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(
        tuple((opt(char('.')), opt(char('_')), tag("extab"), opt(char('_')))),
        |_| SectionName::ExTab,
    )(input)
}

fn extabindex<'a, E>(
    input: &'a str,
) -> IResult<&'a str, SectionName<&'a str>, E>
where
    E: ParseError<&'a str>,
{
    map(
        tuple((
            opt(char('.')),
            opt(char('_')),
            alt((tag("extabindex"), tag("exidx"))),
            opt(char('_')),
        )),
        |_| SectionName::ExTabIndex,
    )(input)
}

fn string_base<'a, E>(
    input: &'a str,
) -> IResult<&'a str, Identifier<&'a str>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    preceded(
        tag("@stringBase"),
        map(map_res(digit1, |s| u8::from_str_radix(s, 10)), |idx| {
            Identifier::StringBase { idx }
        }),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::Line;
    use anyhow::{anyhow, Context, Result};
    use memmap2::Mmap;
    use nom_supreme::error::ErrorTree;
    use rayon::{prelude::ParallelIterator, str::ParallelString};
    use std::{fs::File, path::PathBuf, str};
    use test_case::test_case;

    fn parse_lines<'a>(input: &'a str) -> Result<Vec<Line<&'a str>>> {
        let vec = input
            .par_lines()
            .map(|line| {
                super::line::<ErrorTree<&'a str>>(line).map(|(_, line)| line)
            })
            .collect::<Result<Vec<Line<&'a str>>, _>>()
            .map_err(|err| anyhow!(format!("{err:#?}")))?;

        Ok(vec)
    }

    #[test_case("GALE01.2.map" ; "melee")]
    #[test_case("GM8E01.0.map" ; "prime 1.0")]
    #[test_case("GM8E01.0D.map" ; "prime 1.0 debug")]
    #[test_case("GM8E01.1.map" ; "prime 1.1")]
    #[test_case("GM8E01.1D.map" ; "prime 1.1 debug")]
    #[test_case("GM8K01.map" ; "prime kor")]
    #[test_case("GM8K01D.map" ; "prime kor debug")]
    #[test_case("GMBE8P.map" ; "super monkey ball")]
    #[test_case("GPIE01.map" ; "pikmin")]
    #[test_case("GPVE01.map" ; "pikmin2")]
    fn test_file(path: &str) -> Result<()> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(path);
        let file = File::open(path).context("Failed to open the map file.")?;
        let mmap = unsafe { Mmap::map(&file) }
            .context("Failed to create the memory map.")?;
        let input = str::from_utf8(mmap.as_ref())
            .context("Failed to convert to UTF-8.")?;
        let _lines = parse_lines(input).context("Failed to parse lines.")?;

        Ok(())
    }
}
