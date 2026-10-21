//! Drawing shared by the graph and tree figures: grid positions to pixels (kept to a sane size),
//! round nodes with a name inside, edges that stop at the node edge (with an arrow head when
//! directed), and weight labels.
use crate::figures::chart::{self, INK};
use crate::figures::svg::Doc;

/// The longest side, in px, a drawing may grow to however far apart nodes are placed.
const MAX_EXTENT: f64 = 2200.0;

/// Grid positions as pixel centres starting `margin` from the top-left, spaced `cell` apart, and
/// the size of the drawing. A spread wider than `MAX_EXTENT` is scaled down to fit.
pub fn to_px(grid: &[(f64, f64)], cell: (f64, f64), margin: (f64, f64)) -> (Vec<(f64, f64)>, f64, f64) {
    if grid.is_empty() {
        return (Vec::new(), margin.0 * 2.0, margin.1 * 2.0);
    }
    let min_x = grid.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = grid.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = grid.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = grid.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let (sx, sy) = (((max_x - min_x) * cell.0).max(1.0), ((max_y - min_y) * cell.1).max(1.0));
    let k = (MAX_EXTENT / sx).min(MAX_EXTENT / sy).min(1.0);
    let pts = grid.iter().map(|p| (margin.0 + (p.0 - min_x) * cell.0 * k, margin.1 + (p.1 - min_y) * cell.1 * k)).collect();
    (pts, margin.0 * 2.0 + sx * k, margin.1 * 2.0 + sy * k)
}

/// The radius of a node circle that holds `label` at text size `fs`.
pub fn node_radius(label: &str, fs: f64) -> f64 {
    (label.chars().count() as f64 * fs * 0.34 + 10.0).max(fs * 1.5)
}

pub struct NodeStyle<'a> {
    pub fill: &'a str,
    pub stroke: &'a str,
    pub sw: f64,
    pub text: &'a str,
}

pub fn node(d: &mut Doc, c: (f64, f64), r: f64, label: &str, fs: f64, st: &NodeStyle) {
    d.circle(c.0, c.1, r, st.fill, st.stroke, st.sw);
    d.raw(&format!(
        r##"<text x="{:.2}" y="{:.2}" font-size="{fs}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
        c.0,
        c.1 + fs * 0.35,
        st.text,
        crate::figures::svg::esc(label)
    ));
}

/// A line between two circles that starts and ends at their edges, shifted sideways by `off` px
/// (so a pair of opposite arrows do not overlap), with an arrow head at `b` if `directed`.
/// Returns the middle of the drawn line.
#[allow(clippy::too_many_arguments)]
pub fn edge(d: &mut Doc, a: (f64, f64), ra: f64, b: (f64, f64), rb: f64, directed: bool, off: f64, color: &str, sw: f64, dash: &str) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (-uy, ux);
    let p = (a.0 + ux * ra + nx * off, a.1 + uy * ra + ny * off);
    let q = (b.0 - ux * rb + nx * off, b.1 - uy * rb + ny * off);
    let mid = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
    if len < ra + rb + 2.0 {
        return mid;
    }
    if directed {
        let hl = 11.0;
        let base = (q.0 - ux * hl, q.1 - uy * hl);
        stroke(d, p, base, color, sw, dash);
        d.poly(&[q, (base.0 + nx * 4.5, base.1 + ny * 4.5), (base.0 - nx * 4.5, base.1 - ny * 4.5)], color, color, 0.5);
    } else {
        stroke(d, p, q, color, sw, dash);
    }
    mid
}

fn stroke(d: &mut Doc, a: (f64, f64), b: (f64, f64), color: &str, sw: f64, dash: &str) {
    if dash.is_empty() {
        d.line(a.0, a.1, b.0, b.1, color, sw);
    } else {
        d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{dash}"/>"##, a.0, a.1, b.0, b.1));
    }
}

/// Centred text in a given colour (a step number on a dark badge).
pub fn text_fill(d: &mut Doc, x: f64, y: f64, fs: f64, fill: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        crate::figures::svg::esc(s)
    ));
}

/// A small label on a white patch (an edge weight) centred at `c`.
pub fn tag(d: &mut Doc, c: (f64, f64), fs: f64, s: &str) {
    let w = s.chars().count() as f64 * fs * 0.6 + 8.0;
    d.rect(c.0 - w / 2.0, c.1 - fs * 0.75, w, fs * 1.5, "#fff", "none", 0.0);
    chart::text(d, c.0, c.1 + fs * 0.35, fs, "middle", s);
}

/// Left-aligned lines of text starting at `y`; returns the y after the last.
pub fn lines(d: &mut Doc, x: f64, mut y: f64, fs: f64, rows: &[String]) -> f64 {
    for r in rows {
        y += fs * 1.65;
        d.raw(&format!(
            r##"<text x="{x:.2}" y="{:.2}" font-size="{fs}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
            y - fs * 0.35,
            crate::figures::svg::esc(r)
        ));
    }
    y
}

/// Width in px a row of text takes at `fs`, roughly.
pub fn text_w(s: &str, fs: f64) -> f64 {
    s.chars().count() as f64 * fs * 0.56
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_start_at_the_margin_and_huge_spreads_shrink() {
        let (p, w, h) = to_px(&[(0.0, 0.0), (2.0, 1.0)], (100.0, 80.0), (30.0, 30.0));
        assert_eq!(p, vec![(30.0, 30.0), (230.0, 110.0)]);
        assert_eq!((w, h), (260.0, 140.0));
        let (p, w, _) = to_px(&[(-1000.0, 0.0), (1000.0, 0.0)], (100.0, 80.0), (30.0, 30.0));
        assert!(w <= MAX_EXTENT + 60.0 + 1.0 && p[1].0 > p[0].0);
        let (p, w, h) = to_px(&[], (100.0, 80.0), (30.0, 30.0));
        assert!(p.is_empty() && w == 60.0 && h == 60.0);
        let (p, _, _) = to_px(&[(5.0, 5.0)], (100.0, 80.0), (30.0, 30.0));
        assert_eq!(p, vec![(30.0, 30.0)]);
    }

    #[test]
    fn edges_between_touching_circles_draw_nothing_and_never_nan() {
        let mut d = Doc::new(100.0, 100.0);
        edge(&mut d, (10.0, 10.0), 20.0, (10.0, 10.0), 20.0, true, 5.0, "#000", 2.0, "");
        edge(&mut d, (10.0, 10.0), 20.0, (30.0, 10.0), 20.0, true, 5.0, "#000", 2.0, "");
        assert!(!d.finish().contains("NaN"));
    }
}
