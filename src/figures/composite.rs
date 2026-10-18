//! Composite shape model: the `spec` text of the composite-shape figure as editable items.
//!
//! The spec is a list of rows separated by `;` (or new lines). Pieces: `rect X Y W H`,
//! `tri X1 Y1 X2 Y2 X3 Y3`, `poly X Y X Y ...`, `circle CX CY R`, `semi CX CY R u/d/l/r`,
//! `quarter CX CY R ne/nw/se/sw`, each optionally ending with a shading word (blue, red, green,
//! yellow) and ` | LABEL` (text drawn at the piece's centre). Marks: `dash`, `dim`, `ticks`,
//! `arrows`, `text`, `ra`, and the Label-mode rows `side X1 Y1 X2 Y2 dashed|dotted|hidden|solid
//! [thin|regular|thick]` (an outline side's line), `angle X Y [ext] [measure] [arcs N] [square]
//! [shade] [| TEXT]` (an angle mark at a corner) and `point X Y [NAME]` (a corner point). The renderer (`math::composite_render`) and the canvas editor
//! (`figuresui::composite_canvas`) both read the spec through `parse`, so they always agree.
//!
//! Rows are kept verbatim unless edited (`Item::set`), so a typed row the editor never touched
//! round-trips byte for byte, unknown rows included.
use super::math::{ell, quarter_pts};

pub type Pt = (f64, f64);

/// Shading words and their fills (the reference's Blue / Red / Green / Yellow).
pub const SHADES: [(&str, &str); 4] = [("blue", "#cfe1fa"), ("red", "#fbd0d0"), ("green", "#c8f2d8"), ("yellow", "#fde8a0")];

pub fn shade_hex(word: &str) -> Option<&'static str> {
    let w = word.to_lowercase();
    SHADES.iter().find(|(k, _)| *k == w).map(|(_, h)| *h)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect { x: f64, y: f64, w: f64, h: f64 },
    Tri([Pt; 3]),
    Poly(Vec<Pt>),
    Circle { c: Pt, r: f64 },
    /// `dir`: the way it bulges, u / d / l / r.
    Semi { c: Pt, r: f64, dir: String },
    /// `dir`: ne / nw / se / sw.
    Quarter { c: Pt, r: f64, dir: String },
    Dash(Pt, Pt),
    /// Dimension line; empty text shows the measured length.
    Dim(Pt, Pt, String),
    Ticks(Pt, Pt, u8),
    Arrows(Pt, Pt, u8),
    Text(Pt, String),
    /// Right-angle mark at a corner, NE / NW / SE / SW.
    Ra(Pt, String),
    /// Line of the outline side (a, b): style solid / dashed / dotted / hidden, thickness thin /
    /// regular / thick (`side X1 Y1 X2 Y2 STYLE [THICKNESS]`).
    SideStyle(Pt, Pt, String, String),
    /// Angle mark at corner `v`: the interior angle, or the exterior one (`ext`); its label is the
    /// measure (`measure`) or the row's ` | TEXT`; congruence arcs 0-3, a right-angle square, and
    /// the row's shading word fills the wedge.
    Angle { v: Pt, ext: bool, measure: bool, arcs: u8, square: bool },
    /// A point marker on a corner, with an optional name (`point X Y [NAME]`).
    Point(Pt, String),
}

impl Shape {
    pub fn is_piece(&self) -> bool {
        matches!(self, Shape::Rect { .. } | Shape::Tri(_) | Shape::Poly(_) | Shape::Circle { .. } | Shape::Semi { .. } | Shape::Quarter { .. })
    }
    pub fn name(&self) -> &'static str {
        match self {
            Shape::Rect { .. } => "Rectangle",
            Shape::Tri(_) => "Triangle",
            Shape::Poly(_) => "Polygon",
            Shape::Circle { .. } => "Circle",
            Shape::Semi { .. } => "Semicircle",
            Shape::Quarter { .. } => "Quarter circle",
            Shape::Dash(..) => "Line",
            Shape::Dim(..) => "Dimension",
            Shape::Ticks(..) => "Congruence ticks",
            Shape::Arrows(..) => "Parallel arrows",
            Shape::Text(..) => "Text",
            Shape::Ra(..) => "Right angle",
            Shape::SideStyle(..) => "Side line",
            Shape::Angle { .. } => "Angle",
            Shape::Point(..) => "Point",
        }
    }
}

/// One spec row.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// None = a row that does not parse (kept as typed).
    pub shape: Option<Shape>,
    /// Shading word (lower case) of a piece.
    pub shade: Option<String>,
    /// Text after ` | ` on a piece row.
    pub label: String,
    raw: String,
    dirty: bool,
}

impl Item {
    pub fn new(shape: Shape) -> Item {
        Item { shape: Some(shape), shade: None, label: String::new(), raw: String::new(), dirty: true }
    }
    pub fn is_piece(&self) -> bool {
        self.shape.as_ref().is_some_and(|s| s.is_piece())
    }
    /// Whether the row carries a shading word and a ` | label` (pieces and angle marks).
    fn decorated(&self) -> bool {
        self.shape.as_ref().is_some_and(|s| s.is_piece() || matches!(s, Shape::Angle { .. }))
    }
    /// Marks the row as edited: it is written from its fields from now on.
    pub fn touch(&mut self) {
        self.dirty = true;
    }
    pub fn set(&mut self, s: Shape) {
        self.shape = Some(s);
        self.dirty = true;
    }
    pub fn set_shade(&mut self, w: Option<&str>) {
        self.shade = w.map(|w| w.to_lowercase());
        self.dirty = true;
    }
    pub fn set_label(&mut self, l: &str) {
        self.label = l.trim().to_string();
        self.dirty = true;
    }
    /// The row's spec text.
    pub fn row(&self) -> String {
        let Some(s) = self.shape.as_ref().filter(|_| self.dirty) else { return self.raw.clone() };
        let n = fmt_n;
        let p = |q: Pt| format!("{} {}", n(q.0), n(q.1));
        let mut out = match s {
            Shape::Rect { x, y, w, h } => format!("rect {} {} {} {}", n(*x), n(*y), n(*w), n(*h)),
            Shape::Tri(v) => format!("tri {} {} {}", p(v[0]), p(v[1]), p(v[2])),
            Shape::Poly(v) => format!("poly {}", v.iter().map(|q| p(*q)).collect::<Vec<_>>().join(" ")),
            Shape::Circle { c, r } => format!("circle {} {}", p(*c), n(*r)),
            Shape::Semi { c, r, dir } => format!("semi {} {} {dir}", p(*c), n(*r)),
            Shape::Quarter { c, r, dir } => format!("quarter {} {} {dir}", p(*c), n(*r)),
            Shape::Dash(a, b) => format!("dash {} {}", p(*a), p(*b)),
            Shape::Dim(a, b, t) => format!("dim {} {} {}", p(*a), p(*b), t.trim()).trim_end().to_string(),
            Shape::Ticks(a, b, k) => format!("ticks {} {} {k}", p(*a), p(*b)),
            Shape::Arrows(a, b, k) => format!("arrows {} {} {k}", p(*a), p(*b)),
            Shape::Text(a, t) => format!("text {} {}", p(*a), t.trim()),
            Shape::Ra(a, d) => format!("ra {} {d}", p(*a)),
            Shape::SideStyle(a, b, st, th) => {
                let mut r = format!("side {} {} {st}", p(*a), p(*b));
                if th != "regular" {
                    r.push(' ');
                    r.push_str(th);
                }
                r
            }
            Shape::Angle { v, ext, measure, arcs, square } => {
                let mut r = format!("angle {}", p(*v));
                if *ext {
                    r.push_str(" ext");
                }
                if *measure {
                    r.push_str(" measure");
                }
                if *arcs > 0 {
                    r.push_str(&format!(" arcs {arcs}"));
                }
                if *square {
                    r.push_str(" square");
                }
                r
            }
            Shape::Point(a, n) => format!("point {} {}", p(*a), n.trim()).trim_end().to_string(),
        };
        if self.decorated() {
            if let Some(w) = &self.shade {
                out.push(' ');
                out.push_str(w);
            }
            if !self.label.trim().is_empty() {
                out.push_str(" | ");
                out.push_str(self.label.trim());
            }
        }
        out
    }
}

/// Compact number: up to 4 decimals, no trailing zeros, "-0" -> "0".
pub fn fmt_n(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() { "0".into() } else { s }
}

const PIECE_WORDS: [&str; 6] = ["rect", "tri", "poly", "circle", "semi", "quarter"];
/// Line styles and thicknesses of `side` rows (the first of each is the plain outline).
pub const LINE_STYLES: [&str; 4] = ["solid", "dashed", "dotted", "hidden"];
pub const THICKNESSES: [&str; 3] = ["regular", "thin", "thick"];

pub fn parse_row(row: &str) -> Item {
    let raw = row.trim().to_string();
    let kw = raw.split_whitespace().next().unwrap_or("").to_lowercase();
    // only piece rows carry a label: `text` rows may use | for absolute values
    let (head, label) = match raw.split_once('|') {
        Some((h, l)) if PIECE_WORDS.contains(&kw.as_str()) || kw == "angle" => (h.to_string(), l.trim().to_string()),
        _ => (raw.clone(), String::new()),
    };
    let tk: Vec<&str> = head.split_whitespace().collect();
    let shape = shape_of(&kw, &tk);
    let shade = if shape.as_ref().is_some_and(|s| s.is_piece() || matches!(s, Shape::Angle { .. })) {
        tk.iter().skip(1).find_map(|t| shade_hex(t).map(|_| t.to_lowercase()))
    } else {
        None
    };
    Item { shape, shade, label, raw, dirty: false }
}

fn shape_of(kw: &str, tk: &[&str]) -> Option<Shape> {
    let num = |i: usize| tk.get(i).and_then(|s| s.replace('\u{2212}', "-").parse::<f64>().ok()).filter(|v| v.is_finite());
    let pt = |i: usize| Some((num(i)?, num(i + 1)?));
    let word = |i: usize| tk.get(i).map(|s| s.to_lowercase());
    Some(match kw {
        "rect" => Shape::Rect { x: num(1)?, y: num(2)?, w: num(3)?, h: num(4)? },
        "tri" => Shape::Tri([pt(1)?, pt(3)?, pt(5)?]),
        "poly" => {
            let v: Vec<f64> = tk[1..].iter().filter_map(|s| s.replace('\u{2212}', "-").parse::<f64>().ok()).collect();
            if v.len() < 6 {
                return None;
            }
            Shape::Poly(v.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect())
        }
        "circle" => Shape::Circle { c: pt(1)?, r: num(3)? },
        "semi" => {
            let d = match word(4).as_deref() { Some("d") => "d", Some("l") => "l", Some("r") => "r", _ => "u" };
            Shape::Semi { c: pt(1)?, r: num(3)?, dir: d.into() }
        }
        "quarter" => {
            let d = match word(4).as_deref() { Some("nw") => "nw", Some("sw") => "sw", Some("se") => "se", _ => "ne" };
            Shape::Quarter { c: pt(1)?, r: num(3)?, dir: d.into() }
        }
        "dash" => Shape::Dash(pt(1)?, pt(3)?),
        "dim" => Shape::Dim(pt(1)?, pt(3)?, tk.get(5..).map(|t| t.join(" ")).unwrap_or_default()),
        "ticks" | "arrows" => {
            let (a, b) = (pt(1)?, pt(3)?);
            let k = num(5).unwrap_or(1.0).clamp(1.0, 3.0) as u8;
            if kw == "ticks" { Shape::Ticks(a, b, k) } else { Shape::Arrows(a, b, k) }
        }
        "text" => Shape::Text(pt(1)?, tk.get(3..).map(|t| t.join(" ")).unwrap_or_default()),
        "ra" => Shape::Ra(pt(1)?, tk.get(3).map(|s| s.to_uppercase()).unwrap_or_else(|| "NE".into())),
        "side" => {
            let words: Vec<String> = tk.iter().skip(5).map(|t| t.to_lowercase()).collect();
            let st = LINE_STYLES.iter().find(|w| words.iter().any(|t| t == *w)).unwrap_or(&"solid");
            let th = THICKNESSES.iter().find(|w| words.iter().any(|t| t == *w)).unwrap_or(&"regular");
            Shape::SideStyle(pt(1)?, pt(3)?, st.to_string(), th.to_string())
        }
        "angle" => {
            let v = pt(1)?;
            let words: Vec<String> = tk.iter().skip(3).map(|t| t.to_lowercase()).collect();
            let has = |w: &str| words.iter().any(|t| t == w);
            let arcs = words.iter().position(|t| t == "arcs").and_then(|i| words.get(i + 1)).and_then(|n| n.parse::<f64>().ok()).map(|n| n.clamp(0.0, 3.0) as u8).unwrap_or(0);
            Shape::Angle { v, ext: has("ext") || has("exterior"), measure: has("measure"), arcs, square: has("square") }
        }
        "point" => Shape::Point(pt(1)?, tk.get(3..).map(|t| t.join(" ")).unwrap_or_default()),
        _ => return None,
    })
}

pub fn parse(spec: &str) -> Vec<Item> {
    spec.split([';', '\n']).filter(|r| !r.trim().is_empty()).map(parse_row).collect()
}

pub fn to_spec(items: &[Item]) -> String {
    items.iter().map(|i| i.row()).filter(|r| !r.trim().is_empty()).collect::<Vec<_>>().join("; ")
}

// ───────────────────────── geometry ─────────────────────────

pub fn dist(a: Pt, b: Pt) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}
fn near(a: Pt, b: Pt) -> bool {
    dist(a, b) < 1e-6
}
pub fn snap(v: f64, step: f64) -> f64 {
    if step > 0.0 { (v / step).round() * step } else { v }
}
pub fn snap_pt(q: Pt, step: f64) -> Pt {
    (snap(q.0, step), snap(q.1, step))
}

fn semi_angles(dir: &str) -> (f64, f64) {
    match dir { "d" => (180.0, 360.0), "l" => (90.0, 270.0), "r" => (-90.0, 90.0), _ => (0.0, 180.0) }
}

/// Outline of a piece (model units, y up); empty for marks.
pub fn outline(s: &Shape) -> Vec<Pt> {
    match s {
        Shape::Rect { x, y, w, h } => vec![(*x, *y), (x + w, *y), (x + w, y + h), (*x, y + h)],
        Shape::Tri(v) => v.to_vec(),
        Shape::Poly(v) => v.clone(),
        Shape::Circle { c, r } => {
            let mut v = ell(c.0, c.1, *r, *r, 0.0, 360.0);
            v.pop();
            v
        }
        Shape::Semi { c, r, dir } => {
            let (a0, a1) = semi_angles(dir);
            ell(c.0, c.1, *r, *r, a0, a1)
        }
        Shape::Quarter { c, r, dir } => quarter_pts(c.0, c.1, *r, dir),
        _ => vec![],
    }
}

/// The straight sides of a piece (a circle has none; a semicircle its diameter; a quarter
/// circle its two radii).
pub fn straight_sides(s: &Shape) -> Vec<(Pt, Pt)> {
    match s {
        Shape::Rect { .. } | Shape::Tri(_) | Shape::Poly(_) => {
            let v = outline(s);
            (0..v.len()).map(|i| (v[i], v[(i + 1) % v.len()])).filter(|(a, b)| !near(*a, *b)).collect()
        }
        Shape::Semi { .. } => {
            let v = outline(s);
            vec![(v[v.len() - 1], v[0])]
        }
        Shape::Quarter { .. } => {
            let v = outline(s);
            vec![(v[0], v[1]), (v[v.len() - 1], v[0])]
        }
        _ => vec![],
    }
}

/// The curved runs of a piece's outline, as polylines: a circle all round, a semicircle's or a
/// quarter circle's arc (their straight sides are `straight_sides`).
pub fn curved_edges(s: &Shape) -> Vec<Vec<Pt>> {
    let v = outline(s);
    match s {
        Shape::Circle { .. } => {
            let mut c = v.clone();
            if let Some(f) = v.first() {
                c.push(*f);
            }
            vec![c]
        }
        Shape::Semi { .. } => vec![v],
        Shape::Quarter { .. } => vec![v[1..].to_vec()],
        _ => vec![],
    }
}

/// The defining points of a mark (endpoints / position); empty for pieces.
pub fn mark_points(s: &Shape) -> Vec<Pt> {
    match s {
        Shape::Dash(a, b) | Shape::Dim(a, b, _) | Shape::Ticks(a, b, _) | Shape::Arrows(a, b, _) => vec![*a, *b],
        Shape::Text(a, _) | Shape::Ra(a, _) | Shape::Point(a, _) | Shape::Angle { v: a, .. } => vec![*a],
        Shape::SideStyle(a, b, ..) => vec![*a, *b],
        _ => vec![],
    }
}

/// Every point that should be in view: piece outlines and mark points.
pub fn all_points(items: &[Item]) -> Vec<Pt> {
    let mut v = vec![];
    for s in items.iter().filter_map(|i| i.shape.as_ref()) {
        v.extend(outline(s));
        v.extend(mark_points(s));
    }
    v
}

pub fn bbox(pts: &[Pt]) -> Option<(f64, f64, f64, f64)> {
    if pts.is_empty() {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in pts {
        x0 = x0.min(q.0);
        y0 = y0.min(q.1);
        x1 = x1.max(q.0);
        y1 = y1.max(q.1);
    }
    Some((x0, y0, x1, y1))
}

/// Point in polygon (even-odd).
pub fn inside(poly: &[Pt], q: Pt) -> bool {
    let mut c = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.1 > q.1) != (b.1 > q.1) && q.0 < (b.0 - a.0) * (q.1 - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
        j = i;
    }
    c
}

pub fn seg_dist(q: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-18 {
        return dist(q, a);
    }
    let t = (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    dist(q, (a.0 + t * dx, a.1 + t * dy))
}

fn closed_dist(poly: &[Pt], q: Pt) -> f64 {
    (0..poly.len()).map(|i| seg_dist(q, poly[i], poly[(i + 1) % poly.len()])).fold(f64::MAX, f64::min)
}

/// Area centroid of a polygon (falls back to the vertex mean when degenerate).
pub fn centroid(v: &[Pt]) -> Pt {
    let n = v.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (v[i], v[(i + 1) % n]);
        let c = p.0 * q.1 - q.0 * p.1;
        a += c;
        cx += (p.0 + q.0) * c;
        cy += (p.1 + q.1) * c;
    }
    if a.abs() < 1e-12 {
        let k = n as f64;
        return (v.iter().map(|q| q.0).sum::<f64>() / k, v.iter().map(|q| q.1).sum::<f64>() / k);
    }
    (cx / (3.0 * a), cy / (3.0 * a))
}

/// Topmost item under `q` (model units); marks win over pieces; `tol` in model units.
pub fn hit(items: &[Item], q: Pt, tol: f64) -> Option<usize> {
    for (i, it) in items.iter().enumerate().rev() {
        let Some(s) = &it.shape else { continue };
        let ok = match s {
            Shape::Dash(a, b) | Shape::Dim(a, b, _) | Shape::Ticks(a, b, _) | Shape::Arrows(a, b, _) => seg_dist(q, *a, *b) <= tol,
            Shape::Text(a, _) => dist(q, *a) <= tol * 2.5,
            Shape::Ra(a, _) => dist(q, *a) <= tol * 1.5,
            _ => false,
        };
        if ok {
            return Some(i);
        }
    }
    for (i, it) in items.iter().enumerate().rev() {
        let Some(s) = &it.shape else { continue };
        if !s.is_piece() {
            continue;
        }
        let o = outline(s);
        if inside(&o, q) || closed_dist(&o, q) <= tol {
            return Some(i);
        }
    }
    None
}

pub fn translate(s: &mut Shape, d: Pt) {
    let m = |q: &mut Pt| {
        q.0 += d.0;
        q.1 += d.1;
    };
    match s {
        Shape::Rect { x, y, .. } => {
            *x += d.0;
            *y += d.1;
        }
        Shape::Tri(v) => v.iter_mut().for_each(m),
        Shape::Poly(v) => v.iter_mut().for_each(m),
        Shape::Circle { c, .. } | Shape::Semi { c, .. } | Shape::Quarter { c, .. } => m(c),
        Shape::Dash(a, b) | Shape::Dim(a, b, _) | Shape::Ticks(a, b, _) | Shape::Arrows(a, b, _) => {
            m(a);
            m(b);
        }
        Shape::Text(a, _) | Shape::Ra(a, _) | Shape::Point(a, _) | Shape::Angle { v: a, .. } => m(a),
        Shape::SideStyle(a, b, ..) => {
            m(a);
            m(b);
        }
    }
}

/// Whether `q` lies on the boundary of piece `s` (or at its centre, for round pieces).
fn on_piece(s: &Shape, q: Pt) -> bool {
    let tol = 1e-6;
    if straight_sides(s).iter().any(|(a, b)| seg_dist(q, *a, *b) < tol) {
        return true;
    }
    match s {
        Shape::Circle { c, r } | Shape::Semi { c, r, .. } | Shape::Quarter { c, r, .. } => {
            near(*c, q) || ((dist(*c, q) - r).abs() < tol && (inside(&outline(s), q) || closed_dist(&outline(s), q) < 0.05 * r.max(0.1)))
        }
        Shape::Rect { .. } | Shape::Tri(_) | Shape::Poly(_) => false,
        _ => false,
    }
}
fn on_or_in(s: &Shape, q: Pt) -> bool {
    on_piece(s, q) || inside(&outline(s), q)
}

/// Marks that belong to piece `i`: every point on its boundary (dims, ticks, right angles, a
/// radius) or, for a text, inside it.
pub fn attached(items: &[Item], i: usize) -> Vec<usize> {
    let Some(ps) = items.get(i).and_then(|it| it.shape.as_ref()).filter(|s| s.is_piece()) else { return vec![] };
    let mut out = vec![];
    for (j, it) in items.iter().enumerate() {
        let Some(s) = &it.shape else { continue };
        if s.is_piece() {
            continue;
        }
        let pts = mark_points(s);
        let ok = match s {
            Shape::Text(a, _) => on_or_in(ps, *a),
            _ => pts.iter().all(|q| on_piece(ps, *q)),
        };
        if ok {
            out.push(j);
        }
    }
    out
}

/// Moves item `i` by `d`; a piece takes its attached marks along.
pub fn move_item(items: &mut [Item], i: usize, d: Pt) {
    let att = attached(items, i);
    for j in std::iter::once(i).chain(att) {
        if let Some(s) = items[j].shape.as_mut() {
            translate(s, d);
            items[j].dirty = true;
        }
    }
}

/// Duplicates item `i` offset by `off` (a piece is copied without its marks); returns the copy.
pub fn duplicate(items: &mut Vec<Item>, i: usize, off: Pt) -> Option<usize> {
    let mut c = items.get(i)?.clone();
    if let Some(s) = c.shape.as_mut() {
        translate(s, off);
    }
    c.dirty = true;
    items.push(c);
    Some(items.len() - 1)
}

/// Deletes item `i`; a piece also takes the marks that belong only to it.
pub fn delete(items: &mut Vec<Item>, i: usize) {
    if i >= items.len() {
        return;
    }
    let others: Vec<Shape> = items.iter().enumerate().filter(|(j, it)| *j != i && it.is_piece()).filter_map(|(_, it)| it.shape.clone()).collect();
    let mut gone: Vec<usize> = attached(items, i)
        .into_iter()
        .filter(|&j| {
            let pts = items[j].shape.as_ref().map(mark_points).unwrap_or_default();
            !pts.iter().all(|q| others.iter().any(|o| on_or_in(o, *q)))
        })
        .collect();
    gone.push(i);
    gone.sort_unstable();
    gone.dedup();
    for j in gone.into_iter().rev() {
        items.remove(j);
    }
}

// ───────────────────────── grips (resize handles) ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grip {
    /// Rectangle corner 0..4 (counter-clockwise from bottom-left).
    Corner(usize),
    /// Rectangle side 0 bottom, 1 right, 2 top, 3 left.
    Side(usize),
    /// Triangle / polygon vertex.
    Vertex(usize),
    Radius,
    /// Mark endpoint 0 / 1.
    End(usize),
}

pub fn grips(s: &Shape) -> Vec<(Grip, Pt)> {
    match s {
        Shape::Rect { x, y, w, h } => {
            let c = [(*x, *y), (x + w, *y), (x + w, y + h), (*x, y + h)];
            let mut v: Vec<(Grip, Pt)> = c.iter().enumerate().map(|(k, q)| (Grip::Corner(k), *q)).collect();
            for k in 0..4 {
                let (a, b) = (c[k], c[(k + 1) % 4]);
                v.push((Grip::Side(k), ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)));
            }
            v
        }
        Shape::Tri(v) => v.iter().enumerate().map(|(k, q)| (Grip::Vertex(k), *q)).collect(),
        Shape::Poly(v) => v.iter().enumerate().map(|(k, q)| (Grip::Vertex(k), *q)).collect(),
        Shape::Circle { c, r } => vec![(Grip::Radius, (c.0 + r, c.1))],
        Shape::Semi { c, r, dir } => {
            let q = match dir.as_str() { "d" => (c.0, c.1 - r), "l" => (c.0 - r, c.1), "r" => (c.0 + r, c.1), _ => (c.0, c.1 + r) };
            vec![(Grip::Radius, q)]
        }
        Shape::Quarter { c, r, dir } => {
            let a: f64 = match dir.as_str() { "nw" => 135.0, "sw" => 225.0, "se" => 315.0, _ => 45.0 };
            vec![(Grip::Radius, (c.0 + r * a.to_radians().cos(), c.1 + r * a.to_radians().sin()))]
        }
        Shape::Dash(a, b) | Shape::Dim(a, b, _) | Shape::Ticks(a, b, _) | Shape::Arrows(a, b, _) => vec![(Grip::End(0), *a), (Grip::End(1), *b)],
        Shape::Text(..) | Shape::Ra(..) | Shape::SideStyle(..) | Shape::Angle { .. } | Shape::Point(..) => vec![],
    }
}

const MIN_SIZE: f64 = 0.1;

/// Moves grip `g` of `s` to `q`.
pub fn drag_grip(s: &mut Shape, g: Grip, q: Pt) {
    match (s, g) {
        (Shape::Rect { x, y, w, h }, Grip::Corner(k)) => {
            let c = [(*x, *y), (*x + *w, *y), (*x + *w, *y + *h), (*x, *y + *h)];
            let opp = c[(k + 2) % 4];
            *x = q.0.min(opp.0);
            *y = q.1.min(opp.1);
            *w = (q.0 - opp.0).abs().max(MIN_SIZE);
            *h = (q.1 - opp.1).abs().max(MIN_SIZE);
        }
        (Shape::Rect { x, y, w, h }, Grip::Side(k)) => {
            let (x1, y1) = (*x + *w, *y + *h);
            match k {
                0 => { *y = q.1.min(y1 - MIN_SIZE); *h = y1 - *y; }
                1 => { *w = (q.0 - *x).max(MIN_SIZE); }
                2 => { *h = (q.1 - *y).max(MIN_SIZE); }
                _ => { *x = q.0.min(x1 - MIN_SIZE); *w = x1 - *x; }
            }
        }
        (Shape::Tri(v), Grip::Vertex(k)) if k < 3 => v[k] = q,
        (Shape::Poly(v), Grip::Vertex(k)) if k < v.len() => v[k] = q,
        (Shape::Circle { c, r } | Shape::Semi { c, r, .. } | Shape::Quarter { c, r, .. }, Grip::Radius) => *r = dist(*c, q).max(MIN_SIZE),
        (Shape::Dash(a, b) | Shape::Dim(a, b, _) | Shape::Ticks(a, b, _) | Shape::Arrows(a, b, _), Grip::End(k)) => {
            if k == 0 { *a = q } else { *b = q }
        }
        _ => {}
    }
}

/// Drags grip `g` of item `i`; marks sitting on a vertex that moved follow it.
pub fn drag_grip_linked(items: &mut [Item], i: usize, g: Grip, q: Pt) {
    let Some(mut s) = items.get(i).and_then(|it| it.shape.clone()) else { return };
    let before = outline(&s);
    drag_grip(&mut s, g, q);
    let after = outline(&s);
    items[i].set(s);
    if before.len() != after.len() || !matches!(g, Grip::Corner(_) | Grip::Side(_) | Grip::Vertex(_)) {
        return;
    }
    let moved: Vec<(Pt, Pt)> = before.iter().zip(&after).filter(|(a, b)| !near(**a, **b)).map(|(a, b)| (*a, *b)).collect();
    for (j, it) in items.iter_mut().enumerate() {
        if j == i || it.is_piece() {
            continue;
        }
        let Some(s) = it.shape.as_mut() else { continue };
        let mut hit = false;
        let mut f = |p: &mut Pt| {
            if let Some((_, to)) = moved.iter().find(|(from, _)| near(*from, *p)) {
                *p = *to;
                hit = true;
            }
        };
        match s {
            Shape::Dash(a, b) | Shape::Dim(a, b, _) | Shape::Ticks(a, b, _) | Shape::Arrows(a, b, _) => {
                f(a);
                f(b);
            }
            Shape::Ra(a, _) | Shape::Point(a, _) | Shape::Angle { v: a, .. } => f(a),
            Shape::SideStyle(a, b, ..) => {
                f(a);
                f(b);
            }
            _ => {}
        }
        if hit {
            it.dirty = true;
        }
    }
}

// ───────────────────────── new pieces ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewKind { Rect, Tri, Semi, Quarter, Circle }

/// A piece dragged out from `a` to `b` (None when the drag is too small: use `default_piece`).
pub fn piece_from_drag(kind: NewKind, a: Pt, b: Pt) -> Option<Shape> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.abs().max(dy.abs()) < MIN_SIZE {
        return None;
    }
    Some(match kind {
        NewKind::Rect => {
            if dx.abs() < MIN_SIZE || dy.abs() < MIN_SIZE {
                return None;
            }
            Shape::Rect { x: a.0.min(b.0), y: a.1.min(b.1), w: dx.abs(), h: dy.abs() }
        }
        NewKind::Tri => {
            if dx.abs() < MIN_SIZE || dy.abs() < MIN_SIZE {
                return None;
            }
            // base along the start's row, apex over the middle at the end's row
            let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
            Shape::Tri([(x0, a.1), (x1, a.1), ((x0 + x1) / 2.0, b.1)])
        }
        NewKind::Semi => {
            let (r, d) = if dx.abs() >= dy.abs() { (dx.abs(), if dx > 0.0 { "r" } else { "l" }) } else { (dy.abs(), if dy > 0.0 { "u" } else { "d" }) };
            Shape::Semi { c: a, r, dir: d.into() }
        }
        NewKind::Quarter => {
            let d = match (dx >= 0.0, dy >= 0.0) { (true, true) => "ne", (false, true) => "nw", (false, false) => "sw", (true, false) => "se" };
            Shape::Quarter { c: a, r: dx.abs().max(dy.abs()), dir: d.into() }
        }
        NewKind::Circle => Shape::Circle { c: a, r: dist(a, b) },
    })
}

/// A piece placed by a plain click at `a`.
pub fn default_piece(kind: NewKind, a: Pt) -> Shape {
    match kind {
        NewKind::Rect => Shape::Rect { x: a.0, y: a.1, w: 4.0, h: 3.0 },
        NewKind::Tri => Shape::Tri([a, (a.0 + 4.0, a.1), (a.0 + 2.0, a.1 + 3.0)]),
        NewKind::Semi => Shape::Semi { c: a, r: 2.0, dir: "u".into() },
        NewKind::Quarter => Shape::Quarter { c: a, r: 3.0, dir: "ne".into() },
        NewKind::Circle => Shape::Circle { c: a, r: 2.0 },
    }
}

/// The free-drawn polygon from clicked points (repeats dropped; 3 points make a triangle).
pub fn poly_from(pts: &[Pt]) -> Option<Shape> {
    let mut v: Vec<Pt> = vec![];
    for q in pts {
        if v.last().is_none_or(|l| !near(*l, *q)) {
            v.push(*q);
        }
    }
    while v.len() > 1 && near(v[0], v[v.len() - 1]) {
        v.pop();
    }
    match v.len() {
        0..=2 => None,
        3 => Some(Shape::Tri([v[0], v[1], v[2]])),
        _ => Some(Shape::Poly(v)),
    }
}

// ───────────────────────── outer sides, side marks, corners ─────────────────────────

/// A straight piece of the composite outline: `out` is its outward unit normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Side {
    pub a: Pt,
    pub b: Pt,
    pub out: Pt,
    pub piece: usize,
}
impl Side {
    pub fn mid(&self) -> Pt {
        ((self.a.0 + self.b.0) / 2.0, (self.a.1 + self.b.1) / 2.0)
    }
    pub fn len(&self) -> f64 {
        dist(self.a, self.b)
    }
}

fn outward(poly: &[Pt], a: Pt, b: Pt) -> Pt {
    let l = dist(a, b).max(1e-12);
    let n = ((b.1 - a.1) / l, -(b.0 - a.0) / l);
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let e = (l * 1e-3).max(1e-4);
    if inside(poly, (mid.0 + n.0 * e, mid.1 + n.1 * e)) { (-n.0, -n.1) } else { n }
}

/// The straight sides of the composite outline: every piece side minus the stretches it shares
/// with another piece (the L-shape gives 6 sides).
pub fn outer_sides(items: &[Item]) -> Vec<Side> {
    let pieces: Vec<(usize, &Shape)> = items.iter().enumerate().filter_map(|(i, it)| it.shape.as_ref().filter(|s| s.is_piece()).map(|s| (i, s))).collect();
    let mut out = vec![];
    for &(i, s) in &pieces {
        let poly = outline(s);
        for (a, b) in straight_sides(s) {
            let l = dist(a, b);
            if l < 1e-9 {
                continue;
            }
            let u = ((b.0 - a.0) / l, (b.1 - a.1) / l);
            let t = |q: Pt| (q.0 - a.0) * u.0 + (q.1 - a.1) * u.1;
            let off = |q: Pt| ((q.0 - a.0) * u.1 - (q.1 - a.1) * u.0).abs();
            let mut cover: Vec<(f64, f64)> = vec![];
            for &(j, o) in &pieces {
                if j == i {
                    continue;
                }
                for (c, d) in straight_sides(o) {
                    if off(c) > 1e-6 || off(d) > 1e-6 {
                        continue;
                    }
                    let (t0, t1) = (t(c).min(t(d)).max(0.0), t(c).max(t(d)).min(l));
                    if t1 - t0 > 1e-6 {
                        cover.push((t0, t1));
                    }
                }
            }
            cover.sort_by(|x, y| x.0.total_cmp(&y.0));
            let n = outward(&poly, a, b);
            let at = |s: f64| (a.0 + u.0 * s, a.1 + u.1 * s);
            let mut cur = 0.0;
            for (t0, t1) in cover {
                if t0 - cur > 1e-6 {
                    out.push(Side { a: at(cur), b: at(t0), out: n, piece: i });
                }
                cur = f64::max(cur, t1);
            }
            if l - cur > 1e-6 {
                out.push(Side { a: at(cur), b: at(l), out: n, piece: i });
            }
        }
    }
    out
}

fn same_seg(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let tol = 1e-4;
    (dist(a, c) < tol && dist(b, d) < tol) || (dist(a, d) < tol && dist(b, c) < tol)
}

/// The label state of a side: None, Some("") for its measure, Some(text) for a typed label.
pub fn side_label(items: &[Item], a: Pt, b: Pt) -> Option<String> {
    items.iter().find_map(|it| match &it.shape {
        Some(Shape::Dim(c, d, t)) if same_seg(a, b, *c, *d) => Some(t.clone()),
        _ => None,
    })
}

/// Sets the dimension label of side (a, b): None removes it, Some("") shows the measure,
/// Some(text) a typed label. The dim runs so its label sits on the `out` side.
pub fn set_side_label(items: &mut Vec<Item>, a: Pt, b: Pt, out: Pt, label: Option<&str>) {
    let pos = items.iter().position(|it| matches!(&it.shape, Some(Shape::Dim(c, d, _)) if same_seg(a, b, *c, *d)));
    let Some(text) = label else {
        if let Some(p) = pos {
            items.remove(p);
        }
        return;
    };
    // the renderer puts the label on the right of the direction of travel: (uy, -ux) in model units
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (p, q) = if dy * out.0 - dx * out.1 >= 0.0 { (a, b) } else { (b, a) };
    let s = Shape::Dim(p, q, text.trim().to_string());
    match pos {
        Some(i) => items[i].set(s),
        None => items.push(Item::new(s)),
    }
}

/// Ticks (arrows = false) or parallel arrows on side (a, b): 0 = none.
pub fn side_marks(items: &[Item], a: Pt, b: Pt, arrows: bool) -> u8 {
    items
        .iter()
        .find_map(|it| match &it.shape {
            Some(Shape::Ticks(c, d, k)) if !arrows && same_seg(a, b, *c, *d) => Some(*k),
            Some(Shape::Arrows(c, d, k)) if arrows && same_seg(a, b, *c, *d) => Some(*k),
            _ => None,
        })
        .unwrap_or(0)
}

pub fn set_side_marks(items: &mut Vec<Item>, a: Pt, b: Pt, arrows: bool, n: u8) {
    let pos = items.iter().position(|it| match &it.shape {
        Some(Shape::Ticks(c, d, _)) => !arrows && same_seg(a, b, *c, *d),
        Some(Shape::Arrows(c, d, _)) => arrows && same_seg(a, b, *c, *d),
        _ => false,
    });
    if n == 0 {
        if let Some(p) = pos {
            items.remove(p);
        }
        return;
    }
    let s = if arrows { Shape::Arrows(a, b, n.min(3)) } else { Shape::Ticks(a, b, n.min(3)) };
    match pos {
        Some(i) => items[i].set(s),
        None => items.push(Item::new(s)),
    }
}

/// Corners of the outline (ends of the outer sides), without repeats.
pub fn outer_corners(items: &[Item]) -> Vec<Pt> {
    let mut v: Vec<Pt> = vec![];
    for s in outer_sides(items) {
        for q in [s.a, s.b] {
            if !v.iter().any(|w| dist(*w, q) < 1e-6) {
                v.push(q);
            }
        }
    }
    v
}

fn in_any(items: &[Item], q: Pt) -> bool {
    items.iter().filter_map(|it| it.shape.as_ref()).filter(|s| s.is_piece()).any(|s| inside(&outline(s), q))
}

/// The right-angle mark direction for a square corner of the shape at `v` (the one quadrant
/// the shape fills there), if it is one.
pub fn corner_dir(items: &[Item], v: Pt) -> Option<&'static str> {
    let e = 1e-3;
    let quads = [("NE", (e, e)), ("NW", (-e, e)), ("SW", (-e, -e)), ("SE", (e, -e))];
    let ins: Vec<&str> = quads.iter().filter(|(_, d)| in_any(items, (v.0 + d.0, v.1 + d.1))).map(|(n, _)| *n).collect();
    (ins.len() == 1).then(|| ins[0])
}

/// Adds or removes the right-angle mark at corner `v`; returns whether one is there now.
pub fn toggle_ra(items: &mut Vec<Item>, v: Pt) -> bool {
    if let Some(p) = items.iter().position(|it| matches!(&it.shape, Some(Shape::Ra(a, _)) if dist(*a, v) < 1e-4)) {
        items.remove(p);
        return false;
    }
    match corner_dir(items, v) {
        Some(d) => {
            items.push(Item::new(Shape::Ra(v, d.into())));
            true
        }
        None => false,
    }
}

/// Points worth snapping to: piece corners, side midpoints and round-piece centres.
pub fn snap_targets(items: &[Item]) -> Vec<Pt> {
    let mut v = vec![];
    for s in items.iter().filter_map(|it| it.shape.as_ref()).filter(|s| s.is_piece()) {
        for (a, b) in straight_sides(s) {
            v.push(a);
            v.push(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
        }
        if let Shape::Circle { c, .. } | Shape::Semi { c, .. } | Shape::Quarter { c, .. } = s {
            v.push(*c);
        }
    }
    // where pieces meet (an outline corner part way along a side) and the outline's midpoints
    let (c, m) = line_targets(items);
    v.extend(c);
    v.extend(m);
    v
}

// ───────────────────────── add on a side ─────────────────────────

/// The piece the reference adds on an outside side: a rectangle as wide as the side and 3/4 as
/// deep, an isosceles triangle as tall, or a semicircle on it (axis-aligned sides only).
pub fn piece_on_side(kind: NewKind, s: &Side) -> Option<Shape> {
    let (a, b, n) = (s.a, s.b, s.out);
    let l = s.len();
    if l < 1e-9 {
        return None;
    }
    let dep = snap(l * 0.75, 0.5).max(0.5);
    let axis = (a.0 - b.0).abs() < 1e-9 || (a.1 - b.1).abs() < 1e-9;
    let mid = s.mid();
    Some(match kind {
        NewKind::Rect => {
            let (c, d) = ((b.0 + n.0 * dep, b.1 + n.1 * dep), (a.0 + n.0 * dep, a.1 + n.1 * dep));
            if axis {
                let (x0, y0, x1, y1) = bbox(&[a, b, c, d])?;
                Shape::Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
            } else {
                Shape::Poly(vec![a, b, c, d])
            }
        }
        NewKind::Tri => Shape::Tri([a, b, (mid.0 + n.0 * dep, mid.1 + n.1 * dep)]),
        NewKind::Semi | NewKind::Quarter | NewKind::Circle => {
            if !axis {
                return None;
            }
            let dir = if n.1 > 0.5 { "u" } else if n.1 < -0.5 { "d" } else if n.0 > 0.0 { "r" } else { "l" };
            Shape::Semi { c: mid, r: l / 2.0, dir: dir.into() }
        }
    })
}

// ───────────────────────── side lines, angles, corner points, turn ─────────────────────────

/// Line style and thickness of outline side (a, b): ("solid", "regular") unless a `side` row says
/// otherwise.
pub fn side_style(items: &[Item], a: Pt, b: Pt) -> (String, String) {
    items
        .iter()
        .find_map(|it| match &it.shape {
            Some(Shape::SideStyle(c, d, st, th)) if same_seg(a, b, *c, *d) => Some((st.clone(), th.clone())),
            _ => None,
        })
        .unwrap_or_else(|| ("solid".into(), "regular".into()))
}

/// Sets the line of side (a, b); the plain solid regular line removes the row.
pub fn set_side_style(items: &mut Vec<Item>, a: Pt, b: Pt, style: &str, thick: &str) {
    let pos = items.iter().position(|it| matches!(&it.shape, Some(Shape::SideStyle(c, d, ..)) if same_seg(a, b, *c, *d)));
    let style = if LINE_STYLES.contains(&style) { style } else { "solid" };
    let thick = if THICKNESSES.contains(&thick) { thick } else { "regular" };
    if style == "solid" && thick == "regular" {
        if let Some(p) = pos {
            items.remove(p);
        }
        return;
    }
    match pos {
        // an existing row keeps its own direction
        Some(i) => {
            if let Some(Shape::SideStyle(c, d, ..)) = items[i].shape.clone() {
                items[i].set(Shape::SideStyle(c, d, style.into(), thick.into()));
            }
        }
        None => items.push(Item::new(Shape::SideStyle(a, b, style.into(), thick.into()))),
    }
}

/// The two rays of an angle mark (unit vectors, model units, y up): the mark sweeps
/// counter-clockwise from `r1` to `r2` through `deg` degrees. `ext_ray` is the extended side of an
/// exterior angle (drawn as a short dashed line).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngleGeo {
    pub v: Pt,
    pub r1: Pt,
    pub r2: Pt,
    pub deg: f64,
    pub ext_ray: Option<Pt>,
}

fn unit(a: Pt, b: Pt) -> Pt {
    let l = dist(a, b).max(1e-12);
    ((b.0 - a.0) / l, (b.1 - a.1) / l)
}
fn ang_of(u: Pt) -> f64 {
    u.1.atan2(u.0).to_degrees()
}

/// The interior angle of the outline at corner `v` (as its two rays, swept counter-clockwise),
/// or the exterior angle (`ext`): between the side arriving at `v` (walking the outline clockwise)
/// carried on past it, and the side leaving it. None when `v` is not a corner where exactly two
/// outline sides meet, or the sides run straight on.
pub fn angle_geo(items: &[Item], v: Pt, ext: bool) -> Option<AngleGeo> {
    let sides: Vec<Side> = outer_sides(items).into_iter().filter(|s| dist(s.a, v) < 1e-6 || dist(s.b, v) < 1e-6).collect();
    if sides.len() != 2 {
        return None;
    }
    let far = |s: &Side| if dist(s.a, v) < 1e-6 { s.b } else { s.a };
    // the incoming side has the shape on the right of the direction of travel
    let incoming = |s: &Side| {
        let e = unit(v, far(s));
        let t = (-e.0, -e.1);
        t.1 * s.out.0 - t.0 * s.out.1 < 0.0
    };
    let (sin, sout) = match (incoming(&sides[0]), incoming(&sides[1])) {
        (true, false) => (sides[0], sides[1]),
        (false, true) => (sides[1], sides[0]),
        _ => return None,
    };
    let (e_in, e_out) = (unit(v, far(&sin)), unit(v, far(&sout)));
    let interior = (ang_of(e_out) - ang_of(e_in)).rem_euclid(360.0);
    if (interior - 180.0).abs() < 1e-6 || interior < 1e-6 {
        return None;
    }
    if !ext {
        return Some(AngleGeo { v, r1: e_in, r2: e_out, deg: interior, ext_ray: None });
    }
    let t = (-e_in.0, -e_in.1);
    Some(if interior < 180.0 {
        AngleGeo { v, r1: e_out, r2: t, deg: 180.0 - interior, ext_ray: Some(t) }
    } else {
        AngleGeo { v, r1: t, r2: e_out, deg: interior - 180.0, ext_ray: Some(t) }
    })
}

/// Whether direction `u` lies inside the counter-clockwise sweep of `g`.
pub fn in_sweep(g: &AngleGeo, u: Pt) -> bool {
    (ang_of(u) - ang_of(g.r1)).rem_euclid(360.0) <= g.deg
}

/// The settings of the angle mark at corner `v` (interior or exterior).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AngleMark {
    pub measure: bool,
    /// Typed label (shown when not `measure`).
    pub text: String,
    pub arcs: u8,
    pub square: bool,
    pub shade: Option<String>,
}

impl AngleMark {
    pub fn is_empty(&self) -> bool {
        !self.measure && self.text.trim().is_empty() && self.arcs == 0 && !self.square && self.shade.is_none()
    }
}

fn angle_pos(items: &[Item], v: Pt, ext: bool) -> Option<usize> {
    items.iter().position(|it| matches!(&it.shape, Some(Shape::Angle { v: w, ext: e, .. }) if *e == ext && dist(*w, v) < 1e-4))
}

pub fn angle_mark(items: &[Item], v: Pt, ext: bool) -> AngleMark {
    let Some(i) = angle_pos(items, v, ext) else { return AngleMark::default() };
    let it = &items[i];
    match &it.shape {
        Some(Shape::Angle { measure, arcs, square, .. }) => AngleMark { measure: *measure, text: it.label.clone(), arcs: *arcs, square: *square, shade: it.shade.clone() },
        _ => AngleMark::default(),
    }
}

/// Writes the angle mark at corner `v`; an empty mark removes the row.
pub fn set_angle_mark(items: &mut Vec<Item>, v: Pt, ext: bool, m: &AngleMark) {
    let pos = angle_pos(items, v, ext);
    if m.is_empty() {
        if let Some(p) = pos {
            items.remove(p);
        }
        return;
    }
    let s = Shape::Angle { v, ext, measure: m.measure, arcs: m.arcs.min(3), square: m.square };
    let i = match pos {
        Some(i) => i,
        None => {
            items.push(Item::new(s.clone()));
            items.len() - 1
        }
    };
    let it = &mut items[i];
    it.set(s);
    it.set_shade(m.shade.as_deref());
    it.set_label(if m.measure { "" } else { &m.text });
}

/// Whether the interior corner at `v` carries a right-angle mark (an `ra` row or a squared angle).
pub fn has_right_mark(items: &[Item], v: Pt) -> bool {
    items.iter().any(|it| matches!(&it.shape, Some(Shape::Ra(a, _)) if dist(*a, v) < 1e-4)) || angle_mark(items, v, false).square
}

/// Puts the interior right-angle mark at `v` on or off: an `ra` row where the corner is square to
/// the grid (as the presets have), else the angle row's `square`.
pub fn set_right_mark(items: &mut Vec<Item>, v: Pt, on: bool) {
    if has_right_mark(items, v) == on {
        return;
    }
    let ra = items.iter().position(|it| matches!(&it.shape, Some(Shape::Ra(a, _)) if dist(*a, v) < 1e-4));
    if !on {
        if let Some(p) = ra {
            items.remove(p);
        }
        let mut m = angle_mark(items, v, false);
        if m.square {
            m.square = false;
            set_angle_mark(items, v, false, &m);
        }
        return;
    }
    if corner_dir(items, v).is_some() {
        toggle_ra(items, v);
    } else {
        let mut m = angle_mark(items, v, false);
        m.square = true;
        set_angle_mark(items, v, false, &m);
    }
}

/// The corner point at `v`: None = no point, Some(name) (possibly empty) = a point is shown.
pub fn corner_point(items: &[Item], v: Pt) -> Option<String> {
    items.iter().find_map(|it| match &it.shape {
        Some(Shape::Point(a, n)) if dist(*a, v) < 1e-4 => Some(n.clone()),
        _ => None,
    })
}

pub fn set_corner_point(items: &mut Vec<Item>, v: Pt, name: Option<&str>) {
    let pos = items.iter().position(|it| matches!(&it.shape, Some(Shape::Point(a, _)) if dist(*a, v) < 1e-4));
    match (name, pos) {
        (None, Some(p)) => {
            items.remove(p);
        }
        (None, None) => {}
        (Some(n), Some(p)) => items[p].set(Shape::Point(v, n.trim().to_string())),
        (Some(n), None) => items.push(Item::new(Shape::Point(v, n.trim().to_string()))),
    }
}

/// Where a corner's point name goes: the unit direction (model units) away from the shape,
/// opposite the interior angle's bisector.
pub fn outside_dir(items: &[Item], v: Pt) -> Pt {
    if let Some(g) = angle_geo(items, v, false) {
        let b = (ang_of(g.r1) + g.deg / 2.0 + 180.0).to_radians();
        return (b.cos(), b.sin());
    }
    let n = outer_sides(items).into_iter().filter(|s| dist(s.a, v) < 1e-6 || dist(s.b, v) < 1e-6).fold((0.0, 0.0), |acc, s| (acc.0 + s.out.0, acc.1 + s.out.1));
    let l = (n.0 * n.0 + n.1 * n.1).sqrt();
    if l > 1e-9 { (n.0 / l, n.1 / l) } else { (-std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2) }
}

/// Centre the figure turns about: the middle of the pieces' bounding box (of every point when
/// there are no pieces).
pub fn turn_centre(items: &[Item]) -> Pt {
    let pts: Vec<Pt> = items.iter().filter_map(|it| it.shape.as_ref()).filter(|s| s.is_piece()).flat_map(outline).collect();
    let pts = if pts.is_empty() { all_points(items) } else { pts };
    bbox(&pts).map(|(x0, y0, x1, y1)| ((x0 + x1) / 2.0, (y0 + y1) / 2.0)).unwrap_or((0.0, 0.0))
}

/// `q` turned counter-clockwise by `deg` about `c` (model units, y up).
pub fn rot(q: Pt, c: Pt, deg: f64) -> Pt {
    if deg == 0.0 || !deg.is_finite() {
        return q;
    }
    let (s, k) = deg.to_radians().sin_cos();
    let (x, y) = (q.0 - c.0, q.1 - c.1);
    (c.0 + x * k - y * s, c.1 + x * s + y * k)
}

/// Lines mode snap markers: corners and meeting points (circles) and side midpoints (diamonds).
pub fn line_targets(items: &[Item]) -> (Vec<Pt>, Vec<Pt>) {
    let mut corners: Vec<Pt> = vec![];
    let push = |v: &mut Vec<Pt>, q: Pt| {
        if !v.iter().any(|w| dist(*w, q) < 1e-6) {
            v.push(q);
        }
    };
    for q in outer_corners(items) {
        push(&mut corners, q);
    }
    for s in items.iter().filter_map(|it| it.shape.as_ref()).filter(|s| s.is_piece()) {
        for (a, _) in straight_sides(s) {
            push(&mut corners, a);
        }
        if let Shape::Circle { c, .. } | Shape::Semi { c, .. } | Shape::Quarter { c, .. } = s {
            push(&mut corners, *c);
        }
    }
    let mut mids: Vec<Pt> = vec![];
    for s in outer_sides(items) {
        push(&mut mids, s.mid());
    }
    mids.retain(|m| !corners.iter().any(|c| dist(*c, *m) < 1e-6));
    (corners, mids)
}

// ───────────────────────── tests ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const L: &str = "rect 0 0 8 3; rect 0 3 3 5; dim 0 0 8 0 8 cm; dim 8 0 8 3 3 cm; dim 8 3 3 3 5 cm; dim 3 3 3 8 5 cm; dim 3 8 0 8 3 cm; dim 0 8 0 0 8 cm; ra 0 0 NE; ra 8 0 NW; ra 0 8 SE";

    fn close(a: Pt, b: Pt) -> bool {
        dist(a, b) < 1e-6
    }

    #[test]
    fn untouched_rows_round_trip_verbatim() {
        for spec in [L, "rect 0 0 6 4; semi 3 4 3 u; dash 3 4 6 4; text 4.5 4.6 r = 3", "rect 0 0 6 4 blue; bogus row 1 2; poly 0 0 1 0 1 1 0 1 green | A", "text 1 1 |x| = 3"] {
            let items = parse(spec);
            assert_eq!(to_spec(&items), spec.split(';').map(|s| s.trim()).collect::<Vec<_>>().join("; "));
        }
        // new lines also separate rows
        assert_eq!(parse("rect 0 0 1 1\ncircle 0 0 1").len(), 2);
    }

    #[test]
    fn rows_parse_like_the_renderer() {
        let it = parse("rect 0 0 6 4 Red | Area A; semi 1 2 3; quarter 0 0 2 SW; ticks 0 0 1 0 9; ra 1 1; text 1 1 |x|; poly 0 0 4 0 4 3 yellow | 24");
        assert_eq!(it[0].shape, Some(Shape::Rect { x: 0.0, y: 0.0, w: 6.0, h: 4.0 }));
        assert_eq!((it[0].shade.as_deref(), it[0].label.as_str()), (Some("red"), "Area A"));
        assert_eq!(it[1].shape, Some(Shape::Semi { c: (1.0, 2.0), r: 3.0, dir: "u".into() }));
        assert_eq!(it[2].shape, Some(Shape::Quarter { c: (0.0, 0.0), r: 2.0, dir: "sw".into() }));
        assert_eq!(it[3].shape, Some(Shape::Ticks((0.0, 0.0), (1.0, 0.0), 3)));
        assert_eq!(it[4].shape, Some(Shape::Ra((1.0, 1.0), "NE".into())));
        assert_eq!(it[5].shape, Some(Shape::Text((1.0, 1.0), "|x|".into())));
        // a numeric label is not read as more polygon points
        assert_eq!(it[6].shape, Some(Shape::Poly(vec![(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)])));
        assert_eq!(it[6].label, "24");
        assert!(parse("rect 1 2").first().unwrap().shape.is_none());
    }

    #[test]
    fn edited_rows_are_rewritten_with_shade_and_label() {
        let mut it = parse("rect 0 0 6 4");
        it[0].set_shade(Some("Green"));
        it[0].set_label("A = 24");
        assert_eq!(to_spec(&it), "rect 0 0 6 4 green | A = 24");
        let back = parse(&to_spec(&it));
        assert_eq!(back[0].shade.as_deref(), Some("green"));
        assert_eq!(back[0].label, "A = 24");
        it[0].set_shade(None);
        it[0].set_label("");
        assert_eq!(to_spec(&it), "rect 0 0 6 4");
        let mut m = vec![Item::new(Shape::Dim((0.0, 0.0), (2.5, 0.0), String::new()))];
        assert_eq!(to_spec(&m), "dim 0 0 2.5 0");
        m[0].set(Shape::Text((0.333333333, -0.0), "hi".into()));
        assert_eq!(to_spec(&m), "text 0.3333 0 hi");
    }

    #[test]
    fn move_takes_attached_marks_along() {
        let mut it = parse(L);
        // the upright rectangle carries the dims on its own sides, not the base's
        move_item(&mut it, 1, (1.0, 0.0));
        let s = to_spec(&it);
        assert!(s.starts_with("rect 0 0 8 3; rect 1 3 3 5;"), "{s}");
        assert!(s.contains("dim 4 3 4 8 5 cm") && s.contains("dim 4 8 1 8 3 cm") && s.contains("ra 1 8 SE"), "{s}");
        assert!(s.contains("dim 0 0 8 0 8 cm") && s.contains("ra 0 0 NE"), "{s}");
        // a text inside a semicircle and the radius line move with it
        let mut a = parse("rect 0 0 6 4; semi 3 4 3 u; dash 3 4 6 4; text 4.5 4.6 r = 3");
        move_item(&mut a, 1, (0.0, 1.0));
        assert_eq!(to_spec(&a), "rect 0 0 6 4; semi 3 5 3 u; dash 3 5 6 5; text 4.5 5.6 r = 3");
    }

    #[test]
    fn duplicate_and_delete() {
        let mut it = parse(L);
        let n = it.len();
        let j = duplicate(&mut it, 0, (1.0, -1.0)).unwrap();
        assert_eq!((j, it.len()), (n, n + 1));
        assert_eq!(it[j].row(), "rect 1 -1 8 3");
        it.truncate(n);
        // deleting the upright rectangle drops the marks only it owns
        delete(&mut it, 1);
        let s = to_spec(&it);
        assert!(!s.contains("rect 0 3 3 5") && !s.contains("dim 3 3 3 8") && !s.contains("ra 0 8"), "{s}");
        assert!(s.contains("dim 0 0 8 0 8 cm") && s.contains("dim 8 3 3 3 5 cm"), "{s}");
        delete(&mut it, 99);
    }

    #[test]
    fn rect_grips_resize_and_linked_marks_follow() {
        let mut s = Shape::Rect { x: 0.0, y: 0.0, w: 4.0, h: 3.0 };
        drag_grip(&mut s, Grip::Corner(2), (6.0, 5.0));
        assert_eq!(s, Shape::Rect { x: 0.0, y: 0.0, w: 6.0, h: 5.0 });
        drag_grip(&mut s, Grip::Corner(0), (8.0, 7.0)); // past the opposite corner: flips
        assert_eq!(s, Shape::Rect { x: 6.0, y: 5.0, w: 2.0, h: 2.0 });
        let mut s = Shape::Rect { x: 0.0, y: 0.0, w: 4.0, h: 3.0 };
        drag_grip(&mut s, Grip::Side(1), (7.0, 99.0));
        drag_grip(&mut s, Grip::Side(3), (-1.0, 0.0));
        drag_grip(&mut s, Grip::Side(0), (0.0, 9.0)); // clamps at the top
        assert_eq!(Item::new(s).row(), "rect -1 2.9 8 0.1");
        let mut it = parse("rect 0 0 8 5; dim 0 0 8 0; dim 8 0 8 5; ra 8 5 SW");
        drag_grip_linked(&mut it, 0, Grip::Side(1), (10.0, 2.0));
        assert_eq!(to_spec(&it), "rect 0 0 10 5; dim 0 0 10 0; dim 10 0 10 5; ra 10 5 SW");
        let mut c = Shape::Semi { c: (0.0, 0.0), r: 2.0, dir: "u".into() };
        assert!(close(grips(&c)[0].1, (0.0, 2.0)));
        drag_grip(&mut c, Grip::Radius, (3.0, 4.0));
        assert_eq!(c, Shape::Semi { c: (0.0, 0.0), r: 5.0, dir: "u".into() });
        let mut t = Shape::Tri([(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)]);
        drag_grip(&mut t, Grip::Vertex(2), (1.0, 5.0));
        assert_eq!(t, Shape::Tri([(0.0, 0.0), (4.0, 0.0), (1.0, 5.0)]));
    }

    #[test]
    fn new_pieces_from_drags_and_clicks() {
        assert_eq!(piece_from_drag(NewKind::Rect, (4.0, 3.0), (1.0, 1.0)), Some(Shape::Rect { x: 1.0, y: 1.0, w: 3.0, h: 2.0 }));
        assert_eq!(piece_from_drag(NewKind::Rect, (0.0, 0.0), (5.0, 0.0)), None);
        assert_eq!(piece_from_drag(NewKind::Tri, (0.0, 0.0), (4.0, 3.0)), Some(Shape::Tri([(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)])));
        assert_eq!(piece_from_drag(NewKind::Semi, (0.0, 0.0), (-3.0, 1.0)), Some(Shape::Semi { c: (0.0, 0.0), r: 3.0, dir: "l".into() }));
        assert_eq!(piece_from_drag(NewKind::Semi, (0.0, 0.0), (1.0, -2.0)), Some(Shape::Semi { c: (0.0, 0.0), r: 2.0, dir: "d".into() }));
        assert_eq!(piece_from_drag(NewKind::Quarter, (0.0, 0.0), (-2.0, 3.0)), Some(Shape::Quarter { c: (0.0, 0.0), r: 3.0, dir: "nw".into() }));
        assert_eq!(piece_from_drag(NewKind::Circle, (0.0, 0.0), (3.0, 4.0)), Some(Shape::Circle { c: (0.0, 0.0), r: 5.0 }));
        assert_eq!(piece_from_drag(NewKind::Circle, (0.0, 0.0), (0.0, 0.0)), None);
        for k in [NewKind::Rect, NewKind::Tri, NewKind::Semi, NewKind::Quarter, NewKind::Circle] {
            let s = default_piece(k, (1.0, 1.0));
            assert!(s.is_piece() && outline(&s).len() >= 3);
            let row = Item::new(s.clone()).row();
            assert_eq!(parse(&row)[0].shape, Some(s), "{row}");
        }
        assert_eq!(poly_from(&[(0.0, 0.0), (0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 0.0)]), Some(Shape::Tri([(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)])));
        assert!(matches!(poly_from(&[(0.0, 0.0), (4.0, 0.0), (5.0, 2.0), (1.0, 3.0)]), Some(Shape::Poly(v)) if v.len() == 4));
        assert_eq!(poly_from(&[(0.0, 0.0), (1.0, 1.0)]), None);
    }

    #[test]
    fn hit_testing_prefers_marks_then_top_piece() {
        let it = parse("rect 0 0 6 4; rect 2 1 2 2; dim 0 0 6 0");
        assert_eq!(hit(&it, (3.0, 2.0), 0.1), Some(1));
        assert_eq!(hit(&it, (1.0, 2.0), 0.1), Some(0));
        assert_eq!(hit(&it, (3.0, 0.05), 0.1), Some(2));
        assert_eq!(hit(&it, (9.0, 9.0), 0.1), None);
        let c = parse("circle 0 0 2");
        assert_eq!(hit(&c, (1.0, 1.0), 0.1), Some(0));
        assert_eq!(hit(&c, (1.9, 1.9), 0.1), None);
    }

    #[test]
    fn outer_sides_split_shared_stretches() {
        let it = parse(L);
        let s = outer_sides(&it);
        // 7 sides: the left edge is two pieces' sides meeting end to end
        assert_eq!(s.len(), 7, "{s:?}");
        let per: f64 = s.iter().map(|x| x.len()).sum();
        assert!((per - 32.0).abs() < 1e-9);
        // the base's top side keeps only the stretch right of the upright rectangle
        let top = s.iter().find(|x| x.piece == 0 && (x.a.1 - 3.0).abs() < 1e-9).unwrap();
        assert!((top.len() - 5.0).abs() < 1e-9 && top.out == (0.0, 1.0));
        // every outward normal points away from the shape
        for x in &s {
            let m = x.mid();
            assert!(!in_any(&it, (m.0 + x.out.0 * 0.01, m.1 + x.out.1 * 0.01)), "{x:?}");
        }
        // arch: the semicircle's diameter sits on the rectangle's top, so neither counts
        let arch = parse("rect 0 0 6 4; semi 3 4 3 u");
        assert_eq!(outer_sides(&arch).len(), 3);
        assert_eq!(outer_corners(&arch).len(), 4);
    }

    #[test]
    fn side_labels_and_marks_toggle() {
        let mut it = parse("rect 0 0 8 3");
        let s = outer_sides(&it);
        let bottom = *s.iter().find(|x| x.out == (0.0, -1.0)).unwrap();
        let right = *s.iter().find(|x| x.out == (1.0, 0.0)).unwrap();
        assert_eq!(side_label(&it, bottom.a, bottom.b), None);
        set_side_label(&mut it, bottom.b, bottom.a, bottom.out, Some(""));
        set_side_label(&mut it, right.a, right.b, right.out, Some("h"));
        // dims run so the label is outside: bottom left to right, right side bottom to top
        assert_eq!(to_spec(&it), "rect 0 0 8 3; dim 0 0 8 0; dim 8 0 8 3 h");
        assert_eq!(side_label(&it, bottom.a, bottom.b), Some(String::new()));
        set_side_label(&mut it, right.a, right.b, right.out, Some("3 cm"));
        assert_eq!(side_label(&it, right.b, right.a).as_deref(), Some("3 cm"));
        set_side_label(&mut it, bottom.a, bottom.b, bottom.out, None);
        assert_eq!(to_spec(&it), "rect 0 0 8 3; dim 8 0 8 3 3 cm");
        set_side_marks(&mut it, bottom.a, bottom.b, false, 2);
        set_side_marks(&mut it, bottom.a, bottom.b, true, 1);
        assert_eq!((side_marks(&it, bottom.a, bottom.b, false), side_marks(&it, bottom.a, bottom.b, true)), (2, 1));
        set_side_marks(&mut it, bottom.a, bottom.b, false, 0);
        assert_eq!(side_marks(&it, bottom.a, bottom.b, false), 0);
        assert_eq!(it.len(), 3);
    }

    #[test]
    fn right_angle_corners() {
        let mut it = parse(L);
        it.retain(|i| !matches!(i.shape, Some(Shape::Ra(..))));
        assert_eq!(corner_dir(&it, (0.0, 0.0)), Some("NE"));
        assert_eq!(corner_dir(&it, (8.0, 3.0)), Some("SW"));
        assert_eq!(corner_dir(&it, (3.0, 3.0)), None); // the inside corner
        assert!(toggle_ra(&mut it, (3.0, 8.0)));
        assert!(to_spec(&it).ends_with("ra 3 8 SW"));
        assert!(!toggle_ra(&mut it, (3.0, 8.0)));
        assert!(!to_spec(&it).contains("ra "));
    }

    #[test]
    fn pieces_added_on_sides() {
        let it = parse("rect 0 0 6 4");
        let s = outer_sides(&it);
        let top = s.iter().find(|x| x.out == (0.0, 1.0)).unwrap();
        assert_eq!(piece_on_side(NewKind::Rect, top), Some(Shape::Rect { x: 0.0, y: 4.0, w: 6.0, h: 4.5 }));
        assert_eq!(piece_on_side(NewKind::Tri, top), Some(Shape::Tri([top.a, top.b, (3.0, 8.5)])));
        assert_eq!(piece_on_side(NewKind::Semi, top), Some(Shape::Semi { c: (3.0, 4.0), r: 3.0, dir: "u".into() }));
        let right = s.iter().find(|x| x.out == (1.0, 0.0)).unwrap();
        assert_eq!(piece_on_side(NewKind::Semi, right), Some(Shape::Semi { c: (6.0, 2.0), r: 2.0, dir: "r".into() }));
        let tri = parse("tri 0 0 4 0 0 4");
        let hyp = outer_sides(&tri).into_iter().find(|x| x.out.0 > 0.1 && x.out.1 > 0.1).unwrap();
        assert!(matches!(piece_on_side(NewKind::Rect, &hyp), Some(Shape::Poly(v)) if v.len() == 4));
        assert_eq!(piece_on_side(NewKind::Semi, &hyp), None);
    }

    #[test]
    fn centroid_and_snap() {
        assert!(close(centroid(&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)]), (2.0, 1.0)));
        assert!(close(centroid(&[(0.0, 0.0), (1.0, 1.0)]), (0.5, 0.5)));
        assert_eq!(snap_pt((1.26, -0.74), 0.5), (1.5, -0.5));
        assert_eq!(snap(1.26, 0.0), 1.26);
        assert_eq!(fmt_n(-0.00001), "0");
        assert!(snap_targets(&parse("rect 0 0 2 2; circle 5 5 1")).contains(&(5.0, 5.0)));
    }

    const HOUSE: &str = "rect 0 0 6 4; tri 0 4 6 4 3 7";

    #[test]
    fn side_rows_parse_write_and_toggle() {
        let it = parse("side 6 0 6 4 Dashed THICK; side 0 0 6 0 dotted; side 0 4 0 0 hidden thin; side 1 1 2 2 wobbly");
        assert_eq!(it[0].shape, Some(Shape::SideStyle((6.0, 0.0), (6.0, 4.0), "dashed".into(), "thick".into())));
        assert_eq!(it[1].shape, Some(Shape::SideStyle((0.0, 0.0), (6.0, 0.0), "dotted".into(), "regular".into())));
        assert_eq!(it[2].shape, Some(Shape::SideStyle((0.0, 4.0), (0.0, 0.0), "hidden".into(), "thin".into())));
        // unknown words fall back to the plain line
        assert_eq!(it[3].shape, Some(Shape::SideStyle((1.0, 1.0), (2.0, 2.0), "solid".into(), "regular".into())));
        assert!(parse("side 1 2 3").first().unwrap().shape.is_none());
        let mut it = parse(HOUSE);
        let right = *outer_sides(&it).iter().find(|s| s.out == (1.0, 0.0)).unwrap();
        assert_eq!(side_style(&it, right.a, right.b), ("solid".to_string(), "regular".to_string()));
        set_side_style(&mut it, right.b, right.a, "dashed", "thick");
        assert_eq!(to_spec(&it), format!("{HOUSE}; side 6 4 6 0 dashed thick"));
        assert_eq!(side_style(&it, right.a, right.b), ("dashed".to_string(), "thick".to_string()));
        set_side_style(&mut it, right.a, right.b, "solid", "thin");
        assert_eq!(to_spec(&it), format!("{HOUSE}; side 6 4 6 0 solid thin"));
        let back = parse(&to_spec(&it));
        assert_eq!(side_style(&back, right.a, right.b), ("solid".to_string(), "thin".to_string()));
        set_side_style(&mut it, right.a, right.b, "solid", "regular");
        assert_eq!(to_spec(&it), HOUSE, "the plain line drops the row");
        // the row follows the corner it sits on and the piece it belongs to
        let mut it = parse("rect 0 0 6 4; side 6 0 6 4 dotted");
        drag_grip_linked(&mut it, 0, Grip::Side(1), (8.0, 2.0));
        assert_eq!(to_spec(&it), "rect 0 0 8 4; side 8 0 8 4 dotted");
        move_item(&mut it, 0, (1.0, 1.0));
        assert_eq!(to_spec(&it), "rect 1 1 8 4; side 9 1 9 5 dotted");
        // not selectable on the canvas (the Label tool edits it), gone with its piece
        assert_eq!(hit(&it, (9.0, 3.0), 0.1), Some(0));
        delete(&mut it, 0);
        assert!(it.is_empty());
    }

    #[test]
    fn angle_rows_parse_write_and_round_trip() {
        let it = parse("angle 0 4 ext measure arcs 2 square red | x; angle 3 7; angle 1 1 Arcs 9 GREEN | 2x + 1");
        assert_eq!(it[0].shape, Some(Shape::Angle { v: (0.0, 4.0), ext: true, measure: true, arcs: 2, square: true }));
        assert_eq!((it[0].shade.as_deref(), it[0].label.as_str()), (Some("red"), "x"));
        assert_eq!(it[1].shape, Some(Shape::Angle { v: (3.0, 7.0), ext: false, measure: false, arcs: 0, square: false }));
        assert_eq!(it[2].shape, Some(Shape::Angle { v: (1.0, 1.0), ext: false, measure: false, arcs: 3, square: false }));
        assert_eq!((it[2].shade.as_deref(), it[2].label.as_str()), (Some("green"), "2x + 1"));
        // untouched rows round-trip byte for byte; edited ones are written canonically
        let spec = "angle 0 4 ext measure arcs 2 square red | x; angle 3 7";
        assert_eq!(to_spec(&parse(spec)), spec);
        let mut it = parse(HOUSE);
        let m = AngleMark { measure: false, text: "y".into(), arcs: 1, square: false, shade: Some("blue".into()) };
        set_angle_mark(&mut it, (0.0, 4.0), false, &m);
        assert_eq!(to_spec(&it), format!("{HOUSE}; angle 0 4 arcs 1 blue | y"));
        assert_eq!(angle_mark(&parse(&to_spec(&it)), (0.0, 4.0), false), m);
        let ext = AngleMark { measure: true, ..Default::default() };
        set_angle_mark(&mut it, (0.0, 4.0), true, &ext);
        assert!(to_spec(&it).ends_with("; angle 0 4 ext measure"), "{}", to_spec(&it));
        // measure wins over a typed label; an empty mark removes the row
        set_angle_mark(&mut it, (0.0, 4.0), false, &AngleMark { measure: true, text: "y".into(), ..Default::default() });
        assert!(to_spec(&it).contains("angle 0 4 measure;") || to_spec(&it).contains("angle 0 4 measure"), "{}", to_spec(&it));
        assert_eq!(angle_mark(&it, (0.0, 4.0), false).text, "");
        set_angle_mark(&mut it, (0.0, 4.0), false, &AngleMark::default());
        set_angle_mark(&mut it, (0.0, 4.0), true, &AngleMark::default());
        assert_eq!(to_spec(&it), HOUSE);
        // marks on the corner move with the piece
        let mut it = parse("rect 0 0 6 4; angle 6 4 arcs 1; point 6 4 C");
        move_item(&mut it, 0, (0.0, -1.0));
        assert_eq!(to_spec(&it), "rect 0 -1 6 4; angle 6 3 arcs 1; point 6 3 C");
    }

    #[test]
    fn angles_at_corners_inside_and_out() {
        let it = parse(HOUSE);
        let deg = |v: Pt, ext: bool| angle_geo(&it, v, ext).map(|g| (g.deg * 1e6).round() / 1e6);
        assert_eq!(deg((0.0, 0.0), false), Some(90.0));
        assert_eq!(deg((0.0, 4.0), false), Some(135.0));
        assert_eq!(deg((0.0, 4.0), true), Some(45.0));
        assert_eq!(deg((3.0, 7.0), false), Some(90.0));
        assert_eq!(deg((6.0, 0.0), true), Some(90.0));
        assert_eq!(deg((9.0, 9.0), false), None);
        // the interior wedge holds the shape; the exterior one lies outside it, along the
        // extended side
        for v in outer_corners(&it) {
            let g = angle_geo(&it, v, false).unwrap();
            let b = ((g.r1.1.atan2(g.r1.0)).to_degrees() + g.deg / 2.0).to_radians();
            assert!(in_any(&it, (v.0 + b.cos() * 0.05, v.1 + b.sin() * 0.05)), "{v:?}");
            let e = angle_geo(&it, v, true).unwrap();
            let b = ((e.r1.1.atan2(e.r1.0)).to_degrees() + e.deg / 2.0).to_radians();
            assert!(!in_any(&it, (v.0 + b.cos() * 0.05, v.1 + b.sin() * 0.05)), "{v:?}");
            assert!(e.ext_ray.is_some() && (e.deg + g.deg - 180.0).abs() < 1e-9);
            assert!(in_sweep(&g, (b.cos(), b.sin())) == false);
        }
        // the L-shape's inside corner is reflex; where two sides run straight on there is no angle
        let l = parse("rect 0 0 8 3; rect 0 3 3 5");
        assert!((angle_geo(&l, (3.0, 3.0), false).unwrap().deg - 270.0).abs() < 1e-9);
        assert!((angle_geo(&l, (3.0, 3.0), true).unwrap().deg - 90.0).abs() < 1e-9);
        assert_eq!(angle_geo(&l, (0.0, 3.0), false), None);
    }

    #[test]
    fn right_angle_marks_from_the_angle_popover() {
        // a corner square to the grid uses the presets' ra row
        let mut it = parse("rect 0 0 6 4");
        assert!(!has_right_mark(&it, (6.0, 0.0)));
        set_right_mark(&mut it, (6.0, 0.0), true);
        assert_eq!(to_spec(&it), "rect 0 0 6 4; ra 6 0 NW");
        assert!(has_right_mark(&it, (6.0, 0.0)));
        set_right_mark(&mut it, (6.0, 0.0), false);
        assert_eq!(to_spec(&it), "rect 0 0 6 4");
        // a slanted square corner uses the angle row's square
        let mut d = parse("poly 0 0 2 2 0 4 -2 2");
        set_right_mark(&mut d, (2.0, 2.0), true);
        assert_eq!(to_spec(&d), "poly 0 0 2 2 0 4 -2 2; angle 2 2 square");
        assert!(has_right_mark(&d, (2.0, 2.0)));
        set_right_mark(&mut d, (2.0, 2.0), false);
        assert_eq!(to_spec(&d), "poly 0 0 2 2 0 4 -2 2");
    }

    #[test]
    fn corner_points_and_their_names() {
        let mut it = parse(HOUSE);
        assert_eq!(corner_point(&it, (0.0, 0.0)), None);
        set_corner_point(&mut it, (0.0, 0.0), Some(""));
        assert_eq!(to_spec(&it), format!("{HOUSE}; point 0 0"));
        set_corner_point(&mut it, (0.0, 0.0), Some(" A "));
        assert_eq!(to_spec(&it), format!("{HOUSE}; point 0 0 A"));
        assert_eq!(corner_point(&parse(&to_spec(&it)), (0.0, 0.0)).as_deref(), Some("A"));
        assert_eq!(parse("point 1 2 P Q")[0].shape, Some(Shape::Point((1.0, 2.0), "P Q".into())));
        set_corner_point(&mut it, (0.0, 0.0), None);
        assert_eq!(to_spec(&it), HOUSE);
        // names go away from the shape: down-left at the bottom-left corner, up at the apex
        let d = outside_dir(&it, (0.0, 0.0));
        assert!(d.0 < -0.5 && d.1 < -0.5, "{d:?}");
        let d = outside_dir(&it, (3.0, 7.0));
        assert!(d.1 > 0.9, "{d:?}");
    }

    #[test]
    fn turning_about_the_middle_of_the_pieces() {
        let it = parse("rect 0 0 4 2; dim 0 0 4 0");
        assert_eq!(turn_centre(&it), (2.0, 1.0));
        assert!(close(rot((4.0, 0.0), (2.0, 1.0), 90.0), (3.0, 3.0)));
        assert!(close(rot(rot((1.3, -2.0), (2.0, 1.0), 37.0), (2.0, 1.0), -37.0), (1.3, -2.0)));
        assert_eq!(rot((1.0, 1.0), (0.0, 0.0), 0.0), (1.0, 1.0));
        assert_eq!(turn_centre(&[]), (0.0, 0.0));
    }

    #[test]
    fn lines_mode_markers() {
        let (c, m) = line_targets(&parse(HOUSE));
        assert_eq!((c.len(), m.len()), (5, 5), "{c:?} {m:?}");
        assert!(m.contains(&(1.5, 5.5)) && m.contains(&(3.0, 0.0)));
        // the L-shape's meeting point, where the upright meets the base's top side
        let (c, _) = line_targets(&parse("rect 0 0 8 3; rect 0 3 3 5"));
        assert!(c.iter().any(|q| close(*q, (3.0, 3.0))));
        assert!(snap_targets(&parse(HOUSE)).contains(&(1.5, 5.5)));
        // curved runs: the semicircle's arc, not its diameter
        let arch = parse("semi 3 4 3 u");
        let runs = curved_edges(arch[0].shape.as_ref().unwrap());
        assert_eq!(runs.len(), 1);
        assert!(close(runs[0][0], (6.0, 4.0)) && close(*runs[0].last().unwrap(), (0.0, 4.0)));
        assert!(curved_edges(&Shape::Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }).is_empty());
    }
}
