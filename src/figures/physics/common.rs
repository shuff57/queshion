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

/// Arrowhead length for a shaft of width `sw`: bold heads, about 5.5 shaft widths long.
pub fn head_len(sw: f64) -> f64 {
    (sw * 5.5).max(12.0)
}

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


/// Splits a label into (text, level) runs: level 0 base, -1 subscript, 1 superscript. A leading
/// `\vec{..}` or `\mathbf{..}` wrapper overrides the style.
pub fn parse_tex(s: &str, st: u8) -> (Vec<(String, i8)>, u8) {
    let mut s = s.trim().to_string();
    let mut st = st;
    for (pre, sty) in [("\\vec{", VEC), ("\\mathbf{", BOLD), ("\\mathrm{", UPRIGHT), ("\\text{", UPRIGHT)] {
        if let Some(rest) = s.strip_prefix(pre) {
            if let Some(i) = rest.find('}') {
                s = format!("{}{}", &rest[..i], &rest[i + 1..]);
                st = sty;
            }
        }
    }
    let s = s.replace('\\', "");
    let mut runs: Vec<(String, i8)> = Vec::new();
    let push = |runs: &mut Vec<(String, i8)>, t: String, l: i8| {
        if t.is_empty() { return; }
        if let Some(last) = runs.last_mut() { if last.1 == l { last.0.push_str(&t); return; } }
        runs.push((t, l));
    };
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut base = String::new();
    while i < cs.len() {
        let c = cs[i];
        if (c == '_' || c == '^') && i + 1 < cs.len() {
            push(&mut runs, greek_words(&base), 0);
            base.clear();
            let lvl = if c == '_' { -1 } else { 1 };
            let mut t = String::new();
            if cs[i + 1] == '{' {
                let mut j = i + 2;
                while j < cs.len() && cs[j] != '}' { t.push(cs[j]); j += 1; }
                i = j + 1;
            } else if c == '_' {
                // a bare subscript runs to the next space or ^
                let mut j = i + 1;
                while j < cs.len() && cs[j] != ' ' && cs[j] != '^' && cs[j] != '_' { t.push(cs[j]); j += 1; }
                i = j;
            } else {
                let mut j = i + 1;
                if cs[j] == '-' { t.push('-'); j += 1; }
                while j < cs.len() && (cs[j].is_alphanumeric()) { t.push(cs[j]); j += 1; if !cs[j - 1].is_ascii_digit() { break; } }
                i = j;
            }
            push(&mut runs, greek_words(&t).replace('-', "−"), lvl);
        } else {
            base.push(c);
            i += 1;
        }
    }
    push(&mut runs, greek_words(&base), 0);
    if runs.is_empty() { runs.push((String::new(), 0)); }
    (runs, st)
}

/// Replaces whole alphabetic words that name a Greek letter.
pub fn greek_words(s: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    for c in s.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_alphabetic() { word.push(c); continue; }
        if !word.is_empty() { out.push_str(&greek(&word)); word.clear(); }
        if c != '\0' { out.push(c); }
    }
    out
}

/// Text of a label with a Text / Blank line / None mode param: "" draws a blank line and
/// "none" draws nothing (the conventions of `G::label_dir` / `G::label_at`).
pub fn lab_mode(p: &crate::figures::Params, text_key: &str, mode_key: &str) -> String {
    match p.s(mode_key) {
        "Blank line" => String::new(),
        "None" => "none".into(),
        _ => p.s(text_key).to_string(),
    }
}

/// Chart title from `title` + `title_mode` (Text / Blank line / None). Returns true when
/// something was drawn. An empty title in Text mode draws nothing (the old behaviour).
pub fn draw_title(g: &mut G, p: &crate::figures::Params, cx: f64, y: f64) -> bool {
    match p.s("title_mode") {
        "None" => false,
        "Blank line" => { g.line(cx - 120.0, y + 2.0, cx + 120.0, y + 2.0, INK, 1.5); true }
        _ => {
            let t = p.s("title").trim();
            if t.is_empty() { return false; }
            g.title_text(cx, y, 19.0, t);
            true
        }
    }
}

/// True when the chart title takes up room.
pub fn has_title(p: &crate::figures::Params) -> bool {
    match p.s("title_mode") { "None" => false, "Blank line" => true, _ => !p.s("title").trim().is_empty() }
}

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
    pub mir: bool,
    /// Draw `tex` labels with a white halo (for labels over gridlines).
    pub halo: bool,
    /// Fill colour of `tex` labels.
    pub tcol: &'static str,
}

impl G {
    pub fn new(w: f64, h: f64, mir: bool) -> Self { G { d: Doc::new(w, h), w, mir, halo: false, tcol: "#111" } }
    pub fn x(&self, x: f64) -> f64 { if self.mir { self.w - x } else { x } }
    /// Text anchor after mirroring (start and end swap so labels stay on the same side).
    pub fn anc<'a>(&self, a: &'a str) -> &'a str {
        if !self.mir { return a; }
        match a { "start" => "end", "end" => "start", o => o }
    }
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

    /// Force / vector arrow: the shaft stops inside the head (`head_len`), as the reference
    /// draws them with a bold, wide head.
    pub fn arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let hl = head_len(sw).min(len * 0.6);
        let (bx, by) = (x2 - dx / len * hl * 0.8, y2 - dy / len * hl * 0.8);
        self.line(x1, y1, bx, by, c, sw);
        self.head(x1, y1, x2, y2, c, sw);
    }
    /// Arrowhead only, at (x2,y2) pointing away from (x1,y1).
    pub fn head(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / len, dy / len);
        let hl = head_len(sw).min(len * 0.6);
        let (bx, by) = (x2 - ux * hl, y2 - uy * hl);
        self.poly(&[(x2, y2), (bx - uy * hl * 0.45, by + ux * hl * 0.45), (bx + uy * hl * 0.45, by - ux * hl * 0.45)], c, c, 0.5);
    }
    pub fn dash_arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, c: &str, sw: f64) {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let hl = head_len(sw).min(len * 0.6);
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
        let anchor = self.anc(anchor);
        let wt = if bold { "bold" } else { "normal" };
        let _ = write!(self.d_raw(), r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-weight="{wt}" font-family="{SANS}" fill="#111">{}</text>"##, esc(s));
    }
    /// Bold sans title text that understands `^2` / `_x` like `tex` (upright).
    pub fn title_text(&mut self, x: f64, y: f64, size: f64, s: &str) {
        let (runs, _) = parse_tex(&supers(s), UPRIGHT);
        let x = self.x(x);
        let mut body = String::new();
        let mut cur = 0i8;
        for (t, lvl) in &runs {
            let dy = (*lvl - cur) as f64 * -size * 0.3;
            let fs = if *lvl == 0 { size } else { size * 0.7 };
            let _ = write!(body, r#"<tspan font-size="{fs:.1}" dy="{dy:.1}">{}</tspan>"#, esc(t));
            cur = *lvl;
        }
        let _ = write!(self.d_raw(), r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="middle" font-weight="bold" font-family="{SANS}" fill="#111">{body}</text>"##);
    }
    /// Text rotated -90 degrees about its anchor (y-axis titles).
    pub fn vtext(&mut self, x: f64, y: f64, size: f64, s: &str, bold: bool) {
        let x = self.x(x);
        let wt = if bold { "bold" } else { "normal" };
        let _ = write!(self.d_raw(), r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="middle" font-weight="{wt}" font-family="{SANS}" fill="#111" transform="rotate(-90 {x:.2} {y:.2})">{}</text>"##, esc(s));
    }
    /// Math-style label: italic serif. `_x` / `_{xy}` subscript, `^x` / `^{xy}` superscript,
    /// whole words like `theta` become Greek, `\vec{F}` / `\mathbf{F}` switch the style.
    pub fn tex(&mut self, x: f64, y: f64, size: f64, anchor: &str, s: &str, st: u8) {
        let (runs, st) = parse_tex(s, st);
        let x = self.x(x);
        let anchor = self.anc(anchor);
        let style = if st == BOLD || st == UPRIGHT { "normal" } else { "italic" };
        let wt = if st == BOLD { "bold" } else { "normal" };
        let mut body = String::new();
        let mut cur = 0i8;
        let mut base_w = 0.0;
        let mut total = 0.0;
        for (t, lvl) in &runs {
            let dy = match (cur, *lvl) {
                (a, b) if a == b => 0.0,
                (0, -1) => size * 0.2, (0, 1) => -size * 0.38,
                (-1, 0) => -size * 0.2, (1, 0) => size * 0.38,
                (-1, 1) => -size * 0.58, _ => size * 0.58,
            };
            let fs = if *lvl == 0 { size } else { size * 0.66 };
            // digits, and word subscripts such as "air" or "app", stay upright; single letters are italic
            let word = *lvl == -1 && t.chars().count() >= 2 && t.chars().all(|c| c.is_ascii_lowercase());
            let fst = if *lvl != 0 && (word || t.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '+')) { "normal" } else { style };
            let _ = write!(body, r#"<tspan font-size="{fs:.1}" dy="{dy:.1}" font-style="{fst}">{}</tspan>"#, esc(t));
            let w = t.chars().count() as f64 * fs * if *lvl == 0 { 0.55 } else { 0.62 };
            if *lvl == 0 && total == base_w { base_w += w; }
            total += w;
            cur = *lvl;
        }
        let (tc, halo) = (self.tcol, self.halo);
        let _ = write!(self.d_raw(), r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-style="{style}" font-weight="{wt}" font-family="{SERIF}" fill="{}"{}>{body}</text>"##, tc, if halo { r##" stroke="#fff" stroke-width="5" stroke-linejoin="round" paint-order="stroke""## } else { "" });
        if st == VEC {
            let bw = base_w.max(size * 0.5);
            let left = match anchor { "start" => x, "end" => x - total, _ => x - total / 2.0 };
            let (a, b) = (left + bw * 0.05, left + bw * 0.95 + 2.0);
            let ay = y - size * 0.92;
            let c = self.tcol;
            let _ = write!(self.d_raw(), r##"<polyline points="{a:.1},{ay:.1} {b:.1},{ay:.1}" stroke="{c}" stroke-width="1.3" fill="none"/><polygon points="{b:.1},{ay:.1} {:.1},{:.1} {:.1},{:.1}" fill="{c}"/>"##, b - 4.5, ay - 2.4, b - 4.5, ay + 2.4);
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
}

/// "m/s^2" -> "m/s²" for the common cases.
pub fn supers(s: &str) -> String {
    s.replace("^2", "²").replace("^3", "³").replace("^-1", "⁻¹").replace("^-2", "⁻²")
}
