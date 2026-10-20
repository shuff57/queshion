//! Direct-manipulation handles for the math geometry figures (polygons, angles, parallel lines,
//! solids). Positions come from the renderer's own fit (`math::probe_map`) so handles sit exactly
//! on the drawn vertices; drag closures invert that fit and write only the params they drive.
#![allow(dead_code)]
use std::sync::Arc;

use super::interact::{angle_deg, Cursor, Handle};
use super::math::{poly_base, probe_map, render_for};
use super::Params;

type Pt = (f64, f64);

/// Renderer fit: model (y up) -> screen (y down).
#[derive(Clone, Copy)]
struct Fm { s: f64, ox: f64, oy: f64 }
impl Fm {
    fn of(id: &str, p: &Params) -> Option<Fm> {
        let (s, ox, oy) = probe_map(render_for(id)?, p)?;
        (s.is_finite() && s > 1e-9).then_some(Fm { s, ox, oy })
    }
    fn scr(&self, q: Pt) -> Pt { (self.ox + q.0 * self.s, self.oy - q.1 * self.s) }
    fn mdl(&self, q: Pt) -> Pt { ((q.0 - self.ox) / self.s, (self.oy - q.1) / self.s) }
}

fn rot(q: Pt, deg: f64) -> Pt {
    let (c, s) = (deg.to_radians().cos(), deg.to_radians().sin());
    (q.0 * c - q.1 * s, q.0 * s + q.1 * c)
}
fn len(a: Pt) -> f64 { (a.0 * a.0 + a.1 * a.1).sqrt() }
fn d2(a: Pt, b: Pt) -> f64 { len((a.0 - b.0, a.1 - b.1)) }
fn deg(y: f64, x: f64) -> f64 { y.atan2(x).to_degrees() }
fn norm180(a: f64) -> f64 { let r = a.rem_euclid(360.0); if r > 180.0 { r - 360.0 } else { r } }

type Rule = Arc<dyn Fn(&mut Params, Pt) + Send + Sync>;
fn rule(f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Rule { Arc::new(f) }

/// Turns model-space vertices into handles: shown at rotate(model), dragged through unrotate.
/// Uses the flip + total turn (side-at-bottom and Turn) of the render `Fm::of` just ran.
fn poly_handles(fm: Fm, _rot_deg: f64, items: Vec<(Pt, &str, Rule)>) -> Vec<Handle> {
    let (flip, tot) = super::math::last_xf();
    let fx = move |q: Pt| if flip { (-q.0, q.1) } else { q };
    items
        .into_iter()
        .map(|(m, tip, f)| {
            let pos = fm.scr(rot(fx(m), tot));
            Handle::knob(pos, tip, move |p, s| f(p, fx(rot(fm.mdl(s), -tot))))
        })
        .collect()
}

pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<Handle>> {
    let _ = size;
    Some(match id {
        "triangle" => triangle(p)?,
        "rectangle" => rectangle(p)?,
        "parallelogram" => parallelogram(p)?,
        "trapezoid" => trapezoid(p)?,
        "kite" => kite(p)?,
        "regular-polygon" => regular_polygon(p)?,
        "angles" => angles(p),
        "parallel-lines" => parallel_lines(p),
        "prism" => prism(p)?,
        "pyramid" => pyramid(p)?,
        "cylinder" => round_solid(id, p, true)?,
        "cone" => round_solid(id, p, false)?,
        "sphere" => sphere(p)?,
        _ => return None,
    })
}

// ---------- polygons ----------
// The flat shapes are given by measures (text rows), so a drag works out the new shape and writes
// the new values back into the rows that were given (computed rows stay blank).

use super::math::{quad_vals_of, write_givens, Q_V, RAY_V, TRI_V};

fn tri_measures(a: Pt, b: Pt, c: Pt) -> [Option<f64>; 6] {
    let ang = |v: Pt, p: Pt, q: Pt| {
        let (u, w) = ((p.0 - v.0, p.1 - v.1), (q.0 - v.0, q.1 - v.1));
        ((u.0 * w.0 + u.1 * w.1) / (len(u) * len(w)).max(1e-12)).clamp(-1.0, 1.0).acos().to_degrees()
    };
    [Some(ang(a, b, c)), Some(ang(b, c, a)), Some(ang(c, a, b)), Some(d2(a, b)), Some(d2(b, c)), Some(d2(c, a))]
}

fn triangle(p: &Params) -> Option<Vec<Handle>> {
    let fm = Fm::of("triangle", p)?;
    let s = super::math::tri_solve(p).ok()?;
    let [_, b, c] = s.pts;
    let items: Vec<(Pt, &str, Rule)> = vec![
        (b, "Drag B along the base", rule(move |p, m| {
            let Ok(s) = super::math::tri_solve(p) else { return };
            let [a, _, c] = s.pts;
            write_givens(p, &TRI_V, &tri_measures(a, (m.0.max(1e-3), 0.0), c));
        })),
        (c, "Drag vertex C", rule(move |p, m| {
            let Ok(s) = super::math::tri_solve(p) else { return };
            let [a, b, _] = s.pts;
            if m.1.abs() < 1e-6 { return; }
            write_givens(p, &TRI_V, &tri_measures(a, b, (m.0, m.1.abs())));
        })),
    ];
    Some(poly_handles(fm, p.f("rotate"), items))
}

/// Handles for a quadrilateral: each moves one corner to wherever `shape` puts the four corners
/// for the dragged point, then writes the given rows (and the trapezoid's h) from that shape.
fn quad_handles(id: &'static str, p: &Params, corners: &[(usize, &'static str)], shape: fn(&Params, usize, Pt, [Pt; 4]) -> Option<[Pt; 4]>) -> Option<Vec<Handle>> {
    let fm = Fm::of(id, p)?;
    let s = super::math::quad_solve(id, p).ok()?;
    let items: Vec<(Pt, &str, Rule)> = corners.iter().map(|&(k, tip)| {
        let r = rule(move |p: &mut Params, m: Pt| {
            let Ok(s) = super::math::quad_solve(id, p) else { return };
            let Some(pts) = shape(p, k, m, s.pts) else { return };
            write_givens(p, &Q_V, &quad_vals_of(&pts));
            if id == "trapezoid" { write_givens(p, &["m_h"], &[Some(pts[3].1)]); }
        });
        (s.pts[k], tip, r)
    }).collect();
    Some(poly_handles(fm, p.f("rotate"), items))
}

fn rectangle(p: &Params) -> Option<Vec<Handle>> {
    fn shape(p: &Params, k: usize, m: Pt, q: [Pt; 4]) -> Option<[Pt; 4]> {
        let sq = p.s("kind") == "Square";
        let (w, h) = match k { 1 => (m.0, q[2].1), 2 => if sq { let v = (m.0 + m.1) / 2.0; (v, v) } else { (m.0, m.1) }, _ => (q[1].0, m.1) };
        let h = if sq { w } else { h };
        (w > 1e-3 && h > 1e-3).then_some([(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)])
    }
    let mut c = vec![(1, "Drag to change the width"), (2, "Drag the corner to resize")];
    if p.s("kind") != "Square" { c.push((3, "Drag to change the height")); }
    quad_handles("rectangle", p, &c, shape)
}

fn parallelogram(p: &Params) -> Option<Vec<Handle>> {
    fn shape(p: &Params, k: usize, m: Pt, q: [Pt; 4]) -> Option<[Pt; 4]> {
        let rh = p.s("kind") == "Rhombus";
        let (ab, d) = if k == 1 {
            let ab = m.0;
            (ab, if rh { let t = q[3].1.atan2(q[3].0); (ab * t.cos(), ab * t.sin()) } else { q[3] })
        } else {
            let d = (m.0, m.1.abs());
            (if rh { len(d) } else { q[1].0 }, d)
        };
        (ab > 1e-3 && d.1 > 1e-3).then_some([(0.0, 0.0), (ab, 0.0), (ab + d.0, d.1), d])
    }
    quad_handles("parallelogram", p, &[(1, "Drag to change side AB"), (3, "Drag D: side DA and angle A")], shape)
}

fn trapezoid(p: &Params) -> Option<Vec<Handle>> {
    fn shape(p: &Params, k: usize, m: Pt, q: [Pt; 4]) -> Option<[Pt; 4]> {
        let (ab, h, off, cd) = (q[1].0, q[3].1, q[3].0, q[2].0 - q[3].0);
        let kind = p.s("kind");
        let (ab, h, off, cd) = match (k, kind) {
            (1, "Isosceles trapezoid") => { let ab2 = m.0; (ab2, h, (ab2 - cd) / 2.0, cd) }
            (1, _) => (m.0, h, off, cd),
            (3, "Isosceles trapezoid") => (ab, m.1, m.0, ab - 2.0 * m.0),
            (3, "Right trapezoid") => (ab, m.1, 0.0, cd),
            (3, _) => (ab, m.1, m.0, cd + off - m.0),
            (_, "Isosceles trapezoid") => (ab, m.1, ab - m.0, 2.0 * m.0 - ab),
            (_, "Right trapezoid") => (ab, m.1, 0.0, m.0),
            _ => (ab, m.1, off, m.0 - off),
        };
        (ab > 1e-3 && h > 1e-3 && cd > 1e-3).then_some([(0.0, 0.0), (ab, 0.0), (off + cd, h), (off, h)])
    }
    quad_handles("trapezoid", p, &[(1, "Drag to change base AB"), (3, "Drag D: height and slant"), (2, "Drag C: top base CD and height")], shape)
}

fn kite(p: &Params) -> Option<Vec<Handle>> {
    fn shape(_p: &Params, k: usize, m: Pt, q: [Pt; 4]) -> Option<[Pt; 4]> {
        let (a, d) = if k == 3 { (q[0], (0.0, m.1.min(-1e-3))) } else { let x = -m.0.abs(); ((x, m.1.min(-1e-3)), q[3]) };
        (d.1 < a.1 - 1e-3).then_some([a, (0.0, 0.0), (-a.0, a.1), d])
    }
    quad_handles("kite", p, &[(0, "Drag: short side AB and angle B"), (2, "Drag: short side and angle B"), (3, "Drag D: long sides")], shape)
}

fn regular_polygon(p: &Params) -> Option<Vec<Handle>> {
    let fm = Fm::of("regular-polygon", p)?;
    let pts = super::math::regular_polygon_pts(p);
    let n = pts.len();
    let half = std::f64::consts::PI / n as f64;
    let from = p.s("from").to_string();
    let rotd = p.f("rotate");
    let mut out = Vec::new();
    for (k, model) in pts.iter().enumerate() {
        let phi = model.1.atan2(model.0).to_degrees();
        let (flip, _) = super::math::last_xf();
        let fm_model = if flip { (-model.0, model.1) } else { *model };
        let pos = fm.scr(rot(fm_model, rotd));
        let from = from.clone();
        let phi_s = if flip { 180.0 - phi } else { phi };
        let _ = k;
        out.push(Handle::knob(pos, "Drag a vertex: size and rotation", move |p, s| {
            let m = fm.mdl(s);
            let rr = len(m).max(1e-6);
            let val = match from.as_str() { "Radius" => rr, "Apothem" => rr * half.cos(), _ => 2.0 * rr * half.sin() };
            p.set_f("value", val);
            p.set_f("rotate", norm180(deg(m.1, m.0) - phi_s));
        }));
    }
    Some(out)
}

// ---------- angles ----------

const AC: Pt = (240.0, 180.0);
const ALEN: f64 = 140.0;

fn nums(s: &str) -> Vec<f64> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter_map(|t| t.trim().parse().ok()).collect()
}
fn put_list(p: &mut Params, key: &str, v: &[f64]) {
    let s: Vec<String> = v.iter().map(|x| if (x - x.round()).abs() < 1e-9 { format!("{}", *x as i64) } else { format!("{x}") }).collect();
    p.set_s(key, &s.join(", "));
}
fn toggle(p: &mut Params, key: &str, n: f64, on: bool) {
    let mut v = nums(p.s(key));
    v.retain(|x| *x != n);
    if on { v.push(n); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); }
    put_list(p, key, &v);
}
fn dirp(c: Pt, a: f64, r: f64) -> Pt { (c.0 + r * a.to_radians().cos(), c.1 - r * a.to_radians().sin()) }

fn angles(p: &Params) -> Vec<Handle> {
    let (dirs, wrap) = super::math::angle_dirs(p);
    if dirs.len() < 2 { return vec![]; }
    let mut out = Vec::new();
    for (r, src, back) in &dirs {
        if *src == 0 { continue; }
        let key = RAY_V[*src];
        let off = if *back { 180.0 } else { 0.0 };
        out.push(Handle::knob(dirp(AC, *r, ALEN), "Drag the ray to change its direction", move |p, s| {
            let a = (angle_deg(AC, s) - off).rem_euclid(360.0).round();
            p.set_s(key, &format!("{}", a as i64));
        }));
    }
    // Sector label zones: click cycles degrees -> x -> blank.
    let rays: Vec<f64> = dirs.iter().map(|d| d.0).collect();
    let mut secs: Vec<(f64, f64)> = rays.windows(2).map(|w| (w[0], w[1])).collect();
    if wrap { secs.push((*rays.last().unwrap(), rays[0] + 360.0)); }
    for (i, (a0, a1)) in secs.iter().enumerate() {
        let idx = (i + 1) as f64;
        let mid = (a0 + a1) / 2.0;
        let span = a1 - a0;
        let r = if span < 28.0 { 70.0 } else { 58.0 };
        out.push(Handle::zone(dirp(AC, mid, r), (34.0, 24.0), "Click: show degrees / x / blank", move |p| {
            let unk = nums(p.s("unknown")).contains(&idx);
            let hid = nums(p.s("hide")).contains(&idx);
            match (unk, hid) {
                (false, false) => toggle(p, "unknown", idx, true),
                (true, _) => { toggle(p, "unknown", idx, false); toggle(p, "hide", idx, true); }
                (false, true) => toggle(p, "hide", idx, false),
            }
        }));
    }
    turned(out, AC, p.f("turn"))
}

// ---------- parallel lines ----------

fn parallel_lines(p: &Params) -> Vec<Handle> {
    let (w, h, ys) = super::math::pl_layout(p);
    let nl = ys.len();
    let nt = p.i("transversals").clamp(1, 2) as usize;
    let mut out = Vec::new();
    let rad = 26.0;
    for ti in 0..nt {
        let th = if ti == 0 { p.f("angle") } else { p.f("angle_b").clamp(15.0, 165.0) };
        let key = if ti == 0 { "angle" } else { "angle_b" };
        let cx0 = super::math::pl_cx0(p, ti, &ys, w);
        let tr = th.to_radians();
        let slope = (tr.cos(), -tr.sin());
        let cross: Vec<Pt> = ys.iter().map(|y| { let t = (*y - ys[0]) / slope.1; (cx0 + slope.0 * t, *y) }).collect();
        let tl = 90.0 / slope.1.abs().max(0.2);
        let tip = (cross[0].0 + slope.0 * tl, cross[0].1 + slope.1 * tl);
        let c0 = cross[0];
        out.push(Handle::knob(tip, "Drag the transversal to change the angle", move |p, s| {
            let a = angle_deg(c0, s);
            p.set_f(key, if a > 180.0 { a - 180.0 } else { a });
        }));
        for (i, c) in cross.iter().enumerate() {
            let secs = [(th, 180.0, 1usize), (0.0, th, 2), (360.0 - (180.0 - th), 360.0, 3), (180.0, 180.0 + th, 4)];
            for (a0, a1, k) in secs {
                let num = (ti * 4 * nl + i * 4 + k) as f64;
                let mid = (a0 + a1) / 2.0;
                let r = if a1 - a0 < 50.0 { rad + 16.0 } else { rad + 8.0 };
                out.push(Handle::zone(dirp(*c, mid, r), (30.0, 22.0), "Click to label or unlabel this angle", move |p| {
                    let on = nums(p.s("marked")).contains(&num);
                    toggle(p, "marked", num, !on);
                }));
            }
        }
    }
    let last = ys[nl - 1];
    out.push(Handle::bar((100.0, last), true, "Drag to change the gap between lines", move |p, s| {
        // last line sits at 120 + gap * (lines - 1) (see math::pl_layout)
        let nl = p.i("lines").clamp(2, 3) as f64;
        p.set_f("gap", (s.1 - 120.0) / (nl - 1.0) / 100.0);
    }));
    let _ = h;
    turned(out, (w / 2.0, h / 2.0), p.f("turn"))
}

/// Handles laid out for an unturned figure, turned with it about `c` (drags see unturned points).
fn turned(hs: Vec<Handle>, c: Pt, turn: f64) -> Vec<Handle> {
    if turn.abs() < 1e-9 { return hs; }
    hs.into_iter()
        .map(|mut h| {
            h.pos = super::math::turn_scr(h.pos, c, turn);
            if let Some(f) = h.drag.take() {
                h.drag = Some(Arc::new(move |p: &mut Params, s: Pt| f(p, super::math::turn_scr(s, c, -turn))));
            }
            h
        })
        .collect()
}

// ---------- 3D solids ----------

use super::math::{cone_height, depth_sx, edge_height, fmt, oblp, prism_geom, pyramid_height, typed};

const KC: f64 = 0.5 * 0.766_044_443_118_978; // 0.5 cos 40
const KS: f64 = 0.5 * 0.642_787_609_686_539_3; // 0.5 sin 40
fn proj(q: (f64, f64, f64), sx: f64) -> Pt { oblp(q, sx) }

fn base_of(p: &Params, kinds: &str) -> Vec<(f64, f64, f64)> {
    poly_base(kinds, p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"))
}

/// Handles on the (standing) base shared by prism and pyramid.
fn base_handles(fm: Fm, p: &Params, kind: &str, out: &mut Vec<Handle>) {
    let sx = depth_sx(p);
    let b = base_of(p, kind);
    if kind == "Regular polygon" {
        let (b0, b1) = (b[0], b[1]);
        let pe = proj((b1.0 - b0.0, b1.1 - b0.1, 0.0), sx);
        let pe_n = pe.0 * pe.0 + pe.1 * pe.1;
        out.push(Handle::knob(fm.scr(proj(b1, sx)), "Drag to change the side of the base", move |p, s| {
            let m = fm.mdl(s);
            let o = proj(poly_base("Regular polygon", 0.0, 0.0, p.i("n").clamp(3, 12) as usize, p.f("side"))[0], sx);
            let t = ((m.0 - o.0) * pe.0 + (m.1 - o.1) * pe.1) / pe_n.max(1e-9);
            p.set_f("side", t * p.f("side"));
        }));
        return;
    }
    out.push(Handle::knob(fm.scr(proj(b[1], sx)), "Drag to change the length", move |p, s| p.set_f("length", fm.mdl(s).0)));
    if b.len() >= 3 {
        let iso = kind == "Isosceles triangle";
        out.push(Handle::knob(fm.scr(proj(b[2], sx)), "Drag: width and length", move |p, s| {
            let m = fm.mdl(s);
            let w = m.1 / KS;
            p.set_f("width", w);
            let x = m.0 - sx * w * KC;
            p.set_f("length", if iso { 2.0 * x } else { x });
        }));
    }
}

fn prism(p: &Params) -> Option<Vec<Handle>> {
    let fm = Fm::of("prism", p)?;
    let sx = depth_sx(p);
    let kind = p.s("base").to_string();
    let (v, k, lying, _) = prism_geom(p);
    let mut out = Vec::new();
    if lying {
        // front face is the base polygon (x, z); the depth is the prism's height
        if kind != "Regular polygon" {
            out.push(Handle::knob(fm.scr(proj(v[1], sx)), "Drag to change the triangle's base", move |p, s| p.set_f("length", fm.mdl(s).0)));
            out.push(Handle::knob(fm.scr(proj(v[2], sx)), "Drag to change the triangle's height", move |p, s| p.set_f("width", fm.mdl(s).1)));
        }
        out.push(Handle::knob(fm.scr(proj(v[k + 1], sx)), "Drag to change the length (depth)", move |p, s| p.set_f("height", fm.mdl(s).1 / KS)));
        return Some(out);
    }
    let b = base_of(p, &kind);
    let top = v[k];
    let (y0, x0) = (b[0].1, b[0].0);
    base_handles(fm, p, &kind, &mut out);
    out.push(Handle::knob(fm.scr(proj(top, sx)), "Drag to change the height", move |p, s| {
        let m = fm.mdl(s);
        let h = m.1 - y0 * KS;
        let oblique = p.s("stands") == "Oblique";
        if oblique { p.set_f("lean", m.0 - sx * y0 * KC - x0); }
        if oblique && typed(p, "edge").is_some() {
            p.set_s("edge", &fmt((h * h + p.f("lean").powi(2)).sqrt(), 2));
        } else {
            p.set_f("height", h);
        }
    }));
    Some(out)
}

fn pyramid(p: &Params) -> Option<Vec<Handle>> {
    let fm = Fm::of("pyramid", p)?;
    let sx = depth_sx(p);
    let kind = p.s("base").to_string();
    let b = base_of(p, &kind);
    let k = b.len() as f64;
    let c = (b.iter().map(|q| q.0).sum::<f64>() / k, b.iter().map(|q| q.1).sum::<f64>() / k);
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let apex = (c.0 + lean, c.1, pyramid_height(p));
    let mut out = Vec::new();
    base_handles(fm, p, &kind, &mut out);
    out.push(Handle::knob(fm.scr(proj(apex, sx)), "Drag the apex: height and lean", move |p, s| {
        let m = fm.mdl(s);
        let base = poly_base(p.s("base"), p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"));
        let k = base.len() as f64;
        let c = (base.iter().map(|q| q.0).sum::<f64>() / k, base.iter().map(|q| q.1).sum::<f64>() / k);
        let h = m.1 - c.1 * KS;
        if typed(p, "slant").is_some() && p.s("stands") != "Oblique" {
            let mid = ((base[0].0 + base[1].0) / 2.0, (base[0].1 + base[1].1) / 2.0);
            let a2 = (mid.0 - c.0).powi(2) + (mid.1 - c.1).powi(2);
            p.set_s("slant", &fmt((h.max(0.1).powi(2) + a2).sqrt(), 2));
        } else {
            p.set_f("height", h);
        }
        if p.s("stands") == "Oblique" { p.set_f("lean", m.0 - sx * c.1 * KC - c.0); }
    }));
    Some(out)
}

fn round_solid(id: &str, p: &Params, _cyl: bool) -> Option<Vec<Handle>> {
    let fm = Fm::of(id, p)?;
    let r = super::math::solid_r(p);
    let h = if id == "cone" { cone_height(p) } else { edge_height(p, p.s("stands") == "Oblique") };
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let top = fm.scr((lean, h));
    let rad = fm.scr((r, 0.0));
    let cone = id == "cone";
    Some(vec![
        Handle::knob(rad, "Drag to change the radius", move |p, s| super::math::set_solid_r(p, fm.mdl(s).0)).with_cursor(Cursor::ResizeH),
        Handle::knob(top, "Drag to change the height (and lean if oblique)", move |p, s| {
            let m = fm.mdl(s);
            let h = m.1;
            let oblique = p.s("stands") == "Oblique";
            if oblique { p.set_f("lean", m.0); }
            if cone && !oblique && typed(p, "slant").is_some() {
                p.set_s("slant", &fmt((h * h + super::math::solid_r(p).powi(2)).sqrt(), 2));
            } else if !cone && oblique && typed(p, "edge").is_some() {
                p.set_s("edge", &fmt((h * h + p.f("lean").powi(2)).sqrt(), 2));
            } else {
                p.set_f("height", h);
            }
        }),
    ])
}

fn sphere(p: &Params) -> Option<Vec<Handle>> {
    let fm = Fm::of("sphere", p)?;
    Some(vec![Handle::knob(fm.scr((super::math::solid_r(p), 0.0)), "Drag to change the radius", move |p, s| super::math::set_solid_r(p, fm.mdl(s).0)).with_cursor(Cursor::ResizeH)])
}
