//! Shared graph-paper style plot helper for the population graphs.
use super::*;

pub struct Axes {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    pub xs: f64,
    pub ys: f64,
}

impl Axes {
    pub fn px(&self, v: f64) -> f64 { self.x + (v - self.x0) / (self.x1 - self.x0) * self.w }
    pub fn py(&self, v: f64) -> f64 { self.y + self.h - (v - self.y0) / (self.y1 - self.y0) * self.h }
    pub fn pt(&self, p: (f64, f64)) -> (f64, f64) { (self.px(p.0), self.py(p.1)) }
}

/// Per-axis presentation options (reference-style axis controls).
#[derive(Clone)]
pub struct AxOpts {
    /// Number every n-th gridline: `None` = automatic, `Some(0)` = no numbers.
    pub xnum: Option<i64>,
    pub ynum: Option<i64>,
    /// Italic variable written past the right end of the x-axis / above the top of the y-axis.
    pub xend: String,
    pub yend: String,
    /// End markers: "None", "Triangle arrow", "Line arrow", "Circle".
    pub x_lo: String,
    pub x_hi: String,
    pub y_lo: String,
    pub y_hi: String,
    /// Titles: "Text", "Blank line" (a line to write on) or "None".
    pub xt_mode: String,
    pub yt_mode: String,
}

impl Default for AxOpts {
    fn default() -> Self {
        AxOpts {
            xnum: None, ynum: None, xend: String::new(), yend: String::new(),
            x_lo: "None".into(), x_hi: "None".into(), y_lo: "None".into(), y_hi: "None".into(),
            xt_mode: "Text".into(), yt_mode: "Text".into(),
        }
    }
}

/// "Every 2nd line" -> Some(2), "No numbers" -> Some(0), anything else -> None (automatic).
pub fn numbers_every(s: &str) -> Option<i64> {
    match s {
        "Every line" => Some(1),
        "Every 2nd line" => Some(2),
        "Every 5th line" => Some(5),
        "Every 10th line" => Some(10),
        "No numbers" => Some(0),
        _ => None,
    }
}

fn end_marker(d: &mut Doc, kind: &str, x: f64, y: f64, dx: f64, dy: f64) {
    // (dx, dy) = unit direction pointing outwards
    let (px, py) = (-dy, dx);
    match kind {
        "Triangle arrow" => {
            let (tx, ty) = (x + dx * 12.0, y + dy * 12.0);
            d.poly(&[(tx, ty), (x + px * 5.5, y + py * 5.5), (x - px * 5.5, y - py * 5.5)], INK, INK, 1.0);
            d.line(x - dx * 1.0, y - dy * 1.0, x, y, INK, 1.6);
        }
        "Line arrow" => {
            let (tx, ty) = (x + dx * 12.0, y + dy * 12.0);
            d.line(x, y, tx, ty, INK, 1.6);
            d.polyline(&[(tx - dx * 8.0 + px * 6.0, ty - dy * 8.0 + py * 6.0), (tx, ty), (tx - dx * 8.0 - px * 6.0, ty - dy * 8.0 - py * 6.0)], INK, 1.6);
        }
        "Circle" => {
            d.line(x, y, x + dx * 6.0, y + dy * 6.0, INK, 1.6);
            d.circle(x + dx * 10.0, y + dy * 10.0, 4.0, "#fff", INK, 1.6);
        }
        _ => {}
    }
}

/// Advance width (em) of `c` in Arial, regular or bold: the reference lays its text out with these.
pub fn arial_em(c: char, bold: bool) -> f64 {
    const LO: [u16; 26] = [556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500];
    const LO_B: [u16; 26] = [556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500];
    const UP: [u16; 26] = [667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611];
    const UP_B: [u16; 26] = [722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611];
    let v = match c {
        'a'..='z' => (if bold { LO_B } else { LO })[(c as u8 - b'a') as usize],
        'A'..='Z' => (if bold { UP_B } else { UP })[(c as u8 - b'A') as usize],
        '0'..='9' => 556,
        ' ' | '.' | ',' | '/' | '!' | '|' => 278,
        ':' | ';' => if bold { 333 } else { 278 },
        '(' | ')' | '-' => 333,
        '%' => 889,
        '\u{2013}' | '\u{2212}' => 556,
        _ => 600,
    };
    v as f64 / 1000.0
}

/// Advance width (em) of `c` in Times italic (the genotype face).
pub fn times_i_em(c: char) -> f64 {
    const LO: [u16; 26] = [500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444, 278, 722, 500, 500, 500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389];
    const UP: [u16; 26] = [611, 611, 667, 722, 611, 611, 722, 722, 333, 444, 667, 556, 833, 667, 722, 611, 722, 611, 500, 556, 722, 611, 833, 611, 556, 556];
    let v = match c {
        'a'..='z' => LO[(c as u8 - b'a') as usize],
        'A'..='Z' => UP[(c as u8 - b'A') as usize],
        '0'..='9' => 500,
        ' ' => 250,
        _ => 500,
    };
    v as f64 / 1000.0
}

/// Width of a tick number in the reference's charts: 0.6 em of its 14 px labels.
pub const TICK_CH: f64 = 8.4;

/// Characters in the widest y number the axis will print (so the left margin can be sized first).
pub fn y_label_chars(y0: f64, y1: f64, ys: f64, ynum: Option<i64>) -> usize {
    let ny = ((y1 - y0) / ys).round().max(1.0) as i64;
    let ey = ynum.unwrap_or(((ny as f64) / 12.0).ceil().max(1.0) as i64);
    if ey <= 0 { return 1; }
    (0..=ny).filter(|i| i % ey == 0).map(|i| fnum(y0 + i as f64 * ys).chars().count()).max().unwrap_or(1)
}

/// The reference's left plot margin: the y title column plus the widest tick number.
pub fn ref_left(chars: usize) -> f64 { 50.8 + TICK_CH * chars.max(1) as f64 }

/// Gridlines (major + optional minor), frame, tick numbers and axis titles; numbering
/// frequency, end labels, end markers and blank-line titles are in [`draw_axes_ex`].
pub fn draw_axes_ex(d: &mut Doc, a: &Axes, minor: i64, xt: &str, yt: &str, o: &AxOpts) {
    let nx = ((a.x1 - a.x0) / a.xs).round().max(1.0) as i64;
    let ny = ((a.y1 - a.y0) / a.ys).round().max(1.0) as i64;
    if minor > 1 {
        for i in 0..nx * minor + 1 {
            if i % minor == 0 { continue; }
            let xx = a.px(a.x0 + i as f64 * a.xs / minor as f64);
            d.line(xx, a.y, xx, a.y + a.h, "#b5b5b5", 0.5);
        }
        for i in 0..ny * minor + 1 {
            if i % minor == 0 { continue; }
            let yy = a.py(a.y0 + i as f64 * a.ys / minor as f64);
            d.line(a.x, yy, a.x + a.w, yy, "#b5b5b5", 0.5);
        }
    }
    // hairlines drawn without anti-aliasing, as the reference's grid group does
    d.raw(r#"<g shape-rendering="crispEdges">"#);
    for i in 0..=nx {
        let xx = a.px(a.x0 + i as f64 * a.xs);
        d.line(xx, a.y, xx, a.y + a.h, "#111827", 1.0);
    }
    for i in 0..=ny {
        let yy = a.py(a.y0 + i as f64 * a.ys);
        d.line(a.x, yy, a.x + a.w, yy, "#111827", 1.0);
    }
    d.raw("</g>");
    // the reference's axes: the left and bottom edges drawn heavier than the grid, no frame
    d.line(a.x, a.y + a.h, a.x + a.w, a.y + a.h, "#111827", 2.4);
    d.line(a.x, a.y + a.h, a.x, a.y, "#111827", 2.4);
    let ex = o.xnum.unwrap_or(((nx as f64) / 11.0).ceil().max(1.0) as i64);
    let ey = o.ynum.unwrap_or(((ny as f64) / 12.0).ceil().max(1.0) as i64);
    let mut maxlab = 1usize;
    // the reference prints one "0" at the origin, ending left of the corner, instead of one per axis
    let origin_once = ex > 0 && ey > 0 && a.x0 == 0.0 && a.y0 == 0.0;
    if ex > 0 {
        for i in (0..=nx).filter(|i| i % ex == 0) {
            let v = a.x0 + i as f64 * a.xs;
            if origin_once && i == 0 { bold(d, a.px(v) - 8.0, a.y + a.h + 20.0, 14.0, "end", &fnum(v)); }
            else { bold(d, a.px(v), a.y + a.h + 20.0, 14.0, "middle", &fnum(v)); }
        }
    }
    if ey > 0 {
        for i in (0..=ny).filter(|i| i % ey == 0) {
            let v = a.y0 + i as f64 * a.ys;
            let l = fnum(v);
            maxlab = maxlab.max(l.chars().count());
            if origin_once && i == 0 { continue; }
            bold(d, a.x - 6.0, a.py(v) + 4.9, 14.0, "end", &l);
        }
    }
    // end markers and end labels
    let (bx, by) = (a.x, a.y + a.h);
    end_marker(d, &o.x_hi, a.x + a.w, by, 1.0, 0.0);
    end_marker(d, &o.x_lo, bx, by, -1.0, 0.0);
    end_marker(d, &o.y_hi, bx, a.y, 0.0, -1.0);
    end_marker(d, &o.y_lo, bx, by, 0.0, 1.0);
    let xgap = if o.x_hi == "None" { 8.0 } else { 20.0 };
    let ygap = if o.y_hi == "None" { 8.0 } else { 20.0 };
    if !o.xend.is_empty() {
        txt(d, a.x + a.w + xgap, by + 5.0, 15.0, "start", SERIF, r#"font-style="italic" font-weight="bold""#, &o.xend);
    }
    if !o.yend.is_empty() {
        txt(d, bx, a.y - ygap, 15.0, "middle", SERIF, r#"font-style="italic" font-weight="bold""#, &o.yend);
    }
    match o.xt_mode.as_str() {
        "None" => {}
        "Blank line" => { d.line(a.x + a.w / 2.0 - 90.0, a.y + a.h + 48.0, a.x + a.w / 2.0 + 90.0, a.y + a.h + 48.0, INK, 1.3); }
        _ => if !xt.is_empty() { bold(d, a.x + a.w / 2.0, a.y + a.h + 44.8, 16.8, "middle", xt); },
    }
    let cx = a.x - 24.2 - maxlab as f64 * TICK_CH;
    let cy = a.y + a.h / 2.0;
    match o.yt_mode.as_str() {
        "None" => {}
        "Blank line" => { d.line(cx - 4.0, cy - (a.h * 0.3).min(90.0), cx - 4.0, cy + (a.h * 0.3).min(90.0), INK, 1.3); }
        _ => if !yt.is_empty() {
            let fs = (a.h * 0.95 / (yt.chars().count().max(1) as f64 * 0.62)).min(16.8);
            txt(d, cx, cy, fs, "middle", SANS, &format!(r#"font-weight="bold" dominant-baseline="central" transform="rotate(-90 {cx:.1} {cy:.1})""#), yt);
        },
    }
}

/// Start a clip group that confines drawing to the plot area; call `end_clip` afterwards.
pub fn begin_clip(d: &mut Doc, a: &Axes, id: &str) {
    d.raw(&format!(
        r#"<clipPath id="{id}"><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}"/></clipPath><g clip-path="url(#{id})">"#,
        a.x - 1.0, a.y - 1.0, a.w + 2.0, a.h + 2.0
    ));
}

pub fn end_clip(d: &mut Doc) { d.raw("</g>"); }

/// Horizontal legend of (name, color, dash) items centered on `cx`.
pub fn legend(d: &mut Doc, cx: f64, y: f64, items: &[(String, String, String)]) {
    // the reference: a 26 px sample, 6 px gap, 14.7 px bold name (0.6 em a character), 28 px between items
    let cw = 8.82;
    let widths: Vec<f64> = items.iter().map(|(n, _, _)| 26.0 + 6.0 + n.chars().count() as f64 * cw + 28.0).collect();
    let total: f64 = widths.iter().sum::<f64>() - 28.0;
    let mut x = cx - total / 2.0;
    for (i, (n, col, dash)) in items.iter().enumerate() {
        if dash.is_empty() {
            d.line(x, y, x + 26.0, y, col, 3.0);
        } else {
            dashed(d, &[(x, y), (x + 26.0, y)], col, 3.0, "7 4");
        }
        bold(d, x + 32.0, y + 4.92, 14.7, "start", n);
        x += widths[i];
    }
}
