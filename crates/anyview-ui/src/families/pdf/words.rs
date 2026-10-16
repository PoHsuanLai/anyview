//! The words of a page and a selection of them: which word a point is at, and the lines the words
//! from one to another make. Pure arithmetic over the boxes the page's text gave, in reading order.

use anyview_core::{PagePoint, PageRect};

/// How far `at` is from the span `low` to `high`; nothing within it.
fn gap(at: u32, low: u32, high: u32) -> u64 {
    u64::from(low.saturating_sub(at).max(at.saturating_sub(high)))
}

/// The word `at` is on, else the one nearest to it; a point between two lines goes to the nearer
/// line before it goes to the nearer word along a line. `None` for a page with no words.
pub(super) fn nearest(words: &[PageRect], at: PagePoint) -> Option<usize> {
    words
        .iter()
        .enumerate()
        .min_by_key(|(_, word)| {
            let across = gap(at.x().0, word.left().0, word.right().0);
            let down = gap(at.y().0, word.top().0, word.bottom().0);
            down * down * 16 + across * across
        })
        .map(|(index, _)| index)
}

/// Whether `word` is on the line `line` is the box of so far: they overlap by more than half of
/// the smaller of their heights.
fn on_line(line: PageRect, word: PageRect) -> bool {
    let top = line.top().0.max(word.top().0);
    let bottom = line.bottom().0.min(word.bottom().0);
    bottom.saturating_sub(top) * 2 > line.height().min(word.height())
}

/// `words` as the boxes of the lines they make: runs of words on one line are joined.
pub(super) fn lines(words: &[PageRect]) -> Vec<PageRect> {
    let mut out: Vec<PageRect> = Vec::new();
    for word in words {
        if let Some(line) = out.last_mut()
            && on_line(*line, *word)
        {
            *line = line.joined(*word);
            continue;
        }
        out.push(*word);
    }
    out
}

/// The lines of the words from the one at `anchor` to the one at `head`, either way round.
pub(super) fn between(words: &[PageRect], anchor: usize, head: usize) -> Vec<PageRect> {
    let (first, last) = (anchor.min(head), anchor.max(head));
    words
        .get(first..=last.min(words.len().saturating_sub(1)))
        .map_or_else(Vec::new, lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::Permille;

    fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
        PageRect::new(
            Permille(left),
            Permille(top),
            Permille(right),
            Permille(bottom),
        )
    }

    /// Two lines of words: three on the first, two on the second.
    fn words() -> Vec<PageRect> {
        vec![
            rect(100, 100, 200, 120),
            rect(210, 100, 300, 120),
            rect(310, 101, 400, 121),
            rect(100, 130, 180, 150),
            rect(190, 130, 280, 150),
        ]
    }

    #[test]
    fn a_point_finds_the_word_it_is_on_or_the_nearest_one() {
        // name, the point, the word
        let cases = [
            ("on a word", (250, 110), Some(1)),
            ("between two words of a line", (205, 110), Some(0)),
            ("between the lines, nearer the second", (150, 127), Some(3)),
            ("past the end of the first line", (700, 110), Some(2)),
            ("above everything", (250, 0), Some(1)),
            ("below the last line", (500, 900), Some(4)),
        ];
        for (name, (x, y), want) in cases {
            assert_eq!(nearest(&words(), PagePoint::at(x, y)), want, "{name}");
        }
        assert_eq!(nearest(&[], PagePoint::at(1, 1)), None);
    }

    #[test]
    fn the_words_from_one_to_another_make_one_box_for_each_line() {
        // name, anchor, head, the lines
        let cases = [
            ("one word", 1, 1, vec![rect(210, 100, 300, 120)]),
            ("two words of a line", 0, 1, vec![rect(100, 100, 300, 120)]),
            (
                "backwards is the same",
                1,
                0,
                vec![rect(100, 100, 300, 120)],
            ),
            (
                "across the lines",
                1,
                3,
                vec![rect(210, 100, 400, 121), rect(100, 130, 180, 150)],
            ),
            (
                "everything",
                0,
                4,
                vec![rect(100, 100, 400, 121), rect(100, 130, 280, 150)],
            ),
            (
                "a head past the end is the end",
                3,
                9,
                vec![rect(100, 130, 280, 150)],
            ),
        ];
        for (name, anchor, head, want) in cases {
            assert_eq!(between(&words(), anchor, head), want, "{name}");
        }
    }
}
