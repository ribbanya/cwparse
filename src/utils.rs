#[cfg(test)]
pub(crate) mod test_utils {
    use std::fmt::Debug;

    use prettydiff::diff_lines;

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
}
