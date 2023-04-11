use nom::{
    bytes::complete::take_while1, character::complete::char,
    combinator::recognize, error::ParseError, sequence::separated_pair,
    IResult,
};

pub(crate) fn is_filename(c: char) -> bool {
    match c {
        '\x00'..='\x1F' // Control characters
        | '<'
        | '>'
        | ':'
        | '"'
        | '/'
        | '\\'
        | '|'
        | '?'
        | '*' => false,
        _ => true,
    }
}

pub(crate) fn filename<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(separated_pair(
        take_while1(|c| is_filename(c) && c != '.'),
        char('.'),
        take_while1(|c| is_filename(c) && !c.is_ascii_whitespace()),
    ))(input)
}
