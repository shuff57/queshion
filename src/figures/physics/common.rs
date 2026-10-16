//! Shared drawing helpers for the physics generators: a mirror-aware canvas wrapper, math-style
//! labels (italic with subscripts), arrows, arcs and graph axes.
use crate::figures::svg::{esc, Doc};
use std::fmt::Write;

pub const INK: &str = "#111111";
pub const FILL: &str = "#e9ebef";
pub const GREY: &str = "#6b7280";
pub const SERIF: &str = "Liberation Serif, Noto Serif, DejaVu Serif, Times New Roman, serif";
pub const SANS: &str = "Liberation Sans, Noto Sans, DejaVu Sans, Arial, sans-serif";
pub const SW: f64 = 2.5;

pub const PAL: [&str; 6] = ["#c0392b", "#1f5fbf", "#1e8449", "#b9770e", "#7d3c98", "#0e7c8b"];

/// Label styles for `G::tex`.
pub const ITALIC: u8 = 0;
pub const VEC: u8 = 1; // italic with an arrow overhead
pub const BOLD: u8 = 2;
pub const UPRIGHT: u8 = 3;

pub type Pt = (f64, f64);

/// Point at distance `r` from (cx,cy) at `deg` degrees, y-up angle convention.
pub fn pol(cx: f64, cy: f64, r: f64, deg: f64) -> Pt {
    let a = deg.to_radians();
    (cx + r * a.cos(), cy - r * a.sin())
}

pub fn arc_pts(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> Vec<Pt> {
    let n = (((a1 - a0).abs() / 4.0).ceil() as usize).max(2);
    (0..=n).map(|i| pol(cx, cy, r, a0 + (a1 - a0) * i as f64 / n as f64)).collect()
}

pub fn greek(s: &str) -> String {
    match s {
        "theta" => "θ", "lambda" => "λ", "alpha" => "α", "beta" => "β", "mu" => "μ", "phi" => "φ",
        "omega" => "ω", "Omega" => "Ω", "delta" => "δ", "Delta" => "Δ", "rho" => "ρ", "tau" => "τ",
        "sigma" => "σ", "pi" => "π", "gamma" => "γ", "epsilon" => "ε",
        other => other,
    }
    .to_string()
}

pub fn tw(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.58 }

/// Nice tick step (1, 2, 5 x 10^k) giving roughly `target` ticks over `range`.
pub fn nice_step(range: f64, target: f64) -> f64 {
    let raw = (range / target).max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    let m = if f <= 1.0 { 1.0 } else if f <= 2.0 { 2.0 } else if f <= 5.0 { 5.0 } else { 10.0 };
    m * mag
}

pub fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 { format!("{}", v.round() as i64) } else {
        let s = format!("{:.2}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Canvas wrapper: all x coordinates are mirrored when `mir` is set (text anchors are kept).
pub struct G {
    pub d: Doc,
    pub w: f64,
    pub h: f64,
    pub mir: bool,
}

impl G {
    pub fn new(w: f64, h: f64, mir: bool) -> Self { G { d: Doc::new(w, h), w, h, mir } }
    pub fn x(&self, x: f64) -> f64 { if self.mir { self.w - x } else { x } }
    pub fn finish(&self) -> String { self.d.finish() }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (a, b) = (self.x(x1), self.x(x2));
        self.d.line(a, y1, b, y2, c, sw);
    }
    pub fn dash(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (a, b) = (self.x(x1), self.x(x2));
        let _ = write!(self.d_raw(), r#"<line x1="{a:.2}" y1="{y1:.2}" x2="{b:.2}" y2="{y2:.2}" stroke="{c}" stroke-width="{sw}" stroke-dasharray="{:.1} {:.1}" stroke-linecap="butt"/>"#, sw * 2.4 + 2.0, sw * 2.0 + 2.5);
    }
    fn d_raw(&mut self) -> RawW<'_> { RawW(&mut self.d) }

    pub fn arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (a, b) = (self.x(x1), self.x(x2));
        self.d.arrow(a, y1, b, y2, c, sw);
    }
    /// Arrowhead only, at (x2,y2) pointing away from (x1,y1).
    pub fn head(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / len, dy / len);
        let hl = (sw * 4.0).max(9.0);
        let (bx, by) = (x2 - ux * hl, y2 - uy * hl);
        self.poly(&[(x2, y2), (bx - uy * hl * 0.4, by + ux * hl * 0.4), (bx + uy * hl * 0.4, by - ux * hl * 0.4)], c, c, 0.5);
    }
    pub fn dash_arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let hl = (sw * 4.0).max(9.0);
        let (bx, by) = (x2 - dx / len * hl * 0.8, y2 - dy / len * hl * 0.8);
        self.dash(x1, y1, bx, by, c, sw);
        self.head(x1, y1, x2, y2, c, sw);
    }
    pub fn dim_arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        self.line(x1, y1, x2, y2, c, sw);
        self.head(x2, y2, x1, y1, c, sw);
        self.head(x1, y1, x2, y2, c, sw);
    }
    pub fn poly(&mut self, pts: &[Pt], fill: &str, stroke: &str, sw: f64) {
        let p: Vec<Pt> = pts.iter().map(|&(x, y)| (self.x(x), y)).collect();
        self.d.poly(&p, fill, stroke, sw);
    }
    pub fn polyline(&mut self, pts: &[Pt], stroke: &str, sw: f64) {
        let p: Vec<Pt> = pts.iter().map(|&(x, y)| (self.x(x), y)).collect();
        self.d.polyline(&p, stroke, sw);
    }
    pub fn dash_polyline(&mut self, pts: &[Pt], stroke: &str, sw: f64) {
        let p: Vec<String> = pts.iter().map(|&(x, y)| format!("{:.2},{:.2}", self.x(x), y)).collect();
        let _ = write!(self.d_raw(), r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" stroke-dasharray="{:.1} {:.1}" stroke-linejoin="round"/>"#, p.join(" "), sw * 2.6 + 2.0, sw * 2.2 + 2.5);
    }
    pub fn pattern_polyline(&mut self, pts: &[Pt], stroke: &str, sw: f64, dash: &str) {
        let p: Vec<String> = pts.iter().map(|&(x, y)| format!("{:.2},{:.2}", self.x(x), y)).collect();
        let _ = write!(self.d_raw(), r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" stroke-dasharray="{dash}" stroke-linejoin="round"/>"#, p.join(" "));
    }
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64) {
        let x = self.x(cx);
        self.d.circle(x, cy, r, fill, stroke, sw);
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str, stroke: &str, sw: f64) {
        self.poly(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], fill, stroke, sw);
    }
    /// Sans text (axis numbers etc.). `bold` selects weight.
    pub fn text(&mut self, x: f64, y: f64, size: f64, anchor: &str, s: &str, bold: bool) {
        let x = self.x(x);
        let wt = if bold { "bold" } else { "normal" };
        let _ = write!(self.d_raw(), r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-weight="{wt}" font-family="{SANS}" fill="#111">{}</text>"##, esc(s));
    }
    /// Text rotated -90 degrees about its anchor (y-axis titles).
    pub fn vtext(&mut self, x: f64, y: f64, size: f64, s: &str, bold: bool) {
        let x = self.x(x);
        let wt = if bold { "bold" } else { "normal" };
        let _ = write!(self.d_raw(), r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="middle" font-weight="{wt}" font-family="{SANS}" fill="#111" transform="rotate(-90 {x:.2} {y:.2})">{}</text>"##, esc(s));
    }
    /// Math-style label: italic serif, `_` starts a subscript, `theta` etc. become Greek.
    pub fn tex(&mut self, x: f64, y: f64, size: f64, anchor: &str, s: &str, st: u8) {
        let (base, sub) = match s.split_once('_') {
            Some((b, r)) => (greek(b), r.trim_matches(|c| c == '{' || c == '}').to_string()),
            None => (greek(s), String::new()),
        };
        let x = self.x(x);
        let style = if st == BOLD || st == UPRIGHT { "normal" } else { "italic" };
        let wt = if st == BOLD { "bold" } else { "normal" };
        let subx = if sub.is_empty() { String::new() } else {
            format!(r#"<tspan font-size="{:.1}" dy="{:.1}" font-style="{}">{}</tspan>"#, size * 0.66, size * 0.2, if sub.chars().all(|c| c.is_ascii_digit()) { "normal" } else { style }, esc(&sub))
        };
        let _ = write!(self.d_raw(), r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-style="{style}" font-weight="{wt}" font-family="{SERIF}" fill="#111">{}{subx}</text>"##, esc(&base));
        if st == VEC {
            let bw = base.chars().count() as f64 * size * 0.55;
            let total = bw + sub.chars().count() as f64 * size * 0.42;
            let left = match anchor { "start" => x, "end" => x - total, _ => x - total / 2.0 };
            let (a, b) = (left + bw * 0.05, left + bw * 0.95);
            let ay = y - size * 0.92;
            let _ = write!(self.d_raw(), r##"<polyline points="{a:.1},{ay:.1} {b:.1},{ay:.1}" stroke="#111" stroke-width="1.3" fill="none"/><polygon points="{b:.1},{ay:.1} {:.1},{:.1} {:.1},{:.1}" fill="#111"/>"##, b - 4.5, ay - 2.4, b - 4.5, ay + 2.4);
        }
    }
    /// Label placed beyond a point `(px,py)` in direction `(ux,uy)` (pixel y-down).
    /// Empty text draws a blank answer line; "none" draws nothing.
    pub fn label_dir(&mut self, px: f64, py: f64, ux: f64, uy: f64, size: f64, s: &str, st: u8) {
        let t = s.trim();
        if t.eq_ignore_ascii_case("none") { return; }
        let off = 10.0;
        let (lx, ly) = (px + ux * off, py + uy * off);
        let anchor = if ux > 0.4 { "start" } else if ux < -0.4 { "end" } else { "middle" };
        let by = ly + size * 0.35 + uy * size * 0.55;
        if t.is_empty() {
            let (x0, x1) = match anchor { "start" => (lx, lx + 26.0), "end" => (lx - 26.0, lx), _ => (lx - 13.0, lx + 13.0) };
            self.line(x0, by + 2.0, x1, by + 2.0, INK, 1.5);
        } else {
            self.tex(lx, by, size, anchor, t, st);
        }
    }
    /// Label centred on a point.
    pub fn label_at(&mut self, cx: f64, cy: f64, size: f64, s: &str, st: u8) {
        let t = s.trim();
        if t.eq_ignore_ascii_case("none") { return; }
        if t.is_empty() { self.line(cx - 13.0, cy + size * 0.35, cx + 13.0, cy + size * 0.35, INK, 1.5); return; }
        self.tex(cx, cy + size * 0.35, size, "middle", t, st);
    }
    /// Ground / ceiling line with hatch marks. `down` hatches go below the line when true.
    pub fn hatch(&mut self, x0: f64, x1: f64, y: f64, down: bool) {
        self.line(x0, y, x1, y, INK, SW);
        let s = if down { 1.0 } else { -1.0 };
        let mut x = x0 + 4.0;
        while x < x1 + 6.0 {
            self.line(x, y, x - 9.0, y + 10.0 * s, INK, 1.4);
            x += 15.0;
        }
    }
    pub fn raw(&mut self, s: &str) { self.d.raw(s); }
}

struct RawW<'a>(&'a mut Doc);
impl<'a> Write for RawW<'a> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result { self.0.raw(s); Ok(()) }
}

/// Rectangular plot area with numbered, optionally gridded axes.
pub struct Axes {
    pub x0: f64, pub y0: f64, pub w: f64, pub h: f64,
    pub xmin: f64, pub xmax: f64, pub ymin: f64, pub ymax: f64,
}

impl Axes {
    pub fn px(&self, x: f64) -> f64 { self.x0 + (x - self.xmin) / (self.xmax - self.xmin) * self.w }
    pub fn py(&self, y: f64) -> f64 { self.y0 + self.h - (y - self.ymin) / (self.ymax - self.ymin) * self.h }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, g: &mut G, xstep: f64, ystep: f64, grid: bool, numbers: bool, xlabel: &str, ylabel: &str, fs: f64) {
        let (l, r, t, b) = (self.x0, self.x0 + self.w, self.y0, self.y0 + self.h);
        let ticks = |lo: f64, hi: f64, st: f64| -> Vec<f64> {
            let mut v = Vec::new();
            let mut k = (lo / st).ceil();
            while k * st <= hi + st * 1e-6 { v.push(k * st); k += 1.0; }
            v
        };
        for xv in ticks(self.xmin, self.xmax, xstep) {
            let x = self.px(xv);
            if grid { g.line(x, t, x, b, "#555", 0.8); } else { g.line(x, b, x, b - 6.0, INK, 1.4); }
            if numbers { g.text(x, b + fs + 6.0, fs, "middle", &fmt_num(xv), true); }
        }
        for yv in ticks(self.ymin, self.ymax, ystep) {
            let y = self.py(yv);
            if grid { g.line(l, y, r, y, "#555", 0.8); } else { g.line(l, y, l + 6.0, y, INK, 1.4); }
            if numbers { g.text(l - 8.0, y + fs * 0.35, fs, "end", &fmt_num(yv), true); }
        }
        // frame, left axis and zero line
        g.polyline(&[(l, t), (r, t), (r, b), (l, b), (l, t)], INK, 1.2);
        g.line(l, t, l, b, INK, 2.6);
        g.line(l, b, r, b, INK, 2.6);
        if self.ymin < 0.0 && self.ymax > 0.0 { let y = self.py(0.0); g.line(l, y, r, y, INK, 2.6); }
        if !xlabel.is_empty() { g.text((l + r) / 2.0, b + fs * 2.0 + 14.0, fs + 3.0, "middle", xlabel, true); }
        if !ylabel.is_empty() { g.vtext(l - fs * 3.2 - 8.0, (t + b) / 2.0, fs + 3.0, ylabel, true); }
    }
}

/// "m/s^2" -> "m/s²" for the common cases.
pub fn supers(s: &str) -> String {
    s.replace("^2", "²").replace("^3", "³").replace("^-1", "⁻¹").replace("^-2", "⁻²")
}
