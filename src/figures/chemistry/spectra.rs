//! Line (atomic) spectra, photoelectron spectra and mass spectra.
use super::data::*;
use super::*;
use crate::figures::{ParamDef, Params, Subject};

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "line-spectrum", subject: Subject::Chemistry, title: "Line Spectrum",
            blurb: "Atomic emission or absorption line spectra for common elements, on a 400-700 nm scale.",
            params: line_params, render: line_render },
        FigureDef { id: "photoelectron-spectrum", subject: Subject::Chemistry, title: "Photoelectron Spectrum",
            blurb: "PES peaks for the first 18 elements: one peak per subshell, height set by the electron count.",
            params: pes_params, render: pes_render },
        FigureDef { id: "mass-spectrum", subject: Subject::Chemistry, title: "Mass Spectrum",
            blurb: "Isotope abundance bars for an element, a diatomic molecule of it, or your own isotope list.",
            params: ms_params, render: ms_render },
    ]
}

// ---------------------------------------------------------------------------------------------
// Line spectrum
// ---------------------------------------------------------------------------------------------

type Lines = &'static [(f64, f64)];
/// (bool key for the old per-element toggles, name, symbol, visible lines (nm, relative brightness)).
const SPECTRA: [(&str, &str, &str, Lines); 15] = [
    ("hydrogen", "Hydrogen", "H", &[(410.2, 0.3), (434.0, 0.45), (486.1, 0.8), (656.3, 1.0)]),
    ("helium", "Helium", "He", &[(447.1, 0.6), (471.3, 0.3), (492.2, 0.3), (501.6, 0.8), (587.6, 1.0), (667.8, 0.5)]),
    ("lithium", "Lithium", "Li", &[(460.3, 0.3), (610.4, 0.4), (670.8, 1.0)]),
    ("sodium", "Sodium", "Na", &[(568.8, 0.3), (589.0, 1.0), (589.6, 1.0), (615.4, 0.2), (616.1, 0.2)]),
    ("potassium", "Potassium", "K", &[(404.4, 0.3), (404.7, 0.3), (691.1, 0.4), (693.9, 0.5), (766.5, 1.0), (769.9, 0.9)]),
    ("calcium", "Calcium", "Ca", &[(422.7, 1.0), (445.5, 0.3), (558.9, 0.3), (616.2, 0.5), (643.9, 0.6)]),
    ("strontium", "Strontium", "Sr", &[(460.7, 1.0), (483.2, 0.3), (496.2, 0.4), (548.0, 0.3), (640.8, 0.5), (650.4, 0.4), (687.8, 0.4), (707.0, 0.4)]),
    ("barium", "Barium", "Ba", &[(455.4, 1.0), (493.4, 0.8), (553.5, 1.0), (577.8, 0.4), (614.2, 0.6), (649.7, 0.5), (652.7, 0.3), (659.5, 0.4)]),
    ("copper", "Copper", "Cu", &[(465.1, 0.4), (510.6, 0.7), (515.3, 0.8), (521.8, 1.0), (570.0, 0.3), (578.2, 0.5)]),
    ("zinc", "Zinc", "Zn", &[(468.0, 0.5), (472.2, 0.6), (481.1, 0.8), (518.2, 0.3), (636.2, 1.0)]),
    ("cadmium", "Cadmium", "Cd", &[(467.8, 0.6), (480.0, 0.8), (508.6, 1.0), (643.8, 1.0)]),
    ("mercury", "Mercury", "Hg", &[(404.7, 0.6), (435.8, 1.0), (546.1, 1.0), (577.0, 0.6), (579.1, 0.6)]),
    ("neon", "Neon", "Ne", &[(585.2, 0.7), (588.2, 0.5), (603.0, 0.4), (614.3, 0.6), (626.6, 0.5), (640.2, 1.0), (650.6, 0.6), (692.9, 0.5)]),
    ("argon", "Argon", "Ar", &[(415.9, 0.4), (420.1, 0.4), (427.2, 0.3), (434.8, 0.5), (451.1, 0.3), (696.5, 0.6), (706.7, 0.5), (738.4, 0.5), (750.4, 1.0), (763.5, 0.8)]),
    ("krypton", "Krypton", "Kr", &[(427.4, 0.4), (431.9, 0.4), (436.3, 0.3), (446.4, 0.3), (557.0, 0.8), (587.1, 1.0), (645.6, 0.3), (758.7, 0.5), (760.2, 0.6)]),
];
/// The per-element toggles that existed before the strip list (kept for saved figures).
const OLD_KEYS: [(&str, &str); 7] = [("hydrogen", "Hydrogen"), ("helium", "Helium"), ("neon", "Neon"), ("sodium", "Sodium"), ("mercury", "Mercury"), ("lithium", "Lithium"), ("calcium", "Calcium")];

fn line_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("style", "Style", &["Emission", "Absorption", "Print"], "Emission"),
        ParamDef::choice("brightness", "Brightness", &["All equal", "Relative"], "All equal"),
        ParamDef::bool("labels", "Label wavelengths", false),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::list("custom", "Extra custom row: wavelengths in nm (nm or nm:brightness)", ""),
    ];
    for (k, l) in OLD_KEYS { v.push(ParamDef::bool(k, l, k == "hydrogen")); }
    v.extend([
        ParamDef::list("strips", "Strips, one per row (overrides the ticks above): Hydrogen | Unknown = Hydrogen + Sodium | Element X: 425 510:0.5 630", ""),
        ParamDef::choice("label_style", "Labels", &["Names", "Symbols", "Blank", "None"], "Names"),
        ParamDef::float("nm_from", "Wavelength axis from (nm)", 300.0, 900.0, 5.0, 400.0),
        ParamDef::float("nm_to", "Wavelength axis to (nm)", 300.0, 900.0, 5.0, 700.0),
        ParamDef::choice("tick_every", "Small ticks every", &["No small ticks", "5 nm", "10 nm", "20 nm", "25 nm", "50 nm"], "10 nm"),
        ParamDef::choice("num_every", "Number every", &["10 nm", "20 nm", "25 nm", "50 nm", "100 nm"], "50 nm"),
        ParamDef::choice("axis_pos", "Axis", &["Under the last", "Under each", "None"], "Under the last"),
        ParamDef::choice("axis_title_style", "Axis title", &TITLE2, "Text"),
        ParamDef::text("axis_title", "Axis title text", "Wavelength (nm)"),
        title_style(false),
        answer_param("Answer key (what each mixture contains)"),
    ]);
    v
}

fn spectrum_of(name: &str) -> Option<&'static (&'static str, &'static str, &'static str, Lines)> {
    let n = name.trim();
    SPECTRA.iter().find(|e| e.1.eq_ignore_ascii_case(n) || e.2 == n || e.0.eq_ignore_ascii_case(n))
}

struct Strip { name: String, sym: String, lines: Vec<(f64, f64)>, parts: Vec<String>, custom: bool }

/// Rows drawn: the strip list if given, else the ticked elements and the optional custom row.
fn line_rows(p: &Params) -> Vec<Strip> {
    let spec = p.s("strips").trim();
    if spec.is_empty() {
        let mut rows: Vec<Strip> = OLD_KEYS.iter().filter(|k| p.b(k.0)).filter_map(|k| spectrum_of(k.1)).map(|e| Strip { name: e.1.into(), sym: e.2.into(), lines: e.3.to_vec(), parts: vec![], custom: false }).collect();
        let custom = custom_lines(p.s("custom"));
        if !custom.is_empty() { rows.push(Strip { name: "Custom".into(), sym: "?".into(), lines: custom, parts: vec![], custom: true }); }
        return rows;
    }
    let mut rows: Vec<Strip> = Vec::new();
    for raw in spec.split(|c| c == ';' || c == '\n') {
        let t = raw.trim();
        if t.is_empty() { continue; }
        if let Some((name, rest)) = t.split_once('=') {
            let mut lines: Vec<(f64, f64)> = Vec::new();
            let mut parts = Vec::new();
            for comp in rest.split(|c| c == '+' || c == ',' || c == '&').flat_map(|c| c.split(" and ")) {
                let c = comp.trim();
                if c.is_empty() { continue; }
                let found = rows.iter().find(|r| r.name.eq_ignore_ascii_case(c)).map(|r| (r.name.clone(), r.lines.clone()))
                    .or_else(|| spectrum_of(c).map(|e| (e.1.to_string(), e.3.to_vec())));
                if let Some((n, l)) = found { parts.push(n); lines.extend(l); }
            }
            rows.push(Strip { name: name.trim().into(), sym: "?".into(), lines, parts, custom: false });
        } else if let Some((name, rest)) = t.split_once(':') {
            rows.push(Strip { name: name.trim().into(), sym: name.split_whitespace().last().filter(|w| w.chars().count() <= 2).map(String::from).unwrap_or_else(|| name.trim().chars().take(2).collect()), lines: custom_lines(rest), parts: vec![], custom: true });
        } else if let Some(e) = spectrum_of(t) {
            rows.push(Strip { name: e.1.into(), sym: e.2.into(), lines: e.3.to_vec(), parts: vec![], custom: false });
        } else {
            rows.push(Strip { name: t.into(), sym: "?".into(), lines: vec![], parts: vec![], custom: true });
        }
    }
    rows
}

// ---------------------------------------------------------------- strip cards
//
// The side panel edits the strips as cards (element select, your own lines, mixtures, move up /
// down, remove, "Edit lines"). The cards read and write the `strips` list the generator parses;
// a figure saved before the strip list (per-element ticks + a custom row) is copied into it on
// the first card edit.

/// One strip card.
#[derive(Clone, Debug, PartialEq)]
pub enum StripCard {
    /// A built-in element, by name ("Hydrogen").
    Element(String),
    /// Your own lines: a name and the typed wavelengths ("410.2, 434, 486:0.5").
    Own { name: String, lines: String },
    /// A mixture: a name and the strips it is made of (names of earlier strips or elements).
    Mixture { name: String, parts: Vec<String> },
}

impl StripCard {
    /// The name the strip is drawn with (and that mixtures refer to).
    pub fn name(&self) -> &str {
        match self { StripCard::Element(n) => n, StripCard::Own { name, .. } | StripCard::Mixture { name, .. } => name }
    }
}

/// The elements a strip can show: (symbol, name).
pub fn strip_elements() -> Vec<(&'static str, &'static str)> {
    SPECTRA.iter().map(|e| (e.2, e.1)).collect()
}

/// An element's lines as the "Edit lines" text ("410.2, 434, 486.1, 656.3").
pub fn element_lines_text(name: &str) -> String {
    spectrum_of(name).map(|e| e.3.iter().map(|(nm, _)| format!("{nm:.1}")).collect::<Vec<_>>().join(", ")).unwrap_or_default()
}

/// The strips as cards (the strip list, or for older figures the ticked elements and custom row).
pub fn strip_cards(p: &Params) -> Vec<StripCard> {
    let spec = p.s("strips").trim();
    if spec.is_empty() {
        let mut v: Vec<StripCard> = OLD_KEYS.iter().filter(|k| p.b(k.0)).map(|k| StripCard::Element(k.1.to_string())).collect();
        if !custom_lines(p.s("custom")).is_empty() {
            v.push(StripCard::Own { name: "Custom".into(), lines: p.s("custom").trim().to_string() });
        }
        return v;
    }
    spec.split([';', '\n'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            if let Some((name, rest)) = t.split_once('=') {
                let parts = rest.split(['+', ',', '&']).flat_map(|c| c.split(" and ")).map(str::trim).filter(|c| !c.is_empty()).map(String::from).collect();
                StripCard::Mixture { name: name.trim().into(), parts }
            } else if let Some((name, rest)) = t.split_once(':') {
                StripCard::Own { name: name.trim().into(), lines: rest.trim().into() }
            } else if let Some(e) = spectrum_of(t) {
                StripCard::Element(e.1.to_string())
            } else {
                StripCard::Own { name: t.into(), lines: String::new() }
            }
        })
        .collect()
}

/// Writes the cards back as the strip list.
pub fn set_strip_cards(p: &mut Params, cards: &[StripCard]) {
    let clean = |s: &str, bad: &[char]| s.chars().map(|c| if c == ';' || c == '\n' || bad.contains(&c) { ' ' } else { c }).collect::<String>().trim().to_string();
    let text = cards
        .iter()
        .map(|c| match c {
            StripCard::Element(n) => n.clone(),
            StripCard::Own { name, lines } => format!("{}: {}", clean(name, &[':', '=']), clean(lines, &['='])).trim_end().to_string(),
            StripCard::Mixture { name, parts } => {
                let parts: Vec<String> = parts.iter().map(|x| clean(x, &['+', ',', '&', '=', ':'])).filter(|x| !x.is_empty()).collect();
                format!("{} = {}", clean(name, &[':', '=']), parts.join(" + ")).trim_end().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    p.set_s("strips", &text);
}

/// How many of strip `i`'s lines fall outside the wavelength axis (and so aren't drawn).
pub fn strip_outside(p: &Params, i: usize) -> usize {
    let rows = line_rows(p);
    let lay = line_lay(p, &rows);
    rows.get(i).map(|r| r.lines.iter().filter(|(nm, _)| *nm < lay.lo || *nm > lay.hi).count()).unwrap_or(0)
}

/// The axis range strips are drawn over (nm).
pub fn strip_axis(p: &Params) -> (f64, f64) {
    let rows = line_rows(p);
    let lay = line_lay(p, &rows);
    (lay.lo, lay.hi)
}

/// "656.3, 486:0.5" (commas or spaces) -> [(nm, brightness)].
fn custom_lines(s: &str) -> Vec<(f64, f64)> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter_map(|t| {
        let mut it = t.trim().split(':');
        let nm = it.next()?.trim().parse::<f64>().ok()?;
        let b = it.next().and_then(|b| b.trim().parse::<f64>().ok()).unwrap_or(1.0);
        Some((nm, b.clamp(0.0, 1.0)))
    }).collect()
}

/// Layout: (label column x of the strip start, strip end, first strip top, strip height, pitch, axis under each).
struct LineLay { x0: f64, x1: f64, top: f64, sh: f64, pitch: f64, each: bool, lo: f64, hi: f64, w: f64, h: f64 }

fn line_lay(p: &Params, rows: &[Strip]) -> LineLay {
    let labw = match p.s("label_style") {
        "Symbols" => 56.0,
        "Blank" => 140.0,
        "None" => 12.0,
        _ => rows.iter().map(|r| r.name.chars().count()).max().unwrap_or(6) as f64 * 11.0 + 26.0,
    };
    let x0 = 14.0 + labw;
    let x1 = x0 + 620.0;
    let axis = p.s("axis_pos");
    let each = axis == "Under each";
    let ax_h = if axis == "None" { 0.0 } else { 30.0 + if p.s("axis_title_style") == "None" { 0.0 } else { 26.0 } };
    let lab_extra = if p.b("labels") { 30.0 } else { 0.0 };
    let sh = 46.0;
    let pitch = sh + 16.0 + lab_extra + if each { ax_h } else { 0.0 };
    let top = 14.0 + lab_extra;
    let n = rows.len().max(1) as f64;
    let h = top + pitch * n - 16.0 - if each { ax_h } else { 0.0 } + ax_h + 18.0;
    let (mut lo, mut hi) = (p.f("nm_from"), p.f("nm_to"));
    if hi <= lo + 10.0 { lo = 400.0; hi = 700.0; }
    LineLay { x0, x1, top, sh, pitch, each, lo, hi, w: x1 + 30.0, h }
}

fn line_render(p: &Params) -> String {
    let rows = line_rows(p);
    let lay = line_lay(p, &rows);
    let (x0, x1, sh, lo, hi) = (lay.x0, lay.x1, lay.sh, lay.lo, lay.hi);
    let style = p.s("style");
    let rel = p.s("brightness") == "Relative";
    let mut d = Doc::new(lay.w, lay.h);
    let px = |nm: f64| x0 + (nm - lo) / (hi - lo) * (x1 - x0);
    let nmv = |s: &str| s.trim_end_matches(" nm").parse::<f64>().unwrap_or(0.0);
    let tick_every = nmv(p.s("tick_every"));
    let num_every = nmv(p.s("num_every")).max(5.0);
    let axis = |d: &mut Doc, ay: f64| {
        d.line(x0, ay, x1, ay, INK, 1.4);
        if tick_every > 0.0 {
            let mut v = (lo / tick_every).ceil() * tick_every;
            while v <= hi + 1e-9 { d.line(px(v), ay, px(v), ay + 5.0, INK, 1.0); v += tick_every; }
        }
        let mut v = (lo / num_every).ceil() * num_every;
        while v <= hi + 1e-9 {
            d.line(px(v), ay, px(v), ay + 9.0, INK, 1.4);
            tx(d, px(v), ay + 24.0, 14.0, "middle", INK, false, &tick(v));
            v += num_every;
        }
        if p.s("axis_title_style") != "None" && !p.s("axis_title").trim().is_empty() {
            tx(d, (x0 + x1) / 2.0, ay + 48.0, 15.0, "middle", INK, true, p.s("axis_title"));
        }
    };
    for (r, e) in rows.iter().enumerate() {
        let y = lay.top + r as f64 * lay.pitch;
        match p.s("label_style") {
            "Symbols" => { tx(&mut d, x0 - 12.0, y + sh / 2.0 + 6.0, 18.0, "end", INK, true, &e.sym); }
            "Blank" => { d.line(16.0, y + sh / 2.0 + 6.0, x0 - 14.0, y + sh / 2.0 + 6.0, INK, 1.4); }
            "None" => {}
            _ => { tx(&mut d, x0 - 12.0, y + sh / 2.0 + 6.0, 18.0, "end", INK, true, &e.name); }
        }
        match style {
            "Emission" => { d.rect(x0, y, x1 - x0, sh, "#000", "#333", 1.0); }
            "Absorption" => {
                let steps = 300;
                for k in 0..steps {
                    let nm = lo + (hi - lo) * (k as f64 + 0.5) / steps as f64;
                    let wpx = (x1 - x0) / steps as f64;
                    d.rect(x0 + k as f64 * wpx, y, wpx + 0.6, sh, &hex(wavelength_rgb(nm)), "none", 0.0);
                }
                d.rect(x0, y, x1 - x0, sh, "none", "#333", 1.0);
            }
            _ => { d.rect(x0, y, x1 - x0, sh, "#fff", INK, 1.4); }
        }
        let mut last_label = -100.0;
        let mut ls = e.lines.clone();
        ls.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(nm, inten) in ls.iter() {
            if !(lo..=hi).contains(&nm) { continue; }
            let x = px(nm);
            let o = if rel { 0.25 + 0.75 * inten } else { 1.0 };
            let col = match style { "Emission" => hex(wavelength_rgb(nm)), _ => "#000000".into() };
            d.raw(&format!(r#"<line x1="{x:.2}" y1="{y:.2}" x2="{x:.2}" y2="{:.2}" stroke="{col}" stroke-width="{}" stroke-opacity="{o:.2}"/>"#, y + sh, if style == "Print" { 2.2 } else { 2.6 }));
            if p.b("labels") && px(nm) - px(last_label) > 11.0 {
                tr(&mut d, x + 4.0, y - 4.0, 10.0, -90.0, "start", &format!("{nm}"));
                last_label = nm;
            }
        }
        if lay.each { axis(&mut d, y + sh); }
    }
    if p.s("axis_pos") == "Under the last" && !rows.is_empty() {
        axis(&mut d, lay.top + (rows.len() - 1) as f64 * lay.pitch + sh);
    }
    let mut foot = Vec::new();
    if p.b("answer") {
        for r in rows.iter().filter(|r| !r.parts.is_empty()) {
            let names: Vec<String> = r.parts.iter().map(|n| if spectrum_of(n).is_some() { n.to_lowercase() } else { n.clone() }).collect();
            let list = match names.len() { 1 => names[0].clone(), n => format!("{} and {}", names[..n - 1].join(", "), names[n - 1]) };
            foot.push(format!("{}: {list}", r.name));
        }
    }
    wrap(p, d.finish(), &foot)
}

// ---------------------------------------------------------------------------------------------
// PES
// ---------------------------------------------------------------------------------------------

const PES_COMPARE: [&str; 55] = ["None", "1 H - Hydrogen", "2 He - Helium", "3 Li - Lithium", "4 Be - Beryllium", "5 B - Boron", "6 C - Carbon",
    "7 N - Nitrogen", "8 O - Oxygen", "9 F - Fluorine", "10 Ne - Neon", "11 Na - Sodium", "12 Mg - Magnesium",
    "13 Al - Aluminum", "14 Si - Silicon", "15 P - Phosphorus", "16 S - Sulfur", "17 Cl - Chlorine", "18 Ar - Argon",
    "19 K - Potassium", "20 Ca - Calcium", "21 Sc - Scandium", "22 Ti - Titanium", "23 V - Vanadium", "24 Cr - Chromium",
    "25 Mn - Manganese", "26 Fe - Iron", "27 Co - Cobalt", "28 Ni - Nickel", "29 Cu - Copper", "30 Zn - Zinc",
    "31 Ga - Gallium", "32 Ge - Germanium", "33 As - Arsenic", "34 Se - Selenium", "35 Br - Bromine", "36 Kr - Krypton",
    "37 Rb - Rubidium", "38 Sr - Strontium", "39 Y - Yttrium", "40 Zr - Zirconium", "41 Nb - Niobium", "42 Mo - Molybdenum",
    "43 Tc - Technetium", "44 Ru - Ruthenium", "45 Rh - Rhodium", "46 Pd - Palladium", "47 Ag - Silver", "48 Cd - Cadmium",
    "49 In - Indium", "50 Sn - Tin", "51 Sb - Antimony", "52 Te - Tellurium", "53 I - Iodine", "54 Xe - Xenon"];

fn pes_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("element", "Element (Z = 1-54)", &ELEMENT_CHOICES[..54], "6 C - Carbon"),
        ParamDef::bool("labels", "Sublevel labels on the peaks", true),
        ParamDef::bool("counts", "Electron counts over the peaks (like 2p⁶)", false),
        ParamDef::choice("source", "Peaks from", &["Element", "Custom peaks"], "Element"),
        ParamDef::list("custom", "Custom peaks (energy MJ/mol : electrons : label)", "28.6:2:1s, 1.72:2:2s, 1.09:2:2p"),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::choice("compare", "Compare with (drawn dashed behind it)", &PES_COMPARE, "None"),
        ParamDef::choice("scale", "Energy axis scale", &["Logarithmic", "Broken", "Linear"], "Logarithmic"),
        ParamDef::choice("units", "Energy units", &["MJ/mol", "eV"], "MJ/mol"),
        ParamDef::bool("gridlines", "Gridlines at each number of electrons", true),
        ParamDef::choice("drawn", "Drawn as", &["Peaks", "Bars"], "Peaks"),
        ParamDef::choice("sublabel_style", "Sublevel labels as", &["Shown", "Blank lines", "None"], "Shown"),
        ParamDef::bool("energies", "Binding energies over the peaks", false),
        ParamDef::bool("names", "Element name over the spectrum", true),
        ParamDef::bool("numbered", "Numbered electrons axis", true),
        title_style(false),
        answer_param("Answer key (electron configuration)"),
    ]
}

/// Peaks as (label, electrons, binding energy MJ/mol).
fn pes_peaks(p: &Params) -> Vec<(String, u8, f64)> {
    if p.s("source") == "Custom peaks" {
        let v: Vec<(String, u8, f64)> = p.s("custom").split(|c| c == ',' || c == ';').filter_map(|t| {
            let parts: Vec<&str> = t.trim().split(':').collect();
            let en = parts.first()?.trim().parse::<f64>().ok()?;
            let e = parts.get(1)?.trim().parse::<f64>().ok()?.round().clamp(1.0, 14.0) as u8;
            Some((parts.get(2).map(|l| l.trim().to_string()).unwrap_or_default(), e, en.clamp(0.05, 2000.0)))
        }).collect();
        if !v.is_empty() { return v; }
    }
    element_peaks(z_of(p.s("element")).min(54))
}

fn element_peaks(z: usize) -> Vec<(String, u8, f64)> {
    pes_energies(z).into_iter().map(|(code, e, en)| (format!("{}{}", code / 10, ['s', 'p', 'd', 'f'][(code % 10) as usize]), e, en)).collect()
}

const PES_W: f64 = 720.0;
const PES_H: f64 = 470.0;

#[derive(Clone)]
enum PesScale { Log(f64, f64), Lin(f64, f64), Broken(Vec<(f64, f64, f64, f64)>) }

/// Axis geometry; energies are in display units (MJ/mol or eV).
#[derive(Clone)]
struct PesGeom { x0: f64, x1: f64, y0: f64, y1: f64, ymax: f64, k: f64, scale: PesScale }

impl PesGeom {
    fn px(&self, e: f64) -> f64 {
        match &self.scale {
            PesScale::Log(lo, hi) => self.x0 + (hi.ln() - e.max(1e-9).ln()) / (hi.ln() - lo.ln()) * (self.x1 - self.x0),
            PesScale::Lin(lo, hi) => self.x0 + (hi - e) / (hi - lo) * (self.x1 - self.x0),
            PesScale::Broken(g) => {
                for &(lo, hi, xa, xb) in g { if e >= lo && e <= hi { return xa + (hi - e) / (hi - lo) * (xb - xa); } }
                -1000.0
            }
        }
    }
    fn inv(&self, x: f64) -> Option<f64> {
        match &self.scale {
            PesScale::Log(lo, hi) => Some((hi.ln() - (x - self.x0) / (self.x1 - self.x0) * (hi.ln() - lo.ln())).exp()),
            PesScale::Lin(lo, hi) => Some(hi - (x - self.x0) / (self.x1 - self.x0) * (hi - lo)),
            PesScale::Broken(_) => None,
        }
    }
    fn py(&self, n: f64) -> f64 { self.y1 - n / self.ymax * (self.y1 - self.y0) }
}

fn pes_geom(p: &Params, peaks: &[(String, u8, f64)], extra: &[(String, u8, f64)]) -> PesGeom {
    let k = if p.s("units") == "eV" { 1.0 / 0.096485 } else { 1.0 };
    let all: Vec<f64> = peaks.iter().chain(extra.iter()).map(|x| x.2 * k).collect();
    let emax = all.iter().cloned().fold(0.0, f64::max);
    let emin = all.iter().cloned().fold(f64::MAX, f64::min);
    let maxe = peaks.iter().chain(extra.iter()).map(|x| x.1).max().unwrap_or(2) as f64;
    let ymax = (((maxe + 2.0) / 2.0).ceil() * 2.0).max(8.0);
    let (x0, x1, y0, y1) = (if p.b("numbered") { 80.0 } else { 62.0 }, PES_W - 30.0, 60.0, PES_H - 90.0);
    let scale = match p.s("scale") {
        "Linear" => { let st = super::frame::nice(emax * 1.08, 8.0); PesScale::Lin(0.0, (emax * 1.08 / st).ceil() * st) }
        "Broken" => {
            let mut es = all.clone();
            es.sort_by(|a, b| b.total_cmp(a));
            es.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            let mut groups: Vec<Vec<f64>> = Vec::new();
            for e in es {
                match groups.last_mut() { Some(g) if e > g.last().unwrap() / 2.2 => g.push(e), _ => groups.push(vec![e]) }
            }
            let n = groups.len().max(1) as f64;
            let gap = 30.0;
            let gw = (x1 - x0 - gap * (n - 1.0)) / n;
            PesScale::Broken(groups.iter().enumerate().map(|(i, g)| {
                let (hi, lo) = (g[0], *g.last().unwrap());
                let pad = ((hi - lo) * 0.3).max(hi * 0.06);
                let xa = x0 + i as f64 * (gw + gap);
                (lo - pad, hi + pad, xa, xa + gw)
            }).collect())
        }
        _ => PesScale::Log(10f64.powf((emin / 1.5).log10().floor()), 10f64.powf((emax * 1.5).log10().ceil())),
    };
    PesGeom { x0, x1, y0, y1, ymax, k, scale }
}

fn thousands(v: f64) -> String {
    let t = tick(v);
    if v >= 1000.0 && !t.contains('.') {
        let b: Vec<char> = t.chars().collect();
        let mut out = String::new();
        for (i, c) in b.iter().enumerate() { if i > 0 && (b.len() - i) % 3 == 0 { out.push(','); } out.push(*c); }
        out
    } else { t }
}

fn pes_curve(g: &PesGeom, peaks: &[(String, u8, f64)]) -> Vec<(f64, f64)> {
    let sigma = 5.5;
    let samples = 900;
    (0..=samples).map(|i| {
        let x = g.x0 + (g.x1 - g.x0) * i as f64 / samples as f64;
        // max (not sum) so peaks that crowd together on a linear axis never stack past their heights
        let yv: f64 = peaks.iter().map(|(_, e, en)| *e as f64 * (-((x - g.px(en * g.k)).powi(2)) / (2.0 * sigma * sigma)).exp()).fold(0.0, f64::max);
        (x, g.py(yv))
    }).collect()
}

fn pes_render(p: &Params) -> String {
    let peaks = pes_peaks(p);
    let cmp_z = if p.s("compare") == "None" || p.s("compare").is_empty() { None } else { Some(z_of(p.s("compare")).min(54)) };
    let cmp = cmp_z.map(element_peaks).unwrap_or_default();
    let g = pes_geom(p, &peaks, &cmp);
    let (x0, x1, y0, y1) = (g.x0, g.x1, g.y0, g.y1);
    let mut d = Doc::new(PES_W, PES_H);
    for n in 0..=(g.ymax as i64) {
        let y = g.py(n as f64);
        if p.b("gridlines") && n > 0 { d.line(x0, y, x1, y, "#c9ccd6", 1.0); }
        if p.b("numbered") {
            d.line(x0 - 5.0, y, x0, y, INK, 1.4);
            tx(&mut d, x0 - 9.0, y + 5.0, 14.0, "end", INK, false, &n.to_string());
        }
    }
    d.line(x0, y0 - 8.0, x0, y1, INK, 2.0);
    // x axis + ticks
    match &g.scale {
        PesScale::Log(lo, hi) => {
            d.line(x0, y1, x1, y1, INK, 2.0);
            let mut dec = *lo;
            while dec <= hi * 1.0001 {
                d.line(g.px(dec), y1, g.px(dec), y1 + 8.0, INK, 1.4);
                tx(&mut d, g.px(dec), y1 + 24.0, 14.0, "middle", INK, false, &thousands(dec));
                if dec < *hi { for m in 2..10 { let v = dec * m as f64; d.line(g.px(v), y1, g.px(v), y1 + 4.0, INK, 1.0); } }
                dec *= 10.0;
            }
        }
        PesScale::Lin(lo, hi) => {
            d.line(x0, y1, x1, y1, INK, 2.0);
            let st = super::frame::nice(hi - lo, 8.0);
            let mut v = *lo;
            while v <= hi + 1e-9 { d.line(g.px(v), y1, g.px(v), y1 + 7.0, INK, 1.4); tx(&mut d, g.px(v), y1 + 24.0, 14.0, "middle", INK, false, &thousands(v)); v += st; }
        }
        PesScale::Broken(gs) => {
            for (i, &(lo, hi, xa, xb)) in gs.iter().enumerate() {
                let (sa, sb) = (if i == 0 { x0 } else { xa - 8.0 }, if i + 1 == gs.len() { x1 } else { xb + 8.0 });
                d.line(sa, y1, sb, y1, INK, 2.0);
                if i + 1 < gs.len() {
                    for bx in [xb + 8.0, xb + 22.0] { d.line(bx - 4.0, y1 + 8.0, bx + 4.0, y1 - 8.0, INK, 1.6); }
                }
                let st = super::frame::nice(hi - lo, 3.0);
                let mut v = (lo / st).ceil() * st;
                while v <= hi + 1e-9 { d.line(g.px(v), y1, g.px(v), y1 + 7.0, INK, 1.4); tx(&mut d, g.px(v), y1 + 24.0, 14.0, "middle", INK, false, &thousands((v * 1000.0).round() / 1000.0)); v += st; }
            }
        }
    }
    tx(&mut d, (x0 + x1) / 2.0, y1 + 56.0, 17.0, "middle", INK, true, &format!("Binding energy ({})", if p.s("units") == "eV" { "eV" } else { "MJ/mol" }));
    d.raw(&format!(r##"<text x="22" y="{:.2}" font-size="16" text-anchor="middle" font-family="{FONT}" font-weight="bold" fill="{INK}" transform="rotate(-90 22 {:.2})">Relative number of electrons</text>"##, (y0 + y1) / 2.0, (y0 + y1) / 2.0));
    let bars = p.s("drawn") == "Bars";
    let draw_set = |d: &mut Doc, set: &[(String, u8, f64)], dashed_: bool| {
        if bars {
            for (_, e, en) in set {
                let x = g.px(en * g.k) + if dashed_ { 9.0 } else { 0.0 };
                let top = g.py(*e as f64);
                if dashed_ { d.raw(&format!(r##"<rect x="{:.2}" y="{top:.2}" width="10" height="{:.2}" fill="none" stroke="#6b7080" stroke-width="2" stroke-dasharray="5 4"/>"##, x - 5.0, y1 - top)); }
                else { d.rect(x - 6.0, top, 12.0, y1 - top, "#141414", "#141414", 1.0); }
            }
        } else {
            let pts = pes_curve(&g, set);
            if dashed_ {
                let pp: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
                d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="#6b7080" stroke-width="2.6" stroke-dasharray="7 5"/>"##, pp.join(" ")));
            } else { d.polyline(&pts, "#141414", 2.6); }
        }
    };
    if !cmp.is_empty() { draw_set(&mut d, &cmp, true); }
    draw_set(&mut d, &peaks, false);
    for (l, e, en) in &peaks {
        let c = g.px(en * g.k);
        let mut y = g.py(*e as f64) - 10.0;
        if p.b("energies") {
            let v = en * g.k;
            let t = if v >= 100.0 { thousands(v.round()) } else if v >= 10.0 { num(v, 1) } else { num(v, 2) };
            tx(&mut d, c, y, 13.0, "middle", "#333", false, &t);
            y -= 17.0;
        }
        let lab = if p.b("counts") { format!("{l}{}", sup(*e as u32)) } else { l.clone() };
        if p.b("labels") || p.b("counts") {
            match p.s("sublabel_style") {
                "None" if !p.b("counts") => {}
                "Blank lines" => { d.line(c - 16.0, y + 2.0, c + 16.0, y + 2.0, INK, 1.3); }
                _ => { tx(&mut d, c, y, 16.0, "middle", INK, true, &lab); }
            }
        }
    }
    let z = z_of(p.s("element")).min(54);
    let custom = p.s("source") == "Custom peaks";
    if let Some(cz) = cmp_z {
        let lx = x1 - 210.0;
        d.line(lx, 30.0, lx + 40.0, 30.0, "#141414", 2.6);
        tx(&mut d, lx + 48.0, 36.0, 17.0, "start", INK, true, if custom { "Custom" } else { symbol(z) });
        d.raw(&format!(r##"<line x1="{:.2}" y1="30" x2="{:.2}" y2="30" stroke="#6b7080" stroke-width="2.6" stroke-dasharray="7 5"/>"##, lx + 110.0, lx + 150.0));
        tx(&mut d, lx + 158.0, 36.0, 17.0, "start", INK, true, symbol(cz));
    } else if p.b("names") && !custom {
        tx(&mut d, x1, 36.0, 18.0, "end", INK, true, symbol(z));
    }
    let mut foot = Vec::new();
    if p.b("answer") {
        if custom {
            foot.push(peaks.iter().map(|(l, e, _)| format!("{l}{}", sup(*e as u32))).collect::<Vec<_>>().join(" "));
        } else {
            foot.push(format!("{} ({}): {}", name(z), symbol(z), config_text(&config(z, 0), None)));
        }
    }
    wrap(p, d.finish(), &foot)
}

// ---------------------------------------------------------------------------------------------
// Mass spectrum
// ---------------------------------------------------------------------------------------------

fn ms_params() -> Vec<ParamDef> {
    let names: Vec<&'static str> = ISOTOPES.iter().map(|e| e.0).collect();
    let mut v = vec![
        ParamDef::choice("source", "Peaks from", &["Element", "Custom isotopes"], "Element"),
        ParamDef::choice("element", "Element", &names, "Chlorine"),
        ParamDef::text("custom", "Custom isotopes (mass:abundance %, up to 6)", "35:75.8, 37:24.2"),
        ParamDef::bool("molecule", "Diatomic molecule (X2)", false),
        ParamDef::bool("relative", "Peak heights relative to the tallest (off = percent)", true),
        ParamDef::bool("values", "Abundances over the peaks", false),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::text("custom_name", "Custom element name", "Element X"),
        ParamDef::bool("show_name", "Element name over the spectrum", true),
        ParamDef::int("leave_out", "Leave a peak out (0 = none, 1 = lightest, 2 = next...)", 0, 10, 0),
        answer_param("Answer key (relative atomic mass, missing peak)"),
        title_style(true),
        ParamDef::choice("xtitle_style", "x-axis title", &TITLE3, "Text"),
        ParamDef::text("xtitle", "x-axis title text", "Mass-to-charge ratio (m/z)"),
        ParamDef::choice("ytitle_style", "y-axis title", &TITLE3, "Text"),
        ParamDef::text("ytitle", "y-axis title text", "Abundance (%)"),
        ParamDef::bool("xaxis_fit", "x-axis: fit to the peaks", true),
    ];
    v.extend(xaxis_params(AxisDefaults { from: 22.0, to: 28.0, by: 0.5, num: "Every 2nd line" }, 400.0));
    v.extend(yaxis_params(AxisDefaults { from: 0.0, to: 100.0, by: 10.0, num: "Every line" }, 1000.0));
    v.push(minor_param());
    v
}

fn ms_element(p: &Params) -> Option<&'static (&'static str, &'static str, f64, &'static [(u32, f64)])> {
    if p.s("source") == "Custom isotopes" { None } else { ISOTOPES.iter().find(|e| e.0 == p.s("element")) }
}

/// Raw isotopes (mass, abundance) of the chosen source, sorted by mass.
fn ms_iso(p: &Params) -> Vec<(u32, f64)> {
    let mut iso: Vec<(u32, f64)> = Vec::new();
    if p.s("source") == "Custom isotopes" {
        for tok in p.s("custom").split(|c| c == ',' || c == ';' || c == '\n') {
            let mut it = tok.split(|c| c == ':' || c == '=');
            if let (Some(a), Some(b)) = (it.next(), it.next()) {
                if let (Ok(m), Ok(ab)) = (a.trim().parse::<u32>(), b.trim().trim_end_matches('%').parse::<f64>()) {
                    if ab > 0.0 && m > 0 && !iso.iter().any(|x| x.0 == m) && iso.len() < 6 { iso.push((m, ab)); }
                }
            }
        }
    } else if let Some(e) = ms_element(p) {
        iso = e.3.to_vec();
    }
    iso.sort_by_key(|x| x.0);
    iso
}

/// Displayed bars (mass, height 0..100) for raw isotopes `iso`.
fn ms_bars(p: &Params, iso: &[(u32, f64)]) -> Vec<(u32, f64)> {
    let tot: f64 = iso.iter().map(|x| x.1).sum();
    let mut bars: Vec<(u32, f64)> = iso.iter().map(|&(m, a)| (m, a / tot * 100.0)).collect();
    if p.s("source") == "Custom isotopes" && !p.b("molecule") && (tot - 100.0).abs() < 0.05 {
        bars = iso.to_vec();
    }
    if p.b("molecule") {
        let mut v: Vec<(u32, f64)> = Vec::new();
        for (i, &(m1, a1)) in bars.iter().enumerate() {
            for &(m2, a2) in bars.iter().skip(i) {
                let pr = a1 / 100.0 * a2 / 100.0 * if m1 == m2 { 1.0 } else { 2.0 } * 100.0;
                match v.iter_mut().find(|x| x.0 == m1 + m2) { Some(x) => x.1 += pr, None => v.push((m1 + m2, pr)) }
            }
        }
        v.sort_by_key(|x| x.0);
        bars = v;
    }
    if p.b("relative") {
        let maxv = bars.iter().map(|x| x.1).fold(0.0, f64::max);
        for b in bars.iter_mut() { b.1 = b.1 / maxv * 100.0; }
    }
    bars
}

const MS_W: f64 = 700.0;
const MS_H: f64 = 520.0;

/// Graph frame + bars shared by the renderer and its handles.
fn ms_layout(p: &Params) -> Option<(Graph, Vec<(u32, f64)>)> {
    let iso = ms_iso(p);
    if iso.is_empty() { return None; }
    let bars = ms_bars(p, &iso);
    let mut x = Axis::read(p, "xaxis");
    if p.b("xaxis_fit") {
        x.from = bars.first()?.0 as f64 - 2.0;
        x.to = bars.last()?.0 as f64 + 2.0;
        x.fix();
    }
    let y = Axis::read(p, "yaxis");
    let xt = axis_title(p, "xtitle_style", p.s("xtitle")).is_some();
    let yt = axis_title(p, "ytitle_style", p.s("ytitle")).is_some();
    let top_extra = if p.b("show_name") { 26.0 } else { 0.0 };
    let (x0, x1, y0, y1) = plot_rect(MS_W, MS_H, &x, &y, xt, yt, top_extra);
    let minor = match p.s("minor_grid") { "2 per block" => 2, "4 per block" => 4, "5 per block" => 5, _ => 0 };
    Some((Graph { x, y, minor, x0, x1, y0, y1 }, bars))
}

/// Value printed over a peak.
fn ms_value(p: &Params, v: f64) -> String {
    if p.b("relative") { if (v - 100.0).abs() < 1e-9 { "100".into() } else { format!("{v:.2}") } }
    else if p.s("source") == "Custom isotopes" && !p.b("molecule") { format!("{}%", tick((v * 100.0).round() / 100.0)) }
    else { format!("{v:.2}%") }
}

/// Name over the spectrum and in the answer key.
fn ms_name(p: &Params) -> (String, String) {
    match ms_element(p) {
        Some(e) if p.b("molecule") => (e.0.to_string(), format!("{}{}", e.1, sub(2))),
        Some(e) => (e.0.to_string(), e.1.to_string()),
        None => (p.s("custom_name").trim().to_string(), String::new()),
    }
}

fn ms_render(p: &Params) -> String {
    let Some((g, bars)) = ms_layout(p) else {
        let mut d = Doc::new(460.0, 120.0);
        tx(&mut d, 230.0, 66.0, 15.0, "middle", "#b03030", false, "Enter isotopes as mass:abundance, e.g. 35:75.8, 37:24.2");
        return d.finish();
    };
    let relative = p.b("relative");
    let mut d = Doc::new(MS_W, MS_H);
    let xt = axis_title(p, "xtitle_style", p.s("xtitle"));
    let ytxt = if relative && p.s("ytitle") == "Abundance (%)" { "Relative abundance" } else { p.s("ytitle") };
    let yt = axis_title(p, "ytitle_style", ytxt);
    draw_graph(&mut d, &g, xt, yt);
    let leave = p.i("leave_out").max(0) as usize;
    let bw = ((g.x1 - g.x0) / (g.x.to - g.x.from) * 0.28).clamp(5.0, 18.0);
    for (k, &(mass, v)) in bars.iter().enumerate() {
        if leave == k + 1 { continue; }
        let x = g.px(mass as f64);
        if x < g.x0 - 0.5 || x > g.x1 + 0.5 || v <= g.y.from { continue; }
        let top = g.py(v.min(g.y.to));
        d.rect(x - bw / 2.0, top, bw, g.y1 - top, "#141a29", "#141a29", 1.0);
        if p.b("values") {
            tx(&mut d, x, top - 8.0, 15.0, "middle", INK, true, &ms_value(p, v));
        }
    }
    let (name, sym) = ms_name(p);
    if p.b("show_name") && !name.is_empty() {
        let shown = if p.b("molecule") { format!("{name} ({sym})") } else { name.clone() };
        let up = if g.y.lab.is_some() { 22.0 } else { 0.0 };
        tx(&mut d, g.x1, g.y0 - 14.0 - up, 20.0, "end", INK, true, &shown);
    }
    let mut foot = Vec::new();
    if p.b("answer") {
        let iso = ms_iso(p);
        let ram = match ms_element(p) {
            Some(e) => e.2,
            None => { let t: f64 = iso.iter().map(|x| x.1).sum(); iso.iter().map(|x| x.0 as f64 * x.1).sum::<f64>() / t }
        };
        let what = if p.b("molecule") { "relative molecular mass" } else { "relative atomic mass" };
        let r = if p.b("molecule") { 2.0 * ram } else { ram };
        foot.push(if sym.is_empty() { format!("{name}: {what} {r:.2}") } else { format!("{name} ({sym}): {what} {r:.2}") });
        if let Some(&(m, v)) = bars.get(leave.wrapping_sub(1)) {
            foot.push(format!("Missing peak: m/z {m}, {}", ms_value(p, v)));
        }
    }
    wrap(p, d.finish(), &foot)
}

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

use crate::figures::interact::{Cursor, Handle};

pub(super) fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    match id {
        "line-spectrum" => Some(line_handles(p)),
        "photoelectron-spectrum" => Some(pes_handles(p)),
        "mass-spectrum" => Some(ms_handles(p)),
        _ => None,
    }
}

fn line_handles(p: &Params) -> Vec<Handle> {
    let rows = line_rows(p);
    let l = line_lay(p, &rows);
    let (x0, x1, lo, hi) = (l.x0, l.x1, l.lo, l.hi);
    let mut v = Vec::new();
    if !p.s("strips").trim().is_empty() { return v; }
    for (r, e) in rows.iter().enumerate() {
        let y = l.top + r as f64 * l.pitch + l.sh / 2.0;
        if !e.custom {
            if let Some(&(key, _)) = OLD_KEYS.iter().find(|k| k.1 == e.name) {
                v.push(Handle::zone((x0 / 2.0, y), (x0 - 10.0, 26.0), "Click to hide this element's spectrum", move |q| q.set_b(key, false)));
            }
            continue;
        }
        for (i, &(nm, _)) in e.lines.iter().enumerate() {
            if !(lo..=hi).contains(&nm) { continue; }
            v.push(Handle::knob((x0 + (nm - lo) / (hi - lo) * (x1 - x0), y), "Drag to change this line's wavelength", move |q, t| {
                let new = (lo + (t.0 - x0) / (x1 - x0) * (hi - lo)).clamp(lo, hi);
                let new = (new * 2.0).round() / 2.0;
                let mut toks: Vec<String> = q.s("custom").split(|c: char| c == ',' || c == ';' || c.is_whitespace()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                let Some(tok) = toks.iter_mut().filter(|t| t.split(':').next().and_then(|n| n.trim().parse::<f64>().ok()).is_some()).nth(i) else { return };
                let mut parts = tok.split(':');
                let old: f64 = parts.next().and_then(|n| n.trim().parse().ok()).unwrap_or(0.0);
                if (old - new).abs() < 0.26 { return; }
                let rest: Vec<&str> = parts.collect();
                *tok = if rest.is_empty() { format!("{new}") } else { format!("{new}:{}", rest.join(":")) };
                let joined = toks.join(", ");
                q.set_s("custom", &joined);
            }).with_cursor(Cursor::ResizeH));
        }
    }
    v
}

fn pes_handles(p: &Params) -> Vec<Handle> {
    let mut v = Vec::new();
    let peaks = pes_peaks(p);
    let cmp: Vec<(String, u8, f64)> = if p.s("compare") == "None" || p.s("compare").is_empty() { vec![] } else { element_peaks(z_of(p.s("compare")).min(54)) };
    let g = pes_geom(p, &peaks, &cmp);
    if p.s("source") == "Custom peaks" && g.inv(0.0).is_some() {
        let n_ok = p.s("custom").split(|c| c == ',' || c == ';').filter(|t| {
            let parts: Vec<&str> = t.trim().split(':').collect();
            parts.len() >= 2 && parts[0].trim().parse::<f64>().is_ok() && parts[1].trim().parse::<f64>().is_ok()
        }).count();
        if n_ok > 0 {
            for (i, (_, e, en)) in peaks.iter().enumerate() {
                v.push(Handle::knob((g.px(en * g.k), g.py(*e as f64)), "Drag: sideways = binding energy, up/down = electrons", move |q, t| {
                    let mut toks: Vec<String> = q.s("custom").split(|c| c == ',' || c == ';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                    let peaks = pes_peaks(q);
                    let cmp: Vec<(String, u8, f64)> = if q.s("compare") == "None" || q.s("compare").is_empty() { vec![] } else { element_peaks(z_of(q.s("compare")).min(54)) };
                    let g = pes_geom(q, &peaks, &cmp);
                    let Some((e_old, en_old)) = peaks.get(i).map(|x| (x.1, x.2)) else { return };
                    let Some(disp) = g.inv(t.0) else { return };
                    let en_new = (disp / g.k).clamp(0.05, 2000.0);
                    let e_new = ((g.y1 - t.1) / (g.y1 - g.y0) * g.ymax).round().clamp(1.0, 14.0) as u8;
                    if e_new == e_old && (en_new / en_old - 1.0).abs() < 0.01 { return; }
                    let Some(tok) = toks.get_mut(i) else { return };
                    let label = tok.split(':').nth(2).map(|l| l.trim().to_string());
                    *tok = match label { Some(l) => format!("{}:{e_new}:{l}", sig(en_new)), None => format!("{}:{e_new}", sig(en_new)) };
                    let joined = toks.join(", ");
                    q.set_s("custom", &joined);
                }));
            }
        }
    } else if p.s("source") != "Custom peaks" {
        v.push(
            Handle::zone(((g.x0 + g.x1) / 2.0, (g.y0 + g.y1) / 2.0), (g.x1 - g.x0, g.y1 - g.y0), "Scroll to change the element", |_| {}).with_scroll(|q, dy| {
                let z = z_of(q.s("element")) as i64;
                let nz = (z.min(54) - 1 + if dy > 0.0 { 1 } else { -1 }).rem_euclid(54) + 1;
                q.set_s("element", ELEMENT_CHOICES[(nz - 1) as usize]);
            }),
        );
    }
    v
}

/// Three significant figures without trailing noise.
fn sig(x: f64) -> String {
    let s = if x >= 100.0 { format!("{x:.0}") } else if x >= 10.0 { format!("{x:.1}") } else { format!("{x:.2}") };
    s
}

fn ms_handles(p: &Params) -> Vec<Handle> {
    if p.b("molecule") { return Vec::new(); }
    let Some((g, bars)) = ms_layout(p) else { return Vec::new() };
    let leave = p.i("leave_out").max(0) as usize;
    bars.iter().enumerate().filter(|&(k, &(m, h))| {
        let x = g.px(m as f64);
        k + 1 != leave && h <= g.y.to && h > g.y.from && x >= g.x0 && x <= g.x1
    }).map(|(i, &(mass, h))| {
        Handle::knob((g.px(mass as f64), g.py(h)), "Drag: sideways = mass number, up/down = abundance", move |q, t| {
            let iso = ms_iso(q);
            let Some((g, bars)) = ms_layout(q) else { return };
            let Some(&(m_old, h_old)) = bars.get(i) else { return };
            let m_new = g.vx(t.0).round().clamp(1.0, 300.0) as u32;
            let h_new = g.vy(t.1).clamp(0.5, 100.0);
            let m_new = if iso.iter().enumerate().any(|(k, x)| k != i && x.0 == m_new) { m_old } else { m_new };
            if m_new == m_old && (h_new - h_old).abs() < 0.8 { return; }
            let others: Vec<f64> = iso.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, x)| x.1).collect();
            let ab_old = iso[i].1;
            let ab_new = if others.is_empty() {
                ab_old
            } else if q.b("relative") {
                (h_new / 100.0 * others.iter().cloned().fold(0.0, f64::max)).max(0.01)
            } else {
                let s = (h_new / 100.0).min(0.95);
                (s / (1.0 - s) * others.iter().sum::<f64>()).max(0.01)
            };
            let mut list = iso.clone();
            list[i] = (m_new, ab_new);
            list.sort_by_key(|x| x.0);
            let text = list.iter().map(|(m, a)| format!("{m}:{}", sig(*a))).collect::<Vec<_>>().join(", ");
            if q.s("source") != "Custom isotopes" {
                let nm = q.s("element").to_string();
                q.set_s("custom_name", &nm);
            }
            q.set_s("source", "Custom isotopes");
            q.set_s("custom", &text);
        })
    }).collect()
}

// ---------------------------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------------------------

use crate::figures::{Example, Value};

fn ex(name: &'static str, params: Vec<(&'static str, Value)>) -> Example { Example { name, params } }
fn s(v: &str) -> Value { Value::S(v.into()) }
fn b(v: bool) -> Value { Value::B(v) }
fn i(v: i64) -> Value { Value::I(v) }
fn f(v: f64) -> Value { Value::F(v) }

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    Some(match id {
        "mass-spectrum" => vec![
            ex("Mass spectrum of magnesium", vec![("element", s("Magnesium")), ("relative", b(false)), ("values", b(true))]),
            ex("Mass spectrum of chlorine with its relative atomic mass", vec![("element", s("Chlorine")), ("relative", b(false)), ("values", b(true)), ("answer", b(true))]),
            ex("Identify the element from its mass spectrum", vec![("element", s("Bromine")), ("relative", b(false)), ("values", b(true)), ("show_name", b(false)), ("xaxis_by", f(1.0)), ("xaxis_num", s("Every line"))]),
            ex("Draw the missing peak: silicon", vec![("element", s("Silicon")), ("relative", b(false)), ("values", b(true)), ("leave_out", i(2)), ("answer", b(true))]),
            ex("Mass spectrum of a made-up element", vec![("source", s("Custom isotopes")), ("custom", s("20:90, 22:10")), ("custom_name", s("Element Q")), ("relative", b(false)), ("values", b(true)), ("answer", b(true))]),
            ex("Mass spectrum of zirconium (relative abundance)", vec![("element", s("Zirconium")), ("relative", b(true)), ("values", b(true)), ("xaxis_by", f(1.0)), ("xaxis_num", s("Every line"))]),
        ],
        "photoelectron-spectrum" => vec![
            ex("Photoelectron spectrum of sodium", vec![("element", s("11 Na - Sodium"))]),
            ex("Photoelectron spectrum of neon with binding energies", vec![("element", s("10 Ne - Neon")), ("energies", b(true))]),
            ex("PES comparing sodium and magnesium", vec![("element", s("12 Mg - Magnesium")), ("compare", s("11 Na - Sodium"))]),
            ex("Identify the element from its photoelectron spectrum", vec![("element", s("15 P - Phosphorus")), ("names", b(false)), ("sublabel_style", s("Blank lines")), ("answer", b(true))]),
            ex("Photoelectron spectrum of calcium (broken axis, eV)", vec![("element", s("20 Ca - Calcium")), ("scale", s("Broken")), ("units", s("eV")), ("drawn", s("Bars"))]),
            ex("Photoelectron spectrum of iron", vec![("element", s("26 Fe - Iron")), ("counts", b(true))]),
        ],
        "line-spectrum" => vec![
            ex("Emission spectra: hydrogen, helium, sodium and an unknown mixture", vec![("strips", s("Hydrogen; Helium; Sodium; Unknown = Hydrogen + Sodium")), ("answer", b(true))]),
            ex("Hydrogen emission spectrum (Balmer series)", vec![("strips", s("Hydrogen")), ("labels", b(true))]),
            ex("Absorption spectra: hydrogen and helium", vec![("strips", s("Hydrogen; Helium")), ("style", s("Absorption"))]),
            ex("Flame test metals: lithium, sodium, strontium, copper", vec![("strips", s("Lithium; Sodium; Strontium; Copper"))]),
            ex("Identify the element (black and white)", vec![("strips", s("Mercury; Neon; Helium")), ("style", s("Print")), ("label_style", s("Blank"))]),
            ex("Made-up elements and an unknown mixture", vec![("strips", s("Element X: 425 510 630; Element Y: 455 545 600; Element Z: 480 575 665; Unknown = Element X + Element Z")), ("answer", b(true))]),
        ],
        _ => return None,
    })
}
