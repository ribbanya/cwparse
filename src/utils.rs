#[cfg(test)]
pub(crate) mod test_utils {
    use crate::map;
    use anyhow::{anyhow, Context, Result};
    use nom_supreme::error::ErrorTree;
    use prettydiff::diff_lines;
    use rayon::{prelude::ParallelIterator, str::ParallelString};
    use std::fmt::Debug;

    pub(crate) fn assert_diff<T>(expected: &T, actual: &T)
    where
        T: Eq + Debug,
    {
        if expected != actual {
            eprintln!("assertion failed: `(expected == actual)`");
            diff_lines(&format!("{:#?}", expected), &format!("{:#?}", actual))
                .set_show_lines(false)
                .prettytable();
            panic!();
        }
    }

    pub(crate) fn parse_lines<'a>(
        input: &'a str,
    ) -> Result<Vec<map::Line<&'a str>>> {
        input
            .par_lines()
            .map(|input| {
                map::line::<ErrorTree<&'a str>>(input)
                    .map(|(_, output)| output)
            })
            .collect::<Result<Vec<map::Line<&'a str>>, _>>()
            .map_err(|err| anyhow!(format!("{err:#?}")))
            .context("Failed to parse lines.")
    }
}
