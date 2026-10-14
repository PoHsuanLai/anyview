//! The specimen: a few lines of sample text set in the font, as vector outlines.
//!
//! Outlines rather than a font handed to the renderer: the pane needs no font loading, the peek is
//! a small value whatever the font's size, and what is drawn is exactly the face asked about.
//! Lines are set with each glyph's advance width and no kerning or shaping, which is enough for a
//! glance at a face's character.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// Units to the em in a specimen's coordinates, whatever the font's own.
pub const EM: u32 = 1000;

/// What each alphabet line says; a line of a script the font lacks is left out.
const SAMPLES: &[&str] = &["ABCDEFGHIJKLM", "abcdefghijklm", "0123456789 &?!@"];

/// The pangram a type specimen sets, as Font Book does.
const PANGRAM: &str = "The quick brown fox jumps over the lazy dog";

/// What the size ramp says at each size.
const RAMP: &str = "Hamburgefonstiv";

/// The characters a replacement line shows.
const FALLBACK_CHARS: usize = 14;

/// The characters a replacement pangram shows, counted from the start of the font's own.
const FALLBACK_RUN: usize = 40;

/// The characters a replacement ramp shows.
const FALLBACK_RAMP: usize = 8;

/// One line of sample text as an outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecimenLine {
    /// What it says.
    pub text: String,
    /// SVG path data, `y` down, in [`EM`] units to the em, drawn with the baseline `ascent` below
    /// the top.
    pub path: String,
    /// The line's width in those units.
    pub width: u32,
    /// The line's height in those units: ascent plus descent.
    pub height: u32,
}

/// Sample lines set in a font.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Specimen {
    /// The alphabet and digit lines, top first. Empty when the font draws none of the samples or
    /// its own characters.
    pub lines: Vec<SpecimenLine>,
    /// The font's family and style name set in itself, when the font can spell it.
    pub title: Option<SpecimenLine>,
    /// A sentence using every letter, or a run of the font's own characters when it lacks Latin.
    pub pangram: Option<SpecimenLine>,
    /// A short line a viewer sets at a ramp of sizes.
    pub ramp: Option<SpecimenLine>,
}

/// What to say in each part of a specimen, chosen from what a font covers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Plan {
    pub(crate) title: Option<String>,
    pub(crate) lines: Vec<String>,
    pub(crate) pangram: Option<String>,
    pub(crate) ramp: Option<String>,
}

/// `text` with the characters `covers` refuses taken out, or `None` when less than half of its
/// letters are left.
fn shown(text: &str, covers: &dyn Fn(char) -> bool) -> Option<String> {
    let kept: String = text.chars().filter(|c| covers(*c)).collect();
    let count = |s: &str| s.chars().filter(|c| !c.is_whitespace()).count();
    (count(&kept) > 0 && count(&kept) * 2 >= count(text)).then_some(kept)
}

/// What a font that covers `covers` is shown saying. A font with Latin gets the alphabet, the
/// pangram and the ramp text; one without gets a run of what `own` lists (its own characters, in
/// the order it maps them), never a box. `title` is the family and style name.
pub(crate) fn plan(
    title: &str,
    covers: &dyn Fn(char) -> bool,
    own: &dyn Fn() -> Vec<char>,
) -> Plan {
    let title = shown(title, covers);
    let lines: Vec<String> = SAMPLES.iter().filter_map(|s| shown(s, covers)).collect();
    if !lines.is_empty() {
        return Plan {
            title,
            lines,
            pangram: shown(PANGRAM, covers),
            ramp: shown(RAMP, covers),
        };
    }
    let own = own();
    let run = |from: usize, to: usize| -> Option<String> {
        let text: String = own.iter().skip(from).take(to - from).collect();
        (!text.is_empty()).then_some(text)
    };
    Plan {
        title,
        lines: run(0, FALLBACK_CHARS).into_iter().collect(),
        pangram: run(FALLBACK_CHARS, FALLBACK_RUN),
        ramp: run(0, FALLBACK_RAMP),
    }
}

/// The specimen of `font`, whose family and style are called `title`.
pub(crate) fn specimen(font: &FontRef<'_>, title: &str) -> Specimen {
    let charmap = font.charmap();
    let covers = |c: char| charmap.map(c).is_some();
    let own = || {
        let mut chars: Vec<char> = charmap
            .mappings()
            .filter_map(|(code, glyph)| char::from_u32(code).map(|c| (c, glyph)))
            .filter(|(c, glyph)| !c.is_control() && !c.is_whitespace() && glyph.to_u32() != 0)
            .map(|(c, _)| c)
            .take(OWN_SCAN)
            .collect();
        // Letters, digits and ideographs first: a symbol font keeps what it has, in its order.
        if chars.iter().filter(|c| c.is_alphanumeric()).count() >= FALLBACK_CHARS {
            chars.retain(|c| c.is_alphanumeric());
        }
        chars
    };
    let chosen = plan(title, &covers, &own);
    let line = |text: &str| -> Option<SpecimenLine> {
        let glyphs: Vec<(char, GlyphId)> = text
            .chars()
            .filter_map(|c| charmap.map(c).map(|glyph| (c, glyph)))
            .collect();
        (!glyphs.is_empty()).then(|| set(font, &glyphs))
    };
    Specimen {
        lines: chosen.lines.iter().filter_map(|t| line(t)).collect(),
        title: chosen.title.as_deref().and_then(|t| line(t)),
        pangram: chosen.pangram.as_deref().and_then(|t| line(t)),
        ramp: chosen.ramp.as_deref().and_then(|t| line(t)),
    }
}

/// How many mapped characters are looked at when a font has none of the samples.
const OWN_SCAN: usize = 2000;

/// `glyphs` set in a row.
fn set(font: &FontRef<'_>, glyphs: &[(char, GlyphId)]) -> SpecimenLine {
    let metrics = font.metrics(Size::unscaled(), LocationRef::default());
    let per_em = f32::from(metrics.units_per_em.max(1));
    let scale = EM as f32 / per_em;
    let advances = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let outlines = font.outline_glyphs();
    let ascent = (metrics.ascent * scale).max(1.0);
    let descent = (-metrics.descent * scale).max(0.0);
    let mut pen = PathPen {
        path: String::new(),
        scale,
        origin: 0.0,
        baseline: ascent,
    };
    let mut text = String::new();
    for (c, glyph) in glyphs {
        text.push(*c);
        if let Some(outline) = outlines.get(*glyph) {
            // A glyph that cannot be drawn leaves a gap; the advance still moves on.
            let _ = outline.draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut pen,
            );
        }
        pen.origin += advances.advance_width(*glyph).unwrap_or(per_em / 2.0) * scale;
    }
    SpecimenLine {
        text,
        path: pen.path,
        width: whole(pen.origin),
        height: whole(ascent + descent),
    }
}

fn whole(value: f32) -> u32 {
    // Rounded and clamped to what a u32 holds, so the cast cannot wrap.
    value.round().clamp(0.0, u32::MAX as f32) as u32
}

/// An [`OutlinePen`] that writes integer SVG path data, moving each glyph to the pen's origin and
/// turning the font's `y`-up into the page's `y`-down.
struct PathPen {
    path: String,
    scale: f32,
    origin: f32,
    baseline: f32,
}

impl PathPen {
    fn point(&self, x: f32, y: f32) -> String {
        let px = (self.origin + x * self.scale).round() as i32;
        let py = (self.baseline - y * self.scale).round() as i32;
        format!("{px} {py}")
    }
}

impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        let at = self.point(x, y);
        self.path.push_str(&format!("M{at}"));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let at = self.point(x, y);
        self.path.push_str(&format!("L{at}"));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (control, at) = (self.point(cx0, cy0), self.point(x, y));
        self.path.push_str(&format!("Q{control} {at}"));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (first, second, at) = (self.point(cx0, cy0), self.point(cx1, cy1), self.point(x, y));
        self.path.push_str(&format!("C{first} {second} {at}"));
    }

    fn close(&mut self) {
        self.path.push('Z');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own_of(text: &'static str) -> impl Fn() -> Vec<char> {
        move || text.chars().collect()
    }

    #[test]
    fn the_sample_is_chosen_from_what_the_font_covers() {
        let latin = |c: char| c.is_ascii();
        let cjk = |c: char| ('\u{4e00}'..='\u{9fff}').contains(&c);
        let symbols = |c: char| ('\u{2460}'..='\u{2473}').contains(&c);
        let ideographs = "你好世界東京都大阪府中華人民共和国日本語文字の見本を表示します";
        let circled = "\u{2460}\u{2461}\u{2462}\u{2463}\u{2464}";
        // name, covers, the font's own characters, the title, then what is said
        let cases: Vec<(&str, &dyn Fn(char) -> bool, &'static str, &str, Plan)> = vec![
            (
                "latin",
                &latin as &dyn Fn(char) -> bool,
                "",
                "Inter Bold",
                Plan {
                    title: Some("Inter Bold".to_owned()),
                    lines: SAMPLES.iter().map(|s| (*s).to_owned()).collect(),
                    pangram: Some(PANGRAM.to_owned()),
                    ramp: Some(RAMP.to_owned()),
                },
            ),
            (
                "cjk only: its own run, and a Latin name it cannot spell is left out",
                &cjk as &dyn Fn(char) -> bool,
                ideographs,
                "Noto Serif CJK",
                Plan {
                    title: None,
                    lines: vec![ideographs.chars().take(FALLBACK_CHARS).collect()],
                    pangram: Some(
                        ideographs
                            .chars()
                            .skip(FALLBACK_CHARS)
                            .take(FALLBACK_RUN - FALLBACK_CHARS)
                            .collect(),
                    ),
                    ramp: Some(ideographs.chars().take(FALLBACK_RAMP).collect()),
                },
            ),
            (
                "symbols only: no pangram when the font has too few characters for one",
                &symbols as &dyn Fn(char) -> bool,
                circled,
                "Circled",
                Plan {
                    title: None,
                    lines: vec![circled.to_owned()],
                    pangram: None,
                    ramp: Some(circled.to_owned()),
                },
            ),
            (
                "nothing at all",
                &symbols as &dyn Fn(char) -> bool,
                "",
                "",
                Plan::default(),
            ),
        ];
        for (name, covers, own, title, expected) in cases {
            assert_eq!(plan(title, covers, &own_of(own)), expected, "{name}");
        }
    }
}
