//! Lewis structures: formula parser + electron bookkeeping + flat (four sides) or shaped
//! (VSEPR) layout, dot or line bonds, resonance structures, question modes ("bonds only",
//! "skeleton") and deliberate mistakes (central atom, bonds, lone electrons) with an answer key.
use super::data::*;
use super::*;
use crate::figures::{ParamDef, Params, Subject};

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: "lewis-structures", subject: Subject::Chemistry, title: "Lewis Structures",
        blurb: "Type a formula (or a molecule's name from the list) and get its dot-and-line structure with lone pairs, resonance and question modes.",
        params: params, render: render,
    }]
}

const PRESETS: [&str; 23] = [
    "None (use the formula)", "C₂H₆, ethane", "C₂H₄, ethene", "C₂H₂, ethyne", "CH₃OH, methanol", "C₂H₅OH, ethanol",
    "CH₃OCH₃, dimethyl ether", "CH₃COOH, acetic acid", "CH₃COO⁻, acetate", "CH₃COCl, acetyl chloride", "HCOOH, formic acid", "CH₃NH₂, methylamine", "N₂H₄, hydrazine",
    "H₂O₂, hydrogen peroxide", "N₂O, nitrous oxide", "HOCl, hypochlorous acid", "HNO₂, nitrous acid", "HNO₃, nitric acid",
    "H₂CO₃, carbonic acid", "HCO₃⁻, hydrogen carbonate", "H₂SO₄, sulfuric acid", "H₃PO₄, phosphoric acid", "HClO₄, perchloric acid",
];

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("formula", "Formula or name (e.g. H2O, CO2, NH4+, SO4 2-, ethanol)", "H2O"),
        ParamDef::choice("preset", "Or pick a molecule with more than one central atom (overrides the formula)", &PRESETS, "None (use the formula)"),
        ParamDef::bool("lone", "Show lone pairs", true),
        ParamDef::bool("formal", "Formal charges (label each atom whose formal charge isn't 0)", false),
        ParamDef::bool("expand", "Expand the octet to lower formal charges", false),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::text("tweak", "Changes: b2=3 (bond 2 triple, 0 = none), e1=4 (atom 1 has 4 lone electrons); or click bonds and atoms", ""),
        ParamDef::choice("shape", "Shape", &["Flat", "Shaped"], "Flat"),
        ParamDef::choice("bond_style", "Bonds", &["Lines", "Dots"], "Lines"),
        ParamDef::choice("resonance", "Resonance structures", &["Show one", "Show all"], "Show one"),
        ParamDef::int("resonance_pick", "Which resonance structure", 1, 6, 1),
        ParamDef::choice("question", "Give students", &["Full structure", "Bonds only", "Skeleton"], "Full structure"),
        ParamDef::text("central", "Central atom (symbol; blank = the correct one)", ""),
        title_style(false),
        answer_param("Answer key (the full structure, or the mistakes in a changed structure)"),
    ]
}

#[derive(Clone)]
struct Atom { sym: String, x: f64, y: f64, nb: i32, forced: Option<Vec<f64>>, val: i32 }
#[derive(Clone)]
struct Bond { a: usize, b: usize, order: i32 }
#[derive(Clone)]
struct Mol { atoms: Vec<Atom>, bonds: Vec<Bond>, charge: i32, centre: Option<usize> }

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
    fn electrons(&self) -> i32 { self.atoms.iter().map(|a| a.nb).sum::<i32>() + 2 * self.bonds.iter().map(|b| b.order).sum::<i32>() }
}

fn preset(name: &str) -> Mol {
    let mut m = Mol { atoms: vec![], bonds: vec![], charge: 0, centre: None };
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
        "CH₃COO⁻" => { m.charge = -1; let a = m.add("C", None, 0.0, 1); let b = m.add("C", Some(a), 0.0, 1); m.add("O", Some(b), 90.0, 2); m.add("O", Some(b), 330.0, 1); hs(&mut m, a, &[90.0, 180.0, 270.0]); }
        "HNO₂" => { let h = m.add("H", None, 0.0, 1); let o = m.add("O", Some(h), 0.0, 1); let n = m.add("N", Some(o), 30.0, 1); m.add("O", Some(n), 330.0, 2); }
        "HCO₃⁻" => { m.charge = -1; let c = m.add("C", None, 0.0, 1); m.add("O", Some(c), 90.0, 2); let o1 = m.add("O", Some(c), 210.0, 1); m.add("O", Some(c), 330.0, 1); hs(&mut m, o1, &[150.0]); }
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


/// Terminal directions (degrees) and central lone-pair directions for `nt` terminals and `nlp`
/// lone pairs on the central atom.
fn directions(flat: bool, nt: usize, nlp: usize) -> (Vec<f64>, Vec<f64>) {
    let sides = [180.0, 0.0, 270.0, 90.0];
    if flat {
        if nt + nlp <= 4 {
            let t: Vec<f64> = sides.iter().take(nt).cloned().collect();
            let l: Vec<f64> = sides.iter().skip(nt).take(nlp).cloned().collect();
            return (t, l);
        }
        let more = [180.0, 0.0, 270.0, 90.0, 135.0, 45.0, 225.0, 315.0];
        return (more.iter().take(nt).cloned().collect(), more.iter().skip(nt).take(nlp).cloned().collect());
    }
    let (t, l): (Vec<f64>, Vec<f64>) = match (nt, nlp) {
        (1, n) => (vec![0.0], [180.0, 90.0, 270.0].iter().take(n).cloned().collect()),
        (2, 0) => (vec![180.0, 0.0], vec![]),
        (2, 1) => (vec![210.0, 330.0], vec![90.0]),
        (2, 2) => (vec![225.0, 315.0], vec![135.0, 45.0]),
        (2, 3) => (vec![180.0, 0.0], vec![90.0, 270.0, 225.0]),
        (3, 0) => (vec![90.0, 210.0, 330.0], vec![]),
        (3, 1) => (vec![210.0, 270.0, 330.0], vec![90.0]),
        (3, 2) => (vec![90.0, 270.0, 0.0], vec![150.0, 210.0]),
        (4, 0) => (vec![90.0, 210.0, 270.0, 330.0], vec![]),
        (4, 1) => (vec![90.0, 270.0, 200.0, 340.0], vec![20.0]),
        (4, 2) => (vec![0.0, 90.0, 180.0, 270.0], vec![45.0, 225.0]),
        (5, 0) => (vec![90.0, 270.0, 0.0, 150.0, 210.0], vec![]),
        (5, 1) => (vec![270.0, 0.0, 180.0, 210.0, 330.0], vec![90.0]),
        (6, 0) => (vec![90.0, 270.0, 30.0, 150.0, 210.0, 330.0], vec![]),
        _ => {
            let more = [90.0, 270.0, 0.0, 180.0, 135.0, 315.0, 45.0, 225.0];
            (more.iter().take(nt).cloned().collect(), more.iter().skip(nt).take(nlp).cloned().collect())
        }
    };
    (t, l)
}

fn from_formula(raw: &str, expand: bool, flat: bool, central: Option<&str>) -> Result<Mol, String> {
    let (parts, charge) = parse(raw)?;
    let mut syms: Vec<String> = Vec::new();
    for (s, n) in &parts { for _ in 0..*n { syms.push(s.clone()); } }
    if syms.len() > 9 { return Err("Too many atoms; use the multi-centre list for larger molecules".into()); }
    if syms.len() == 1 {
        let mut m = Mol { atoms: vec![], bonds: vec![], charge, centre: Some(0) };
        let a = m.add(&syms[0], None, 0.0, 1);
        let v = m.atoms[a].val;
        m.atoms[a].nb = if syms[0] == "H" { (1 - charge).max(0) } else { (v - charge).max(0) };
        if syms[0] == "He" { m.atoms[a].nb = 2; }
        return Ok(m);
    }
    // central atom: lowest electronegativity among non-H atoms (first on ties), or the one asked for
    let mut ci = None;
    let mut best = f64::MAX;
    for (i, s) in syms.iter().enumerate() {
        if s == "H" { continue; }
        let en = atom_info(s).map(|a| a.1).unwrap_or(9.0);
        if en < best - 1e-9 { best = en; ci = Some(i); }
    }
    if let Some(cs) = central.map(str::trim).filter(|c| !c.is_empty()) {
        if let Some(i) = syms.iter().position(|s| s == cs) { ci = Some(i); }
    }
    let ci = ci.unwrap_or(0);
    let mut m = Mol { atoms: vec![], bonds: vec![], charge, centre: Some(0) };
    let c = m.add(&syms[ci], None, 0.0, 1);
    let mut terms = Vec::new();
    for (i, s) in syms.iter().enumerate() { if i != ci { terms.push(m.add(s, None, 0.0, 1)); } }
    for &t in &terms { m.bonds.push(Bond { a: c, b: t, order: 1 }); }
    let total: i32 = m.atoms.iter().map(|a| a.val).sum::<i32>() - charge;
    let mut rem = total - 2 * terms.len() as i32;
    for &t in &terms {
        let cap = if m.atoms[t].sym == "H" { 0 } else { 6 };
        let give = cap.min(rem.max(0));
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
    let nlp = (m.atoms[c].nb / 2) as usize;
    let (tdirs, ldirs) = directions(flat, terms.len(), nlp);
    for (k, &t) in terms.iter().enumerate() {
        let a = tdirs.get(k).cloned().unwrap_or(45.0 * k as f64);
        m.atoms[t].x = L * a.to_radians().cos();
        m.atoms[t].y = -L * a.to_radians().sin();
    }
    if nlp > 0 && ldirs.len() == nlp { m.atoms[c].forced = Some(ldirs); }
    Ok(m)
}

/// All resonance structures: the multiple bonds moved among equivalent terminal atoms.
fn resonances(m: &Mol) -> Vec<Mol> {
    let Some(c) = m.centre else { return vec![m.clone()] };
    let terms: Vec<usize> = m.bonds.iter().filter(|b| b.a == c).map(|b| b.b).collect();
    let mut out = vec![m.clone()];
    let Some(sym) = terms.iter().find(|&&t| m.bonds.iter().any(|b| b.b == t && b.order == 2)).map(|&t| m.atoms[t].sym.clone()) else { return out };
    let group: Vec<usize> = terms.iter().cloned().filter(|&t| m.atoms[t].sym == sym && m.bonds.iter().any(|b| b.b == t && b.order <= 2)).collect();
    let k = group.iter().filter(|&&t| m.bonds.iter().any(|b| b.b == t && b.order == 2)).count();
    if k == 0 || k >= group.len() { return out; }
    let n = group.len();
    out.clear();
    for mask in 0u32..(1 << n) {
        if mask.count_ones() as usize != k { continue; }
        let mut s = m.clone();
        for (j, &t) in group.iter().enumerate() {
            let ord = if mask & (1 << j) != 0 { 2 } else { 1 };
            if let Some(b) = s.bonds.iter_mut().find(|b| b.a == c && b.b == t) { b.order = ord; }
            s.atoms[t].nb = (8 - 2 * ord).max(0);
            s.atoms[t].forced = None;
        }
        out.push(s);
        if out.len() >= 6 { break; }
    }
    // the generator's own structure first
    if let Some(pos) = out.iter().position(|s| s.bonds.iter().zip(m.bonds.iter()).all(|(a, b)| a.order == b.order)) { out.swap(0, pos); }
    out
}

fn ang_dist(a: f64, b: f64) -> f64 { let d = (a - b).rem_euclid(360.0); d.min(360.0 - d) }

/// "C₂H₅OH" / "c2h5oh" / "ethanol" -> the preset with that formula or name.
fn preset_for_text(t: &str) -> Option<&'static str> {
    let norm = |s: &str| -> String {
        let mut o = s.trim().to_lowercase();
        for (i, c) in "₀₁₂₃₄₅₆₇₈₉".chars().enumerate() { o = o.replace(c, &i.to_string()); }
        o.replace('⁻', "-").replace('−', "-").replace(' ', "")
    };
    let q = norm(t);
    if q.is_empty() { return None; }
    PRESETS.iter().skip(1).find(|p| {
        let (f, n) = p.split_once(',').unwrap_or((p, ""));
        norm(f) == q || norm(n) == q
    }).copied()
}

/// Common names students type, mapped to formulas the parser reads (case-insensitive, trimmed).
const NAMES: [(&str, &str); 52] = [
    ("water", "H2O"), ("ammonia", "NH3"), ("methane", "CH4"), ("carbon dioxide", "CO2"), ("carbon monoxide", "CO"),
    ("hydrogen", "H2"), ("oxygen", "O2"), ("nitrogen", "N2"), ("fluorine", "F2"), ("chlorine", "Cl2"), ("bromine", "Br2"), ("iodine", "I2"),
    ("hydrogen fluoride", "HF"), ("hydrogen chloride", "HCl"), ("hydrogen bromide", "HBr"), ("hydrogen iodide", "HI"),
    ("hydrogen sulfide", "H2S"), ("hydrogen cyanide", "HCN"), ("sulfur dioxide", "SO2"), ("sulfur trioxide", "SO3"),
    ("nitrogen dioxide", "NO2"), ("nitrogen monoxide", "NO"), ("nitric oxide", "NO"), ("ozone", "O3"),
    ("ammonium", "NH4+"), ("hydronium", "H3O+"), ("hydroxide", "OH-"), ("cyanide", "CN-"),
    ("nitrate", "NO3-"), ("nitrite", "NO2-"), ("sulfate", "SO4 2-"), ("sulfite", "SO3 2-"), ("carbonate", "CO3 2-"),
    ("phosphate", "PO4 3-"), ("chlorate", "ClO3-"), ("perchlorate", "ClO4-"), ("hypochlorite", "ClO-"), ("chlorite", "ClO2-"),
    ("boron trifluoride", "BF3"), ("beryllium chloride", "BeCl2"), ("phosphorus trichloride", "PCl3"), ("phosphorus pentachloride", "PCl5"),
    ("sulfur hexafluoride", "SF6"), ("xenon tetrafluoride", "XeF4"), ("xenon difluoride", "XeF2"), ("carbon tetrachloride", "CCl4"),
    ("silicon tetrafluoride", "SiF4"), ("formaldehyde", "CH2O"), ("methanal", "CH2O"), ("silane", "SiH4"), ("phosphine", "PH3"),
    ("hydrogen peroxide", "H2O2"),
];

/// Formula for a typed common name (None when `t` is not one of them).
fn name_formula(t: &str) -> Option<&'static str> {
    let q = t.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    NAMES.iter().find(|n| n.0 == q).map(|n| n.1)
}

/// The correct structure (no changes) for the current params.
fn base(p: &Params, central: Option<&str>) -> Result<Mol, String> {
    let flat = p.s("shape") != "Shaped";
    if p.s("preset") != "None (use the formula)" && !p.s("preset").is_empty() {
        return Ok(preset(p.s("preset")));
    }
    if let Some(pr) = preset_for_text(p.s("formula")) { return Ok(preset(pr)); }
    let f = name_formula(p.s("formula")).unwrap_or(p.s("formula"));
    if let Some(pr) = preset_for_text(f) { return Ok(preset(pr)); }
    from_formula(f, p.b("expand"), flat, central)
}

/// Structures to draw (one, or every resonance structure) with the changes applied.
fn build_all(p: &Params) -> Result<Vec<Mol>, String> {
    let central = Some(p.s("central")).filter(|c| !c.trim().is_empty());
    let m = base(p, central)?;
    let mut list = resonances(&m);
    if p.s("resonance") != "Show all" {
        let k = (p.i("resonance_pick").max(1) as usize - 1).min(list.len() - 1);
        list = vec![list.swap_remove(k)];
    }
    if list.len() == 1 { apply_tweaks(&mut list[0], p.s("tweak")); }
    Ok(list)
}

/// Single structure used by handles and the tests ("b2=3", "l1=2" pairs, "e1=3" electrons).
fn build(p: &Params) -> Result<Mol, String> { build_all(p).map(|mut v| v.swap_remove(0)) }

fn apply_tweaks(m: &mut Mol, tweak: &str) {
    for tok in tweak.split_whitespace() {
        let Some((k, v)) = tok.split_once('=') else { continue };
        let (Some(kind), Ok(i), Ok(n)) = (k.chars().next(), k[1..].parse::<usize>(), v.parse::<i32>()) else { continue };
        match kind {
            'b' => if let Some(b) = m.bonds.get_mut(i) { b.order = n.clamp(0, 3); },
            'l' => if let Some(a) = m.atoms.get_mut(i) { a.nb = (2 * n).clamp(0, 8); a.forced = None; },
            'e' => if let Some(a) = m.atoms.get_mut(i) { a.nb = n.clamp(0, 8); a.forced = None; },
            _ => {}
        }
    }
}

struct Frame { w: f64, h: f64, ox: f64, oy: f64, x0: f64, x1: f64, y0: f64, y1: f64 }

fn frame(m: &Mol) -> Frame {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for a in &m.atoms { x0 = x0.min(a.x); x1 = x1.max(a.x); y0 = y0.min(a.y); y1 = y1.max(a.y); }
    let margin = 56.0;
    let extra = if m.charge != 0 { 16.0 } else { 0.0 };
    let w = (x1 - x0 + 2.0 * margin + 2.0 * extra).max(160.0);
    let h = (y1 - y0 + 2.0 * margin + 2.0 * extra).max(140.0);
    let (ox, oy) = ((w - (x1 - x0)) / 2.0 - x0, (h - (y1 - y0)) / 2.0 - y0);
    Frame { w, h, ox, oy, x0, x1, y0, y1 }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode { Full, Bonds, Skeleton }

/// Draws `m` with its frame's top-left at (dx, dy).
fn draw_mol(d: &mut Doc, m: &Mol, fr: &Frame, dx: f64, dy: f64, p: &Params, mode: Mode) {
    let pos = |i: usize| (m.atoms[i].x + fr.ox + dx, m.atoms[i].y + fr.oy + dy);
    let dots = p.s("bond_style") == "Dots";
    if mode != Mode::Skeleton {
        for b in &m.bonds {
            if b.order <= 0 { continue; }
            let (ax, ay) = pos(b.a);
            let (bx, by) = pos(b.b);
            let (ddx, ddy) = (bx - ax, by - ay);
            let len = (ddx * ddx + ddy * ddy).sqrt().max(1e-6);
            let (ux, uy) = (ddx / len, ddy / len);
            let (nx, ny) = (-uy, ux);
            if dots {
                let (mx, my) = ((ax + bx) / 2.0, (ay + by) / 2.0);
                for k in 0..b.order {
                    let off = (k as f64 - (b.order - 1) as f64 / 2.0) * 7.0;
                    for sgn in [-1.0, 1.0] {
                        d.circle(mx + ux * off + nx * 3.6 * sgn, my + uy * off + ny * 3.6 * sgn, 2.4, INK, INK, 0.5);
                    }
                }
                continue;
            }
            let g = |s: &str| if s == "H" { 10.0 } else { 13.0 };
            let (sa, sb) = (g(&m.atoms[b.a].sym), g(&m.atoms[b.b].sym));
            let offs: Vec<f64> = match b.order { 1 => vec![0.0], 2 => vec![-3.5, 3.5], _ => vec![-5.0, 0.0, 5.0] };
            for o in offs {
                d.line(ax + ux * sa + nx * o, ay + uy * sa + ny * o, bx - ux * sb + nx * o, by - uy * sb + ny * o, INK, 2.0);
            }
        }
    }
    let flat = p.s("shape") != "Shaped";
    for (i, a) in m.atoms.iter().enumerate() {
        let (x, y) = pos(i);
        tx(d, x, y + 8.0, 24.0, "middle", INK, false, &a.sym);
        if mode == Mode::Full && p.b("lone") {
            let mut dirs: Vec<f64> = Vec::new();
            for b in m.bonds.iter().filter(|b| b.order > 0 || dots) {
                let other = if b.a == i { Some(b.b) } else if b.b == i { Some(b.a) } else { None };
                if let Some(o) = other {
                    let (ox2, oy2) = pos(o);
                    dirs.push((-(oy2 - y)).atan2(ox2 - x).to_degrees().rem_euclid(360.0));
                }
            }
            let pairs = (a.nb / 2).max(0) as usize;
            let single = a.nb % 2 == 1;
            let slots = pairs + usize::from(single);
            let mut chosen: Vec<f64> = Vec::new();
            match &a.forced {
                Some(f) if f.len() == pairs => chosen = f.clone(),
                _ => {
                    let mut all = dirs.clone();
                    for _ in 0..slots {
                        let mut best = (f64::MIN, 0.0);
                        let step = if flat { 45.0 } else { 15.0 };
                        for k in 0..(360.0 / step) as i64 {
                            let ang = k as f64 * step;
                            let mind = all.iter().map(|&o| ang_dist(ang, o)).fold(360.0, f64::min);
                            let bonus = if (ang % 90.0).abs() < 1e-6 { if flat { 40.0 } else { 1.0 } } else if (ang % 45.0).abs() < 1e-6 { 0.5 } else { 0.0 };
                            let score = mind.min(if flat { 90.0 } else { 120.0 }) + bonus;
                            if score > best.0 + 1e-9 { best = (score, ang); }
                        }
                        chosen.push(best.1);
                        all.push(best.1);
                    }
                }
            }
            for (k, &ang) in chosen.iter().enumerate() {
                let r = if a.sym.len() > 1 { 25.0 } else { 19.0 };
                let (c, s) = (ang.to_radians().cos(), ang.to_radians().sin());
                let (px, py) = (x + r * c, y - r * s);
                if k < pairs {
                    for sgn in [-1.0, 1.0] { d.circle(px + s * 3.8 * sgn, py + c * 3.8 * sgn, 2.4, INK, INK, 0.5); }
                } else {
                    d.circle(px, py, 2.4, INK, INK, 0.5);
                }
            }
        }
        if mode == Mode::Full && p.b("formal") {
            let fc = m.formal(i);
            if fc != 0 {
                let t = format!("{}{}", if fc > 0 { "+" } else { "−" }, fc.abs());
                tx(d, x + 16.0, y - 14.0, 12.0, "start", INK, false, &t);
            }
        }
    }
    if m.charge != 0 {
        let (bx0, bx1, by0, by1) = (fr.x0 + fr.ox + dx - 40.0, fr.x1 + fr.ox + dx + 40.0, fr.y0 + fr.oy + dy - 40.0, fr.y1 + fr.oy + dy + 40.0);
        d.polyline(&[(bx0 + 10.0, by0), (bx0, by0), (bx0, by1), (bx0 + 10.0, by1)], INK, 2.0);
        d.polyline(&[(bx1 - 10.0, by0), (bx1, by0), (bx1, by1), (bx1 - 10.0, by1)], INK, 2.0);
        let c = m.charge.abs();
        let t = format!("{}{}", if c > 1 { c.to_string() } else { String::new() }, if m.charge > 0 { "+" } else { "−" });
        tx(d, bx1 + 4.0, by0 + 6.0, 17.0, "start", INK, true, &t);
    }
}

/// Formula with subscript digits for messages ("H2O" -> "H₂O").
fn pretty(f: &str) -> String {
    let mut out = String::new();
    let mut prev_alpha = false;
    for ch in f.trim().chars() {
        if ch.is_ascii_digit() && prev_alpha { out.push_str(&sub(ch.to_digit(10).unwrap_or(0))); continue; }
        prev_alpha = ch.is_ascii_alphabetic() || (prev_alpha && ch.is_ascii_digit());
        out.push(ch);
    }
    out
}

/// What is wrong with the drawn structure `m` (compared with electron counting rules).
fn mistakes(p: &Params, m: &Mol, correct: &Mol) -> Vec<String> {
    let mut v = Vec::new();
    let name = if p.s("preset") != "None (use the formula)" && !p.s("preset").is_empty() { p.s("preset").split(',').next().unwrap_or("").to_string() } else { pretty(name_formula(p.s("formula")).unwrap_or(p.s("formula"))) };
    if let (Some(a), Some(b)) = (m.centre, correct.centre) {
        if m.atoms[a].sym != correct.atoms[b].sym { v.push(format!("The central atom should be {}, not {}.", correct.atoms[b].sym, m.atoms[a].sym)); }
    }
    let (have, want) = (m.electrons(), correct.electrons());
    if have != want { v.push(format!("It shows {have} valence electrons, but {name} has {want}.")); }
    let mut seen = std::collections::BTreeSet::new();
    for (i, a) in m.atoms.iter().enumerate() {
        let around = a.nb + 2 * m.bond_sum(i);
        let msg = if a.sym == "H" {
            if around > 2 { Some(format!("An H atom has {around} electrons around it, but it only has room for 2.")) }
            else if around < 2 { Some("An H atom is missing its bond.".to_string()) } else { None }
        } else {
            let period = atom_info(&a.sym).map(|x| x.2).unwrap_or(2);
            let deficient = matches!(a.sym.as_str(), "Be" | "B" | "Al");
            if around > 8 && period <= 2 { Some(format!("{} has {around} electrons around it, but it can’t have more than 8.", a.sym)) }
            else if around < 8 && !deficient { Some(format!("{} has only {around} electrons around it, not a full octet.", a.sym)) } else { None }
        };
        if let Some(t) = msg { if seen.insert(t.clone()) { v.push(t); } }
    }
    v
}

/// Layout of everything drawn: structures side by side (resonance), optional answer below.
struct Lay { mols: Vec<(Mol, Frame, f64, f64)>, w: f64, h: f64, answer: Option<(Mol, Frame, f64, f64)> }

fn layout(p: &Params) -> Result<Lay, String> {
    let mols = build_all(p)?;
    let gap = 60.0;
    let mut x = 0.0;
    let mut placed = Vec::new();
    let mut hmax: f64 = 0.0;
    for m in mols {
        let fr = frame(&m);
        hmax = hmax.max(fr.h);
        let w = fr.w;
        placed.push((m, fr, x, 0.0));
        x += w + gap;
    }
    let mut w = x - gap;
    for pl in placed.iter_mut() { pl.3 = (hmax - pl.1.h) / 2.0; }
    let mut h = hmax;
    let mut answer = None;
    let question = match p.s("question") { "Bonds only" => Mode::Bonds, "Skeleton" => Mode::Skeleton, _ => Mode::Full };
    if p.b("answer") && question != Mode::Full && placed.len() == 1 {
        let m = placed[0].0.clone();
        let fr = frame(&m);
        let ax = (w - fr.w) / 2.0;
        let ay = h + 40.0;
        h = ay + fr.h;
        w = w.max(fr.w);
        answer = Some((m, fr, ax, ay));
    }
    // room for the "Mistakes" list under a changed structure
    let changed = !p.s("tweak").trim().is_empty() || !p.s("central").trim().is_empty();
    let target = if p.b("answer") && changed { 520.0f64.max(w) } else { w.max(160.0) };
    let shift = (target - w) / 2.0;
    if shift > 0.0 {
        for pl in placed.iter_mut() { pl.2 += shift; }
        if let Some(a) = answer.as_mut() { a.2 += shift; }
    }
    Ok(Lay { mols: placed, w: target, h, answer })
}

fn render(p: &Params) -> String {
    let lay = match layout(p) {
        Ok(l) => l,
        Err(e) => {
            let mut d = Doc::new(420.0, 120.0);
            tx(&mut d, 210.0, 66.0, 15.0, "middle", "#b03030", false, &e);
            return d.finish();
        }
    };
    let question = match p.s("question") { "Bonds only" => Mode::Bonds, "Skeleton" => Mode::Skeleton, _ => Mode::Full };
    let mut d = Doc::new(lay.w, lay.h);
    let n = lay.mols.len();
    for (k, (m, fr, x, y)) in lay.mols.iter().enumerate() {
        draw_mol(&mut d, m, fr, *x, *y, p, question);
        if k + 1 < n {
            let ax = x + fr.w + 8.0;
            let ay = y + fr.h / 2.0;
            d.arrow(ax + 22.0, ay, ax + 44.0, ay, INK, 1.4);
            d.arrow(ax + 22.0, ay, ax, ay, INK, 1.4);
        }
    }
    if let Some((m, fr, x, y)) = &lay.answer {
        tx(&mut d, lay.w / 2.0, y - 8.0, 20.0, "middle", INK, true, "Answer");
        draw_mol(&mut d, m, fr, *x, *y, p, Mode::Full);
    }
    let mut foot = Vec::new();
    let changed = !p.s("tweak").trim().is_empty() || !p.s("central").trim().is_empty();
    if p.b("answer") && changed && lay.mols.len() == 1 {
        if let Ok(correct) = base(p, None) {
            let ms = mistakes(p, &lay.mols[0].0, &correct);
            if !ms.is_empty() {
                foot.push("Mistakes".to_string());
                for t in ms { foot.push(format!("• {t}")); }
            }
        }
    }
    wrap(p, d.finish(), &foot)
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
    let Ok(lay) = layout(p) else { return Some(Vec::new()) };
    if lay.mols.len() != 1 { return Some(Vec::new()); }
    let (m, fr, dx, dy) = &lay.mols[0];
    let pos = |i: usize| (m.atoms[i].x + fr.ox + dx, m.atoms[i].y + fr.oy + dy);
    let mut v = Vec::new();
    for (i, b) in m.bonds.iter().enumerate() {
        let (a, c) = (pos(b.a), pos(b.b));
        let order = b.order;
        v.push(Handle::zone(((a.0 + c.0) / 2.0, (a.1 + c.1) / 2.0), (22.0, 22.0), "Click to cycle the bond: single, double, triple, none", move |q| {
            set_tweak(q, &format!("b{i}"), (order + 1) % 4);
        }));
    }
    if p.b("lone") {
        for (i, a) in m.atoms.iter().enumerate() {
            let nb = a.nb;
            let c = pos(i);
            v.push(Handle::zone((c.0, c.1 - 2.0), (30.0, 30.0), "Click to add lone electrons to this atom (wraps back to none)", move |q| {
                set_tweak(q, &format!("e{i}"), if nb >= 8 { 0 } else { nb + 1 });
            }));
        }
    }
    if p.s("preset") == "None (use the formula)" || p.s("preset").is_empty() {
        v.push(Handle::label((lay.w / 2.0, 14.0), (150.0, 24.0), "formula", "Double-click to edit the formula"));
    }
    let _ = (fr.x0, fr.x1, fr.y0, fr.y1, fr.h);
    Some(v)
}

// ---------------------------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------------------------

use crate::figures::{Example, Value};
fn ex(name: &'static str, params: Vec<(&'static str, Value)>) -> Example { Example { name, params } }
fn sv(v: &str) -> Value { Value::S(v.into()) }
fn bv(v: bool) -> Value { Value::B(v) }

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    if id != "lewis-structures" { return None; }
    Some(vec![
        ex("Lewis structure of CO2", vec![("formula", sv("CO2"))]),
        ex("Lewis structure of H2O (shaped)", vec![("formula", sv("H2O")), ("shape", sv("Shaped"))]),
        ex("Lewis structure of NH3", vec![("formula", sv("NH3"))]),
        ex("Lewis dot structure of CH4", vec![("formula", sv("CH4")), ("bond_style", sv("Dots"))]),
        ex("Lewis structure of HCN", vec![("formula", sv("HCN"))]),
        ex("Lewis dot diagram of the nitride ion N3-", vec![("formula", sv("N 3-"))]),
        ex("Resonance structures of nitrate NO3-", vec![("formula", sv("NO3-")), ("resonance", sv("Show all")), ("formal", bv(true))]),
        ex("Lewis structure of sulfate SO4 2-", vec![("formula", sv("SO4 2-")), ("formal", bv(true))]),
        ex("Find the mistake: water", vec![("formula", sv("H2O")), ("tweak", sv("b0=2")), ("answer", bv(true))]),
        ex("Skeleton for students: ethanol", vec![("formula", sv("ethanol")), ("question", sv("Skeleton")), ("answer", bv(true))]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::Params;

    #[test]
    fn common_names_build() {
        let defs = params();
        for name in ["water", "Ammonia", " methane ", "ethane", "ethene", "ethyne", "carbon dioxide", "Carbon  Monoxide", "hydrogen",
            "oxygen", "nitrogen", "chlorine", "hydrogen chloride", "hydrogen fluoride", "hydrogen sulfide", "sulfur dioxide",
            "sulfur trioxide", "nitrogen dioxide", "ozone", "ammonium", "hydroxide", "nitrate", "nitrite", "sulfate", "carbonate",
            "phosphate", "ethanol", "hydronium", "cyanide", "hydrogen peroxide"] {
            let mut p = Params::defaults(&defs);
            p.set_s("formula", name);
            let m = base(&p, None).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(m.atoms.len() >= 2, "{name}");
            assert!(render(&p).contains("<svg"), "{name}");
        }
        let mut p = Params::defaults(&defs);
        p.set_s("formula", "ammonia");
        let m = base(&p, None).unwrap();
        assert_eq!(m.atoms.iter().filter(|a| a.sym == "H").count(), 3);
        p.set_s("formula", "sulfate");
        assert_eq!(base(&p, None).unwrap().charge, -2);
    }
}
