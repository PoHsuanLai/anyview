//! Natural order: `2.png` before `10.png`.

use std::cmp::Ordering;

/// One run of a name: digits compare as numbers, the rest as lower-cased text.
enum Run<'a> {
    Number(&'a str),
    Text(&'a str),
}

fn runs(name: &str) -> impl Iterator<Item = Run<'_>> {
    let mut rest = name;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let digits = first.is_ascii_digit();
        let end = rest
            .find(|c: char| c.is_ascii_digit() != digits)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        rest = tail;
        Some(if digits {
            Run::Number(run)
        } else {
            Run::Text(run)
        })
    })
}

fn number_order(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// How two entry names sort for reading: runs of digits by value, other text without regard to
/// case, and the names themselves last so the order is total.
pub fn natural_order(a: &str, b: &str) -> Ordering {
    let mut left = runs(a);
    let mut right = runs(b);
    loop {
        let order = match (left.next(), right.next()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(Run::Number(x)), Some(Run::Number(y))) => number_order(x, y),
            (Some(Run::Text(x)), Some(Run::Text(y))) => x.to_lowercase().cmp(&y.to_lowercase()),
            (Some(Run::Number(_)), Some(Run::Text(_))) => Ordering::Less,
            (Some(Run::Text(_)), Some(Run::Number(_))) => Ordering::Greater,
        };
        if order != Ordering::Equal {
            return order;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sort_the_way_a_person_counts() {
        // name, first, second, the first sorts
        const CASES: &[(&str, &str, &str, Ordering)] = &[
            ("two before ten", "2.png", "10.png", Ordering::Less),
            (
                "leading zeros are no more",
                "002.png",
                "10.png",
                Ordering::Less,
            ),
            ("case is ignored", "a.png", "B.png", Ordering::Less),
            (
                "folders by their own numbers",
                "ch2/1.png",
                "ch10/1.png",
                Ordering::Less,
            ),
            ("a prefix first", "p", "p1", Ordering::Less),
            (
                "equal numbers fall to the text",
                "01.png",
                "1.png",
                Ordering::Less,
            ),
            ("the same name", "a.png", "a.png", Ordering::Equal),
        ];
        for (name, a, b, want) in CASES {
            assert_eq!(natural_order(a, b), *want, "{name}");
        }
    }
}
