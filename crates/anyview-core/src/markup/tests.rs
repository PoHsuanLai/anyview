use super::*;
use crate::units::{PageIndex, Permille};

fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
    PageRect::new(
        Permille(left),
        Permille(top),
        Permille(right),
        Permille(bottom),
    )
}

fn mark(shape: MarkShape) -> Mark {
    Mark {
        page: PageIndex(0),
        color: MarkColor::Yellow,
        shape,
        existing: None,
    }
}

#[test]
fn a_rectangle_moves_as_far_as_the_page_lets_it_with_its_size_kept() {
    // name, the move, where the rectangle ends
    let start = rect(100, 100, 300, 150);
    let cases = [
        ("free", (50, -20), rect(150, 80, 350, 130)),
        ("held at the left edge", (-500, 0), rect(0, 100, 200, 150)),
        (
            "held at the right edge",
            (900, 0),
            rect(800, 100, 1000, 150),
        ),
        ("held at the top", (0, -500), rect(100, 0, 300, 50)),
        ("held at the foot", (0, 2000), rect(100, 950, 300, 1000)),
    ];
    for (name, (dx, dy), want) in cases {
        assert_eq!(start.moved(dx, dy), want, "{name}");
    }
}

#[test]
fn two_corners_make_the_same_rectangle_whichever_way_round() {
    let (a, b) = (PagePoint::at(300, 40), PagePoint::at(100, 90));
    assert_eq!(PageRect::spanning(a, b), rect(100, 40, 300, 90));
    assert_eq!(PageRect::spanning(b, a), rect(100, 40, 300, 90));
}

#[test]
fn a_pointer_is_on_a_mark_when_it_is_in_reach_of_its_ink() {
    let line = mark(MarkShape::Line {
        from: PagePoint::at(100, 100),
        to: PagePoint::at(500, 100),
    });
    let words = mark(MarkShape::Highlight(vec![
        rect(100, 100, 400, 120),
        rect(100, 130, 300, 150),
    ]));
    // name, the mark, the point, whether it is on it
    let cases = [
        ("on the line", &line, PagePoint::at(300, 100), true),
        ("near the line", &line, PagePoint::at(300, 105), true),
        ("far from the line", &line, PagePoint::at(300, 160), false),
        ("past the line's end", &line, PagePoint::at(560, 100), false),
        (
            "on a highlighted line",
            &words,
            PagePoint::at(200, 110),
            true,
        ),
        (
            "between two highlighted lines",
            &words,
            PagePoint::at(350, 126),
            true,
        ),
        ("off a highlight", &words, PagePoint::at(700, 300), false),
    ];
    for (name, mark, at, on) in cases {
        assert_eq!(mark.is_at(at, 8), on, "{name}");
    }
}

#[test]
fn moving_a_mark_moves_all_of_it_by_the_same_amount() {
    let words = mark(MarkShape::Underline(vec![
        rect(100, 100, 400, 120),
        rect(100, 130, 300, 150),
    ]));
    let moved = words.moved(10, 20);
    assert_eq!(
        moved.shape,
        MarkShape::Underline(vec![rect(110, 120, 410, 140), rect(110, 150, 310, 170)])
    );
    let line = mark(MarkShape::Arrow {
        from: PagePoint::at(950, 500),
        to: PagePoint::at(990, 520),
    });
    // Held at the edge as a whole: the line keeps its length.
    assert_eq!(
        line.moved(100, 0).shape,
        MarkShape::Arrow {
            from: PagePoint::at(960, 500),
            to: PagePoint::at(1000, 520),
        }
    );
}

#[test]
fn a_colour_in_the_file_is_the_nearest_of_the_palette() {
    assert_eq!(MarkColor::nearest(255, 255, 0), MarkColor::Yellow);
    assert_eq!(MarkColor::nearest(250, 20, 20), MarkColor::Red);
    assert_eq!(MarkColor::nearest(0, 0, 0), MarkColor::Black);
    assert_eq!(MarkColor::nearest(10, 132, 255), MarkColor::Blue);
}

#[test]
fn a_note_covers_its_icon() {
    let note = mark(MarkShape::Note {
        at: PagePoint::at(100, 200),
        text: "x".into(),
    });
    assert_eq!(
        note.bounds(),
        rect(100, 200, 100 + NOTE_SIZE.0, 200 + NOTE_SIZE.1)
    );
}
