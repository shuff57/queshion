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
