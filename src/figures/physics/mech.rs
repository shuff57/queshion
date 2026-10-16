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
const FDEF: [(f64, &str); 6] = [(270.0, "F_g"), (90.0, "F_N"), (0.0, "F_f"), (180.0, "F_T"), (45.0, "F_app"), (225.0, "F_s")];

fn fbd_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("body", "Body", &["Dot", "Block", "Ball", "Cart"], "Dot"),
        ParamDef::int("n", "Number of forces", 1, 6, 2),
    ];
    for (i, k) in FK.iter().enumerate() {
        v.push(ParamDef::float(k[0], k[1], 0.0, 360.0, 1.0, FDEF[i].0));
        v.push(ParamDef::float(k[2], k[3], 0.4, 1.6, 0.05, 1.0));
        v.push(ParamDef::text(k[4], k[5], FDEF[i].1));
    }
    v.push(ParamDef::choice("notation", "Vector notation", &["None", "Arrow", "Bold"], "None"));
    v.push(ParamDef::choice("motion", "Motion (drawn beside the body)", &["None", "Velocity", "Acceleration", "Both"], "None"));
    v.push(ParamDef::float("mdir", "Motion direction (deg)", 0.0, 360.0, 1.0, 0.0));
    v.push(ParamDef::bool("mirror", "Mirror (flip left to right)", false));
    v.push(ParamDef::bool("color", "Color (for slides)", false));
    v
}

fn ray_exit(kind: &str, ang: f64) -> f64 {
    let (hw, hh, circ) = match kind { "Block" => (38.0, 28.0, false), "Ball" => (28.0, 28.0, true), "Cart" => (48.0, 22.0, false), _ => (6.0, 6.0, true) };
    if circ { return hw; }
    let a = ang.to_radians();
    let (c, s) = (a.cos().abs().max(1e-6), a.sin().abs().max(1e-6));
    (hw / c).min(hh / s)
}

fn fbd_render(p: &Params) -> String {
    let mut g = G::new(480.0, 360.0, p.b("mirror"));
    let col = p.b("color");
    let (cx, cy) = (240.0, 180.0);
    let body = p.s("body").to_string();
    match body.as_str() {
        "Block" => g.rect(cx - 38.0, cy - 28.0, 76.0, 56.0, FILL, INK, SW),
        "Ball" => g.circle(cx, cy, 28.0, FILL, INK, SW),
        "Cart" => {
            g.rect(cx - 48.0, cy - 22.0, 96.0, 28.0, FILL, INK, SW);
            g.circle(cx - 26.0, cy + 12.0, 9.0, FILL, INK, SW);
            g.circle(cx + 26.0, cy + 12.0, 9.0, FILL, INK, SW);
        }
        _ => {}
    }
    let st = match p.s("notation") { "Arrow" => VEC, "Bold" => BOLD, _ => ITALIC };
    for i in 0..(p.i("n").clamp(1, 6) as usize) {
        let ang = p.f(FK[i][0]);
        let len = p.f(FK[i][2]);
        let total = ray_exit(&body, ang) + 20.0 + 55.0 * len;
        let (tx, ty) = pol(cx, cy, total, ang);
        let c = colr(col, i);
        g.arrow(cx, cy, tx, ty, c, SW);
        let (ux, uy) = ((tx - cx) / total, (ty - cy) / total);
        g.label_dir(tx, ty, ux, uy, 24.0, p.s(FK[i][4]), st);
    }
    if body == "Dot" { g.circle(cx, cy, 6.0, INK, INK, 1.0); }
    // motion indicators beside the body
    let m = p.s("motion").to_string();
    let md = p.f("mdir");
    let mut row = 0.0;
    for (name, lab) in [("Velocity", "v"), ("Acceleration", "a")] {
        if m == name || m == "Both" {
            let (mx, my) = (415.0, 292.0 + row);
            let (a, b) = (pol(mx, my, -26.0, md), pol(mx, my, 26.0, md));
            g.dash_arrow(a.0, a.1, b.0, b.1, if col { PAL[1] } else { INK }, SW);
            g.label_dir(b.0, b.1, (b.0 - mx) / 26.0, (b.1 - my) / 26.0, 20.0, lab, st);
            row += 40.0;
        }
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

fn vector_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::int("n", "Number of vectors", 1, 4, 2)];
    for (i, k) in VK.iter().enumerate() {
        v.push(ParamDef::float(k[0], k[1], 0.0, 10.0, 0.5, VDEF[i].0));
        v.push(ParamDef::float(k[2], k[3], 0.0, 360.0, 1.0, VDEF[i].1));
        v.push(ParamDef::text(k[4], k[5], VDEF[i].2));
        v.push(ParamDef::choice(k[6], k[7], &ARROW_STYLES, "Drawn"));
    }
    v.push(ParamDef::bool("resultant", "Show resultant", true));
    v.push(ParamDef::choice("res_style", "Resultant arrow", &ARROW_STYLES, "Drawn"));
    v.push(ParamDef::text("res_lab", "Resultant label", "R"));
    v.push(ParamDef::bool("mark_angle", "Mark its angle", false));
    v.push(ParamDef::bool("components", "Show its components", false));
    v.push(ParamDef::choice("grid", "Grid and axes", &["Grid", "Axes only", "None"], "Grid"));
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

fn vector_render(p: &Params) -> String {
    let (w, h) = (480.0, 360.0);
    let mut g = G::new(w, h, p.b("mirror"));
    let col = p.b("color");
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
    let u = ((w - 60.0) / (gx1 - gx0)).min((h - 50.0) / (gy1 - gy0)).min(46.0);
    let (ox, oy) = ((w - u * (gx1 - gx0)) / 2.0 - gx0 * u, (h + u * (gy1 - gy0)) / 2.0 + gy0 * u);
    let pxy = |q: Pt| (ox + q.0 * u, oy - q.1 * u);
    let mode = p.s("grid").to_string();
    if mode == "Grid" {
        let mut x = gx0;
        while x <= gx1 + 1e-9 { let a = pxy((x, gy0)); let b = pxy((x, gy1)); g.line(a.0, a.1, b.0, b.1, "#b6bcc6", 1.0); x += 1.0; }
        let mut y = gy0;
        while y <= gy1 + 1e-9 { let a = pxy((gx0, y)); let b = pxy((gx1, y)); g.line(a.0, a.1, b.0, b.1, "#b6bcc6", 1.0); y += 1.0; }
        let (a, b) = (pxy((gx0, gy0)), pxy((gx1, gy1)));
        g.polyline(&[(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1), (a.0, a.1)], "#8a919d", 1.2);
    } else if mode == "Axes only" {
        let (a, b) = (pxy((gx0, 0.0)), pxy((gx1, 0.0)));
        g.arrow(a.0, a.1, b.0, b.1, INK, 1.6);
        let (c, d) = (pxy((0.0, gy0)), pxy((0.0, gy1)));
        g.arrow(c.0, c.1, d.0, d.1, INK, 1.6);
        g.tex(b.0 - 4.0, b.1 + 20.0, 16.0, "end", "x", ITALIC);
        g.tex(d.0 + 10.0, d.1 + 8.0, 16.0, "start", "y", ITALIC);
    }
    // components of the resultant
    if p.b("components") && n >= 1 {
        let (o, c, r) = (pxy((0.0, 0.0)), pxy((res.0, 0.0)), pxy(res));
        let cc = if col { PAL[3] } else { GREY };
        if res.0.abs() > 0.01 { g.dash_arrow(o.0, o.1, c.0, c.1, cc, 1.8); }
        if res.1.abs() > 0.01 { g.dash_arrow(c.0, c.1, r.0, r.1, cc, 1.8); }
        let sx = if res.1 >= 0.0 { 1.0 } else { -1.0 };
        g.tex((o.0 + c.0) / 2.0, (o.1 + c.1) / 2.0 + 20.0 * sx + 5.0, 16.0, "middle", "R_x", ITALIC);
        let sg = if res.0 >= 0.0 { 1.0 } else { -1.0 };
        g.tex((c.0 + r.0) / 2.0 + 10.0 * sg, (c.1 + r.1) / 2.0 + 5.0, 16.0, if sg > 0.0 { "start" } else { "end" }, "R_y", ITALIC);
    }
    // the vectors
    for i in 0..n {
        let (a, b) = (pxy(pts[i]), pxy(pts[i + 1]));
        let c = colr(col, i);
        styled_arrow(&mut g, p.s(VK[i][6]), a, b, c, SW);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-6);
        // right-hand side of the direction of travel (pixel y-down: rotate (dx,dy) by +90deg)
        let (nx, ny) = (-dy / l, dx / l);
        g.label_at((a.0 + b.0) / 2.0 + nx * 17.0, (a.1 + b.1) / 2.0 + ny * 17.0, 20.0, p.s(VK[i][4]), ITALIC);
    }
    if p.b("resultant") && n >= 2 {
        let (a, b) = (pxy((0.0, 0.0)), pxy(res));
        let c = if col { PAL[0] } else { INK };
        styled_arrow(&mut g, p.s("res_style"), a, b, c, SW + 0.5);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / l, dx / l);
        g.label_at((a.0 + b.0) / 2.0 - nx * 18.0, (a.1 + b.1) / 2.0 - ny * 18.0, 20.0, p.s("res_lab"), ITALIC);
    }
    if p.b("mark_angle") && n >= 1 {
        let ang = res.1.atan2(res.0).to_degrees().rem_euclid(360.0);
        let o = pxy((0.0, 0.0));
        let r = (u * 1.1).clamp(22.0, 40.0);
        g.polyline(&arc_pts(o.0, o.1, r, 0.0, ang), INK, 1.4);
        let mid = pol(o.0, o.1, r + 24.0, ang / 2.0);
        g.tex(mid.0, mid.1 + 6.0, 17.0, "middle", &format!("{:.1}°", ang), UPRIGHT);
    }
    g.finish()
}

// ---------------------------------------------------------------- inclined plane

const DIRS: [&str; 3] = ["None", "Up the ramp", "Down the ramp"];

fn incline_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("obj", "Object", &["Block", "Ball", "Cart"], "Block"),
        ParamDef::text("obj_lab", "Object label", "m"),
        ParamDef::int("size", "Object size (%)", 50, 150, 100),
        ParamDef::float("pos", "Where on the ramp", 0.15, 0.85, 0.05, 0.55),
        ParamDef::float("angle", "Ramp angle (deg)", 5.0, 75.0, 1.0, 30.0),
        ParamDef::bool("show_angle", "Mark the angle", true),
        ParamDef::text("ang_lab", "Angle label", "theta"),
        ParamDef::choice("surface", "Surface", &["Smooth", "Rough"], "Smooth"),
        ParamDef::bool("gravity", "Gravity", false),
        ParamDef::bool("normal", "Normal force", false),
        ParamDef::choice("friction", "Friction", &DIRS, "None"),
        ParamDef::choice("applied", "Applied force", &DIRS, "None"),
        ParamDef::choice("velocity", "Velocity", &DIRS, "None"),
        ParamDef::choice("accel", "Acceleration", &DIRS, "None"),
        ParamDef::bool("mark_len", "Mark the length of the ramp", false),
        ParamDef::bool("mark_h", "Mark the height of the ramp", false),
        ParamDef::bool("mirror", "Mirror (flip left to right)", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

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
    // rough surface texture just inside the slope
    if p.s("surface") == "Rough" {
        let mut s = 8.0;
        while s < lh - 4.0 {
            let (qx, qy) = (ax + uu.0 * s, ground + uu.1 * s);
            g.line(qx, qy, qx - nn.0 * 7.0 - uu.0 * 5.0, qy - nn.1 * 7.0 - uu.1 * 5.0, INK, 1.2);
            s += 11.0;
        }
    }
    // object
    let sz = 50.0 * p.f("size") / 100.0;
    let s0 = p.f("pos") * lh;
    let base = (ax + uu.0 * s0, ground + uu.1 * s0);
    let kind = p.s("obj").to_string();
    let (com, half_h, half_l) = match kind.as_str() {
        "Ball" => {
            let r = sz / 2.0;
            let c = (base.0 + nn.0 * r, base.1 + nn.1 * r);
            g.circle(c.0, c.1, r, FILL, INK, SW);
            (c, r, r)
        }
        "Cart" => {
            let (len, hh, r) = (sz * 1.5, sz * 0.5, sz * 0.14);
            let loc = |l: f64, hgt: f64| (base.0 + uu.0 * l + nn.0 * hgt, base.1 + uu.1 * l + nn.1 * hgt);
            let pts = [loc(-len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r + hh), loc(-len / 2.0, 2.0 * r + hh)];
            g.poly(&pts, FILL, INK, SW);
            for l in [-len * 0.3, len * 0.3] { let c = loc(l, r); g.circle(c.0, c.1, r, "#fff", INK, SW); }
            (loc(0.0, 2.0 * r + hh / 2.0), 2.0 * r + hh / 2.0, len / 2.0)
        }
        _ => {
            let loc = |l: f64, hgt: f64| (base.0 + uu.0 * l + nn.0 * hgt, base.1 + uu.1 * l + nn.1 * hgt);
            let pts = [loc(-sz / 2.0, 0.0), loc(sz / 2.0, 0.0), loc(sz / 2.0, sz), loc(-sz / 2.0, sz)];
            g.poly(&pts, FILL, INK, SW);
            (loc(0.0, sz / 2.0), sz / 2.0, sz / 2.0)
        }
    };
    if kind != "Cart" || true { g.label_at(com.0, com.1, 20.0, p.s("obj_lab"), ITALIC); }
    // force arrows from the centre of the object
    let dirv = |d: &str, sgn: f64| -> Option<(f64, f64)> {
        match d { "Up the ramp" => Some((uu.0 * sgn, uu.1 * sgn)), "Down the ramp" => Some((-uu.0 * sgn, -uu.1 * sgn)), _ => None }
    };
    let mut forces: Vec<((f64, f64), &str, &str, usize)> = Vec::new();
    if p.b("gravity") { forces.push(((0.0, 1.0), "F_g", "g", 0)); }
    if p.b("normal") { forces.push((nn, "F_N", "n", 1)); }
    if let Some(d) = dirv(p.s("friction"), 1.0) { forces.push((d, "F_f", "f", 2)); }
    if let Some(d) = dirv(p.s("applied"), 1.0) { forces.push((d, "F_app", "a", 3)); }
    for (d, lab, _k, i) in forces {
        let l = 62.0 + half_h.max(half_l) * 0.9;
        let tip = (com.0 + d.0 * l, com.1 + d.1 * l);
        g.arrow(com.0, com.1, tip.0, tip.1, colr(col, i), SW);
        g.label_dir(tip.0, tip.1, d.0, d.1, 20.0, lab, ITALIC);
    }
    // velocity / acceleration: dashed, drawn beside the object (off the ramp side)
    let mut off = half_h + 34.0;
    for (name, key, lab) in [("velocity", "velocity", "v"), ("accel", "accel", "a")] {
        let _ = name;
        if let Some(d) = dirv(p.s(key), 1.0) {
            let c = (com.0 + nn.0 * off, com.1 + nn.1 * off);
            let (a, b) = ((c.0 - d.0 * 30.0, c.1 - d.1 * 30.0), (c.0 + d.0 * 30.0, c.1 + d.1 * 30.0));
            g.dash_arrow(a.0, a.1, b.0, b.1, if col { PAL[1] } else { INK }, SW);
            g.label_dir(b.0, b.1, d.0, d.1, 20.0, lab, ITALIC);
            off += 30.0;
        }
    }
    // angle mark
    if p.b("show_angle") {
        let r = (lh * 0.22).clamp(34.0, 60.0);
        g.polyline(&arc_pts(ax, ground, r, 0.0, th), INK, 1.5);
        let m = pol(ax, ground, r + 20.0, th / 2.0);
        g.label_at(m.0, m.1, 20.0, p.s("ang_lab"), ITALIC);
    }
    if p.b("mark_len") {
        let o = -26.0;
        let (s0, s1) = (46.0, lh - 24.0);
        let (a, b) = ((ax + uu.0 * s0 + nn.0 * o, ground + uu.1 * s0 + nn.1 * o), (ax + uu.0 * s1 + nn.0 * o, ground + uu.1 * s1 + nn.1 * o));
                g.dim_arrow(a.0, a.1, b.0, b.1, INK, 1.4);
        g.label_at((a.0 + b.0) / 2.0 - nn.0 * 14.0, (a.1 + b.1) / 2.0 - nn.1 * 14.0, 20.0, "L", ITALIC);
    }
    if p.b("mark_h") {
        let x = bx + 28.0;
        g.line(tx, ty, x + 6.0, ty, INK, 1.0);
        g.dim_arrow(x, ty, x, ground, INK, 1.4);
        g.label_at(x + 14.0, (ty + ground) / 2.0, 20.0, "h", ITALIC);
    }
    g.finish()
}

// ---------------------------------------------------------------- pulley

fn pulley_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("setup", "Setup", &["Atwood", "Table", "Ramp", "Block & tackle"], "Atwood"),
        ParamDef::text("l_lab", "Left object label", "m_1"),
        ParamDef::int("l_size", "Left object size (%)", 60, 150, 100),
        ParamDef::text("r_lab", "Right object label", "m_2"),
        ParamDef::int("r_size", "Right object size (%)", 60, 150, 100),
        ParamDef::choice("lower", "Hangs lower", &["Neither", "Left", "Right"], "Neither"),
        ParamDef::bool("third", "Hang a third object below", false),
        ParamDef::text("t_lab", "Third object label", "m_3"),
        ParamDef::float("angle", "Ramp angle (deg)", 15.0, 60.0, 1.0, 30.0),
        ParamDef::bool("tension", "Tension", false),
        ParamDef::bool("gravity", "Gravity", false),
        ParamDef::choice("accel", "Acceleration", &["None", "Right falls", "Left falls"], "None"),
        ParamDef::bool("mirror", "Mirror (flip left to right)", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

fn pulley_wheel(g: &mut G, cx: f64, cy: f64, r: f64) {
    g.circle(cx, cy, r, "#f3f4f7", INK, SW);
    g.circle(cx, cy, 8.0 * (r / 44.0).max(0.6), "none", INK, 1.8);
    g.circle(cx, cy, 2.6, INK, INK, 1.0);
}

fn mass_box(g: &mut G, cx: f64, top: f64, s: f64, lab: &str) {
    g.rect(cx - s / 2.0, top, s, s, FILL, INK, SW);
    g.label_at(cx, top + s / 2.0, 22.0, lab, ITALIC);
}

fn pulley_render(p: &Params) -> String {
    let (w, h) = (480.0, 430.0);
    let mut g = G::new(w, h, p.b("mirror"));
    let col = p.b("color");
    let setup = p.s("setup").to_string();
    let (ls, rs) = (60.0 * p.f("l_size") / 100.0, 60.0 * p.f("r_size") / 100.0);
    let lower = p.s("lower").to_string();
    let (dl, dr) = (if lower == "Left" { 45.0 } else { 0.0 }, if lower == "Right" { 45.0 } else { 0.0 });
    let tcol = colr(col, 0);
    let gcol = colr(col, 1);
    let acol = colr(col, 2);
    let accel = p.s("accel").to_string();
    // helper-collected geometry: (center x, top y, size) of the hanging masses
    let mut hang: Vec<(f64, f64, f64)> = Vec::new();
    let r = 42.0;
    match setup.as_str() {
        "Table" => {
            // tabletop at left, pulley at its right edge, right mass hangs
            let (top, edge) = (170.0, 290.0);
            g.rect(40.0, top, edge - 40.0, 14.0, "#f3f4f7", INK, SW);
            g.line(60.0, top + 14.0, 60.0, 385.0, INK, SW);
            g.line(edge - 20.0, top + 14.0, edge - 20.0, 385.0, INK, SW);
            let pc = (edge + 24.0, top - ls / 2.0 + 0.0 + r * 0.0 + 2.0);
            let pr = 24.0;
            let pcy = top - ls / 2.0 + pr;
            let _ = pc;
            let pcx = edge + 8.0;
            g.line(edge, top + 7.0, pcx, pcy, INK, SW);
            pulley_wheel(&mut g, pcx, pcy, pr);
            let lx = 160.0 + 0.0;
            let ytop = top - ls;
            let sy = pcy - pr;
            mass_box(&mut g, lx, ytop, ls, p.s("l_lab"));
            g.line(lx + ls / 2.0, sy.max(ytop + ls / 2.0) , pcx, sy, INK, 2.0);
            let _ = (ytop, sy);
            let rx = pcx + pr;
            let rt = 250.0 + dr;
            g.line(rx, pcy, rx, rt, INK, 2.0);
            hang.push((rx, rt, rs));
            mass_box(&mut g, rx, rt, rs, p.s("r_lab"));
            if p.b("tension") {
                g.arrow(lx + ls / 2.0, ytop + ls / 2.0, lx + ls / 2.0 + 56.0, ytop + ls / 2.0, tcol, SW);
                g.tex(lx + ls / 2.0 + 28.0, ytop + ls / 2.0 - 8.0, 20.0, "middle", "T", ITALIC);
            }
            if p.b("gravity") {
                g.arrow(lx, ytop + ls / 2.0, lx, ytop + ls / 2.0 + 56.0, gcol, SW);
                g.tex(lx + 8.0, ytop + ls / 2.0 + 56.0, 20.0, "start", "F_g", ITALIC);
            }
            if accel != "None" {
                let right = accel == "Right falls";
                let (a, b) = if right { (lx - ls / 2.0 - 70.0, lx - ls / 2.0 - 8.0) } else { (lx - ls / 2.0 - 8.0, lx - ls / 2.0 - 70.0) };
                g.arrow(a, ytop + ls / 2.0, b, ytop + ls / 2.0, acol, SW);
                g.tex((a + b) / 2.0, ytop + ls / 2.0 - 10.0, 20.0, "middle", "a", ITALIC);
            }
        }
        "Ramp" => {
            let th = p.f("angle").to_radians();
            let ground = 350.0;
            let ax = 30.0;
            let hgt = 190.0;
            let bw = (hgt / th.tan()).min(280.0);
            let tp = (ax + bw, ground - bw * th.tan());
            g.poly(&[(ax, ground), (tp.0, ground), tp], "#f3f4f7", INK, SW);
            g.hatch(ax - 10.0, tp.0 + 80.0, ground, true);
            let uu = (th.cos(), -th.sin());
            let nn = (-th.sin(), -th.cos());
            let sz = ls;
            // string line is parallel to the slope through the block centre
            let hh = sz / 2.0;
            // pulley sits at the apex, circle tangent to the string line
            let pr = 24.0;
            let q = (tp.0 + nn.0 * hh + uu.0 * hh * th.tan(), tp.1 + nn.1 * hh + uu.1 * hh * th.tan());
            let c = (q.0 - nn.0 * pr, q.1 - nn.1 * pr);
            let sb = 0.5 * ((tp.0 - ax) / th.cos());
            let base = (ax + uu.0 * sb, ground + uu.1 * sb);
            let loc = |l: f64, hgt: f64| (base.0 + uu.0 * l + nn.0 * hgt, base.1 + uu.1 * l + nn.1 * hgt);
            let pts = [loc(-sz / 2.0, 0.0), loc(sz / 2.0, 0.0), loc(sz / 2.0, sz), loc(-sz / 2.0, sz)];
            g.poly(&pts, FILL, INK, SW);
            let com = loc(0.0, sz / 2.0);
            g.label_at(com.0, com.1, 22.0, p.s("l_lab"), ITALIC);
            let sa = loc(sz / 2.0, sz / 2.0);
            g.line(sa.0, sa.1, q.0, q.1, INK, 2.0);
            pulley_wheel(&mut g, c.0, c.1, pr);
            let rx = c.0 + pr;
            let rt = c.1 + 80.0 + dr;
            g.line(rx, c.1, rx, rt, INK, 2.0);
            let rot = nn.1.atan2(nn.0).to_degrees();
            let _ = rot;
            mass_box(&mut g, rx, rt, rs, p.s("r_lab"));
            hang.push((rx, rt, rs));
            if p.b("tension") {
                let tip = (com.0 + uu.0 * 60.0, com.1 + uu.1 * 60.0);
                g.arrow(com.0, com.1, tip.0, tip.1, tcol, SW);
                g.label_dir(tip.0, tip.1, uu.0, uu.1, 20.0, "T", ITALIC);
            }
            if p.b("gravity") {
                g.arrow(com.0, com.1, com.0, com.1 + 56.0, gcol, SW);
                g.label_dir(com.0, com.1 + 56.0, 0.0, 1.0, 20.0, "F_g", ITALIC);
            }
            if accel != "None" {
                let d = if accel == "Right falls" { (-uu.0, -uu.1) } else { (uu.0, uu.1) };
                let c0 = (com.0 + nn.0 * (sz / 2.0 + 28.0), com.1 + nn.1 * (sz / 2.0 + 28.0));
                let (a, b) = ((c0.0 - d.0 * 26.0, c0.1 - d.1 * 26.0), (c0.0 + d.0 * 26.0, c0.1 + d.1 * 26.0));
                g.arrow(a.0, a.1, b.0, b.1, acol, SW);
                g.label_dir(b.0, b.1, d.0, d.1, 20.0, "a", ITALIC);
            }
        }
        "Block & tackle" => {
            g.hatch(110.0, 380.0, 30.0, false);
            // fixed pulley and a movable pulley carrying the load
            let (fx, fy) = (200.0, 100.0);
            g.line(fx, 30.0, fx, fy - 30.0, INK, SW);
            pulley_wheel(&mut g, fx, fy, 30.0);
            let (mx, my) = (fx + 60.0, 220.0 + dl);
            g.line(mx + 30.0, 30.0, mx + 30.0, my, INK, 2.0); // rope tied to the ceiling
            pulley_wheel(&mut g, mx, my, 30.0);
            // rope: ceiling -> right side of movable -> underneath -> left side -> fixed pulley top -> down
            g.polyline(&arc_pts(mx, my, 30.0, 0.0, -180.0), INK, 2.0);
            g.line(mx - 30.0, my, fx + 30.0, fy, INK, 2.0);
            g.polyline(&arc_pts(fx, fy, 30.0, 0.0, 180.0), INK, 2.0);
            g.line(fx - 30.0, fy, fx - 30.0, 330.0, INK, 2.0);
            g.arrow(fx - 30.0, 300.0, fx - 30.0, 372.0, tcol, SW);
            g.tex(fx - 42.0, 350.0, 22.0, "end", "F", ITALIC);
            // load
            g.line(mx, my + 3.0, mx, my + 70.0, INK, 2.0);
            mass_box(&mut g, mx, my + 70.0, ls, p.s("l_lab"));
            hang.push((mx, my + 70.0, ls));
            if p.b("tension") {
                g.arrow(mx - 30.0, my - 12.0, mx - 30.0, my - 58.0, tcol, 2.0);
                g.arrow(mx + 30.0, my - 12.0, mx + 30.0, my - 58.0, tcol, 2.0);
                g.tex(mx - 6.0, my - 36.0, 18.0, "end", "T", ITALIC);
                g.tex(mx + 36.0, my - 36.0, 18.0, "start", "T", ITALIC);
            }
            if accel != "None" {
                let down = accel == "Right falls";
                let (a, b) = if down { (my + 92.0, my + 140.0) } else { (my + 140.0, my + 92.0) };
                g.arrow(mx + ls / 2.0 + 24.0, a, mx + ls / 2.0 + 24.0, b, acol, SW);
                g.tex(mx + ls / 2.0 + 34.0, (a + b) / 2.0 + 6.0, 20.0, "start", "a", ITALIC);
            }
        }
        _ => {
            // Atwood machine
            g.hatch(130.0, 350.0, 30.0, false);
            let (cx, cy) = (240.0, 105.0);
            g.line(cx, 30.0, cx, cy - r, INK, SW);
            pulley_wheel(&mut g, cx, cy, r);
            let (lx, rx) = (cx - r, cx + r);
            let (lt, rt) = (235.0 + dl, 235.0 + dr);
            g.line(lx, cy, lx, lt, INK, 2.0);
            g.line(rx, cy, rx, rt, INK, 2.0);
            mass_box(&mut g, lx, lt, ls, p.s("l_lab"));
            mass_box(&mut g, rx, rt, rs, p.s("r_lab"));
            hang.push((lx, lt, ls));
            hang.push((rx, rt, rs));
            if p.b("tension") {
                for (x, t) in [(lx, lt), (rx, rt)] {
                    g.arrow(x, t, x, t - 50.0, tcol, SW);
                    g.tex(x + 8.0, t - 22.0, 18.0, "start", "T", ITALIC);
                }
            }
            if accel != "None" {
                let right_down = accel == "Right falls";
                for (side, (x, t, s)) in [(-1.0, (lx, lt, ls)), (1.0, (rx, rt, rs))] {
                    let down = (side > 0.0) == right_down;
                    let (a, b) = if down { (t + 8.0, t + 56.0) } else { (t + 56.0, t + 8.0) };
                    let ax = x + side * (s / 2.0 + 22.0);
                    g.arrow(ax, a, ax, b, acol, SW);
                    g.tex(ax + side * 8.0, (a + b) / 2.0 + 6.0, 20.0, if side > 0.0 { "start" } else { "end" }, "a", ITALIC);
                }
            }
        }
    }
    // third object hangs below the right-hand (or load) mass
    if p.b("third") {
        if let Some(&(x, t, s)) = hang.last() {
            let ss = 46.0;
            g.line(x, t + s, x, t + s + 34.0, INK, 2.0);
            mass_box(&mut g, x, t + s + 34.0, ss, p.s("t_lab"));
        }
    }
    // weights of hanging masses (offset beside the string so they stay readable)
    if p.b("gravity") && setup != "Table" && setup != "Ramp" {
        let third = p.b("third");
        let n_h = hang.len();
        for (i, &(x, t, s)) in hang.iter().enumerate() {
            if third && i + 1 == n_h {
                // the third object carries the weight arrow instead
                let (ty, ts) = (t + s + 34.0, 46.0);
                g.arrow(x, ty + ts, x, ty + ts + 42.0, gcol, SW);
                g.tex(x + 8.0, ty + ts + 40.0, 18.0, "start", "F_g", ITALIC);
                continue;
            }
            g.arrow(x, t + s, x, t + s + 42.0, gcol, SW);
            g.tex(x + 8.0, t + s + 40.0, 18.0, "start", "F_g", ITALIC);
        }
    }
    g.finish()
}

// ---------------------------------------------------------------- projectile motion

fn proj_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("from", "Launch from", &["Level ground", "A cliff"], "Level ground"),
        ParamDef::float("angle", "Launch angle (deg)", 5.0, 85.0, 1.0, 45.0),
        ParamDef::float("cliff", "Cliff height (relative)", 0.1, 1.2, 0.05, 0.4),
        ParamDef::bool("show_angle", "Mark the angle", true),
        ParamDef::text("ang_lab", "Angle label", "theta"),
        ParamDef::choice("object", "Object", &["Ball", "Dot"], "Ball"),
        ParamDef::int("size", "Object size (%)", 30, 150, 60),
        ParamDef::choice("path", "Path", &["Dashed", "Solid", "None"], "Dashed"),
        ParamDef::int("steps", "Ball at equal time steps (0 = off)", 0, 12, 0),
        ParamDef::bool("show_v", "Launch velocity", true),
        ParamDef::text("v_lab", "Velocity label", "v_0"),
        ParamDef::bool("comps", "Its components", false),
        ParamDef::bool("show_g", "Acceleration due to gravity", false),
        ParamDef::bool("mark_h", "Mark the maximum height", false),
        ParamDef::bool("mark_r", "Mark the range", false),
        ParamDef::bool("mirror", "Mirror (flip left to right)", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

fn proj_render(p: &Params) -> String {
    let (w, h) = (560.0, 340.0);
    let mut g = G::new(w, h, p.b("mirror"));
    let col = p.b("color");
    let cliff = p.s("from") == "A cliff";
    let th = p.f("angle").clamp(5.0, 85.0);
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
    let pos = |t: f64| (lx + s * vx * t, ly - s * (vy * t - 0.5 * t * t));
    // ground and cliff
    g.hatch(10.0, w - 10.0, ground, true);
    if cliff {
        g.poly(&[(10.0, ly + r), (lx + r, ly + r), (lx + r, ground), (10.0, ground)], "#f3f4f7", INK, SW);
        g.hatch(10.0, w - 10.0, ground, true);
    }
    // path
    let pts: Vec<Pt> = (0..=80).map(|i| pos(t_land * i as f64 / 80.0)).collect();
    let pc = if col { PAL[1] } else { INK };
    match p.s("path") {
        "Dashed" => g.dash_polyline(&pts, pc, 2.0),
        "Solid" => g.polyline(&pts, pc, 2.0),
        _ => {}
    }
    let end = *pts.last().unwrap();
    if p.s("path") == "None" { let _ = end; }
    // equal time steps
    let steps = p.i("steps");
    if steps > 0 {
        let rr = (r * 0.55).max(5.0);
        for i in 1..=steps {
            let q = pos(t_land * i as f64 / steps as f64);
            g.circle(q.0, q.1, rr, FILL, INK, 1.8);
        }
    }
    // launch object
    if p.s("object") == "Dot" { g.circle(lx, ly, 4.5, INK, INK, 1.0); } else { g.circle(lx, ly, r, FILL, INK, SW); }
    // velocity, components, angle
    let vlen = 80.0;
    let tip = pol(lx, ly, r + vlen, th);
    let vc = if col { PAL[0] } else { INK };
    if p.b("comps") {
        let cc = if col { PAL[2] } else { GREY };
        let (ex, ey) = (lx + (r + vlen) * vx, ly);
        g.dash_arrow(lx + r * 0.0, ly, ex, ey, cc, 1.8);
        g.dash_arrow(ex, ey, tip.0, tip.1, cc, 1.8);
        g.tex((lx + ex) / 2.0 + 6.0, ly + 24.0, 18.0, "middle", "v_0x", ITALIC);
        g.tex(ex + 8.0, (ey + tip.1) / 2.0 + 6.0, 18.0, "start", "v_0y", ITALIC);
    }
    if p.b("show_v") {
        let a = pol(lx, ly, r, th);
        g.arrow(a.0, a.1, tip.0, tip.1, vc, SW);
        g.label_dir(tip.0, tip.1, -vy * 0.2 - 0.3, -vy.max(0.4), 21.0, p.s("v_lab"), ITALIC);
    }
    if p.b("show_angle") {
        let ar = r + 34.0 + if p.b("comps") { 0.0 } else { 0.0 };
        g.polyline(&arc_pts(lx, ly, ar, 0.0, th), INK, 1.5);
        let m = pol(lx, ly, ar + 16.0, th / 2.0);
        g.label_at(m.0 + 6.0, m.1, 19.0, p.s("ang_lab"), ITALIC);
    }
    if p.b("show_g") {
        let gx = w - 36.0;
        g.arrow(gx, 70.0, gx, 130.0, if col { PAL[3] } else { INK }, SW);
        g.tex(gx - 10.0, 108.0, 20.0, "end", "g", ITALIC);
    }
    if p.b("mark_h") {
        let apex = pos(vy);
        g.dash(lx, apex.1, apex.0 + 40.0, apex.1, GREY, 1.0);
        let x = apex.0 + 28.0;
        g.dim_arrow(x, apex.1, x, ly, INK, 1.4);
        g.dash(apex.0, apex.1, apex.0, ly, GREY, 1.0);
        g.tex(x + 8.0, (apex.1 + ly) / 2.0 + 6.0, 20.0, "start", "H", ITALIC);
    }
    if p.b("mark_r") {
        let (a, b) = (lx, end.0);
        let y = ground + 24.0;
        g.line(a, ground + 2.0, a, y + 6.0, INK, 1.0);
        g.line(b, ground + 2.0, b, y + 6.0, INK, 1.0);
        g.dim_arrow(a, y, b, y, INK, 1.4);
        g.tex((a + b) / 2.0, y - 6.0, 20.0, "middle", "R", ITALIC);
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
