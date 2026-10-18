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
    for i in 0..=nx {
        let xx = a.px(a.x0 + i as f64 * a.xs);
        d.line(xx, a.y, xx, a.y + a.h, "#3a3a3a", 0.8);
    }
    for i in 0..=ny {
        let yy = a.py(a.y0 + i as f64 * a.ys);
        d.line(a.x, yy, a.x + a.w, yy, "#3a3a3a", 0.8);
    }
    d.rect(a.x, a.y, a.w, a.h, "none", INK, 1.6);
    let ex = o.xnum.unwrap_or(((nx as f64) / 11.0).ceil().max(1.0) as i64);
    let ey = o.ynum.unwrap_or(((ny as f64) / 12.0).ceil().max(1.0) as i64);
    let mut maxlab = 1usize;
    if ex > 0 {
        for i in (0..=nx).filter(|i| i % ex == 0) {
            let v = a.x0 + i as f64 * a.xs;
            bold(d, a.px(v), a.y + a.h + 18.0, 12.0, "middle", &fnum(v));
        }
    }
    if ey > 0 {
        for i in (0..=ny).filter(|i| i % ey == 0) {
            let v = a.y0 + i as f64 * a.ys;
            let l = fnum(v);
            maxlab = maxlab.max(l.chars().count());
            bold(d, a.x - 7.0, a.py(v) + 4.0, 12.0, "end", &l);
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
        _ => if !xt.is_empty() { bold(d, a.x + a.w / 2.0, a.y + a.h + 44.0, 15.0, "middle", xt); },
    }
    let cx = a.x - 14.0 - maxlab as f64 * 7.4;
    let cy = a.y + a.h / 2.0;
    match o.yt_mode.as_str() {
        "None" => {}
        "Blank line" => { d.line(cx - 4.0, cy - (a.h * 0.3).min(90.0), cx - 4.0, cy + (a.h * 0.3).min(90.0), INK, 1.3); }
        _ => if !yt.is_empty() {
            let fs = (a.h * 0.95 / (yt.chars().count().max(1) as f64 * 0.62)).min(15.0);
            txt(d, cx, cy, fs, "middle", SANS, &format!(r#"font-weight="bold" transform="rotate(-90 {cx:.1} {cy:.1})""#), yt);
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
    let widths: Vec<f64> = items.iter().map(|(n, _, _)| 46.0 + n.chars().count() as f64 * 8.2 + 26.0).collect();
    let total: f64 = widths.iter().sum();
    let mut x = cx - total / 2.0;
    for (i, (n, col, dash)) in items.iter().enumerate() {
        if dash.is_empty() {
            d.line(x, y, x + 36.0, y, col, 3.0);
        } else {
            dashed(d, &[(x, y), (x + 36.0, y)], col, 3.0, dash);
        }
        bold(d, x + 46.0, y + 4.5, 14.0, "start", n);
        x += widths[i];
    }
}
