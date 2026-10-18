//! Chemistry figure generators (original parametric SVG, modelled on the kinds of figures a
//! chemistry teacher needs: lab-instrument readings, curves, atomic structure and spectra).
//! Submodules live in `chemistry/`; this file holds shared drawing helpers and the registry.
#![allow(dead_code)]
use super::svg::Doc;
use super::{interact, FigureDef, Params};

mod handles;
mod atoms;
pub mod frame;
pub mod magnify;
pub use frame::*;
mod curves;
pub mod data;
mod lewis;
mod measure;
mod spectra;

pub const INK: &str = "#1b1b1b";
pub const GLASS: &str = "#f3f8fb";
pub const GLASS_EDGE: &str = "#56626b";
pub const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";

/// Number formatted with `dp` decimals.
pub fn num(v: f64, dp: usize) -> String {
    let s = format!("{:.*}", dp, v);
    if s.starts_with("-") && s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') { s[1..].to_string() } else { s }
}

/// Text with colour/weight control.
pub fn tx(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, color: &str, bold: bool, s: &str) {
    let w = if bold { "bold" } else { "normal" };
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{FONT}" font-weight="{w}" fill="{color}">{}</text>"##,
        super::svg::esc(s)
    ));
}

/// Rotated text about (x, y).
pub fn tr(d: &mut Doc, x: f64, y: f64, size: f64, deg: f64, anchor: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{FONT}" fill="{INK}" transform="rotate({deg} {x:.2} {y:.2})">{}</text>"##,
        super::svg::esc(s)
    ));
}

pub fn dashed(d: &mut Doc, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, sw: f64) {
    d.raw(&format!(
        r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{color}" stroke-width="{sw}" stroke-dasharray="4 3"/>"#
    ));
}

/// (fill, edge) for the lab "Liquid color" choice.
pub fn liquid(name: &str) -> (&'static str, &'static str) {
    match name {
        "Blue" => ("#9ccbf0", "#4f90c4"),
        "Red" => ("#f09aa0", "#c4525c"),
        "Green" => ("#a5dcab", "#52a35c"),
        _ => ("#c9cfd5", "#8a949d"),
    }
}

pub fn shade(name: &str) -> &'static str {
    match name {
        "White" => "#ffffff",
        "Light gray" => "#d4d4d4",
        "Gray" => "#9a9a9a",
        "Dark gray" => "#5c5c5c",
        _ => "#161616",
    }
}

/// Tiny deterministic PRNG so "random-looking" layouts are stable for given params.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
    }
    pub fn range(&mut self, a: f64, b: f64) -> f64 { a + (b - a) * self.next() }
}

/// Wavelength (nm) to an approximate display colour.
pub fn wavelength_rgb(nm: f64) -> (u8, u8, u8) {
    let (r, g, b) = if nm < 380.0 { (0.5, 0.0, 0.7) }
    else if nm < 440.0 { (-(nm - 440.0) / 60.0, 0.0, 1.0) }
    else if nm < 490.0 { (0.0, (nm - 440.0) / 50.0, 1.0) }
    else if nm < 510.0 { (0.0, 1.0, -(nm - 510.0) / 20.0) }
    else if nm < 580.0 { ((nm - 510.0) / 70.0, 1.0, 0.0) }
    else if nm < 645.0 { (1.0, -(nm - 645.0) / 65.0, 0.0) }
    else if nm <= 780.0 { (1.0, 0.0, 0.0) } else { (0.5, 0.0, 0.0) };
    let f = if nm < 420.0 { 0.45 + 0.55 * (nm - 380.0).max(0.0) / 40.0 } else if nm > 700.0 { 0.35 + 0.65 * ((780.0 - nm) / 80.0).clamp(0.0, 1.0) } else { 1.0 };
    let c = |v: f64| (255.0 * (v * f).clamp(0.0, 1.0).powf(0.8)).round() as u8;
    (c(r), c(g), c(b))
}
pub fn hex(c: (u8, u8, u8)) -> String { format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2) }

/// Draw a small solid object (shared by the length, mass and displacement figures).
/// `kind`: Marbles | Rock | Cube | Metal cylinder. Sits on y = `base` centred on `cx`.
pub fn object(d: &mut Doc, kind: &str, cx: f64, base: f64, size: f64, seed: u64) {
    match kind {
        "Marbles" => {
            let r = size * 0.3;
            for i in 0..3 {
                let (x, y) = match i { 0 => (cx - r, base - r), 1 => (cx + r, base - r), _ => (cx, base - r - r * 1.74) };
                d.circle(x, y, r, "#8fb4d6", "#2e4f6e", 1.5);
                d.circle(x - r * 0.3, y - r * 0.3, r * 0.25, "#d9e9f6", "none", 0.0);
            }
        }
        "Rock" => {
            let mut rg = Rng(seed ^ 0x9e37);
            let n = 9;
            let pts: Vec<(f64, f64)> = (0..n).map(|i| {
                let a = std::f64::consts::PI * 2.0 * i as f64 / n as f64;
                let rr = size * 0.5 * rg.range(0.78, 1.0);
                (cx + rr * a.cos(), base - size * 0.36 + rr * 0.72 * a.sin())
            }).collect();
            let mut pts2 = pts.clone();
            for p in pts2.iter_mut() { if p.1 > base { p.1 = base; } }
            d.poly(&pts2, "#b9b2a6", "#4d4840", 1.6);
            d.polyline(&[(cx - size * 0.15, base - size * 0.45), (cx + size * 0.05, base - size * 0.3), (cx + size * 0.2, base - size * 0.38)], "#7d766b", 1.0);
        }
        "Cube" => {
            let s = size * 0.8;
            d.rect(cx - s / 2.0, base - s, s, s, "#d9c08a", "#5f4b1f", 1.6);
            let t = s * 0.28;
            d.poly(&[(cx - s / 2.0, base - s), (cx - s / 2.0 + t, base - s - t), (cx + s / 2.0 + t, base - s - t), (cx + s / 2.0, base - s)], "#ecdcb4", "#5f4b1f", 1.6);
            d.poly(&[(cx + s / 2.0, base - s), (cx + s / 2.0 + t, base - s - t), (cx + s / 2.0 + t, base - t), (cx + s / 2.0, base)], "#bfa56e", "#5f4b1f", 1.6);
        }
        _ => {
            let w = size * 0.55;
            let h = size * 0.85;
            d.rect(cx - w / 2.0, base - h, w, h, "#b9c1c9", "#3a444d", 1.6);
            d.raw(&format!(r##"<ellipse cx="{cx:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" fill="#dfe4e8" stroke="#3a444d" stroke-width="1.6"/>"##, base - h, w / 2.0, w * 0.18));
            d.line(cx - w * 0.28, base - h + 6.0, cx - w * 0.28, base - 4.0, "#eef1f3", 2.0);
        }
    }
}

/// Optional centred figure title from the `title` param (nothing when empty).
pub fn title_at(d: &mut Doc, p: &Params, cx: f64, y: f64, size: f64) {
    let t = p.s("title").trim();
    if !t.is_empty() { tx(d, cx, y, size, "middle", INK, true, t); }
}

/// Palette for curve colours.
pub const CURVE_COLORS: [&str; 6] = ["#1f5fa8", "#c0392b", "#2e8b57", "#8e44ad", "#d68910", "#1b1b1b"];

/// "Start from an example" entries for this subject's figures.
pub fn examples(id: &str) -> Option<Vec<super::Example>> {
    spectra::examples(id)
        .or_else(|| atoms::examples(id))
        .or_else(|| curves::examples(id))
        .or_else(|| lewis::examples(id))
        .or_else(|| measure::examples(id))
}

/// Handles are laid out in the figure's own coordinates; the chart-title band the wrapper adds
/// shifts the figure down, so positions and pointer coordinates are shifted to match.
/// "Add ..." chips / action buttons for this subject's figures.
/// particle-diagram: the reference's "Add a kind" adds kinds 2-4 (it allows four), each new kind
/// starting as 4 lone, medium, white, uncharged particles (in both boxes), plus "Shuffle".
/// bohr-model: "Fill" (shells back to the automatic ones) and "Shuffle" (nucleus balls).
pub fn chips(id: &str) -> Option<super::ChipsCard> {
    use super::{Chip, ChipsCard, Value};
    let kind = |label: &'static str, k: [&'static str; 6]| Chip {
        label,
        set: vec![
            (k[0], Value::I(4)),
            (k[1], Value::I(4)),
            (k[2], Value::S("Alone".into())),
            (k[3], Value::S("M".into())),
            (k[4], Value::S("White".into())),
            (k[5], Value::S("None".into())),
        ],
    };
    let shuffle = Chip { label: "Shuffle", set: vec![("shuffle", Value::I(i64::MIN))] };
    match id {
        "particle-diagram" => Some(ChipsCard {
            title: "Add a kind",
            count_key: "",
            max: 0,
            chips: vec![
                kind("Kind 2", ["b_count", "b2_count", "b_shape", "b_size", "b_shade", "b_charge"]),
                kind("Kind 3", ["c_count", "c2_count", "c_shape", "c_size", "c_shade", "c_charge"]),
                kind("Kind 4", ["d_count", "d2_count", "d_shape", "d_size", "d_shade", "d_charge"]),
                shuffle,
            ],
        }),
        "bohr-model" => Some(ChipsCard {
            title: "Electrons and nucleus",
            count_key: "",
            max: 0,
            chips: vec![Chip { label: "Fill the shells", set: vec![("shells_custom", Value::S(String::new()))] }, shuffle],
        }),
        _ => None,
    }
}

pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<interact::Handle>> {
    let dy = frame::title_band(p);
    let hs = handles::handles(id, p, (size.0, size.1 - dy))?;
    if dy == 0.0 {
        return Some(hs);
    }
    Some(hs.into_iter().map(|mut h| {
        h.pos.1 += dy;
        if let Some(f) = h.drag.take() {
            h.drag = Some(std::sync::Arc::new(move |q: &mut Params, t: (f64, f64)| f(q, (t.0, t.1 - dy))));
        }
        h
    }).collect())
}

pub fn defs() -> Vec<FigureDef> {
    let mut v = Vec::new();
    v.extend(measure::defs());
    v.extend(curves::defs());
    v.extend(atoms::defs());
    v.extend(lewis::defs());
    v.extend(spectra::defs());
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{to_png, Kind, ParamDef, Params, Value};
    use std::path::PathBuf;

    pub fn outdir() -> PathBuf {
        let p = PathBuf::from("/tmp/claude-1000/-home-shuff57/4fa1e9b4-a716-475e-b753-2c60208b3c78/scratchpad/chemistry");
        let _ = std::fs::create_dir_all(&p);
        p
    }

    fn find(id: &str) -> FigureDef { defs().into_iter().find(|d| d.id == id).unwrap_or_else(|| panic!("no {id}")) }

    fn run(id: &str, tag: &str, over: &[(&str, Value)]) {
        let d = find(id);
        let pd = (d.params)();
        let mut p = Params::defaults(&pd);
        for (k, v) in over {
            assert!(pd.iter().any(|x| x.key == *k), "{id}: unknown param {k}");
            p.0.insert(k.to_string(), v.clone());
        }
        let svg = (d.render)(&p);
        let png = to_png(&svg, 1.0).unwrap_or_else(|e| panic!("{id}/{tag}: {e}"));
        std::fs::write(outdir().join(format!("{id}-{tag}.png")), png).unwrap();
        // every figure must differ if params differ is checked per-test below
    }
    fn s(x: &str) -> Value { Value::S(x.into()) }
    fn f(x: f64) -> Value { Value::F(x) }
    fn i(x: i64) -> Value { Value::I(x) }
    fn b(x: bool) -> Value { Value::B(x) }

    /// Applies an action chip the way the panel does (fresh-random values get a fixed number here).
    fn apply(p: &mut Params, chip: &crate::figures::Chip) {
        for (k, v) in &chip.set {
            let v = if *v == Value::I(i64::MIN) { Value::I(123) } else { v.clone() };
            p.0.insert(k.to_string(), v);
        }
    }

    /// The reference's nine particle-diagram examples, incl. the two alloys; in the steel one the
    /// small carbon atoms sit in the holes between iron atoms (not on lattice sites), and the
    /// gaps switch leaves solids without it exactly as they were.
    #[test]
    fn particle_examples_and_interstitial_alloy() {
        let names: Vec<&str> = examples("particle-diagram").unwrap().iter().map(|e| e.name).collect();
        assert_eq!(names.len(), 9, "the reference lists nine: {names:?}");
        assert_eq!(names[3], "Mixture of an element and a compound", "in the reference order");
        for want in ["Mixture of an element and a compound", "Interstitial alloy: steel", "Substitutional alloy: brass"] {
            assert!(names.contains(&want), "{want}");
        }
        let d = find("particle-diagram");
        let defs = (d.params)();
        let ex = examples("particle-diagram").unwrap();
        let steel = ex.iter().find(|e| e.name.contains("steel")).unwrap();
        let mut p = Params::defaults(&defs);
        for (k, v) in &steel.params {
            p.0.insert(k.to_string(), v.clone());
        }
        let svg = (d.render)(&p);
        // circles as (cx, cy, r)
        let circles: Vec<(f64, f64, f64)> = svg
            .split("<circle").skip(1)
            .filter_map(|c| {
                let g = |n: &str| c.split(&format!(" {n}=\"")).nth(1)?.split('"').next()?.parse::<f64>().ok();
                Some((g("cx")?, g("cy")?, g("r")?))
            })
            .collect();
        let big: Vec<_> = circles.iter().filter(|c| c.2 > 12.0 && c.0 < 360.0).collect();
        let small: Vec<_> = circles.iter().filter(|c| c.2 < 6.0 && c.0 < 360.0).collect();
        assert_eq!(big.len(), 24);
        assert_eq!(small.len(), 6);
        for s in &small {
            // inside a hole: four iron neighbours, none overlapping it
            let near = big.iter().filter(|b| ((b.0 - s.0).hypot(b.1 - s.1)) < 30.0).count();
            assert_eq!(near, 4, "carbon at {s:?} sits in a hole");
            assert!(big.iter().all(|b| (b.0 - s.0).hypot(b.1 - s.1) >= b.2 + s.2 - 0.5), "no overlap at {s:?}");
        }
        // gaps on / off is all that differs, and plain solids ignore the default
        let mut q = p.clone();
        q.set_b("gaps", false);
        assert_ne!((d.render)(&q), svg);
        let mut solid = Params::defaults(&defs);
        solid.set_s("state", "Solid");
        let before = (d.render)(&solid);
        solid.set_b("gaps", true);
        assert_eq!((d.render)(&solid), before, "one size of particle: nothing to put in the gaps");
    }

    #[test]
    fn chips_add_kinds_and_actions() {
        for id in ["particle-diagram", "bohr-model"] {
            let d = find(id);
            let pd = (d.params)();
            let card = chips(id).unwrap();
            assert!(card.count_key.is_empty());
            for c in &card.chips {
                for (k, v) in &c.set {
                    let def = pd.iter().find(|x| x.key == *k).unwrap_or_else(|| panic!("{id}: chip {} sets unknown {k}", c.label));
                    if let (Kind::Choice(o), Value::S(sv)) = (&def.kind, v) { assert!(o.contains(&sv.as_str()), "{id} {k}={sv}"); }
                }
            }
        }
        // Each "Add a kind" chip makes a new kind of particle appear.
        let d = find("particle-diagram");
        let mut p = Params::defaults(&(d.params)());
        let card = chips("particle-diagram").unwrap();
        let mut last = (d.render)(&p);
        for c in card.chips.iter().take(3) {
            apply(&mut p, c);
            let svg = (d.render)(&p);
            assert!(svg.len() > last.len(), "{} added nothing", c.label);
            last = svg;
        }
        assert_eq!(p.i("d_count"), 4);
        assert_eq!(p.s("c_shade"), "White");
        // Shuffle moves the particles.
        apply(&mut p, &card.chips[3]);
        assert_ne!((d.render)(&p), last);
        // Bohr "Fill" drops hand-set shells.
        let b = find("bohr-model");
        let mut q = Params::defaults(&(b.params)());
        let auto = (b.render)(&q);
        q.set_s("shells_custom", "2, 3");
        assert_ne!((b.render)(&q), auto);
        apply(&mut q, &chips("bohr-model").unwrap().chips[0]);
        assert_eq!((b.render)(&q), auto);
    }

    #[test]
    fn ids_unique_and_complete() {
        let ids: Vec<_> = defs().iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), 16);
        for d in defs() {
            for p in (d.params)() {
                match (&p.kind, &p.default) {
                    (Kind::Choice(o), Value::S(v)) => assert!(o.contains(&v.as_str()), "{} {} default not in choices", d.id, p.key),
                    _ => {}
                }
            }
        }
        let _: Option<ParamDef> = None;
    }

    #[test]
    fn instruments() {
        run("volume-reading", "cyl", &[]);
        run("volume-reading", "buret", &[("instrument", s("Buret")), ("size", s("50 mL")), ("reading", f(17.35)), ("color", s("Blue"))]);
        run("volume-reading", "beaker", &[("instrument", s("Beaker")), ("size", s("250 mL")), ("reading", f(130.0)), ("color", s("Green")), ("dotted", b(true))]);
        run("volume-reading", "cyl10", &[("size", s("10 mL")), ("reading", f(6.4)), ("color", s("Red")), ("dotted", b(true))]);
        run("volume-by-displacement", "rock", &[("object", s("Rock")), ("size", s("50 mL")), ("before", f(20.0)), ("after", f(27.0))]);
        run("volume-by-displacement", "marbles", &[]);
        run("volume-by-displacement", "cube", &[("object", s("Cube")), ("color", s("Blue"))]);
        run("gas-syringe", "full", &[]);
        run("gas-syringe", "only", &[("show", s("Syringe only")), ("size", s("50 cm³")), ("volume", f(31.5)), ("unit", s("mL"))]);
        run("length-reading", "metric", &[]);
        run("length-reading", "inch", &[("units", s("Imperial (in)")), ("object", s("Rock")), ("length", f(3.25)), ("offset", f(1.0)), ("dashes", b(true))]);
        run("length-reading", "marbles", &[("object", s("Marbles")), ("dashes", b(true)), ("offset", f(2.2)), ("length", f(5.1))]);
        run("mass-reading", "triple", &[]);
        run("mass-reading", "triple2", &[("mass", f(347.8)), ("pan", s("Rock"))]);
        run("mass-reading", "digital", &[("balance", s("Digital balance")), ("pan", s("Metal cylinder")), ("mass", f(12.345)), ("places", s("0.001"))]);
        run("temperature-reading", "c", &[]);
        run("temperature-reading", "f", &[("unit", s("Fahrenheit (°F)")), ("temperature", f(98.6)), ("liquid", s("Blue"))]);
        run("temperature-reading", "probe", &[("thermometer", s("Digital probe")), ("unit", s("Kelvin (K)")), ("temperature", f(310.2))]);
        run("ph-reading", "digital", &[]);
        run("ph-reading", "analog", &[("instrument", s("Analog meter")), ("ph", f(9.3))]);
        run("ph-reading", "paper", &[("instrument", s("pH paper")), ("ph", f(2.0))]);
    }

    #[test]
    fn curves() {
        run("titration-curve", "weak", &[]);
        run("titration-curve", "strong", &[("kind", s("Strong acid with a strong base")), ("equiv", s("Dot and lines to the axes"))]);
        run("titration-curve", "base", &[("kind", s("Weak base with a strong acid")), ("pk", f(4.75)), ("half", b(true))]);
        run("titration-curve", "sb", &[("kind", s("Strong base with a strong acid")), ("molarity", f(0.2)), ("volume", f(40.0)), ("titrant", f(0.15))]);
        run("heating-cooling-curve", "water", &[]);
        run("heating-cooling-curve", "cool", &[("mode", s("Cooling")), ("substance", s("Ethanol")), ("start", f(100.0)), ("end", f(-150.0)), ("xaxis", s("Heat added"))]);
        run("heating-cooling-curve", "nacl", &[("substance", s("Sodium chloride")), ("start", f(25.0)), ("end", f(1500.0)), ("labels", b(false))]);
    }

    #[test]
    fn atoms() {
        run("bohr-model", "c", &[]);
        run("bohr-model", "fe", &[("element", s("26 Fe - Iron")), ("placement", s("Paired")), ("nucleus", s("Protons and neutrons"))]);
        run("bohr-model", "cl-ion", &[("element", s("17 Cl - Chlorine")), ("charge", i(-1)), ("empty", b(true)), ("nucleus", s("Blank"))]);
        run("orbital-diagram", "c", &[]);
        run("orbital-diagram", "fe", &[("element", s("26 Fe - Iron")), ("layout", s("Energy ladder")), ("shorthand", b(true))]);
        run("orbital-diagram", "cu", &[("element", s("29 Cu - Copper")), ("charge", i(1))]);
        run("orbital-diagram", "blank", &[("element", s("17 Cl - Chlorine")), ("blank", b(true)), ("config", b(false))]);
        run("particle-diagram", "gas", &[]);
        run("particle-diagram", "ba", &[("boxes", s("Before and after")), ("state", s("Liquid")), ("a_count", i(6)), ("b_count", i(5)), ("a_shape", s("Pair"))]);
        run("particle-diagram", "lattice", &[("layout", s("Lattice")), ("state", s("Solid")), ("a_shape", s("Alone")), ("b_count", i(6))]);
        run("particle-diagram", "tri", &[("a_shape", s("Bent")), ("b_shape", s("Four around")), ("b_count", i(3)), ("c_count", i(2)), ("c_shape", s("In a line"))]);
    }

    #[test]
    fn lewis_and_spectra() {
        for (f_, t) in [("H2O", "h2o"), ("CO2", "co2"), ("NH4+", "nh4"), ("SO4 2-", "so4"), ("XeF4", "xef4"), ("PCl5", "pcl5"), ("BF3", "bf3"), ("SF6", "sf6"), ("HCN", "hcn"), ("O3", "o3"), ("NO2", "no2")] {
            run("lewis-structures", t, &[("formula", s(f_))]);
        }
        run("lewis-structures", "acetic", &[("preset", s("CH₃COOH, acetic acid"))]);
        run("lewis-structures", "hno3", &[("preset", s("HNO₃, nitric acid")), ("formal", b(true))]);
        run("lewis-structures", "ethene", &[("preset", s("C₂H₄, ethene"))]);
        run("line-spectrum", "emi", &[]);
        run("line-spectrum", "abs", &[("style", s("Absorption")), ("hydrogen", b(true)), ("helium", b(true)), ("brightness", s("Relative"))]);
        run("line-spectrum", "print", &[("style", s("Print")), ("mercury", b(true)), ("neon", b(true))]);
        run("photoelectron-spectrum", "c", &[]);
        run("photoelectron-spectrum", "cl", &[("element", s("17 Cl - Chlorine")), ("labels", b(false))]);
        run("mass-spectrum", "cl", &[]);
        run("mass-spectrum", "cl2", &[("molecule", b(true))]);
        run("mass-spectrum", "mg", &[("element", s("Magnesium")), ("relative", b(false))]);
        run("mass-spectrum", "custom", &[("source", s("Custom isotopes")), ("custom", s("10:20, 11:80"))]);
    }
}
