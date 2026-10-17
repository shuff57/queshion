//! Mechanics figures: free-body diagram, vector diagram, inclined plane, pulley, projectile motion.
use super::common::*;
use crate::figures::{FigureDef, ParamDef, Params, Subject};

fn colr(on: bool, i: usize) -> &'static str { if on { PAL[i % PAL.len()] } else { INK } }

// ---------------------------------------------------------------- free-body diagram

const FK: [[&str; 6]; 6] = [
    ["f1_ang", "Force 1 direction (deg)", "f1_len", "Force 1 length", "f1_lab", "Force 1 label"],
    ["f2_ang", "Force 2 direction (deg)", "f2_len", "Force 2 length", "f2_lab", "Force 2 label"],
    ["f3_ang", "Force 3 direction (deg)", "f3_len", "Force 3 length", "f3_lab", "Force 3 label"],
    ["f4_ang", "Force 4 direction (deg)", "f4_len", "Force 4 length", "f4_lab", "Force 4 label"],
    ["f5_ang", "Force 5 direction (deg)", "f5_len", "Force 5 length", "f5_lab", "Force 5 label"],
    ["f6_ang", "Force 6 direction (deg)", "f6_len", "Force 6 length", "f6_lab", "Force 6 label"],
];
/// Per-force extras: label mode, mark angle, measured from, angle label mode, angle label, components.
const FX: [[&str; 6]; 6] = [
    ["f1_lab_mode", "f1_mark", "f1_from", "f1_alab_mode", "f1_alab", "f1_comps"],
    ["f2_lab_mode", "f2_mark", "f2_from", "f2_alab_mode", "f2_alab", "f2_comps"],
    ["f3_lab_mode", "f3_mark", "f3_from", "f3_alab_mode", "f3_alab", "f3_comps"],
    ["f4_lab_mode", "f4_mark", "f4_from", "f4_alab_mode", "f4_alab", "f4_comps"],
    ["f5_lab_mode", "f5_mark", "f5_from", "f5_alab_mode", "f5_alab", "f5_comps"],
    ["f6_lab_mode", "f6_mark", "f6_from", "f6_alab_mode", "f6_alab", "f6_comps"],
];
const FXL: [[&str; 6]; 6] = [
    ["Force 1 label shows", "Force 1: mark its angle", "Force 1 angle measured from", "Force 1 angle label shows", "Force 1 angle label", "Force 1: show its components"],
    ["Force 2 label shows", "Force 2: mark its angle", "Force 2 angle measured from", "Force 2 angle label shows", "Force 2 angle label", "Force 2: show its components"],
    ["Force 3 label shows", "Force 3: mark its angle", "Force 3 angle measured from", "Force 3 angle label shows", "Force 3 angle label", "Force 3: show its components"],
    ["Force 4 label shows", "Force 4: mark its angle", "Force 4 angle measured from", "Force 4 angle label shows", "Force 4 angle label", "Force 4: show its components"],
    ["Force 5 label shows", "Force 5: mark its angle", "Force 5 angle measured from", "Force 5 angle label shows", "Force 5 angle label", "Force 5: show its components"],
    ["Force 6 label shows", "Force 6: mark its angle", "Force 6 angle measured from", "Force 6 angle label shows", "Force 6 angle label", "Force 6: show its components"],
];
const FDEF: [(f64, &str); 6] = [(270.0, "F_g"), (90.0, "F_N"), (0.0, "F_f"), (180.0, "F_T"), (45.0, "F_app"), (225.0, "F_s")];
const FMODES: [&str; 3] = ["Text", "Blank line", "None"];

fn fbd_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("body", "Body", &["Dot", "Block", "Ball", "Cart"], "Dot"),
        ParamDef::int("body_size", "Body size (%)", 50, 200, 100),
        ParamDef::int("n", "Number of forces", 1, 6, 2),
    ];
    for (i, k) in FK.iter().enumerate() {
        let x = FX[i];
        let xl = FXL[i];
        v.push(ParamDef::float(k[0], k[1], 0.0, 360.0, 1.0, FDEF[i].0));
        v.push(ParamDef::float(k[2], k[3], 0.25, 2.0, 0.05, 1.0));
        v.push(ParamDef::choice(x[0], xl[0], &FMODES, "Text"));
        v.push(ParamDef::text(k[4], k[5], FDEF[i].1));
        v.push(ParamDef::bool(x[1], xl[1], false));
        v.push(ParamDef::choice(x[2], xl[2], &["Horizontal", "Vertical"], "Horizontal"));
        v.push(ParamDef::choice(x[3], xl[3], &FMODES, "Text"));
        v.push(ParamDef::text(x[4], xl[4], "theta"));
        v.push(ParamDef::bool(x[5], xl[5], false));
    }
    v.push(ParamDef::text("body_lab", "Label inside the body (blank = none)", ""));
    v.push(ParamDef::choice("mv_lab_mode", "Velocity label shows", &FMODES, "Text"));
    v.push(ParamDef::text("mv_lab", "Velocity label", "v"));
    v.push(ParamDef::choice("ma_lab_mode", "Acceleration label shows", &FMODES, "Text"));
    v.push(ParamDef::text("ma_lab", "Acceleration label", "a"));
    v.push(ParamDef::choice("notation", "Vector notation", &["None", "Arrow", "Bold"], "None"));
    v.push(ParamDef::choice("motion", "Motion (drawn beside the body)", &["None", "Velocity", "Acceleration", "Both"], "None"));
    v.push(ParamDef::float("mdir", "Velocity direction (deg)", 0.0, 360.0, 1.0, 0.0));
    v.push(ParamDef::float("adir", "Acceleration direction (deg; -1 = same as velocity)", -1.0, 360.0, 1.0, -1.0));
    v.push(ParamDef::bool("mirror", "Mirror (flip left to right)", false));
    v.push(ParamDef::bool("color", "Color (for slides)", false));
    v
}

/// Distance from the body's centre to its edge along `ang` (body scaled by `sc`).
pub(super) fn ray_exit(kind: &str, ang: f64, sc: f64) -> f64 {
    let (hw, hh, circ) = match kind { "Block" => (38.0, 28.0, false), "Ball" => (28.0, 28.0, true), "Cart" => (48.0, 22.0, false), _ => (6.0, 6.0, true) };
    let (hw, hh) = if kind == "Dot" { (hw, hh) } else { (hw * sc, hh * sc) };
    if circ { return hw; }
    let a = ang.to_radians();
    let (c, s) = (a.cos().abs().max(1e-6), a.sin().abs().max(1e-6));
    (hw / c).min(hh / s)
}

/// Geometry shared by the renderer and the handles.
pub(super) struct FGeo {
    pub w: f64,
    pub h: f64,
    pub cx: f64,
    pub cy: f64,
    pub sc: f64,
    /// (tail, tip, direction deg) per force; tails are offset when forces share a direction.
    pub forces: Vec<(Pt, Pt, f64)>,
    /// (centre, direction deg, label key, label mode key) per motion arrow.
    pub motion: Vec<(Pt, f64, &'static str, &'static str)>,
}

pub(super) fn fbd_geo(p: &Params) -> FGeo {
    let body = p.s("body");
    let sc = (p.f("body_size") / 100.0).clamp(0.5, 2.0);
    let n = p.i("n").clamp(1, 6) as usize;
    let angs: Vec<f64> = (0..n).map(|i| p.f(FK[i][0]).rem_euclid(360.0)).collect();
    // raw geometry around (0,0)
    let mut forces = vec![];
    for i in 0..n {
        let ang = angs[i];
        let same: Vec<usize> = (0..n).filter(|&j| (angs[j] - ang).abs() < 1.0).collect();
        let k = same.iter().position(|&j| j == i).unwrap_or(0) as f64;
        let off = (k - (same.len() as f64 - 1.0) / 2.0) * 14.0;
        let (ox, oy) = pol(0.0, 0.0, off, ang - 90.0);
        let total = ray_exit(body, ang, sc) + 20.0 + 55.0 * p.f(FK[i][2]);
        let tip = pol(ox, oy, total, ang);
        forces.push(((ox, oy), tip, ang));
    }
    let m = p.s("motion");
    let (sv, sa) = (m == "Velocity" || m == "Both", m == "Acceleration" || m == "Both");
    let mut right = 60.0f64 * if body == "Dot" { 0.5 } else { sc };
    for (_, t, _) in &forces { if t.1.abs() < 70.0 { right = right.max(t.0 + 40.0); } }
    let mx = right.max(100.0) + 60.0;
    let mut motion = vec![];
    let adir = if p.f("adir") < 0.0 { p.f("mdir") } else { p.f("adir") };
    let rows = sv as usize + sa as usize;
    let mut y = if rows == 2 { -36.0 } else { 0.0 };
    if sv { motion.push(((mx, y), p.f("mdir"), "mv_lab", "mv_lab_mode")); y += 72.0; }
    if sa { motion.push(((mx, y), adir, "ma_lab", "ma_lab_mode")); }
    // bounding box (labels need ~34 px past a tip)
    let (mut x0, mut x1, mut y0, mut y1) = (-70.0f64 * sc, 70.0f64 * sc, -50.0f64 * sc, 50.0f64 * sc);
    for (_, t, _) in &forces { x0 = x0.min(t.0 - 40.0); x1 = x1.max(t.0 + 40.0); y0 = y0.min(t.1 - 36.0); y1 = y1.max(t.1 + 36.0); }
    for (c, _, _, _) in &motion { x0 = x0.min(c.0 - 50.0); x1 = x1.max(c.0 + 50.0); y0 = y0.min(c.1 - 50.0); y1 = y1.max(c.1 + 50.0); }
    // keep the old 480x360 canvas centred on the body unless something sticks out
    let hw = 240f64.max(-x0 + 16.0).max(x1 + 16.0);
    let hh = 180f64.max(-y0 + 16.0).max(y1 + 16.0);
    let (w, h, cx, cy) = (2.0 * hw, 2.0 * hh, hw, hh);
    let sh = |q: Pt| (q.0 + cx, q.1 + cy);
    FGeo {
        w, h, cx, cy, sc,
        forces: forces.into_iter().map(|(a, b, g)| (sh(a), sh(b), g)).collect(),
        motion: motion.into_iter().map(|(c, a, k, mk)| (sh(c), a, k, mk)).collect(),
    }
}

/// `F_A` -> `F_{Ax}` (component labels keep the force's name).
fn comp_label(lab: &str, axis: &str) -> String {
    let t = lab.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") { return format!("F_{axis}"); }
    match t.split_once('_') {
        Some((b, s)) => format!("{b}_{{{}{axis}}}", s.trim_matches(|c| c == '{' || c == '}')),
        None => format!("{t}_{axis}"),
    }
}

fn fbd_render(p: &Params) -> String {
    let geo = fbd_geo(p);
    let (cx, cy, sc) = (geo.cx, geo.cy, geo.sc);
    let mut g = G::new(geo.w, geo.h, p.b("mirror"));
    let col = p.b("color");
    let body = p.s("body").to_string();
    match body.as_str() {
        "Block" => g.rect(cx - 38.0 * sc, cy - 28.0 * sc, 76.0 * sc, 56.0 * sc, FILL, INK, SW),
        "Ball" => g.circle(cx, cy, 28.0 * sc, FILL, INK, SW),
        "Cart" => {
            g.rect(cx - 48.0 * sc, cy - 22.0 * sc, 96.0 * sc, 28.0 * sc, FILL, INK, SW);
            g.circle(cx - 26.0 * sc, cy + 12.0 * sc, 9.0 * sc, FILL, INK, SW);
            g.circle(cx + 26.0 * sc, cy + 12.0 * sc, 9.0 * sc, FILL, INK, SW);
        }
        _ => {}
    }
    let st = match p.s("notation") { "Arrow" => VEC, "Bold" => BOLD, _ => ITALIC };
    // components and angle marks under the arrows
    for (i, &(tail, tip, ang)) in geo.forces.iter().enumerate() {
        let x = FX[i];
        if p.b(x[5]) {
            let cc = if col { PAL[i % PAL.len()] } else { GREY };
            let (ex, ey) = (tip.0, tail.1);
            if (tip.0 - tail.0).abs() > 4.0 { g.dash_arrow(tail.0, tail.1, ex, ey, cc, 1.8); }
            if (tip.1 - tail.1).abs() > 4.0 { g.dash_arrow(ex, ey, tip.0, tip.1, cc, 1.8); }
            let lab = p.s(FK[i][4]);
            let sy = if tip.1 < tail.1 { 1.0 } else { -1.0 };
            g.tex((tail.0 + ex) / 2.0, tail.1 + 22.0 * sy + 6.0, 18.0, "middle", &comp_label(lab, "x"), ITALIC);
            let sx = if tip.0 > tail.0 { 1.0 } else { -1.0 };
            g.tex(ex + 8.0 * sx, (ey + tip.1) / 2.0 + 6.0, 18.0, if sx > 0.0 { "start" } else { "end" }, &comp_label(lab, "y"), ITALIC);
        }
        if p.b(x[1]) {
            let refa = if p.s(x[2]) == "Vertical" {
                if ang.rem_euclid(360.0) <= 180.0 { 90.0 } else { 270.0 }
            } else if (ang.rem_euclid(360.0) - 180.0).abs() < 90.0 { 180.0 } else { 0.0 };
            // signed sweep from the reference to the force, the short way round
            let mut d = (ang - refa).rem_euclid(360.0);
            if d > 180.0 { d -= 360.0; }
            let r0 = ray_exit(&body, refa, sc);
            let r = (r0 + 26.0).max(44.0);
            let e = pol(tail.0, tail.1, r + 14.0, refa);
            let s0 = pol(tail.0, tail.1, r0.max(4.0), refa);
            g.dash(s0.0, s0.1, e.0, e.1, INK, 1.3);
            g.polyline(&arc_pts(tail.0, tail.1, r, refa, refa + d), INK, 1.5);
            let mid = pol(tail.0, tail.1, r + 16.0, refa + d / 2.0);
            let t = match p.s(x[3]) { "Blank line" => String::new(), "None" => "none".into(), _ => p.s(x[4]).to_string() };
            g.label_at(mid.0, mid.1, 22.0, &t, ITALIC);
        }
    }
    for (i, &(tail, tip, _)) in geo.forces.iter().enumerate() {
        let c = colr(col, i);
        g.arrow(tail.0, tail.1, tip.0, tip.1, c, SW);
        let l = ((tip.0 - tail.0).powi(2) + (tip.1 - tail.1).powi(2)).sqrt().max(1e-6);
        let (mut ux, mut uy) = ((tip.0 - tail.0) / l, (tip.1 - tail.1) / l);
        // side-by-side arrows (same direction): push their labels apart sideways
        let side = (tail.0 - cx) * (-uy) + (tail.1 - cy) * ux; // offset along the arrow's normal
        if side.abs() > 1.0 {
            let s = side.signum();
            let (nx, ny) = (-uy * s, ux * s);
            let (vx, vy) = (ux * 0.5 + nx, uy * 0.5 + ny);
            let vl = (vx * vx + vy * vy).sqrt();
            ux = vx / vl;
            uy = vy / vl;
        }
        g.label_dir(tip.0, tip.1, ux, uy, 24.0, &lab_mode(p, FK[i][4], FX[i][0]), st);
    }
    if body == "Dot" { g.circle(cx, cy, 6.0, INK, INK, 1.0); } else { g.circle(cx, cy, 3.0, INK, INK, 1.0); }
    if body != "Dot" && !p.s("body_lab").trim().is_empty() { g.label_at(cx, cy + 14.0 * sc, 20.0, p.s("body_lab"), ITALIC); }
    // motion indicators beside the body: dashed, since they are not forces
    for &(c, md, key, mk) in &geo.motion {
        let (a, b) = (pol(c.0, c.1, -26.0, md), pol(c.0, c.1, 26.0, md));
        g.dash_arrow(a.0, a.1, b.0, b.1, if col { PAL[1] } else { INK }, SW);
        let lab = lab_mode(p, key, mk);
        // label beside the middle of the arrow (right of travel)
        let (ux, uy) = ((b.0 - c.0) / 26.0, (b.1 - c.1) / 26.0);
        let (nx, ny) = (-uy, ux);
        let s = if nx < -0.1 || (nx.abs() <= 0.1 && ny > 0.0) { -1.0 } else { 1.0 };
        g.label_dir(c.0 + nx * s * 4.0, c.1 + ny * s * 4.0, nx * s, ny * s, 21.0, &lab, st);
    }
    g.finish()
}

// ---------------------------------------------------------------- vector diagram

const VK: [[&str; 8]; 4] = [
    ["v1_mag", "Vector 1 magnitude", "v1_ang", "Vector 1 direction (deg)", "v1_lab", "Vector 1 label", "v1_style", "Vector 1 arrow"],
    ["v2_mag", "Vector 2 magnitude", "v2_ang", "Vector 2 direction (deg)", "v2_lab", "Vector 2 label", "v2_style", "Vector 2 arrow"],
    ["v3_mag", "Vector 3 magnitude", "v3_ang", "Vector 3 direction (deg)", "v3_lab", "Vector 3 label", "v3_style", "Vector 3 arrow"],
    ["v4_mag", "Vector 4 magnitude", "v4_ang", "Vector 4 direction (deg)", "v4_lab", "Vector 4 label", "v4_style", "Vector 4 arrow"],
];
const VDEF: [(f64, f64, &str); 4] = [(4.0, 0.0, "A"), (3.0, 90.0, "B"), (2.0, 180.0, "C"), (2.0, 270.0, "D")];
const ARROW_STYLES: [&str; 3] = ["Drawn", "Dashed", "Left off"];

const VMODE: [[&str; 2]; 4] = [
    ["v1_lab_mode", "Vector 1 label shows"], ["v2_lab_mode", "Vector 2 label shows"],
    ["v3_lab_mode", "Vector 3 label shows"], ["v4_lab_mode", "Vector 4 label shows"],
];
const LMODES: [&str; 3] = ["Text", "Blank line", "None"];

fn vector_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::int("n", "Number of vectors", 1, 4, 2)];
    for (i, k) in VK.iter().enumerate() {
        v.push(ParamDef::float(k[0], k[1], 0.0, 12.0, 0.5, VDEF[i].0));
        v.push(ParamDef::float(k[2], k[3], 0.0, 360.0, 1.0, VDEF[i].1));
        v.push(ParamDef::choice(VMODE[i][0], VMODE[i][1], &LMODES, "Text"));
        v.push(ParamDef::text(k[4], k[5], VDEF[i].2));
        v.push(ParamDef::choice(k[6], k[7], &ARROW_STYLES, "Drawn"));
    }
    v.push(ParamDef::bool("resultant", "Show resultant", true));
    v.push(ParamDef::choice("res_style", "Resultant arrow", &ARROW_STYLES, "Drawn"));
    v.push(ParamDef::choice("res_lab_mode", "Resultant label shows", &LMODES, "Text"));
    v.push(ParamDef::text("res_lab", "Resultant label", "R"));
    v.push(ParamDef::bool("mark_angle", "Mark its angle", false));
    v.push(ParamDef::choice("ang_lab_mode", "Angle label shows", &["Text", "Blank line", "Degrees"], "Text"));
    v.push(ParamDef::text("ang_lab", "Angle label", "theta"));
    v.push(ParamDef::bool("components", "Show its components", false));
    v.push(ParamDef::choice("grid", "Grid and axes", &["Grid", "Axes only", "None"], "Grid"));
    v.push(ParamDef::bool("axes", "x and y axes, from the first tail", false));
    v.push(ParamDef::bool("mirror", "Mirror (flip left to right)", false));
    v.push(ParamDef::bool("color", "Color (for slides)", false));
    v
}

fn styled_arrow(g: &mut G, style: &str, a: Pt, b: Pt, c: &str, sw: f64) {
    match style {
        "Dashed" => g.dash_arrow(a.0, a.1, b.0, b.1, c, sw),
        "Left off" => {}
        _ => g.arrow(a.0, a.1, b.0, b.1, c, sw),
    }
}

pub(super) struct VLay { pub n: usize, pub pts: Vec<Pt>, pub res: Pt, pub u: f64, pub ox: f64, pub oy: f64, pub gx0: f64, pub gx1: f64, pub gy0: f64, pub gy1: f64 }

/// Grid layout of the vector diagram: head-to-tail points (grid units, y up) and pixel mapping.
pub(super) fn vlayout(p: &Params, w: f64, h: f64) -> VLay {
    let n = p.i("n").clamp(1, 4) as usize;
    // head-to-tail points in grid units (y up)
    let mut pts = vec![(0.0, 0.0)];
    for k in VK.iter().take(n) {
        let (m, a) = (p.f(k[0]), p.f(k[2]).to_radians());
        let l = *pts.last().unwrap();
        pts.push((l.0 + m * a.cos(), l.1 + m * a.sin()));
    }
    let res = *pts.last().unwrap();
    let (mut x0, mut x1, mut y0, mut y1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for q in &pts { x0 = x0.min(q.0); x1 = x1.max(q.0); y0 = y0.min(q.1); y1 = y1.max(q.1); }
    let gx0 = x0.floor() - 1.0;
    let gx1 = (x1.ceil() + 1.0).max(gx0 + 4.0);
    let gy0 = y0.floor() - 1.0;
    let gy1 = (y1.ceil() + 1.0).max(gy0 + 4.0);
    let u = ((w - 60.0) / (gx1 - gx0)).min((h - 50.0) / (gy1 - gy0)).min(64.0);
    let (ox, oy) = ((w - u * (gx1 - gx0)) / 2.0 - gx0 * u, (h + u * (gy1 - gy0)) / 2.0 + gy0 * u);
    VLay { n, pts, res, u, ox, oy, gx0, gx1, gy0, gy1 }
}

/// Label centres of the vectors and the resultant: each on the side of its arrow away from the
/// middle of the figure (ties: right of travel, left for a lone vector / the resultant).
pub(super) fn vlabel_pos(p: &Params, l: &VLay) -> (Vec<Pt>, Pt) {
    let pxy = |q: Pt| (l.ox + q.0 * l.u, l.oy - q.1 * l.u);
    let mut cen: Vec<Pt> = l.pts.iter().map(|&q| pxy(q)).collect();
    if p.b("components") { cen.push(pxy((l.res.0, 0.0))); }
    let k = cen.len() as f64;
    let mid = (cen.iter().map(|q| q.0).sum::<f64>() / k, cen.iter().map(|q| q.1).sum::<f64>() / k);
    // offset grows with the label's half-extent across the arrow
    let place = |a: Pt, b: Pt, pref: f64, text: &str| -> Pt {
        let tw = parse_tex(text, ITALIC).0.iter().map(|(t, lv)| t.chars().count() as f64 * if *lv == 0 { 13.0 } else { 9.0 }).sum::<f64>();
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / len, dx / len);
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let d = (m.0 - mid.0) * nx + (m.1 - mid.1) * ny;
        let s = if d.abs() < 2.0 { pref } else { d.signum() };
        let off = 8.0 + (tw / 2.0 + 4.0) * nx.abs() + 13.0 * ny.abs();
        (m.0 + nx * s * off, m.1 + ny * s * off)
    };
    let v = (0..l.n).map(|i| place(pxy(l.pts[i]), pxy(l.pts[i + 1]), if l.n == 1 { -1.0 } else { 1.0 }, p.s(VK[i][4]))).collect();
    (v, place(pxy((0.0, 0.0)), pxy(l.res), -1.0, p.s("res_lab")))
}

fn vector_render(p: &Params) -> String {
    let (w, h) = (480.0, 360.0);
    let mut g = G::new(w, h, p.b("mirror"));
    g.halo = true;
    let col = p.b("color");
    let (vcol, ccol) = if col { ("#7c3aed", "#a78bfa") } else { (INK, "#6b7280") };
    if col { g.tcol = "#7c3aed"; }
    let VLay { n, pts, res, u, ox, oy, gx0, gx1, gy0, gy1 } = vlayout(p, w, h);
    let pxy = |q: Pt| (ox + q.0 * u, oy - q.1 * u);
    let mode = p.s("grid").to_string();
    let fs = 25.0;
    if mode == "Grid" {
        let mut x = gx0;
        while x <= gx1 + 1e-9 { let a = pxy((x, gy0)); let b = pxy((x, gy1)); g.line(a.0, a.1, b.0, b.1, "#cdd1d8", 1.2); x += 1.0; }
        let mut y = gy0;
        while y <= gy1 + 1e-9 { let a = pxy((gx0, y)); let b = pxy((gx1, y)); g.line(a.0, a.1, b.0, b.1, "#cdd1d8", 1.2); y += 1.0; }
    }
    if mode == "Axes only" || p.b("axes") {
        // double-headed axes through the first tail
        let (a, b) = (pxy((gx0 + 0.25, 0.0)), pxy((gx1 - 0.25, 0.0)));
        g.dim_arrow(a.0, a.1, b.0, b.1, INK, 1.8);
        let (c, d) = (pxy((0.0, gy0 + 0.25)), pxy((0.0, gy1 - 0.25)));
        g.dim_arrow(c.0, c.1, d.0, d.1, INK, 1.8);
        let tc = g.tcol;
        g.tcol = "#111";
        g.tex(b.0 + 10.0, b.1 + 8.0, 24.0, "start", "x", ITALIC);
        g.tex(d.0, d.1 - 10.0, 24.0, "middle", "y", ITALIC);
        g.tcol = tc;
    }
    let lab = |i: usize| lab_mode(p, VK[i][4], VMODE[i][0]);
    // components of the resultant (of vector 1 when there is no resultant)
    if p.b("components") && n >= 1 {
        let (o, c, r) = (pxy((0.0, 0.0)), pxy((res.0, 0.0)), pxy(res));
        if res.0.abs() > 0.01 { g.dash_arrow(o.0, o.1, c.0, c.1, ccol, 2.2); }
        if res.1.abs() > 0.01 { g.dash_arrow(c.0, c.1, r.0, r.1, ccol, 2.2); }
        let base = if n >= 2 && p.b("resultant") { p.s("res_lab").to_string() } else { p.s(VK[0][4]).to_string() };
        let base = { let b = base.trim(); if b.is_empty() || b.eq_ignore_ascii_case("none") { "R".to_string() } else { b.split('_').next().unwrap_or("R").to_string() } };
        let tc = g.tcol;
        g.tcol = if col { "#a78bfa" } else { "#6b7280" };
        let sx = if res.1 >= 0.0 { 1.0 } else { -1.0 };
        g.tex(o.0 + (c.0 - o.0) * 0.3, (o.1 + c.1) / 2.0 + 26.0 * sx + 4.0, fs, "middle", &format!("{base}_x"), ITALIC);
        let sg = if res.0 >= 0.0 { -1.0 } else { 1.0 };
        g.tex((c.0 + r.0) / 2.0 + 12.0 * sg, (c.1 + r.1) / 2.0 + 8.0, fs, if sg > 0.0 { "start" } else { "end" }, &format!("{base}_y"), ITALIC);
        g.tcol = tc;
    }
    let (vlabs, rlab) = vlabel_pos(p, &vlayout(p, w, h));
    // the vectors
    for i in 0..n {
        let (a, b) = (pxy(pts[i]), pxy(pts[i + 1]));
        let c = if col { vcol } else { colr(false, i) };
        styled_arrow(&mut g, p.s(VK[i][6]), a, b, c, SW + 1.0);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-6);
        let _ = l;
        g.label_at(vlabs[i].0, vlabs[i].1, fs, &lab(i), ITALIC);
    }
    if p.b("resultant") && n >= 2 {
        let (a, b) = (pxy((0.0, 0.0)), pxy(res));
        styled_arrow(&mut g, p.s("res_style"), a, b, vcol, SW + 1.0);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let _ = (dx, dy);
        g.label_at(rlab.0, rlab.1, fs, &lab_mode(p, "res_lab", "res_lab_mode"), ITALIC);
    }
    if p.b("mark_angle") && n >= 1 {
        let ang = res.1.atan2(res.0).to_degrees().rem_euclid(360.0);
        let o = pxy((0.0, 0.0));
        let r = (u * 1.1).clamp(22.0, 40.0);
        g.polyline(&arc_pts(o.0, o.1, r, 0.0, ang), INK, 1.6);
        let tc = g.tcol;
        g.tcol = "#111";
        match p.s("ang_lab_mode") {
            "Degrees" => {
                let mid = pol(o.0, o.1, r + 24.0, ang / 2.0);
                g.tex(mid.0, mid.1 + 6.0, 17.0, "middle", &format!("{:.1}°", ang), UPRIGHT);
            }
            m => {
                let mid = pol(o.0, o.1, r + 16.0, ang / 2.0);
                let t = if m == "Blank line" { String::new() } else { p.s("ang_lab").to_string() };
                g.label_at(mid.0 + 4.0, mid.1, 22.0, &t, ITALIC);
            }
        }
        g.tcol = tc;
    }
    g.finish()
}

// ---------------------------------------------------------------- inclined plane

const DIRS: [&str; 3] = ["None", "Up the ramp", "Down the ramp"];
const IMODES: [&str; 3] = ["Text", "Blank line", "None"];

fn incline_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("obj", "Object", &["Block", "Ball", "Cart"], "Block"),
        ParamDef::choice("obj_lab_mode", "Object label shows", &IMODES, "Text"),
        ParamDef::text("obj_lab", "Object label", "m"),
        ParamDef::int("size", "Object size (%)", 50, 200, 100),
        ParamDef::float("pos", "Where on the ramp", 0.15, 0.85, 0.05, 0.55),
        ParamDef::bool("two", "A second block higher up, tied by a string", false),
        ParamDef::text("obj2_lab", "Second block label", "m_2"),
        ParamDef::bool("tension", "Tension in the string", false),
        ParamDef::float("angle", "Ramp angle (deg)", 5.0, 75.0, 1.0, 30.0),
        ParamDef::bool("show_angle", "Mark the angle", true),
        ParamDef::choice("ang_lab_mode", "Angle label shows", &IMODES, "Text"),
        ParamDef::text("ang_lab", "Angle label (e.g. theta or 30°)", "theta"),
        ParamDef::choice("surface", "Surface", &["Smooth", "Rough"], "Smooth"),
        ParamDef::bool("gravity", "Gravity", false),
        ParamDef::choice("grav_lab_mode", "Gravity label shows", &IMODES, "Text"),
        ParamDef::text("grav_lab", "Gravity label", "F_g"),
        ParamDef::bool("normal", "Normal force", false),
        ParamDef::choice("norm_lab_mode", "Normal force label shows", &IMODES, "Text"),
        ParamDef::text("norm_lab", "Normal force label", "F_N"),
        ParamDef::choice("friction", "Friction", &DIRS, "None"),
        ParamDef::choice("fric_lab_mode", "Friction label shows", &IMODES, "Text"),
        ParamDef::text("fric_lab", "Friction label", "F_f"),
        ParamDef::choice("applied", "Applied force", &DIRS, "None"),
        ParamDef::choice("app_lab_mode", "Applied force label shows", &IMODES, "Text"),
        ParamDef::text("app_lab", "Applied force label", "F_A"),
        ParamDef::choice("velocity", "Velocity", &DIRS, "None"),
        ParamDef::choice("vel_lab_mode", "Velocity label shows", &IMODES, "Text"),
        ParamDef::text("vel_lab", "Velocity label", "v"),
        ParamDef::choice("accel", "Acceleration", &DIRS, "None"),
        ParamDef::choice("acc_lab_mode", "Acceleration label shows", &IMODES, "Text"),
        ParamDef::text("acc_lab", "Acceleration label", "a"),
        ParamDef::bool("mark_len", "Mark the length of the ramp", false),
        ParamDef::text("len_lab", "Length label", "L"),
        ParamDef::bool("mark_h", "Mark the height of the ramp", false),
        ParamDef::text("h_lab", "Height label", "h"),
        ParamDef::bool("mirror", "Mirror (flip left to right)", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

/// `F_g` -> `F_{g1}` for the k-th of two objects (labels without a subscript gain one).
fn indexed(lab: &str, k: usize, two: bool) -> String {
    if !two || lab.is_empty() || lab == "none" || lab.trim() == "T" { return lab.to_string(); }
    match lab.split_once('_') {
        Some((b, s)) => format!("{b}_{{{}{k}}}", s.trim_matches(|c| c == '{' || c == '}')),
        None => format!("{lab}_{k}"),
    }
}

/// A drawn object on the ramp: centre, half height (along the normal), half length (along the
/// ramp), whether it is round.
#[derive(Clone, Copy)]
struct RampObj { com: Pt, hh: f64, hl: f64, round: bool }

impl RampObj {
    /// Point where a ray from the centre along unit `d` leaves the object.
    fn exit(&self, d: Pt, uu: Pt, nn: Pt) -> Pt {
        let t = if self.round { self.hh } else {
            let (a, b) = ((d.0 * uu.0 + d.1 * uu.1).abs().max(1e-6), (d.0 * nn.0 + d.1 * nn.1).abs().max(1e-6));
            (self.hl / a).min(self.hh / b)
        };
        (self.com.0 + d.0 * t, self.com.1 + d.1 * t)
    }
}

/// Distance of the ramp-length mark above the slope for an object of half height `hh`.
pub(super) fn len_mark_off(hh: f64) -> f64 { 2.0 * hh + 34.0 }

fn incline_render(p: &Params) -> String {
    let (w, h) = (520.0, 330.0);
    let mut g = G::new(w, h, p.b("mirror"));
    let col = p.b("color");
    let th = p.f("angle").clamp(5.0, 75.0);
    let tr = th.to_radians();
    let ground = 285.0;
    let ax = 50.0;
    let bw = (390.0f64).min(205.0 / tr.tan());
    let (bx, tx, ty) = (ax + bw, ax + bw, ground - bw * tr.tan());
    let (uu, nn) = ((tr.cos(), -tr.sin()), (-tr.sin(), -tr.cos())); // up-ramp, outward normal
    let lh = bw / tr.cos();
    g.poly(&[(ax, ground), (bx, ground), (tx, ty)], "#f3f4f7", INK, SW);
    g.hatch(ax - 28.0, bx + 28.0, ground, true);
    // rough surface: short ticks just inside the slope
    if p.s("surface") == "Rough" {
        let mut s = 8.0;
        while s < lh - 4.0 {
            let (qx, qy) = (ax + uu.0 * s, ground + uu.1 * s);
            g.line(qx, qy, qx - nn.0 * 7.0 - uu.0 * 5.0, qy - nn.1 * 7.0 - uu.1 * 5.0, INK, 1.2);
            s += 11.0;
        }
    }
    // objects
    let sz = 50.0 * p.f("size") / 100.0;
    let two = p.b("two");
    let kind = p.s("obj").to_string();
    let draw_obj = |g: &mut G, kind: &str, s0: f64, lab: &str| -> RampObj {
        let base = (ax + uu.0 * s0, ground + uu.1 * s0);
        let loc = |l: f64, hgt: f64| (base.0 + uu.0 * l + nn.0 * hgt, base.1 + uu.1 * l + nn.1 * hgt);
        let o = match kind {
            "Ball" => {
                let r = sz / 2.0;
                let c = loc(0.0, r);
                g.circle(c.0, c.1, r, FILL, INK, SW);
                RampObj { com: c, hh: r, hl: r, round: true }
            }
            "Cart" => {
                let (len, hh, r) = (sz * 1.5, sz * 0.5, sz * 0.14);
                let pts = [loc(-len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r + hh), loc(-len / 2.0, 2.0 * r + hh)];
                g.poly(&pts, FILL, INK, SW);
                for l in [-len * 0.3, len * 0.3] { let c = loc(l, r); g.circle(c.0, c.1, r, "#fff", INK, SW); }
                RampObj { com: loc(0.0, 2.0 * r + hh / 2.0), hh: hh / 2.0 + r, hl: len / 2.0, round: false }
            }
            _ => {
                let pts = [loc(-sz / 2.0, 0.0), loc(sz / 2.0, 0.0), loc(sz / 2.0, sz), loc(-sz / 2.0, sz)];
                g.poly(&pts, FILL, INK, SW);
                RampObj { com: loc(0.0, sz / 2.0), hh: sz / 2.0, hl: sz / 2.0, round: false }
            }
        };
        let st = if lab.trim().chars().next().is_some_and(|c| c.is_ascii_digit()) { UPRIGHT } else { ITALIC };
        g.label_at(o.com.0, o.com.1, 20.0, lab, st);
        o
    };
    let pos = p.f("pos");
    let mut objs: Vec<RampObj> = vec![];
    if two {
        // the pair is centred on `pos`, the first object lower down
        let gap = (sz * 1.9).max(lh * 0.32);
        let s1 = (pos * lh - gap / 2.0).max(sz * 0.8);
        let s2 = (s1 + gap).min(lh - sz * 0.8);
        let o1 = draw_obj(&mut g, "Block", s1, &lab_mode(p, "obj_lab", "obj_lab_mode"));
        let o2 = draw_obj(&mut g, "Block", s2, p.s("obj2_lab"));
        // the string between them
        let a = (o1.com.0 + uu.0 * o1.hl, o1.com.1 + uu.1 * o1.hl);
        let b = (o2.com.0 - uu.0 * o2.hl, o2.com.1 - uu.1 * o2.hl);
        g.line(a.0, a.1, b.0, b.1, INK, 1.4);
        objs.push(o1);
        objs.push(o2);
    } else {
        objs.push(draw_obj(&mut g, &kind, pos * lh, &lab_mode(p, "obj_lab", "obj_lab_mode")));
    }
    let dirv = |d: &str| -> Option<Pt> {
        match d { "Up the ramp" => Some(uu), "Down the ramp" => Some((-uu.0, -uu.1)), _ => None }
    };
    let arrow_len = 46.0 + sz * 0.3;
    let force = |g: &mut G, from: Pt, d: Pt, lab: &str, i: usize| {
        let tip = (from.0 + d.0 * arrow_len, from.1 + d.1 * arrow_len);
        g.arrow(from.0, from.1, tip.0, tip.1, colr(col, i), SW);
        g.label_dir(tip.0, tip.1, d.0, d.1, 19.0, lab, ITALIC);
    };
    let n_obj = objs.len();
    for (k, o) in objs.iter().enumerate() {
        let k1 = k + 1;
        if p.b("gravity") {
            let from = o.exit((0.0, 1.0), uu, nn);
            force(&mut g, from, (0.0, 1.0), &indexed(&lab_mode(p, "grav_lab", "grav_lab_mode"), k1, two), 0);
        }
        if p.b("normal") {
            let from = (o.com.0 + nn.0 * o.hh, o.com.1 + nn.1 * o.hh);
            force(&mut g, from, nn, &indexed(&lab_mode(p, "norm_lab", "norm_lab_mode"), k1, two), 1);
        }
        if let Some(d) = dirv(p.s("friction")) {
            // along the surface from the face it acts towards
            let sgn = d.0 * uu.0 + d.1 * uu.1;
            let from = (o.com.0 + uu.0 * o.hl * sgn - nn.0 * o.hh * 0.55, o.com.1 + uu.1 * o.hl * sgn - nn.1 * o.hh * 0.55);
            force(&mut g, from, d, &indexed(&lab_mode(p, "fric_lab", "fric_lab_mode"), k1, two), 2);
        }
        if let Some(d) = dirv(p.s("applied")) {
            // applied force acts on the last (upper) object
            if k + 1 == n_obj {
                let sgn = d.0 * uu.0 + d.1 * uu.1;
                let from = (o.com.0 + uu.0 * o.hl * sgn, o.com.1 + uu.1 * o.hl * sgn);
                force(&mut g, from, d, &lab_mode(p, "app_lab", "app_lab_mode"), 3);
            }
        }
        if two && p.b("tension") {
            let (d, sgn) = if k == 0 { (uu, 1.0) } else { ((-uu.0, -uu.1), -1.0) };
            let from = (o.com.0 + uu.0 * o.hl * sgn, o.com.1 + uu.1 * o.hl * sgn);
            let tip = (from.0 + d.0 * 34.0, from.1 + d.1 * 34.0);
            g.arrow(from.0, from.1, tip.0, tip.1, colr(col, 4), SW);
            let m = ((from.0 + tip.0) / 2.0 + nn.0 * 14.0, (from.1 + tip.1) / 2.0 + nn.1 * 14.0);
            g.label_at(m.0, m.1, 18.0, "T", ITALIC);
        }
    }
    // velocity / acceleration: dashed, drawn beside (above) the upper object
    let top = *objs.last().unwrap();
    let mut off = top.hh + 34.0;
    for (key, mk, lk) in [("velocity", "vel_lab_mode", "vel_lab"), ("accel", "acc_lab_mode", "acc_lab")] {
        if let Some(d) = dirv(p.s(key)) {
            // above the object and a little ahead, clear of the normal force
            let ahead = top.hl * 0.9 + 14.0;
            let c = (top.com.0 + nn.0 * off + d.0 * ahead, top.com.1 + nn.1 * off + d.1 * ahead);
            let (a, b) = ((c.0 - d.0 * 26.0, c.1 - d.1 * 26.0), (c.0 + d.0 * 26.0, c.1 + d.1 * 26.0));
            g.dash_arrow(a.0, a.1, b.0, b.1, if col { PAL[1] } else { INK }, SW);
            g.label_dir(c.0 + nn.0 * 2.0, c.1 + nn.1 * 2.0, nn.0, nn.1, 19.0, &lab_mode(p, lk, mk), ITALIC);
            off += 34.0;
        }
    }
    // angle mark
    if p.b("show_angle") {
        let r = (lh * 0.22).clamp(34.0, 60.0);
        g.polyline(&arc_pts(ax, ground, r, 0.0, th), INK, 1.5);
        let m = pol(ax, ground, r + 20.0, th / 2.0);
        let t = lab_mode(p, "ang_lab", "ang_lab_mode");
        let st = if t.chars().next().is_some_and(|c| c.is_ascii_digit()) { UPRIGHT } else { ITALIC };
        g.label_at(m.0 + if st == UPRIGHT { 8.0 } else { 0.0 }, m.1, 20.0, &t, st);
    }
    if p.b("mark_len") {
        // dimension line above the slope, clear of the objects, with extension lines
        let o = len_mark_off(top.hh);
        let (a, b) = ((ax + nn.0 * o, ground + nn.1 * o), (tx + nn.0 * o, ty + nn.1 * o));
        g.line(ax + nn.0 * 6.0, ground + nn.1 * 6.0, ax + nn.0 * (o + 8.0), ground + nn.1 * (o + 8.0), INK, 1.0);
        g.line(tx + nn.0 * 6.0, ty + nn.1 * 6.0, tx + nn.0 * (o + 8.0), ty + nn.1 * (o + 8.0), INK, 1.0);
        g.dim_arrow(a.0, a.1, b.0, b.1, INK, 1.4);
        g.label_at((a.0 + b.0) / 2.0 + nn.0 * 14.0, (a.1 + b.1) / 2.0 + nn.1 * 14.0, 20.0, p.s("len_lab"), ITALIC);
    }
    if p.b("mark_h") {
        let x = bx + 28.0;
        g.line(tx, ty, x + 6.0, ty, INK, 1.0);
        g.dim_arrow(x, ty, x, ground, INK, 1.4);
        g.label_at(x + 14.0, (ty + ground) / 2.0, 20.0, p.s("h_lab"), ITALIC);
    }
    g.finish()
}

// ---------------------------------------------------------------- pulley (see pulley.rs)

use super::pulley::{pulley_params, pulley_render};

// ---------------------------------------------------------------- projectile motion

const PMODES: [&str; 3] = ["Text", "Blank line", "None"];

fn proj_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("from", "Launch from", &["Level ground", "A cliff"], "Level ground"),
        ParamDef::float("angle", "Launch angle (deg; 0 = horizontal, off a cliff)", 0.0, 85.0, 1.0, 45.0),
        ParamDef::float("cliff", "Cliff height (relative)", 0.1, 1.2, 0.05, 0.4),
        ParamDef::bool("show_angle", "Mark the angle", true),
        ParamDef::choice("ang_lab_mode", "Angle label shows", &PMODES, "Text"),
        ParamDef::text("ang_lab", "Angle label (e.g. theta or 30°)", "theta"),
        ParamDef::choice("object", "Object", &["Ball", "Dot"], "Ball"),
        ParamDef::int("size", "Object size (%)", 30, 150, 60),
        ParamDef::choice("path", "Path", &["Dashed", "Solid", "None"], "Dashed"),
        ParamDef::int("steps", "Ball at equal time steps (0 = off)", 0, 12, 0),
        ParamDef::bool("letters", "Letter the positions A, B, C...", false),
        ParamDef::bool("show_v", "Launch velocity", true),
        ParamDef::choice("v_lab_mode", "Velocity label shows", &PMODES, "Text"),
        ParamDef::text("v_lab", "Velocity label", "v_0"),
        ParamDef::bool("comps", "Its components", false),
        ParamDef::choice("vx_lab_mode", "Horizontal component label shows", &PMODES, "Text"),
        ParamDef::text("vx_lab", "Horizontal component label", "v_{0x}"),
        ParamDef::choice("vy_lab_mode", "Vertical component label shows", &PMODES, "Text"),
        ParamDef::text("vy_lab", "Vertical component label", "v_{0y}"),
        ParamDef::bool("show_g", "Acceleration due to gravity", false),
        ParamDef::choice("g_lab_mode", "Gravity label shows", &PMODES, "Text"),
        ParamDef::text("g_lab", "Gravity label", "g"),
        ParamDef::bool("mark_h", "Mark the maximum height", false),
        ParamDef::text("h_lab", "Height label", "H"),
        ParamDef::bool("mark_r", "Mark the range", false),
        ParamDef::text("r_lab", "Range label", "R"),
        ParamDef::bool("mark_cliff", "Mark the cliff height", false),
        ParamDef::text("cliff_lab", "Cliff height label", "h"),
        ParamDef::bool("mirror", "Mirror (flip left to right)", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

pub(super) struct PG { pub th: f64, pub vx: f64, pub vy: f64, #[allow(dead_code)] pub h0: f64, pub r: f64, pub ground: f64, pub lx: f64, pub ly: f64, pub s: f64, pub t_land: f64 }

/// Launch geometry of the projectile figure (canvas width `w`).
pub(super) fn proj_geo(p: &Params, w: f64) -> PG {
    let cliff = p.s("from") == "A cliff";
    // a horizontal launch only makes sense off a cliff
    let th = p.f("angle").clamp(if cliff { 0.0 } else { 5.0 }, 85.0);
    let (vx, vy) = (th.to_radians().cos(), th.to_radians().sin());
    let h0 = if cliff { p.f("cliff") } else { 0.0 };
    let r = if p.s("object") == "Dot" { 4.0 } else { 34.0 * p.f("size") / 100.0 };
    let ground = 285.0;
    let lx = if cliff { 100.0 } else { 70.0 };
    // normalised flight (v0 = g = 1); scale to fit the canvas
    let t_land0 = vy + (vy * vy + 2.0 * h0).sqrt();
    let range0 = vx * t_land0;
    let hmax0 = h0 + vy * vy / 2.0;
    let s = ((w - lx - 60.0) / range0).min((ground - 50.0 - 2.0 * r) / hmax0).min(500.0);
    let drop_px = h0 * s + r; // launch centre above the ground line
    let ly = ground - drop_px;
    let t_land = vy + (vy * vy + 2.0 * drop_px / s).sqrt();
    PG { th, vx, vy, h0, r, ground, lx, ly, s, t_land }
}

fn proj_render(p: &Params) -> String {
    let (w, h) = (560.0, 340.0);
    let mut g = G::new(w, h, p.b("mirror"));
    let col = p.b("color");
    let cliff = p.s("from") == "A cliff";
    let PG { th, vx, vy, r, ground, lx, ly, s, t_land, .. } = proj_geo(p, w);
    let pos = |t: f64| (lx + s * vx * t, ly - s * (vy * t - 0.5 * t * t));
    // ground (a plain line, as in the reference) and cliff
    if cliff {
        g.poly(&[(46.0, ly + r), (lx + r, ly + r), (lx + r, ground), (46.0, ground)], "#f3f4f7", INK, 1.8);
    }
    g.line(10.0, ground, w - 10.0, ground, INK, 1.8);
    // path
    let pts: Vec<Pt> = (0..=80).map(|i| pos(t_land * i as f64 / 80.0)).collect();
    let pc = if col { PAL[1] } else { INK };
    match p.s("path") {
        "Dashed" => g.pattern_polyline(&pts, pc, 1.8, "7 5"),
        "Solid" => g.polyline(&pts, pc, 2.0),
        _ => {}
    }
    let end = *pts.last().unwrap();
    // equal time steps (with optional letters; A is the launch position)
    let steps = p.i("steps");
    let letters = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M"];
    let br = if p.s("object") == "Dot" { 4.5 } else { r };
    if steps > 0 {
        for i in 1..=steps {
            let q = pos(t_land * i as f64 / steps as f64);
            if p.s("object") == "Dot" { g.circle(q.0, q.1, 4.5, INK, INK, 1.0); } else { g.circle(q.0, q.1, r, FILL, INK, 1.8); }
        }
    }
    if p.b("letters") {
        let n = steps.max(0) as usize;
        for i in 0..=n {
            let t = if n == 0 { 0.0 } else { t_land * i as f64 / n as f64 };
            let q = pos(t);
            // above the ball, nudged outwards from the middle of the flight
            let dx = if t < vy - 1e-9 { -1.0 } else if t > vy + 1e-9 { 1.0 } else { 0.0 };
            g.tex(q.0 + dx * (br + 6.0), q.1 - br - 8.0, 18.0, if dx < 0.0 { "end" } else if dx > 0.0 { "start" } else { "middle" }, letters[i.min(12)], UPRIGHT);
        }
    }
    // launch object
    if p.s("object") == "Dot" { g.circle(lx, ly, 4.5, INK, INK, 1.0); } else { g.circle(lx, ly, r, FILL, INK, SW); }
    // velocity, components, angle: dashed arrows (they are not forces)
    let vlen = 56.0;
    let tip = pol(lx, ly, br + vlen, th);
    let vc = if col { PAL[0] } else { INK };
    if p.b("comps") {
        let cc = if col { PAL[2] } else { GREY };
        let (ex, ey) = (lx + (br + vlen) * vx, ly);
        let tc = g.tcol;
        g.tcol = if col { PAL[2] } else { GREY };
        if vx > 0.05 {
            g.dash_arrow(lx + br * vx, ly, ex, ey, cc, 1.5);
            g.label_at(ex + 22.0, ly + 2.0, 17.0, &lab_mode(p, "vx_lab", "vx_lab_mode"), ITALIC);
        }
        if vy > 0.05 {
            let up = (lx, ly - (br + vlen) * vy);
            g.dash_arrow(lx, ly - br, up.0, up.1, cc, 1.5);
            g.dash(up.0, up.1, tip.0, tip.1, cc, 0.9);
            g.label_at(lx - br - 20.0, (ly + up.1) / 2.0, 17.0, &lab_mode(p, "vy_lab", "vy_lab_mode"), ITALIC);
        }
        g.tcol = tc;
    }
    if p.b("show_v") {
        let a = pol(lx, ly, br, th);
        g.dash_arrow(a.0, a.1, tip.0, tip.1, vc, SW);
        g.label_dir(tip.0, tip.1, -0.2, -1.0, 19.0, &lab_mode(p, "v_lab", "v_lab_mode"), ITALIC);
    }
    if p.b("show_angle") && th > 0.5 {
        let ar = br + 26.0;
        g.dash(lx + br, ly, lx + ar + 12.0, ly, GREY, 1.0);
        g.polyline(&arc_pts(lx, ly, ar, 0.0, th), INK, 1.4);
        let m = pol(lx, ly, ar + 15.0, th / 2.0);
        let t = lab_mode(p, "ang_lab", "ang_lab_mode");
        let st = if t.chars().next().is_some_and(|c| c.is_ascii_digit()) { UPRIGHT } else { ITALIC };
        g.label_at(m.0 + 6.0, m.1, 18.0, &t, st);
    }
    if p.b("show_g") {
        // beside the path on its way down
        // under the path on its way down, clear of the curve
        let q = pos(vy + (t_land - vy) * 0.45);
        let gx = q.0.min(w - 30.0);
        let gy = (q.1 + br + 14.0).min(ground - 52.0);
        g.dash_arrow(gx, gy, gx, gy + 40.0, if col { PAL[3] } else { INK }, SW);
        g.label_dir(gx + 2.0, gy + 20.0, 1.0, 0.0, 18.0, &lab_mode(p, "g_lab", "g_lab_mode"), ITALIC);
    }
    if p.b("mark_h") && vy > 0.05 {
        // from the launch level up to the top of the flight, at the apex
        let apex = pos(vy);
        let top = apex.1 - br;
        g.dim_arrow(apex.0, top, apex.0, ly, INK, 1.2);
        g.label_at(apex.0 + 14.0, (top + ly) / 2.0, 19.0, p.s("h_lab"), ITALIC);
    }
    if p.b("mark_r") {
        let (a, b) = (lx, end.0);
        let y = ground + 22.0;
        g.line(a, ground + 2.0, a, y + 6.0, INK, 0.8);
        g.line(b, ground + 2.0, b, y + 6.0, INK, 0.8);
        g.dim_arrow(a, y, b, y, INK, 1.2);
        g.label_at((a + b) / 2.0, y + 16.0, 19.0, p.s("r_lab"), ITALIC);
    }
    if p.b("mark_cliff") && cliff {
        let x = 32.0;
        let top = ly + r;
        g.line(x - 4.0, top, 46.0, top, INK, 0.8);
        g.dim_arrow(x, top, x, ground, INK, 1.2);
        g.label_at(x - 13.0, (top + ground) / 2.0, 19.0, p.s("cliff_lab"), ITALIC);
    }
    g.finish()
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "free-body-diagram", subject: Subject::Physics, title: "Free Body Diagram",
            blurb: "A dot, block, ball or cart with labelled force arrows at any angle, plus optional velocity and acceleration.",
            params: fbd_params, render: fbd_render },
        FigureDef { id: "vector-diagram", subject: Subject::Physics, title: "Vector Diagram",
            blurb: "Head-to-tail vectors on a grid with a resultant, its angle and its components.",
            params: vector_params, render: vector_render },
        FigureDef { id: "inclined-plane", subject: Subject::Physics, title: "Inclined Plane",
            blurb: "A block, ball or cart on a ramp of any angle, with force, velocity and acceleration arrows.",
            params: incline_params, render: incline_render },
        FigureDef { id: "pulley", subject: Subject::Physics, title: "Pulley",
            blurb: "Atwood machine, table, ramp and block-and-tackle pulley setups with tension, weight and acceleration marks.",
            params: pulley_params, render: pulley_render },
        FigureDef { id: "projectile-motion", subject: Subject::Physics, title: "Projectile Motion",
            blurb: "A parabolic flight from the ground or a cliff with launch velocity, components, time-step positions and marks.",
            params: proj_params, render: proj_render },
    ]
}
