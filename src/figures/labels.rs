//! Draggable figure labels (the reference's "Drag to move this label").
//!
//! A renderer that supports it wraps each movable label in [`wrap`], which emits
//! `<g data-lbl="<id>" data-box="cx cy hw hh" transform="translate(dx dy)">…</g>`: the label's
//! stable id, its unmoved box in SVG units, and the user's offset. Offsets live in ONE text param,
//! [`KEY`] (`"id:dx,dy;id:dx,dy"` in SVG units), so saved figures, presets, copied specs, undo and
//! every output path (preview, PNG/SVG export, attach -> PDF/DOCX) honour them through the
//! figure's own `render`. Specs saved before the param existed simply have no offsets.
//!
//! The interaction layer never guesses where labels are: [`boxes`] parses the rendered SVG, so
//! hit-testing always matches what was drawn.
use std::cell::RefCell;
use std::collections::BTreeMap;

use super::interact::{Cursor, Handle, Shape};
use super::{ParamDef, Params};

/// Param holding the label offsets.
pub const KEY: &str = "label_offsets";

/// Figures whose labels can be dragged (the 11 reference geometry pages that offer it).
pub const FIGURES: [&str; 11] = [
    "triangle", "rectangle", "parallelogram", "trapezoid", "kite", "regular-polygon",
    "prism", "pyramid", "cylinder", "cone", "sphere",
];

/// Figures with the reference's Turn slider (+ Straighten): the flat shapes.
pub const TURNABLE: [&str; 6] = ["triangle", "rectangle", "parallelogram", "trapezoid", "kite", "regular-polygon"];

pub fn draggable(id: &str) -> bool {
    FIGURES.contains(&id)
}

pub fn turnable(id: &str) -> bool {
    TURNABLE.contains(&id)
}

/// The offsets param (a plain text param; the side panel shows it as "Reset label positions").
pub fn param() -> ParamDef {
    ParamDef::text(KEY, "Label positions", "")
}

/// Appends the offsets param to a figure's params.
pub fn with_param(mut v: Vec<ParamDef>) -> Vec<ParamDef> {
    v.push(param());
    v
}

pub type Offsets = BTreeMap<String, (f64, f64)>;

/// `"a:3,-4.5;s0:0,12"` -> map. Malformed entries are skipped (never panics).
pub fn parse(s: &str) -> Offsets {
    let mut m = Offsets::new();
    for it in s.split(';') {
        let Some((id, xy)) = it.split_once(':') else { continue };
        let Some((x, y)) = xy.split_once(',') else { continue };
        let (Ok(x), Ok(y)) = (x.trim().parse::<f64>(), y.trim().parse::<f64>()) else { continue };
        let id = id.trim();
        if id.is_empty() || !x.is_finite() || !y.is_finite() {
            continue;
        }
        m.insert(id.to_string(), (x, y));
    }
    m
}

fn num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    let r = if r == 0.0 { 0.0 } else { r }; // no "-0"
    format!("{r}")
}

/// Map -> canonical text (sorted ids, one decimal, zero offsets dropped).
pub fn format(m: &Offsets) -> String {
    m.iter()
        .filter(|(_, (x, y))| (x * 10.0).round() != 0.0 || (y * 10.0).round() != 0.0)
        .map(|(k, (x, y))| format!("{k}:{},{}", num(*x), num(*y)))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn get(p: &Params, id: &str) -> (f64, f64) {
    parse(p.s(KEY)).get(id).copied().unwrap_or((0.0, 0.0))
}

/// Sets label `id`'s offset (removing it when it is back at its own place).
pub fn set(p: &mut Params, id: &str, off: (f64, f64)) {
    let mut m = parse(p.s(KEY));
    m.insert(id.to_string(), off);
    p.set_s(KEY, &format(&m));
}

pub fn any_moved(p: &Params) -> bool {
    !parse(p.s(KEY)).is_empty()
}

pub fn reset(p: &mut Params) {
    p.set_s(KEY, "");
}

thread_local! {
    /// Offsets of the figure being rendered on this thread (set by `rendering`).
    static OFFS: RefCell<Option<Offsets>> = const { RefCell::new(None) };
    /// The figure's Turn (degrees, counter-clockwise) while its labels are drawn: offsets are
    /// stored unturned and turn with the figure, as on the reference.
    static TURN: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

/// Draws the labels emitted by `f` with offsets turned by `deg` (the figure's Turn).
pub fn turned<R>(deg: f64, f: impl FnOnce() -> R) -> R {
    let prev = TURN.with(|t| t.replace(if deg.is_finite() { deg } else { 0.0 }));
    let out = f();
    TURN.with(|t| t.set(prev));
    out
}

/// Screen offset (y down) turned counter-clockwise by `deg`.
pub fn turn_off(o: (f64, f64), deg: f64) -> (f64, f64) {
    if deg == 0.0 {
        return o;
    }
    let (c, s) = (deg.to_radians().cos(), deg.to_radians().sin());
    (o.0 * c + o.1 * s, -o.0 * s + o.1 * c)
}

/// Runs a renderer with `p`'s label offsets in scope, so every `wrap` inside it applies them.
pub fn rendering<R>(p: &Params, f: impl FnOnce() -> R) -> R {
    let prev = OFFS.with(|o| o.replace(Some(parse(p.s(KEY)))));
    let out = f();
    OFFS.with(|o| *o.borrow_mut() = prev);
    out
}

/// Wraps one label's SVG (`inner`, drawn unmoved around centre `c` with half extent `half`) in a
/// group carrying its id and box, translated by its offset. Outside `rendering` it still tags the
/// label (offset 0), which keeps raw renders and hit-testing consistent.
pub fn wrap(id: &str, c: (f64, f64), half: (f64, f64), inner: &str) -> String {
    let stored = OFFS.with(|o| o.borrow().as_ref().and_then(|m| m.get(id).copied())).unwrap_or((0.0, 0.0));
    let turn = TURN.with(|t| t.get());
    let (dx, dy) = turn_off(stored, turn);
    let tr = if dx == 0.0 && dy == 0.0 { String::new() } else { format!(r#" transform="translate({dx:.2} {dy:.2})""#) };
    let rot = if turn == 0.0 { String::new() } else { format!(r#" data-turn="{turn}""#) };
    format!(r#"<g data-lbl="{id}" data-box="{:.2} {:.2} {:.2} {:.2}"{rot}{tr}>{inner}</g>"#, c.0, c.1, half.0.max(1.0), half.1.max(1.0))
}

/// Box of a centred text label of `chars` characters at font `size` (serif digits/letters are
/// about 0.5em wide).
pub fn text_half(text: &str, size: f64) -> (f64, f64) {
    let n = text.chars().count().max(1) as f64;
    ((n * size * 0.5) / 2.0 + 2.0, size * 0.55)
}

/// One movable label found in a rendered SVG.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelBox {
    pub id: String,
    /// Unmoved centre (SVG units).
    pub base: (f64, f64),
    pub half: (f64, f64),
    /// Drawn offset (the stored one turned by `turn`).
    pub off: (f64, f64),
    /// The figure's Turn the label was drawn with (degrees).
    pub turn: f64,
}

impl LabelBox {
    pub fn centre(&self) -> (f64, f64) {
        (self.base.0 + self.off.0, self.base.1 + self.off.1)
    }
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!(" {name}=\"");
    let i = tag.find(&pat)? + pat.len();
    tag[i..].split('"').next()
}

/// Every `wrap`ped label of `svg`, in drawing order.
pub fn boxes(svg: &str) -> Vec<LabelBox> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<g data-lbl=\"") {
        rest = &rest[i..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        rest = &rest[end.min(rest.len())..];
        let Some(id) = attr(tag, "data-lbl") else { continue };
        let b: Vec<f64> = attr(tag, "data-box").unwrap_or("").split_whitespace().filter_map(|t| t.parse().ok()).collect();
        if b.len() != 4 {
            continue;
        }
        let off = attr(tag, "transform")
            .and_then(|t| t.strip_prefix("translate(")?.strip_suffix(')').map(|s| s.to_string()))
            .map(|s| {
                let v: Vec<f64> = s.split(|c: char| c == ' ' || c == ',').filter_map(|t| t.parse().ok()).collect();
                (v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0))
            })
            .unwrap_or((0.0, 0.0));
        let turn = attr(tag, "data-turn").and_then(|t| t.parse().ok()).unwrap_or(0.0);
        out.push(LabelBox { id: id.to_string(), base: (b[0], b[1]), half: (b[2], b[3]), off, turn });
    }
    out
}

/// Drag handles for every movable label of figure `render` at `p`: invisible zones over the
/// drawn text (outlined on hover, move cursor); dragging writes only `KEY`.
pub fn handles(render: fn(&Params) -> String, p: &Params) -> Vec<Handle> {
    let svg = render(p);
    let (w, h) = super::svg_size(&svg);
    boxes(&svg)
        .into_iter()
        .map(|b| {
            let (id, base, turn) = (b.id.clone(), b.base, b.turn);
            // the label's centre stays on the canvas, so it can always be grabbed again
            let mut h = Handle::knob(b.centre(), "Drag to move this label", move |p: &mut Params, s| {
                let (x, y) = (s.0.clamp(0.0, w), s.1.clamp(0.0, h));
                if x.is_finite() && y.is_finite() {
                    set(p, &id, turn_off((x - base.0, y - base.1), -turn));
                }
            });
            h.shape = Shape::Zone;
            h.cursor = Cursor::Move;
            h.half = b.half;
            h.label_drag = true;
            h
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_round_trip_and_ignore_garbage() {
        let m = parse("a0: 3,-4.5 ; s1:0,12;bad;x:1;y:a,b;:1,2");
        assert_eq!(m.len(), 2);
        assert_eq!(m["a0"], (3.0, -4.5));
        assert_eq!(m["s1"], (0.0, 12.0));
        let t = format(&m);
        assert_eq!(t, "a0:3,-4.5;s1:0,12");
        assert_eq!(parse(&t), m);
        // zero offsets vanish, -0 never appears
        let mut p = Params::default();
        set(&mut p, "v0", (0.04, -0.04));
        assert_eq!(p.s(KEY), "");
        set(&mut p, "v0", (5.26, -1.0));
        set(&mut p, "a2", (1.0, 1.0));
        assert_eq!(p.s(KEY), "a2:1,1;v0:5.3,-1");
        assert_eq!(get(&p, "v0"), (5.3, -1.0));
        assert!(any_moved(&p));
        set(&mut p, "v0", (0.0, 0.0));
        assert_eq!(p.s(KEY), "a2:1,1");
        reset(&mut p);
        assert!(!any_moved(&p));
    }

    #[test]
    fn wrap_applies_offsets_only_while_rendering() {
        let mut p = Params::default();
        p.set_s(KEY, "s0:10,-5");
        let plain = wrap("s0", (50.0, 60.0), (8.0, 9.0), "<text>12</text>");
        assert!(!plain.contains("transform"), "{plain}");
        let moved = rendering(&p, || wrap("s0", (50.0, 60.0), (8.0, 9.0), "<text>12</text>"));
        assert!(moved.contains(r#"transform="translate(10.00 -5.00)""#), "{moved}");
        let other = rendering(&p, || wrap("s1", (1.0, 2.0), (3.0, 4.0), "x"));
        assert!(!other.contains("transform"));
        let b = boxes(&format!("<svg>{moved}{other}</svg>"));
        assert_eq!(b.len(), 2);
        assert_eq!(b[0], LabelBox { id: "s0".into(), base: (50.0, 60.0), half: (8.0, 9.0), off: (10.0, -5.0), turn: 0.0 });
        assert_eq!(b[0].centre(), (60.0, 55.0));
        assert_eq!(b[1].off, (0.0, 0.0));
        // scope restored
        assert!(!wrap("s0", (0.0, 0.0), (1.0, 1.0), "").contains("transform"));
    }

    #[test]
    fn offsets_turn_with_the_figure() {
        // a quarter turn counter-clockwise takes "right" to "up" (y down on screen)
        let o = turn_off((10.0, 0.0), 90.0);
        assert!((o.0).abs() < 1e-9 && (o.1 + 10.0).abs() < 1e-9, "{o:?}");
        let back = turn_off(turn_off((3.0, -7.0), 37.0), -37.0);
        assert!((back.0 - 3.0).abs() < 1e-9 && (back.1 + 7.0).abs() < 1e-9);
        let mut p = Params::default();
        p.set_s(KEY, "s0:10,0");
        let g = rendering(&p, || turned(90.0, || wrap("s0", (50.0, 60.0), (8.0, 9.0), "t")));
        assert!(g.contains(r#"data-turn="90""#) && g.contains("translate(0.00 -10.00)"), "{g}");
        let b = &boxes(&g)[0];
        assert_eq!(b.turn, 90.0);
        assert!((b.centre().1 - 50.0).abs() < 1e-9);
    }
}
