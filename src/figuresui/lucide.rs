//! A few of the chemistry reference's stroke icons (Lucide, 24 x 24 grid, 2 px round stroke),
//! drawn straight from their SVG path data so the toolbar, header pills and section tiles match
//! the reference glyph for glyph.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// One drawing step of an icon, in the 24 x 24 grid.
#[derive(Clone, Copy)]
pub enum Op {
    Path(&'static str),
    Circle(f32, f32, f32),
    /// A filled disc: x, y, radius
    Dot(f32, f32, f32),
    /// x, y, width, height, corner radius
    Rect(f32, f32, f32, f32, f32),
}

pub const COPY: &[Op] = &[Op::Rect(8.0, 8.0, 14.0, 14.0, 2.0), Op::Path("M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2")];
pub const IMAGE_DOWN: &[Op] = &[
    Op::Path("M10.3 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v10l-3.1-3.1a2 2 0 0 0-2.814.014L6 21"),
    Op::Path("m14 19 3 3v-5.5"),
    Op::Path("m17 22 3-3"),
    Op::Circle(9.0, 9.0, 2.0),
];
pub const FILE_DOWN: &[Op] = &[
    Op::Path("M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"),
    Op::Path("M14 2v5a1 1 0 0 0 1 1h5"),
    Op::Path("M12 18v-6"),
    Op::Path("m9 15 3 3 3-3"),
];
pub const SHARE: &[Op] = &[Op::Path("M12 2v13"), Op::Path("m16 6-4-4-4 4"), Op::Path("M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8")];
pub const UNDO: &[Op] = &[Op::Path("M9 14 4 9l5-5"), Op::Path("M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11")];
pub const REDO: &[Op] = &[Op::Path("m15 14 5-5-5-5"), Op::Path("M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13")];
pub const INFO: &[Op] = &[Op::Circle(12.0, 12.0, 10.0), Op::Path("M12 16v-4"), Op::Path("M12 8h.01")];
pub const ARROW_UP_RIGHT: &[Op] = &[Op::Path("M7 7h10v10"), Op::Path("M7 17 17 7")];
pub const MAP_PIN: &[Op] = &[
    Op::Path("M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0"),
    Op::Circle(12.0, 10.0, 3.0),
];
/// The Start-from-an-example glyph on an 18 px grid, 1.5 px line.
pub const IMAGES: &[Op] = &[
    Op::Rect(6.0, 1.5, 10.5, 10.5, 1.5),
    Op::Path("M4 6H3a1.5 1.5 0 0 0-1.5 1.5V15a1.5 1.5 0 0 0 1.5 1.5H12"),
    Op::Dot(9.75, 5.25, 1.45),
    Op::Path("M8.3 12.1 16 7"),
];
pub const DICES: &[Op] = &[
    Op::Rect(2.0, 10.0, 12.0, 12.0, 2.0),
    Op::Path("m17.92 14 3.5-3.5a2.24 2.24 0 0 0 0-3l-5-4.92a2.24 2.24 0 0 0-3 0L10 6"),
    Op::Path("M6 18h.01"),
    Op::Path("M10 14h.01"),
    Op::Path("M15 6h.01"),
    Op::Path("M18 9h.01"),
];
pub const GIT_FORK: &[Op] = &[
    Op::Circle(12.0, 18.0, 3.0),
    Op::Circle(6.0, 6.0, 3.0),
    Op::Circle(18.0, 6.0, 3.0),
    Op::Path("M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9"),
    Op::Path("M12 12v3"),
];
pub const PIPETTE: &[Op] = &[
    Op::Path("m2 22 1-1h3l9-9"),
    Op::Path("M3 21v-3l9-9"),
    Op::Path("m15 6 3.4-3.4a2.1 2.1 0 1 1 3 3L18 9l.4.4a2.1 2.1 0 1 1-3 3l-3.8-3.8a2.1 2.1 0 1 1 3-3l.4.4Z"),
];
pub const BOOKMARK_PLUS: &[Op] = &[Op::Path("M17 3a2 2 0 0 1 2 2v15a1 1 0 0 1-1.496.868l-4.512-2.578a2 2 0 0 0-1.984 0l-4.512 2.578A1 1 0 0 1 5 20V5a2 2 0 0 1 2-2z"), Op::Path("M12 7v6"), Op::Path("M9 10h6")];
pub const SPLINE: &[Op] = &[Op::Path("M5 17A12 12 0 0 1 17 5"), Op::Circle(19.0, 5.0, 2.0), Op::Circle(5.0, 19.0, 2.0)];

/// Paints `ops` into the square `rect` (its side is the icon's pixel size).
pub fn paint(p: &Painter, rect: Rect, col: Color32, ops: &[Op]) {
    paint_grid(p, rect, col, ops, 24.0, 2.0);
}

/// `paint` for a drawing on a `grid` x `grid` square with a `stroke`-unit line.
pub fn paint_grid(p: &Painter, rect: Rect, col: Color32, ops: &[Op], grid: f32, stroke: f32) {
    let k = rect.width() / grid;
    let st = Stroke::new(stroke * k, col);
    let at = |x: f32, y: f32| Pos2::new(rect.left() + x * k, rect.top() + y * k);
    for op in ops {
        match *op {
            Op::Circle(cx, cy, r) => {
                p.circle_stroke(at(cx, cy), r * k, st);
            }
            Op::Dot(cx, cy, r) => {
                p.circle_filled(at(cx, cy), r * k, col);
            }
            Op::Rect(x, y, w, h, r) => {
                p.rect_stroke(Rect::from_min_size(at(x, y), Vec2::new(w * k, h * k)), r * k, st, egui::StrokeKind::Middle);
            }
            Op::Path(d) => {
                for (pts, closed) in flatten(d) {
                    let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| at(x, y)).collect();
                    if pts.len() == 2 && (pts[0] - pts[1]).length() < 0.2 * k.max(0.5) + 0.01 {
                        // a dot ("M12 8h.01")
                        p.circle_filled(pts[0], k, col);
                        continue;
                    }
                    p.add(Shape::line(pts.clone(), st));
                    if closed {
                        p.add(Shape::line(vec![*pts.last().unwrap(), pts[0]], st));
                    }
                    // round caps
                    if !closed {
                        p.circle_filled(pts[0], k, col);
                        p.circle_filled(*pts.last().unwrap(), k, col);
                    }
                }
            }
        }
    }
}

fn numbers(s: &str, i: &mut usize) -> Option<f32> {
    let b = s.as_bytes();
    while *i < b.len() && (b[*i] == b' ' || b[*i] == b',') {
        *i += 1;
    }
    if *i >= b.len() || b[*i].is_ascii_alphabetic() {
        return None;
    }
    let start = *i;
    if b[*i] == b'-' || b[*i] == b'+' {
        *i += 1;
    }
    let mut dot = false;
    while *i < b.len() {
        match b[*i] {
            b'0'..=b'9' => *i += 1,
            b'.' if !dot => {
                dot = true;
                *i += 1;
            }
            _ => break,
        }
    }
    s[start..*i].parse().ok()
}

/// Flattens SVG path data (M m L l H h V v C c A a Z z) into polylines; the flag is whether the
/// subpath was closed.
fn flatten(d: &str) -> Vec<(Vec<(f32, f32)>, bool)> {
    let b = d.as_bytes();
    let mut i = 0;
    let (mut x, mut y, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut out: Vec<(Vec<(f32, f32)>, bool)> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    let mut cmd = b'M';
    while i < b.len() {
        while i < b.len() && (b[i] == b' ' || b[i] == b',') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i].is_ascii_alphabetic() {
            cmd = b[i];
            i += 1;
            if cmd == b'z' || cmd == b'Z' {
                if !cur.is_empty() {
                    out.push((std::mem::take(&mut cur), true));
                }
                x = sx;
                y = sy;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let (Some(a), Some(c)) = (numbers(d, &mut i), numbers(d, &mut i)) else { break };
                if !cur.is_empty() {
                    out.push((std::mem::take(&mut cur), false));
                }
                x = ox + a;
                y = oy + c;
                sx = x;
                sy = y;
                cur.push((x, y));
                // further pairs after a move are line-tos
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let (Some(a), Some(c)) = (numbers(d, &mut i), numbers(d, &mut i)) else { break };
                x = ox + a;
                y = oy + c;
                cur.push((x, y));
            }
            b'H' => {
                let Some(a) = numbers(d, &mut i) else { break };
                x = ox + a;
                cur.push((x, y));
            }
            b'V' => {
                let Some(a) = numbers(d, &mut i) else { break };
                y = oy + a;
                cur.push((x, y));
            }
            b'C' => {
                let mut v = [0.0f32; 6];
                let mut ok = true;
                for s in v.iter_mut() {
                    match numbers(d, &mut i) {
                        Some(n) => *s = n,
                        None => ok = false,
                    }
                }
                if !ok {
                    break;
                }
                let p0 = (x, y);
                let p1 = (ox + v[0], oy + v[1]);
                let p2 = (ox + v[2], oy + v[3]);
                let p3 = (ox + v[4], oy + v[5]);
                for s in 1..=14 {
                    let t = s as f32 / 14.0;
                    let u = 1.0 - t;
                    let px = u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
                    let py = u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
                    cur.push((px, py));
                }
                x = p3.0;
                y = p3.1;
            }
            b'A' => {
                let mut v = [0.0f32; 7];
                let mut ok = true;
                for s in v.iter_mut() {
                    match numbers(d, &mut i) {
                        Some(n) => *s = n,
                        None => ok = false,
                    }
                }
                if !ok {
                    break;
                }
                let (ex, ey) = (ox + v[5], oy + v[6]);
                arc(&mut cur, (x, y), v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0, (ex, ey));
                x = ex;
                y = ey;
            }
            _ => break,
        }
    }
    if !cur.is_empty() {
        out.push((cur, false));
    }
    out
}

/// Appends an SVG elliptical arc from `p0` to `p1` (endpoint parameterisation) as line segments.
#[allow(clippy::too_many_arguments)]
fn arc(out: &mut Vec<(f32, f32)>, p0: (f32, f32), rx: f32, ry: f32, rot_deg: f32, large: bool, sweep: bool, p1: (f32, f32)) {
    if p0 == p1 {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        out.push(p1);
        return;
    }
    let phi = rot_deg.to_radians();
    let (cp, sp) = (phi.cos(), phi.sin());
    let dx = (p0.0 - p1.0) / 2.0;
    let dy = (p0.1 - p1.1) / 2.0;
    let x1 = cp * dx + sp * dy;
    let y1 = -sp * dx + cp * dy;
    let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lam > 1.0 {
        let s = lam.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = (num / den).max(0.0).sqrt();
    if large == sweep {
        co = -co;
    }
    let cxp = co * rx * y1 / ry;
    let cyp = -co * ry * x1 / rx;
    let cx = cp * cxp - sp * cyp + (p0.0 + p1.0) / 2.0;
    let cy = sp * cxp + cp * cyp + (p0.1 + p1.1) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let th1 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut dth = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
    if !sweep && dth > 0.0 {
        dth -= std::f32::consts::TAU;
    } else if sweep && dth < 0.0 {
        dth += std::f32::consts::TAU;
    }
    let n = ((dth.abs() / std::f32::consts::FRAC_PI_2) * 8.0).ceil().max(3.0) as usize;
    for s in 1..=n {
        let t = th1 + dth * s as f32 / n as f32;
        let (ct, st) = (t.cos(), t.sin());
        out.push((cp * rx * ct - sp * ry * st + cx, sp * rx * ct + cp * ry * st + cy));
    }
}
pub const FLASK_CONICAL: &[Op] = &[
    Op::Path("M14 2v6a2 2 0 0 0 .245.96l5.51 10.08A2 2 0 0 1 18 22H6a2 2 0 0 1-1.755-2.96l5.51-10.08A2 2 0 0 0 10 8V2"),
    Op::Path("M6.453 15h11.094"),
    Op::Path("M8.5 2h7"),
];
pub const ROWS_4: &[Op] = &[Op::Rect(3.0, 3.0, 18.0, 18.0, 2.0), Op::Path("M21 7.5H3"), Op::Path("M21 12H3"), Op::Path("M21 16.5H3")];
pub const RULER: &[Op] = &[
    Op::Path("M21.3 15.3a2.4 2.4 0 0 1 0 3.4l-2.6 2.6a2.4 2.4 0 0 1-3.4 0L2.7 8.7a2.41 2.41 0 0 1 0-3.4l2.6-2.6a2.41 2.41 0 0 1 3.4 0Z"),
    Op::Path("m14.5 12.5 2-2"),
    Op::Path("m11.5 9.5 2-2"),
    Op::Path("m8.5 6.5 2-2"),
    Op::Path("m17.5 15.5 2-2"),
];

// The physics reference's section glyphs (Lucide), copied from its pages.
pub const PH_BOX: &[Op] = &[Op::Path("M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"), Op::Path("m3.3 7 8.7 5 8.7-5"), Op::Path("M12 22V12")];
pub const PH_MOVE_UP_RIGHT: &[Op] = &[Op::Path("M13 5H19V11"), Op::Path("M19 5L5 19")];
pub const PH_GAUGE: &[Op] = &[Op::Path("m12 14 4-4"), Op::Path("M3.34 19a10 10 0 1 1 17.32 0")];
pub const PH_SLIDERS_HORIZONTAL: &[Op] = &[Op::Path("M10 5H3"), Op::Path("M12 19H3"), Op::Path("M14 3v4"), Op::Path("M16 17v4"), Op::Path("M21 12h-9"), Op::Path("M21 19h-5"), Op::Path("M21 5h-7"), Op::Path("M8 10v4"), Op::Path("M8 12H3")];
pub const PH_SIGMA: &[Op] = &[Op::Path("M18 7V5a1 1 0 0 0-1-1H6.5a.5.5 0 0 0-.4.8l4.5 6a2 2 0 0 1 0 2.4l-4.5 6a.5.5 0 0 0 .4.8H17a1 1 0 0 0 1-1v-2")];
pub const PH_GRID_3X3: &[Op] = &[Op::Rect(3.000, 3.000, 18.000, 18.000, 2.000), Op::Path("M3 9h18"), Op::Path("M3 15h18"), Op::Path("M9 3v18"), Op::Path("M15 3v18")];
pub const PH_TRIANGLE: &[Op] = &[Op::Path("M13.73 4a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z")];
pub const PH_RULER: &[Op] = &[Op::Path("M21.3 15.3a2.4 2.4 0 0 1 0 3.4l-2.6 2.6a2.4 2.4 0 0 1-3.4 0L2.7 8.7a2.41 2.41 0 0 1 0-3.4l2.6-2.6a2.41 2.41 0 0 1 3.4 0Z"), Op::Path("m14.5 12.5 2-2"), Op::Path("m11.5 9.5 2-2"), Op::Path("m8.5 6.5 2-2"), Op::Path("m17.5 15.5 2-2")];
pub const PH_COG: &[Op] = &[Op::Path("M11 10.27 7 3.34"), Op::Path("m11 13.73-4 6.93"), Op::Path("M12 22v-2"), Op::Path("M12 2v2"), Op::Path("M14 12h8"), Op::Path("m17 20.66-1-1.73"), Op::Path("m17 3.34-1 1.73"), Op::Path("M2 12h2"), Op::Path("m20.66 17-1.73-1"), Op::Path("m20.66 7-1.73 1"), Op::Path("m3.34 17 1.73-1"), Op::Path("m3.34 7 1.73 1"), Op::Circle(12.000, 12.000, 2.000), Op::Circle(12.000, 12.000, 8.000)];
pub const PH_MOUNTAIN: &[Op] = &[Op::Path("m8 3 4 8 5-5 5 15H2L8 3z")];
pub const PH_CIRCLE: &[Op] = &[Op::Circle(12.000, 12.000, 10.000)];
pub const PH_SPLINE: &[Op] = &[Op::Circle(19.000, 5.000, 2.000), Op::Circle(5.000, 19.000, 2.000), Op::Path("M5 17A12 12 0 0 1 17 5")];
pub const PH_ROUTE: &[Op] = &[Op::Circle(6.000, 19.000, 3.000), Op::Path("M9 19h8.5a3.5 3.5 0 0 0 0-7h-11a3.5 3.5 0 0 1 0-7H15"), Op::Circle(18.000, 5.000, 3.000)];
pub const PH_CHART_LINE: &[Op] = &[Op::Path("M3 3v16a2 2 0 0 0 2 2h16"), Op::Path("m19 9-5 5-4-4-3 3")];
pub const PH_MAP_PIN: &[Op] = &[Op::Path("M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0"), Op::Circle(12.000, 10.000, 3.000)];
pub const PH_HEADING: &[Op] = &[Op::Path("M6 12h12"), Op::Path("M6 20V4"), Op::Path("M18 20V4")];
pub const PH_WEIGHT: &[Op] = &[Op::Circle(12.000, 5.000, 3.000), Op::Path("M6.5 8a2 2 0 0 0-1.905 1.46L2.1 18.5A2 2 0 0 0 4 21h16a2 2 0 0 0 1.925-2.54L19.4 9.5A2 2 0 0 0 17.48 8Z")];
pub const PH_PACKAGE: &[Op] = &[Op::Path("M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z"), Op::Path("M12 22V12"), Op::Path("M3.29 7 12 12 20.71 7"), Op::Path("m7.5 4.27 9 5.15")];
pub const PH_ZOOM_IN: &[Op] = &[Op::Circle(11.000, 11.000, 8.000), Op::Path("M21 21L16.65 16.65"), Op::Path("M11 8L11 14"), Op::Path("M8 11L14 11")];
pub const PH_TYPE: &[Op] = &[Op::Path("M12 4v16"), Op::Path("M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2"), Op::Path("M9 20h6")];
pub const PH_CIRCUIT_BOARD: &[Op] = &[Op::Rect(3.000, 3.000, 18.000, 18.000, 2.000), Op::Path("M11 9h4a2 2 0 0 0 2-2V3"), Op::Circle(9.000, 9.000, 2.000), Op::Path("M7 21v-4a2 2 0 0 1 2-2h4"), Op::Circle(15.000, 15.000, 2.000)];
pub const PH_BATTERY_FULL: &[Op] = &[Op::Path("M10 10v4"), Op::Path("M14 10v4"), Op::Path("M22 14v-4"), Op::Path("M6 10v4"), Op::Rect(2.000, 6.000, 16.000, 12.000, 2.000)];
pub const PH_PALETTE: &[Op] = &[Op::Path("M12 22a1 1 0 0 1 0-20 10 9 0 0 1 10 9 5 5 0 0 1-5 5h-2.25a1.75 1.75 0 0 0-1.4 2.8l.3.4a1.75 1.75 0 0 1-1.4 2.8z"), Op::Circle(13.500, 6.500, 0.500), Op::Circle(17.500, 10.500, 0.500), Op::Circle(6.500, 12.500, 0.500), Op::Circle(8.500, 7.500, 0.500)];
pub const PH_WAVES_HORIZONTAL: &[Op] = &[Op::Path("M2 12q2.5 2 5 0t5 0 5 0 5 0"), Op::Path("M2 19q2.5 2 5 0t5 0 5 0 5 0"), Op::Path("M2 5q2.5 2 5 0t5 0 5 0 5 0")];
pub const PH_MOVE_RIGHT: &[Op] = &[Op::Path("M18 8L22 12L18 16"), Op::Path("M2 12H22")];
pub const PH_MOVE_UP: &[Op] = &[Op::Path("M8 6L12 2L16 6"), Op::Path("M12 2V22")];
pub const PH_CIRCLE_DOT: &[Op] = &[Op::Circle(12.000, 12.000, 1.000), Op::Circle(12.000, 12.000, 10.000)];
pub const PH_GLASSES: &[Op] = &[Op::Circle(6.000, 15.000, 4.000), Op::Circle(18.000, 15.000, 4.000), Op::Path("M14 15a2 2 0 0 0-2-2 2 2 0 0 0-2 2"), Op::Path("M2.5 13 5 7c.7-1.3 1.4-2 3-2"), Op::Path("M21.5 13 19 7c-.7-1.3-1.5-2-3-2")];
pub const PH_ARROW_UP_FROM_LINE: &[Op] = &[Op::Path("m18 9-6-6-6 6"), Op::Path("M12 3v14"), Op::Path("M5 21h14")];
pub const PH_FLASHLIGHT: &[Op] = &[Op::Path("M12 13v1"), Op::Path("M17 2a1 1 0 0 1 1 1v4a3 3 0 0 1-.6 1.8l-.6.8A4 4 0 0 0 16 12v8a2 2 0 0 1-2 2H10a2 2 0 0 1-2-2v-8a4 4 0 0 0-.8-2.4l-.6-.8A3 3 0 0 1 6 7V3a1 1 0 0 1 1-1z"), Op::Path("M6 6h12")];
pub const PH_TAG: &[Op] = &[Op::Path("M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z"), Op::Circle(7.500, 7.500, 0.500)];

pub const ATOM: &[Op] = &[
    Op::Circle(12.0, 12.0, 1.0),
    Op::Path("M20.2 20.2c2.04-2.03.02-7.36-4.5-11.9-4.54-4.52-9.87-6.54-11.9-4.5-2.04 2.03-.02 7.36 4.5 11.9 4.54 4.52 9.87 6.54 11.9 4.5Z"),
    Op::Path("M15.7 15.7c4.52-4.54 6.54-9.87 4.5-11.9-2.03-2.04-7.36-.02-11.9 4.5-4.52 4.54-6.54 9.87-4.5 11.9 2.03 2.04 7.36.02 11.9-4.5Z"),
];

pub const TRASH_2: &[Op] = &[Op::Path("M10 11v6"), Op::Path("M14 11v6"), Op::Path("M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"), Op::Path("M3 6h18"), Op::Path("M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2")];
pub const PENCIL_LINE: &[Op] = &[Op::Path("M13 21h8"), Op::Path("m15 5 4 4"), Op::Path("M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z")];
pub const BAN: &[Op] = &[Op::Circle(12.0, 12.0, 10.0), Op::Path("M4.929 4.929 19.07 19.071")];
