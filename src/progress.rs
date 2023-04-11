#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};
    use memmap2::Mmap;
    use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
    use std::{fs::File, path::PathBuf, str};
    use test_case::test_case;

    use crate::{
        map::{self, Identifier, Line, SectionName},
        memory_table, section_table,
        utils::test_utils::parse_lines,
    };

    // #[test_case("GALE01.2.map" ; "melee")]
    #[test_case("GM8E01.0.map" ; "prime 1.0")]
    // #[test_case("GM8E01.0D.map" ; "prime 1.0 debug")]
    // #[test_case("GM8E01.1.map" ; "prime 1.1")]
    // #[test_case("GM8E01.1D.map" ; "prime 1.1 debug")]
    // #[test_case("GM8K01.map" ; "prime kor")]
    // #[test_case("GM8K01D.map" ; "prime kor debug")]
    // #[test_case("GMBE8P.map" ; "super monkey ball")]
    // #[test_case("GPIE01.map" ; "pikmin")]
    // #[test_case("GPVE01.map" ; "pikmin2")]
    fn progress(path: &str) -> Result<()> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(path);
        let file = File::open(path).context("Failed to open the map file.")?;
        let mmap = unsafe { Mmap::map(&file) }
            .context("Failed to create the memory map.")?;
        let input = str::from_utf8(mmap.as_ref())
            .context("Failed to convert to UTF-8.")?;
        let lines = parse_lines(input)?;

        let (code_total, data_total) = lines
            .par_iter()
            .filter_map(|line| match line {
                Line::MemoryEntry(memory_table::Entry {
                    data, size, ..
                }) => {
                    use memory_table::Data::*;
                    use SectionName::*;
                    match data {
                        Main {
                            name: Text | Init, ..
                        } => Some((*size, 0)),

                        Main {
                            name: Unknown(_), ..
                        }
                        | Debug { .. } => None,

                        _ => Some((0, *size)),
                    }
                }
                _ => None,
            })
            .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));

        let (code_size, data_size) = {
            use section_table::{Data::Parent, Symbol};
            use SectionName::*;

            let mut section: Option<SectionName<&str>> = None;
            let mut code_size: u32 = 0;
            let mut data_size: u32 = 0;

            for line in lines {
                match line {
                    Line::Empty => section = None,
                    Line::SectionTitle(name) => section = Some(name),
                    Line::SectionSymbol(Symbol {
                        id: Identifier::Section { .. },
                        ..
                    }) => (),
                    Line::SectionSymbol(Symbol {
                        data: Parent { size, .. },
                        ..
                    }) => match section {
                        Some(Text | Init) => code_size += size,
                        None | Some(Unknown(_)) => (),
                        _ => data_size += size,
                    },
                    _ => (),
                }
            }

            (code_size, data_size)
        };

        let expected = r#"{"dol": {"code": 367324, "code/total": 3964960, "data": 76507, "data/total": 1983400}}"#;
        let actual = format!(
            r#"{{"dol": {{"code": {code_size}, "code/total": {code_total}, "data": {data_size}, "data/total": {data_total}"#
        );

        println!("{}", prettydiff::diff_words(expected, &actual).format());
        panic!();
    }
}
