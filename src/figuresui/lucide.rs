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
