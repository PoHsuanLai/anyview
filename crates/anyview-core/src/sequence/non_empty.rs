//! `NonEmpty`: a list that always has a first entry.

use std::num::NonZeroUsize;

/// A list with at least one entry, so "the first" and "the last" always exist and no caller
/// decides what an empty list meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmpty<T> {
    head: T,
    tail: Vec<T>,
}

impl<T> NonEmpty<T> {
    /// A list that starts with `head`, followed by `tail`.
    pub fn new(head: T, tail: Vec<T>) -> Self {
        NonEmpty { head, tail }
    }

    /// The entries of `items`, or `None` when it is empty.
    pub fn from_vec(items: Vec<T>) -> Option<Self> {
        let mut items = items.into_iter();
        let head = items.next()?;
        Some(NonEmpty {
            head,
            tail: items.collect(),
        })
    }

    /// How many entries there are: at least one.
    pub fn count(&self) -> NonZeroUsize {
        NonZeroUsize::MIN.saturating_add(self.tail.len())
    }

    /// The entry at `index`, counting from zero.
    pub fn get(&self, index: usize) -> Option<&T> {
        match index.checked_sub(1) {
            None => Some(&self.head),
            Some(in_tail) => self.tail.get(in_tail),
        }
    }

    /// The first entry.
    pub fn first(&self) -> &T {
        &self.head
    }

    /// The last entry.
    pub fn last(&self) -> &T {
        self.tail.last().unwrap_or(&self.head)
    }

    /// The entries in order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.head).chain(self.tail.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_vec_makes_no_list() {
        assert_eq!(NonEmpty::<u8>::from_vec(vec![]), None);
        assert_eq!(NonEmpty::from_vec(vec![7]), Some(NonEmpty::new(7, vec![])));
    }

    #[test]
    fn entries_are_reached_by_index_in_order() {
        const CASES: &[(&str, &[u8], usize, Option<u8>)] = &[
            ("only entry", &[5], 0, Some(5)),
            ("first of many", &[5, 6, 7], 0, Some(5)),
            ("middle", &[5, 6, 7], 1, Some(6)),
            ("last", &[5, 6, 7], 2, Some(7)),
            ("past the end", &[5, 6, 7], 3, None),
        ];
        for (name, items, index, want) in CASES {
            let list = NonEmpty::from_vec(items.to_vec()).unwrap();
            assert_eq!(list.get(*index).copied(), *want, "{name}");
        }
    }

    #[test]
    fn length_first_last_and_iteration_agree() {
        const CASES: &[(&str, &[u8])] = &[("one", &[1]), ("two", &[1, 2]), ("four", &[1, 2, 3, 4])];
        for (name, items) in CASES {
            let list = NonEmpty::from_vec(items.to_vec()).unwrap();
            assert_eq!(list.count().get(), items.len(), "{name}");
            assert_eq!(list.first(), &items[0], "{name}");
            assert_eq!(list.last(), &items[items.len() - 1], "{name}");
            assert_eq!(
                list.iter().copied().collect::<Vec<_>>(),
                items.to_vec(),
                "{name}"
            );
        }
    }
}
