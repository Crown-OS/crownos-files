use std::cmp::Ordering;

/// Natural order over raw name bytes: digit runs compare by value
/// (`img9 < img010 < img11`) and ASCII letters compare case-folded.
///
/// Runs that differ only in leading zeros, or names that differ only in case,
/// compare `Equal`; callers that need a total order break the tie on the bytes.
pub fn natural_cmp(mut left: &[u8], mut right: &[u8]) -> Ordering {
    loop {
        let (Some((&l, l_tail)), Some((&r, r_tail))) = (left.split_first(), right.split_first())
        else {
            return left.len().cmp(&right.len());
        };
        if l.is_ascii_digit() && r.is_ascii_digit() {
            let (l_number, l_rest) = split_number(left);
            let (r_number, r_rest) = split_number(right);
            let ordering = l_number
                .len()
                .cmp(&r_number.len())
                .then_with(|| l_number.cmp(r_number));
            if ordering.is_ne() {
                return ordering;
            }
            (left, right) = (l_rest, r_rest);
        } else {
            let ordering = l.to_ascii_lowercase().cmp(&r.to_ascii_lowercase());
            if ordering.is_ne() {
                return ordering;
            }
            (left, right) = (l_tail, r_tail);
        }
    }
}

fn split_number(bytes: &[u8]) -> (&[u8], &[u8]) {
    let end = bytes
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(bytes.len());
    let (digits, rest) = bytes.split_at(end);
    let zeros = digits.iter().take_while(|&&byte| byte == b'0').count();
    (&digits[zeros..], rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_sorted(expected: &[&str]) {
        let mut names = expected.to_vec();
        names.reverse();
        names.sort_by(|a, b| natural_cmp(a.as_bytes(), b.as_bytes()));
        assert_eq!(names, expected);
    }

    #[test]
    fn digit_runs_compare_by_value() {
        assert_sorted(&["file1", "file2", "file10", "file20", "file100"]);
        assert_sorted(&[
            "1999-3-3",
            "1999-12-25",
            "2000-1-2",
            "2000-1-10",
            "2000-3-23",
        ]);
    }

    #[test]
    fn leading_zeros_do_not_change_the_value() {
        assert_sorted(&["img9", "img010", "img11", "img0100"]);
        assert_eq!(natural_cmp(b"a01", b"a1"), Ordering::Equal);
        assert_eq!(natural_cmp(b"x2-y08", b"x2-y7"), Ordering::Greater);
    }

    #[test]
    fn ascii_letters_fold_case() {
        assert_sorted(&["alpha", "Beta", "gamma", "Zeta"]);
        assert_eq!(natural_cmp(b"README", b"readme"), Ordering::Equal);
        assert_sorted(&["Pic2", "pic05", "PIC7", "pic100"]);
    }

    #[test]
    fn a_prefix_sorts_first() {
        assert_sorted(&["", "a", "a1", "ab", "b"]);
    }

    #[test]
    fn numbers_beyond_u64_still_compare() {
        assert_sorted(&["v99999999999999999999", "v100000000000000000000"]);
    }

    #[test]
    fn non_ascii_bytes_compare_by_code_point() {
        assert_sorted(&["zeta", "ångström", "éclair"]);
    }
}
