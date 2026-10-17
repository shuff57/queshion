//! Particle diagrams, Bohr models and orbital (box) diagrams.
use super::data::*;
use super::*;
use crate::figures::{ParamDef, Params, Subject};
use std::f64::consts::PI;

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "particle-diagram", subject: Subject::Chemistry, title: "Particle Diagram",
            blurb: "Atoms, ions or molecules drawn as circles in a box or lattice, as a gas, liquid or solid; optional before-and-after pair.",
            params: particle_params, render: particle_render },
        FigureDef { id: "bohr-model", subject: Subject::Chemistry, title: "Bohr Model",
            blurb: "Nucleus (balls, counts or symbol) and electron shells for any element or ion, with a key, shell labels and gained/lost electrons.",
            params: bohr_params, render: bohr_render },
        FigureDef { id: "orbital-diagram", subject: Subject::Chemistry, title: "Orbital Diagram",
            blurb: "Orbital boxes with spin arrows following the aufbau order, Hund's rule and the Pauli principle.",
            params: orbital_params, render: orbital_render },
    ]
}

// ---------------------------------------------------------------------------------------------
// Bohr model
// ---------------------------------------------------------------------------------------------

const COLORS9: [&str; 9] = ["Red", "Orange", "Yellow", "Green", "Blue", "Purple", "Gray", "White", "Black"];

pub(super) fn color9(name: &str) -> &'static str {
    match name {
        "Red" => "#e2483c", "Orange" => "#f08a2c", "Yellow" => "#f3d23b", "Green" => "#3ba150", "Blue" => "#3a74d6",
        "Purple" => "#8b55c8", "Gray" => "#a6a6a6", "White" => "#ffffff", _ => "#222222",
    }
}
/// Text colour that reads on `fill`.
fn on(fill: &str) -> &'static str { if matches!(fill, "#ffffff" | "#f3d23b" | "#a6a6a6") { INK } else { "#ffffff" } }

fn bohr_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "6 C - Carbon"),
        ParamDef::int("charge", "Charge", -8, 8, 0),
        ParamDef::choice("nucleus", "Draw the nucleus as", &["Symbol", "Protons and neutrons", "Blank", "Balls"], "Symbol"),
        ParamDef::choice("placement", "Electron placement", &["Evenly spaced", "Paired"], "Evenly spaced"),
        ParamDef::bool("empty", "Empty rings (students draw the electrons)", false),
        ParamDef::bool("caption", "Caption with element name", false),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::bool("gained_lost", "Gained and lost electrons (gained in their own colour, lost as empty spots)", false),
        ParamDef::int("neutrons", "Neutrons (-1 = from the mass number)", -1, 200, -1),
        ParamDef::list("shells_custom", "Electrons per shell, n = 1, 2, ... (blank = automatic)", ""),
        ParamDef::int("shuffle", "Shuffle the nucleus balls (seed)", 0, 999, 0),
        ParamDef::choice("proton_color", "Proton color", &COLORS9, "Red"),
        ParamDef::choice("proton_symbol", "Proton symbol", &["+", "p", "p⁺", "None"], "+"),
        ParamDef::choice("neutron_color", "Neutron color", &COLORS9, "Gray"),
        ParamDef::choice("neutron_symbol", "Neutron symbol", &["0", "n", "n⁰", "None"], "None"),
        ParamDef::choice("electron_color", "Electron color", &COLORS9, "Blue"),
        ParamDef::choice("electron_symbol", "Electron symbol", &["−", "e", "e⁻", "None"], "None"),
        ParamDef::bool("key", "Key: list each part drawn as a ball or dot", false),
        ParamDef::bool("shell_labels", "Shell labels (n = 1, n = 2 ... on each ring)", false),
        ParamDef::bool("brackets", "Brackets: put an ion in square brackets with its charge", false),
        title_style(false),
    ]
}

/// Geometry shared by the renderer and its handles.
pub(super) struct Bohr { pub z: usize, pub shown: Vec<u32>, pub neutral: Vec<u32>, pub r0: f64, pub radii: Vec<f64>, pub cx: f64, pub cy: f64, pub w: f64, pub h: f64, pub neutrons: usize }

pub(super) fn bohr_geom(p: &Params) -> Bohr {
    let z = z_of(p.s("element"));
    let charge = p.i("charge");
    let custom: Vec<u32> = p.s("shells_custom").split(|c| c == ',' || c == ';' || c == ' ').filter_map(|t| t.trim().parse::<u32>().ok()).take(8).collect();
    let mut shown = if custom.iter().any(|&c| c > 0) { custom } else { shells(z, charge) };
    while shown.len() > 1 && shown.last() == Some(&0) { shown.pop(); }
    let neutral = shells(z, 0);
    let neutrons = if p.i("neutrons") >= 0 { p.i("neutrons") as usize } else { mass_number(z).saturating_sub(z) };
    let nb = z + neutrons;
    let r0 = if p.s("nucleus") == "Balls" { (36.0f64).max(10.0 * (nb as f64 / 0.85).sqrt() * 0.9).min(70.0) } else { 36.0 };
    let nrings = if p.b("gained_lost") { shown.len().max(neutral.len()) } else { shown.len() }.max(1);
    let gap = if nrings > 5 { 30.0 } else if nrings > 4 { 34.0 } else { 40.0 };
    let radii: Vec<f64> = (0..nrings).map(|i| r0 + 26.0 + gap * i as f64).collect();
    let rmax = *radii.last().unwrap();
    let side = 2.0 * (rmax + 24.0);
    let bw = if p.b("brackets") && charge != 0 { 40.0 } else { 0.0 };
    let key_w = if p.b("key") { 220.0 } else { 0.0 };
    let w = side + 2.0 * bw + key_w;
    let h = side + if p.b("caption") { 36.0 } else { 0.0 };
    Bohr { z, shown, neutral, r0, radii, cx: bw + side / 2.0, cy: side / 2.0, w, h, neutrons }
}

fn ball(d: &mut Doc, x: f64, y: f64, r: f64, fill: &str, sym: &str) {
    d.circle(x, y, r, fill, INK, 1.6);
    if sym != "None" && !sym.is_empty() {
        let fs = if sym.chars().count() > 1 { r * 1.05 } else { r * 1.35 };
        tx(d, x, y + fs * 0.36, fs, "middle", on(fill), true, sym);
    }
}

/// Electron positions around a ring of radius `r` for `n` electrons.
fn electron_angles(n: usize, paired: bool, r: f64) -> Vec<f64> {
    let mut v = Vec::new();
    if n == 0 { return v; }
    if paired {
        let groups = n.div_ceil(2);
        for g in 0..groups {
            let base = -PI / 2.0 + 2.0 * PI * g as f64 / groups as f64;
            if g * 2 + 1 < n { let da = 9.0 / r; v.push(base - da); v.push(base + da); } else { v.push(base); }
        }
    } else {
        for k in 0..n { v.push(-PI / 2.0 + 2.0 * PI * k as f64 / n as f64); }
    }
    v
}

fn bohr_render(p: &Params) -> String {
    let g = bohr_geom(p);
    let z = g.z;
    let charge = p.i("charge");
    let (cx, cy) = (g.cx, g.cy);
    let mut d = Doc::new(g.w, g.h);
    let ecol = color9(p.s("electron_color"));
    let esym = p.s("electron_symbol");
    let er = if esym == "None" { 6.5 } else { 7.5 };
    let paired = p.s("placement") == "Paired";
    let gl = p.b("gained_lost");
    let mut any_lost = false;
    let mut any_gained = false;
    for (i, &r) in g.radii.iter().enumerate() {
        d.circle(cx, cy, r, "none", INK, 2.0);
        if p.b("shell_labels") {
            let a = 0.70 * PI;
            let (lx, ly) = (cx + r * a.cos() + 6.0, cy + r * a.sin() + 4.0);
            d.raw(&format!(r##"<text x="{lx:.2}" y="{ly:.2}" font-size="12" font-family="{FONT}" fill="{INK}" stroke="#fff" stroke-width="4" paint-order="stroke">n = {}</text>"##, i + 1));
        }
        if p.b("empty") { continue; }
        let now = *g.shown.get(i).unwrap_or(&0) as usize;
        let was = *g.neutral.get(i).unwrap_or(&0) as usize;
        let slots = if gl { now.max(was) } else { now };
        for (k, a) in electron_angles(slots, paired, r).into_iter().enumerate() {
            let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
            if gl && k >= now {
                any_lost = true;
                d.raw(&format!(r##"<circle cx="{x:.2}" cy="{y:.2}" r="{er}" fill="#fff" stroke="{INK}" stroke-width="1.3" stroke-dasharray="2.5 2"/>"##));
            } else if gl && k >= was {
                any_gained = true;
                ball(&mut d, x, y, er, color9("Green"), esym);
            } else {
                ball(&mut d, x, y, er, ecol, esym);
            }
        }
    }
    let r0 = g.r0;
    let pcol = color9(p.s("proton_color"));
    let ncol = color9(p.s("neutron_color"));
    match p.s("nucleus") {
        "Blank" => { d.circle(cx, cy, r0, "#ececec", INK, 2.0); }
        "Balls" => {
            let n = z + g.neutrons;
            let br = (r0 * (0.85 / n.max(1) as f64).sqrt()).min(10.0);
            // hex grid points nearest the centre
            let mut pts: Vec<(f64, f64)> = Vec::new();
            let m = (n as f64).sqrt() as i64 + 3;
            for row in -m..=m {
                for col in -m..=m {
                    let x = (col as f64 + if row % 2 != 0 { 0.5 } else { 0.0 }) * 2.0 * br * 0.98;
                    let y = row as f64 * 2.0 * br * 0.98 * 0.866;
                    pts.push((x, y));
                }
            }
            let mut rg = Rng(7 + p.i("shuffle") as u64 * 7919 + z as u64);
            let mut keyed: Vec<(f64, (f64, f64))> = pts.iter().map(|&q| (q.0.hypot(q.1) + rg.next() * br * 0.3, q)).collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut pts: Vec<(f64, f64)> = keyed.into_iter().map(|k| k.1).take(n).collect();
            pts.truncate(n);
            let mut kinds: Vec<bool> = (0..n).map(|k| k < z).collect();
            for i in (1..kinds.len()).rev() { let j = (rg.next() * (i as f64 + 1.0)) as usize % (i + 1); kinds.swap(i, j); }
            // draw back to front (top rows first) so the cluster reads as a pile
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| pts[a].1.total_cmp(&pts[b].1));
            for k in order {
                let (x, y) = pts[k];
                if kinds[k] { ball(&mut d, cx + x, cy + y, br, pcol, p.s("proton_symbol")); }
                else { ball(&mut d, cx + x, cy + y, br, ncol, p.s("neutron_symbol")); }
            }
        }
        "Protons and neutrons" => {
            d.circle(cx, cy, r0, "#ececec", INK, 2.0);
            tx(&mut d, cx, cy - 3.0, 15.0, "middle", INK, true, &format!("{z} p⁺"));
            tx(&mut d, cx, cy + 16.0, 15.0, "middle", INK, true, &format!("{} n⁰", g.neutrons));
        }
        _ => {
            d.circle(cx, cy, r0, "#ececec", INK, 2.0);
            tx(&mut d, cx, cy + 8.0, 24.0, "middle", INK, true, symbol(z));
        }
    }
    let side = g.cy * 2.0;
    if p.b("brackets") && charge != 0 {
        let (bx0, bx1, by0, by1) = (cx - side / 2.0 - 22.0, cx + side / 2.0 + 22.0, 10.0, side - 10.0);
        d.polyline(&[(bx0 + 10.0, by0), (bx0, by0), (bx0, by1), (bx0 + 10.0, by1)], INK, 2.0);
        d.polyline(&[(bx1 - 10.0, by0), (bx1, by0), (bx1, by1), (bx1 - 10.0, by1)], INK, 2.0);
        let c = charge.abs();
        let t = format!("{}{}", if c > 1 { c.to_string() } else { String::new() }, if charge > 0 { "+" } else { "−" });
        tx(&mut d, bx1 + 4.0, by0 + 8.0, 17.0, "start", INK, true, &t);
    }
    if p.b("key") {
        let kx = g.w - 210.0;
        let mut rows: Vec<(&str, &str, &str, bool)> = Vec::new();
        if p.s("nucleus") == "Balls" {
            rows.push(("Proton", pcol, p.s("proton_symbol"), true));
            rows.push(("Neutron", ncol, p.s("neutron_symbol"), true));
        }
        if !p.b("empty") {
            rows.push(("Electron", ecol, esym, false));
            if gl && any_lost { rows.push(("Lost electron", "lost", "None", false)); }
            if gl && any_gained { rows.push(("Gained electron", color9("Green"), esym, false)); }
        }
        if !rows.is_empty() {
            let kh = 50.0 + 34.0 * rows.len() as f64;
            let ky = cy - kh / 2.0;
            d.rect(kx, ky, 190.0, kh, "#fff", INK, 1.4);
            tx(&mut d, kx + 14.0, ky + 28.0, 17.0, "start", INK, true, "Key");
            for (k, (name, col, sym, big)) in rows.iter().enumerate() {
                let y = ky + 58.0 + 34.0 * k as f64;
                let r = if *big { 9.0 } else { er };
                if *col == "lost" {
                    d.raw(&format!(r##"<circle cx="{:.2}" cy="{y:.2}" r="{er}" fill="#fff" stroke="{INK}" stroke-width="1.3" stroke-dasharray="2.5 2"/>"##, kx + 26.0));
                } else {
                    ball(&mut d, kx + 26.0, y, r, col, sym);
                }
                tx(&mut d, kx + 48.0, y + 6.0, 16.0, "start", INK, false, name);
            }
        }
    }
    if p.b("caption") {
        let ion = match charge { 0 => String::new(), c if c > 0 => format!(" {}{}", if c > 1 { c.to_string() } else { String::new() }, "+"), c => format!(" {}{}", if c < -1 { (-c).to_string() } else { String::new() }, "-") };
        tx(&mut d, cx, g.h - 12.0, 15.0, "middle", INK, true, &format!("{} ({}{})", name(z), symbol(z), ion));
    }
    wrap(p, d.finish(), &[])
}

// ---------------------------------------------------------------------------------------------
// Orbital diagram
// ---------------------------------------------------------------------------------------------

fn orbital_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "6 C - Carbon"),
        ParamDef::int("charge", "Charge", -4, 8, 0),
        ParamDef::choice("layout", "Arrangement (row = in a row, ladder = by energy)", &["Energy row", "Energy ladder"], "Energy row"),
        ParamDef::bool("shorthand", "Noble gas core (write [Ar] and draw only the orbitals after it)", false),
        ParamDef::bool("blank", "Empty orbitals (students fill in the arrows)", false),
        ParamDef::bool("config", "Configuration line", true),
        ParamDef::bool("caption", "Caption with element name", false),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::bool("config_blank", "Configuration line as a blank line to write on", false),
        ParamDef::choice("order", "Order", &["Filling order", "By shell"], "Filling order"),
        ParamDef::choice("exceptions", "Exceptions (Cr, Cu ...)", &["Real", "Filling order"], "Real"),
        ParamDef::choice("arrows", "Arrows", &["Full arrows", "Half arrows"], "Full arrows"),
        ParamDef::choice("orbitals", "Orbitals drawn as", &["Squares", "Lines"], "Squares"),
        ParamDef::choice("symbol_style", "Symbol", &["Symbol", "Blank line", "None"], "Symbol"),
        ParamDef::choice("sublabels", "Sublevel labels", &["Shown", "Blank lines", "None"], "Shown"),
        ParamDef::int("extra_empty", "Empty sublevels after the last", 0, 4, 0),
        ParamDef::text("edits", "Orbital changes, e.g. 2p2=pair 2p3=empty (or click the orbitals)", ""),
        title_style(false),
        answer_param("Answer key (electron configuration)"),
    ]
}

fn spin_arrows(d: &mut Doc, x: f64, y: f64, bs: f64, state: u8, half: bool) {
    let a = |d: &mut Doc, xx: f64, upw: bool| {
        let (y1, y2) = if upw { (y + bs - 6.0, y + 6.0) } else { (y + 6.0, y + bs - 6.0) };
        if half {
            d.line(xx, y1, xx, y2, INK, 2.0);
            let dir = if upw { 1.0 } else { -1.0 };
            // harpoon: one barb, on the left for up and the right for down
            d.line(xx, y2, xx - 7.0 * dir, y2 + 9.0 * dir, INK, 2.0);
        } else {
            d.line(xx, y1, xx, y2, INK, 2.0);
            let dir = if upw { 1.0 } else { -1.0 };
            d.polyline(&[(xx - 6.0, y2 + 8.0 * dir), (xx, y2), (xx + 6.0, y2 + 8.0 * dir)], INK, 2.0);
        }
    };
    match state {
        3 => { a(d, x + bs * 0.34, true); a(d, x + bs * 0.66, false); }
        1 => a(d, x + bs / 2.0, true),
        2 => a(d, x + bs / 2.0, false),
        _ => {}
    }
}

/// Everything the orbital renderer and its handles need.
pub(super) struct OrbLay {
    pub subs: Vec<Sub>, pub core: Option<&'static str>, pub states: Vec<Vec<u8>>, pub w: f64, pub h: f64,
    /// (sub index, orbital index, x, y) of each box.
    pub boxes: Vec<(usize, usize, f64, f64)>, pub labels: Vec<(f64, f64)>, pub sym_xy: (f64, f64), pub core_xy: (f64, f64), pub cfg_y: f64, pub bs: f64, pub ladder: bool,
}

fn orb_symbol(z: usize, charge: i64) -> String {
    let c = match charge { 0 => String::new(), c if c > 0 => format!("{}⁺", if c > 1 { sup(c as u32) } else { String::new() }), c => format!("{}⁻", if c < -1 { sup((-c) as u32) } else { String::new() }) };
    format!("{}{c}", symbol(z))
}

pub(super) fn orb_layout(p: &Params) -> OrbLay {
    let z = z_of(p.s("element"));
    let charge = p.i("charge");
    let real = p.s("exceptions") != "Filling order";
    let mut subs = config_ex(z, charge, real);
    if charge > 0 {
        // keep the sublevels the ion emptied, drawn empty
        for s0 in config_ex(z, 0, real) {
            if !subs.iter().any(|s| s.n == s0.n && s.l == s0.l) { subs.push(Sub { n: s0.n, l: s0.l, e: 0 }); }
        }
        let pos = |s: &Sub| AUFBAU.iter().position(|&a| a == (s.n, s.l)).unwrap_or(0);
        subs.sort_by_key(pos);
    }
    let mut core: Option<&'static str> = None;
    if p.b("shorthand") {
        if let Some((cz, cs)) = noble_core(z) {
            let hide = config(cz, 0);
            let before = subs.len();
            subs.retain(|s| !hide.iter().any(|h| h.n == s.n && h.l == s.l));
            if subs.len() < before { core = Some(cs); }
        }
    }
    let extra = p.i("extra_empty").clamp(0, 4) as usize;
    if extra > 0 {
        let last = subs.iter().map(|s| AUFBAU.iter().position(|&a| a == (s.n, s.l)).unwrap_or(0)).max().unwrap_or(0);
        for &(n, l) in AUFBAU.iter().skip(last + 1).take(extra) { subs.push(Sub { n, l, e: 0 }); }
    }
    if p.s("order") == "By shell" { subs.sort_by_key(|s| (s.n, s.l)); }
    // spin states with the edits applied
    let edits: Vec<(String, u8)> = p.s("edits").split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter_map(|t| {
        let (k, v) = t.split_once('=')?;
        let st = match v.trim() { "pair" | "updown" => 3, "up" => 1, "down" => 2, "empty" | "none" => 0, _ => return None };
        Some((k.trim().to_string(), st))
    }).collect();
    let states: Vec<Vec<u8>> = subs.iter().map(|s| {
        let m = (2 * s.l + 1) as usize;
        let e = s.e as usize;
        (0..m).map(|k| {
            let key = format!("{}{}", s.label(), k + 1);
            if let Some((_, st)) = edits.iter().rev().find(|x| x.0 == key) { return *st; }
            let up = k < e.min(m);
            let down = e > m && k < e - m;
            match (up, down) { (true, true) => 3, (true, false) => 1, _ => 0 }
        }).collect()
    }).collect();
    let bs = 40.0;
    let ladder = p.s("layout") == "Energy ladder";
    let sym_w = match p.s("symbol_style") { "None" => 0.0, "Blank line" => 70.0, _ => orb_symbol(z, charge).chars().count() as f64 * 15.0 + 30.0 };
    let core_w = if core.is_some() { 70.0 } else { 0.0 };
    let cap = if p.b("caption") { 30.0 } else { 0.0 };
    let cfg_h = if p.b("config") { 46.0 } else { 0.0 };
    let mut boxes = Vec::new();
    let mut labels = Vec::new();
    if !ladder {
        let gap = 26.0;
        let x_start = 24.0 + sym_w + core_w;
        let maxw = 760.0;
        let rowh = bs + 50.0;
        let (mut x, mut row) = (x_start, 0usize);
        let mut right: f64 = 0.0;
        let top = 24.0 + cap;
        for (i, s) in subs.iter().enumerate() {
            let gw = (2 * s.l + 1) as f64 * bs;
            if x > x_start && x + gw > maxw { x = x_start; row += 1; }
            let y = top + row as f64 * rowh;
            for k in 0..(2 * s.l + 1) as usize { boxes.push((i, k, x + k as f64 * bs, y)); }
            labels.push((x + gw / 2.0, y + bs + 24.0));
            right = right.max(x + gw);
            x += gw + gap;
        }
        let w = (right + 24.0).max(260.0);
        let h = top + (row + 1) as f64 * rowh + cfg_h + 6.0;
        let cfg_y = top + (row + 1) as f64 * rowh + 22.0;
        OrbLay { subs, core, states, w, h, boxes, labels, sym_xy: (24.0, top + bs / 2.0 + 10.0), core_xy: (24.0 + sym_w, top + bs / 2.0 + 7.0), cfg_y, bs, ladder }
    } else {
        let rowh = 62.0;
        let n = subs.len().max(1);
        let x0 = 130.0 + core_w;
        let w = x0 + 7.0 * bs + 40.0;
        let top = 30.0 + cap + if sym_w > 0.0 { 40.0 } else { 0.0 };
        let base_y = top + rowh * n as f64 + 10.0;
        for (i, s) in subs.iter().enumerate() {
            let y = base_y - rowh * (i as f64 + 1.0) + 8.0;
            for k in 0..(2 * s.l + 1) as usize { boxes.push((i, k, x0 + k as f64 * bs, y)); }
            labels.push((x0 - 14.0, y + bs / 2.0 + 5.0));
        }
        let h = base_y + 20.0 + cfg_h;
        OrbLay { subs, core, states, w, h, boxes, labels, sym_xy: (40.0, top - 20.0), core_xy: (60.0, base_y - 30.0), cfg_y: base_y + 36.0, bs, ladder }
    }
}

fn orbital_render(p: &Params) -> String {
    let z = z_of(p.s("element"));
    let charge = p.i("charge");
    let lay = orb_layout(p);
    let bs = lay.bs;
    let mut d = Doc::new(lay.w, lay.h);
    if p.b("caption") { tx(&mut d, lay.w / 2.0, 22.0, 16.0, "middle", INK, true, &format!("{} ({})", name(z), symbol(z))); }
    match p.s("symbol_style") {
        "None" => {}
        "Blank line" => { d.line(lay.sym_xy.0, lay.sym_xy.1, lay.sym_xy.0 + 50.0, lay.sym_xy.1, INK, 1.5); }
        _ => { tx(&mut d, lay.sym_xy.0, lay.sym_xy.1, 30.0, "start", INK, true, &orb_symbol(z, charge)); }
    }
    if let Some(c) = lay.core { tx(&mut d, lay.core_xy.0, lay.core_xy.1, 22.0, "start", INK, false, &format!("[{c}]")); }
    if lay.ladder {
        let (top, bot) = (lay.boxes.iter().map(|b| b.3).fold(f64::MAX, f64::min) - 14.0, lay.boxes.iter().map(|b| b.3).fold(0.0, f64::max) + bs + 10.0);
        d.arrow(40.0 + if lay.core.is_some() { 0.0 } else { 0.0 }, bot, 40.0, top, INK, 1.8);
        tr(&mut d, 26.0, (top + bot) / 2.0, 14.0, -90.0, "middle", "Energy");
    }
    let squares = p.s("orbitals") != "Lines";
    let half = p.s("arrows") == "Half arrows";
    for &(i, k, x, y) in &lay.boxes {
        if squares { d.rect(x, y, bs, bs, "#ffffff", INK, 2.0); } else { d.line(x + 4.0, y + bs, x + bs - 4.0, y + bs, INK, 2.2); }
        if !p.b("blank") { spin_arrows(&mut d, x, y, bs, lay.states[i][k], half); }
    }
    for (i, s) in lay.subs.iter().enumerate() {
        let (lx, ly) = lay.labels[i];
        let anchor = if lay.ladder { "end" } else { "middle" };
        match p.s("sublabels") {
            "None" => {}
            "Blank lines" => { let x0 = if lay.ladder { lx - 40.0 } else { lx - 20.0 }; d.line(x0, ly + 2.0, x0 + 40.0, ly + 2.0, INK, 1.3); }
            _ => { tx(&mut d, lx, ly, 18.0, anchor, INK, false, &s.label()); }
        }
    }
    let shown: Vec<Sub> = lay.subs.iter().filter(|s| s.e > 0).cloned().collect();
    let text = config_text(&shown, lay.core);
    if p.b("config") {
        if p.b("config_blank") { d.line(lay.w / 2.0 - 140.0, lay.cfg_y + 4.0, lay.w / 2.0 + 140.0, lay.cfg_y + 4.0, INK, 1.4); }
        else { tx(&mut d, if lay.ladder { lay.w / 2.0 } else { lay.boxes.first().map_or(24.0, |b| b.2) }, lay.cfg_y, 19.0, if lay.ladder { "middle" } else { "start" }, INK, false, &text); }
    }
    let mut foot = Vec::new();
    if p.b("answer") { foot.push(format!("{}: {}", orb_symbol(z, charge), text)); }
    wrap(p, d.finish(), &foot)
}

// ---------------------------------------------------------------------------------------------
// Particle diagram
// ---------------------------------------------------------------------------------------------

const SHAPES: [&str; 6] = ["Alone", "Pair", "Bent", "In a line", "Three around", "Four around"];
const SIZES: [&str; 5] = ["XS", "S", "M", "L", "XL"];
const SHADES: [&str; 5] = ["White", "Light gray", "Gray", "Dark gray", "Black"];
const CHARGES: [&str; 7] = ["None", "+", "−", "2+", "2−", "3+", "3−"];
const KINDS: [&str; 4] = ["a", "b", "c", "d"];

fn particle_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("layout", "Layout", &["In a box", "Lattice"], "In a box"),
        ParamDef::choice("boxes", "Boxes", &["One box", "Before and after"], "One box"),
        ParamDef::choice("state", "State (box, or before)", &["Gas", "Liquid", "Solid"], "Gas"),
        ParamDef::choice("state2", "State after", &["Gas", "Liquid", "Solid"], "Gas"),
    ];
    v.push(ParamDef::text("title", "Chart title text", ""));
    v.push(ParamDef::text("lab_before", "Left box label", "Before"));
    v.push(ParamDef::text("lab_after", "Right box label", "After"));
    let spec: [(&'static str, i64, &'static str, &'static str, &'static str, [&'static str; 5], [&'static str; 5]); 3] = [
        ("a", 4, "Alone", "L", "Light gray",
            ["a_count", "a_shape", "a_size", "a_shade", "a2_count"],
            ["Particle 1: how many", "Particle 1: shape", "Particle 1: size", "Particle 1: shade", "Particle 1: how many after"]),
        ("b", 0, "Alone", "M", "Dark gray",
            ["b_count", "b_shape", "b_size", "b_shade", "b2_count"],
            ["Particle 2: how many", "Particle 2: shape", "Particle 2: size", "Particle 2: shade", "Particle 2: how many after"]),
        ("c", 0, "Alone", "M", "White",
            ["c_count", "c_shape", "c_size", "c_shade", "c2_count"],
            ["Particle 3: how many", "Particle 3: shape", "Particle 3: size", "Particle 3: shade", "Particle 3: how many after"]),
    ];
    for (k, cnt, shape, size, shade_, key, lab) in spec {
        v.push(ParamDef::int(key[0], lab[0], 0, 24, cnt));
        v.push(ParamDef::choice(key[1], lab[1], &SHAPES, shape));
        v.push(ParamDef::choice(key[2], lab[2], &SIZES, size));
        v.push(ParamDef::choice(key[3], lab[3], &SHADES, shade_));
        v.push(ParamDef::int(key[4], lab[4], 0, 24, if k == "a" { 4 } else { 0 }));
    }
    v.extend([
        ParamDef::int("d_count", "Particle 4: how many", 0, 24, 0),
        ParamDef::choice("d_shape", "Particle 4: shape", &SHAPES, "Alone"),
        ParamDef::choice("d_size", "Particle 4: size", &SIZES, "S"),
        ParamDef::choice("d_shade", "Particle 4: shade", &SHADES, "Black"),
        ParamDef::int("d2_count", "Particle 4: how many after", 0, 24, 0),
    ]);
    let more: [[(&'static str, &'static str); 5]; 4] = [
        [("a_charge", "Particle 1: charge (centre atom)"), ("a_osize", "Particle 1: outer atom size"), ("a_oshade", "Particle 1: outer atom shade"), ("a_ocharge", "Particle 1: outer atom charge"), ("a_name", "Particle 1: name in the key")],
        [("b_charge", "Particle 2: charge (centre atom)"), ("b_osize", "Particle 2: outer atom size"), ("b_oshade", "Particle 2: outer atom shade"), ("b_ocharge", "Particle 2: outer atom charge"), ("b_name", "Particle 2: name in the key")],
        [("c_charge", "Particle 3: charge (centre atom)"), ("c_osize", "Particle 3: outer atom size"), ("c_oshade", "Particle 3: outer atom shade"), ("c_ocharge", "Particle 3: outer atom charge"), ("c_name", "Particle 3: name in the key")],
        [("d_charge", "Particle 4: charge (centre atom)"), ("d_osize", "Particle 4: outer atom size"), ("d_oshade", "Particle 4: outer atom shade"), ("d_ocharge", "Particle 4: outer atom charge"), ("d_name", "Particle 4: name in the key")],
    ];
    let onames: [(&'static str, &'static str); 4] = [("a_oname", "Particle 1: outer atom name (key, each atom)"), ("b_oname", "Particle 2: outer atom name (key, each atom)"), ("c_oname", "Particle 3: outer atom name (key, each atom)"), ("d_oname", "Particle 4: outer atom name (key, each atom)")];
    for (i, m) in more.iter().enumerate() {
        v.push(ParamDef::choice(m[0].0, m[0].1, &CHARGES, "None"));
        v.push(ParamDef::choice(m[1].0, m[1].1, &SIZES, "S"));
        v.push(ParamDef::choice(m[2].0, m[2].1, &SHADES, "White"));
        v.push(ParamDef::choice(m[3].0, m[3].1, &CHARGES, "None"));
        v.push(ParamDef::text(m[4].0, m[4].1, ""));
        v.push(ParamDef::text(onames[i].0, onames[i].1, ""));
    }
    v.extend([
        ParamDef::int("shuffle", "Shuffle (seed)", 0, 999, 0),
        ParamDef::choice("key_show", "Show", &["Box only", "Box and key", "Key only"], "Box only"),
        ParamDef::choice("key_list", "Key lists", &["Each particle", "Each atom"], "Each particle"),
        ParamDef::text("note", "Key note", ""),
        ParamDef::choice("border", "Box border", &["Single", "Double", "None"], "Single"),
        title_style(false),
    ]);
    v
}

fn atom_radius(sz: &str) -> f64 { match sz { "XS" => 5.0, "S" => 7.5, "M" => 10.0, "L" => 13.0, _ => 17.0 } }

/// One particle kind: centre atom + outer atoms.
#[derive(Clone)]
struct Species { shape: String, r: f64, fill: &'static str, ch: String, ro: f64, fillo: &'static str, cho: String }

fn species(p: &Params, k: &str) -> Species {
    let g = |x: &str| p.s(&format!("{k}_{x}")).to_string();
    Species {
        shape: g("shape"), r: atom_radius(&g("size")), fill: shade(&g("shade")), ch: g("charge"),
        ro: atom_radius(&g("osize")), fillo: shade(&g("oshade")), cho: g("ocharge"),
    }
}

/// Outer-atom offsets for a molecule shape (centre atom at the origin).
fn mol_offsets(sp: &Species) -> Vec<(f64, f64)> {
    let d = (sp.r + sp.ro) * 0.9;
    match sp.shape.as_str() {
        "Pair" => vec![(d, 0.0)],
        "Bent" => { let a = 52.0f64.to_radians(); vec![(-d * a.sin(), d * a.cos()), (d * a.sin(), d * a.cos())] }
        "In a line" => vec![(-d, 0.0), (d, 0.0)],
        "Three around" => (0..3).map(|i| { let a = PI * 2.0 * i as f64 / 3.0 - PI / 2.0; (d * a.cos(), d * a.sin()) }).collect(),
        "Four around" => vec![(d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)],
        _ => vec![],
    }
}

/// Centre of the drawing relative to the centre atom (Pair is centred between its atoms).
fn mol_centre(sp: &Species) -> (f64, f64) {
    let o = mol_offsets(sp);
    if sp.shape == "Pair" || sp.shape == "Bent" { let n = o.len() as f64 + 1.0; (o.iter().map(|q| q.0).sum::<f64>() / n, o.iter().map(|q| q.1).sum::<f64>() / n) } else { (0.0, 0.0) }
}

fn bound(sp: &Species) -> f64 {
    let c = mol_centre(sp);
    mol_offsets(sp).iter().map(|(x, y)| ((x - c.0).powi(2) + (y - c.1).powi(2)).sqrt() + sp.ro).fold(sp.r + c.0.hypot(c.1), f64::max)
}

fn atom(d: &mut Doc, x: f64, y: f64, r: f64, fill: &str, ch: &str) {
    d.circle(x, y, r, fill, INK, 1.6);
    if ch != "None" && !ch.is_empty() {
        let col = if matches!(fill, "#161616" | "#5c5c5c") { "#ffffff" } else { INK };
        let fs = if ch.chars().count() > 1 { r * 0.95 } else { r * 1.35 };
        tx(d, x, y + fs * 0.36, fs.max(7.0), "middle", col, true, ch);
    }
}

fn draw_mol(d: &mut Doc, sp: &Species, x: f64, y: f64, rot: f64) {
    let (c, s) = (rot.cos(), rot.sin());
    let m = mol_centre(sp);
    let place = |ox: f64, oy: f64| { let (ox, oy) = (ox - m.0, oy - m.1); (x + ox * c - oy * s, y + ox * s + oy * c) };
    for (ox, oy) in mol_offsets(sp) {
        let (px, py) = place(ox, oy);
        atom(d, px, py, sp.ro, sp.fillo, &sp.cho);
    }
    let (px, py) = place(0.0, 0.0);
    atom(d, px, py, sp.r, sp.fill, &sp.ch);
}

fn fill_box(d: &mut Doc, bx: f64, by: f64, bs: f64, state: &str, layout_lattice: bool, items: &[Species], seed: u64, border: &str) {
    match border {
        "None" => {}
        "Double" => { d.rect(bx, by, bs, bs, "none", INK, 1.6); d.rect(bx - 5.0, by - 5.0, bs + 10.0, bs + 10.0, "none", INK, 1.6); }
        _ => { d.rect(bx, by, bs, bs, "none", INK, 2.2); }
    }
    if items.is_empty() { return; }
    let mut rg = Rng(seed);
    let m = 8.0;
    if layout_lattice {
        let n = items.len();
        let maxb = items.iter().map(bound).fold(0.0, f64::max);
        let minb = items.iter().map(bound).fold(f64::MAX, f64::min);
        let cols = ((n as f64).sqrt().ceil() as usize).max(1);
        let rows = n.div_ceil(cols);
        // neighbours alternate kinds; diagonal neighbours share a kind
        let tight = if (maxb - minb).abs() < 1e-9 { 2.0 * maxb } else { (maxb + minb).max(1.42 * maxb) };
        let step = (tight * 1.02).min((bs - 2.0 * m) / cols.max(rows) as f64);
        let gx = bx + (bs - step * cols as f64) / 2.0 + step / 2.0;
        let gy = by + (bs - step * rows as f64) / 2.0 + step / 2.0;
        // alternate the kinds over the grid (checkerboard for two kinds)
        let mut cells: Vec<(usize, usize)> = (0..rows).flat_map(|r| (0..cols).map(move |c| (r, c))).take(n).collect();
        cells.sort_by_key(|&(r, c)| ((r + c) % 2, r, c));
        for (k, (r, c)) in cells.iter().enumerate() {
            let sp = &items[k.min(n - 1)];
            let scale = (step / 2.0 / bound(sp)).min(1.0);
            let mut s2 = sp.clone();
            s2.r *= scale;
            s2.ro *= scale;
            draw_mol(d, &s2, gx + *c as f64 * step, gy + *r as f64 * step, 0.0);
        }
        return;
    }
    let mut placed: Vec<(f64, f64, f64)> = Vec::new();
    let (ymin, ymax) = match state { "Gas" => (by + m, by + bs - m), _ => (by + bs * 0.28, by + bs - m) };
    if state == "Solid" {
        let maxb = items.iter().map(bound).fold(0.0, f64::max);
        let step = maxb * 2.0 * 1.02;
        let per_row = (((bs - 2.0 * m) / step).floor() as usize).max(1);
        let mut idx: Vec<usize> = (0..items.len()).collect();
        for i in (1..idx.len()).rev() { let j = (rg.next() * (i as f64 + 1.0)) as usize % (i + 1); idx.swap(i, j); }
        for (k, &ii) in idx.iter().enumerate() {
            let (r, c) = (k / per_row, k % per_row);
            let in_row = per_row.min(items.len() - r * per_row);
            let x = bx + bs / 2.0 + (c as f64 - (in_row as f64 - 1.0) / 2.0) * step;
            let y = by + bs - m - step / 2.0 - r as f64 * step;
            draw_mol(d, &items[ii], x, y, 0.0);
        }
        return;
    }
    let pad_try: [f64; 3] = if state == "Gas" { [bs * 0.06, bs * 0.03, 1.0] } else { [2.5, 1.0, 0.0] };
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| bound(&items[b]).total_cmp(&bound(&items[a])));
    for &ii in &order {
        let sp = &items[ii];
        let rb = bound(sp);
        'outer: for &pad in &pad_try {
            for _ in 0..1500 {
                let x = rg.range(bx + m + rb, bx + bs - m - rb);
                let y = rg.range(ymin + rb, ymax - rb);
                if placed.iter().all(|&(px, py, pr)| ((px - x).powi(2) + (py - y).powi(2)).sqrt() >= pr + rb + pad) {
                    placed.push((x, y, rb));
                    draw_mol(d, sp, x, y, rg.range(0.0, PI * 2.0));
                    break 'outer;
                }
            }
        }
    }
}

/// Key rows: (drawable, name) for each particle kind or each distinct atom.
fn key_rows(p: &Params) -> Vec<(Species, String)> {
    let mut rows: Vec<(Species, String)> = Vec::new();
    for (i, k) in KINDS.iter().enumerate() {
        let used = p.i(&format!("{k}_count")) > 0 || (p.s("boxes") == "Before and after" && p.i(&format!("{k}2_count")) > 0);
        if !used { continue; }
        let sp = species(p, k);
        let nm = p.s(&format!("{k}_name")).trim().to_string();
        if p.s("key_list") == "Each atom" {
            let mut c = sp.clone();
            c.shape = "Alone".into();
            if !rows.iter().any(|r| r.0.r == c.r && r.0.fill == c.fill && r.0.ch == c.ch) { rows.push((c, nm)); }
            if sp.shape != "Alone" {
                let o = Species { shape: "Alone".into(), r: sp.ro, fill: sp.fillo, ch: sp.cho.clone(), ro: sp.ro, fillo: sp.fillo, cho: sp.cho.clone() };
                let on = p.s(&format!("{k}_oname")).trim().to_string();
                if !rows.iter().any(|r| r.0.r == o.r && r.0.fill == o.fill && r.0.ch == o.ch) { rows.push((o, on)); }
            }
        } else {
            rows.push((sp, if nm.is_empty() { String::new() } else { nm }));
        }
        let _ = i;
    }
    rows
}

fn draw_key(d: &mut Doc, p: &Params, x: f64, ycen: f64) {
    let rows = key_rows(p);
    if rows.is_empty() { return; }
    let rowh: Vec<f64> = rows.iter().map(|r| (bound(&r.0) * 2.0 + 10.0).max(34.0)).collect();
    let note = p.s("note").trim();
    let kh = 52.0 + rowh.iter().sum::<f64>() + if note.is_empty() { 0.0 } else { 30.0 };
    let ky = ycen - kh / 2.0;
    d.rect(x, ky, 200.0, kh, "#fff", INK, 1.6);
    tx(d, x + 14.0, ky + 30.0, 18.0, "start", INK, true, "Key");
    let mut y = ky + 44.0;
    for (k, (sp, nm)) in rows.iter().enumerate() {
        let cy = y + rowh[k] / 2.0;
        let bw = bound(sp);
        draw_mol(d, sp, x + 16.0 + bw, cy, 0.0);
        let tx0 = x + 30.0 + 2.0 * bw;
        if nm.is_empty() { d.line(tx0, cy + 6.0, x + 188.0, cy + 6.0, INK, 1.2); } else { tx(d, tx0, cy + 6.0, 16.0, "start", INK, false, nm); }
        y += rowh[k];
    }
    if !note.is_empty() { tx(d, x + 14.0, y + 20.0, 13.0, "start", INK, false, note); }
}

fn particle_render(p: &Params) -> String {
    let two = p.s("boxes") == "Before and after";
    let lattice = p.s("layout") == "Lattice";
    let show = p.s("key_show");
    let build = |after: bool| -> Vec<Species> {
        let mut v = Vec::new();
        for k in KINDS {
            let n = p.i(&format!("{k}{}_count", if after { "2" } else { "" })).clamp(0, 24);
            let sp = species(p, k);
            for _ in 0..n { v.push(sp.clone()); }
        }
        v
    };
    let bs = 320.0;
    let boxes_w = if show == "Key only" { 0.0 } else if two { bs * 2.0 + 150.0 } else { bs + 40.0 };
    let key_w = if show == "Box only" { 0.0 } else { 224.0 };
    let w = boxes_w + key_w + if show == "Key only" { 20.0 } else { 0.0 };
    let h = bs + if two { 70.0 } else { 40.0 };
    let mut d = Doc::new(w, h);
    let seed = p.i("shuffle").max(0) as u64 * 7919;
    if show != "Key only" {
        let before = build(false);
        fill_box(&mut d, 20.0, 20.0, bs, p.s("state"), lattice, &before, 11 + seed, p.s("border"));
        if two {
            let after = build(true);
            d.arrow(bs + 38.0, 20.0 + bs / 2.0, bs + 112.0, 20.0 + bs / 2.0, INK, 2.2);
            fill_box(&mut d, bs + 130.0, 20.0, bs, p.s("state2"), lattice, &after, 23 + seed, p.s("border"));
            tx(&mut d, 20.0 + bs / 2.0, bs + 52.0, 15.0, "middle", INK, true, p.s("lab_before"));
            tx(&mut d, bs + 130.0 + bs / 2.0, bs + 52.0, 15.0, "middle", INK, true, p.s("lab_after"));
        }
    }
    if show != "Box only" {
        draw_key(&mut d, p, boxes_w + 12.0, 20.0 + bs / 2.0);
    }
    wrap(p, d.finish(), &[])
}

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

use crate::figures::interact::Handle;

/// Move the element choice by `delta` (wraps within the first 36 elements).
fn step_element(q: &mut Params, delta: i64, max_z: i64) {
    let z = z_of(q.s("element")) as i64;
    let nz = (z - 1 + delta).rem_euclid(max_z) + 1;
    q.set_s("element", ELEMENT_CHOICES[(nz - 1) as usize]);
}

pub(super) fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    match id {
        "bohr-model" => {
            let g = bohr_geom(p);
            let (cx, cy) = (g.cx, g.cy);
            let rmax = *g.radii.last().unwrap();
            Some(vec![
                Handle::zone((cx, cy), (g.r0 * 1.8, g.r0 * 1.8), "Click: next element. Scroll: change element", |q| step_element(q, 1, 118))
                    .with_scroll(|q, dy| step_element(q, if dy > 0.0 { 1 } else { -1 }, 118)),
                Handle::zone((cx, cy - rmax), (60.0, 22.0), "Click: add an electron (charge). Scroll up adds, down removes", |q| {
                    let c = q.i("charge");
                    q.set_i("charge", if c <= -8 { 8 } else { c - 1 });
                })
                .with_scroll(|q, dy| q.set_i("charge", (q.i("charge") - if dy > 0.0 { 1 } else { -1 }).clamp(-8, 8))),
            ])
        }
        "orbital-diagram" => {
            let lay = orb_layout(p);
            let mut v = Vec::new();
            for &(i, k, x, y) in &lay.boxes {
                let key = format!("{}{}", lay.subs[i].label(), k + 1);
                let next = match lay.states[i][k] { 0 => "up", 1 => "down", 2 => "pair", _ => "empty" };
                v.push(Handle::zone((x + lay.bs / 2.0, y + lay.bs / 2.0), (lay.bs, lay.bs), "Click: change this orbital (empty, up, down, pair). Scroll: change the charge", move |q| {
                    let mut toks: Vec<String> = q.s("edits").split_whitespace().filter(|t| !t.starts_with(&format!("{key}="))).map(String::from).collect();
                    toks.push(format!("{key}={next}"));
                    q.set_s("edits", &toks.join(" "));
                }).with_scroll(|q, dy| q.set_i("charge", (q.i("charge") - if dy > 0.0 { 1 } else { -1 }).clamp(-4, 8))));
            }
            v.push(Handle::zone((lay.sym_xy.0 + 20.0, lay.sym_xy.1 - 10.0), (50.0, 36.0), "Click: next element. Scroll: change element", |q| step_element(q, 1, 118))
                .with_scroll(|q, dy| step_element(q, if dy > 0.0 { 1 } else { -1 }, 118)));
            Some(v)
        }
        "particle-diagram" => {
            if p.s("key_show") == "Key only" { return Some(Vec::new()); }
            let two = p.s("boxes") == "Before and after";
            let bs = 320.0;
            let mut v = Vec::new();
            let boxes: Vec<(f64, &'static str, &'static str)> = if two {
                vec![(20.0 + bs / 2.0, "a_count", "Before"), (bs + 130.0 + bs / 2.0, "a2_count", "After")]
            } else {
                vec![(20.0 + bs / 2.0, "a_count", "")]
            };
            for (cx, _first, _) in &boxes {
                let after = *cx > bs;
                let keys: [&'static str; 4] = if after { ["a2_count", "b2_count", "c2_count", "d2_count"] } else { ["a_count", "b_count", "c_count", "d_count"] };
                v.push(
                    Handle::zone((*cx, 20.0 + bs / 2.0), (bs, bs), "Click: add a particle. Scroll up adds, down removes", move |q| {
                        q.set_i(keys[0], q.i(keys[0]) + 1);
                    })
                    .with_scroll(move |q, dy| {
                        if dy > 0.0 {
                            q.set_i(keys[0], q.i(keys[0]) + 1);
                        } else if let Some(k) = keys.iter().rev().find(|k| q.i(k) > 0) {
                            q.set_i(k, q.i(k) - 1);
                        }
                    }),
                );
            }
            if two {
                v.push(Handle::label((20.0 + bs / 2.0, bs + 52.0), (110.0, 24.0), "lab_before", "Double-click to rename"));
                v.push(Handle::label((bs + 130.0 + bs / 2.0, bs + 52.0), (110.0, 24.0), "lab_after", "Double-click to rename"));
            }
            Some(v)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------------------------

#[allow(unused_imports)]
use crate::figures::{Example, Value};
#[allow(dead_code)]
fn ex(name: &'static str, params: Vec<(&'static str, Value)>) -> Example { Example { name, params } }

fn sv(v: &str) -> Value { Value::S(v.into()) }
fn bv(v: bool) -> Value { Value::B(v) }
fn iv(v: i64) -> Value { Value::I(v) }

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    Some(match id {
        "bohr-model" => vec![
            ex("Bohr model of sodium", vec![("element", sv("11 Na - Sodium")), ("nucleus", sv("Balls"))]),
            ex("Bohr model of carbon", vec![("element", sv("6 C - Carbon")), ("nucleus", sv("Balls"))]),
            ex("Bohr model of oxygen", vec![("element", sv("8 O - Oxygen")), ("nucleus", sv("Balls"))]),
            ex("Bohr model of calcium", vec![("element", sv("20 Ca - Calcium")), ("nucleus", sv("Protons and neutrons"))]),
            ex("Bohr model of the sodium ion Na+", vec![("element", sv("11 Na - Sodium")), ("charge", iv(1)), ("nucleus", sv("Balls")), ("gained_lost", bv(true)), ("key", bv(true)), ("brackets", bv(true))]),
            ex("Bohr model of the chloride ion Cl-", vec![("element", sv("17 Cl - Chlorine")), ("charge", iv(-1)), ("nucleus", sv("Balls")), ("gained_lost", bv(true)), ("key", bv(true)), ("brackets", bv(true))]),
            ex("Blank Bohr model of aluminum", vec![("element", sv("13 Al - Aluminum")), ("nucleus", sv("Protons and neutrons")), ("empty", bv(true)), ("shell_labels", bv(true))]),
        ],
        "orbital-diagram" => vec![
            ex("Orbital diagram of oxygen", vec![("element", sv("8 O - Oxygen")), ("config", bv(false))]),
            ex("Orbital diagram of iron", vec![("element", sv("26 Fe - Iron")), ("config", bv(false))]),
            ex("Orbital diagram of chromium (exception)", vec![("element", sv("24 Cr - Chromium")), ("shorthand", bv(true)), ("answer", bv(true))]),
            ex("Orbital diagram of the Fe3+ ion", vec![("element", sv("26 Fe - Iron")), ("charge", iv(3)), ("shorthand", bv(true)), ("config", bv(false))]),
            ex("Orbital diagram and configuration of sulfur", vec![("element", sv("16 S - Sulfur"))]),
            ex("Hund's rule mistake: nitrogen", vec![("element", sv("7 N - Nitrogen")), ("edits", sv("2p1=pair 2p3=empty")), ("config", bv(false))]),
        ],
        "particle-diagram" => vec![
            ex("Mixture of two elements", vec![("a_count", iv(6)), ("a_size", sv("M")), ("a_shade", sv("Light gray")), ("b_count", iv(5)), ("b_shape", sv("Pair")), ("b_size", sv("M")), ("b_shade", sv("Dark gray")), ("b_osize", sv("M")), ("b_oshade", sv("Dark gray"))]),
            ex("Pure compound: water molecules", vec![("a_count", iv(10)), ("a_shape", sv("Bent")), ("a_size", sv("L")), ("a_shade", sv("Gray")), ("a_osize", sv("S")), ("a_oshade", sv("White"))]),
            ex("Pure element: diatomic molecules", vec![("a_count", iv(8)), ("a_shape", sv("Pair")), ("a_size", sv("M")), ("a_shade", sv("Gray")), ("a_osize", sv("M")), ("a_oshade", sv("Gray"))]),
            ex("Before and after: hydrogen and oxygen react", vec![("boxes", sv("Before and after")), ("a_count", iv(4)), ("a_shape", sv("Pair")), ("a_size", sv("S")), ("a_shade", sv("White")), ("a_osize", sv("S")), ("a_oshade", sv("White")), ("a2_count", iv(0)), ("b_count", iv(2)), ("b_shape", sv("Pair")), ("b_size", sv("M")), ("b_shade", sv("Dark gray")), ("b_osize", sv("M")), ("b_oshade", sv("Dark gray")), ("c2_count", iv(4)), ("c_shape", sv("Bent")), ("c_size", sv("M")), ("c_shade", sv("Dark gray")), ("c_osize", sv("S")), ("c_oshade", sv("White"))]),
            ex("Key of each atom: water and carbon dioxide", vec![("a_count", iv(5)), ("a_shape", sv("Bent")), ("a_size", sv("M")), ("a_shade", sv("Gray")), ("a_osize", sv("S")), ("a_oshade", sv("White")), ("a_name", sv("O atom")), ("a_oname", sv("H atom")), ("b_count", iv(3)), ("b_shape", sv("In a line")), ("b_size", sv("M")), ("b_shade", sv("Black")), ("b_osize", sv("M")), ("b_oshade", sv("Gray")), ("b_name", sv("C atom")), ("key_show", sv("Box and key")), ("key_list", sv("Each atom"))]),
            ex("Ionic solid lattice: NaCl", vec![("layout", sv("Lattice")), ("state", sv("Solid")), ("a_count", iv(12)), ("a_size", sv("XL")), ("a_shade", sv("Light gray")), ("a_charge", sv("−")), ("a_name", sv("Cl⁻ ion")), ("b_count", iv(12)), ("b_size", sv("M")), ("b_shade", sv("Dark gray")), ("b_charge", sv("+")), ("b_name", sv("Na⁺ ion")), ("key_show", sv("Box and key")), ("border", sv("None"))]),
        ],
        _ => return None,
    })
}
