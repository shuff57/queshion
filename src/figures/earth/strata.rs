//! A strata cross-section for relative dating: rock layers drawn flat, tilted, folded or cut by a
//! fault, with an optional dyke cutting through them. The layers are lettered from the top and
//! named in a list; the order in which they formed is the answer, so the student copy (Show
//! answers off) leaves the numbers out and the "oldest to youngest" line blank.
use crate::figures::chart;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const STRUCTURES: [&str; 4] = ["Flat layers", "Tilted layers", "Folded layers (anticline)", "Faulted layers"];
const MAX_LAYERS: usize = 6;
const MAX_SPEC: usize = 600;
const INK: &str = "#111827";
const DEFAULT_LAYERS: &str = "Sandstone\nShale\nLimestone\nSiltstone\nConglomerate";
const X0: f64 = 40.0;
const X1: f64 = 720.0;
const SURFACE: f64 = 100.0;
const BOTTOM: f64 = 350.0;
const FAULT_X: f64 = 400.0;
const THROW: f64 = 46.0;
const DYKE: (f64, f64) = (566.0, 592.0);

pub fn def() -> FigureDef {
    FigureDef {
        id: "strata",
        subject: Subject::EarthScience,
        title: "Strata and relative dating",
        blurb: "A cross-section of rock layers, flat, tilted, folded or faulted, with an optional dyke, lettered for relative dating: the order of formation is worked out, or left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "strata" => vec![
            Example { name: "Flat layers", params: vec![] },
            Example { name: "Tilted layers", params: vec![("structure", s(STRUCTURES[1]))] },
            Example { name: "Folded layers", params: vec![("structure", s(STRUCTURES[2]))] },
            Example { name: "A fault", params: vec![("structure", s(STRUCTURES[3]))] },
            Example { name: "A dyke cutting flat layers", params: vec![("dyke", Value::B(true))] },
            Example { name: "Blank for students", params: vec![("structure", s(STRUCTURES[3])), ("dyke", Value::B(true)), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("layers", "Layers, top to bottom", DEFAULT_LAYERS).hint("One rock name per line, youngest-looking at the top; 2 to 6 layers"),
        ParamDef::choice("structure", "Structure", &STRUCTURES, STRUCTURES[0]),
        ParamDef::bool("dyke", "Add a dyke (an igneous intrusion cutting the layers)", false),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the layers, their letters and the list of rocks and leaves the order of formation blank"),
    ]
}

fn parse(spec: &str) -> Result<Vec<String>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The layers are limited to {MAX_SPEC} characters"));
    }
    let v: Vec<String> = spec.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| l.chars().take(24).collect()).collect();
    if v.len() < 2 {
        return Err("Give at least two layers, one per line".into());
    }
    if v.len() > MAX_LAYERS {
        return Err(format!("A figure is limited to {MAX_LAYERS} layers"));
    }
    Ok(v)
}

/// How far the layer boundaries are moved from level at column `x`, by structure.
fn offset(structure: usize, x: f64) -> f64 {
    match structure {
        1 => (x - 380.0) * 0.22,
        2 => -75.0 * (1.0 - ((x - 380.0) / 340.0).powi(2)).max(0.0),
        3 if x > FAULT_X => THROW,
        _ => 0.0,
    }
}

fn surface(structure: usize, x: f64) -> f64 {
    if structure == 3 && x > FAULT_X { SURFACE + THROW } else { SURFACE }
}

/// Upper and lower edge of layer `k` of `n` at column `x`.
fn edges(structure: usize, n: usize, k: usize, x: f64) -> (f64, f64) {
    let t = (BOTTOM - SURFACE) / n as f64;
    let s = surface(structure, x);
    let b = |j: usize| -> f64 { (SURFACE + j as f64 * t + offset(structure, x)).clamp(s, BOTTOM) };
    (if k == 0 { s } else { b(k) }, if k == n - 1 { BOTTOM } else { b(k + 1) })
}

/// The order of formation, oldest first: the layers from the bottom up, then anything that cuts them.
pub fn order(n: usize, structure: usize, dyke: bool) -> Vec<String> {
    let mut v: Vec<String> = (0..n).rev().map(|k| ((b'A' + k as u8) as char).to_string()).collect();
    if structure == 3 {
        v.push("F".into());
    }
    if dyke {
        v.push("D".into());
    }
    v
}

fn render(p: &Params) -> String {
    let names = match parse(p.s("layers")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let structure = STRUCTURES.iter().position(|s| *s == p.s("structure")).unwrap_or(0);
    let dyke = p.b("dyke");
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let n = names.len();
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let legend_rows = n + usize::from(structure == 3) + usize::from(dyke);
    let h = top + BOTTOM + 40.0 + legend_rows as f64 * fs * 1.9 + fs * 4.5;
    let mut d = Doc::new(760.0, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, 380.0, 22.0, fs * 1.3, "middle", title);
    }
    d.raw(&format!(r##"<g transform="translate(0 {top})">"##));
    let colours = ["#e6c88a", "#9a8f86", "#c9d3c0", "#d9a79a", "#c4b0d6", "#b7cfd8"];
    // the blocks the section is built from: one, or two either side of a fault
    let blocks: Vec<(f64, f64)> = if structure == 3 { vec![(X0, FAULT_X), (FAULT_X, X1)] } else { vec![(X0, X1)] };
    let mut best: Vec<(f64, f64, f64)> = vec![(0.0, 0.0, 0.0); n];
    for k in 0..n {
        let fill = if fills.is_print() { fills.series(k) } else { colours[k].to_string() };
        for &(a, b) in &blocks {
            let steps = (((b - a) / 8.0).ceil() as usize).max(1);
            let xs: Vec<f64> = (0..=steps).map(|i| a + (b - a) * i as f64 / steps as f64).collect();
            let mut pts: Vec<(f64, f64)> = xs.iter().map(|x| (*x, edges(structure, n, k, *x).0)).collect();
            pts.extend(xs.iter().rev().map(|x| (*x, edges(structure, n, k, *x).1)));
            d.poly(&pts, &fill, "none", 0.0);
            for x in &xs {
                let (u, l) = edges(structure, n, k, *x);
                if l - u > best[k].2 {
                    best[k] = (*x, (u + l) / 2.0, l - u);
                }
            }
        }
    }
    // the bedding lines, then the frame
    for k in 1..n {
        for &(a, b) in &blocks {
            let steps = (((b - a) / 8.0).ceil() as usize).max(1);
            let pts: Vec<(f64, f64)> = (0..=steps).map(|i| a + (b - a) * i as f64 / steps as f64).map(|x| (x, edges(structure, n, k, x).0)).collect();
            d.polyline(&pts, INK, 1.4);
        }
    }
    if structure == 3 {
        d.line(FAULT_X, SURFACE, FAULT_X, BOTTOM, INK, 3.4);
        d.arrow(FAULT_X - 14.0, 250.0, FAULT_X - 14.0, 214.0, INK, 2.4);
        d.arrow(FAULT_X + 14.0, 214.0, FAULT_X + 14.0, 250.0, INK, 2.4);
    }
    if dyke {
        d.rect(DYKE.0, SURFACE, DYKE.1 - DYKE.0, BOTTOM - SURFACE, if fills.is_print() { "#222" } else { "#7a2e2e" }, INK, 1.6);
    }
    d.rect(X0, SURFACE, X1 - X0, BOTTOM - SURFACE, "none", INK, 2.0);
    // letters on each layer, and the order of formation as numbers in the key
    let ord = order(n, structure, dyke);
    for k in 0..n {
        let (x, y, _) = best[k];
        d.circle(x, y, 13.0, "#fff", INK, 1.8);
        chart::text(&mut d, x, y + fs * 0.36, fs, "middle", &((b'A' + k as u8) as char).to_string());
        if answers {
            let rank = ord.iter().position(|o| *o == ((b'A' + k as u8) as char).to_string()).unwrap() + 1;
            d.raw(&format!(r##"<rect x="{:.1}" y="{:.1}" width="26" height="22" rx="4" fill="#fff" stroke="{INK}" stroke-width="1.4"/>"##, x + 20.0, y - 11.0));
            chart::text(&mut d, x + 33.0, y + fs * 0.34, fs * 0.95, "middle", &rank.to_string());
        }
    }
    if structure == 3 {
        d.circle(FAULT_X, SURFACE - 22.0, 13.0, "#fff", INK, 1.8);
        chart::text(&mut d, FAULT_X, SURFACE - 22.0 + fs * 0.36, fs, "middle", "F");
    }
    if dyke {
        let x = (DYKE.0 + DYKE.1) / 2.0;
        d.circle(x, SURFACE - 22.0, 13.0, "#fff", INK, 1.8);
        chart::text(&mut d, x, SURFACE - 22.0 + fs * 0.36, fs, "middle", "D");
    }
    d.raw("</g>");
    // the list of rocks
    let ly = top + BOTTOM + 34.0;
    let mut y = ly;
    let mut row = |d: &mut Doc, letter: &str, text: &str| {
        d.circle(52.0, y - fs * 0.35, 12.0, "#fff", INK, 1.6);
        chart::text(d, 52.0, y, fs, "middle", letter);
        chart::text(d, 76.0, y, fs, "start", text);
        y += fs * 1.9;
    };
    for (k, name) in names.iter().enumerate() {
        row(&mut d, &((b'A' + k as u8) as char).to_string(), name);
    }
    if structure == 3 {
        row(&mut d, "F", "Fault");
    }
    if dyke {
        row(&mut d, "D", "Dyke (igneous intrusion)");
    }
    let by = y + fs * 0.9;
    if answers {
        let t = format!("Oldest to youngest: {}", ord.join(", "));
        chart::text(&mut d, 40.0, by, fs * 1.1, "start", &t);
        let note = match (structure == 3, dyke) {
            (false, false) => "Each layer is younger than the one below it (the law of superposition).",
            (true, false) => "The fault cuts every layer, so it is younger than all of them.",
            (false, true) => "The dyke cuts every layer, so it is younger than all of them.",
            (true, true) => "The fault and the dyke each cut every layer they cross, so both are younger than all of the layers.",
        };
        chart::text(&mut d, 40.0, by + fs * 1.7, fs * 0.9, "start", note);
    } else {
        chart::text(&mut d, 40.0, by, fs * 1.1, "start", "Oldest to youngest:");
        d.line(40.0 + fs * 10.5, by + 3.0, 720.0, by + 3.0, INK, 1.0);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn order_runs_from_the_bottom_up_then_the_cutting_features() {
        assert_eq!(order(3, 0, false), vec!["C", "B", "A"]);
        assert_eq!(order(3, 3, false), vec!["C", "B", "A", "F"]);
        assert_eq!(order(2, 3, true), vec!["B", "A", "F", "D"]);
        assert_eq!(order(2, 1, true), vec!["B", "A", "D"]);
    }

    #[test]
    fn layer_edges_never_cross_and_stay_in_the_section() {
        for structure in 0..4 {
            for n in 2..=6 {
                for i in 0..=68 {
                    let x = X0 + (X1 - X0) * i as f64 / 68.0;
                    let mut last = surface(structure, x);
                    for k in 0..n {
                        let (u, l) = edges(structure, n, k, x);
                        assert!(u >= last - 1e-9 && l >= u - 1e-9 && l <= BOTTOM + 1e-9, "{structure} {n} {k} {x}");
                        last = l;
                    }
                    assert_eq!(last, BOTTOM);
                }
            }
        }
    }

    #[test]
    fn the_fault_drops_the_far_block() {
        let (u0, _) = edges(3, 5, 1, FAULT_X - 1.0);
        let (u1, _) = edges(3, 5, 1, FAULT_X + 1.0);
        assert!((u1 - u0 - THROW).abs() < 1e-9);
    }

    #[test]
    fn key_writes_the_order() {
        let svg = render(&defaults());
        assert!(svg.contains("Oldest to youngest: E, D, C, B, A") && svg.contains("law of superposition"));
        let mut p = defaults();
        p.set_s("structure", STRUCTURES[3]);
        p.set_b("dyke", true);
        let svg = render(&p);
        assert!(svg.contains("Oldest to youngest: E, D, C, B, A, F, D") && svg.contains(">Fault<") && svg.contains("Dyke (igneous intrusion)"));
    }

    #[test]
    fn student_copy_leaves_the_order_blank() {
        let mut p = defaults();
        p.set_s("structure", STRUCTURES[3]);
        p.set_b("dyke", true);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains(">Sandstone<") && student.contains("Oldest to youngest:") && !student.contains("superposition") && !student.contains("younger"));
        assert!(!student.contains("A, F") && key.contains("E, D, C, B, A, F, D"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_layers_are_explained() {
        for (spec, needle) in [("", "at least two"), ("Sand", "at least two"), ("a\nb\nc\nd\ne\nf\ng", "limited to 6")] {
            let mut p = defaults();
            p.set_s("layers", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let mut p = defaults();
        p.set_s("layers", &"x".repeat(700));
        assert!(render(&p).contains("limited to 600"));
    }

    #[test]
    fn print_style_is_grey() {
        for structure in STRUCTURES {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("structure", structure);
            p.set_b("dyke", true);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("strata").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for spec in ["\u{0}\n\u{0}", "<b>\n</b>", "\u{1f600}\n\u{1f600}", "a\nb", &"y\n".repeat(6)] {
            for structure in STRUCTURES {
                for dyke in [true, false] {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_s("layers", spec);
                        p.set_s("structure", structure);
                        p.set_b("dyke", dyke);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?} {structure}");
                    }
                }
            }
        }
    }
}
