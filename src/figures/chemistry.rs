//! Chemistry figure generators (original parametric SVG, modelled on the kinds of figures a
//! chemistry teacher needs: lab-instrument readings, curves, atomic structure and spectra).
//! Submodules live in `chemistry/`; this file holds shared drawing helpers and the registry.
#![allow(dead_code)]
use super::svg::Doc;
use super::{interact, FigureDef, Params};

mod handles;
pub mod atoms;
pub mod frame;
pub mod magnify;
pub use frame::*;
mod curves;
pub mod data;
mod lewis;
mod measure;
pub mod notes;
pub mod spectra;

pub const INK: &str = "#111";
pub const GLASS: &str = "#f3f8fb";
pub const GLASS_EDGE: &str = "#56626b";
pub const FONT: &str = "Arial, Helvetica, Liberation Sans, Noto Sans, DejaVu Sans, sans-serif";

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

/// Text whose gap below a tick end (`ye`) shrinks with the type inside a magnifier.
pub fn txy(d: &mut Doc, x: f64, y: f64, ye: f64, size: f64, anchor: &str, color: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" data-ye="{ye:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{FONT}" font-weight="normal" fill="{color}">{}</text>"##,
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
        _ => ("#dcdcdc", "#8a949d"),
    }
}

/// A section help line that depends on the figure's state (`sections::live_help`): the Lewis
/// structure's central atom, the instruments' reading ranges.
pub fn live_help(id: &str, title: &str, p: &Params) -> Option<String> {
    match (id, title) {
        ("lewis-structures", "Structure") => Some(lewis::central_note(p)),
        ("lewis-structures", "Changes") if p.s("question") != "Full structure" && !lewis::has_changes(p) => {
            Some("Changes need the full structure. Set Question to Full structure to make a \u{201c}find the mistake\u{201d} question.".to_string())
        }
        ("line-spectrum", "Look") => Some(match p.s("style") {
            "Absorption" => "Dark lines across a rainbow, as a cool gas takes out of white light.",
            "Print" => "Black lines on white, for a black-and-white copier.",
            _ => "Bright colored lines on black, as a glowing gas gives off.",
        }.to_string()),
        ("bohr-model", "Atom or ion") => {
            let z = data::z_of(p.s("element"));
            let a = data::mass_number(z);
            Some(format!(
                "Picking an element sets its protons, its neutrons from its mass number ({}: {} \u{2212} {} = {}) and a neutral atom\u{2019}s shells. Changing the charge sets the shells for that ion.",
                data::symbol(z), a, z, a.saturating_sub(z)
            ))
        }
        ("volume-reading", "Reading") | ("volume-by-displacement", "Readings") | ("gas-syringe", "Volume of gas")
        | ("length-reading", "Length and position") | ("mass-reading", "Mass") | ("temperature-reading", "Temperature")
        | ("ph-reading", "pH") => measure::range_note(id, p),
        _ => None,
    }
}

/// The mass spectrum's Isotopes card note (relative atomic mass line, working).
pub fn mass_spectrum_info(p: &Params) -> Option<(String, String)> {
    spectra::ms_info(p)
}

/// The chemistry page's one-line section summaries ("100 mL graduated cylinder"), where the
/// reference words them differently from the generic "value · value" digest.
pub fn skin_summary(id: &str, title: &str, p: &Params) -> Option<String> {
    if id == "lewis-structures" {
        return lewis::skin_summary(title, p);
    }
    if let Some(s) = super::math::flat_skin_summary(id, title, p) {
        return Some(s);
    }
    if let Some(s) = super::physics::skin_summary(id, title, p) {
        return Some(s);
    }
    if id == "mass-spectrum" {
        return spectra::ms_summary(title, p);
    }
    if let Some(s) = atom_summary(id, title, p) {
        return Some(s);
    }
    if let Some(s) = super::biology::pipette::skin_summary(id, title, p) {
        return Some(s);
    }
    if let Some(s) = super::biology::gel::skin_summary(id, title, p) {
        return Some(s);
    }
    measure::skin_summary(id, title, p)
}

/// "Sodium, 1s² 2s² 2p⁶ 3s¹" style summaries of the orbital diagram and photoelectron pages.
fn atom_summary(id: &str, title: &str, p: &Params) -> Option<String> {
    use data::{config, config_ex, config_text, name, symbol, z_of};
    let charge = p.i("charge") as i64;
    let title_line = |p: &Params| format!("{}, {}", if p.s("title").trim().is_empty() { "No title" } else { "Title" }, if p.b("answer") { "answer key" } else { "no answer key" });
    match (id, title) {
        ("orbital-diagram", "Atom or ion") => {
            let z = z_of(p.s("element"));
            let ion = match charge { 0 => String::new(), c if c > 0 => format!("{}{}", if c > 1 { c.to_string() } else { String::new() }, "+"), c => format!("{}{}", if c < -1 { (-c).to_string() } else { String::new() }, "-") };
            Some(format!("{}{}, {}, {} electrons", symbol(z), ion, name(z).to_lowercase(), (z as i64 - charge).max(0)))
        }
        ("orbital-diagram", "Configuration") => {
            let z = z_of(p.s("element"));
            let subs = if p.s("exceptions") == "Real" { config(z, charge) } else { config_ex(z, charge, false) };
            Some(config_text(&subs, None))
        }
        ("orbital-diagram", "Look") => Some(format!("{}, {}", p.s("arrows"), p.s("orbitals").to_lowercase())),
        ("orbital-diagram", "For the student") => {
            let mut v: Vec<&str> = Vec::new();
            if p.b("blank") { v.push("orbitals"); }
            if p.b("config_blank") { v.push("configuration"); }
            if p.s("symbol_style") == "Blank line" { v.push("symbol"); }
            if p.s("sublabels") == "Blank lines" { v.push("sublevel labels"); }
            Some(if v.is_empty() { "Nothing left blank".to_string() } else { format!("{} left blank", v.join(", ")) })
        }
        ("orbital-diagram", "Changes") => Some(if p.s("edits").trim().is_empty() && p.i("extra_empty") == 0 { "Ground state".to_string() } else { "Changed".to_string() }),
        ("orbital-diagram", "Title and answer key") | ("photoelectron-spectrum", "Title and answer key") => Some(title_line(p)),
        ("photoelectron-spectrum", "Element") => {
            let z = z_of(p.s("element")).min(54);
            Some(format!("{}, {}", name(z), config_text(&config(z, 0), None)))
        }
        ("photoelectron-spectrum", "Energy axis") => Some(format!("{}, {}", p.s("scale"), p.s("units"))),
        ("photoelectron-spectrum", "Peaks") => {
            let mut v = vec![p.s("drawn").to_string(), match p.s("sublabel_style") { "Blank lines" => "blank labels", "None" => "no labels", _ => "labeled" }.to_string()];
            if p.b("counts") { v.push("electron counts".into()); }
            if p.b("energies") { v.push("energies".into()); }
            Some(v.join(", "))
        }
        ("photoelectron-spectrum", "For the student") => Some(format!("Element {}, electrons {}", if p.b("names") { "named" } else { "not named" }, if p.b("numbered") { "numbered" } else { "not numbered" })),
        _ => None,
    }
}

/// The chemistry page's intro line above "Start from an example", where known.
pub fn skin_intro(id: &str) -> Option<&'static str> {
    match id {
        "triangle" => Some("Make a printable triangle drawn to scale for your class."),
        "volume-reading" => Some("Make printable graduated cylinder, buret and beaker figures for chemistry tests."),
        "volume-by-displacement" => Some("Make printable water displacement figures for chemistry tests."),
        "gas-syringe" => Some("Make printable gas syringe figures for chemistry tests."),
        "length-reading" => Some("Make printable length measurement figures for chemistry tests."),
        "mass-reading" => Some("Make printable balance figures for chemistry tests."),
        "titration-curve" => Some("Make printable titration curves for chemistry tests."),
        "heating-cooling-curve" => Some("Make printable heating and cooling curves for chemistry tests."),
        "particle-diagram" => Some("Make printable particle diagrams for AP Chemistry tests."),
        "bohr-model" => Some("Make printable Bohr model diagrams for chemistry tests."),
        "lewis-structures" => Some("Make printable Lewis structures for chemistry tests."),
        "orbital-diagram" => Some("Make printable orbital diagrams for chemistry tests."),
        "line-spectrum" => Some("Make printable line spectra for chemistry tests."),
        "photoelectron-spectrum" => Some("Make printable photoelectron spectra (PES) for AP Chemistry tests."),
        "mass-spectrum" => Some("Make printable mass spectra of elements for chemistry tests."),
        "temperature-reading" => Some("Make printable thermometer figures for chemistry tests."),
        "ph-reading" => Some("Make printable pH meter figures for chemistry tests."),
        "punnett-square" => Some("Make printable Punnett squares for biology tests."),
        "population-growth" => Some("Make printable population growth graphs for biology tests."),
        "pedigree" => Some("Make printable pedigree charts for biology tests."),
        "predator-prey" => Some("Make printable predator–prey graphs for biology tests."),
        "gel-electrophoresis" => Some("Make printable gel electrophoresis results for biology tests."),
        "micropipette-reading" => Some("Make printable micropipette figures for biology tests."),
        "coordinate-grid" => Some("Make a printable coordinate grid (graph paper) for your class."),
        "number-line" => Some("Make a printable number line for your class."),
        "rectangle" => Some("Make a printable rectangle or square drawn to scale for your class."),
        "parallelogram" => Some("Make a printable parallelogram or rhombus drawn to scale for your class."),
        "trapezoid" => Some("Make a printable trapezoid drawn to scale for your class: a right, isosceles or scalene trapezoid, from its bases and height."),
        "kite" => Some("Make a printable kite drawn to scale for your class."),
        "regular-polygon" => Some("Make a printable regular polygon for your class, from an equilateral triangle to a 20-gon."),
        "prism" => Some("Make a printable prism drawn to scale for your class."),
        "cylinder" => Some("Make a printable cylinder drawn to scale for your class."),
        "pyramid" => Some("Make a printable pyramid drawn to scale for your class."),
        "cone" => Some("Make a printable cone drawn to scale for your class."),
        "sphere" => Some("Make a printable sphere or hemisphere for your class."),
        "box-plot" => Some("Make a printable box plot for your class."),
        "mapping-diagram" => Some("Make a printable mapping diagram for your class."),
        "ruler-reading" => Some("Make a printable ruler figure for your class."),
        "angles" => Some("Make a printable angle diagram for your class: a single angle of any size, adjacent angles, angles on a straight line, angles around a point or vertical angles."),
        "parallel-lines" => Some("Make a printable diagram of parallel lines cut by a transversal for your class."),
        "waves" => Some("Make printable wave diagrams for physics tests."),
        "motion-graphs" => Some("Make printable motion graphs for physics tests."),
        "spring-scale" => Some("Make printable spring scale figures for physics tests."),
        "circuit-diagram" => Some("Make printable circuit diagrams for physics tests."),
        "free-body-diagram" => Some("Make printable free body diagrams for physics tests."),
        "vector-diagram" => Some("Make printable vector diagrams for physics tests."),
        "inclined-plane" => Some("Make printable inclined plane figures for physics tests."),
        "pulley" => Some("Make printable pulley figures for physics tests."),
        "projectile-motion" => Some("Make printable projectile motion figures for physics tests."),
        _ => None,
    }
}

/// The Random button of the instrument readings.
pub fn random_reading(id: &str, p: &mut Params, seed: u64) -> bool {
    measure::random_reading(id, p, seed)
}

pub use lewis::{answer_available as lewis_answer_available, changes_count as lewis_changes_count, mistake_lines as lewis_mistakes, central_choices as lewis_central_choices, edit_rows as lewis_edit_rows, has_changes as lewis_has_changes, relevant as lewis_relevant, set_edit as lewis_set_edit};
pub use curves::{tit_end_note, tit_readout, tit_relevant, tit_section_summary};

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
pub const CURVE_COLORS: [&str; 6] = ["#2563eb", "#c0392b", "#2e8b57", "#8e44ad", "#d68910", "#1b1b1b"];

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

    /// The strip cards read and write the strip list: a round trip draws the same spectrum for
    /// the opening figure, every example and an old ticked-elements figure; card edits (element
    /// select, Edit lines, reorder, remove, mixtures) land in the list.
    #[test]
    fn line_spectrum_strip_cards() {
        use spectra::{set_strip_cards, strip_cards, strip_outside, StripCard as C};
        let d = find("line-spectrum");
        let defs = (d.params)();
        let mut cases = vec![crate::figures::start_params(&d), Params::defaults(&defs)];
        for ex in examples("line-spectrum").unwrap() {
            let mut p = Params::defaults(&defs);
            for (k, v) in &ex.params { p.0.insert(k.to_string(), v.clone()); }
            cases.push(p);
        }
        let mut old = Params::defaults(&defs);
        old.set_b("neon", true);
        old.set_b("mercury", true);
        cases.push(old.clone());
        for p in &cases {
            let cards = strip_cards(p);
            let mut q = p.clone();
            set_strip_cards(&mut q, &cards);
            assert_eq!((d.render)(&q), (d.render)(p), "{:?}", p.s("strips"));
            assert_eq!(strip_cards(&q), cards);
        }
        // an old figure (ticks, no strip list) becomes cards and moves into the list on edit
        assert_eq!(strip_cards(&old), vec![C::Element("Hydrogen".into()), C::Element("Neon".into()), C::Element("Mercury".into())]);
        let p0 = crate::figures::start_params(&d);
        let mut c = strip_cards(&p0);
        assert_eq!(c[3], C::Mixture { name: "Unknown".into(), parts: vec!["Hydrogen".into(), "Sodium".into()] });
        c[1] = C::Element("Lithium".into());
        c.swap(0, 2); // Sodium first
        c[2] = C::Own { name: "Hydrogen".into(), lines: spectra::element_lines_text("Hydrogen") }; // "Edit lines"
        let mut p = p0.clone();
        set_strip_cards(&mut p, &c);
        assert_eq!(p.s("strips"), "Sodium; Lithium; Hydrogen: 410.2, 434.0, 486.1, 656.3; Unknown = Hydrogen + Sodium");
        // the edited copy of hydrogen draws the same lines as the element
        assert_eq!(strip_cards(&p)[2], c[2]);
        c.remove(1);
        set_strip_cards(&mut p, &c);
        assert_eq!(strip_cards(&p).len(), 3);
        // separators typed into names can't split or re-type a strip
        set_strip_cards(&mut p, &[C::Own { name: "X;=:".into(), lines: "450; 500".into() }, C::Mixture { name: "M".into(), parts: vec!["X".into(), "Helium".into()] }]);
        assert_eq!(strip_cards(&p), vec![C::Own { name: "X".into(), lines: "450  500".into() }, C::Mixture { name: "M".into(), parts: vec!["X".into(), "Helium".into()] }]);
        // lines outside the axis are counted per strip
        let mut q = p0.clone();
        assert_eq!(strip_outside(&q, 1), 0);
        q.set_f("nm_to", 650.0);
        assert_eq!(strip_outside(&q, 0), 1); // hydrogen 656.3
        assert_eq!(strip_outside(&q, 1), 1); // helium 667.8
    }

    /// Particle kinds as cards: nouns and summaries as on the reference, removing a kind moves the
    /// later ones up, a reset kind starts as 4 lone medium white particles.
    #[test]
    fn particle_kind_cards() {
        use atoms::*;
        let d = find("particle-diagram");
        let defs = (d.params)();
        let mut p = crate::figures::start_params(&d);
        assert_eq!((0..4).filter(|&i| kind_used(&p, i)).count(), 2);
        assert_eq!((kind_noun(&p, 0), kind_noun(&p, 1)), ("Ion", "Ion"));
        p.set_s("a_charge", "None");
        assert_eq!(kind_noun(&p, 0), "Atom");
        p.set_s("a_shape", "Pair");
        assert_eq!(kind_noun(&p, 0), "Molecule");
        assert_eq!(kind_summary(&p, 0), "4 molecules");
        reset_kind(&mut p, 2);
        assert!(kind_used(&p, 2));
        assert_eq!(kind_summary(&p, 2), "4 medium white atoms");
        let before_b = (p.s("b_size").to_string(), p.s("b_charge").to_string());
        remove_kind(&mut p, &defs, 0);
        assert_eq!((p.s("a_size").to_string(), p.s("a_charge").to_string()), before_b);
        assert_eq!(p.s("b_shade"), "White");
        assert!(!kind_used(&p, 2) && !kind_used(&p, 3));
        // what is left draws as the same two kinds set up by hand
        let mut q = Params::defaults(&defs);
        for k in ["a", "b", "c", "d"] { q.set_i(&format!("{k}_count"), 0); q.set_i(&format!("{k}2_count"), 0); }
        for key in ["count", "shape", "size", "shade", "charge", "osize", "oshade", "ocharge", "name", "oname"] {
            for (dst, src) in [("a", "a"), ("b", "b")] {
                let v = p.0.get(&format!("{src}_{key}")).cloned().unwrap();
                q.0.insert(format!("{dst}_{key}"), v);
            }
        }
        for k in ["a", "b"] { q.set_i(&format!("{k}2_count"), p.i(&format!("{k}2_count"))); }
        q.set_i("shuffle", p.i("shuffle"));
        assert_eq!((d.render)(&q), (d.render)(&p));
        assert!(is_kind_key("c2_count") && is_kind_key("d_oname") && !is_kind_key("shuffle") && !is_kind_key("key_show"));
    }

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
        let small: Vec<_> = circles.iter().filter(|c| c.2 < 7.0 && c.0 < 360.0).collect();
        assert_eq!(big.len(), 24);
        assert_eq!(small.len(), 6);
        for s in &small {
            // inside a hole: four iron neighbours, none overlapping it
            let near = big.iter().filter(|b| ((b.0 - s.0).hypot(b.1 - s.1)) < 45.0).count();
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
