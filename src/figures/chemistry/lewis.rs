//! Lewis structures: formula parser + electron bookkeeping + VSEPR-style flat layout.
use super::data::*;
use super::*;
use crate::figures::{ParamDef, Params, Subject};

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: "lewis-structures", subject: Subject::Chemistry, title: "Lewis Structures",
        blurb: "Type a formula (or pick a multi-centre molecule) and get its dot-and-line structure with lone pairs.",
        params: params, render: render,
    }]
}

const PRESETS: [&str; 20] = [
    "None (use the formula)", "C₂H₆, ethane", "C₂H₄, ethene", "C₂H₂, ethyne", "CH₃OH, methanol", "C₂H₅OH, ethanol",
    "CH₃OCH₃, dimethyl ether", "CH₃COOH, acetic acid", "CH₃COCl, acetyl chloride", "HCOOH, formic acid", "CH₃NH₂, methylamine", "N₂H₄, hydrazine",
    "H₂O₂, hydrogen peroxide", "N₂O, nitrous oxide", "HOCl, hypochlorous acid", "HNO₃, nitric acid",
    "H₂CO₃, carbonic acid", "H₂SO₄, sulfuric acid", "H₃PO₄, phosphoric acid", "HClO₄, perchloric acid",
];

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("formula", "Formula (e.g. H2O, CO2, NH4+, SO4 2-)", "H2O"),
        ParamDef::choice("preset", "Or pick a molecule from the list (overrides the formula)", &PRESETS, "None (use the formula)"),
        ParamDef::bool("lone", "Show lone pairs", true),
        ParamDef::bool("formal", "Show formal charges", false),
        ParamDef::bool("expand", "Expand the octet to lower formal charges", false),
        ParamDef::text("title", "Title (optional)", ""),
        ParamDef::text("tweak", "Manual edits (click bonds and atoms on the figure; blank = none)", ""),
    ]
}

struct Atom { sym: String, x: f64, y: f64, nb: i32, forced: Option<Vec<f64>>, val: i32 }
struct Bond { a: usize, b: usize, order: i32 }
struct Mol { atoms: Vec<Atom>, bonds: Vec<Bond>, charge: i32 }

const L: f64 = 72.0;

impl Mol {
    fn add(&mut self, sym: &str, parent: Option<usize>, ang: f64, order: i32) -> usize {
        let (x, y) = match parent {
            Some(p) => (self.atoms[p].x + L * ang.to_radians().cos(), self.atoms[p].y - L * ang.to_radians().sin()),
            None => (0.0, 0.0),
        };
        let val = atom_info(sym).map(|a| a.0).unwrap_or(4);
        self.atoms.push(Atom { sym: sym.into(), x, y, nb: 0, forced: None, val });
        let i = self.atoms.len() - 1;
        if let Some(p) = parent { self.bonds.push(Bond { a: p, b: i, order }); }
        i
    }
    fn bond_sum(&self, i: usize) -> i32 { self.bonds.iter().filter(|b| b.a == i || b.b == i).map(|b| b.order).sum() }
    fn octet_fill(&mut self) {
        for i in 0..self.atoms.len() {
            let bs = self.bond_sum(i);
            self.atoms[i].nb = if self.atoms[i].sym == "H" { 0 } else { (8 - 2 * bs).max(0) };
        }
    }
    fn formal(&self, i: usize) -> i32 { self.atoms[i].val - self.atoms[i].nb - self.bond_sum(i) }
}

fn preset(name: &str) -> Mol {
    let mut m = Mol { atoms: vec![], bonds: vec![], charge: 0 };
    let hs = |m: &mut Mol, at: usize, angs: &[f64]| { for &a in angs { m.add("H", Some(at), a, 1); } };
    let key = name.split(',').next().unwrap_or("");
    match key {
        "C₂H₆" => { let a = m.add("C", None, 0.0, 1); let b = m.add("C", Some(a), 0.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); hs(&mut m, b, &[90.0, 0.0, 270.0]); }
        "C₂H₄" => { let a = m.add("C", None, 0.0, 1); let b = m.add("C", Some(a), 0.0, 2); hs(&mut m, a, &[135.0, 225.0]); hs(&mut m, b, &[45.0, 315.0]); }
        "C₂H₂" => { let a = m.add("C", None, 0.0, 1); let b = m.add("C", Some(a), 0.0, 3); hs(&mut m, a, &[180.0]); hs(&mut m, b, &[0.0]); }
        "CH₃OH" => { let a = m.add("C", None, 0.0, 1); let o = m.add("O", Some(a), 0.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); hs(&mut m, o, &[330.0]); }
        "C₂H₅OH" => { let a = m.add("C", None, 0.0, 1); let b = m.add("C", Some(a), 0.0, 1); let o = m.add("O", Some(b), 0.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); hs(&mut m, b, &[90.0, 270.0]); hs(&mut m, o, &[330.0]); }
        "CH₃OCH₃" => { let a = m.add("C", None, 0.0, 1); let o = m.add("O", Some(a), 330.0, 1); let c = m.add("C", Some(o), 30.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); hs(&mut m, c, &[90.0, 0.0, 270.0]); }
        "CH₃COOH" => { let a = m.add("C", None, 0.0, 1); let b = m.add("C", Some(a), 0.0, 1); m.add("O", Some(b), 90.0, 2); let o = m.add("O", Some(b), 0.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); hs(&mut m, o, &[330.0]); m.atoms[o].forced = None; }
        "CH₃COCl" => { let a = m.add("C", None, 0.0, 1); let b = m.add("C", Some(a), 0.0, 1); m.add("O", Some(b), 90.0, 2); m.add("Cl", Some(b), 330.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); }
        "HCOOH" => { let a = m.add("C", None, 0.0, 1); m.add("O", Some(a), 90.0, 2); hs(&mut m, a, &[180.0]); let o = m.add("O", Some(a), 0.0, 1); hs(&mut m, o, &[330.0]); }
        "CH₃NH₂" => { let a = m.add("C", None, 0.0, 1); let n = m.add("N", Some(a), 0.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); hs(&mut m, n, &[45.0, 315.0]); }
        "N₂H₄" => { let a = m.add("N", None, 0.0, 1); let b = m.add("N", Some(a), 0.0, 1); hs(&mut m, a, &[135.0, 225.0]); hs(&mut m, b, &[45.0, 315.0]); }
        "H₂O₂" => { let h = m.add("H", None, 0.0, 1); let o = m.add("O", Some(h), 330.0, 1); let o2 = m.add("O", Some(o), 30.0, 1); hs(&mut m, o2, &[330.0]); }
        "N₂O" => { let a = m.add("N", None, 0.0, 1); let b = m.add("N", Some(a), 0.0, 3); m.add("O", Some(b), 0.0, 1); }
        "HOCl" => { let h = m.add("H", None, 0.0, 1); let o = m.add("O", Some(h), 330.0, 1); m.add("Cl", Some(o), 30.0, 1); }
        "HNO₃" => { let n = m.add("N", None, 0.0, 1); m.add("O", Some(n), 90.0, 2); m.add("O", Some(n), 270.0, 1); let o = m.add("O", Some(n), 0.0, 1); hs(&mut m, o, &[330.0]); }
        "H₂CO₃" => { let c = m.add("C", None, 0.0, 1); m.add("O", Some(c), 90.0, 2); let o1 = m.add("O", Some(c), 180.0, 1); let o2 = m.add("O", Some(c), 0.0, 1); hs(&mut m, o1, &[210.0]); hs(&mut m, o2, &[330.0]); }
        "H₂SO₄" => { let s = m.add("S", None, 0.0, 1); m.add("O", Some(s), 90.0, 2); m.add("O", Some(s), 270.0, 2); let o1 = m.add("O", Some(s), 180.0, 1); let o2 = m.add("O", Some(s), 0.0, 1); hs(&mut m, o1, &[210.0]); hs(&mut m, o2, &[330.0]); }
        "H₃PO₄" => { let p = m.add("P", None, 0.0, 1); m.add("O", Some(p), 90.0, 2); let o1 = m.add("O", Some(p), 180.0, 1); let o2 = m.add("O", Some(p), 0.0, 1); let o3 = m.add("O", Some(p), 270.0, 1); hs(&mut m, o1, &[210.0]); hs(&mut m, o2, &[330.0]); hs(&mut m, o3, &[300.0]); }
        _ => { let c = m.add("Cl", None, 0.0, 1); m.add("O", Some(c), 90.0, 2); m.add("O", Some(c), 270.0, 2); m.add("O", Some(c), 0.0, 2); let o = m.add("O", Some(c), 180.0, 1); hs(&mut m, o, &[210.0]); }
    }
    m.octet_fill();
    m
}

/// Parse "SO4 2-", "NH4+", "H2O" into element counts and charge.
fn parse(raw: &str) -> Result<(Vec<(String, usize)>, i32), String> {
    let mut s: String = raw.trim().replace('−', "-").replace('⁻', "-").replace('⁺', "+");
    // unicode subscripts -> digits
    for (i, c) in "₀₁₂₃₄₅₆₇₈₉".chars().enumerate() { s = s.replace(c, &i.to_string()); }
    for (i, c) in "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().enumerate() { s = s.replace(c, &format!("^{i}")); }
    s = s.replace("^-", "-").replace("^+", "+");
    let mut charge = 0;
    if let Some(sign) = s.chars().last().filter(|c| *c == '+' || *c == '-') {
        let sg = if sign == '+' { 1 } else { -1 };
        s.pop();
        let digits: String = s.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
        let before = &s[..s.len() - digits.len()];
        let sep = before.ends_with(' ') || before.ends_with('^');
        let mut mag = 1;
        if !digits.is_empty() {
            if sep { mag = digits.parse().unwrap_or(1); s.truncate(s.len() - digits.len()); }
            else if digits.len() >= 2 { mag = digits[digits.len() - 1..].parse().unwrap_or(1); s.pop(); }
        }
        charge = sg * mag;
    }
    let s: String = s.chars().filter(|c| !c.is_whitespace() && *c != '^').collect();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out: Vec<(String, usize)> = Vec::new();
    while i < cs.len() {
        if !cs[i].is_ascii_uppercase() { return Err(format!("Can't read '{}' in the formula", cs[i])); }
        let mut sym = cs[i].to_string();
        i += 1;
        while i < cs.len() && cs[i].is_ascii_lowercase() { sym.push(cs[i]); i += 1; }
        let mut n = String::new();
        while i < cs.len() && cs[i].is_ascii_digit() { n.push(cs[i]); i += 1; }
        if atom_info(&sym).is_none() { return Err(format!("Unknown element {sym}")); }
        out.push((sym, n.parse().unwrap_or(1).max(1)));
    }
    if out.is_empty() { return Err("Enter a formula".into()); }
    Ok((out, charge))
}

fn from_formula(raw: &str, expand: bool) -> Result<Mol, String> {
    let (parts, charge) = parse(raw)?;
    let mut syms: Vec<String> = Vec::new();
    for (s, n) in &parts { for _ in 0..*n { syms.push(s.clone()); } }
    if syms.len() > 9 { return Err("Too many atoms; use the multi-centre list for larger molecules".into()); }
    if syms.len() == 1 {
        let mut m = Mol { atoms: vec![], bonds: vec![], charge };
        let a = m.add(&syms[0], None, 0.0, 1);
        let v = m.atoms[a].val;
        m.atoms[a].nb = if syms[0] == "H" { 1 - charge } else { (v - charge).max(0) };
        if syms[0] == "He" { m.atoms[a].nb = 2; }
        return Ok(m);
    }
    // central atom: lowest electronegativity among non-H atoms (first on ties)
    let mut ci = None;
    let mut best = f64::MAX;
    for (i, s) in syms.iter().enumerate() {
        if s == "H" { continue; }
        let en = atom_info(s).map(|a| a.1).unwrap_or(9.0);
        if en < best - 1e-9 { best = en; ci = Some(i); }
    }
    let ci = ci.unwrap_or(0);
    let mut m = Mol { atoms: vec![], bonds: vec![], charge };
    let c = m.add(&syms[ci], None, 0.0, 1);
    let mut terms = Vec::new();
    for (i, s) in syms.iter().enumerate() { if i != ci { terms.push(m.add(s, None, 0.0, 1)); } }
    for &t in &terms { m.bonds.push(Bond { a: c, b: t, order: 1 }); }
    let total: i32 = m.atoms.iter().map(|a| a.val).sum::<i32>() - charge;
    let mut rem = total - 2 * terms.len() as i32;
    for &t in &terms {
        let give = if m.atoms[t].sym == "H" { 0 } else { 6.min(rem.max(0)) };
        m.atoms[t].nb = give;
        rem -= give;
    }
    m.atoms[c].nb = rem.max(0);
    let period = atom_info(&m.atoms[c].sym).map(|a| a.2).unwrap_or(2);
    let deficient_ok = matches!(m.atoms[c].sym.as_str(), "Be" | "B" | "Al" | "H");
    let central_e = |m: &Mol| m.atoms[c].nb + 2 * m.bond_sum(c);
    let convert = |m: &mut Mol| -> bool {
        let mut pick: Option<usize> = None;
        for (k, &t) in terms.iter().enumerate() {
            if m.atoms[t].sym != "H" && m.atoms[t].nb >= 2 && pick.map_or(true, |p| m.atoms[terms[p]].nb < m.atoms[t].nb) { pick = Some(k); }
        }
        if let Some(k) = pick {
            let t = terms[k];
            m.atoms[t].nb -= 2;
            if let Some(b) = m.bonds.iter_mut().find(|b| b.b == t) { b.order += 1; }
            true
        } else { false }
    };
    if !deficient_ok {
        while central_e(&m) + 2 <= 8 && central_e(&m) < 8 { if !convert(&mut m) { break; } }
    }
    if expand && period >= 3 {
        while m.formal(c) > 0 { if !convert(&mut m) { break; } }
    }
    // geometry
    let nlp = (m.atoms[c].nb / 2) as usize;
    let nt = terms.len();
    let dom = nt + nlp;
    let order: &[f64] = match dom {
        0 | 1 => &[180.0], 2 => &[180.0, 0.0], 3 => &[90.0, 210.0, 330.0], 4 => &[90.0, 0.0, 180.0, 270.0],
        5 => &[90.0, 270.0, 0.0, 150.0, 210.0], 6 if nlp == 0 => &[90.0, 270.0, 30.0, 150.0, 210.0, 330.0], _ => &[90.0, 270.0, 0.0, 180.0, 135.0, 315.0, 45.0, 225.0],
    };
    // 2 domains with bent shapes: put lone pairs along the same positions
    let order_v: Vec<f64> = order.to_vec();
    for (k, &t) in terms.iter().enumerate() {
        let a = order_v[k % order_v.len()];
        m.atoms[t].x = L * a.to_radians().cos();
        m.atoms[t].y = -L * a.to_radians().sin();
    }
    if nlp > 0 { m.atoms[c].forced = Some((0..nlp).map(|i| order_v[(nt + i) % order_v.len()]).collect()); }
    // for bent molecules (3 domains, 2 terminals) the VSEPR angle is ~120/109: use 3-domain layout
    // single terminal at the centre atom: put it on the right
    if nt == 1 { m.atoms[terms[0]].x = L; m.atoms[terms[0]].y = 0.0; if nlp > 0 { m.atoms[c].forced = None; } }
    Ok(m)
}

fn ang_dist(a: f64, b: f64) -> f64 { let d = (a - b).rem_euclid(360.0); d.min(360.0 - d) }

/// Builds the molecule for the current params including manual edits from `tweak`
/// ("b2=3" sets bond 2 to a triple bond, "l1=2" gives atom 1 two lone pairs).
fn build(p: &Params) -> Result<Mol, String> {
    let expand = p.b("expand");
    let mut m = if p.s("preset") != "None (use the formula)" && !p.s("preset").is_empty() {
        preset(p.s("preset"))
    } else {
        from_formula(p.s("formula"), expand)?
    };
    for tok in p.s("tweak").split_whitespace() {
        let Some((k, v)) = tok.split_once('=') else { continue };
        let (Some(kind), Ok(i), Ok(n)) = (k.chars().next(), k[1..].parse::<usize>(), v.parse::<i32>()) else { continue };
        match kind {
            'b' => if let Some(b) = m.bonds.get_mut(i) { b.order = n.clamp(1, 3); },
            'l' => if let Some(a) = m.atoms.get_mut(i) { a.nb = (2 * n).clamp(0, 8); a.forced = None; },
            _ => {}
        }
    }
    Ok(m)
}

struct Frame { w: f64, h: f64, ox: f64, oy: f64, x0: f64, x1: f64, y0: f64, y1: f64 }

fn frame(m: &Mol) -> Frame {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for a in &m.atoms { x0 = x0.min(a.x); x1 = x1.max(a.x); y0 = y0.min(a.y); y1 = y1.max(a.y); }
    let margin = 56.0;
    let extra = if m.charge != 0 { 16.0 } else { 0.0 };
    let w = (x1 - x0 + 2.0 * margin + 2.0 * extra).max(200.0);
    let h = (y1 - y0 + 2.0 * margin + 2.0 * extra).max(170.0);
    let (ox, oy) = ((w - (x1 - x0)) / 2.0 - x0, (h - (y1 - y0)) / 2.0 - y0);
    Frame { w, h, ox, oy, x0, x1, y0, y1 }
}

fn render(p: &Params) -> String {
    let m = match build(p) {
        Ok(m) => m,
        Err(e) => {
            let mut d = Doc::new(420.0, 120.0);
            tx(&mut d, 210.0, 66.0, 15.0, "middle", "#b03030", false, &e);
            return d.finish();
        }
    };
    let Frame { w, h, ox, oy, x0, x1, y0, y1 } = frame(&m);
    let ion = m.charge != 0;
    let mut d = Doc::new(w, h);
    title_at(&mut d, p, w / 2.0, 22.0, 15.0);
    let pos = |i: usize| (m.atoms[i].x + ox, m.atoms[i].y + oy);
    // bonds
    for b in &m.bonds {
        let (ax, ay) = pos(b.a);
        let (bx, by) = pos(b.b);
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / len, dy / len);
        let (nx, ny) = (-uy, ux);
        let g = |s: &str| if s == "H" { 10.0 } else { 13.0 };
        let (sa, sb) = (g(&m.atoms[b.a].sym), g(&m.atoms[b.b].sym));
        let offs: Vec<f64> = match b.order { 1 => vec![0.0], 2 => vec![-3.5, 3.5], _ => vec![-5.0, 0.0, 5.0] };
        for o in offs {
            d.line(ax + ux * sa + nx * o, ay + uy * sa + ny * o, bx - ux * sb + nx * o, by - uy * sb + ny * o, INK, 2.0);
        }
    }
    // atoms
    for (i, a) in m.atoms.iter().enumerate() {
        let (x, y) = pos(i);
        tx(&mut d, x, y + 8.0, 24.0, "middle", INK, true, &a.sym);
        if p.b("lone") {
            let mut dirs: Vec<f64> = Vec::new();
            for b in &m.bonds {
                let other = if b.a == i { Some(b.b) } else if b.b == i { Some(b.a) } else { None };
                if let Some(o) = other {
                    let (ox2, oy2) = pos(o);
                    dirs.push((-(oy2 - y)).atan2(ox2 - x).to_degrees().rem_euclid(360.0));
                }
            }
            let pairs = (a.nb / 2).max(0) as usize;
            let single = a.nb % 2 == 1;
            let mut chosen: Vec<f64> = Vec::new();
            if let Some(f) = &a.forced { chosen = f.clone(); }
            else {
                let mut all = dirs.clone();
                for _ in 0..(pairs + if single { 1 } else { 0 }) {
                    let mut best = (f64::MIN, 0.0);
                    for k in 0..24 {
                        let ang = k as f64 * 15.0;
                        let mind = all.iter().map(|&o| ang_dist(ang, o)).fold(360.0, f64::min);
                        let bonus = if (ang % 90.0).abs() < 1e-6 { 1.0 } else if (ang % 45.0).abs() < 1e-6 { 0.5 } else { 0.0 };
                        let score = mind.min(120.0) + bonus;
                        if score > best.0 + 1e-9 { best = (score, ang); }
                    }
                    chosen.push(best.1);
                    all.push(best.1);
                }
            }
            for (k, &ang) in chosen.iter().enumerate() {
                let r = if a.sym.len() > 1 { 27.0 } else { 21.0 };
                let (c, s) = (ang.to_radians().cos(), ang.to_radians().sin());
                let (px, py) = (x + r * c, y - r * s);
                if k < pairs || a.forced.is_some() {
                    for sgn in [-1.0, 1.0] { d.circle(px + s * 3.6 * sgn, py + c * 3.6 * sgn, 2.3, INK, INK, 0.5); }
                } else {
                    d.circle(px, py, 2.3, INK, INK, 0.5);
                }
            }
            if a.forced.is_some() && single {
                d.circle(x + 21.0, y - 21.0, 2.3, INK, INK, 0.5);
            }
        }
        if p.b("formal") {
            let fc = m.formal(i);
            if fc != 0 {
                let t = if fc.abs() == 1 { if fc > 0 { "+".to_string() } else { "−".to_string() } } else { format!("{}{}", fc.abs(), if fc > 0 { "+" } else { "−" }) };
                d.circle(x + 17.0, y - 20.0, 8.0, "#fff", "#444", 1.0);
                tx(&mut d, x + 17.0, y - 16.5, 10.0, "middle", INK, true, &t);
            }
        }
    }
    if ion {
        let (bx0, bx1, by0, by1) = (x0 + ox - 40.0, x1 + ox + 40.0, y0 + oy - 40.0, y1 + oy + 40.0);
        d.polyline(&[(bx0 + 10.0, by0), (bx0, by0), (bx0, by1), (bx0 + 10.0, by1)], INK, 2.0);
        d.polyline(&[(bx1 - 10.0, by0), (bx1, by0), (bx1, by1), (bx1 - 10.0, by1)], INK, 2.0);
        let c = m.charge.abs();
        let t = format!("{}{}", if c > 1 { c.to_string() } else { String::new() }, if m.charge > 0 { "+" } else { "−" });
        tx(&mut d, bx1 + 4.0, by0 + 6.0, 17.0, "start", INK, true, &t);
    }
    d.finish()
}

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

use crate::figures::interact::Handle;

/// Rewrites one `tweak` token (`b3=2`), keeping the others.
fn set_tweak(q: &mut Params, key: &str, val: i32) {
    let mut toks: Vec<String> = q.s("tweak").split_whitespace().filter(|t| !t.starts_with(&format!("{key}="))).map(String::from).collect();
    toks.push(format!("{key}={val}"));
    q.set_s("tweak", &toks.join(" "));
}

pub(super) fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != "lewis-structures" {
        return None;
    }
    let Ok(m) = build(p) else { return Some(Vec::new()) };
    let fr = frame(&m);
    let pos = |i: usize| (m.atoms[i].x + fr.ox, m.atoms[i].y + fr.oy);
    let mut v = Vec::new();
    for (i, b) in m.bonds.iter().enumerate() {
        let (a, c) = (pos(b.a), pos(b.b));
        let order = b.order;
        v.push(Handle::zone(((a.0 + c.0) / 2.0, (a.1 + c.1) / 2.0), (22.0, 22.0), "Click to cycle the bond: single, double, triple", move |q| {
            set_tweak(q, &format!("b{i}"), order % 3 + 1);
        }));
    }
    if p.b("lone") {
        for (i, a) in m.atoms.iter().enumerate() {
            if a.sym == "H" { continue; }
            let pairs = a.nb / 2;
            let bs = m.bond_sum(i);
            let cap = ((8 - 2 * bs).max(0) / 2).max(pairs);
            let c = pos(i);
            v.push(Handle::zone((c.0, c.1 - 2.0), (30.0, 30.0), "Click to change the number of lone pairs on this atom", move |q| {
                set_tweak(q, &format!("l{i}"), if pairs >= cap { 0 } else { pairs + 1 });
            }));
        }
    }
    if p.s("preset") == "None (use the formula)" || p.s("preset").is_empty() {
        v.push(Handle::label((fr.w / 2.0, 22.0), (150.0, 24.0), "formula", "Double-click to edit the formula"));
    }
    let _ = (fr.x0, fr.x1, fr.y0, fr.y1);
    Some(v)
}
