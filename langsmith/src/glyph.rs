use crate::term::Palette;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash, Hasher};

/// Anchors are laid out on a fixed 3x3 grid:
///   0 1 2
///   3 4 5
///   6 7 8
/// A stroke connects (or marks) anchors. This is deliberately a small,
/// closed vocabulary rather than free-form drawing -- constrained grammars
/// are what make generated glyphs look like a coherent script instead of
/// random scribbles (see Ogham, Cistercian numerals, Hangul jamo).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stroke {
    Line(u8, u8),
    Arc(u8, u8),
    Dot(u8),
    Hook(u8, u8),
}

impl Stroke {
    fn map_anchors(&self, f: fn(u8) -> u8) -> Stroke {
        match *self {
            Stroke::Line(a, b) => {
                let (na, nb) = (f(a), f(b));
                Stroke::Line(na.min(nb), na.max(nb))
            }
            Stroke::Arc(a, b) => {
                let (na, nb) = (f(a), f(b));
                Stroke::Arc(na.min(nb), na.max(nb))
            }
            Stroke::Dot(a) => Stroke::Dot(f(a)),
            Stroke::Hook(a, b) => Stroke::Hook(f(a), f(b)),
        }
    }

    fn sort_key(&self) -> (u8, u8, u8) {
        match *self {
            Stroke::Line(a, b) => (0, a, b),
            Stroke::Arc(a, b) => (1, a, b),
            Stroke::Dot(a) => (2, a, 0),
            Stroke::Hook(a, b) => (3, a, b),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Shape {
    pub strokes: Vec<Stroke>,
    pub seed: u64,
}

fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn mirror_h(i: u8) -> u8 {
    let (r, c) = (i / 3, i % 3);
    r * 3 + (2 - c)
}

fn mirror_v(i: u8) -> u8 {
    let (r, c) = (i / 3, i % 3);
    (2 - r) * 3 + c
}

fn random_stroke(rng: &mut StdRng) -> Stroke {
    let a = rng.gen_range(0u8..9);
    let other = |rng: &mut StdRng| loop {
        let b = rng.gen_range(0u8..9);
        if b != a {
            return b;
        }
    };
    match rng.gen_range(0..4) {
        0 => {
            let b = other(rng);
            Stroke::Line(a.min(b), a.max(b))
        }
        1 => {
            let b = other(rng);
            Stroke::Arc(a.min(b), a.max(b))
        }
        2 => Stroke::Dot(a),
        _ => Stroke::Hook(a, other(rng)),
    }
}

impl Shape {
    /// Deterministically build a shape from a raw seed: same seed, same glyph, always.
    pub fn from_seed(seed: u64) -> Shape {
        let mut rng = StdRng::seed_from_u64(seed);
        let target = rng.gen_range(3..=6);
        let mut set: HashSet<Stroke> = HashSet::new();
        let mut tries = 0;
        while set.len() < target && tries < 40 {
            set.insert(random_stroke(&mut rng));
            tries += 1;
        }
        let mut strokes: Vec<Stroke> = set.into_iter().collect();
        strokes.sort_by_key(|s| s.sort_key());
        Shape { strokes, seed }
    }

    /// Generate a glyph for `key` (usually the symbol id or gloss), retrying
    /// with a bumped seed whenever it comes out too similar to an existing
    /// glyph in the same language -- cheap collision avoidance so a language
    /// doesn't end up with three near-identical symbols by chance.
    pub fn generate(key: &str, avoid: &[Shape]) -> Shape {
        let base = hash_str(key);
        for attempt in 0u64..30 {
            let seed = base.wrapping_add(attempt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let candidate = Shape::from_seed(seed);
            if avoid.iter().all(|a| candidate.similarity(a) < 0.6) {
                return candidate;
            }
        }
        Shape::from_seed(base)
    }

    /// Jaccard similarity over stroke sets -- used for collision avoidance.
    pub fn similarity(&self, other: &Shape) -> f64 {
        let a: HashSet<_> = self.strokes.iter().collect();
        let b: HashSet<_> = other.strokes.iter().collect();
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }
        let inter = a.intersection(&b).count() as f64;
        let union = a.union(&b).count() as f64;
        if union == 0.0 {
            0.0
        } else {
            inter / union
        }
    }

    pub fn mirror(&self, axis: Axis) -> Shape {
        let f = match axis {
            Axis::Horizontal => mirror_h,
            Axis::Vertical => mirror_v,
        };
        let mut strokes: Vec<Stroke> = self.strokes.iter().map(|s| s.map_anchors(f)).collect();
        strokes.sort_by_key(|s| s.sort_key());
        Shape {
            strokes,
            seed: self.seed ^ 0xABCD_1234,
        }
    }

    /// Union of two shapes' strokes -- the visual analogue of `compose`.
    pub fn blend(&self, other: &Shape) -> Shape {
        let mut set: HashSet<Stroke> = self.strokes.iter().cloned().collect();
        set.extend(other.strokes.iter().cloned());
        let mut strokes: Vec<Stroke> = set.into_iter().collect();
        strokes.sort_by_key(|s| s.sort_key());
        strokes.truncate(8);
        Shape {
            strokes,
            seed: self.seed ^ other.seed,
        }
    }

    /// Add one new stroke not already present -- "more specific than parent".
    pub fn extend(&self, salt: u64) -> Shape {
        let mut rng = StdRng::seed_from_u64(self.seed ^ salt);
        let existing: HashSet<Stroke> = self.strokes.iter().cloned().collect();
        for _ in 0..30 {
            let candidate = random_stroke(&mut rng);
            if !existing.contains(&candidate) {
                let mut strokes = self.strokes.clone();
                strokes.push(candidate);
                strokes.sort_by_key(|s| s.sort_key());
                return Shape {
                    strokes,
                    seed: self.seed ^ salt,
                };
            }
        }
        self.clone()
    }

    /// Remove one stroke -- "more abstract than child". Keeps at least 2 strokes.
    pub fn reduce(&self, salt: u64) -> Shape {
        if self.strokes.len() <= 2 {
            return self.clone();
        }
        let mut rng = StdRng::seed_from_u64(self.seed ^ salt);
        let idx = rng.gen_range(0..self.strokes.len());
        let mut strokes = self.strokes.clone();
        strokes.remove(idx);
        Shape {
            strokes,
            seed: self.seed ^ salt,
        }
    }

    fn anchor_xy(i: u8, size: f64, margin: f64) -> (f64, f64) {
        let r = (i / 3) as f64;
        let c = (i % 3) as f64;
        let span = size - 2.0 * margin;
        (margin + c * span / 2.0, margin + r * span / 2.0)
    }

    /// Anchors that at least one stroke touches. Only these get a visible
    /// grid point when rendered; unused anchors are never drawn.
    fn touched_anchors(&self) -> BTreeSet<u8> {
        let mut set = BTreeSet::new();
        for s in &self.strokes {
            match *s {
                Stroke::Line(a, b) | Stroke::Arc(a, b) | Stroke::Hook(a, b) => {
                    set.insert(a);
                    set.insert(b);
                }
                Stroke::Dot(a) => {
                    set.insert(a);
                }
            }
        }
        set
    }

    /// Standalone SVG for one glyph, drawn in the given colours.
    pub fn to_svg(&self, pal: &Palette) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\" width=\"400\" height=\"400\">\n\
             <rect width=\"100\" height=\"100\" fill=\"{bg}\"/>\n{body}</svg>\n",
            bg = pal.bg.hex(),
            body = self.to_svg_fragment(100.0, 20.0, &pal.fg.hex())
        )
    }

    /// The glyph's strokes plus a grid point at every anchor a stroke
    /// touches (all in `fg`), for embedding in a standalone file or a sheet.
    /// Anchors nothing touches get no marker at all.
    pub fn to_svg_fragment(&self, size: f64, margin: f64, fg: &str) -> String {
        let mut strokes = String::new();
        for s in &self.strokes {
            strokes.push_str(&self.stroke_svg(s, size, margin, fg));
        }
        let mut nodes = String::new();
        for i in self.touched_anchors() {
            let (x, y) = Shape::anchor_xy(i, size, margin);
            nodes.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.6\"/>\n",
                x, y
            ));
        }
        format!(
            "<g stroke=\"{fg}\" stroke-width=\"4.5\" stroke-linecap=\"butt\" fill=\"none\">\n{strokes}</g>\n\
             <g fill=\"{fg}\" stroke=\"none\">\n{nodes}</g>\n",
            fg = fg,
            strokes = strokes,
            nodes = nodes
        )
    }

    fn stroke_svg(&self, s: &Stroke, size: f64, margin: f64, fg: &str) -> String {
        match *s {
            Stroke::Line(a, b) => {
                let (x1, y1) = Shape::anchor_xy(a, size, margin);
                let (x2, y2) = Shape::anchor_xy(b, size, margin);
                format!(
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\n",
                    x1, y1, x2, y2
                )
            }
            Stroke::Arc(a, b) => {
                // A visibly curved path (not just a kinked line) so arcs
                // read as their own stroke type, not a variant of Line.
                let (x1, y1) = Shape::anchor_xy(a, size, margin);
                let (x2, y2) = Shape::anchor_xy(b, size, margin);
                let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
                let (dx, dy) = (x2 - x1, y2 - y1);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (ox, oy) = (-dy / len * 22.0, dx / len * 22.0);
                format!(
                    "<path d=\"M {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}\"/>\n",
                    x1, y1, mx + ox, my + oy, x2, y2
                )
            }
            Stroke::Dot(a) => {
                // A solid disc, clearly bigger than the small grid-point
                // markers -- unambiguously "a dot stroke".
                let (x, y) = Shape::anchor_xy(a, size, margin);
                format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"6\" fill=\"{}\" stroke=\"none\"/>\n",
                    x, y, fg
                )
            }
            Stroke::Hook(a, b) => {
                // A straight line plus a short perpendicular flag with a
                // small dot at its tip -- reads as "line that turns",
                // distinct from both Line and Arc.
                let (x1, y1) = Shape::anchor_xy(a, size, margin);
                let (x2, y2) = Shape::anchor_xy(b, size, margin);
                let (dx, dy) = (x2 - x1, y2 - y1);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (ux, uy) = (dx / len, dy / len);
                let (px, py) = (-uy * 16.0, ux * 16.0);
                let (tx, ty) = (x2 + px, y2 + py);
                format!(
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\n\
                     <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\n\
                     <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"{}\" stroke=\"none\"/>\n",
                    x1, y1, x2, y2, x2, y2, tx, ty, tx, ty, fg
                )
            }
        }
    }

    fn stroke_counts(&self) -> (usize, usize, usize, usize) {
        let mut counts = (0, 0, 0, 0); // line, arc, dot, hook
        for s in &self.strokes {
            match s {
                Stroke::Line(..) => counts.0 += 1,
                Stroke::Arc(..) => counts.1 += 1,
                Stroke::Dot(..) => counts.2 += 1,
                Stroke::Hook(..) => counts.3 += 1,
            }
        }
        counts
    }

    /// A compact, single-line stroke inventory for table/tree/diff contexts
    /// where a multi-line grid would break the layout, e.g. "<2L 1A 1D>".
    pub fn summary(&self) -> String {
        let (l, a, d, h) = self.stroke_counts();
        let mut parts = Vec::new();
        if l > 0 {
            parts.push(format!("{}L", l));
        }
        if a > 0 {
            parts.push(format!("{}A", a));
        }
        if d > 0 {
            parts.push(format!("{}D", d));
        }
        if h > 0 {
            parts.push(format!("{}H", h));
        }
        format!("<{}>", parts.join(" "))
    }
}
