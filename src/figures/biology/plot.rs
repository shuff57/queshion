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

/// Gridlines (major + optional minor), frame, tick numbers and axis titles.
pub fn draw_axes(d: &mut Doc, a: &Axes, minor: i64, xt: &str, yt: &str) {
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
    let ex = ((nx as f64) / 11.0).ceil().max(1.0) as i64;
    let ey = ((ny as f64) / 12.0).ceil().max(1.0) as i64;
    let mut maxlab = 1usize;
    for i in (0..=nx).filter(|i| i % ex == 0) {
        let v = a.x0 + i as f64 * a.xs;
        bold(d, a.px(v), a.y + a.h + 18.0, 12.0, "middle", &fnum(v));
    }
    for i in (0..=ny).filter(|i| i % ey == 0) {
        let v = a.y0 + i as f64 * a.ys;
        let l = fnum(v);
        maxlab = maxlab.max(l.chars().count());
        bold(d, a.x - 7.0, a.py(v) + 4.0, 12.0, "end", &l);
    }
    if !xt.is_empty() {
        bold(d, a.x + a.w / 2.0, a.y + a.h + 44.0, 15.0, "middle", xt);
    }
    if !yt.is_empty() {
        let fs = (a.h * 0.95 / (yt.chars().count().max(1) as f64 * 0.62)).min(15.0);
        let cx = a.x - 14.0 - maxlab as f64 * 7.4;
        let cy = a.y + a.h / 2.0;
        txt(d, cx, cy, fs, "middle", SANS, &format!(r#"font-weight="bold" transform="rotate(-90 {cx:.1} {cy:.1})""#), yt);
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
