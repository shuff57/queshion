//! Direct-manipulation handles for the physics figures (see `figures::interact`).
//! Geometry mirrors the renderers in `mech.rs`, `graphs.rs` and `instr.rs`: the layout helpers
//! those files expose (`vlayout`, `proj_geo`, `motion_axes`, ...) are reused where they exist and
//! the remaining constants are copied next to a comment naming the renderer line they come from.
use super::graphs::{build_segs, motion_axes, motion_layout, series, SK};
use super::mech::{proj_geo, ray_exit, vlayout, PG};
use crate::figures::interact::{angle_deg, cycle, dist, polar, Cursor, Handle, Pt};
use crate::figures::Params;

pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<Handle>> {
    Some(match id {
        "free-body-diagram" => fbd(p, size),
        "vector-diagram" => vector(p, size),
        "inclined-plane" => incline(p, size),
        "pulley" => pulley(p, size),
        "projectile-motion" => projectile(p, size),
        "motion-graphs" => motion(p, size),
        "waves" => waves(p, size),
        "spring-scale" => spring(p, size),
        "circuit-diagram" => circuit(p, size),
        _ => return None,
    })
}

/// Mirror helper: the renderers flip every x when `mirror` is set (an involution).
#[derive(Clone, Copy)]
struct M { w: f64, on: bool }
impl M {
    fn new(p: &Params, size: (f64, f64)) -> Self { M { w: size.0, on: p.b("mirror") } }
    fn x(self, x: f64) -> f64 { if self.on { self.w - x } else { x } }
    fn p(self, q: Pt) -> Pt { (self.x(q.0), q.1) }
}

/// Text-label zone centred on `c` editing param `key` (double-click).
fn lab(m: M, c: Pt, w: f64, key: &'static str) -> Handle {
    Handle::label(m.p(c), (w, 24.0), key, "Double-click to edit the label")
}

/// Adds `d` notches to the leading number of a value string such as "12 V".
fn bump(s: &str, d: f64) -> String {
    let t = s.trim_start();
    let end = t.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(t.len());
    let (num, rest) = t.split_at(end);
    let Ok(v) = num.parse::<f64>() else { return s.to_string() };
    let nv = (v + if d >= 0.0 { 1.0 } else { -1.0 }).max(0.0);
    let n = if (nv - nv.round()).abs() < 1e-9 { format!("{}", nv.round() as i64) } else { format!("{nv:.1}") };
    format!("{n}{rest}")
}

// ------------------------------------------------------------------ free-body diagram

const FANG: [&str; 6] = ["f1_ang", "f2_ang", "f3_ang", "f4_ang", "f5_ang", "f6_ang"];
const FLEN: [&str; 6] = ["f1_len", "f2_len", "f3_len", "f4_len", "f5_len", "f6_len"];
const FLAB: [&str; 6] = ["f1_lab", "f2_lab", "f3_lab", "f4_lab", "f5_lab", "f6_lab"];

fn fbd(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let (cx, cy) = (240.0, 180.0); // fbd_render
    let body = p.s("body").to_string();
    let mut out = vec![];
    if body != "Dot" {
        out.push(lab(m, (cx, cy), 50.0, "body_lab"));
    }
    for i in 0..p.i("n").clamp(1, 6) as usize {
        let (ang, len) = (p.f(FANG[i]), p.f(FLEN[i]));
        let total = ray_exit(&body, ang) + 20.0 + 55.0 * len;
        let tip = polar((cx, cy), ang, total);
        let (ux, uy) = ((tip.0 - cx) / total, (tip.1 - cy) / total);
        out.push(lab(m, (tip.0 + ux * 28.0, tip.1 + uy * 22.0), 54.0, FLAB[i]));
        let b = body.clone();
        out.push(Handle::knob(m.p(tip), "Drag to set this force's direction and length", move |p, q| {
            let q = m.p(q);
            let r = dist((cx, cy), q);
            if r < 4.0 { return; }
            let a = angle_deg((cx, cy), q);
            p.set_f(FANG[i], a);
            p.set_f(FLEN[i], (r - ray_exit(&b, a) - 20.0) / 55.0);
        }));
    }
    let mo = p.s("motion");
    if mo != "None" {
        let c = (415.0, 292.0); // first motion row
        let tip = polar(c, p.f("mdir"), 26.0);
        out.push(Handle::knob(m.p(tip), "Drag to turn the velocity / acceleration arrow", move |p, q| {
            p.set_f("mdir", angle_deg(c, m.p(q)));
        }));
        let (key, row) = if mo == "Acceleration" { ("ma_lab", 0.0) } else { ("mv_lab", 0.0) };
        let _ = row;
        out.push(lab(m, (tip.0 + (tip.0 - c.0) * 0.8, tip.1 + (tip.1 - c.1) * 0.8), 40.0, key));
    }
    out
}

// ------------------------------------------------------------------ vector diagram

const VMAG: [&str; 4] = ["v1_mag", "v2_mag", "v3_mag", "v4_mag"];
const VANG: [&str; 4] = ["v1_ang", "v2_ang", "v3_ang", "v4_ang"];
const VLAB: [&str; 4] = ["v1_lab", "v2_lab", "v3_lab", "v4_lab"];

fn vector(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let l = vlayout(p, 480.0, 360.0); // vector_render canvas
    let (ox, oy, u) = (l.ox, l.oy, l.u);
    let pxy = |q: Pt| (ox + q.0 * u, oy - q.1 * u);
    let mut out = vec![];
    for i in 0..l.n {
        let (a, b) = (pxy(l.pts[i]), pxy(l.pts[i + 1]));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / len, dx / len);
        out.push(lab(m, ((a.0 + b.0) / 2.0 + nx * 17.0, (a.1 + b.1) / 2.0 + ny * 17.0), 34.0, VLAB[i]));
        let start = l.pts[i];
        out.push(Handle::knob(m.p(b), "Drag the tip to change this vector's length and direction", move |p, q| {
            let q = m.p(q);
            let g = ((q.0 - ox) / u - start.0, (oy - q.1) / u - start.1);
            let mag = (g.0 * g.0 + g.1 * g.1).sqrt();
            p.set_f(VMAG[i], mag);
            if mag > 1e-6 {
                p.set_f(VANG[i], g.1.atan2(g.0).to_degrees().rem_euclid(360.0));
            }
        }));
    }
    if p.b("resultant") && l.n >= 2 {
        let (a, b) = (pxy((0.0, 0.0)), pxy(l.res));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / len, dx / len);
        out.push(lab(m, ((a.0 + b.0) / 2.0 - nx * 18.0, (a.1 + b.1) / 2.0 - ny * 18.0), 34.0, "res_lab"));
    }
    out
}

// ------------------------------------------------------------------ inclined plane

/// incline_render: (ax, ground, lh, up-ramp unit, outward normal) for ramp angle `th`.
fn incline_geo(th: f64) -> (f64, f64, f64, Pt, Pt) {
    let tr = th.to_radians();
    let (ax, ground) = (50.0, 285.0);
    let bw = (390.0f64).min(205.0 / tr.tan());
    (ax, ground, bw / tr.cos(), (tr.cos(), -tr.sin()), (-tr.sin(), -tr.cos()))
}

fn incline(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let th = p.f("angle").clamp(5.0, 75.0);
    let (ax, ground, lh, uu, nn) = incline_geo(th);
    let sz = 50.0 * p.f("size") / 100.0;
    let s0 = p.f("pos") * lh;
    let base = (ax + uu.0 * s0, ground + uu.1 * s0);
    let up = match p.s("obj") { "Cart" => sz * 0.53, _ => sz / 2.0 };
    let com = (base.0 + nn.0 * up, base.1 + nn.1 * up);
    let r = (lh * 0.22).clamp(34.0, 60.0);
    let arc = polar((ax, ground), th, r);
    let mut out = vec![];
    out.push(lab(m, com, 40.0, "obj_lab"));
    out.push(lab(m, polar((ax, ground), th / 2.0, r + 20.0), 40.0, "ang_lab"));
    if p.b("mark_len") {
        out.push(lab(m, (ax + uu.0 * (lh / 2.0) - nn.0 * 40.0, ground + uu.1 * (lh / 2.0) - nn.1 * 40.0), 30.0, "len_lab"));
    }
    out.push(Handle::knob(m.p(arc), "Drag to change the ramp angle", move |p, q| {
        p.set_f("angle", angle_deg((ax, ground), m.p(q)));
    }));
    out.push(Handle::knob(m.p(base), "Drag to slide the object along the ramp", move |p, q| {
        let q = m.p(q);
        let s = (q.0 - ax) * uu.0 + (q.1 - ground) * uu.1;
        p.set_f("pos", s / lh);
    }).with_cursor(Cursor::Move));
    out
}

// ------------------------------------------------------------------ pulley

/// A hanging box: centre x, current top y, its rest top y, size, which side name it lowers as.
struct Box_ { cx: f64, top: f64, rest: f64, s: f64, side: &'static str, size_key: &'static str, lab: &'static str }

fn pulley(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let (ls, rs) = (60.0 * p.f("l_size") / 100.0, 60.0 * p.f("r_size") / 100.0);
    let lower = p.s("lower");
    let (dl, dr) = (if lower == "Left" { 45.0 } else { 0.0 }, if lower == "Right" { 45.0 } else { 0.0 });
    let mut boxes: Vec<Box_> = vec![];
    let mut out = vec![];
    match p.s("setup") {
        "Table" => {
            let (top, edge) = (170.0, 290.0);
            boxes.push(Box_ { cx: 160.0, top: top - ls, rest: top - ls, s: ls, side: "", size_key: "l_size", lab: "l_lab" });
            boxes.push(Box_ { cx: edge + 8.0 + 24.0, top: 250.0 + dr, rest: 250.0, s: rs, side: "Right", size_key: "r_size", lab: "r_lab" });
        }
        "Ramp" => {
            let th = p.f("angle").to_radians();
            let (ground, ax, hgt) = (350.0, 30.0, 190.0);
            let bw = (hgt / th.tan()).min(280.0);
            let tp = (ax + bw, ground - bw * th.tan());
            let (uu, nn) = ((th.cos(), -th.sin()), (-th.sin(), -th.cos()));
            let (hh, pr) = (ls / 2.0, 24.0);
            let q = (tp.0 + nn.0 * hh + uu.0 * hh * th.tan(), tp.1 + nn.1 * hh + uu.1 * hh * th.tan());
            let c = (q.0 - nn.0 * pr, q.1 - nn.1 * pr);
            boxes.push(Box_ { cx: c.0 + pr, top: c.1 + 80.0 + dr, rest: c.1 + 80.0, s: rs, side: "Right", size_key: "r_size", lab: "r_lab" });
            let sb = 0.5 * ((tp.0 - ax) / th.cos());
            let base = (ax + uu.0 * sb, ground + uu.1 * sb);
            out.push(lab(m, (base.0 + nn.0 * ls / 2.0, base.1 + nn.1 * ls / 2.0), 40.0, "l_lab"));
            let k = polar((ax, ground), p.f("angle"), 60.0);
            out.push(Handle::knob(m.p(k), "Drag to change the ramp angle", move |p, q| {
                p.set_f("angle", angle_deg((ax, ground), m.p(q)));
            }));
        }
        "Block & tackle" => {
            let (mx, my) = (260.0, 220.0 + dl);
            boxes.push(Box_ { cx: mx, top: my + 70.0, rest: 290.0, s: ls, side: "Left", size_key: "l_size", lab: "l_lab" });
        }
        _ => {
            boxes.push(Box_ { cx: 240.0 - 42.0, top: 235.0 + dl, rest: 235.0, s: ls, side: "Left", size_key: "l_size", lab: "l_lab" });
            boxes.push(Box_ { cx: 240.0 + 42.0, top: 235.0 + dr, rest: 235.0, s: rs, side: "Right", size_key: "r_size", lab: "r_lab" });
        }
    }
    for b in boxes {
        out.push(lab(m, (b.cx, b.top + b.s / 2.0), 36.0, b.lab));
        let (cx, side, rest, key) = (b.cx, b.side, b.rest, b.size_key);
        out.push(Handle::diamond(m.p((cx + b.s / 2.0, b.top)), "Drag to resize this object", move |p, q| {
            let q = m.p(q);
            p.set_i(key, ((2.0 * (q.0 - cx).abs()) * 100.0 / 60.0).round() as i64);
        }).with_cursor(Cursor::ResizeH));
        if !side.is_empty() {
            let key = b.size_key;
            out.push(Handle::knob(m.p((cx, b.top + b.s)), "Drag down to let this object hang lower", move |p, q| {
                let s = 60.0 * p.f(key) / 100.0;
                let d = q.1 - s - rest;
                if d > 22.0 {
                    p.set_s("lower", side);
                } else if p.s("lower") == side {
                    p.set_s("lower", "Neither");
                }
            }).with_cursor(Cursor::ResizeV));
        }
    }
    out
}

// ------------------------------------------------------------------ projectile motion

fn projectile(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let w = size.0;
    let PG { th, vx, vy, r, ground, lx, ly, s, t_land, .. } = proj_geo(p, w);
    let pos = |t: f64| (lx + s * vx * t, ly - s * (vy * t - 0.5 * t * t));
    let tip = polar((lx, ly), th, r + 80.0);
    let mut out = vec![];
    out.push(lab(m, polar((lx, ly), th / 2.0, r + 50.0), 36.0, "ang_lab").tap_pos(m, (6.0, 0.0)));
    out.push(lab(m, (tip.0 - vy * 2.0 - 3.0, tip.1 - vy.max(0.4) * 10.0 - 10.0), 40.0, "v_lab"));
    if p.b("mark_h") {
        let apex = pos(vy);
        out.push(lab(m, (apex.0 + 44.0, (apex.1 + ly) / 2.0), 30.0, "h_lab"));
    }
    if p.b("mark_r") {
        let end = pos(t_land);
        out.push(lab(m, ((lx + end.0) / 2.0, ground + 24.0 - 14.0), 30.0, "r_lab"));
    }
    // object: scroll to resize
    out.push(Handle::zone(m.p((lx, ly)), (2.0 * r + 6.0, 2.0 * r + 6.0), "Scroll to resize the object", |_| {})
        .with_scroll(|p, d| p.set_i("size", p.i("size") + if d >= 0.0 { 5 } else { -5 })));
    out.push(Handle::knob(m.p(tip), "Drag to aim the launch velocity", move |p, q| {
        p.set_f("angle", angle_deg((lx, ly), m.p(q)));
    }));
    if p.s("from") == "A cliff" {
        let k = (lx * 0.5, ly + r);
        out.push(Handle::bar(m.p(k), true, "Drag to change the cliff height", move |p, q| {
            p.set_f("cliff", (ground - q.1) / s);
        }));
    }
    out
}

trait TapPos { fn tap_pos(self, m: M, d: Pt) -> Self; }
impl TapPos for Handle {
    fn tap_pos(mut self, m: M, d: Pt) -> Self {
        self.pos = (self.pos.0 + if m.on { -d.0 } else { d.0 }, self.pos.1 + d.1);
        self
    }
}

// ------------------------------------------------------------------ motion graphs

#[derive(Clone, Copy)]
struct Ax { x0: f64, y0: f64, w: f64, h: f64, xmin: f64, xmax: f64, ymin: f64, ymax: f64 }

fn motion(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let segs = build_segs(p);
    let ((w, _), panels, _) = motion_layout(p);
    let np = panels.len();
    let mut out = vec![];
    out.push(lab(m, (w / 2.0 + 20.0, 24.0), 220.0, "title"));
    for (pi, (which, rect)) in panels.iter().enumerate() {
        let which = *which;
        let (axs, _, _) = motion_axes(&segs, which, *rect);
        let a = Ax { x0: axs.x0, y0: axs.y0, w: axs.w, h: axs.h, xmin: axs.xmin, xmax: axs.xmax, ymin: axs.ymin, ymax: axs.ymax };
        let ykey = match which { 0 => "yt_pos", 1 => "yt_vel", _ => "yt_acc" };
        out.push(Handle::label(m.p((50.0, a.y0 + a.h / 2.0)), (24.0, 120.0), ykey, "Double-click to edit the axis title"));
        if pi + 1 == np {
            out.push(Handle::label(m.p((a.x0 + a.w / 2.0, a.y0 + a.h + 40.0)), (200.0, 22.0), "xt", "Double-click to edit the axis title"));
        }
        if which == 2 { continue; }
        if which == 0 {
            let s0 = segs[0];
            out.push(Handle::bar(m.p((axs.px(0.0), axs.py(s0.x0))), true, "Drag to set the starting position", move |p, q| {
                p.set_f("x0", a.ymin + (a.y0 + a.h - q.1) / a.h * (a.ymax - a.ymin));
            }));
        }
        for (i, s) in segs.iter().enumerate() {
            let end = (axs.px(s.t1), axs.py(series(s, which, s.t1)));
            let t0 = s.t0;
            out.push(Handle::knob(m.p(end), "Drag to change this segment's duration and end", move |p, q| {
                let q = m.p(q);
                let t1 = a.xmin + (q.0 - a.x0) / a.w * (a.xmax - a.xmin);
                let val = a.ymin + (a.y0 + a.h - q.1) / a.h * (a.ymax - a.ymin);
                let segs = build_segs(p);
                let sg = segs[i];
                let dur = (t1 - t0).max(0.5);
                p.set_f(SK[i][4], dur);
                let kind = p.s(SK[i][0]).to_string();
                if kind == "At rest" { return; }
                let constant = kind.starts_with("Constant");
                let v1 = if which == 1 {
                    val
                } else if constant {
                    (val - sg.x0) / dur
                } else {
                    2.0 * (val - sg.x0) / dur - sg.v0
                };
                if kind.starts_with("Constant") {
                    if v1.abs() > 1e-9 {
                        p.set_s(SK[i][0], if v1 < 0.0 { "Constant velocity back" } else { "Constant velocity forward" });
                    }
                    p.set_f(SK[i][2], v1.abs());
                } else if kind == "Speeding up" {
                    if v1.abs() > 1e-9 {
                        p.set_s(SK[i][6], if v1 < 0.0 { "Back" } else { "Forward" });
                    }
                    p.set_f(SK[i][2], v1.abs());
                } else {
                    // slowing down keeps the sign of the motion it slows
                    let sgn = if sg.v0 < 0.0 { -1.0 } else if sg.v0 > 0.0 { 1.0 } else if p.s(SK[i][6]) == "Back" { -1.0 } else { 1.0 };
                    if sg.v0 == 0.0 && v1.abs() > 1e-9 {
                        p.set_s(SK[i][6], if v1 < 0.0 { "Back" } else { "Forward" });
                    }
                    p.set_f(SK[i][2], if v1 * sgn < 0.0 { 0.0 } else { v1.abs() });
                }
            }));
        }
    }
    out
}

// ------------------------------------------------------------------ waves

fn waves(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let kind = p.s("kind").to_string();
    let time = p.s("against") == "Time";
    let (a, wl) = (p.f("amp"), p.f("wl").max(0.5));
    let cyc = p.i("cycles").clamp(1, 5) as f64;
    let xmax = wl * cyc;
    let trans = kind != "Longitudinal";
    let long = kind != "Transverse";
    let w = size.0;
    let h = size.1;
    let top = if p.s("title").trim().is_empty() { 24.0 } else { 52.0 };
    let (ox, pw) = (100.0, 460.0);
    let px = move |x: f64| ox + x / xmax * pw;
    let mut out = vec![];
    out.push(lab(m, (w / 2.0 + 20.0, 24.0), 220.0, "title"));
    let _ = time;
    let mut y_cursor = top;
    let damp = move |p: &mut Params, here: f64| {
        // The axis rescales with the wavelength, so steer by ratio (damped) rather than absolute.
        let f = ((here - ox) / (px(wl) - ox)).clamp(0.2, 5.0);
        p.set_f("wl", wl * f.powf(0.12));
    };
    if trans {
        let ph = if kind == "Both" { 260.0 } else { h - top - 85.0 };
        let ystep = super::common::nice_step(a * 2.6, 8.0);
        let ymax = (a * 1.35 / ystep).ceil() * ystep;
        let py = move |y: f64| top + ph / 2.0 - y / ymax * (ph / 2.0);
        out.push(Handle::knob(m.p((px(wl / 4.0), py(a))), "Drag the crest up or down to set the amplitude", move |p, q| {
            let q = m.p(q);
            p.set_f("amp", (top + ph / 2.0 - q.1) / (ph / 2.0) * ymax);
        }).with_cursor(Cursor::ResizeV));
        out.push(Handle::knob(m.p((px(wl), py(0.0))), "Drag sideways to stretch or squeeze the wavelength; scroll to add or remove cycles", move |p, q| {
            damp(p, m.p(q).0);
        }).with_cursor(Cursor::ResizeH).with_scroll(|p, d| p.set_i("cycles", p.i("cycles") + if d >= 0.0 { 1 } else { -1 })));
        out.push(lab(m, (px(wl / 4.0) + 16.0, (py(0.0) + py(a)) / 2.0), 30.0, "amp_lab"));
        let (s, e) = if xmax >= wl * 1.25 - 1e-9 { (wl / 4.0, wl * 1.25) } else { (0.0, wl) };
        let y = py(a + (ymax - a) * 0.5);
        out.push(lab(m, ((px(s) + px(e)) / 2.0, y - 14.0), 36.0, "wl_lab"));
        out.push(lab(m, (w / 2.0 + 20.0 - 10.0, top - 0.0 + ph + 40.0), 200.0, "xt").tap_skip(kind == "Both"));
        out.push(Handle::label(m.p((50.0, top + ph / 2.0)), (24.0, 120.0), "yt", "Double-click to edit the axis title"));
        y_cursor += ph + if kind == "Both" { 46.0 } else { 0.0 };
    }
    if long {
        let (bh, by) = (70.0, y_cursor + 20.0);
        if !trans {
            out.push(Handle::knob(m.p((px(wl), by + bh / 2.0)), "Drag sideways to stretch or squeeze the wavelength; scroll to add or remove cycles", move |p, q| {
                damp(p, m.p(q).0);
            }).with_cursor(Cursor::ResizeH).with_scroll(|p, d| p.set_i("cycles", p.i("cycles") + if d >= 0.0 { 1 } else { -1 })));
            if p.b("mark_wl") {
                let (s, e) = if xmax >= wl * 1.25 - 1e-9 { (wl / 4.0, wl * 1.25) } else { (0.0, wl) };
                out.push(lab(m, ((px(s) + px(e)) / 2.0, by - 26.0 - 14.0), 36.0, "wl_lab"));
            }
        }
        out.push(Handle::label(m.p((ox + pw / 2.0, by + bh + 14.0 + 42.0)), (200.0, 22.0), "xt", "Double-click to edit the axis title"));
    }
    out
}

trait TapSkip { fn tap_skip(self, skip: bool) -> Self; }
impl TapSkip for Handle {
    /// Used to park a label zone off-canvas when its text is not drawn in this layout.
    fn tap_skip(mut self, skip: bool) -> Self {
        if skip { self.pos = (-100.0, -100.0); self.edit = None; self.click = None; }
        self
    }
}

// ------------------------------------------------------------------ spring scale

fn spring(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let w = size.0;
    let cap = p.s("cap").trim_end_matches(" N").parse::<f64>().unwrap_or(10.0);
    let view = p.s("view");
    let mut out = vec![];
    out.push(lab(m, (w / 2.0, 22.0), 200.0, "title"));
    if view == "Magnifier only" {
        return out;
    }
    let ix = if view == "Instrument only" { w / 2.0 } else { 130.0 };
    let (y0, y1) = (130.0, 424.0);
    let value = p.f("reading").min(cap) + p.f("offset") * cap;
    let py = y0 + (y1 - y0) * value.clamp(-0.08 * cap, 1.06 * cap) / cap;
    out.push(Handle::knob(m.p((ix + 34.0, py)), "Drag to set the force on the hook", move |p, q| {
        let v = (q.1 - y0) / (y1 - y0) * cap - p.f("offset") * cap;
        let cur = p.f("reading").min(cap);
        if (v - cur).abs() < 0.004 * cap { return; }
        p.set_f("reading", v.clamp(0.0, cap));
    }).with_cursor(Cursor::ResizeV));
    let opts: Vec<&'static str> = vec!["1 N", "2.5 N", "5 N", "10 N", "20 N", "30 N", "50 N"];
    out.push(Handle::zone(m.p((ix, 65.0)), (124.0, 24.0), "Click to change the capacity", move |p| cycle(p, "cap", &opts)));
    if p.s("hook") == "Block" {
        out.push(lab(m, (ix, 521.0), 48.0, "block_lab"));
    }
    out
}

// ------------------------------------------------------------------ circuit diagram

const RKIND: [&str; 4] = ["r1_kind", "r2_kind", "r3_kind", "r4_kind"];
const RNAME: [&str; 4] = ["r1_name", "r2_name", "r3_name", "r4_name"];
const RVAL: [&str; 4] = ["r1_val", "r2_val", "r3_val", "r4_val"];

fn circuit(p: &Params, size: (f64, f64)) -> Vec<Handle> {
    let m = M::new(p, size);
    let parallel = p.s("layout") == "In parallel";
    let n = p.i("n").clamp(1, 4) as usize;
    let (top_y, bot_y, lx) = (if parallel { 90.0 } else { 110.0 }, 280.0, 110.0);
    let rx = if parallel { 215.0 + 85.0 * (n as f64 - 1.0) + 50.0 } else { 480.0 };
    let mut out = vec![];
    out.push(lab(m, (size.0 / 2.0, 22.0), 220.0, "title"));
    for i in 0..n {
        let bulb = p.s(RKIND[i]) == "Bulb";
        let len: f64 = if bulb { 40.0 } else if n >= 4 && !parallel { 56.0 } else { 64.0 };
        let toggle = move |p: &mut Params| cycle(p, RKIND[i], &["Resistor", "Bulb"]);
        if parallel {
            let cx = 215.0 + 85.0 * i as f64;
            let cy = (top_y + bot_y) / 2.0;
            out.push(Handle::zone(m.p((cx, cy)), (36.0, len.max(60.0)), "Click to switch between resistor and bulb", toggle));
            out.push(lab(m, (cx + 48.0, cy - 8.0), 50.0, RNAME[i]));
            out.push(lab(m, (cx + 50.0, cy + 16.0), 56.0, RVAL[i]).with_scroll(move |p, d| {
                let v = bump(p.s(RVAL[i]), d);
                p.set_s(RVAL[i], &v);
            }));
        } else {
            let span = rx - 30.0 - (lx + 80.0);
            let cx = lx + 80.0 + span * (i as f64 + 0.5) / n as f64;
            out.push(Handle::zone(m.p((cx, top_y)), (len, 34.0), "Click to switch between resistor and bulb", toggle));
            out.push(lab(m, (cx, top_y - 36.0), 56.0, RNAME[i]));
            out.push(lab(m, (cx, top_y + 34.0), 56.0, RVAL[i]).with_scroll(move |p, d| {
                let v = bump(p.s(RVAL[i]), d);
                p.set_s(RVAL[i], &v);
            }));
        }
    }
    let sw_mid = if parallel { 150.0 } else { 270.0 };
    if p.s("switch") != "None" {
        out.push(Handle::zone(m.p((sw_mid, bot_y - 12.0)), (70.0, 44.0), "Click to open or close the switch", |p| {
            let next = if p.s("switch") == "Closed" { "Open" } else { "Closed" };
            p.set_s("switch", next);
        }));
    }
    let cells = match p.s("cell") { "One cell" => 1, "Battery of three cells" => 3, _ => 2 };
    let by = (top_y + bot_y) / 2.0;
    out.push(Handle::zone(m.p((lx, by)), (54.0, 12.0 * (cells * 2) as f64 + 10.0), "Click to add a cell to the battery", |p| {
        cycle(p, "cell", &["One cell", "Battery of two cells", "Battery of three cells"]);
    }));
    let named = !p.s("cell_name").trim().is_empty();
    out.push(lab(m, (lx + 36.0 + 28.0, by + if named { 14.0 } else { 1.0 }), 60.0, "cell_val").with_scroll(|p, d| {
        let v = bump(p.s("cell_val"), d);
        p.set_s("cell_val", &v);
    }));
    out
}

#[cfg(test)]
mod overlay_tests {
    use super::*;
    use crate::figures::{registry, svg_size, to_png, Subject};

    /// Writes each physics figure with its handles drawn on top (red = knob, blue = zone box)
    /// to $FIG_OVERLAY_DIR for eyeballing that handle geometry matches the renderer.
    #[test]
    fn overlay_dump() {
        let Ok(dir) = std::env::var("FIG_OVERLAY_DIR") else { return };
        let _ = std::fs::create_dir_all(&dir);
        for d in registry().into_iter().filter(|d| d.subject == Subject::Physics) {
            let mut variants = vec![("def", Params::defaults(&(d.params)()))];
            let mut q = Params::defaults(&(d.params)());
            match d.id {
                "free-body-diagram" => { q.set_s("body", "Block"); q.set_i("n", 4); q.set_s("motion", "Both"); }
                "vector-diagram" => { q.set_i("n", 3); q.set_b("mirror", true); }
                "inclined-plane" => { q.set_b("mark_len", true); }
                "pulley" => { q.set_s("setup", "Table"); }
                "projectile-motion" => { q.set_s("from", "A cliff"); q.set_b("mark_h", true); q.set_b("mark_r", true); }
                "motion-graphs" => { q.set_s("show", "All three, stacked"); }
                "waves" => { q.set_s("kind", "Both"); }
                "circuit-diagram" => { q.set_s("layout", "In parallel"); q.set_s("switch", "Closed"); }
                _ => {}
            }
            variants.push(("alt", q));
            if d.id == "pulley" {
                for s in ["Ramp", "Block & tackle", "Atwood"] {
                    let mut q = Params::defaults(&(d.params)());
                    q.set_s("setup", s);
                    variants.push(("s", q));
                }
            }
            for (vi, (tag, p)) in variants.iter().enumerate() {
                let svg = (d.render)(p);
                let size = svg_size(&svg);
                let mut extra = String::new();
                for h in d.handles(p, size) {
                    let (x, y) = h.pos;
                    if h.drag.is_some() {
                        extra += &format!(r##"<circle cx="{x}" cy="{y}" r="6" fill="none" stroke="#e00" stroke-width="2.5"/>"##);
                    } else {
                        extra += &format!(r##"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="#06f" stroke-width="1.2"/>"##, x - h.half.0, y - h.half.1, h.half.0 * 2.0, h.half.1 * 2.0);
                    }
                }
                let svg = svg.replace("</svg>", &format!("{extra}</svg>"));
                let png = to_png(&svg, 1.0).unwrap();
                std::fs::write(format!("{dir}/{}_{vi}{tag}.png", d.id), png).unwrap();
            }
        }
    }
}
