//! Equation support for queShion, a thin adapter over the equaSHion crates (`../equaSHion`).
//!
//! * Equations live inside ordinary `String` fields as `$linear$`, where `linear` is equaSHion's
//!   linear format (`(a+b)/c`, `sqrt(x)`, `x^2`, and a LaTeX subset such as `\frac{a}{b}`).
//! * [`stem_ui`] draws text with inline equations, wrapped, on a shared baseline. Each equation
//!   is laid out and rasterised by equaSHion into a white-on-transparent texture (cached by
//!   content, size and dpi) and tinted with the text colour, so dark and light both work.
//! * [`stem_to_plain`] is the Unicode approximation used where math cannot be drawn.
//! * [`png`] / [`to_svg`] feed the exports.

use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use egui::{Color32, FontFamily, FontId, Galley, Pos2, Rect, Response, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation, layout_equation_wrapped};
use equashion_math::{Math, from_linear, to_linear, unknown_commands};

use crate::theme;

pub use equashion_egui::{EditorResponse, EquationEditor};

fn fonts() -> Arc<FontDb> {
    equashion_egui::bundled_fonts()
}

// ---------- segments ----------

/// A piece of question text: plain text, or the linear content between a pair of `$`.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Text(String),
    Math(String),
}

/// A segment with the char range it covers in the source (delimiters included).
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub seg: Segment,
    pub span: Range<usize>,
}

/// Split text on `$...$`. `\$` is a literal dollar. A `$` pair only counts as math when the
/// content is non-empty and neither starts nor ends with whitespace (so "costs $5 and $6" stays text).
pub fn pieces(text: &str) -> Vec<Piece> {
    let ch: Vec<char> = text.chars().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut buf = String::new();
    let mut buf_start = 0usize;
    let mut i = 0;
    let flush = |buf: &mut String, out: &mut Vec<Piece>, start: usize, end: usize| {
        if !buf.is_empty() {
            out.push(Piece { seg: Segment::Text(std::mem::take(buf)), span: start..end });
        }
    };
    while i < ch.len() {
        let c = ch[i];
        if c == '\\' && ch.get(i + 1) == Some(&'$') {
            if buf.is_empty() {
                buf_start = i;
            }
            buf.push('$');
            i += 2;
            continue;
        }
        if c == '$' {
            let dbl = ch.get(i + 1) == Some(&'$');
            let start = if dbl { i + 2 } else { i + 1 };
            let mut j = start;
            let mut close = None;
            while j < ch.len() {
                if ch[j] == '\\' {
                    j += 2;
                    continue;
                }
                if ch[j] == '$' {
                    close = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(j) = close {
                let inner: String = ch[start..j].iter().collect();
                let ok = !inner.trim().is_empty() && !inner.starts_with(char::is_whitespace) && !inner.ends_with(char::is_whitespace);
                if ok {
                    flush(&mut buf, &mut out, buf_start, i);
                    let end = j + if dbl && ch.get(j + 1) == Some(&'$') { 2 } else { 1 };
                    out.push(Piece { seg: Segment::Math(inner), span: i..end });
                    i = end;
                    continue;
                }
            }
            if buf.is_empty() {
                buf_start = i;
            }
            buf.push('$');
            i += 1;
            continue;
        }
        if buf.is_empty() {
            buf_start = i;
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut out, buf_start, ch.len());
    out
}

pub fn rich_segments(text: &str) -> Vec<Segment> {
    pieces(text).into_iter().map(|p| p.seg).collect()
}

/// True when `text` contains at least one `$...$` equation.
pub fn has_math(text: &str) -> bool {
    text.contains('$') && pieces(text).iter().any(|p| matches!(p.seg, Segment::Math(_)))
}

/// Wrap linear content as an inline equation chip string.
pub fn wrap_math(linear: &str) -> String {
    format!("${}$", linear.trim())
}

/// Replace the char range `span` of `text` (or insert at char `caret` when `span` is None).
pub fn splice(text: &str, span: Option<Range<usize>>, caret: usize, insert: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    let r = match span {
        Some(r) => r.start.min(chars.len())..r.end.min(chars.len()),
        None => {
            let c = caret.min(chars.len());
            c..c
        }
    };
    chars.splice(r, insert.chars());
    chars.into_iter().collect()
}

/// The linear content of the equation covering `span` (delimiters stripped).
pub fn content_of(text: &str, span: &Range<usize>) -> String {
    pieces(text)
        .into_iter()
        .find(|p| p.span == *span)
        .and_then(|p| if let Segment::Math(m) = p.seg { Some(m) } else { None })
        .unwrap_or_default()
}

// ---------- parsing ----------

/// Parse linear (or legacy LaTeX-ish) content. None when it is empty or has commands the
/// parser does not know (those would draw as literal text, so they count as a failure here).
pub fn parse(content: &str) -> Option<Math> {
    let m = from_linear(content);
    if m.is_empty() || !unknown_commands(&m).is_empty() {
        return None;
    }
    Some(m)
}

// ---------- plain text ----------

const SUP: &str = "0123456789+-=()ni";
const SUPC: &str = "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿⁱ";
const SUB: &str = "0123456789+-=()";
const SUBC: &str = "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎";

fn script_run(rest: &[char], from: &str, to: &str) -> (String, usize) {
    let f: Vec<char> = from.chars().collect();
    let t: Vec<char> = to.chars().collect();
    let mut out = String::new();
    let mut n = 0;
    for (k, &c) in rest.iter().enumerate() {
        let c2 = if c == '−' { '-' } else { c };
        // a sign only belongs to the script when it leads it (x^-1, not x^2-9)
        if k > 0 && matches!(c2, '+' | '-' | '=' | '(' | ')') {
            break;
        }
        match f.iter().position(|x| *x == c2) {
            Some(i) => {
                out.push(t[i]);
                n += 1;
            }
            None => break,
        }
    }
    (out, n)
}

/// Unicode approximation of one equation: equaSHion's own linear print, with simple
/// exponents and subscripts turned into superscript/subscript characters.
pub fn to_plain_text(content: &str) -> String {
    let Some(m) = parse(content) else { return content.to_string() };
    let lin = to_linear(&m);
    let ch: Vec<char> = lin.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        let (map_from, map_to) = match ch[i] {
            '^' => (SUP, SUPC),
            '_' => (SUB, SUBC),
            _ => {
                out.push(ch[i]);
                i += 1;
                continue;
            }
        };
        let (s, n) = script_run(&ch[i + 1..], map_from, map_to);
        if n > 0 {
            out.push_str(&s);
            i += 1 + n;
        } else {
            out.push(ch[i]);
            i += 1;
        }
    }
    out
}

/// Plain-text rendering of a whole stem (equations converted with [`to_plain_text`]), for
/// exports and cards that cannot draw math. Never shows raw `$` delimiters for valid math.
pub fn stem_to_plain(text: &str) -> String {
    rich_segments(text)
        .into_iter()
        .map(|s| match s {
            Segment::Text(t) => t,
            Segment::Math(m) => to_plain_text(&m),
        })
        .collect()
}

// ---------- SVG / PNG for exports ----------

/// Standalone SVG of the equation (black, display style, 20pt). Empty when it does not parse.
pub fn to_svg(content: &str) -> String {
    let Some(m) = parse(content) else { return String::new() };
    let f = fonts();
    let eq = layout_equation(&*f, &m, DEFAULT_FAMILY, 20.0, true, Rgb::BLACK, 2.0);
    equashion_render::scene_to_svg(&eq.scene, &*f, DEFAULT_FAMILY)
}

/// An equation rendered for a document: straight-alpha PNG bytes plus its size in points.
pub struct EqPng {
    pub png: Vec<u8>,
    pub w_pt: f32,
    pub h_pt: f32,
    /// Baseline, points from the top.
    pub baseline_pt: f32,
}

/// Render `content` (inline style) in black at `size_pt` with `scale` pixels per point.
pub fn png(content: &str, size_pt: f32, scale: f32) -> Option<EqPng> {
    let m = parse(content)?;
    let f = fonts();
    let eq = layout_equation(&*f, &m, DEFAULT_FAMILY, size_pt as f64, false, Rgb::BLACK, 1.0);
    let r = equashion_render::render_scene(&eq.scene, &*f, scale as f64, None);
    if r.width == 0 || r.height == 0 {
        return None;
    }
    let mut bytes = Vec::new();
    let img = image::RgbaImage::from_raw(r.width, r.height, r.to_straight())?;
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).ok()?;
    Some(EqPng { png: bytes, w_pt: r.width as f32 / scale, h_pt: r.height as f32 / scale, baseline_pt: eq.baseline() as f32 })
}

// ---------- textures ----------

struct EqImg {
    tex: TextureHandle,
    size: Vec2,
    baseline: f32,
}

type CacheKey = (String, u32, u32, u32);

thread_local! {
    static CACHE: std::cell::RefCell<HashMap<CacheKey, Option<Rc<EqImg>>>> = std::cell::RefCell::new(HashMap::new());
}

/// Lay out and rasterise (white) at `size` points for the context's dpi. Cached; None when the
/// content does not parse. `max_w` (points) wraps/shrinks an equation wider than the line.
fn eq_image(ctx: &egui::Context, content: &str, size: f32, max_w: f32) -> Option<Rc<EqImg>> {
    let ppp = ctx.pixels_per_point();
    let full_key = (content.to_string(), size.to_bits(), (ppp * 100.0) as u32, 0);
    let hit = CACHE.with(|c| c.borrow().get(&full_key).cloned());
    let mut img = match hit {
        Some(v) => v,
        None => {
            let v = build(ctx, content, size, ppp, f64::INFINITY);
            CACHE.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() > 400 {
                    c.clear();
                }
                c.insert(full_key.clone(), v.clone());
            });
            v
        }
    };
    // Too wide for the line: lay out again wrapped to the available width.
    if let Some(i) = &img {
        if i.size.x > max_w + 0.5 && max_w > 30.0 {
            let key = (content.to_string(), size.to_bits(), (ppp * 100.0) as u32, max_w.floor() as u32 + 1);
            let hit = CACHE.with(|c| c.borrow().get(&key).cloned());
            img = match hit {
                Some(v) => v,
                None => {
                    let v = build(ctx, content, size, ppp, max_w as f64);
                    CACHE.with(|c| c.borrow_mut().insert(key, v.clone()));
                    v
                }
            };
        }
    }
    img
}

fn build(ctx: &egui::Context, content: &str, size: f32, ppp: f32, max_w: f64) -> Option<Rc<EqImg>> {
    let m = parse(content)?;
    let f = fonts();
    let pad = 1.0;
    let eq = if max_w.is_finite() {
        layout_equation_wrapped(&*f, &m, DEFAULT_FAMILY, size as f64, false, Rgb::WHITE, pad, max_w)
    } else {
        layout_equation(&*f, &m, DEFAULT_FAMILY, size as f64, false, Rgb::WHITE, pad)
    };
    let r = equashion_render::render_scene(&eq.scene, &*f, ppp as f64, None);
    if r.width == 0 || r.height == 0 {
        return None;
    }
    let ci = egui::ColorImage::from_rgba_premultiplied([r.width as usize, r.height as usize], &r.pixels);
    let tex = ctx.load_texture("eq", ci, TextureOptions::LINEAR);
    Some(Rc::new(EqImg { tex, size: Vec2::new(r.width as f32 / ppp, r.height as f32 / ppp), baseline: eq.baseline() as f32 }))
}

// ---------- mixed text + equations ----------

enum Elem {
    Word { g: Arc<Galley>, asc: f32, breakable: bool, color: Color32 },
    Math { img: Rc<EqImg>, span: Range<usize>, content: String },
    Break,
}

struct Placed {
    x: f32,
    elem: usize,
}

/// Which equation of a stem was clicked this frame.
#[derive(Clone, Debug)]
pub struct EqHit {
    /// Char range of the whole `$...$` in the text.
    pub span: Range<usize>,
    /// The linear content.
    pub content: String,
}

/// Render `text` (with `$...$` equations) in the house serif.
pub fn stem_ui(ui: &mut Ui, text: &str, size: f32, color: Color32) -> Response {
    stem_ui_with(ui, text, size, color, theme::serif_family())
}

pub fn stem_ui_with(ui: &mut Ui, text: &str, size: f32, color: Color32, family: FontFamily) -> Response {
    stem_ui_edit(ui, text, size, color, family, false).0
}

/// Same, and with `editable` the equations are clickable chips; the clicked one is returned.
pub fn stem_ui_edit(ui: &mut Ui, text: &str, size: f32, color: Color32, family: FontFamily, editable: bool) -> (Response, Option<EqHit>) {
    let avail = ui.available_width().max(40.0);
    let font = FontId::new(size, family);
    let ctx = ui.ctx().clone();
    let mut elems: Vec<Elem> = Vec::new();
    let pad = size * 0.12;

    let push_words = |elems: &mut Vec<Elem>, t: &str, color: Color32, ui: &Ui| {
        let mut first = true;
        for line in t.split('\n') {
            if !first {
                elems.push(Elem::Break);
            }
            first = false;
            for w in split_words(line) {
                let g = ui.painter().layout_no_wrap(w.to_string(), font.clone(), color);
                let (asc, _) = galley_asc_desc(&g);
                elems.push(Elem::Word { g, asc, breakable: w.ends_with(' '), color });
            }
        }
    };

    for p in pieces(text) {
        match p.seg {
            Segment::Text(t) => push_words(&mut elems, &t, color, ui),
            Segment::Math(m) => match eq_image(&ctx, &m, size, avail - 2.0 * pad) {
                Some(img) => elems.push(Elem::Math { img, span: p.span, content: m }),
                None => push_words(&mut elems, &format!("${m}$"), theme::bad(), ui),
            },
        }
    }

    struct Line {
        items: Vec<Placed>,
        asc: f32,
        desc: f32,
        w: f32,
    }
    let new_line = || Line { items: vec![], asc: size * 0.8, desc: size * 0.25, w: 0.0 };
    let width_of = |e: &Elem| -> f32 {
        match e {
            Elem::Word { g, .. } => g.size().x,
            Elem::Math { img, .. } => img.size.x + 2.0 * pad,
            Elem::Break => 0.0,
        }
    };
    let mut lines: Vec<Line> = vec![new_line()];
    let mut prev_breakable = true;
    for (idx, e) in elems.iter().enumerate() {
        if matches!(e, Elem::Break) {
            lines.push(new_line());
            prev_breakable = true;
            continue;
        }
        let w = width_of(e);
        let cur = lines.last_mut().unwrap();
        // a trailing space shouldn't force a wrap
        let trimmed = match e {
            Elem::Word { g, .. } if g.text().ends_with(' ') => w - size * 0.3,
            _ => w,
        };
        if !cur.items.is_empty() && cur.w + trimmed > avail && prev_breakable {
            lines.push(new_line());
        }
        let cur = lines.last_mut().unwrap();
        cur.items.push(Placed { x: cur.w, elem: idx });
        cur.w += w;
        match e {
            Elem::Word { asc, breakable, g, .. } => {
                cur.asc = cur.asc.max(*asc);
                cur.desc = cur.desc.max((g.size().y - asc).max(0.0));
                prev_breakable = *breakable;
            }
            Elem::Math { img, .. } => {
                cur.asc = cur.asc.max(img.baseline);
                cur.desc = cur.desc.max(img.size.y - img.baseline);
                prev_breakable = false;
            }
            Elem::Break => {}
        }
    }

    let leading = size * 0.18;
    let total_h: f32 = lines.iter().map(|l| l.asc + l.desc + leading).sum::<f32>() - leading;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail, total_h.max(size)), Sense::hover());
    let painter = ui.painter_at(rect.expand2(Vec2::new(2.0, 6.0)));
    let ppp = ui.pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp;
    let mut hit = None;
    let mut y = rect.top();
    for line in &lines {
        let base = y + line.asc;
        for p in &line.items {
            match &elems[p.elem] {
                Elem::Word { g, asc, color, .. } => {
                    painter.galley(Pos2::new(rect.left() + p.x, base - asc), g.clone(), *color);
                }
                Elem::Math { img, span, content } => {
                    let min = Pos2::new(snap(rect.left() + p.x + pad), snap(base - img.baseline));
                    let r = Rect::from_min_size(min, img.size);
                    if editable {
                        let hr = r.expand2(Vec2::new(pad, 1.0));
                        let id = ui.id().with(("eqchip", span.start));
                        let rs = ui.interact(hr, id, Sense::click());
                        if rs.hovered() {
                            painter.rect_filled(hr, 5.0, theme::accent_faded());
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if rs.clicked() {
                            hit = Some(EqHit { span: span.clone(), content: content.clone() });
                        }
                    }
                    painter.image(img.tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), color);
                }
                Elem::Break => {}
            }
        }
        y += line.asc + line.desc + leading;
    }
    (resp, hit)
}

/// Split into words, each carrying its trailing spaces.
fn split_words(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b' ' {
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            out.push(&s[start..j]);
            start = j;
            i = j;
        } else {
            i += 1;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

fn galley_asc_desc(g: &Galley) -> (f32, f32) {
    match g.rows.first() {
        Some(r) => {
            let base = r.pos.y + r.row.glyphs.first().map_or(r.row.size.y * 0.8, |gl| gl.pos.y);
            (base, (g.size().y - base).max(0.0))
        }
        None => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments() {
        assert_eq!(
            rich_segments(r"Solve $x^2=4$ for \$5 and $y$."),
            vec![
                Segment::Text("Solve ".into()),
                Segment::Math("x^2=4".into()),
                Segment::Text(" for $5 and ".into()),
                Segment::Math("y".into()),
                Segment::Text(".".into()),
            ]
        );
        assert_eq!(rich_segments("costs $5 and $6"), vec![Segment::Text("costs $5 and $6".into())]);
        assert_eq!(rich_segments("lone $ sign"), vec![Segment::Text("lone $ sign".into())]);
        assert_eq!(rich_segments("$$a$$"), vec![Segment::Math("a".into())]);
        assert!(rich_segments("").is_empty());
        assert!(!has_math("plain"));
        assert!(has_math("a $b$"));
    }

    #[test]
    fn spans_and_splice() {
        let t = "ab $x^2$ cd $y$";
        let ps = pieces(t);
        let m = ps.iter().find(|p| p.seg == Segment::Math("x^2".into())).unwrap();
        assert_eq!(m.span, 3..8);
        assert_eq!(content_of(t, &m.span), "x^2");
        assert_eq!(splice(t, Some(m.span.clone()), 0, "$z$"), "ab $z$ cd $y$");
        assert_eq!(splice("ab", None, 1, "$q$"), "a$q$b");
        assert_eq!(splice("ab", None, 99, "$q$"), "ab$q$");
    }

    #[test]
    fn parse_linear_and_latex() {
        for s in ["x^2", "(a+b)/c", "sqrt(x)", r"\frac{x^2-9}{x-3}", r"\sqrt[3]{x}", r"\sum_{n=1}^{\infty}\frac{1}{n^2}", "pi r^2", "a<=b"] {
            assert!(parse(s).is_some(), "{s}");
            assert!(!to_svg(s).is_empty(), "{s}");
            assert!(!to_plain_text(s).is_empty());
        }
        assert!(parse("").is_none());
        assert!(parse(r"\notacommand").is_none());
    }

    #[test]
    fn plain_text() {
        assert_eq!(to_plain_text("x^2"), "x²");
        assert_eq!(to_plain_text("(x^2-9)/(x-3)"), "(x²−9)/(x−3)");
        assert_eq!(to_plain_text("pi r^2"), "π r²");
        assert!(to_plain_text("sqrt(x)").contains('√'));
        assert!(to_plain_text(r"\frac{a}{b}").contains('/'));
        // a parse failure is passed through untouched
        assert_eq!(to_plain_text(r"\foo"), r"\foo");
        assert_eq!(stem_to_plain(r"Find $x^2$ if it costs \$5"), "Find x² if it costs $5");
        assert!(!stem_to_plain("Solve $(a+b)/c$").contains('$'));
    }

    #[test]
    fn png_renders() {
        let p = png("x^2+1/2", 12.0, 4.0).unwrap();
        assert_eq!(&p.png[1..4], b"PNG");
        assert!(p.w_pt > 5.0 && p.h_pt > 5.0 && p.baseline_pt > 0.0);
        assert!(png("", 12.0, 4.0).is_none());
    }

    /// Run stems through real egui frames (no window): wraps, a failing equation, an editable chip.
    #[test]
    fn stem_frames() {
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        for i in 0..3 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 900.0)));
            input.time = Some(i as f64 * 0.1);
            let out = ctx.run_ui(input, |ui| {
                for w in [40.0, 160.0, 600.0] {
                    ui.allocate_ui(egui::vec2(w, 10.0), |ui| {
                        stem_ui(ui, "Evaluate $(x^2-9)/(x-3)$ and $sqrt(x)$ for the values of x. $\\bad$\nNext line", 16.0, Color32::BLACK);
                        stem_ui_edit(ui, "edit $a/b$", 16.0, Color32::BLACK, theme::serif_family(), true);
                    });
                }
            });
            out.drop_without_applying_deltas();
        }
        assert!(CACHE.with(|c| !c.borrow().is_empty()));
    }
}
