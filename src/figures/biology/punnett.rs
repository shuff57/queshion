//! Punnett square generator: monohybrid, dihybrid and X-linked crosses with complete,
//! incomplete or co-dominance, optional shading and genotype/phenotype ratios.
//!
//! Parents syntax (the "Parents" text): two genotypes separated by "x" (or the multiplication
//! sign). Monohybrid `Tt x Tt`; dihybrid `TtRr x TtRr` (one letter per gene, capital = dominant);
//! X-linked `XCXc x XCY` (X followed by the allele letter, or Y).
//! Phenotype names: comma-separated per gene, genes separated by `;`. Complete dominance wants
//! `dominant, recessive`; incomplete/codominance wants `AA, Aa, aa` or just two names (the
//! heterozygote is then named automatically).
use super::*;
use std::collections::BTreeMap;

/// Grid geometry of the last render (for interaction handles).
#[derive(Clone, Debug, Default)]
pub struct Layout { pub gx: f64, pub p1_y: f64, pub gy: f64, pub cell: f64, pub nc: usize, pub nr: usize, pub ox: f64, pub pb_y: f64 }

thread_local! {
    static LAST: std::cell::RefCell<Option<Layout>> = const { std::cell::RefCell::new(None) };
}

/// Renders and returns the layout (None when the parents cannot be read).
pub fn layout_of(p: &Params) -> Option<Layout> {
    LAST.with(|l| *l.borrow_mut() = None);
    let _ = render(p);
    LAST.with(|l| l.borrow().clone())
}

pub const SHADES: [&str; 6] = ["#bfdbfe", "#fde68a", "#bbf7d0", "#fecaca", "#e9d5ff", "#fed7aa"];

/// Cross kind as an index: 0 mono, 1 di, 2 x-linked.
pub fn cross_of(p: &Params) -> u8 { match p.s("cross") { "Dihybrid" => 1, "X-linked" => 2, _ => 0 } }

/// Toggle the case of one allele (`which` 0/1) of gene `gene` in parent `parent` (0 = top, 1 = side)
/// and write the parents string back. Genotype cycle per click: AA -> Aa -> aa -> AA.
pub fn cycle_gene(p: &mut Params, parent: usize, gene: usize) {
    let cross = match p.s("cross") { "Dihybrid" => Cross::Di, "X-linked" => Cross::XLinked, _ => Cross::Mono };
    let Some((a, b)) = split_parents(p.s("parents")) else { return };
    REG.with(|r| r.borrow_mut().clear());
    XLINK.with(|x| x.set(cross == Cross::XLinked));
    let dom = p.s("dominance").to_string();
    let (Ok(mut pa), Ok(mut pb)) = (parse_parent_dom(&a, cross, &dom), parse_parent_dom(&b, cross, &dom)) else { return };
    let tgt = if parent == 0 { &mut pa } else { &mut pb };
    let Some(g) = tgt.get_mut(gene) else { return };
    if g.iter().any(|c| is_sup(*c)) { return; }
    let flip = |c: char| if c.is_uppercase() { c.to_lowercase().next().unwrap() } else { c.to_uppercase().next().unwrap() };
    if cross == Cross::XLinked && g[1] == 'Y' {
        g[0] = flip(g[0]);
    } else {
        let up = g.iter().filter(|c| c.is_uppercase()).count();
        match up {
            2 => g[1] = flip(g[1]),
            1 => { if g[0].is_uppercase() { g[0] = flip(g[0]); } else { g[1] = flip(g[1]); } if g.iter().filter(|c| c.is_uppercase()).count() == 1 { g.swap(0, 1); } }
            _ => { g[0] = flip(g[0]); g[1] = flip(g[1]); }
        }
        // keep the dominant allele first for readability
        if g[1].is_uppercase() && !g[0].is_uppercase() { g.swap(0, 1); }
    }
    let fmt = |v: &[Pair]| -> String {
        if cross == Cross::XLinked {
            let g = v[0];
            if g[1] == 'Y' { format!("X{}Y", g[0]) } else { format!("X{}X{}", g[0], g[1]) }
        } else {
            v.iter().map(|g| format!("{}{}", allele_text(g[0]), allele_text(g[1]))).collect::<Vec<_>>().join("")
        }
    };
    let s = format!("{} x {}", fmt(&pa), fmt(&pb));
    p.set_s("parents", &s);
}

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("cross", "Cross", &["Monohybrid", "Dihybrid", "X-linked"], "Monohybrid"),
        ParamDef::choice("dominance", "Dominance", &["Complete", "Incomplete", "Codominance"], "Complete"),
        ParamDef::text("parents", "Parents (first goes across the top), e.g. Tt x Tt", "Tt x Tt"),
        ParamDef::text("phenotypes", "Phenotype names, e.g. tall, short (genes split by ;)", "tall, short"),
        ParamDef::bool("filled", "Fill in the squares (off = blank for students)", true),
        ParamDef::choice("shading", "Shading", &["None", "Each phenotype, with a key", "First phenotype only", "Last phenotype only"], "None"),
        ParamDef::bool("geno_ratio", "Show genotype ratio", true),
        ParamDef::bool("pheno_ratio", "Show phenotype ratio", true),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::text("question", "Question (blank = none)", ""),
        ParamDef::color("shade_color", "Shade colour (first / last phenotype only)", &SHADES, SHADES[0]),
        ParamDef::bool("outline_heavy", "Heavy outer border", true),
        // ---- added for parity with the reference controls
        ParamDef::choice("parent_labels", "Parent labels", &["Genotype", "♀ ♂", "Mother", "None"], "Genotype"),
        ParamDef::bool("blank_parents", "Leave the parents' genotypes blank", false),
        ParamDef::bool("blank_gametes", "Leave the gametes blank", false),
        ParamDef::choice("cells", "Offspring cells", &["Filled in", "Blank", "Some blank"], "Filled in"),
        ParamDef::list("blank_cells", "Blank cells for Some blank (numbers, row by row from 1)", "1, 4"),
        ParamDef::bool("cell_phenotype", "Write each phenotype under its genotype", false),
        ParamDef::choice("shade_style", "Shading look", &["Colors", "Gray and patterns"], "Colors"),
        ParamDef::choice("geno_ratio_style", "Genotypes", &["Shown", "Blank line", "None"], "Shown"),
        ParamDef::choice("pheno_ratio_style", "Phenotypes", &["Shown", "Blank line", "None"], "Shown"),
        ParamDef::choice("ratio_form", "Ratios written as", &["Ratio", "Percent", "Fraction"], "Ratio"),
        ParamDef::bool("sex_split", "X-linked: list daughters' and sons' phenotypes separately", false),
    ]
}

/// Toggle offspring cell `idx` (0-based, row by row) in the Some-blank list.
pub fn toggle_blank_cell(p: &mut Params, idx: usize) {
    let mut v: Vec<usize> = p.s("blank_cells").split([',', ';', ' ']).filter_map(|t| t.trim().parse::<usize>().ok()).collect();
    let n = idx + 1;
    if let Some(i) = v.iter().position(|x| *x == n) { v.remove(i); } else { v.push(n); v.sort(); }
    p.set_s("blank_cells", &v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "));
    p.set_s("cells", "Some blank");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cross { Mono, Di, XLinked }

/// One locus: two allele characters (`Y` marks the Y chromosome in X-linked crosses).
type Pair = [char; 2];

fn skip(c: char) -> bool { c.is_whitespace() || c == '^' || c == '{' || c == '}' || c == '_' }

// Alleles with a superscript (C^R, I^A) are interned as private-use chars so the rest of the
// code can keep working with `char` pairs. Plain letters stay themselves.
thread_local! {
    static REG: std::cell::RefCell<Vec<(char, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}
const PUA: u32 = 0xE000;

thread_local! {
    /// True while working on an X-linked cross, where `Y` is the Y chromosome (not a gene letter).
    static XLINK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn is_ychr(c: char) -> bool { c == 'Y' && XLINK.with(|x| x.get()) }

fn intern(base: char, sup: &str) -> char {
    REG.with(|r| {
        let mut r = r.borrow_mut();
        if let Some(i) = r.iter().position(|(b, s)| *b == base && s == sup) {
            return char::from_u32(PUA + i as u32).unwrap();
        }
        r.push((base, sup.to_string()));
        char::from_u32(PUA + (r.len() - 1) as u32).unwrap()
    })
}

/// (base letter, superscript) of an allele char.
fn allele(c: char) -> (char, String) {
    let u = c as u32;
    if (PUA..PUA + 4096).contains(&u) {
        if let Some(v) = REG.with(|r| r.borrow().get((u - PUA) as usize).cloned()) { return v; }
    }
    (c, String::new())
}
fn is_sup(c: char) -> bool { !allele(c).1.is_empty() }
fn reg_idx(c: char) -> usize { if is_sup(c) { (c as u32 - PUA) as usize } else { 99 } }
/// Dominant-looking allele: capital base letter (for superscript alleles too).
fn is_dom(c: char) -> bool { !is_ychr(c) && allele(c).0.is_uppercase() }
/// Allele as typed text, e.g. "C^R", "i".
fn allele_text(c: char) -> String {
    let (b, s) = allele(c);
    if s.is_empty() { b.to_string() } else if s.chars().count() == 1 { format!("{b}^{s}") } else { format!("{b}^{{{s}}}") }
}

/// Split one genotype into allele tokens. `implicit` reads a letter or digit typed straight
/// after the gene's letter as a superscript (CR -> C^R), as for incomplete/co-dominance.
fn tokens(s: &str, implicit: bool) -> Vec<char> {
    let cs: Vec<char> = s.chars().filter(|c| !c.is_whitespace() && *c != '+').collect();
    let gene = cs.iter().find(|c| c.is_alphabetic()).map(|c| c.to_lowercase().next().unwrap());
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if !c.is_alphabetic() { i += 1; continue; }
        i += 1;
        let mut sup = String::new();
        loop {
            match cs.get(i) {
                Some('^') | Some('_') => {
                    i += 1;
                    if cs.get(i) == Some(&'{') {
                        i += 1;
                        while i < cs.len() && cs[i] != '}' { sup.push(cs[i]); i += 1; }
                        i += 1;
                    } else if let Some(n) = cs.get(i) { sup.push(*n); i += 1; }
                }
                Some(n) if implicit && n.is_alphanumeric() && Some(n.to_lowercase().next().unwrap()) != gene => { sup.push(*n); i += 1; }
                _ => break,
            }
        }
        out.push(if sup.is_empty() { c } else { intern(c, &sup) });
    }
    out
}

fn parse_parent_dom(s: &str, cross: Cross, dom: &str) -> Result<Vec<Pair>, String> {
    if cross == Cross::XLinked { return parse_parent(s, cross); }
    let implicit = cross == Cross::Mono && dom != "Complete";
    let has_sup_syntax = s.contains('^') || s.contains('_') || implicit;
    if !has_sup_syntax { return parse_parent(s, cross); }
    let al = tokens(s, implicit);
    let genes = if cross == Cross::Mono { 1 } else { 2 };
    if al.len() != genes * 2 { return Err(format!("need {} alleles in \"{s}\"", genes * 2)); }
    let mut v = Vec::new();
    for g in 0..genes {
        let (a, b) = (al[g * 2], al[g * 2 + 1]);
        let (ba, bb) = (allele(a).0, allele(b).0);
        if ba.to_lowercase().ne(bb.to_lowercase()) { return Err(format!("alleles {} and {} are not the same gene", allele_text(a), allele_text(b))); }
        v.push([a, b]);
    }
    Ok(v)
}

fn parse_parent(s: &str, cross: Cross) -> Result<Vec<Pair>, String> {
    let cs: Vec<char> = s.chars().filter(|c| !skip(*c)).collect();
    match cross {
        Cross::XLinked => {
            let mut al = Vec::new();
            let mut i = 0;
            while i < cs.len() {
                match cs[i] {
                    'Y' => { al.push('Y'); i += 1; }
                    'X' => match cs.get(i + 1) {
                        Some(c) if c.is_alphabetic() && *c != 'X' && *c != 'Y' => { al.push(*c); i += 2; }
                        _ => return Err(format!("X needs an allele letter in \"{s}\" (e.g. XC)")),
                    },
                    c => return Err(format!("unexpected \"{c}\" in \"{s}\"")),
                }
            }
            if al.len() != 2 || al[0] == 'Y' { return Err(format!("need two sex chromosomes in \"{s}\"")); }
            Ok(vec![[al[0], al[1]]])
        }
        _ => {
            let genes = if cross == Cross::Mono { 1 } else { 2 };
            if cs.len() != genes * 2 || !cs.iter().all(|c| c.is_alphabetic()) {
                return Err(format!("need {} letters in \"{s}\"", genes * 2));
            }
            let mut v = Vec::new();
            for g in 0..genes {
                let (a, b) = (cs[g * 2], cs[g * 2 + 1]);
                if a.to_lowercase().ne(b.to_lowercase()) { return Err(format!("letters {a}{b} are not the same gene")); }
                v.push([a, b]);
            }
            Ok(v)
        }
    }
}

fn split_parents(s: &str) -> Option<(String, String)> {
    if let Some((a, b)) = s.split_once('×').or_else(|| s.split_once('*')) {
        return Some((a.into(), b.into()));
    }
    if let Some((a, b)) = s.split_once(" x ") { return Some((a.into(), b.into())); }
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 2 { return Some((parts[0].into(), parts[1].into())); }
    None
}

fn gametes(parent: &[Pair], cross: Cross) -> Vec<Vec<char>> {
    match cross {
        Cross::Di => {
            let (a, b) = (parent[0], parent[1]);
            vec![vec![a[0], b[0]], vec![a[0], b[1]], vec![a[1], b[0]], vec![a[1], b[1]]]
        }
        _ => vec![vec![parent[0][0]], vec![parent[0][1]]],
    }
}

/// Dominant first, then lower case, Y last.
fn order(a: char, b: char) -> Pair {
    let rank = |c: char| if is_ychr(c) { 3000 } else if is_dom(c) { reg_idx(c).min(99) } else { 1000 + reg_idx(c).min(99) };
    if rank(a) <= rank(b) { [a, b] } else { [b, a] }
}

fn geno_parts(genes: &[Pair], cross: Cross) -> Vec<Part> {
    let mut v = Vec::new();
    for g in genes {
        for &c in g {
            if cross == Cross::XLinked {
                if c == 'Y' { v.push(Part::geno("Y", false)); } else { v.push(Part::geno("X", false)); v.push(Part::geno(&c.to_string(), true)); }
            } else {
                let (b, sp) = allele(c);
                v.push(Part::geno(&b.to_string(), false));
                if !sp.is_empty() { v.push(Part::geno(&sp, true)); }
            }
        }
    }
    v
}

fn gamete_parts(g: &[char], cross: Cross) -> Vec<Part> {
    let pairs: Vec<Pair> = g.iter().map(|c| [*c, '\0']).collect();
    let mut v = Vec::new();
    for p in pairs {
        let c = p[0];
        if cross == Cross::XLinked {
            if c == 'Y' { v.push(Part::geno("Y", false)); } else { v.push(Part::geno("X", false)); v.push(Part::geno(&c.to_string(), true)); }
        } else {
            let (b, sp) = allele(c);
            v.push(Part::geno(&b.to_string(), false));
            if !sp.is_empty() { v.push(Part::geno(&sp, true)); }
        }
    }
    v
}

struct Child {
    genes: Vec<Pair>,
    male: bool,
}

fn key_of(c: &Child) -> String { c.genes.iter().map(|g| format!("{}{}", g[0], g[1])).collect() }

/// Phenotype class index per gene: complete 0 dominant / 1 recessive; otherwise 0 / 1 het / 2 recessive.
/// Genes with superscript alleles (C^R C^W, I^A I^B i) get an index from the expressed alleles,
/// so red < pink < white and A < AB < B < O sort like the reference.
fn class_idx(g: Pair, dom: &str) -> usize {
    let alleles: Vec<char> = g.iter().copied().filter(|c| !is_ychr(*c)).collect();
    if multi_allele(g) {
        let (doms, rec) = expressed(&alleles);
        if doms.is_empty() { return 100 + rec.map(reg_idx).unwrap_or(99); }
        // two different superscripted capitals (I^A I^B) always both show
        return 2 * reg_idx(doms[0]) + (doms.len() > 1) as usize;
    }
    let up = alleles.iter().filter(|c| is_dom(**c)).count();
    if dom == "Complete" {
        if up > 0 { 0 } else { 1 }
    } else if alleles.len() == 2 {
        2 - up
    } else if up == 1 { 0 } else { 2 }
}

/// Dominant (capital) alleles present, sorted and unique, and the recessive allele.
fn expressed(alleles: &[char]) -> (Vec<char>, Option<char>) {
    let mut doms: Vec<char> = alleles.iter().copied().filter(|c| is_dom(*c)).collect();
    doms.sort_by_key(|c| reg_idx(*c));
    doms.dedup();
    (doms, alleles.iter().copied().find(|c| !is_dom(*c)))
}

/// Normalised key for phenotype-map lookups: drop markup, sort the characters.
fn norm(s: &str) -> String {
    let mut v: Vec<char> = s.chars().filter(|c| !matches!(c, ' ' | '^' | '{' | '}' | '+' | '_')).collect();
    v.sort();
    v.into_iter().collect()
}

/// `T_: tall; tt: short` or `R:round;r:wrinkled` or `I^A:type A;I^A+I^B:type AB` style names.
fn parse_map(s: &str) -> Vec<(String, String)> {
    if !s.contains(':') { return vec![]; }
    let mut out: Vec<(String, String)> = Vec::new();
    for piece in s.split([';', ',', '\n']) {
        if let Some((k, v)) = piece.split_once(':') {
            out.push((norm(k), v.trim().to_string()));
        } else if let Some(last) = out.last_mut() {
            if !piece.trim().is_empty() { last.1.push_str(", "); last.1.push_str(piece.trim()); }
        }
    }
    out
}

fn map_name(map: &[(String, String)], g: Pair, dom: &str, xlinked: bool) -> Option<String> {
    if map.is_empty() { return None; }
    let alleles: Vec<char> = g.iter().copied().filter(|c| !is_ychr(*c)).collect();
    let mut cands: Vec<String> = Vec::new();
    let txt = |c: char| -> String { if xlinked { format!("X{}", allele_text(c)) } else { allele_text(c) } };
    let (doms, rec) = expressed(&alleles);
    if alleles.iter().any(|c| is_sup(*c)) || xlinked {
        if dom == "Complete" || xlinked {
            if let Some(d) = doms.first() { cands.push(txt(*d)); } else if let Some(r) = rec { cands.push(txt(r)); }
        } else if doms.is_empty() {
            if let Some(r) = rec { cands.push(txt(r)); }
        } else {
            cands.push(doms.iter().map(|c| txt(*c)).collect::<Vec<_>>().join(""));
        }
        cands.push(g.iter().map(|c| if is_ychr(*c) { "Y".to_string() } else { txt(*c) }).collect::<Vec<_>>().join(""));
    } else {
        cands.push(alleles.iter().map(|c| allele_text(*c)).collect());
        if dom == "Complete" {
            if let Some(d) = doms.first() { cands.push(allele_text(*d)); } else if let Some(r) = rec { cands.push(allele_text(r)); cands.push(format!("{0}{0}", allele_text(r))); }
        } else if alleles.len() == 2 && alleles[0] == alleles[1] {
            cands.push(allele_text(alleles[0]));
        }
    }
    for c in cands {
        let k = norm(&c);
        if let Some((_, v)) = map.iter().find(|(mk, _)| *mk == k) { return Some(v.clone()); }
    }
    None
}

/// A gene whose alleles are written with superscripts somewhere in this cross (C^R/C^W,
/// I^A/I^B/i): its genotypes are named from the expressed alleles, even a plain `ii`.
fn multi_allele(g: Pair) -> bool {
    if g.iter().any(|c| is_sup(*c)) { return true; }
    let Some(base) = g.iter().find(|c| !is_ychr(**c)).map(|c| allele(*c).0) else { return false };
    REG.with(|r| r.borrow().iter().any(|(b, _)| b.eq_ignore_ascii_case(&base)))
}

/// The ABO blood-group set: superscripted I alleles A and/or B, with plain i as recessive.
fn is_abo(g: Pair) -> bool {
    let Some(base) = g.iter().find(|c| !is_ychr(**c)).map(|c| allele(*c).0) else { return false };
    if !base.eq_ignore_ascii_case(&'i') { return false; }
    REG.with(|r| {
        let r = r.borrow();
        let sups: Vec<&String> = r.iter().filter(|(b, _)| b.eq_ignore_ascii_case(&'i')).map(|(_, s)| s).collect();
        !sups.is_empty() && sups.iter().all(|s| s.as_str() == "A" || s.as_str() == "B")
    })
}

fn pheno_name(names: &[Vec<String>], gi: usize, idx: usize, g: Pair, dom: &str) -> String {
    if multi_allele(g) {
        let alleles: Vec<char> = g.iter().copied().filter(|c| !is_ychr(*c)).collect();
        let (doms, rec) = expressed(&alleles);
        if is_abo(g) {
            // blood types: I^A I^B = AB, I^A i = A, I^B i = B, ii = O
            if doms.is_empty() { return "O".into(); }
            return doms.iter().map(|c| allele(*c).1).collect::<Vec<_>>().join("");
        }
        let gene_names: &[String] = names.get(gi).map(|v| v.as_slice()).unwrap_or(&[]);
        if gene_names.len() >= 2 && rec.is_none() && dom != "Complete" {
            // two co/incompletely dominant alleles named positionally: first, blend, second
            let first = REG.with(|r| r.borrow().iter().position(|(b, _)| b.eq_ignore_ascii_case(&allele(doms[0]).0)).unwrap_or(0));
            let i3 = if doms.len() > 1 { 1 } else if reg_idx(doms[0]) == first { 0 } else { 2 };
            if gene_names.len() >= 3 { return gene_names[i3].clone(); }
            return match i3 {
                0 => gene_names[0].clone(),
                2 => gene_names[1].clone(),
                _ => if dom == "Codominance" { format!("{} and {}", gene_names[0], gene_names[1]) } else { "intermediate".into() },
            };
        }
        let t: Vec<String> = if doms.is_empty() { rec.into_iter().map(allele_text).collect() } else { doms.iter().map(|c| allele_text(*c)).collect() };
        return t.join(" and ").replace('^', "");
    }
    let gene_names: &[String] = names.get(gi).map(|v| v.as_slice()).unwrap_or(&[]);
    let letter_up = g.iter().find(|c| c.is_alphabetic() && !is_ychr(**c)).copied().unwrap_or('A');
    let (u, l) = (letter_up.to_uppercase().next().unwrap(), letter_up.to_lowercase().next().unwrap());
    if dom == "Complete" {
        if gene_names.len() >= 2 { return gene_names[idx.min(1)].clone(); }
        return if idx == 0 { format!("{u}_") } else { format!("{l}{l}") };
    }
    if gene_names.len() >= 3 { return gene_names[idx.min(2)].clone(); }
    if gene_names.len() == 2 {
        return match idx {
            0 => gene_names[0].clone(),
            2 => gene_names[1].clone(),
            _ => if dom == "Codominance" { format!("{} and {}", gene_names[0], gene_names[1]) } else { "intermediate".into() },
        };
    }
    match idx { 0 => format!("{u}{u}"), 1 => format!("{u}{l}"), _ => format!("{l}{l}") }
}

/// Photocopy-safe fills: gray, hatching, dots, cross-hatching, light gray; the last phenotype is white.
fn pattern_fill(ci: usize, n: usize) -> &'static str {
    if ci + 1 == n && n > 1 { return "#ffffff"; }
    ["#d9d9d9", "url(#pq-hatch)", "url(#pq-dots)", "url(#pq-cross)", "#efefef", "url(#pq-vert)"][ci % 6]
}

const PATTERN_DEFS: &str = r##"<defs><pattern id="pq-hatch" width="10" height="10" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="10" height="10" fill="#fff"/><line x1="0" y1="0" x2="0" y2="10" stroke="#888" stroke-width="1.2"/></pattern><pattern id="pq-dots" width="10" height="10" patternUnits="userSpaceOnUse"><rect width="10" height="10" fill="#fff"/><circle cx="5" cy="5" r="1.5" fill="#555"/></pattern><pattern id="pq-cross" width="10" height="10" patternUnits="userSpaceOnUse"><rect width="10" height="10" fill="#fff"/><path d="M0 0L10 10M10 0L0 10" stroke="#888" stroke-width="1"/></pattern><pattern id="pq-vert" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#fff"/><line x1="4" y1="0" x2="4" y2="8" stroke="#888" stroke-width="1.2"/></pattern></defs>"##;

const FILLS: [&str; 6] = ["#bfdbfe", "#fde68a", "#bbf7d0", "#fecaca", "#e9d5ff", "#fed7aa"];

/// Rough rendered width of a run of parts at font `size`.
fn parts_w(parts: &[Part], size: f64) -> f64 {
    parts.iter().map(|p| p.s.chars().count() as f64 * size * if p.sup { 0.36 } else if p.serif { 0.52 } else if p.bold { 0.6 } else { 0.55 }).sum()
}

fn frac(n: i64, total: i64) -> String {
    let g = gcd(n, total).max(1);
    let (a, b) = (n / g, total / g);
    if b == 1 { format!("{a}") } else { format!("{a}/{b}") }
}

fn pct(n: i64, total: i64) -> String {
    let v = n as f64 * 100.0 / total.max(1) as f64;
    format!("{}%", fnum((v * 10.0).round() / 10.0))
}

pub fn render(p: &Params) -> String {
    REG.with(|r| r.borrow_mut().clear());
    let cross = match p.s("cross") { "Dihybrid" => Cross::Di, "X-linked" => Cross::XLinked, _ => Cross::Mono };
    XLINK.with(|x| x.set(cross == Cross::XLinked));
    let dom = p.s("dominance").to_string();
    let title = p.s("title").trim().to_string();
    let question = p.s("question").trim().to_string();

    let parsed = split_parents(p.s("parents"))
        .ok_or_else(|| "Parents: separate the two genotypes with \"x\"".to_string())
        .and_then(|(a, b)| Ok((parse_parent_dom(&a, cross, &dom)?, parse_parent_dom(&b, cross, &dom)?)));
    let (pa, pb) = match parsed {
        Ok(v) => v,
        Err(e) => {
            let mut d = Doc::new(560.0, 140.0);
            bold(&mut d, 280.0, 60.0, 17.0, "middle", "Cannot read the parents");
            txt(&mut d, 280.0, 90.0, 13.0, "middle", SANS, "", &e);
            return d.finish();
        }
    };
    let ga = gametes(&pa, cross);
    let gb = gametes(&pb, cross);
    let (nc, nr) = (ga.len(), gb.len());

    // children
    let mut kids: Vec<Vec<Child>> = Vec::new();
    for rb in &gb {
        let mut row = Vec::new();
        for ca in &ga {
            let genes: Vec<Pair> = (0..ca.len()).map(|i| order(ca[i], rb[i])).collect();
            let male = cross == Cross::XLinked && genes.iter().any(|g| g.contains(&'Y'));
            row.push(Child { genes, male });
        }
        kids.push(row);
    }

    // phenotype names
    let raw_names = p.s("phenotypes");
    let map = parse_map(raw_names);
    let names: Vec<Vec<String>> = if map.is_empty() {
        raw_names.split(';').map(|g| g.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).collect()
    } else { vec![] };
    let sex_split = cross == Cross::XLinked && p.b("sex_split");
    let xl = cross == Cross::XLinked;
    // (class indexes, name without sex, name with sex)
    let pheno3 = |c: &Child| -> (Vec<usize>, String, String) {
        let idxs: Vec<usize> = c.genes.iter().map(|g| class_idx(*g, &dom)).collect();
        let nm: Vec<String> = c.genes.iter().enumerate()
            .map(|(gi, g)| map_name(&map, *g, &dom, xl).unwrap_or_else(|| pheno_name(&names, gi, idxs[gi], *g, &dom))).collect();
        let base = nm.join(" ");
        let full = if xl { format!("{base} {}", if c.male { "male" } else { "female" }) } else { base.clone() };
        (idxs, base, full)
    };
    let pheno = |c: &Child| -> (Vec<usize>, String) {
        let (i, b, f) = pheno3(c);
        (i, if sex_split { b } else { f })
    };

    // tallies
    let mut geno_count: BTreeMap<(Vec<i32>, String), (i64, Vec<Part>)> = BTreeMap::new();
    let mut ph_count: BTreeMap<(Vec<usize>, bool, String), i64> = BTreeMap::new();
    for row in &kids {
        for c in row {
            let caps: Vec<i32> = c.genes.iter().map(|g| -(g.iter().filter(|x| is_dom(**x)).count() as i32) * 100 + g.iter().map(|x| reg_idx(*x).min(99) as i32).sum::<i32>() + if g.iter().any(|x| is_ychr(*x)) { 1000 } else { 0 }).collect();
            let e = geno_count.entry((caps, key_of(c))).or_insert((0, geno_parts(&c.genes, cross)));
            e.0 += 1;
            let (idx, nm) = pheno(c);
            *ph_count.entry((idx, c.male && !sex_split, nm)).or_insert(0) += 1;
        }
    }
    // shading classes: unique phenotype names in sorted order
    let mut classes: Vec<String> = Vec::new();
    for k in ph_count.keys() { if !classes.contains(&k.2) { classes.push(k.2.clone()); } }
    let class_of = |c: &Child| -> usize { let (_, nm) = pheno(c); classes.iter().position(|k| *k == nm).unwrap_or(0) };
    let shade_mode = p.s("shading").to_string();
    let patterns = p.s("shade_style") == "Gray and patterns";
    let shade_col: &str = if patterns { "#d9d9d9" } else { SHADES.iter().copied().find(|c| c.eq_ignore_ascii_case(p.s("shade_color"))).unwrap_or(SHADES[0]) };
    let ncls = classes.len();
    let shaded = |ci: usize| -> Option<&'static str> {
        match shade_mode.as_str() {
            s if s.starts_with("Each") => Some(if patterns { pattern_fill(ci, ncls) } else { FILLS[ci % FILLS.len()] }),
            s if s.starts_with("First") => (ci == 0).then_some(shade_col),
            s if s.starts_with("Last") => (ci + 1 == ncls).then_some(shade_col),
            _ => None,
        }
    };
    let shade_col_static: &'static str = if patterns { "#d9d9d9" } else { SHADES.iter().copied().find(|c| c.eq_ignore_ascii_case(p.s("shade_color"))).unwrap_or(SHADES[0]) };
    let _ = shade_col;

    // cells
    let cells_mode = if !p.b("filled") { "Blank" } else { match p.s("cells") { "Blank" => "Blank", "Some blank" => "Some blank", _ => "Filled in" } };
    let blank_set: Vec<usize> = p.s("blank_cells").split([',', ';', ' ']).filter_map(|t| t.trim().parse::<usize>().ok()).collect();
    let is_blank = |i: usize, j: usize| -> bool {
        match cells_mode { "Blank" => true, "Some blank" => blank_set.contains(&(j * nc + i + 1)), _ => false }
    };

    // ratio lines: (label, segments); a segment is one item
    let gstyle = if !p.b("geno_ratio") { "None" } else { match p.s("geno_ratio_style") { "Blank line" => "Blank line", "None" => "None", _ => "Shown" } };
    let pstyle = if !p.b("pheno_ratio") { "None" } else { match p.s("pheno_ratio_style") { "Blank line" => "Blank line", "None" => "None", _ => "Shown" } };
    let form = p.s("ratio_form");
    let total: i64 = geno_count.values().map(|v| v.0).sum();
    let gdiv = geno_count.values().map(|v| v.0).fold(0, gcd);
    let pdiv = ph_count.values().copied().fold(0, gcd);
    let (sep, glab, plab) = match form {
        "Percent" | "Fraction" => (", ", "Genotypes: ", "Phenotypes: "),
        _ => (" : ", "Genotype ratio: ", "Phenotype ratio: "),
    };
    let amount = |n: i64, tot: i64, div: i64| -> String {
        match form { "Percent" => pct(n, tot), "Fraction" => frac(n, tot), _ => format!("{}", n / div.max(1)) }
    };
    let mut blocks: Vec<(String, Vec<Vec<Part>>, bool)> = Vec::new(); // label, segments, blank
    if gstyle != "None" {
        let segs: Vec<Vec<Part>> = geno_count.values().map(|(n, gp)| {
            let mut v = vec![Part::plain(&format!("{} ", amount(*n, total, gdiv)))];
            v.extend(gp.iter().cloned());
            v
        }).collect();
        blocks.push((glab.to_string(), segs, gstyle == "Blank line"));
    }
    if pstyle != "None" {
        if sex_split {
            for (male, lab) in [(false, "Daughters: "), (true, "Sons: ")] {
                let mut cnt: Vec<(String, i64)> = Vec::new();
                for row in &kids { for c in row { if c.male != male { continue; } let (_, nm) = pheno(c); if let Some(e) = cnt.iter_mut().find(|e| e.0 == nm) { e.1 += 1; } else { cnt.push((nm, 1)); } } }
                cnt.sort_by(|a, b| classes.iter().position(|k| *k == a.0).cmp(&classes.iter().position(|k| *k == b.0)));
                let tot: i64 = cnt.iter().map(|e| e.1).sum();
                let div = cnt.iter().map(|e| e.1).fold(0, gcd);
                let segs: Vec<Vec<Part>> = if form == "Ratio" && cnt.len() == 1 {
                    vec![vec![Part::plain(&format!("all {}", cnt[0].0))]]
                } else {
                    cnt.iter().map(|(nm, n)| vec![Part::plain(&format!("{} {nm}", amount(*n, tot, div)))]).collect()
                };
                if tot > 0 { blocks.push((lab.to_string(), segs, pstyle == "Blank line")); }
            }
        } else {
            let segs: Vec<Vec<Part>> = ph_count.iter().map(|(k, n)| vec![Part::plain(&format!("{} {}", amount(*n, total, pdiv), k.2))]).collect();
            blocks.push((plab.to_string(), segs, pstyle == "Blank line"));
        }
    }

    // geometry
    let cell = match (cross, nc) { (_, 4) => 82.0, _ => 100.0 };
    let lab_mode = p.s("parent_labels");
    let blank_par = p.b("blank_parents");
    let (pre_top, pre_side) = match lab_mode { "♀ ♂" => ("♀  ", "♂  "), "Mother" => ("Mother ", "Father "), _ => ("", "") };
    let show_par = lab_mode != "None";
    let gp_side = geno_parts(&pb, cross);
    let side_w = if !show_par { 0.0 } else { parts_w(&[Part::b(pre_side)], 28.0) + if blank_par { 80.0 } else { parts_w(&gp_side, 28.0) } };
    let gx_off = (side_w + 86.0).max(120.0);
    let content_w = gx_off + cell * nc as f64;
    let fs_ratio = 19.0;
    let max_line = (content_w + 40.0).max(560.0);
    // pack ratio segments into lines no wider than max_line
    let mut rlines: Vec<Vec<Part>> = Vec::new();
    for (lab, segs, blank) in &blocks {
        let mut line = vec![Part::b(lab)];
        if *blank {
            line.push(Part::plain("______________________"));
            rlines.push(line);
            continue;
        }
        let mut first = true;
        for seg in segs {
            let mut piece = Vec::new();
            if !first { piece.push(Part::plain(sep.trim_end())); }
            let trial_w = parts_w(&line, fs_ratio) + parts_w(&piece, fs_ratio) + parts_w(seg, fs_ratio) + 8.0;
            if !first && trial_w > max_line {
                line.extend(piece);
                rlines.push(std::mem::take(&mut line));
                line = seg.clone();
            } else {
                if !first { piece.push(Part::plain(" ")); }
                line.extend(piece);
                line.extend(seg.iter().cloned());
            }
            first = false;
        }
        rlines.push(line);
    }
    let lines_w = rlines.iter().map(|l| parts_w(l, fs_ratio)).fold(0.0, f64::max);
    let w = (content_w.max(lines_w) + 90.0).max(580.0);
    let ox = (w - content_w) / 2.0;
    let cx_mid = w / 2.0;

    let mut y = 16.0;
    let mut head: Vec<(f64, f64, String, bool)> = Vec::new(); // y, size, text, bold
    if !title.is_empty() { y += 24.0; head.push((y, 21.0, title, true)); y += 12.0; }
    for l in wrap(&question, ((w - 60.0) / 8.0) as usize) { y += 22.0; head.push((y, 15.0, l, false)); }
    if !head.is_empty() { y += 14.0; }
    let p1_y = y + 28.0;
    let gam_y = p1_y + 38.0;
    let gy = gam_y + 14.0;
    let gx = ox + gx_off;
    let grid_h = cell * nr as f64;
    let mut yb = gy + grid_h + 34.0;
    let mut d_h = yb;
    d_h += rlines.len() as f64 * 30.0;
    let key_on = shade_mode.starts_with("Each") || (patterns && shade_mode != "None");
    let key_items: Vec<(String, &'static str)> = if shade_mode.starts_with("Each") {
        classes.iter().enumerate().map(|(ci, k)| (k.clone(), shaded(ci).unwrap_or("#fff"))).collect()
    } else if key_on {
        let ci = if shade_mode.starts_with("First") { 0 } else { ncls.saturating_sub(1) };
        vec![(classes.get(ci).cloned().unwrap_or_default(), shade_col_static)]
    } else { vec![] };
    let key_cols = if w > 900.0 { 3 } else if patterns { 3 } else { 2 };
    let key_rows = key_items.len().div_ceil(key_cols);
    if key_on { d_h += 18.0 + key_rows as f64 * 26.0; }
    d_h += 16.0;

    let mut d = Doc::new(w, d_h);
    if patterns { d.raw(PATTERN_DEFS); }
    for (hy, sz, t, b) in &head {
        txt(&mut d, cx_mid, *hy, *sz, "middle", SANS, if *b { r#"font-weight="bold""# } else { "" }, t);
    }
    // parent labels
    let side_cx;
    if show_par {
        let mut top = Vec::new();
        if !pre_top.is_empty() { top.push(if lab_mode == "Mother" { Part::b(pre_top) } else { Part::plain(pre_top) }); }
        let mut side = Vec::new();
        if !pre_side.is_empty() { side.push(if lab_mode == "Mother" { Part::b(pre_side) } else { Part::plain(pre_side) }); }
        let tcx = gx + cell * nc as f64 / 2.0;
        side_cx = gx - 66.0 - side_w / 2.0;
        // prefix (Mother / ♀) drawn as its own text so the genotype keeps its serif italic face
        let pre_attr = if lab_mode == "Mother" { r#"font-weight="bold""# } else { "" };
        let pre_w = |t: &str| if t.is_empty() { 0.0 } else if lab_mode == "Mother" { parts_w(&[Part::b(t)], 28.0) } else { 30.0 };
        let gtop = geno_parts(&pa, cross);
        let (gw_top, gw_side) = if blank_par { (80.0, 80.0) } else { (parts_w(&gtop, 28.0), parts_w(&gp_side, 28.0)) };
        let tx0 = tcx - (pre_w(pre_top) + gw_top) / 2.0;
        let sx0 = side_cx - (pre_w(pre_side) + gw_side) / 2.0;
        let sy = gy + grid_h / 2.0 + 10.0;
        if !pre_top.is_empty() { txt(&mut d, tx0, p1_y, 28.0, "start", SANS, pre_attr, pre_top.trim_end()); }
        if !pre_side.is_empty() { txt(&mut d, sx0, sy, 28.0, "start", SANS, pre_attr, pre_side.trim_end()); }
        let _ = (&top, &side);
        if blank_par {
            d.line(tx0 + pre_w(pre_top), p1_y + 2.0, tx0 + pre_w(pre_top) + 80.0, p1_y + 2.0, INK, 1.4);
            d.line(sx0 + pre_w(pre_side), sy + 2.0, sx0 + pre_w(pre_side) + 80.0, sy + 2.0, INK, 1.4);
        } else {
            rich(&mut d, tx0 + pre_w(pre_top), p1_y, 28.0, "start", &gtop, true);
            rich(&mut d, sx0 + pre_w(pre_side), sy, 28.0, "start", &gp_side, true);
        }
    } else {
        side_cx = gx - 66.0;
    }
    let gsz = if nc == 4 { 20.0 } else { 26.0 };
    if p.b("blank_gametes") {
        for i in 0..nc { let x = gx + cell * (i as f64 + 0.5); d.line(x - 22.0, gam_y + 2.0, x + 22.0, gam_y + 2.0, INK, 1.3); }
        for j in 0..nr { let yy = gy + cell * (j as f64 + 0.5) + 8.0; d.line(gx - 56.0, yy, gx - 12.0, yy, INK, 1.3); }
    } else {
        for (i, g) in ga.iter().enumerate() {
            rich(&mut d, gx + cell * (i as f64 + 0.5), gam_y, gsz, "middle", &gamete_parts(g, cross), true);
        }
        for (j, g) in gb.iter().enumerate() {
            rich(&mut d, gx - 14.0, gy + cell * (j as f64 + 0.5) + gsz * 0.35, gsz, "end", &gamete_parts(g, cross), true);
        }
    }
    // cells
    let csz = if nc == 4 { 19.0 } else { 28.0 };
    let under = p.b("cell_phenotype");
    for (j, row) in kids.iter().enumerate() {
        for (i, c) in row.iter().enumerate() {
            let (x, yy) = (gx + cell * i as f64, gy + cell * j as f64);
            let blank = is_blank(i, j);
            let fill = if blank { "#ffffff" } else { shaded(class_of(c)).unwrap_or("#ffffff") };
            d.rect(x, yy, cell, cell, fill, "#555", 1.2);
            if !blank {
                let halo = patterns && fill.starts_with("url");
                if halo { d.raw(r##"<g stroke="#fff" stroke-width="5" stroke-linejoin="round" style="paint-order:stroke">"##); }
                let gy_txt = if under { yy + cell / 2.0 + csz * 0.2 } else { yy + cell / 2.0 + csz * 0.35 };
                rich(&mut d, x + cell / 2.0, gy_txt, csz, "middle", &geno_parts(&c.genes, cross), true);
                if under {
                    let (_, nm) = pheno(c);
                    let fs = if nc == 4 { 11.0 } else { 15.0 };
                    let nm = if sex_split { pheno3(c).2 } else { nm };
                    txt(&mut d, x + cell / 2.0, yy + cell / 2.0 + csz * 0.2 + fs + 6.0, fs, "middle", SANS, "", &nm);
                }
                if halo { d.raw("</g>"); }
            }
        }
    }
    d.rect(gx, gy, cell * nc as f64, grid_h, "none", INK, if p.b("outline_heavy") { 3.0 } else { 1.6 });
    LAST.with(|l| *l.borrow_mut() = Some(Layout { gx, p1_y, gy, cell, nc, nr, ox: side_cx - 30.0, pb_y: gy + grid_h / 2.0 }));
    for i in 1..nc { d.line(gx + cell * i as f64, gy, gx + cell * i as f64, gy + grid_h, INK, 1.6); }
    for j in 1..nr { d.line(gx, gy + cell * j as f64, gx + cell * nc as f64, gy + cell * j as f64, INK, 1.6); }

    // key (above the ratios, like the reference)
    if key_on {
        let longest = key_items.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(4) as f64;
        let colw = ((w - 80.0) / key_cols as f64).min(longest * 8.0 + 50.0);
        let kw = colw * key_cols.min(key_items.len()).max(1) as f64;
        let kx = (w - kw) / 2.0;
        let ky = yb - 14.0;
        for (ci, (k, f)) in key_items.iter().enumerate() {
            let (col, row) = (ci % key_cols, ci / key_cols);
            let (x, yy) = (kx + col as f64 * colw, ky + row as f64 * 26.0);
            d.rect(x, yy, 18.0, 18.0, f, INK, 1.2);
            txt(&mut d, x + 26.0, yy + 14.0, 14.0, "start", SANS, "", k);
        }
        yb += 18.0 + key_rows as f64 * 26.0;
    }
    // ratios: left-aligned block centred on the page
    let bx = cx_mid - lines_w / 2.0;
    for l in &rlines {
        rich(&mut d, bx, yb, fs_ratio, "start", l, true);
        yb += 30.0;
    }
    d.finish()
}

/// "Start from an example" entries (the reference gallery).
/// The reference's "Classic setups" dropdown, from its share links.
pub fn classics() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    vec![
        Example { name: "Mendel’s pea plants", params: vec![] },
        Example { name: "Test cross", params: vec![("parents", st("Tt x tt"))] },
        Example { name: "Dihybrid cross", params: vec![
            ("cross", st("Dihybrid")), ("parents", st("RrYy x RrYy")), ("phenotypes", st("R: round; r: wrinkled; Y: yellow; y: green")),
        ] },
        Example { name: "Color blindness", params: vec![
            ("cross", st("X-linked")), ("parents", st("X^B X^b x X^B Y")), ("phenotypes", st("X^B: normal vision; X^b: color-blind")), ("parent_labels", st("♀ ♂")),
        ] },
        Example { name: "Snapdragon color", params: vec![
            ("dominance", st("Incomplete")), ("parents", st("C^R C^W x C^R C^W")), ("phenotypes", st("C^R: red; C^R+C^W: pink; C^W: white")),
        ] },
        Example { name: "ABO blood type", params: vec![
            ("dominance", st("Codominance")), ("parents", st("I^A i x I^B i")), ("phenotypes", st("I^A: type A; I^A+I^B: type AB; I^B: type B; i: type O")),
        ] },
    ]
}

pub fn examples() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    vec![
        Example { name: "Tt x Tt Punnett square: monohybrid cross for pea plant height", params: vec![
            ("shading", st("Each phenotype, with a key")), ("shade_style", st("Gray and patterns")), ("title", st("Pea plant height")),
        ] },
        Example { name: "Dihybrid cross Punnett square: RrYy x RrYy", params: vec![
            ("cross", st("Dihybrid")), ("parents", st("RrYy x RrYy")), ("phenotypes", st("R: round; r: wrinkled; Y: yellow; y: green")),
            ("shading", st("Each phenotype, with a key")), ("shade_style", st("Gray and patterns")),
        ] },
        Example { name: "Color blindness Punnett square: X-linked cross of a carrier mother", params: vec![
            ("cross", st("X-linked")), ("parents", st("X^B X^b x X^B Y")), ("phenotypes", st("X^B: normal vision; X^b: color-blind")),
            ("parent_labels", st("♀ ♂")), ("shading", st("Last phenotype only")), ("shade_style", st("Gray and patterns")),
            ("ratio_form", st("Percent")), ("sex_split", Value::B(true)),
        ] },
        Example { name: "Incomplete dominance Punnett square: snapdragon flower color", params: vec![
            ("dominance", st("Incomplete")), ("parents", st("C^R C^W x C^R C^W")), ("phenotypes", st("C^R: red; C^R+C^W: pink; C^W: white")),
            ("cell_phenotype", Value::B(true)), ("shading", st("Each phenotype, with a key")), ("shade_style", st("Gray and patterns")),
        ] },
        Example { name: "Blank ABO blood type Punnett square worksheet (codominance)", params: vec![
            ("dominance", st("Codominance")), ("parents", st("I^A i x I^B i")), ("phenotypes", st("I^A: type A; I^A+I^B: type AB; I^B: type B; i: type O")),
            ("parent_labels", st("Mother")), ("cells", st("Blank")), ("geno_ratio_style", st("Blank line")), ("pheno_ratio_style", st("Blank line")),
            ("question", st("A mother with type A blood and a father with type B blood each carry the allele for type O. Complete the Punnett square and give the ratios.")),
        ] },
        Example { name: "Test cross: Tt x tt", params: vec![("parents", st("Tt x tt")), ("ratio_form", st("Fraction"))] },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ratios() {
        let svg = render_case("punnett-square", "default", &[]);
        assert!(svg.contains("1 "));
        assert!(svg.contains("3 tall"));
    }

    #[test]
    fn dihybrid_ratio_9331() {
        let svg = render_case("punnett-square", "dihybrid", &[
            ("cross", s("Dihybrid")), ("parents", s("TtRr x TtRr")),
            ("phenotypes", s("tall, short; round, wrinkled")), ("shading", s("Each phenotype, with a key")),
            ("title", s("Dihybrid cross")), ("question", s("Two pea plants heterozygous for height and seed shape are crossed. Complete the square and give the ratio.")),
        ]);
        assert!(svg.contains("9 tall round : 3 tall wrinkled : 3 short round : 1 short wrinkled")
            || svg.contains("9 tall round") && svg.contains("1 short wrinkled"));
    }

    #[test]
    fn xlinked_and_incomplete() {
        let svg = render_case("punnett-square", "xlinked", &[
            ("cross", s("X-linked")), ("parents", s("XCXc x XCY")), ("phenotypes", s("normal vision, colour blind")),
            ("shading", s("Last phenotype only")),
        ]);
        assert!(svg.contains("colour blind male"));
        let svg = render_case("punnett-square", "incomplete", &[
            ("dominance", s("Incomplete")), ("parents", s("Rr x Rr")), ("phenotypes", s("red, white")),
            ("shading", s("Each phenotype, with a key")),
        ]);
        assert!(svg.contains("2 intermediate") && svg.contains("1 red") && svg.contains("1 white"));
        render_case("punnett-square", "blank", &[("filled", Value::B(false)), ("parents", s("TT x tt")), ("geno_ratio", Value::B(false))]);
        render_case("punnett-square", "bad", &[("parents", s("Tt"))]);
        for ex in examples() { render_case("punnett-square", "example", &ex.params); }
        let abo = render_case("punnett-square", "abo", &[("dominance", s("Codominance")), ("parents", s("I^A i x I^B i")),
            ("phenotypes", s("I^A: type A; I^A+I^B: type AB; I^B: type B; i: type O"))]);
        assert!(abo.contains("1 type A") && abo.contains("1 type AB") && abo.contains("1 type O"), "{abo}");
        // ABO with the default (unrelated) phenotype names gives blood types; a user map still wins
        for dm in ["Complete", "Codominance"] {
            let abo2 = render_case("punnett-square", "abo_default", &[("dominance", s(dm)), ("parents", s("I^A i x I^B i"))]);
            for t in ["1 A<", "1 AB<", "1 B<", "1 O<"] { assert!(abo2.contains(t), "{dm}: missing {t} in {abo2}"); }
            assert!(!abo2.contains("short") && !abo2.contains(">2 "), "{dm}: {abo2}");
        }
        let abo3 = render_case("punnett-square", "abo_names", &[("parents", s("IAi x IBi")), ("dominance", s("Codominance")),
            ("phenotypes", s("I^A: type A; I^A+I^B: type AB; I^B: type B; i: type O"))]);
        assert!(abo3.contains("1 type AB") && abo3.contains("1 type O"));
        let snap = render_case("punnett-square", "snap", &[("dominance", s("Incomplete")), ("parents", s("CRCW x CRCW")), ("phenotypes", s("red, white"))]);
        assert!(snap.contains("2 intermediate"));
        let xl = render_case("punnett-square", "xl_pct", &[("cross", s("X-linked")), ("parents", s("X^B X^b x X^B Y")),
            ("phenotypes", s("X^B: normal vision; X^b: color-blind")), ("ratio_form", s("Percent")), ("sex_split", Value::B(true))]);
        assert!(xl.contains("100% normal vision") && xl.contains("50% color-blind"), "{xl}");
    }
}
