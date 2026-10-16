//! Biology figure generators: original re-creations of the generator set found on
//! biologyfigures.com (population growth, Punnett square, pedigree, predator-prey,
//! gel electrophoresis, micropipette reading). Each lives in its own submodule.
use super::svg::{esc, Doc};
use super::*;

mod handles;
mod gel;
mod pedigree;
mod pipette;
mod plot;
mod popdyn;
mod punnett;

/// Direct-manipulation handles for this subject's figures (None = not one of ours).
pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<interact::Handle>> {
    handles::handles(id, p, size)
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef {
            id: "population-growth",
            subject: Subject::Biology,
            title: "Population Growth",
            blurb: "Graph exponential, logistic, overshoot or boom-and-crash growth, with growth-rate panels and census points.",
            params: popdyn::growth_params,
            render: popdyn::growth_render,
        },
        FigureDef {
            id: "punnett-square",
            subject: Subject::Biology,
            title: "Punnett Square",
            blurb: "Cross two parents (monohybrid, dihybrid or X-linked) and get the grid, shading and genotype and phenotype ratios.",
            params: punnett::params,
            render: punnett::render,
        },
        FigureDef {
            id: "pedigree",
            subject: Subject::Biology,
            title: "Pedigree",
            blurb: "Generate a family tree for a chosen inheritance pattern, with affected and carrier symbols, numbering and a key.",
            params: pedigree::params,
            render: pedigree::render,
        },
        FigureDef {
            id: "predator-prey",
            subject: Subject::Biology,
            title: "Predator-Prey Cycles",
            blurb: "Lotka-Volterra population cycles for a predator and its prey, over time or as a phase plot.",
            params: popdyn::pp_params,
            render: popdyn::pp_render,
        },
        FigureDef {
            id: "gel-electrophoresis",
            subject: Subject::Biology,
            title: "Gel Electrophoresis",
            blurb: "Lay out DNA ladders and sample lanes on an agarose gel, positioned by fragment size.",
            params: gel::params,
            render: gel::render,
        },
        FigureDef {
            id: "micropipette-reading",
            subject: Subject::Biology,
            title: "Micropipette Reading",
            blurb: "A micropipette whose digital display reads a chosen volume, for practice reading the dial.",
            params: pipette::params,
            render: pipette::render,
        },
    ]
}

// ---------------------------------------------------------------- shared helpers

pub(crate) const SANS: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, Helvetica, sans-serif";
pub(crate) const SERIF: &str = "Noto Serif, Liberation Serif, DejaVu Serif, Times New Roman, Times, serif";
pub(crate) const INK: &str = "#111";

/// Text with full control: `extra` is raw extra attributes (weight, style, transform...).
pub(crate) fn txt(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, fam: &str, extra: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{fam}" fill="{INK}" {extra}>{}</text>"##,
        esc(s)
    ));
}

pub(crate) fn bold(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str) {
    txt(d, x, y, size, anchor, SANS, r#"font-weight="bold""#, s);
}

/// A run inside a mixed-style line of text.
#[derive(Clone)]
pub(crate) struct Part {
    pub s: String,
    pub sup: bool,
    pub serif: bool,
    pub bold: bool,
}

impl Part {
    pub fn plain(s: &str) -> Part { Part { s: s.into(), sup: false, serif: false, bold: false } }
    pub fn b(s: &str) -> Part { Part { s: s.into(), sup: false, serif: false, bold: true } }
    pub fn geno(s: &str, sup: bool) -> Part { Part { s: s.into(), sup, serif: true, bold: false } }
}

pub(crate) fn rich(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, parts: &[Part], italic_serif: bool) {
    let mut s = format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" fill="{INK}" font-family="{SANS}">"##);
    let mut raised = false;
    for p in parts {
        let fam = if p.serif { SERIF } else { SANS };
        let mut attrs = format!(r#"font-family="{fam}""#);
        if p.bold { attrs.push_str(r#" font-weight="bold""#); }
        if p.serif && italic_serif { attrs.push_str(r#" font-style="italic""#); }
        if p.sup {
            attrs.push_str(&format!(r#" font-size="{:.1}" dy="{:.1}""#, size * 0.65, -size * 0.38));
            raised = true;
        } else if raised {
            attrs.push_str(&format!(r#" dy="{:.1}""#, size * 0.38));
            raised = false;
        }
        s.push_str(&format!("<tspan {attrs}>{}</tspan>", esc(&p.s)));
    }
    s.push_str("</text>");
    d.raw(&s);
}

/// Greedy word wrap by character count.
pub(crate) fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > max {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(w);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// Small deterministic PRNG (xorshift64*).
pub(crate) struct Rng(pub u64);
impl Rng {
    pub fn new(seed: u64) -> Rng { Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03 | 1) }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn f(&mut self) -> f64 { (self.next() >> 11) as f64 / (1u64 << 53) as f64 }
    pub fn below(&mut self, n: usize) -> usize { (self.next() % n.max(1) as u64) as usize }
}

pub(crate) fn dashed(d: &mut Doc, pts: &[(f64, f64)], stroke: &str, sw: f64, dash: &str) {
    let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
    d.raw(&format!(
        r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" stroke-dasharray="{dash}" stroke-linejoin="round" stroke-linecap="round"/>"#,
        p.join(" ")
    ));
}

/// 1-2-5 step giving roughly `n` divisions of `range`.
pub(crate) fn nice_step(range: f64, n: f64) -> f64 {
    let raw = (range / n).max(1e-12);
    let p = 10f64.powf(raw.log10().floor());
    let f = raw / p;
    let m = if f <= 1.5 { 1.0 } else if f <= 3.0 { 2.0 } else if f <= 7.0 { 5.0 } else { 10.0 };
    m * p
}

pub(crate) fn fnum(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        return format!("{}", v.round() as i64);
    }
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub(crate) fn commas(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub(crate) fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a.abs() } else { gcd(b, a % b) } }

#[cfg(test)]
pub(crate) fn out_dir() -> std::path::PathBuf {
    let p = std::path::PathBuf::from("/tmp/claude-1000/-home-shuff57/4fa1e9b4-a716-475e-b753-2c60208b3c78/scratchpad/biology");
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Test helper: render `id` with overrides, write PNG, return svg.
#[cfg(test)]
pub(crate) fn render_case(id: &str, name: &str, over: &[(&str, Value)]) -> String {
    let def = defs().into_iter().find(|d| d.id == id).unwrap();
    let mut p = Params::defaults(&(def.params)());
    for (k, v) in over {
        assert!(p.0.contains_key(*k), "unknown param {k}");
        p.0.insert((*k).to_string(), v.clone());
    }
    let svg = (def.render)(&p);
    let png = to_png(&svg, 1.5).unwrap_or_else(|e| panic!("{id}/{name}: {e}"));
    std::fs::write(out_dir().join(format!("{id}_{name}.png")), png).unwrap();
    svg
}

#[cfg(test)]
pub(crate) fn s(v: &str) -> Value { Value::S(v.into()) }
