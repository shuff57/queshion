//! Shared output conventions for the chemistry figures (modelled on the reference sites):
//! a chart-title band above the figure (Text / Blank line / None), an answer-key footer under
//! it, and a line-graph frame (axis ranges, "count by", numbered every n-th line, minor grid,
//! axis titles, end labels and end markers) used by the mass spectrum, titration curve and
//! heating/cooling curve.
use super::*;
use crate::figures::{ParamDef, Params};

pub const TITLE3: [&str; 3] = ["Text", "Blank line", "None"];
pub const TITLE2: [&str; 2] = ["Text", "None"];
pub const NUMS: [&str; 5] = ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"];
pub const ENDS: [&str; 4] = ["None", "Triangle arrow", "Line arrow", "Circle"];
pub const MINOR: [&str; 4] = ["None", "2 per block", "4 per block", "5 per block"];
pub const LABEL2: [&str; 2] = ["Text", "None"];

/// `title_style` param ("Chart title"). With "Text" an empty `title` draws nothing, so old
/// saved figures (no title) look the same.
pub fn title_style(three: bool) -> ParamDef {
    ParamDef::choice("title_style", "Chart title", if three { &TITLE3 } else { &TITLE2 }, "Text")
}

/// The "Answer key" switch; `hint` is the old parenthetical, now shown on hover.
pub fn answer_param(hint: &'static str) -> ParamDef { ParamDef::bool("answer", "Answer key", false).hint(hint) }

/// Height of the band the wrapper adds above the figure for the chart title.
pub fn title_band(p: &Params) -> f64 {
    match p.s("title_style") {
        "None" => 0.0,
        "Blank line" => 44.0,
        _ => if p.s("title").trim().is_empty() { 0.0 } else { 44.0 },
    }
}

/// Font size that keeps `s` (approx. width 0.55 em per char) within `w`.
fn fit_font(s: &str, base: f64, w: f64) -> f64 {
    let n = s.chars().count().max(1) as f64;
    base.min((w - 16.0) / (n * 0.55)).max(8.0)
}

/// Adds the chart-title band (from `title_style` + `title`) and the answer-key lines under the
/// figure. Content is only shifted down (never sideways), so handles shift by `title_band`.
pub fn wrap(p: &Params, svg: String, footer: &[String]) -> String {
    let top = title_band(p);
    let foot: Vec<&String> = footer.iter().filter(|s| !s.trim().is_empty()).collect();
    if top == 0.0 && foot.is_empty() {
        return svg;
    }
    let (w, h) = crate::figures::svg_size(&svg);
    let marker = r##"fill="#fff"/>"##;
    let Some(i) = svg.find(marker) else { return svg };
    let body = &svg[i + marker.len()..svg.len().saturating_sub(6)];
    let fh = if foot.is_empty() { 0.0 } else { 14.0 + 26.0 * foot.len() as f64 };
    let nh = h + top + fh;
    let mut d = Doc::new(w, nh);
    if top > 0.0 {
        if p.s("title_style") == "Blank line" {
            let lw = (w * 0.55).min(320.0);
            d.line(w / 2.0 - lw / 2.0, 32.0, w / 2.0 + lw / 2.0, 32.0, INK, 1.4);
        } else {
            let t = p.s("title").trim();
            tx(&mut d, w / 2.0, 29.0, fit_font(t, 20.0, w), "middle", INK, true, t);
        }
    }
    d.raw(&format!(r#"<g transform="translate(0 {top})">{body}</g>"#));
    for (k, line) in foot.iter().enumerate() {
        let y = h + top + 4.0 + 26.0 * (k as f64 + 0.75);
        tx(&mut d, w / 2.0, y, fit_font(line, 17.0, w), "middle", INK, false, line);
    }
    d.finish()
}

/// Trimmed number for tick labels: integers without decimals, otherwise up to 3 places.
pub fn tick(v: f64) -> String {
    if (v - v.round()).abs() < 1e-7 {
        return num(v.round(), 0);
    }
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

// ---------------------------------------------------------------------------------------------
// Graph frame
// ---------------------------------------------------------------------------------------------

/// Default ranges for a figure's axes: (from, to, count by, numbers choice).
pub struct AxisDefaults { pub from: f64, pub to: f64, pub by: f64, pub num: &'static str }

/// x-axis params: `xaxis_from/_to/_by/_num/_lab/_labtext/_lo/_hi`.
pub fn xaxis_params(a: AxisDefaults, lim: f64) -> Vec<ParamDef> {
    vec![
        ParamDef::float("xaxis_from", "x-axis: from", -lim, lim, 0.1, a.from),
        ParamDef::float("xaxis_to", "x-axis: to", -lim, lim, 0.1, a.to),
        ParamDef::float("xaxis_by", "x-axis: count by", 0.01, lim, 0.01, a.by),
        ParamDef::choice("xaxis_num", "x-axis: numbers", &NUMS, a.num),
        ParamDef::choice("xaxis_lab", "x-axis: label at the right end", &LABEL2, "None"),
        ParamDef::text("xaxis_lab_text", "x-axis: right-end label text", "x"),
        ParamDef::choice("xaxis_lo", "x-axis: left end", &ENDS, "None"),
        ParamDef::choice("xaxis_hi", "x-axis: right end", &ENDS, "None"),
    ]
}

/// y-axis params: `yaxis_from/_to/_by/_num/_lab/_labtext/_lo/_hi`.
pub fn yaxis_params(a: AxisDefaults, lim: f64) -> Vec<ParamDef> {
    vec![
        ParamDef::float("yaxis_from", "y-axis: from", -lim, lim, 0.1, a.from),
        ParamDef::float("yaxis_to", "y-axis: to", -lim, lim, 0.1, a.to),
        ParamDef::float("yaxis_by", "y-axis: count by", 0.01, lim, 0.01, a.by),
        ParamDef::choice("yaxis_num", "y-axis: numbers", &NUMS, a.num),
        ParamDef::choice("yaxis_lab", "y-axis: label at the top end", &LABEL2, "None"),
        ParamDef::text("yaxis_lab_text", "y-axis: top-end label text", "y"),
        ParamDef::choice("yaxis_lo", "y-axis: bottom end", &ENDS, "None"),
        ParamDef::choice("yaxis_hi", "y-axis: top end", &ENDS, "None"),
    ]
}

pub fn minor_param() -> ParamDef { ParamDef::choice("minor_grid", "Minor gridlines", &MINOR, "None") }

#[derive(Clone, Debug)]
pub struct Axis { pub from: f64, pub to: f64, pub by: f64, pub every: usize, pub lab: Option<String>, pub lo: String, pub hi: String }

impl Axis {
    /// Reads `<pre>_*` params. Degenerate ranges are repaired; too many lines are thinned by
    /// growing "count by" (1-2-5 steps) so the grid stays readable.
    pub fn read(p: &Params, pre: &str) -> Axis {
        let g = |k: &str| format!("{pre}_{k}");
        let every = match p.s(&g("num")) { "Every line" => 1, "Every 2nd line" => 2, "Every 5th line" => 5, "Every 10th line" => 10, "No numbers" => 0, _ => 1 };
        let lab = if p.s(&g("lab")) == "Text" { Some(p.s(&g("lab_text")).to_string()).filter(|s| !s.trim().is_empty()) } else { None };
        let mut a = Axis { from: p.f(&g("from")), to: p.f(&g("to")), by: p.f(&g("by")), every, lab, lo: p.s(&g("lo")).to_string(), hi: p.s(&g("hi")).to_string() };
        a.fix();
        a
    }
    pub fn fix(&mut self) {
        if !self.from.is_finite() { self.from = 0.0; }
        if !self.to.is_finite() || self.to <= self.from { self.to = self.from + 10.0; }
        if !(self.by > 0.0) { self.by = (self.to - self.from) / 10.0; }
        let mut k = 0;
        while (self.to - self.from) / self.by > 80.0 && k < 40 {
            self.by = grow(self.by);
            k += 1;
        }
    }
    pub fn lines(&self) -> Vec<f64> {
        let n = ((self.to - self.from) / self.by + 1e-6).floor() as i64;
        (0..=n).map(|k| self.from + k as f64 * self.by).collect()
    }
}

/// Next "nice" step after `s` in a 1-2-5 sequence.
pub fn grow(s: f64) -> f64 {
    let mag = 10f64.powf(s.log10().floor());
    let m = s / mag;
    if m < 1.5 { 2.0 * mag } else if m < 3.5 { 5.0 * mag } else { 10.0 * mag }
}

/// "Nice" step so that `range` splits into about `n` blocks.
pub fn nice(range: f64, n: f64) -> f64 {
    let raw = (range / n).max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] { if m * mag >= raw - 1e-12 { return m * mag; } }
    10.0 * mag
}

#[derive(Clone, Debug)]
pub struct Graph { pub x: Axis, pub y: Axis, pub minor: usize, pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64 }

impl Graph {
    pub fn px(&self, v: f64) -> f64 { self.x0 + (v - self.x.from) / (self.x.to - self.x.from) * (self.x1 - self.x0) }
    pub fn py(&self, v: f64) -> f64 { self.y1 - (v - self.y.from) / (self.y.to - self.y.from) * (self.y1 - self.y0) }
    pub fn vx(&self, px: f64) -> f64 { self.x.from + (px - self.x0) / (self.x1 - self.x0) * (self.x.to - self.x.from) }
    pub fn vy(&self, py: f64) -> f64 { self.y.from + (self.y1 - py) / (self.y1 - self.y0) * (self.y.to - self.y.from) }
}

/// Title shown for an axis: (style, text) from `<key>_style`-like params.
pub fn axis_title<'a>(p: &'a Params, style_key: &str, text: &'a str) -> Option<Option<&'a str>> {
    match p.s(style_key) {
        "None" => None,
        "Blank line" => Some(None),
        _ => if text.trim().is_empty() { None } else { Some(Some(text)) },
    }
}

/// Plot rectangle for a canvas `w`×`h`, leaving room for numbers and the axis titles.
pub fn plot_rect(w: f64, h: f64, x: &Axis, y: &Axis, has_xt: bool, has_yt: bool, top_extra: f64) -> (f64, f64, f64, f64) {
    let ywide = y.lines().iter().map(|v| tick(*v).len()).max().unwrap_or(2) as f64;
    let left = 16.0 + if has_yt { 34.0 } else { 0.0 } + if y.every > 0 { ywide * 9.5 + 12.0 } else { 6.0 };
    let right = 26.0 + if x.lab.is_some() { 8.0 + x.lab.as_ref().map_or(0, |s| s.chars().count()) as f64 * 9.0 } else { 0.0 };
    let top = 24.0 + top_extra + if y.lab.is_some() { 22.0 } else { 0.0 };
    let bottom = 14.0 + if x.every > 0 { 26.0 } else { 6.0 } + if has_xt { 36.0 } else { 0.0 };
    (left, w - right, top, h - bottom)
}

fn end_marker(d: &mut Doc, kind: &str, x: f64, y: f64, ux: f64, uy: f64) {
    match kind {
        "Triangle arrow" => {
            let (tx_, ty_) = (x + ux * 14.0, y + uy * 14.0);
            d.line(x, y, tx_ - ux * 4.0, ty_ - uy * 4.0, INK, 2.4);
            let (nx, ny) = (-uy, ux);
            d.poly(&[(tx_, ty_), (tx_ - ux * 12.0 + nx * 6.0, ty_ - uy * 12.0 + ny * 6.0), (tx_ - ux * 12.0 - nx * 6.0, ty_ - uy * 12.0 - ny * 6.0)], INK, INK, 1.0);
        }
        "Line arrow" => {
            let (tx_, ty_) = (x + ux * 14.0, y + uy * 14.0);
            d.line(x, y, tx_, ty_, INK, 2.4);
            let (nx, ny) = (-uy, ux);
            d.polyline(&[(tx_ - ux * 10.0 + nx * 7.0, ty_ - uy * 10.0 + ny * 7.0), (tx_, ty_), (tx_ - ux * 10.0 - nx * 7.0, ty_ - uy * 10.0 - ny * 7.0)], INK, 2.2);
        }
        "Circle" => { d.circle(x + ux * 5.0, y + uy * 5.0, 5.0, "#fff", INK, 2.0); }
        _ => {}
    }
}

/// Grid, axes, numbers, end markers/labels and axis titles. Data is drawn by the caller after.
pub fn draw_graph(d: &mut Doc, g: &Graph, xt: Option<Option<&str>>, yt: Option<Option<&str>>) {
    let (x0, x1, y0, y1) = (g.x0, g.x1, g.y0, g.y1);
    let xs = g.x.lines();
    let ys = g.y.lines();
    if g.minor > 1 {
        for w in xs.windows(2) {
            for k in 1..g.minor { let v = w[0] + (w[1] - w[0]) * k as f64 / g.minor as f64; d.line(g.px(v), y0, g.px(v), y1, "#c4c4c4", 0.6); }
        }
        for w in ys.windows(2) {
            for k in 1..g.minor { let v = w[0] + (w[1] - w[0]) * k as f64 / g.minor as f64; d.line(x0, g.py(v), x1, g.py(v), "#c4c4c4", 0.6); }
        }
    }
    for &v in &xs { d.line(g.px(v), y0, g.px(v), y1, "#2b2b2b", 1.1); }
    for &v in &ys { d.line(x0, g.py(v), x1, g.py(v), "#2b2b2b", 1.1); }
    d.line(x0, y1, x1, y1, INK, 2.6);
    d.line(x0, y0, x0, y1, INK, 2.6);
    if g.x.every > 0 {
        for (k, &v) in xs.iter().enumerate() {
            if k % g.x.every == 0 { tx(d, g.px(v), y1 + 24.0, 15.0, "middle", INK, true, &tick(v)); }
        }
    }
    if g.y.every > 0 {
        for (k, &v) in ys.iter().enumerate() {
            if k % g.y.every == 0 { tx(d, x0 - 9.0, g.py(v) + 5.5, 15.0, "end", INK, true, &tick(v)); }
        }
    }
    end_marker(d, &g.x.lo, x0, y1, -1.0, 0.0);
    end_marker(d, &g.x.hi, x1, y1, 1.0, 0.0);
    end_marker(d, &g.y.lo, x0, y1, 0.0, 1.0);
    end_marker(d, &g.y.hi, x0, y0, 0.0, -1.0);
    let xpad = if g.x.hi == "None" { 8.0 } else { 22.0 };
    if let Some(l) = &g.x.lab { tx(d, x1 + xpad, y1 + 5.5, 16.0, "start", INK, true, l); }
    let ypad = if g.y.hi == "None" { 10.0 } else { 22.0 };
    if let Some(l) = &g.y.lab { tx(d, x0, y0 - ypad, 16.0, "middle", INK, true, l); }
    let numh = if g.x.every > 0 { 26.0 } else { 4.0 };
    match xt {
        Some(Some(t)) => { tx(d, (x0 + x1) / 2.0, y1 + numh + 34.0, 17.0, "middle", INK, true, t); }
        Some(None) => { let lw = ((x1 - x0) * 0.5).min(260.0); d.line((x0 + x1) / 2.0 - lw / 2.0, y1 + numh + 34.0, (x0 + x1) / 2.0 + lw / 2.0, y1 + numh + 34.0, INK, 1.3); }
        None => {}
    }
    let ywide = ys.iter().map(|v| tick(*v).len()).max().unwrap_or(2) as f64;
    let yx = x0 - if g.y.every > 0 { ywide * 9.5 + 22.0 } else { 14.0 } - 8.0;
    match yt {
        Some(Some(t)) => { d.raw(&format!(r##"<text x="{yx:.2}" y="{:.2}" font-size="17" text-anchor="middle" font-family="{FONT}" font-weight="bold" fill="{INK}" transform="rotate(-90 {yx:.2} {:.2})">{}</text>"##, (y0 + y1) / 2.0, (y0 + y1) / 2.0, crate::figures::svg::esc(t))); }
        Some(None) => { let lh = ((y1 - y0) * 0.5).min(220.0); d.line(yx - 5.0, (y0 + y1) / 2.0 - lh / 2.0, yx - 5.0, (y0 + y1) / 2.0 + lh / 2.0, INK, 1.3); }
        None => {}
    }
}
