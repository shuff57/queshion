//! Tiny SVG builder shared by all generators.
use std::fmt::Write;

pub struct Doc { w: f64, h: f64, body: String }

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl Doc {
    pub fn new(w: f64, h: f64) -> Self { Doc { w, h, body: String::new() } }
    pub fn raw(&mut self, s: &str) -> &mut Self { self.body.push_str(s); self }
    /// Shared `<defs>` (the print patterns of [`Fills`]); drawn before everything that uses them.
    pub fn defs(&mut self, s: &str) -> &mut Self { if !s.is_empty() { let _ = write!(self.body, "<defs>{s}</defs>"); } self }
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{stroke}" stroke-width="{sw}" stroke-linecap="round"/>"#);
        self
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#);
        self
    }
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#);
        self
    }
    pub fn poly(&mut self, pts: &[(f64, f64)], fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
        let _ = write!(self.body, r#"<polygon points="{}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round"/>"#, p.join(" "));
        self
    }
    pub fn polyline(&mut self, pts: &[(f64, f64)], stroke: &str, sw: f64) -> &mut Self {
        let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
        let _ = write!(self.body, r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"/>"#, p.join(" "));
        self
    }
    pub fn path(&mut self, d: &str, fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"/>"#);
        self
    }
    /// anchor: "start" | "middle" | "end"
    pub fn text(&mut self, x: f64, y: f64, size: f64, anchor: &str, s: &str) -> &mut Self {
        let _ = write!(self.body, r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="#000">{}</text>"##, esc(s));
        self
    }
    /// Arrow from (x1,y1) to (x2,y2) with a filled head.
    pub fn arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, sw: f64) -> &mut Self {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / len, dy / len);
        let hl = (sw * 4.0).max(9.0);
        let (bx, by) = (x2 - ux * hl, y2 - uy * hl);
        self.line(x1, y1, bx, by, color, sw);
        let pts = [(x2, y2), (bx - uy * hl * 0.4, by + ux * hl * 0.4), (bx + uy * hl * 0.4, by - ux * hl * 0.4)];
        self.poly(&pts, color, color, 0.5)
    }
    pub fn finish(&self) -> String {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="#fff"/>{b}</svg>"##,
            w = self.w, h = self.h, b = self.body
        )
    }
}

/// The reference's L/M/S label-size toggle: text size factor for a `label_size` choice
/// (Large is 1.4x and Small 0.8x Medium, as on the reference generators).
pub fn label_factor(choice: &str) -> f64 {
    match choice {
        "Large" => 1.4,
        "Small" => 0.8,
        _ => 1.0,
    }
}

/// The `label_size` param shared by the figures that get the toggle by scaling their output.
pub fn label_size_param() -> super::ParamDef {
    super::ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium")
}

/// Applies the `label_size` param to a finished SVG (`scale_text`); Medium returns it unchanged.
pub fn with_label_size(svg: String, p: &super::Params) -> String {
    let f = label_factor(p.s("label_size"));
    if f == 1.0 { svg } else { scale_text(&svg, f) }
}

/// Multiplies every numeric `font-size="…"` and `dy="…"` (superscript offsets, which generators
/// work out from the font size) by `f`, for generators that draw text at fixed sizes. Values in
/// other units (`0.35em`) are left alone, so they keep scaling with their font.
pub fn scale_text(svg: &str, f: f64) -> String {
    let mut out = String::with_capacity(svg.len() + 64);
    let mut rest = svg;
    loop {
        let next = ["font-size=\"", " dy=\""].iter().filter_map(|pat| rest.find(pat).map(|i| (i, *pat))).min_by_key(|(i, _)| *i);
        let Some((i, pat)) = next else {
            out.push_str(rest);
            break;
        };
        let start = i + pat.len();
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find('"').unwrap_or(rest.len());
        let val = &rest[..end];
        let num = val.trim_end_matches("px");
        match num.parse::<f64>() {
            Ok(v) => {
                let n = ((v * f) * 100.0).round() / 100.0;
                out.push_str(&format!("{n}"));
                if num.len() != val.len() {
                    out.push_str("px");
                }
            }
            Err(_) => out.push_str(val),
        }
        rest = &rest[end..];
    }
    out
}

/// The `style` param of the figures that draw filled series (bars, regions, categories): Colour
/// for the screen, Print for black-and-white exams (hatch patterns instead of colour fills).
pub fn style_param() -> super::ParamDef {
    super::ParamDef::choice("style", "Style", &["Colour", "Print"], "Colour").hint("Print swaps colour fills for hatching that survives a black-and-white copy")
}

const SERIES_COLOURS: [&str; 6] = ["#4f7cd4", "#e0803c", "#4aa36b", "#c9494f", "#8a62c4", "#d6b23a"];

/// Fills for series 0, 1, 2 ... in the figure's chosen style. Call [`Fills::defs`] once (through
/// [`Doc::defs`]) before using [`Fills::series`] in Print style.
#[derive(Clone, Copy, Debug)]
pub struct Fills { print: bool }

impl Fills {
    pub fn new(p: &super::Params) -> Self { Fills { print: p.s("style") == "Print" } }
    pub fn is_print(&self) -> bool { self.print }
    /// The `fill` for series `i` (wraps after six): a colour, or a pattern reference in Print.
    pub fn series(&self, i: usize) -> String {
        if self.print { format!("url(#vs-hatch{})", i % 6) } else { SERIES_COLOURS[i % 6].to_string() }
    }
    /// The same series as a stroke colour for lines: black in Print (the dash style tells them
    /// apart, see [`Fills::dash`]), the series colour otherwise.
    pub fn line(&self, i: usize) -> &'static str {
        if self.print { "#000" } else { SERIES_COLOURS[i % 6] }
    }
    /// `stroke-dasharray` value that tells line series apart in Print; empty in Colour.
    pub fn dash(&self, i: usize) -> &'static str {
        if !self.print { return ""; }
        ["", "6 3", "2 3", "8 3 2 3", "1 4", "10 4"][i % 6]
    }
    /// The pattern definitions for Print style; empty in Colour.
    pub fn defs(&self) -> String {
        if !self.print { return String::new(); }
        let line = |a: &str| format!(r##"<path d="{a}" stroke="#000" stroke-width="1" fill="none"/>"##);
        let pats: [(&str, String); 6] = [
            ("<rect width=\"8\" height=\"8\" fill=\"#d9d9d9\"/>", String::new()),
            ("", line("M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4")),
            ("", format!("{}{}", line("M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4"), line("M-2,6 l4,4 M0,0 l8,8 M6,-2 l4,4"))),
            ("", r##"<circle cx="2" cy="2" r="1.1" fill="#000"/><circle cx="6" cy="6" r="1.1" fill="#000"/>"##.to_string()),
            ("", line("M0,2 h8 M0,6 h8")),
            ("", line("M2,0 v8 M6,0 v8")),
        ];
        let mut out = String::new();
        for (i, (bg, fg)) in pats.iter().enumerate() {
            out.push_str(&format!(r##"<pattern id="vs-hatch{i}" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#fff"/>{bg}{fg}</pattern>"##));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_text_scales_sizes_and_offsets_only() {
        let svg = r#"<text font-size="14" x="3"><tspan font-size="10.0" dy="-5.0">2</tspan><tspan dy="0.35em">x</tspan></text><text font-size="20px">y</text><line stroke-width="2"/>"#;
        let big = scale_text(svg, 1.4);
        assert!(big.contains(r#"font-size="19.6""#), "{big}");
        assert!(big.contains(r#"font-size="14" dy="-7""#), "{big}");
        assert!(big.contains(r#"dy="-7""#), "{big}");
        assert!(big.contains(r#"dy="0.35em""#), "{big}");
        assert!(big.contains(r#"font-size="28px""#), "{big}");
        assert!(big.contains(r#"x="3""#) && big.contains(r#"stroke-width="2""#), "{big}");
        assert_eq!(scale_text(svg, 1.0).replace(".0\"", "\""), svg.replace(".0\"", "\""));
    }

    #[test]
    fn label_factors_match_reference() {
        assert_eq!(label_factor("Large"), 1.4);
        assert_eq!(label_factor("Medium"), 1.0);
        assert_eq!(label_factor("Small"), 0.8);
    }
    #[test]
    fn fills_are_colour_or_pattern_by_style() {
        let defs = [style_param()];
        let mut p = super::super::Params::defaults(&defs);
        let c = Fills::new(&p);
        assert!(!c.is_print() && c.series(1).starts_with('#') && c.defs().is_empty() && c.dash(2).is_empty());
        p.set_s("style", "Print");
        let pr = Fills::new(&p);
        assert!(pr.is_print() && pr.series(7) == "url(#vs-hatch1)" && pr.line(3) == "#000");
        assert_eq!(pr.defs().matches("<pattern").count(), 6);
        let dashes: std::collections::BTreeSet<_> = (0..6).map(|i| pr.dash(i)).collect();
        assert_eq!(dashes.len(), 6, "every line series has its own dash");
    }

    #[test]
    fn print_style_renders_black_and_white_only() {
        let mut p = super::super::Params::defaults(&[style_param()]);
        p.set_s("style", "Print");
        let f = Fills::new(&p);
        let mut d = Doc::new(60.0, 20.0);
        d.defs(&f.defs());
        for i in 0..6 { d.rect(i as f64 * 10.0, 0.0, 10.0, 20.0, &f.series(i), "#000", 1.0); }
        let png = super::super::to_png(&d.finish(), 1.0).expect("patterns rasterize");
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let mut dark = 0;
        for px in img.pixels() {
            assert!(px[0] == px[1] && px[1] == px[2], "grey only, got {px:?}");
            if px[0] < 128 { dark += 1; }
        }
        assert!(dark > 50, "hatching actually drew ({dark} dark pixels)");
    }
}
