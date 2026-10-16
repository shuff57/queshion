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
            blurb: "Nucleus and electron shells for any of the first 36 elements or their ions.",
            params: bohr_params, render: bohr_render },
        FigureDef { id: "orbital-diagram", subject: Subject::Chemistry, title: "Orbital Diagram",
            blurb: "Orbital boxes with spin arrows following the aufbau order, Hund's rule and the Pauli principle.",
            params: orbital_params, render: orbital_render },
    ]
}

// ---------------------------------------------------------------------------------------------
// Bohr model
// ---------------------------------------------------------------------------------------------

fn bohr_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "6 C - Carbon"),
        ParamDef::int("charge", "Ion charge", -3, 4, 0),
        ParamDef::choice("nucleus", "Nucleus label", &["Symbol", "Protons and neutrons", "Blank"], "Symbol"),
        ParamDef::choice("placement", "Electron placement", &["Evenly spaced", "Paired"], "Evenly spaced"),
        ParamDef::bool("empty", "Empty rings (students draw the electrons)", false),
        ParamDef::bool("caption", "Caption with element name", false),
        ParamDef::text("title", "Title (optional)", ""),
    ]
}

fn bohr_render(p: &Params) -> String {
    let z = z_of(p.s("element"));
    let charge = p.i("charge");
    let sh = shells(z, charge);
    let nsh = sh.len().max(1);
    let r0 = 36.0;
    let gap = if nsh > 4 { 34.0 } else { 42.0 };
    let rmax = r0 + 22.0 + gap * (nsh as f64 - 1.0);
    let size = 2.0 * (rmax + 26.0);
    let cx = size / 2.0;
    let h = size + if p.b("caption") { 36.0 } else { 0.0 };
    let mut d = Doc::new(size, h);
    title_at(&mut d, p, cx, 15.0, 13.0);
    for (i, &e) in sh.iter().enumerate() {
        let r = r0 + 22.0 + gap * i as f64;
        d.circle(cx, cx, r, "none", INK, 1.5);
        if p.b("empty") { continue; }
        let paired = p.s("placement") == "Paired";
        let groups = if paired { (e as usize).div_ceil(2) } else { e as usize };
        for g in 0..groups {
            let base = -PI / 2.0 + 2.0 * PI * g as f64 / groups as f64;
            if paired {
                let in_g = if g * 2 + 1 < e as usize { 2 } else { 1 };
                if in_g == 2 {
                    let da = 8.5 / r;
                    for s in [-1.0, 1.0] { let a = base + s * da; d.circle(cx + r * a.cos(), cx + r * a.sin(), 5.5, INK, INK, 1.0); }
                } else {
                    d.circle(cx + r * base.cos(), cx + r * base.sin(), 5.5, INK, INK, 1.0);
                }
            } else {
                d.circle(cx + r * base.cos(), cx + r * base.sin(), 5.5, INK, INK, 1.0);
            }
        }
    }
    d.circle(cx, cx, r0, "#f2f2f2", INK, 1.8);
    match p.s("nucleus") {
        "Blank" => {}
        "Protons and neutrons" => {
            let n = mass_number(z).saturating_sub(z);
            tx(&mut d, cx, cx - 3.0, 13.0, "middle", INK, false, &format!("{z}p⁺"));
            tx(&mut d, cx, cx + 14.0, 13.0, "middle", INK, false, &format!("{n}n⁰"));
        }
        _ => tx(&mut d, cx, cx + 8.0, 24.0, "middle", INK, true, symbol(z)),
    }
    if p.b("caption") {
        let ion = match charge { 0 => String::new(), c if c > 0 => format!(" {}{}", if c > 1 { c.to_string() } else { String::new() }, "+"), c => format!(" {}{}", if c < -1 { (-c).to_string() } else { String::new() }, "-") };
        tx(&mut d, cx, h - 12.0, 15.0, "middle", INK, true, &format!("{} ({}{})", name(z), symbol(z), ion));
    }
    d.finish()
}

// ---------------------------------------------------------------------------------------------
// Orbital diagram
// ---------------------------------------------------------------------------------------------

fn orbital_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "6 C - Carbon"),
        ParamDef::int("charge", "Ion charge", -3, 4, 0),
        ParamDef::choice("layout", "Layout", &["Energy row", "Energy ladder"], "Energy row"),
        ParamDef::bool("shorthand", "Noble-gas shorthand", false),
        ParamDef::bool("blank", "Empty boxes (students fill in the arrows)", false),
        ParamDef::bool("config", "Show electron configuration text", true),
        ParamDef::bool("caption", "Caption with element name", false),
        ParamDef::text("title", "Title (optional)", ""),
    ]
}

fn spin_arrows(d: &mut Doc, x: f64, y: f64, bs: f64, up: bool, down: bool) {
    let a = |d: &mut Doc, xx: f64, upw: bool| {
        let (y1, y2) = if upw { (y + bs - 7.0, y + 7.0) } else { (y + 7.0, y + bs - 7.0) };
        d.arrow(xx, y1, xx, y2, INK, 1.6);
    };
    match (up, down) {
        (true, true) => { a(d, x + bs * 0.33, true); a(d, x + bs * 0.67, false); }
        (true, false) => a(d, x + bs / 2.0, true),
        (false, true) => a(d, x + bs / 2.0, false),
        _ => {}
    }
}

fn orbital_render(p: &Params) -> String {
    let z = z_of(p.s("element"));
    let charge = p.i("charge");
    let mut subs = config(z, charge);
    let mut core_label: Option<&str> = None;
    if p.b("shorthand") && charge == 0 {
        if let Some((cz, cs)) = noble_core(z) {
            let hide = config(cz, 0);
            subs.retain(|s| !hide.iter().any(|h| h.n == s.n && h.l == s.l));
            core_label = Some(cs);
        }
    }
    let text = {
        let shown = config_text(&subs, core_label);
        shown
    };
    let bs = 40.0;
    let blank = p.b("blank");
    let ladder = p.s("layout") == "Energy ladder";
    let draw_boxes = |d: &mut Doc, s: &Sub, x: f64, y: f64| {
        let m = (2 * s.l + 1) as usize;
        let e = s.e as usize;
        for k in 0..m {
            d.rect(x + k as f64 * bs, y, bs, bs, "#ffffff", INK, 1.6);
            if !blank {
                let up = k < e.min(m);
                let down = e > m && k < e - m;
                spin_arrows(d, x + k as f64 * bs, y, bs, up, down);
            }
        }
    };
    let ncfg = if p.b("config") { 1 } else { 0 };
    let cap = if p.b("caption") { 1 } else { 0 };
    if !ladder {
        let gap = 26.0;
        let core_w = if core_label.is_some() { 64.0 } else { 0.0 };
        let total: f64 = subs.iter().map(|s| (2 * s.l + 1) as f64 * bs).sum::<f64>() + gap * (subs.len().saturating_sub(1)) as f64 + core_w;
        let w = (total + 80.0).max(300.0);
        let h = 150.0 + 46.0 * ncfg as f64 + 30.0 * cap as f64;
        let mut d = Doc::new(w, h);
        title_at(&mut d, p, w / 2.0, 16.0, 13.0);
        let mut x = (w - total) / 2.0;
        let y = 40.0 + 30.0 * cap as f64;
        if cap == 1 { tx(&mut d, w / 2.0, 30.0, 16.0, "middle", INK, true, &format!("{} ({})", name(z), symbol(z))); }
        if let Some(c) = core_label {
            tx(&mut d, x + 26.0, y + bs / 2.0 + 6.0, 18.0, "middle", INK, true, &format!("[{c}]"));
            x += core_w;
        }
        for s in &subs {
            draw_boxes(&mut d, s, x, y);
            let gw = (2 * s.l + 1) as f64 * bs;
            tx(&mut d, x + gw / 2.0, y + bs + 22.0, 15.0, "middle", INK, false, &s.label());
            x += gw + gap;
        }
        if ncfg == 1 { tx(&mut d, w / 2.0, y + bs + 70.0, 17.0, "middle", INK, false, &text); }
        d.finish()
    } else {
        let rowh = 62.0;
        let n = subs.len().max(1);
        let w = 90.0 + 5.0 * bs + 120.0;
        let h = 50.0 + rowh * n as f64 + 30.0 + 40.0 * ncfg as f64 + 30.0 * cap as f64;
        let mut d = Doc::new(w, h);
        title_at(&mut d, p, w / 2.0, 16.0, 13.0);
        let top = 30.0 + 30.0 * cap as f64;
        if cap == 1 { tx(&mut d, w / 2.0, 30.0, 16.0, "middle", INK, true, &format!("{} ({})", name(z), symbol(z))); }
        let base_y = top + rowh * n as f64 + 10.0;
        d.arrow(40.0, base_y, 40.0, top - 6.0, INK, 1.8);
        tr(&mut d, 22.0, (top + base_y) / 2.0, 14.0, -90.0, "middle", "Energy");
        for (i, s) in subs.iter().enumerate() {
            let y = base_y - rowh * (i as f64 + 1.0) + 8.0;
            let x = 130.0;
            draw_boxes(&mut d, s, x, y);
            tx(&mut d, x - 12.0, y + bs / 2.0 + 5.0, 15.0, "end", INK, false, &s.label());
        }
        if ncfg == 1 { tx(&mut d, w / 2.0, base_y + 36.0, 16.0, "middle", INK, false, &text); }
        d.finish()
    }
}

// ---------------------------------------------------------------------------------------------
// Particle diagram
// ---------------------------------------------------------------------------------------------

const SHAPES: [&str; 6] = ["Alone", "Pair", "Bent", "In a line", "Three around", "Four around"];
const SIZES: [&str; 5] = ["XS", "S", "M", "L", "XL"];
const SHADES: [&str; 5] = ["White", "Light gray", "Gray", "Dark gray", "Black"];

fn particle_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("layout", "Layout", &["In a box", "Lattice"], "In a box"),
        ParamDef::choice("boxes", "Boxes", &["One box", "Before and after"], "One box"),
        ParamDef::choice("state", "State (box, or before)", &["Gas", "Liquid", "Solid"], "Gas"),
        ParamDef::choice("state2", "State after", &["Gas", "Liquid", "Solid"], "Gas"),
    ];
    v.push(ParamDef::text("title", "Title (optional)", ""));
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
    v
}

fn atom_radius(sz: &str) -> f64 { match sz { "XS" => 5.0, "S" => 7.5, "M" => 10.0, "L" => 13.0, _ => 17.0 } }

/// Atom offsets (unit-less multiples of atom radius r) for a molecule shape.
fn mol_offsets(shape: &str, r: f64) -> Vec<(f64, f64)> {
    let d = 2.0 * r * 0.92;
    match shape {
        "Pair" => vec![(-d / 2.0, 0.0), (d / 2.0, 0.0)],
        "Bent" => { let a = 52.0f64.to_radians(); vec![(0.0, -d * 0.35), (-d * a.sin(), d * (a.cos() - 0.35)), (d * a.sin(), d * (a.cos() - 0.35))] }
        "In a line" => vec![(-d, 0.0), (0.0, 0.0), (d, 0.0)],
        "Three around" => (0..3).map(|i| { let a = PI * 2.0 * i as f64 / 3.0 - PI / 2.0; (d * 1.05 * a.cos(), d * 1.05 * a.sin()) }).chain(std::iter::once((0.0, 0.0))).collect(),
        "Four around" => vec![(0.0, 0.0), (d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)],
        _ => vec![(0.0, 0.0)],
    }
}

struct Species { shape: String, r: f64, fill: &'static str }

fn bound(sp: &Species) -> f64 { mol_offsets(&sp.shape, sp.r).iter().map(|(x, y)| (x * x + y * y).sqrt()).fold(0.0, f64::max) + sp.r }

fn draw_mol(d: &mut Doc, sp: &Species, x: f64, y: f64, rot: f64) {
    let (c, s) = (rot.cos(), rot.sin());
    let mut offs = mol_offsets(&sp.shape, sp.r);
    // draw the centre atom last for "around" shapes so bonds read cleanly
    let n = offs.len();
    if matches!(sp.shape.as_str(), "Three around") { offs.rotate_left(0); }
    for (i, (ox, oy)) in offs.iter().enumerate() {
        let _ = (i, n);
        d.circle(x + ox * c - oy * s, y + ox * s + oy * c, sp.r, sp.fill, INK, 1.5);
    }
}

fn fill_box(d: &mut Doc, bx: f64, by: f64, bs: f64, state: &str, layout_lattice: bool, items: &[Species], seed: u64) {
    d.rect(bx, by, bs, bs, "none", INK, 2.2);
    if items.is_empty() { return; }
    let mut rg = Rng(seed);
    let m = 8.0;
    if layout_lattice {
        let n = items.len();
        let maxb = items.iter().map(|s| s.r).fold(0.0, f64::max);
        let cols = ((n as f64).sqrt().ceil() as usize).max(1);
        let rows = n.div_ceil(cols);
        let step = (2.0 * maxb * 1.02).min((bs - 2.0 * m) / cols.max(rows) as f64);
        let gx = bx + (bs - step * cols as f64) / 2.0 + step / 2.0;
        let gy = by + (bs - step * rows as f64) / 2.0 + step / 2.0;
        // checkerboard over species order
        let mut cells: Vec<(usize, usize)> = (0..rows).flat_map(|r| (0..cols).map(move |c| (r, c))).take(n).collect();
        cells.sort_by_key(|&(r, c)| ((r + c) % 2, r, c));
        for (k, (r, c)) in cells.iter().enumerate() {
            let sp = &items[k.min(n - 1)];
            let rr = sp.r.min(step / 2.0 - 0.5);
            d.circle(gx + *c as f64 * step, gy + *r as f64 * step, rr, sp.fill, INK, 1.5);
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
        let rows = items.len().div_ceil(per_row);
        for (k, &ii) in idx.iter().enumerate() {
            let (r, c) = (k / per_row, k % per_row);
            let in_row = per_row.min(items.len() - r * per_row);
            let x = bx + bs / 2.0 + (c as f64 - (in_row as f64 - 1.0) / 2.0) * step;
            let y = by + bs - m - step / 2.0 - r as f64 * step;
            let _ = rows;
            draw_mol(d, &items[ii], x, y, 0.0);
        }
        return;
    }
    let pad_try: [f64; 3] = if state == "Gas" { [bs * 0.06, bs * 0.03, 1.0] } else { [2.5, 1.0, 0.0] };
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| bound(&items[b]).partial_cmp(&bound(&items[a])).unwrap());
    for &ii in &order {
        let sp = &items[ii];
        let rb = bound(sp);
        let mut ok = false;
        'outer: for &pad in &pad_try {
            for _ in 0..1500 {
                let x = rg.range(bx + m + rb, bx + bs - m - rb);
                let y = rg.range(ymin + rb, ymax - rb);
                if placed.iter().all(|&(px, py, pr)| ((px - x).powi(2) + (py - y).powi(2)).sqrt() >= pr + rb + pad) {
                    placed.push((x, y, rb));
                    draw_mol(d, sp, x, y, rg.range(0.0, PI * 2.0));
                    ok = true;
                    break 'outer;
                }
            }
        }
        if !ok { /* box too crowded: skip rather than overlap */ }
    }
}

fn particle_render(p: &Params) -> String {
    let two = p.s("boxes") == "Before and after";
    let lattice = p.s("layout") == "Lattice";
    let build = |counts: [&str; 3]| -> Vec<Species> {
        let mut v = Vec::new();
        for (i, k) in ["a", "b", "c"].iter().enumerate() {
            let n = p.i(counts[i]).clamp(0, 24);
            let sp = || Species { shape: p.s(&format!("{k}_shape")).to_string(), r: atom_radius(p.s(&format!("{k}_size"))), fill: shade(p.s(&format!("{k}_shade"))) };
            for _ in 0..n { v.push(sp()); }
        }
        v
    };
    let bs = 320.0;
    let w = if two { bs * 2.0 + 150.0 } else { bs + 40.0 };
    let h = bs + if two { 70.0 } else { 40.0 };
    let mut d = Doc::new(w, h);
    title_at(&mut d, p, w / 2.0, 14.0, 13.0);
    let before = build(["a_count", "b_count", "c_count"]);
    fill_box(&mut d, 20.0, 20.0, bs, p.s("state"), lattice, &before, 11);
    if two {
        let after = build(["a2_count", "b2_count", "c2_count"]);
        d.arrow(bs + 38.0, 20.0 + bs / 2.0, bs + 112.0, 20.0 + bs / 2.0, INK, 2.2);
        fill_box(&mut d, bs + 130.0, 20.0, bs, p.s("state2"), lattice, &after, 23);
        tx(&mut d, 20.0 + bs / 2.0, bs + 52.0, 15.0, "middle", INK, true, p.s("lab_before"));
        tx(&mut d, bs + 130.0 + bs / 2.0, bs + 52.0, 15.0, "middle", INK, true, p.s("lab_after"));
    }
    d.finish()
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

pub(super) fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<Handle>> {
    match id {
        "bohr-model" => {
            let z = z_of(p.s("element"));
            let nsh = shells(z, p.i("charge")).len().max(1);
            let r0 = 36.0;
            let gap = if nsh > 4 { 34.0 } else { 42.0 };
            let rmax = r0 + 22.0 + gap * (nsh as f64 - 1.0);
            let cx = rmax + 26.0;
            Some(vec![
                Handle::zone((cx, cx), (66.0, 66.0), "Click: next element. Scroll: change element", |q| step_element(q, 1, 36))
                    .with_scroll(|q, dy| step_element(q, if dy > 0.0 { 1 } else { -1 }, 36)),
                Handle::zone((cx, cx - rmax), (60.0, 22.0), "Click: add an electron (ion charge). Scroll up adds, down removes", |q| {
                    let c = q.i("charge");
                    q.set_i("charge", if c <= -3 { 4 } else { c - 1 });
                })
                .with_scroll(|q, dy| q.set_i("charge", q.i("charge") - if dy > 0.0 { 1 } else { -1 })),
            ])
        }
        "orbital-diagram" => Some(vec![
            Handle::zone((size.0 / 2.0, size.1 * 0.42), (size.0 * 0.8, size.1 * 0.5), "Click: next element. Scroll: change the ion charge", |q| step_element(q, 1, 36))
                .with_scroll(|q, dy| q.set_i("charge", q.i("charge") - if dy > 0.0 { 1 } else { -1 })),
        ]),
        "particle-diagram" => {
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
                let keys: [&'static str; 3] = if after { ["a2_count", "b2_count", "c2_count"] } else { ["a_count", "b_count", "c_count"] };
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
