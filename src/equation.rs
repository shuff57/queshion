//! Equation support for verSHion, a thin adapter over the equaSHion crates (`../equaSHion`).
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

/// One-line preview of a stem for cards, lists and search: markup flattened
/// (`richtext::plain`) and equations as their Unicode approximation.
pub fn stem_preview(text: &str) -> String {
    stem_to_plain(&scripts_to_unicode(&crate::richtext::plain(text)))
}

/// `_{2}` / `^{2+}` (left in by `richtext::plain`) as Unicode sub/superscripts where every
/// character has one, else the bare text ("x_{ab}" -> "x_ab").
fn scripts_to_unicode(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut in_math = false;
    while i < ch.len() {
        let c = ch[i];
        if c == '$' {
            in_math = !in_math;
        }
        if !in_math
            && (c == '_' || c == '^')
            && ch.get(i + 1) == Some(&'{')
            && let Some(end) = (i + 2..ch.len()).find(|&j| ch[j] == '}')
        {
            let inner: String = ch[i + 2..end].iter().collect();
            match crate::richtext::unicode_script(&inner, c == '^') {
                Some(u) => out.push_str(&u),
                None => {
                    out.push(c);
                    out.push_str(&inner);
                }
            }
            i = end + 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
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

/// A typeset equation for the WYSIWYG editor: texture, size in points and baseline from the top
/// (cached like every other equation). None when `content` is not a drawable equation (blank,
/// padded with spaces, or not parseable), in which case the editor shows it as literal text.
pub fn math_image(ctx: &egui::Context, content: &str, size: f32, max_w: f32) -> Option<(egui::TextureId, Vec2, f32)> {
    if content.trim().is_empty() || content.starts_with(char::is_whitespace) || content.ends_with(char::is_whitespace) {
        return None;
    }
    eq_image(ctx, content, size, max_w).map(|i| (i.tex.id(), i.size, i.baseline))
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
    /// A word (with its trailing spaces). `dy` shifts it off the baseline (sub/superscript).
    Word { g: Arc<Galley>, asc: f32, breakable: bool, color: Color32, dy: f32 },
    /// A typeset equation; `span` is its char range in the source when it can be clicked.
    Math { img: Rc<EqImg>, span: Option<Range<usize>>, content: String },
    /// A `___` blank drawn as a writing line.
    Blank { w: f32, breakable: bool, color: Color32 },
    Break,
}

struct Placed {
    x: f32,
    elem: usize,
}

struct FLine {
    items: Vec<Placed>,
    asc: f32,
    desc: f32,
    w: f32,
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

/// Line decorations of a run: `~~strikethrough~~`, and the underline of a link.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Deco {
    pub strike: bool,
    pub under: bool,
    /// `` `code` ``: a tint behind the (monospace) text.
    pub code: bool,
}

/// The colour links show in on a background whose text is `ink` (dark blue on paper, light
/// blue on a dark theme).
pub(crate) fn link_color(ink: Color32) -> Color32 {
    let lum = 0.3 * ink.r() as f32 + 0.59 * ink.g() as f32 + 0.11 * ink.b() as f32;
    if lum < 128.0 { Color32::from_rgb(0x0d, 0x45, 0xad) } else { Color32::from_rgb(0x8a, 0xb4, 0xf8) }
}

/// The tint behind `` `code` `` on a background whose text is `ink`.
pub(crate) fn code_tint(ink: Color32) -> Color32 {
    let lum = 0.3 * ink.r() as f32 + 0.59 * ink.g() as f32 + 0.11 * ink.b() as f32;
    if lum < 128.0 { Color32::from_rgba_unmultiplied(0, 0, 0, 18) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 26) }
}

fn word_galley(ui: &Ui, w: &str, font: &FontId, color: Color32, italics: bool, deco: Deco) -> Arc<Galley> {
    // A real italic face where the family has one; egui's slant only as the fallback.
    let (font, italics) = match italics.then(|| theme::italic_family(ui.ctx(), &font.family)).flatten() {
        Some(family) => (&FontId::new(font.size, family), false),
        None => (font, italics),
    };
    if italics || deco.strike || deco.under || deco.code {
        let line = egui::Stroke::new((font.size / 14.0).max(1.0), color);
        let strikethrough = if deco.strike { line } else { egui::Stroke::NONE };
        let underline = if deco.under { line } else { egui::Stroke::NONE };
        let background = if deco.code { code_tint(color) } else { Color32::TRANSPARENT };
        let job = egui::text::LayoutJob::single_section(
            w.to_string(),
            egui::TextFormat { font_id: font.clone(), color, italics, strikethrough, underline, background, ..Default::default() },
        );
        ui.painter().layout_job(job)
    } else {
        ui.painter().layout_no_wrap(w.to_string(), font.clone(), color)
    }
}

#[allow(clippy::too_many_arguments)]
fn push_word(elems: &mut Vec<Elem>, ui: &Ui, w: &str, font: &FontId, color: Color32, italics: bool, deco: Deco, dy: f32) {
    let g = word_galley(ui, w, font, color, italics, deco);
    let (asc, _) = galley_asc_desc(&g);
    elems.push(Elem::Word { g, asc, breakable: w.ends_with(' '), color, dy });
}

/// Words of `t` (each `\n` a hard break). With `blank_w`, runs of three or more `_` become
/// writing lines that wide.
#[allow(clippy::too_many_arguments)]
fn push_text(elems: &mut Vec<Elem>, ui: &Ui, t: &str, font: &FontId, color: Color32, italics: bool, dy: f32, blank_w: Option<(f32, f32)>) {
    push_text_s(elems, ui, t, font, color, italics, Deco::default(), dy, blank_w);
}

/// [`push_text`] with strikethrough.
#[allow(clippy::too_many_arguments)]
fn push_text_s(elems: &mut Vec<Elem>, ui: &Ui, t: &str, font: &FontId, color: Color32, italics: bool, deco: Deco, dy: f32, blank_w: Option<(f32, f32)>) {
    for (li, line) in t.split('\n').enumerate() {
        if li > 0 {
            elems.push(Elem::Break);
        }
        for w in split_words(line) {
            match blank_w {
                Some((bw, most)) if w.contains("___") => {
                    for (piece, is_blank) in split_blanks(w) {
                        if is_blank {
                            // at least the house blank; a longer run keeps its own length (as it
                            // prints), never wider than the line
                            let us = ui.painter().layout_no_wrap("_".to_string(), font.clone(), color).size().x;
                            let own = piece.trim_end().chars().count() as f32 * us;
                            elems.push(Elem::Blank { w: bw.max(own).min(most.max(bw)), breakable: piece.ends_with(' '), color });
                        } else {
                            push_word(elems, ui, piece, font, color, italics, deco, dy);
                        }
                    }
                }
                _ => push_word(elems, ui, w, font, color, italics, deco, dy),
            }
        }
    }
}

/// Split a word into text pieces and `___` runs (trailing spaces stay a text piece, so the gap
/// after a blank keeps its width and the line can break there).
fn split_blanks(w: &str) -> Vec<(&str, bool)> {
    let mut out = Vec::new();
    let b = w.as_bytes();
    let (mut i, mut start) = (0, 0);
    while i < b.len() {
        if b[i] == b'_' {
            let mut j = i;
            while j < b.len() && b[j] == b'_' {
                j += 1;
            }
            if j - i >= 3 {
                if start < i {
                    out.push((&w[start..i], false));
                }
                out.push((&w[i..j], true));
                start = j;
                i = j;
                continue;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    if start < w.len() {
        out.push((&w[start..], false));
    }
    out
}

fn elem_width(e: &Elem, pad: f32) -> f32 {
    match e {
        Elem::Word { g, .. } => g.size().x,
        Elem::Math { img, .. } => img.size.x + 2.0 * pad,
        Elem::Blank { w, .. } => *w,
        Elem::Break => 0.0,
    }
}

/// Greedy line breaking of `elems` into lines at most `avail` wide.
fn flow(elems: &[Elem], avail: f32, size: f32, pad: f32) -> Vec<FLine> {
    let new_line = || FLine { items: vec![], asc: size * 0.8, desc: size * 0.25, w: 0.0 };
    let mut lines: Vec<FLine> = vec![new_line()];
    let mut prev_breakable = true;
    for (idx, e) in elems.iter().enumerate() {
        if matches!(e, Elem::Break) {
            lines.push(new_line());
            prev_breakable = true;
            continue;
        }
        let w = elem_width(e, pad);
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
            Elem::Word { asc, breakable, g, dy, .. } => {
                cur.asc = cur.asc.max(asc - dy);
                cur.desc = cur.desc.max((g.size().y - asc + dy).max(0.0));
                prev_breakable = *breakable;
            }
            Elem::Math { img, .. } => {
                cur.asc = cur.asc.max(img.baseline);
                cur.desc = cur.desc.max(img.size.y - img.baseline);
                prev_breakable = false;
            }
            Elem::Blank { breakable, .. } => prev_breakable = *breakable,
            Elem::Break => {}
        }
    }
    lines
}

fn flow_height(lines: &[FLine], leading: f32) -> f32 {
    (lines.iter().map(|l| l.asc + l.desc + leading).sum::<f32>() - leading).max(0.0)
}

/// Natural (unwrapped) width of a run of elements.
fn natural_width(elems: &[Elem], pad: f32) -> f32 {
    let mut best: f32 = 0.0;
    let mut cur = 0.0;
    for e in elems {
        if matches!(e, Elem::Break) {
            best = best.max(cur);
            cur = 0.0;
        } else {
            cur += elem_width(e, pad);
        }
    }
    best.max(cur)
}

/// Paint flowed lines with their top-left at `origin`. Returns the clicked equation, if any.
#[allow(clippy::too_many_arguments)]
fn paint_flow(ui: &Ui, painter: &egui::Painter, origin: Pos2, lines: &[FLine], elems: &[Elem], leading: f32, pad: f32, color: Color32, editable: bool) -> Option<EqHit> {
    let ppp = ui.pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp;
    let mut hit = None;
    let mut y = origin.y;
    for line in lines {
        let base = y + line.asc;
        for p in &line.items {
            match &elems[p.elem] {
                Elem::Word { g, asc, color, dy, .. } => {
                    painter.galley(Pos2::new(origin.x + p.x, base - asc + dy), g.clone(), *color);
                }
                Elem::Blank { w, color, .. } => {
                    let yy = snap(base + 2.0);
                    painter.line_segment([Pos2::new(origin.x + p.x + 1.0, yy), Pos2::new(origin.x + p.x + w - 3.0, yy)], egui::Stroke::new(1.0, *color));
                }
                Elem::Math { img, span, content } => {
                    let min = Pos2::new(snap(origin.x + p.x + pad), snap(base - img.baseline));
                    let r = Rect::from_min_size(min, img.size);
                    if editable && let Some(span) = span {
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
    hit
}

/// Same, and with `editable` the equations are clickable chips; the clicked one is returned.
/// The text is literal (no stem markup); see [`rich_ui`] for structured stems.
pub fn stem_ui_edit(ui: &mut Ui, text: &str, size: f32, color: Color32, family: FontFamily, editable: bool) -> (Response, Option<EqHit>) {
    let avail = ui.available_width().max(40.0);
    let font = FontId::new(size, family);
    let ctx = ui.ctx().clone();
    let mut elems: Vec<Elem> = Vec::new();
    let pad = size * 0.12;

    for p in pieces(text) {
        match p.seg {
            Segment::Text(t) => push_text(&mut elems, ui, &t, &font, color, false, 0.0, None),
            Segment::Math(m) => match eq_image(&ctx, &m, size, avail - 2.0 * pad) {
                Some(img) => elems.push(Elem::Math { img, span: Some(p.span), content: m }),
                None => push_text(&mut elems, ui, &format!("${m}$"), &font, theme::bad(), false, 0.0, None),
            },
        }
    }

    let lines = flow(&elems, avail, size, pad);
    let leading = size * 0.18;
    let total_h = flow_height(&lines, leading);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail, total_h.max(size)), Sense::hover());
    let painter = ui.painter_at(rect.expand2(Vec2::new(2.0, 6.0)));
    let hit = paint_flow(ui, &painter, rect.left_top(), &lines, &elems, leading, pad, color, editable);
    (resp, hit)
}

/// Draw parsed blocks at the cursor, `avail` wide. `top`: the stem's own blocks (the lead run
/// goes before the first one); panels of a side-by-side group draw their blocks with `top` false.
fn blocks_ui(ui: &mut Ui, rc: &mut RichCtx, blocks: &[crate::richtext::Node], avail: f32, top: bool) -> Option<EqHit> {
    use crate::richtext::{Block, Node};
    let o = rc.o;
    let size = o.size;
    let gap = size * 0.4;
    let leading = size * o.leading;
    let mut hit = None;
    for (bi, node) in blocks.iter().enumerate() {
        if bi > 0 {
            ui.add_space(gap);
        }
        let first = bi == 0 && top;
        if first && o.lead.is_some() && !matches!(node, Node::Block { block: Block::Para(_), .. }) {
            let mut e = Vec::new();
            rc.lead_elems(ui, &mut e);
            rc.place(ui, &e, 0.0, avail);
            ui.add_space(gap * 0.5);
        }
        let (block, depths) = match node {
            Node::Block { block, center, depths } => {
                // only paragraphs and headings centre (as the editor's Centre button does)
                rc.center = *center && matches!(block, Block::Para(_) | Block::Heading(..));
                (block, depths.as_slice())
            }
            Node::Side(panels) => {
                rc.center = false;
                let n = panels.len().max(1) as f32;
                let col_gap = size * 1.2;
                let w = ((avail - col_gap * (n - 1.0)) / n).max(size * 2.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for (pi, p) in panels.iter().enumerate() {
                        if pi > 0 {
                            ui.add_space(col_gap);
                        }
                        ui.allocate_ui_with_layout(Vec2::new(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(w);
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            hit = hit.take().or(blocks_ui(ui, rc, p, w, false));
                        });
                    }
                });
                continue;
            }
        };
        match block {
            Block::Para(lines) => {
                let mut e = Vec::new();
                if first {
                    rc.lead_elems(ui, &mut e);
                }
                for (li, l) in lines.iter().enumerate() {
                    if li > 0 {
                        e.push(Elem::Break);
                    }
                    rc.line_elems(ui, &mut e, l, avail, false, true);
                }
                hit = hit.take().or(rc.place(ui, &e, 0.0, avail).1);
            }
            Block::Bullets(items) | Block::Numbered(items) | Block::Quote(items) => {
                let kind = match block {
                    Block::Bullets(_) => 0,
                    Block::Numbered(_) => 1,
                    _ => 2,
                };
                // nesting: one step per level; bullets and numbers per level (richtext)
                let depth = |k: usize| depths.get(k).copied().unwrap_or(0);
                let ords = crate::richtext::item_ordinals(&(0..items.len()).map(depth).collect::<Vec<_>>());
                let step = size * 1.25;
                let label = |k: usize| crate::richtext::number_label(depth(k), ords[k]);
                let indent0 = match kind {
                    0 => size * 1.25,
                    1 => (0..items.len()).map(|k| size * (0.45 + 0.55 * label(k).chars().count() as f32)).fold(size * 1.0, f32::max),
                    _ => size * 1.1,
                };
                let mut qtop = f32::NAN;
                let mut qbot = 0.0;
                for (k, l) in items.iter().enumerate() {
                    if k > 0 {
                        ui.add_space(leading);
                    }
                    let indent = indent0 + depth(k) as f32 * step;
                    let mut e = Vec::new();
                    rc.line_elems(ui, &mut e, l, avail - indent, false, true);
                    let (r, h) = rc.place(ui, &e, indent, avail - indent);
                    hit = hit.take().or(h);
                    let font = FontId::new(size, o.family.clone());
                    let base_y = r.top() + size * 0.8;
                    let lx = r.left() + depth(k) as f32 * step;
                    match kind {
                        // a task item's box stands in for its bullet
                        0 if crate::richtext::task_item(l).is_some() => {}
                        0 => {
                            let c = Pos2::new(lx + size * 0.45, base_y - size * 0.3);
                            match depth(k) % 3 {
                                0 => {
                                    ui.painter().circle_filled(c, size * 0.13, o.color);
                                }
                                1 => {
                                    ui.painter().circle_stroke(c, size * 0.12, egui::Stroke::new(1.0, o.color));
                                }
                                _ => {
                                    ui.painter().rect_filled(Rect::from_center_size(c, Vec2::splat(size * 0.2)), 0.0, o.color);
                                }
                            }
                        }
                        1 => {
                            let g = ui.painter().layout_no_wrap(label(k), font, o.color);
                            let (asc, _) = galley_asc_desc(&g);
                            let x = lx + indent0 - size * 0.35 - g.size().x;
                            ui.painter().galley(Pos2::new(x, base_y - asc), g, o.color);
                        }
                        _ => {
                            if qtop.is_nan() {
                                qtop = r.top();
                            }
                            qbot = r.bottom();
                        }
                    }
                }
                if kind == 2 && !qtop.is_nan() {
                    let x = ui.min_rect().left() + size * 0.35;
                    ui.painter().line_segment([Pos2::new(x, qtop), Pos2::new(x, qbot)], egui::Stroke::new(size * 0.16, o.color.gamma_multiply(0.35)));
                }
            }
            Block::Table { header, rows } => {
                hit = hit.take().or(table_ui(ui, rc, *header, rows, avail));
            }
            Block::Heading(level, line) => {
                // bold, a step larger per level (1 is largest)
                let scale = heading_scale(*level);
                let ho = Rich { size: size * scale, family: o.bold_family.clone(), lead: None, ..o.clone() };
                let mut hc = RichCtx { o: &ho, ctx: rc.ctx.clone(), spans: std::mem::take(&mut rc.spans), next_span: rc.next_span, center: rc.center };
                let mut e = Vec::new();
                hc.line_elems(ui, &mut e, line, avail, true, true);
                let h = hc.place(ui, &e, 0.0, avail).1;
                rc.spans = std::mem::take(&mut hc.spans);
                rc.next_span = hc.next_span;
                hit = hit.take().or(h);
            }
            Block::Divider => {
                let (r, _) = ui.allocate_exact_size(Vec2::new(avail, size * 0.9), Sense::hover());
                let y = r.center().y.round() + 0.5;
                ui.painter().hline(r.left()..=r.left() + avail, y, egui::Stroke::new(1.0, o.color.gamma_multiply(0.55)));
            }
            // (side-by-side groups are `Node::Side`, drawn above)
            Block::Side(_) => {}
        }
        rc.center = false;
    }
    hit
}

/// Heading size relative to the body: level 1 largest.
pub fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 1.4,
        2 => 1.2,
        _ => 1.07,
    }
}

// ---------- structured stems (richtext blocks) ----------

/// How [`rich_ui`] draws a stem.
#[derive(Clone, Debug)]
pub struct Rich {
    pub size: f32,
    pub color: Color32,
    pub family: FontFamily,
    pub bold_family: FontFamily,
    /// Extra space between wrapped lines, as a fraction of `size`.
    pub leading: f32,
    /// Equations become clickable chips (the clicked one is returned).
    pub editable: bool,
    /// A run drawn before the first line (e.g. "(2 pts) "): text, size, colour.
    pub lead: Option<(String, f32, Color32)>,
    /// Width of a `___` blank, as a multiple of `size`.
    pub blank_em: f32,
}

impl Rich {
    /// House serif at `size` in `color`.
    pub fn new(size: f32, color: Color32) -> Rich {
        Rich {
            size,
            color,
            family: theme::serif_family(),
            bold_family: theme::serif_bold_family(),
            leading: 0.22,
            editable: false,
            lead: None,
            blank_em: 4.5,
        }
    }
    pub fn leading(mut self, l: f32) -> Rich {
        self.leading = l;
        self
    }
    pub fn editable(mut self, e: bool) -> Rich {
        self.editable = e;
        self
    }
    pub fn lead(mut self, text: impl Into<String>, size: f32, color: Color32) -> Rich {
        self.lead = Some((text.into(), size, color));
        self
    }
}

/// Math spans of `text` in order, with their char ranges (to make richtext math clickable).
fn math_spans(text: &str) -> Vec<(String, Range<usize>)> {
    pieces(text)
        .into_iter()
        .filter_map(|p| match p.seg {
            Segment::Math(m) => Some((m, p.span)),
            Segment::Text(_) => None,
        })
        .collect()
}

struct RichCtx<'a> {
    o: &'a Rich,
    ctx: egui::Context,
    spans: Vec<(String, Range<usize>)>,
    next_span: usize,
    /// The block being placed is in a `::: center` group: its lines are centred.
    center: bool,
}

impl RichCtx<'_> {
    fn pad(&self) -> f32 {
        self.o.size * 0.12
    }

    /// Elements of one richtext line, `avail` wide (equations wider than that are wrapped).
    fn line_elems(&mut self, ui: &Ui, elems: &mut Vec<Elem>, line: &crate::richtext::Line, avail: f32, force_bold: bool, blanks: bool) {
        let size = self.o.size;
        let bw = blanks.then_some(((self.o.blank_em * size).min(avail.max(size * 2.0)), avail.max(size * 2.0)));
        for sp in line {
            let color = self.o.color;
            if sp.math {
                let content = sp.text.as_str();
                let lit = format!("${content}$");
                let ok = !content.trim().is_empty() && !content.starts_with(char::is_whitespace) && !content.ends_with(char::is_whitespace);
                let fam = if force_bold || sp.bold { self.o.bold_family.clone() } else { self.o.family.clone() };
                let font = FontId::new(size, fam);
                if !ok {
                    push_text(elems, ui, &lit, &font, color, sp.italic, 0.0, bw);
                    continue;
                }
                let span = self.spans[self.next_span.min(self.spans.len())..].iter().position(|(m, _)| m == content).map(|k| {
                    let at = self.next_span + k;
                    self.next_span = at + 1;
                    self.spans[at].1.clone()
                });
                match eq_image(&self.ctx, content, size, avail - 2.0 * self.pad()) {
                    Some(img) => elems.push(Elem::Math { img, span, content: content.to_string() }),
                    None => push_text(elems, ui, &lit, &font, theme::bad(), false, 0.0, None),
                }
                continue;
            }
            let fam = if force_bold || sp.bold { self.o.bold_family.clone() } else { self.o.family.clone() };
            let (fsize, dy) = if sp.sup {
                (size * 0.68, -size * 0.34)
            } else if sp.sub {
                (size * 0.68, size * 0.16)
            } else {
                (size, 0.0)
            };
            // `code` in a monospace face on a tint (blanks are never code)
            let font = if sp.code { FontId::new(fsize * 0.92, FontFamily::Monospace) } else { FontId::new(fsize, fam) };
            // Links show blue and underlined (as they print).
            let (color, under) = if sp.link.is_some() { (link_color(color), true) } else { (color, false) };
            push_text_s(elems, ui, &sp.text, &font, color, sp.italic, Deco { strike: sp.strike, under, code: sp.code }, dy, if sp.code { None } else { bw });
        }
    }

    fn lead_elems(&self, ui: &Ui, elems: &mut Vec<Elem>) {
        if let Some((t, s, c)) = &self.o.lead {
            push_text(elems, ui, t, &FontId::new(*s, self.o.family.clone()), *c, false, 0.0, None);
        }
    }

    /// Flow `elems` at `avail`, allocate the space at the cursor (indented by `indent`) and paint.
    fn place(&self, ui: &mut Ui, elems: &[Elem], indent: f32, avail: f32) -> (Rect, Option<EqHit>) {
        let size = self.o.size;
        let leading = size * self.o.leading;
        let mut lines = flow(elems, avail, size, self.pad());
        if self.center {
            for l in &mut lines {
                let dx = ((avail - l.w) / 2.0).max(0.0);
                for p in &mut l.items {
                    p.x += dx;
                }
            }
        }
        let h = flow_height(&lines, leading).max(size);
        let full = ui.available_width().max(indent + avail);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(full, h), Sense::hover());
        let origin = rect.left_top() + Vec2::new(indent, 0.0);
        let painter = ui.painter_at(rect.expand2(Vec2::new(2.0, 6.0)));
        let hit = paint_flow(ui, &painter, origin, &lines, elems, leading, self.pad(), self.o.color, self.o.editable);
        (rect, hit)
    }
}

/// Render a stem with its markup (`crate::richtext`): paragraphs with hard line breaks, bullet
/// and numbered lists with a hanging indent, quotes, bordered tables (header row in bold),
/// bold/italic/strikethrough, sub/superscripts, `___` blanks as writing lines and typeset `$math$`.
pub fn rich_ui(ui: &mut Ui, text: &str, o: &Rich) -> (Response, Option<EqHit>) {
    let avail = ui.available_width().max(40.0);
    let mut rc = RichCtx { o, ctx: ui.ctx().clone(), spans: math_spans(text), next_span: 0, center: false };
    let mut hit = None;
    let blocks = crate::richtext::parse_tree(text);
    let resp = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            hit = blocks_ui(ui, &mut rc, &blocks, avail, true);
        })
        .response;
    (resp, hit)
}

/// Column widths for a table: natural widths when they fit; otherwise narrow columns keep theirs
/// and the wide ones share what is left in proportion to their natural width.
pub fn table_widths(natural: &[f32], avail: f32) -> Vec<f32> {
    let total: f32 = natural.iter().sum();
    if total <= avail || natural.is_empty() {
        return natural.to_vec();
    }
    let n = natural.len() as f32;
    let share = avail / n;
    let narrow: f32 = natural.iter().filter(|w| **w <= share).sum();
    let wide: f32 = natural.iter().filter(|w| **w > share).sum();
    let left = (avail - narrow).max(0.0);
    natural.iter().map(|w| if *w <= share { *w } else { w / wide * left }).collect()
}

fn table_ui(ui: &mut Ui, rc: &mut RichCtx, header: bool, rows: &[Vec<crate::richtext::Line>], avail: f32) -> Option<EqHit> {
    use crate::richtext::line_text;
    let size = rc.o.size;
    let color = rc.o.color;
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncols == 0 {
        return None;
    }
    let hpad = size * 0.45;
    let vpad = size * 0.28;
    let blank_cell = |l: &crate::richtext::Line| {
        let t = line_text(l);
        let t = t.trim();
        !t.is_empty() && t.chars().all(|c| c == '_')
    };
    // Natural widths (unwrapped). A cell that is only `___` is an empty write-in box.
    let write_w = size * 7.0;
    let saved = rc.next_span;
    let mut nat = vec![size * 2.0; ncols];
    for (ri, row) in rows.iter().enumerate() {
        for (ci, cell) in row.iter().enumerate() {
            let w = if blank_cell(cell) {
                write_w
            } else {
                let mut e = Vec::new();
                rc.line_elems(ui, &mut e, cell, f32::INFINITY, header && ri == 0, true);
                natural_width(&e, rc.pad())
            };
            nat[ci] = nat[ci].max(w + 2.0 * hpad);
        }
    }
    rc.next_span = saved;
    let widths = table_widths(&nat, avail - 1.0);
    let leading = size * rc.o.leading;
    // Lay out every cell at its column width.
    let mut laid: Vec<Vec<(Vec<Elem>, Vec<FLine>)>> = Vec::new();
    let mut heights: Vec<f32> = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        let mut cells = Vec::new();
        let mut h = size;
        for ci in 0..ncols {
            let inner = (widths[ci] - 2.0 * hpad).max(size);
            let mut e = Vec::new();
            if let Some(cell) = row.get(ci)
                && !blank_cell(cell)
            {
                rc.line_elems(ui, &mut e, cell, inner, header && ri == 0, true);
            }
            let lines = flow(&e, inner, size, rc.pad());
            h = h.max(flow_height(&lines, leading));
            cells.push((e, lines));
        }
        laid.push(cells);
        heights.push(h + 2.0 * vpad);
    }
    let tw: f32 = widths.iter().sum();
    let th: f32 = heights.iter().sum();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(tw), th), Sense::hover());
    let painter = ui.painter_at(rect.expand(2.0));
    let ppp = ui.pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp + 0.5 / ppp;
    let table = Rect::from_min_size(rect.left_top(), Vec2::new(tw, th));
    let stroke = egui::Stroke::new(1.0, color.gamma_multiply(0.7));
    let mut hit = None;
    let mut y = table.top();
    for (ri, cells) in laid.iter().enumerate() {
        let mut x = table.left();
        for (ci, (e, lines)) in cells.iter().enumerate() {
            let inner = widths[ci] - 2.0 * hpad;
            // Header cells are centred, body cells left-aligned.
            let lw = lines.iter().map(|l| l.w).fold(0.0, f32::max);
            let dx = if header && ri == 0 { ((inner - lw) / 2.0).max(0.0) } else { 0.0 };
            let o = Pos2::new(x + hpad + dx, y + vpad);
            hit = hit.or(paint_flow(ui, &painter, o, lines, e, leading, rc.pad(), color, rc.o.editable));
            x += widths[ci];
        }
        y += heights[ri];
        if ri + 1 < laid.len() {
            painter.hline(table.x_range(), snap(y), stroke);
        }
    }
    let mut x = table.left();
    for w in &widths[..ncols - 1] {
        x += w;
        painter.vline(snap(x), table.y_range(), stroke);
    }
    painter.rect_stroke(Rect::from_min_max(Pos2::new(snap(table.left()), snap(table.top())), Pos2::new(snap(table.right()), snap(table.bottom()))), 0.0, stroke, egui::StrokeKind::Middle);
    hit
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

    /// The page preview sets italic runs in the real Source Serif 4 italic (not a slanted
    /// upright), and its serif advances are the PDF's: a word measured by egui equals the
    /// same word measured with the PDF's width tables (`crate::fonts::metrics`). egui also
    /// kerns and forms ligatures, so long preview lines can run a little narrower.
    #[test]
    fn preview_uses_real_italics_with_the_pdf_metrics() {
        use crate::fonts::{metrics, Style};
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        let pdf_w = |s: Style, t: &str, size: f32| t.bytes().map(|b| metrics(s).widths[b as usize] as f32).sum::<f32>() * size / 1000.0;
        let mut checked = false;
        for i in 0..2 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 600.0)));
            input.time = Some(i as f64 * 0.1);
            let out = ctx.run_ui(input, |ui| {
                if i == 0 {
                    return; // fonts set in setup() apply from the next pass
                }
                let size = 40.0;
                // No ligature or kerning pair: egui shapes with kerning and ligatures, the PDF
                // draws plain advances (it measures exactly what it draws, see export.rs).
                let word = "minimum distance";
                let it = theme::italic_family(ui.ctx(), &theme::serif_family()).expect("serif italic family registered");
                let bit = theme::italic_family(ui.ctx(), &theme::serif_bold_family()).expect("serif bold italic family registered");
                assert_eq!(it, FontFamily::Name("vs-serif-italic".into()));
                assert_eq!(bit, FontFamily::Name("vs-serif-bold-italic".into()));
                for (fam, style) in [(theme::serif_family(), Style::Regular), (theme::serif_bold_family(), Style::Bold), (it.clone(), Style::Italic), (bit.clone(), Style::BoldItalic)] {
                    let g = ui.painter().layout_no_wrap(word.into(), FontId::new(size, fam.clone()), Color32::BLACK);
                    let want = pdf_w(style, word, size);
                    assert!((g.size().x - want).abs() < want * 0.01 + 1.0, "{fam:?}: egui {} vs pdf {want}", g.size().x);
                }
                // word_galley with italics=true picks the italic face: same width as the italic
                // family, and no synthetic slant flag left on the glyph section.
                let g = word_galley(ui, word, &FontId::new(size, theme::serif_family()), Color32::BLACK, true, Deco::default());
                assert!((g.size().x - pdf_w(Style::Italic, word, size)).abs() < 1.5, "{}", g.size().x);
                assert!(g.job.sections.iter().all(|s| !s.format.italics), "real italic face, not a slant");
                assert_ne!(pdf_w(Style::Italic, word, size), pdf_w(Style::Regular, word, size));
                checked = true;
            });
            out.drop_without_applying_deltas();
        }
        assert!(checked);
    }

    #[test]
    fn blanks_split_out_of_words() {
        assert_eq!(split_blanks("___"), vec![("___", true)]);
        assert_eq!(split_blanks("x___. "), vec![("x", false), ("___", true), (". ", false)]);
        assert_eq!(split_blanks("___  "), vec![("___", true), ("  ", false)]);
        assert_eq!(split_blanks("a_b"), vec![("a_b", false)]);
    }

    #[test]
    fn table_columns_fit_or_share() {
        assert_eq!(table_widths(&[50.0, 80.0], 300.0), vec![50.0, 80.0]);
        let w = table_widths(&[40.0, 400.0, 400.0], 300.0);
        assert_eq!(w[0], 40.0);
        assert!((w.iter().sum::<f32>() - 300.0).abs() < 0.01, "{w:?}");
        assert!((w[1] - w[2]).abs() < 0.01);
    }

    #[test]
    fn stem_preview_is_one_flat_line() {
        assert_eq!(stem_preview("Name:\nA | B\nKI | ___"), "Name: A | B · KI | ___");
        assert_eq!(stem_preview("**Find** $x^2$"), "Find x²");
        assert_eq!(stem_preview("ΔH^{2+} and H_{2}O, x_{ab}"), "ΔH²⁺ and H₂O, x_ab");
    }

    /// Structured stems through real egui frames: every block kind, a clickable equation inside
    /// a table cell, and the clicked-chip span pointing at the right `$..$` in the source.
    #[test]
    fn rich_frames() {
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        let text = "Lead **bold** *it* H_{2}O x^{2} ___ $x^2$\n• one\n• two $a/b$\n1. a\n2. b\n> quote\nh1 | h2\n---|---\nKI | $y$\nNO | ___";
        let spans = math_spans(text);
        assert_eq!(spans.len(), 3);
        let mut heights = Vec::new();
        for i in 0..3 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 900.0)));
            input.time = Some(i as f64 * 0.1);
            let out = ctx.run_ui(input, |ui| {
                for w in [60.0, 200.0, 700.0] {
                    ui.allocate_ui(egui::vec2(w, 10.0), |ui| {
                        let o = Rich::new(14.0, Color32::BLACK).editable(true).lead("(1 pt) ", 12.0, Color32::GRAY);
                        let (r, _) = rich_ui(ui, text, &o);
                        heights.push(r.rect.height());
                        rich_ui(ui, "| a | b |\n| c | d |", &Rich::new(14.0, Color32::BLACK));
                        rich_ui(ui, "", &Rich::new(14.0, Color32::BLACK));
                    });
                }
            });
            out.drop_without_applying_deltas();
        }
        // Narrower columns wrap into taller blocks.
        assert!(heights[0] > heights[2], "{heights:?}");
    }
}
