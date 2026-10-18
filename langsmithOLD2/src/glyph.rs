use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
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

/// Maps an anchor (0..9 on the 3x3 grid) to a position on a 5x5 character
/// canvas, where the anchors themselves sit at the even coordinates and the
/// odd coordinates are free for connector characters between them.
fn anchor_grid_pos(i: u8) -> (i32, i32) {
    (((i / 3) as i32) * 2, ((i % 3) as i32) * 2)
}

enum Kind {
    Line,
    Arc,
    Hook,
}

/// Picks a connector character for the straight line between two anchors
/// and drops it at the midpoint cell, which is always a valid integer grid
/// position since anchors sit on even coordinates. Anchor pairs that aren't
/// horizontal, vertical or a clean diagonal (a "knight's move") can't be
/// drawn as a straight character at this resolution, so they fall back to
/// a neutral connector rather than a misleading one.
fn place_connector(grid: &mut [[char; 5]; 5], a: u8, b: u8, kind: Kind) {
    let (r0, c0) = anchor_grid_pos(a);
    let (r1, c1) = anchor_grid_pos(b);
    let (dr, dc) = (r1 - r0, c1 - c0);
    let (mr, mc) = ((r0 + r1) / 2, (c0 + c1) / 2);
    let ch = if dr == 0 {
        match kind {
            Kind::Line => '─',
            Kind::Arc => '⌒',
            Kind::Hook => '¬',
        }
    } else if dc == 0 {
        match kind {
            Kind::Line => '│',
            Kind::Arc => '(',
            Kind::Hook => '⌐',
        }
    } else if dr.abs() == dc.abs() {
        let down_right = (dr > 0) == (dc > 0);
        match kind {
            Kind::Arc => '∿',
            _ => {
                if down_right {
                    '╲'
                } else {
                    '╱'
                }
            }
        }
    } else {
        '·'
    };
    grid[mr as usize][mc as usize] = ch;
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

    /// Self-contained SVG, one glyph per file.
    pub fn to_svg(&self) -> String {
        let size = 100.0;
        let margin = 18.0;
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\" width=\"160\" height=\"160\">\n\
             <rect width=\"100\" height=\"100\" fill=\"white\"/>\n{}</svg>\n",
            self.to_svg_fragment(size, margin)
        )
    }

    fn reference_dots_svg(size: f64, margin: f64) -> String {
        let mut s = String::new();
        for i in 0..9u8 {
            let (x, y) = Shape::anchor_xy(i, size, margin);
            s.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"#ddd\" stroke=\"none\"/>\n",
                x, y
            ));
        }
        s
    }

    /// Full fragment for embedding in a sheet or a standalone file: faint
    /// dots at all 9 real anchors (so the grid the glyph sits on is always
    /// visible), then the actual strokes drawn with flat (not round) line
    /// caps -- round caps were drawing a rounded nub at every stroke
    /// endpoint, which made glyphs look like they had far more than 9
    /// points.
    pub fn to_svg_fragment(&self, size: f64, margin: f64) -> String {
        let mut body = String::new();
        for s in &self.strokes {
            body.push_str(&self.stroke_svg(s, size, margin));
        }
        format!(
            "<g>\n{}</g>\n<g stroke=\"black\" stroke-width=\"4.5\" stroke-linecap=\"butt\" fill=\"none\">\n{}</g>\n",
            Shape::reference_dots_svg(size, margin),
            body
        )
    }

    fn stroke_svg(&self, s: &Stroke, size: f64, margin: f64) -> String {
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
                // A solid filled disc, clearly bigger/rounder than the
                // faint reference dots -- unambiguously "a dot stroke",
                // not just a grid marker.
                let (x, y) = Shape::anchor_xy(a, size, margin);
                format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"6\" fill=\"black\" stroke=\"none\"/>\n",
                    x, y
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
                     <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"black\" stroke=\"none\"/>\n",
                    x1, y1, x2, y2, x2, y2, tx, ty, tx, ty
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

    /// Renders the glyph directly on its real 3x3 anchor grid using a mixed
    /// character vocabulary -- '·' unused anchor, '•' a Dot stroke,
    /// '─'/'│'/'╱'/'╲' straight lines, '⌒'/'('  arcs, '¬'/'⌐' hooks -- so
    /// dots, lines and arcs are visually distinct from each other and the
    /// grid itself is never in doubt (unlike the braille sub-pixel look,
    /// which blurs all stroke types into the same dot-matrix texture).
    pub fn to_glyph_art(&self) -> String {
        let mut grid = [[' '; 5]; 5];
        for i in 0..9u8 {
            let (r, c) = anchor_grid_pos(i);
            grid[r as usize][c as usize] = '·';
        }
        for s in &self.strokes {
            match *s {
                Stroke::Dot(a) => {
                    let (r, c) = anchor_grid_pos(a);
                    grid[r as usize][c as usize] = '•';
                }
                Stroke::Line(a, b) => place_connector(&mut grid, a, b, Kind::Line),
                Stroke::Arc(a, b) => place_connector(&mut grid, a, b, Kind::Arc),
                Stroke::Hook(a, b) => place_connector(&mut grid, a, b, Kind::Hook),
            }
        }
        grid.iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn anchor_px(i: u8, w: i32, h: i32) -> (i32, i32) {
        let r = (i / 3) as i32;
        let c = (i % 3) as i32;
        (c * (w - 1) / 2, r * (h - 1) / 2)
    }

    fn plot(grid: &mut [Vec<bool>], x: i32, y: i32, w: i32, h: i32) {
        if x >= 0 && y >= 0 && x < w && y < h {
            grid[y as usize][x as usize] = true;
        }
    }

    fn line(grid: &mut [Vec<bool>], (mut x0, mut y0): (i32, i32), (x1, y1): (i32, i32), w: i32, h: i32) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            Shape::plot(grid, x0, y0, w, h);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Rasterize onto a `w`x`h` pixel grid and pack into Unicode braille
    /// characters (each cell = 2 cols x 4 rows of dots) for an inline,
    /// terminal-native glyph preview -- no image viewer required.
    pub fn to_braille(&self, w: i32, h: i32) -> String {
        let (wu, hu) = (w as usize, h as usize);
        let mut grid = vec![vec![false; wu]; hu];

        for s in &self.strokes {
            match *s {
                Stroke::Line(a, b) => {
                    let pa = Shape::anchor_px(a, w, h);
                    let pb = Shape::anchor_px(b, w, h);
                    Shape::line(&mut grid, pa, pb, w, h);
                }
                Stroke::Dot(a) => {
                    let (x, y) = Shape::anchor_px(a, w, h);
                    for (ox, oy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                        Shape::plot(&mut grid, x + ox, y + oy, w, h);
                    }
                }
                Stroke::Arc(a, b) => {
                    let (x1, y1) = Shape::anchor_px(a, w, h);
                    let (x2, y2) = Shape::anchor_px(b, w, h);
                    let (dx, dy) = ((x2 - x1) as f64, (y2 - y1) as f64);
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let (ox, oy) = ((-dy / len * 2.0) as i32, (dx / len * 2.0) as i32);
                    let mid = ((x1 + x2) / 2 + ox, (y1 + y2) / 2 + oy);
                    Shape::line(&mut grid, (x1, y1), mid, w, h);
                    Shape::line(&mut grid, mid, (x2, y2), w, h);
                }
                Stroke::Hook(a, b) => {
                    let pa = Shape::anchor_px(a, w, h);
                    let pb = Shape::anchor_px(b, w, h);
                    Shape::line(&mut grid, pa, pb, w, h);
                    let (dx, dy) = ((pb.0 - pa.0) as f64, (pb.1 - pa.1) as f64);
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let stub = (
                        pb.0 + (-dy / len * 2.0) as i32,
                        pb.1 + (dx / len * 2.0) as i32,
                    );
                    Shape::line(&mut grid, pb, stub, w, h);
                }
            }
        }

        let mut out = String::new();
        let rows = hu / 4;
        let cols = wu / 2;
        for cy in 0..rows {
            for cx in 0..cols {
                let mut mask: u32 = 0;
                let bit_at = |dr: usize, dc: usize| -> u32 {
                    let px = cx * 2 + dc;
                    let py = cy * 4 + dr;
                    if grid[py][px] {
                        1
                    } else {
                        0
                    }
                };
                mask |= bit_at(0, 0) << 0;
                mask |= bit_at(1, 0) << 1;
                mask |= bit_at(2, 0) << 2;
                mask |= bit_at(0, 1) << 3;
                mask |= bit_at(1, 1) << 4;
                mask |= bit_at(2, 1) << 5;
                mask |= bit_at(3, 0) << 6;
                mask |= bit_at(3, 1) << 7;
                let ch = char::from_u32(0x2800 + mask).unwrap_or('?');
                out.push(ch);
            }
            out.push('\n');
        }
        out.trim_end_matches('\n').to_string()
    }
}
