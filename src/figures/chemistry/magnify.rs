//! Magnifiers for the instrument figures: a big circle showing an enlarged copy of the scale
//! around the reading, joined to a small circle on the instrument by dashed lines (as on the
//! reference site). The instrument is drawn once into an SVG body; the magnifier reuses it
//! inside a circular clip with a scale transform, so it always matches the instrument exactly.
use super::*;
use crate::figures::{ParamDef, Params};

/// A place to magnify, in instrument units: centre and the spacing of numbered marks there.
#[derive(Clone, Copy, Debug)]
pub struct Focus { pub x: f64, pub y: f64, pub label_px: f64 }

pub fn show_param(opts: &[&'static str], d: &str) -> ParamDef { ParamDef::choice("mag_show", "Show", opts, d) }
pub fn marks_param() -> ParamDef { ParamDef::int("mag_marks", "Numbered marks in the magnifier", 1, 6, 2) }

/// Splits a finished `Doc` SVG into (w, h, body).
pub fn inner(svg: &str) -> (f64, f64, String) {
    let (w, h) = crate::figures::svg_size(svg);
    let marker = r##"fill="#fff"/>"##;
    let body = svg.find(marker).map(|i| svg[i + marker.len()..svg.len().saturating_sub(6)].to_string()).unwrap_or_default();
    (w, h, body)
}

pub fn magnifier_only(p: &Params) -> bool { p.s("mag_show") == "Magnifier only" }
pub fn wants_magnifier(p: &Params) -> bool { let s = p.s("mag_show"); s.contains("agnifier") }

/// Radius of the big magnifier circle for an instrument canvas of height `h`.
fn radius(h: f64, below: bool, n: usize) -> f64 {
    if below { (150.0f64).min(300.0 / n.max(1) as f64 + 40.0) } else { (h * 0.3).clamp(110.0, 170.0) }
}

fn zoom_group(body: &str, f: &Focus, c: (f64, f64), r: f64, k: f64, id: usize) -> String {
    format!(
        r##"<defs><clipPath id="qsmag{id}"><circle cx="{:.2}" cy="{:.2}" r="{r:.2}"/></clipPath></defs><g clip-path="url(#qsmag{id})"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#fff"/><g transform="translate({:.2} {:.2}) scale({k:.4}) translate({:.2} {:.2})">{body}</g></g><circle cx="{:.2}" cy="{:.2}" r="{r:.2}" fill="none" stroke="{INK}" stroke-width="3"/>"##,
        c.0, c.1, c.0 - r, c.1 - r, 2.0 * r, 2.0 * r, c.0, c.1, -f.x, -f.y, c.0, c.1
    )
}

/// Builds the final figure from the instrument `svg` and the places to magnify.
/// `below`: magnifiers go under the instrument (horizontal instruments, or several foci);
/// otherwise one magnifier goes to the right.
pub fn compose(p: &Params, svg: String, foci: &[Focus], below: bool) -> String {
    if !wants_magnifier(p) || foci.is_empty() {
        return svg;
    }
    let (w, h, body) = inner(&svg);
    let n = p.i("mag_marks").clamp(1, 6) as f64;
    let below = below || foci.len() > 1;
    let r = radius(h, below, foci.len());
    let only = magnifier_only(p);
    let window = |f: &Focus| (n * f.label_px * 1.12).max(24.0);
    if only {
        let gap = 24.0;
        let cw = foci.len() as f64 * (2.0 * r + gap) + gap;
        let mut d = Doc::new(cw, 2.0 * r + 2.0 * gap);
        for (i, f) in foci.iter().enumerate() {
            let c = (gap + r + i as f64 * (2.0 * r + gap), gap + r);
            d.raw(&zoom_group(&body, f, c, r, 2.0 * r / window(f), i));
        }
        return d.finish();
    }
    let (cw, ch) = if below { (w.max(foci.len() as f64 * (2.0 * r + 30.0) + 30.0), h + 2.0 * r + 40.0) } else { (w + 2.0 * r + 60.0, h.max(2.0 * r + 20.0)) };
    let mut d = Doc::new(cw, ch);
    d.raw(&body);
    for (i, f) in foci.iter().enumerate() {
        let c = if below {
            let slot = cw / foci.len() as f64;
            let x = if foci.len() == 1 { f.x.clamp(r + 15.0, cw - r - 15.0) } else { slot * (i as f64 + 0.5) };
            (x, h + 20.0 + r)
        } else {
            (w + 30.0 + r, f.y.clamp(r + 10.0, ch - r - 10.0))
        };
        let sr = window(f) / 2.0;
        // dashed joins between the small and the big circle
        let (dx, dy) = (c.0 - f.x, c.1 - f.y);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / len, dx / len);
        for s in [-1.0, 1.0] {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.2" stroke-dasharray="5 4"/>"##,
                f.x + nx * sr * s, f.y + ny * sr * s, c.0 + nx * r * s, c.1 + ny * r * s));
        }
        d.circle(f.x, f.y, sr, "none", INK, 1.4);
        d.raw(&zoom_group(&body, f, c, r, 2.0 * r / window(f), i));
    }
    d.finish()
}
