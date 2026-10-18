//! Pulley figure: Atwood machine (with an optional third mass), block on a table or a ramp tied
//! over a pulley to a hanging mass, and a block and tackle with 2-5 supporting strands.
use super::common::*;
use crate::figures::{ParamDef, Params};

const LM: [&str; 3] = ["Text", "Blank line", "None"];

/// Which controls the chosen setup uses (as the reference shows them): Atwood has the two
/// hanging masses, "hangs lower" and the third mass; table and ramp the cart/block choice, a
/// second object, the surface, normal force and friction (the ramp also its angle); the block
/// and tackle its strands and the load. Everything else stays put but out of sight.
pub(crate) fn pulley_relevant(key: &str, p: &Params) -> bool {
    let setup = p.s("setup");
    let (atwood, tackle) = (setup == "Atwood", setup == "Block & tackle");
    let flat = !atwood && !tackle; // table or ramp
    match key {
        "l_kind" | "second" | "surface" | "normal" | "norm_lab_mode" | "norm_lab" | "friction" | "fric_lab_mode" | "fric_lab" => flat,
        "s_lab" => flat && p.b("second"),
        "angle" | "ang_lab_mode" | "ang_lab" => setup == "Ramp",
        "lower" | "third" => atwood,
        "third_side" | "t_lab_mode" | "t_lab" | "t_size" => atwood && p.b("third"),
        "r_lab_mode" | "r_lab" | "r_size" => !tackle,
        "strands" => tackle,
        _ => true,
    }
}

/// The panel changed the setup from `old`: a block and tackle's load reads "m" (the reference's
/// label) while the other setups' left object reads m_1, so a label still at the other setup's
/// default follows the switch. A typed label is kept, and saved figures are not touched.
pub(crate) fn pulley_setup_changed(old: &str, p: &mut Params) {
    const TACKLE: &str = "Block & tackle";
    let new = p.s("setup").to_string();
    match (old == TACKLE, new == TACKLE, p.s("l_lab")) {
        (false, true, "m_1") => p.set_s("l_lab", "m"),
        (true, false, "m") => p.set_s("l_lab", "m_1"),
        _ => {}
    }
}

pub(super) fn pulley_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("setup", "Setup", &["Atwood", "Table", "Ramp", "Block & tackle"], "Atwood"),
        ParamDef::choice("l_kind", "Object on the table / ramp", &["Block", "Cart"], "Block"),
        ParamDef::choice("l_lab_mode", "Left (table / ramp / load) label shows", &LM, "Text"),
        ParamDef::text("l_lab", "Left object label", "m_1"),
        ParamDef::int("l_size", "Left object size (%)", 50, 200, 100),
        ParamDef::choice("r_lab_mode", "Right (hanging) label shows", &LM, "Text"),
        ParamDef::text("r_lab", "Right object label", "m_2"),
        ParamDef::int("r_size", "Right object size (%)", 50, 200, 100),
        ParamDef::choice("lower", "Hangs lower", &["Neither", "Left", "Right"], "Neither"),
        ParamDef::bool("third", "Hang a third object below", false),
        ParamDef::choice("third_side", "The third hangs below", &["Left", "Right"], "Right"),
        ParamDef::choice("t_lab_mode", "Third object label shows", &LM, "Text"),
        ParamDef::text("t_lab", "Third object label", "m_3"),
        ParamDef::int("t_size", "Third object size (%)", 50, 200, 77),
        ParamDef::bool("second", "Add an object on the table / ramp, tied behind", false),
        ParamDef::text("s_lab", "Its label", "m_3"),
        ParamDef::int("strands", "Block & tackle: strands holding up the load", 2, 5, 2),
        ParamDef::float("angle", "Ramp angle (deg)", 15.0, 60.0, 1.0, 30.0),
        ParamDef::choice("ang_lab_mode", "Angle label shows", &LM, "Text"),
        ParamDef::text("ang_lab", "Angle label", "theta"),
        ParamDef::choice("surface", "Surface (table / ramp)", &["Smooth", "Rough"], "Smooth"),
        ParamDef::bool("tension", "Tension", false),
        ParamDef::choice("tens_lab_mode", "Tension label shows", &LM, "Text"),
        ParamDef::text("tens_lab", "Tension label", "T"),
        ParamDef::bool("gravity", "Gravity", false),
        ParamDef::choice("grav_lab_mode", "Gravity labels show", &LM, "Text"),
        ParamDef::text("grav_lab", "Gravity label (blank = the object's label + g)", ""),
        ParamDef::bool("normal", "Normal force (table / ramp)", false),
        ParamDef::choice("norm_lab_mode", "Normal force label shows", &LM, "Text"),
        ParamDef::text("norm_lab", "Normal force label", "F_N"),
        ParamDef::choice("friction", "Friction (table / ramp)", &["None", "Toward the pulley", "Away from it"], "None"),
        ParamDef::choice("fric_lab_mode", "Friction label shows", &LM, "Text"),
        ParamDef::text("fric_lab", "Friction label", "F_f"),
        ParamDef::choice("accel", "Acceleration (right / hanging / load falls)", &["None", "Right falls", "Left falls"], "None"),
        ParamDef::choice("acc_lab_mode", "Acceleration label shows", &LM, "Text"),
        ParamDef::text("acc_lab", "Acceleration label", "a"),
        ParamDef::bool("mirror", "Mirror (flip left to right)", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

/// A box the handles can grab: centre x, top y, size, its rest top y, which side "Hangs lower"
/// names it (empty = none), its size key and label key.
pub(super) struct PBox { pub cx: f64, pub top: f64, pub s: f64, pub rest: f64, pub side: &'static str, pub size_key: &'static str, pub lab_key: &'static str }

pub(super) struct PGeo { pub w: f64, pub h: f64, pub boxes: Vec<PBox>, pub ramp: Option<(f64, f64)> }

fn sz(p: &Params, k: &str) -> f64 { 60.0 * p.f(k).clamp(50.0, 200.0) / 100.0 }

/// Ramp / table frame: foot (ax, ground), up-slope unit, outward normal, apex, ramp length.
struct Slope { ax: f64, ground: f64, uu: Pt, nn: Pt, apex: Pt }

fn slope(p: &Params) -> Slope {
    if p.s("setup") == "Ramp" {
        let th = p.f("angle").clamp(15.0, 60.0).to_radians();
        let (ax, ground) = (40.0, 420.0);
        let bw = (250.0 / th.tan()).min(330.0);
        Slope { ax, ground, uu: (th.cos(), -th.sin()), nn: (-th.sin(), -th.cos()), apex: (ax + bw, ground - bw * th.tan()) }
    } else {
        // the table top acts as a flat "ramp"
        Slope { ax: 50.0, ground: 230.0, uu: (1.0, 0.0), nn: (0.0, -1.0), apex: (340.0, 230.0) }
    }
}

/// Pulley wheel position for the table / ramp: tangent to a string parallel to the surface at the
/// first object's mid height.
/// On the table the wheel is big and sits at the table's edge, its centre level with the table
/// top's middle (as on the reference), within 22..36 for very small / big objects.
fn slope_wheel(sl: &Slope, hh: f64) -> (Pt, f64) {
    let flat = sl.uu.1.abs() < 1e-9;
    let pr = if flat { (hh + 6.0).clamp(22.0, 36.0) } else { 22.0 };
    let q = if flat { (sl.apex.0 + 10.0 + pr, sl.apex.1 - hh) } else {
        let t = (-sl.uu.1 / sl.uu.0).abs(); // tan(theta)
        (sl.apex.0 + sl.nn.0 * hh + sl.uu.0 * hh * t, sl.apex.1 + sl.nn.1 * hh + sl.uu.1 * hh * t)
    };
    ((q.0 - sl.nn.0 * pr, q.1 - sl.nn.1 * pr), pr)
}

pub(super) fn pulley_geo(p: &Params) -> PGeo {
    let lower = p.s("lower");
    let (dl, dr) = (if lower == "Left" { 45.0 } else { 0.0 }, if lower == "Right" { 45.0 } else { 0.0 });
    let (ls, rs) = (sz(p, "l_size"), sz(p, "r_size"));
    let mut boxes = vec![];
    match p.s("setup") {
        "Table" | "Ramp" => {
            let ramp = p.s("setup") == "Ramp";
            let sl = slope(p);
            let hh = mid_h(p, ls);
            let (c, pr) = slope_wheel(&sl, hh);
            let mut rest = c.1 + pr + 82.0;
            if ramp {
                // keep the hanging mass and its weight label above the ground
                rest = rest.min(sl.ground - dr - rs - if p.b("gravity") { 84.0 } else { 20.0 }).max(c.1 + 50.0);
            }
            boxes.push(PBox { cx: c.0 + pr, top: rest + dr, s: rs, rest, side: "Right", size_key: "r_size", lab_key: "r_lab" });
            // grow the canvas when a big or lowered hanging mass would run off the bottom
            let need = rest + dr + rs + if p.b("gravity") { 116.0 } else { 50.0 };
            let (w, h) = (560.0, need.max(500.0));
            return PGeo { w, h, boxes, ramp: if ramp { Some((sl.ax, sl.ground)) } else { None } };
        }
        "Block & tackle" => {
            let n = p.i("strands").clamp(2, 5) as f64;
            let r = 22.0;
            let x0 = 240.0 - (n - 1.0) * r;
            let bar = 220.0 + r + 14.0;
            let lx = x0 + (n - 1.0) * r + if n as i64 % 2 == 0 { r } else { 0.0 };
            let _ = dl;
            boxes.push(PBox { cx: lx, top: bar + 26.0, s: ls, rest: bar + 26.0, side: "", size_key: "l_size", lab_key: "l_lab" });
        }
        _ => {
            boxes.push(PBox { cx: 240.0 - 42.0, top: 235.0 + dl, s: ls, rest: 235.0, side: "Left", size_key: "l_size", lab_key: "l_lab" });
            boxes.push(PBox { cx: 240.0 + 42.0, top: 235.0 + dr, s: rs, rest: 235.0, side: "Right", size_key: "r_size", lab_key: "r_lab" });
        }
    }
    let mut h: f64 = 430.0;
    if p.b("third") && p.s("setup") == "Atwood" {
        let b = boxes.iter().map(|b| b.top + b.s).fold(0.0, f64::max);
        h = h.max(b + 96.0 + sz(p, "t_size") + 80.0);
    }
    PGeo { w: 480.0, h, boxes, ramp: None }
}

/// Height above the surface of the string's attachment (the object's middle).
fn mid_h(p: &Params, ls: f64) -> f64 { if p.s("l_kind") == "Cart" { ls * 0.51 } else { ls / 2.0 } }

fn wheel(g: &mut G, cx: f64, cy: f64, r: f64) {
    g.circle(cx, cy, r, "#f3f4f7", INK, SW);
    g.circle(cx, cy, r * 0.34, "none", INK, 1.6);
    g.circle(cx, cy, 2.4, INK, INK, 1.0);
}

fn lab_at(g: &mut G, x: f64, y: f64, t: &str, anchor: &str) {
    if t.eq_ignore_ascii_case("none") { return; }
    if t.trim().is_empty() {
        let (a, b) = match anchor { "start" => (x, x + 24.0), "end" => (x - 24.0, x), _ => (x - 12.0, x + 12.0) };
        g.line(a, y + 2.0, b, y + 2.0, INK, 1.4);
    } else {
        g.tex(x, y, 19.0, anchor, t, ITALIC);
    }
}

/// Weight label for an object labelled `obj` (blank text = "<label> g").
fn weight_lab(p: &Params, obj: &str) -> String {
    match p.s("grav_lab_mode") {
        "Blank line" => return String::new(),
        "None" => return "none".into(),
        _ => {}
    }
    let t = p.s("grav_lab").trim();
    if !t.is_empty() { return t.to_string(); }
    let o = obj.trim();
    if o.is_empty() || o == "none" { "F_g".into() } else { format!("{o} g") }
}

/// Square box with its label; returns nothing (geometry is known to the caller).
fn mass_box(g: &mut G, cx: f64, top: f64, s: f64, lab: &str) {
    g.rect(cx - s / 2.0, top, s, s, FILL, INK, SW);
    if lab.trim().is_empty() { g.line(cx - 11.0, top + s / 2.0 + 6.0, cx + 11.0, top + s / 2.0 + 6.0, INK, 1.4); }
    else if lab != "none" { g.tex(cx, top + s / 2.0 + 7.0, 19.0, "middle", lab, ITALIC); }
}

struct Style { col: bool, tens: String, acc: String }

impl Style {
    fn tc(&self) -> &'static str { if self.col { PAL[0] } else { INK } }
    fn gc(&self) -> &'static str { if self.col { PAL[1] } else { INK } }
    fn ac(&self) -> &'static str { if self.col { PAL[2] } else { INK } }
}

/// Tension arrow of length 30 from `a` along unit `d`, label on side `lx` (-1 left / 1 right).
fn t_arrow(g: &mut G, st: &Style, a: Pt, d: Pt, lab: &str, side: f64) {
    let b = (a.0 + d.0 * 30.0, a.1 + d.1 * 30.0);
    g.arrow(a.0, a.1, b.0, b.1, st.tc(), 2.2);
    // label on screen-side `side` for vertical arrows; above (side > 0) / below for others
    let (mut nx, mut ny) = (-d.1, d.0);
    if nx.abs() > 0.3 { if nx * side < 0.0 { nx = -nx; ny = -ny; } } else if (ny < 0.0) != (side > 0.0) { nx = -nx; ny = -ny; }
    let m = ((a.0 + b.0) / 2.0 + nx * 14.0, (a.1 + b.1) / 2.0 + ny * 14.0 + 6.0);
    lab_at(g, m.0, m.1, lab, if nx > 0.3 { "start" } else if nx < -0.3 { "end" } else { "middle" });
}

/// Tension arrow of length 30 from `a` along unit `d` on a string that runs along the table /
/// ramp, its label on the side of the outward normal `n` (above the string, clear of the wheel).
fn t_arrow_along(g: &mut G, st: &Style, a: Pt, d: Pt, lab: &str, n: Pt) {
    let b = (a.0 + d.0 * 30.0, a.1 + d.1 * 30.0);
    g.arrow(a.0, a.1, b.0, b.1, st.tc(), 2.2);
    let m = ((a.0 + b.0) / 2.0 + n.0 * 14.0, (a.1 + b.1) / 2.0 + n.1 * 14.0 + 6.0);
    lab_at(g, m.0, m.1, lab, if n.0 > 0.3 { "start" } else if n.0 < -0.3 { "end" } else { "middle" });
}

/// Weight arrow from the bottom of a hanging box.
fn w_arrow(g: &mut G, st: &Style, x: f64, y: f64, lab: &str) {
    g.arrow(x, y, x, y + 44.0, st.gc(), SW);
    lab_at(g, x, y + 66.0, lab, "middle");
}

/// Dashed acceleration arrow beside a hanging box (down when `down`).
fn a_arrow(g: &mut G, st: &Style, x: f64, top: f64, s: f64, down: bool, side: f64) {
    let (a, b) = if down { (top + s * 0.2, top + s * 0.2 + 34.0) } else { (top + s * 0.2 + 34.0, top + s * 0.2) };
    g.dash_arrow(x, a, x, b, st.ac(), 2.2);
    lab_at(g, x + side * 10.0, (a + b) / 2.0 + 6.0, &st.acc, if side > 0.0 { "start" } else { "end" });
}

pub(super) fn pulley_render(p: &Params) -> String {
    let geo = pulley_geo(p);
    let mut g = G::new(geo.w, geo.h, p.b("mirror"));
    let st = Style { col: p.b("color"), tens: lab_mode(p, "tens_lab", "tens_lab_mode"), acc: lab_mode(p, "acc_lab", "acc_lab_mode") };
    let (llab, rlab, tlab) = (lab_mode(p, "l_lab", "l_lab_mode"), lab_mode(p, "r_lab", "r_lab_mode"), lab_mode(p, "t_lab", "t_lab_mode"));
    let accel = p.s("accel").to_string();
    let (ten, grav) = (p.b("tension"), p.b("gravity"));
    match p.s("setup") {
        "Table" | "Ramp" => slope_setup(&mut g, p, &geo, &st, &llab, &rlab),
        "Block & tackle" => {
            let n = p.i("strands").clamp(2, 5) as usize;
            let r = 22.0;
            let x0 = 240.0 - (n as f64 - 1.0) * r;
            let (ty, by) = (100.0, 220.0);
            let bx = &geo.boxes[0];
            let bar = by + r + 14.0;
            let top_bar = 40.0;
            let (cl, cr) = (x0 - r - 40.0, x0 + (n as f64) * 2.0 * r + 30.0);
            g.hatch(cl, cr, top_bar, false);
            // pulleys alternate top (fixed) and bottom (moving); rope zigzags between them
            let pcx = |k: usize| x0 + 2.0 * r * k as f64;
            let pcy = |k: usize| if k % 2 == 0 { ty } else { by };
            // bottom pulleys hang from the load bar, top pulleys from the ceiling
            let mut bxs = vec![];
            for k in 0..n {
                let (x, y) = (pcx(k), pcy(k));
                if k % 2 == 0 { g.line(x, top_bar, x, y, INK, SW); } else { g.line(x, y, x, bar, INK, SW); bxs.push(x); }
            }
            // rope
            let pull_x = pcx(0) - r;
            g.line(pull_x, ty, pull_x, ty + 150.0, INK, 2.0);
            for k in 0..n {
                let (x, y) = (pcx(k), pcy(k));
                if k % 2 == 0 { g.polyline(&arc_pts(x, y, r, 180.0, 0.0), INK, 2.0); } else { g.polyline(&arc_pts(x, y, r, 180.0, 360.0), INK, 2.0); }
                if k + 1 < n { let nx = pcx(k + 1) - r; g.line(x + r, y, nx, pcy(k + 1), INK, 2.0); }
            }
            let lastx = pcx(n - 1) + r;
            let end_y = if (n - 1) % 2 == 0 { bar } else { top_bar };
            g.line(lastx, pcy(n - 1), lastx, end_y, INK, 2.0);
            let (b0, b1) = (bxs.first().copied().unwrap_or(lastx).min(lastx) - 10.0, bxs.last().copied().unwrap_or(lastx).max(lastx) + 10.0);
            g.line(b0, bar, b1, bar, INK, 4.0);
            for k in 0..n { let (x, y) = (pcx(k), pcy(k)); wheel(&mut g, x, y, r); }
            g.line(bx.cx, bar, bx.cx, bx.top, INK, 2.0);
            mass_box(&mut g, bx.cx, bx.top, bx.s, &llab);
            if ten {
                // pull end, and every strand holding the load
                t_arrow(&mut g, &st, (pull_x, ty + 110.0), (0.0, 1.0), &st.tens, -1.0);
                let mut xs: Vec<f64> = (0..n.saturating_sub(1)).map(|k| pcx(k) + r).collect();
                xs.push(lastx);
                let ym = (ty + by) / 2.0 + 16.0;
                for x in xs { t_arrow(&mut g, &st, (x, ym), (0.0, -1.0), &st.tens, 1.0); }
            }
            if grav { w_arrow(&mut g, &st, bx.cx, bx.top + bx.s, &weight_lab(p, &llab)); }
            if accel != "None" {
                a_arrow(&mut g, &st, bx.cx + bx.s / 2.0 + 16.0, bx.top, bx.s, accel == "Right falls", 1.0);
            }
        }
        _ => {
            // Atwood machine
            g.hatch(130.0, 350.0, 30.0, false);
            let (cx, cy, r) = (240.0, 105.0, 42.0);
            g.line(cx, 30.0, cx, cy - r, INK, SW);
            wheel(&mut g, cx, cy, r);
            let third = p.b("third");
            let tside = if p.s("third_side") == "Left" { 0 } else { 1 };
            let tl = |k: usize| if third { format!("{}_1", st.tens.split('_').next().unwrap_or("T")) } else { let _ = k; st.tens.clone() };
            let ts = sz(p, "t_size");
            for (k, b) in geo.boxes.iter().enumerate() {
                let side = if k == 0 { -1.0 } else { 1.0 };
                let x = b.cx;
                g.line(x, cy, x, b.top, INK, 2.0);
                mass_box(&mut g, x, b.top, b.s, if k == 0 { &llab } else { &rlab });
                let lab = if k == 0 { &llab } else { &rlab };
                let below = third && k == tside;
                if ten {
                    let tlab = if st.tens.is_empty() || st.tens == "none" { st.tens.clone() } else { tl(k) };
                    t_arrow(&mut g, &st, (x, cy + 8.0), (0.0, 1.0), &tlab, side);
                    t_arrow(&mut g, &st, (x, b.top), (0.0, -1.0), &tlab, side);
                }
                if grav {
                    if below {
                        // beside the string to the object below, label to the outside
                        let wx = x + side * 12.0;
                        g.arrow(wx, b.top + b.s, wx, b.top + b.s + 44.0, st.gc(), SW);
                        lab_at(&mut g, wx + side * 8.0, b.top + b.s + 40.0, &weight_lab(p, lab), if side > 0.0 { "start" } else { "end" });
                    } else {
                        w_arrow(&mut g, &st, x, b.top + b.s, &weight_lab(p, lab));
                    }
                }
                let down = (side > 0.0) == (accel == "Right falls");
                if accel != "None" { a_arrow(&mut g, &st, x + side * (b.s / 2.0 + 14.0), b.top, b.s, down, side); }
                if below {
                    let t3 = b.top + b.s + if grav || ten { 96.0 } else { 34.0 };
                    g.line(x, b.top + b.s, x, t3, INK, 2.0);
                    mass_box(&mut g, x, t3, ts, &tlab);
                    if ten {
                        let l2 = if st.tens.is_empty() || st.tens == "none" { st.tens.clone() } else { format!("{}_2", st.tens.split('_').next().unwrap_or("T")) };
                        t_arrow(&mut g, &st, (x - 2.0, b.top + b.s), (0.0, 1.0), &l2, -side);
                        t_arrow(&mut g, &st, (x, t3), (0.0, -1.0), &l2, -side);
                    }
                    if grav { w_arrow(&mut g, &st, x, t3 + ts, &weight_lab(p, &tlab)); }
                    if accel != "None" { a_arrow(&mut g, &st, x + side * (ts / 2.0 + 14.0), t3, ts, down, side); }
                }
            }
        }
    }
    g.finish()
}

/// Table and ramp setups share one drawing: an object on a surface tied over a pulley at the
/// surface's far end to a hanging mass.
fn slope_setup(g: &mut G, p: &Params, geo: &PGeo, st: &Style, llab: &str, rlab: &str) {
    let ramp = p.s("setup") == "Ramp";
    let sl = slope(p);
    let (uu, nn) = (sl.uu, sl.nn);
    let ls = sz(p, "l_size");
    let cart = p.s("l_kind") == "Cart";
    let accel = p.s("accel").to_string();
    // the surface
    if ramp {
        g.poly(&[(sl.ax, sl.ground), (sl.apex.0, sl.ground), sl.apex], "#f3f4f7", INK, SW);
        g.hatch(10.0, geo.w - 10.0, sl.ground, true);
        let th = p.f("angle").clamp(15.0, 60.0);
        g.polyline(&arc_pts(sl.ax, sl.ground, 46.0, 0.0, th), INK, 1.4);
        let m = pol(sl.ax, sl.ground, 64.0, th / 2.0);
        g.label_at(m.0, m.1, 19.0, &lab_mode(p, "ang_lab", "ang_lab_mode"), ITALIC);
    } else {
        let ground = geo.h - 24.0;
        g.hatch(10.0, geo.w - 10.0, ground, true);
        g.rect(sl.ax, sl.ground, sl.apex.0 - sl.ax, 12.0, "#f3f4f7", INK, SW);
        for x in [sl.ax + 14.0, sl.apex.0 - 26.0] { g.rect(x, sl.ground + 12.0, 12.0, ground - sl.ground - 12.0, "#f3f4f7", INK, 1.8); }
    }
    let ramp_len = ((sl.apex.0 - sl.ax).powi(2) + (sl.apex.1 - sl.ground).powi(2)).sqrt();
    if p.s("surface") == "Rough" {
        let mut s = 8.0;
        while s < ramp_len - 4.0 {
            let (qx, qy) = (sl.ax + uu.0 * s, sl.ground + uu.1 * s);
            g.line(qx, qy, qx - nn.0 * 7.0 - uu.0 * 5.0, qy - nn.1 * 7.0 - uu.1 * 5.0, INK, 1.2);
            s += 11.0;
        }
    }
    // the object on the surface (block or cart), centred at `s0` along it
    // returns (centre, centre-to-top, centre-to-bottom, half length)
    let obj = |g: &mut G, s0: f64, lab: &str| -> (Pt, f64, f64, f64) {
        let base = (sl.ax + uu.0 * s0, sl.ground + uu.1 * s0);
        let loc = |l: f64, h: f64| (base.0 + uu.0 * l + nn.0 * h, base.1 + uu.1 * l + nn.1 * h);
        let (com, up, dn, hl) = if cart {
            let (len, h, r) = (ls * 1.4, ls * 0.5, ls * 0.13);
            g.poly(&[loc(-len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r + h), loc(-len / 2.0, 2.0 * r + h)], FILL, INK, SW);
            for l in [-len * 0.3, len * 0.3] { let c = loc(l, r); g.circle(c.0, c.1, r, "#fff", INK, SW); }
            (loc(0.0, 2.0 * r + h / 2.0), h / 2.0, 2.0 * r + h / 2.0, len / 2.0)
        } else {
            g.poly(&[loc(-ls / 2.0, 0.0), loc(ls / 2.0, 0.0), loc(ls / 2.0, ls), loc(-ls / 2.0, ls)], FILL, INK, SW);
            (loc(0.0, ls / 2.0), ls / 2.0, ls / 2.0, ls / 2.0)
        };
        if lab.trim().is_empty() { g.line(com.0 - 11.0, com.1 + 6.0, com.0 + 11.0, com.1 + 6.0, INK, 1.4); }
        else if lab != "none" { g.tex(com.0, com.1 + 7.0, 19.0, "middle", lab, ITALIC); }
        (com, up, dn, hl)
    };
    let s_main = ramp_len * if ramp { 0.5 } else { 0.55 };
    let (com, up, dn, hl) = obj(g, s_main, llab);
    let (wc, pr) = slope_wheel(&sl, mid_h(p, ls));
    // string from the object's front face (at its mid height) to the top of the wheel
    let sa = (com.0 + uu.0 * hl, com.1 + uu.1 * hl);
    let tan_pt = (wc.0 + nn.0 * pr, wc.1 + nn.1 * pr);
    g.line(sa.0, sa.1, tan_pt.0, tan_pt.1, INK, 2.0);
    // bracket from the surface corner to the wheel
    g.line(sl.apex.0, sl.apex.1 + if ramp { 0.0 } else { 6.0 }, wc.0, wc.1, INK, SW);
    let hb = &geo.boxes[0];
    g.line(hb.cx, wc.1, hb.cx, hb.top, INK, 2.0);
    wheel(g, wc.0, wc.1, pr);
    mass_box(g, hb.cx, hb.top, hb.s, rlab);
    // second object behind, tied by a string
    if p.b("second") {
        let gap = hl * 2.0 + 34.0;
        let (c2, _, _, hl2) = obj(g, s_main - gap, p.s("s_lab"));
        let a = (c2.0 + uu.0 * hl2, c2.1 + uu.1 * hl2);
        let b = (com.0 - uu.0 * hl, com.1 - uu.1 * hl);
        g.line(a.0, a.1, b.0, b.1, INK, 2.0);
    }
    let falls = accel == "Right falls";
    if p.b("tension") {
        // both labels above the string (on the table this is where they always were; on the ramp
        // the upper one used to land inside the wheel)
        t_arrow_along(g, st, sa, uu, &st.tens, nn);
        t_arrow_along(g, st, (tan_pt.0 - uu.0 * 4.0, tan_pt.1 - uu.1 * 4.0), (-uu.0, -uu.1), &st.tens, nn);
        if hb.top - (wc.1 + pr) > 76.0 { t_arrow(g, st, (hb.cx, wc.1 + pr + 4.0), (0.0, 1.0), &st.tens, 1.0); }
        t_arrow(g, st, (hb.cx, hb.top), (0.0, -1.0), &st.tens, 1.0);
    }
    if p.b("gravity") {
        // weight of the object on the surface from its bottom face, straight down
        let from = (com.0 - nn.0 * dn, com.1 - nn.1 * dn);
        g.arrow(from.0, from.1, from.0, from.1 + 50.0, st.gc(), SW);
        lab_at(g, from.0, from.1 + 72.0, &weight_lab(p, llab), "middle");
        w_arrow(g, st, hb.cx, hb.top + hb.s, &weight_lab(p, rlab));
    }
    if p.b("normal") {
        let from = (com.0 + nn.0 * up, com.1 + nn.1 * up);
        let tip = (from.0 + nn.0 * 46.0, from.1 + nn.1 * 46.0);
        g.arrow(from.0, from.1, tip.0, tip.1, if st.col { PAL[3] } else { INK }, SW);
        g.label_dir(tip.0, tip.1, nn.0, nn.1, 19.0, &lab_mode(p, "norm_lab", "norm_lab_mode"), ITALIC);
    }
    let fr = p.s("friction");
    if fr != "None" {
        let d = if fr == "Toward the pulley" { uu } else { (-uu.0, -uu.1) };
        let sgn = if fr == "Toward the pulley" { 1.0 } else { -1.0 };
        let from = (com.0 + uu.0 * hl * sgn - nn.0 * dn * 0.6, com.1 + uu.1 * hl * sgn - nn.1 * dn * 0.6);
        let tip = (from.0 + d.0 * 40.0, from.1 + d.1 * 40.0);
        g.arrow(from.0, from.1, tip.0, tip.1, if st.col { PAL[4] } else { INK }, SW);
        g.label_dir(tip.0, tip.1, d.0, d.1, 19.0, &lab_mode(p, "fric_lab", "fric_lab_mode"), ITALIC);
    }
    if accel != "None" {
        // on the surface object (above it, along the surface) and beside the hanging mass
        let d = if falls { uu } else { (-uu.0, -uu.1) };
        let c = (com.0 + nn.0 * (up + 30.0) + uu.0 * (hl * 0.6 + 10.0), com.1 + nn.1 * (up + 30.0) + uu.1 * (hl * 0.6 + 10.0));
        let (a, b) = ((c.0 - d.0 * 18.0, c.1 - d.1 * 18.0), (c.0 + d.0 * 18.0, c.1 + d.1 * 18.0));
        g.dash_arrow(a.0, a.1, b.0, b.1, st.ac(), 2.2);
        g.label_dir(c.0 + nn.0 * 4.0, c.1 + nn.1 * 4.0, nn.0, nn.1, 19.0, &st.acc, ITALIC);
        a_arrow(g, st, hb.cx + hb.s / 2.0 + 14.0, hb.top, hb.s, falls, 1.0);
    }
}
