//! Tiny SVG builder shared by all generators.
use std::fmt::Write;

pub struct Doc { w: f64, h: f64, body: String }

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl Doc {
    pub fn new(w: f64, h: f64) -> Self { Doc { w, h, body: String::new() } }
    pub fn raw(&mut self, s: &str) -> &mut Self { self.body.push_str(s); self }
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
}
