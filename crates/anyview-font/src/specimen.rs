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

/// What each line of sample text says; a line of a script the font lacks is replaced by the
/// characters it does have.
const SAMPLES: &[&str] = &["ABCDEFGHIJKLM", "abcdefghijklm", "0123456789 &?!@"];

/// The characters a replacement line shows.
const FALLBACK_CHARS: usize = 14;

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
    /// The lines, top first. Empty when the font draws none of the samples or its own characters.
    pub lines: Vec<SpecimenLine>,
}

/// The specimen of `font`.
pub(crate) fn specimen(font: &FontRef<'_>) -> Specimen {
    let charmap = font.charmap();
    let mut lines = Vec::new();
    for sample in SAMPLES {
        let glyphs: Vec<(char, GlyphId)> = sample
            .chars()
            .filter_map(|c| charmap.map(c).map(|glyph| (c, glyph)))
            .collect();
        let wanted = sample.chars().count();
        let line = if glyphs.len() * 2 >= wanted {
            glyphs
        } else {
            continue;
        };
        lines.push(set(font, &line));
    }
    if lines.is_empty() {
        let own: Vec<(char, GlyphId)> = charmap
            .mappings()
            .filter_map(|(code, glyph)| char::from_u32(code).map(|c| (c, glyph)))
            .filter(|(c, glyph)| !c.is_control() && !c.is_whitespace() && glyph.to_u32() != 0)
            .take(FALLBACK_CHARS)
            .collect();
        if !own.is_empty() {
            lines.push(set(font, &own));
        }
    }
    Specimen { lines }
}

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
