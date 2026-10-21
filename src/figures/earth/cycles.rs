//! The rock cycle and the water cycle: the stages as boxes joined by arrows, each arrow carrying
//! the process that moves material from one stage to the next. The boxes and arrows are the
//! question; the student copy (Show answers off) leaves either the stage names or the process
//! labels as empty boxes, as chosen.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const CYCLES: [&str; 2] = ["Rock cycle", "Water cycle"];
const HIDES: [&str; 2] = ["Process labels", "Stage names"];
const INK: &str = "#111827";

pub fn def() -> FigureDef {
    FigureDef {
        id: "earth-cycles",
        subject: Subject::EarthScience,
        title: "Rock cycle and water cycle",
        blurb: "The rock cycle or the water cycle as stages joined by labelled arrows, with the process labels or the stage names left blank for students.",
        params,
        render,
    }
}

pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "earth-cycles" => vec![
            Example { name: "Rock cycle", params: vec![] },
            Example { name: "Rock cycle with the shortcuts", params: vec![("extras", Value::B(true))] },
            Example { name: "Water cycle", params: vec![("cycle", s(CYCLES[1]))] },
            Example { name: "Blank process labels", params: vec![("show_answers", Value::B(false))] },
            Example { name: "Blank stage names", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDES[1]))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("cycle", "Cycle", &CYCLES, CYCLES[0]),
        ParamDef::bool("extras", "Show the extra paths (rock shortcuts, transpiration)", false),
        ParamDef::choice("hide", "Leave blank in the student copy", &HIDES, HIDES[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the process labels (or the stage names) as empty boxes for students to fill in"),
    ]
}

struct Edge {
    from: usize,
    to: usize,
    label: &'static str,
    extra: bool,
    /// How far along the arrow its label sits (0 start, 1 tip).
    at: f64,
}

/// The stages (name, centre) and arrows of a cycle.
fn layout(rock: bool) -> (Vec<(&'static str, f64, f64)>, Vec<Edge>) {
    let e = |from, to, label, extra| Edge { from, to, label, extra, at: 0.5 };
    let e2 = |from, to, label, at| Edge { from, to, label, extra: true, at };
    if rock {
        // a pentagon, clockwise from the top
        let c = (360.0, 270.0);
        let at = |deg: f64| {
            let (x, y) = crate::figures::earth::polar(0.0, 0.0, 1.0, deg);
            (c.0 + 270.0 * x, c.1 + 200.0 * y)
        };
        let pts = [at(90.0), at(18.0), at(-54.0), at(-126.0), at(162.0)];
        let names = ["Magma", "Igneous rock", "Sediment", "Sedimentary rock", "Metamorphic rock"];
        (
            names.iter().zip(pts).map(|(n, p)| (*n, p.0, p.1)).collect(),
            vec![
                e(0, 1, "Cooling and crystallisation", false),
                e(1, 2, "Weathering and erosion", false),
                e(2, 3, "Compaction and cementation", false),
                e(3, 4, "Heat and pressure", false),
                e(4, 0, "Melting", false),
                e2(1, 4, "Heat and pressure", 0.5),
                e2(3, 0, "Melting", 0.28),
                e2(4, 2, "Weathering and erosion", 0.28),
            ],
        )
    } else {
        (
            vec![("Ocean", 110.0, 400.0), ("Atmosphere", 110.0, 90.0), ("Clouds", 610.0, 90.0), ("Land surface", 610.0, 400.0), ("Groundwater", 360.0, 540.0)],
            vec![
                e(0, 1, "Evaporation", false),
                e(1, 2, "Condensation", false),
                e(2, 3, "Precipitation", false),
                e(3, 0, "Runoff", false),
                e(3, 4, "Infiltration", false),
                e(4, 0, "Seepage", false),
                e(3, 1, "Transpiration", true),
            ],
        )
    }
}

fn lines(s: &str) -> Vec<String> {
    if s.chars().count() <= 14 || !s.contains(' ') {
        return vec![s.to_string()];
    }
    let words: Vec<&str> = s.split(' ').collect();
    let mut best = (usize::MAX, 1);
    for i in 1..words.len() {
        let (a, b) = (words[..i].join(" ").chars().count(), words[i..].join(" ").chars().count());
        if a.max(b) < best.0 {
            best = (a.max(b), i);
        }
    }
    vec![words[..best.1].join(" "), words[best.1..].join(" ")]
}

fn render(p: &Params) -> String {
    let rock = p.s("cycle") != CYCLES[1];
    let answers = p.b(SHOW_ANSWERS);
    let hide_names = p.s("hide") == HIDES[1];
    let show_names = answers || !hide_names;
    let show_labels = answers || hide_names;
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (nodes, edges) = layout(rock);
    let (w, h) = if rock { (720.0, 540.0 + top) } else { (720.0, 600.0 + top) };
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (bw, bh) = if rock { (150.0, 56.0) } else { (150.0, 56.0) };
    let ctr = |i: usize| (nodes[i].1, nodes[i].2 + top);
    let stroke = if print { "#111827" } else if rock { "#8a4b1f" } else { "#2f6fb5" };
    let mut labels: Vec<((f64, f64), &str)> = Vec::new();
    for ed in edges.iter().filter(|e| p.b("extras") || !e.extra) {
        let (a, b) = (ctr(ed.from), ctr(ed.to));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / len, dy / len);
        let edge = |c: (f64, f64), sgn: f64| {
            let (vx, vy) = (ux * sgn, uy * sgn);
            let sx = if vx.abs() > 1e-9 { (bw / 2.0 + 4.0) / vx.abs() } else { f64::INFINITY };
            let sy = if vy.abs() > 1e-9 { (bh / 2.0 + 4.0) / vy.abs() } else { f64::INFINITY };
            let s = sx.min(sy);
            (c.0 + vx * s, c.1 + vy * s)
        };
        let (p0, p1) = (edge(a, 1.0), edge(b, -1.0));
        let dash = if ed.extra { r#" stroke-dasharray="9 5""# } else { "" };
        d.raw(&format!(r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{stroke}" stroke-width="2.6"{dash}/>"##, p0.0, p0.1, p1.0 - ux * 9.0, p1.1 - uy * 9.0));
        let (nx, ny) = (-uy, ux);
        d.poly(&[(p1.0, p1.1), (p1.0 - ux * 13.0 + nx * 6.5, p1.1 - uy * 13.0 + ny * 6.5), (p1.0 - ux * 13.0 - nx * 6.5, p1.1 - uy * 13.0 - ny * 6.5)], stroke, "none", 0.0);
        labels.push(((p0.0 + (p1.0 - p0.0) * ed.at, p0.1 + (p1.1 - p0.1) * ed.at), ed.label));
    }
    for (i, (name, _, _)) in nodes.iter().enumerate() {
        let (cx, cy) = ctr(i);
        d.rect(cx - bw / 2.0, cy - bh / 2.0, bw, bh, if print { "#e5e5e5" } else if rock { "#f1e4d6" } else { "#e1edf9" }, INK, 2.0);
        if show_names {
            let ls = lines(name);
            let y0 = cy + fs * 0.38 - (ls.len() as f64 - 1.0) * fs * 0.6;
            for (j, l) in ls.iter().enumerate() {
                d.raw(&format!(
                    r##"<text x="{cx:.1}" y="{:.1}" font-size="{}" font-weight="bold" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
                    y0 + j as f64 * fs * 1.2,
                    fs * 1.05,
                    svg::esc(l)
                ));
            }
        }
    }
    // the label boxes go on last so no arrow or box hides one
    let (lw, lh) = (138.0, 38.0);
    for ((x, y), text) in labels {
        if show_labels {
            d.rect(x - lw / 2.0, y - lh / 2.0, lw, lh, "#fff", "none", 0.0);
            let ls = lines(text);
            let y0 = y + fs * 0.35 - (ls.len() as f64 - 1.0) * fs * 0.6;
            for (j, l) in ls.iter().enumerate() {
                chart::text(&mut d, x, y0 + j as f64 * fs * 1.2, fs, "middle", l);
            }
        } else {
            d.rect(x - lw / 2.0, y - lh / 2.0, lw, lh, "#fff", INK, 1.2);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn rock_cycle_names_the_stages_and_processes() {
        let svg = render(&defaults());
        for t in [">Magma<", ">Igneous rock<", ">Sediment<", ">Melting<", ">pressure<", ">Weathering<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert_eq!(svg.matches("stroke-dasharray").count(), 0, "no shortcuts by default");
        let mut p = defaults();
        p.set_b("extras", true);
        assert_eq!(render(&p).matches("stroke-dasharray").count(), 3, "three shortcut arrows");
    }

    #[test]
    fn water_cycle_names_the_stages_and_processes() {
        let mut p = defaults();
        p.set_s("cycle", CYCLES[1]);
        let svg = render(&p);
        for t in [">Ocean<", ">Atmosphere<", ">Clouds<", ">Evaporation<", ">Precipitation<", ">Runoff<", ">Infiltration<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert!(!svg.contains("Transpiration"));
        p.set_b("extras", true);
        assert!(render(&p).contains(">Transpiration<"));
    }

    #[test]
    fn student_copy_blanks_what_was_chosen() {
        for cycle in CYCLES {
            let mut p = defaults();
            p.set_s("cycle", cycle);
            p.set_b("extras", true);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert!(student.contains(">Clouds<") || student.contains(">Magma<"));
            assert!(!student.contains("Melting") && !student.contains("Evaporation") && !student.contains("Runoff"));
            assert_eq!(size_of(&student), size_of(&key));
            p.set_s("hide", HIDES[1]);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert!(!student.contains(">Magma<") && !student.contains(">Ocean<") && (student.contains("Melting") || student.contains("Runoff")));
            assert_eq!(size_of(&student), size_of(&key));
        }
    }

    #[test]
    fn print_style_is_grey() {
        for cycle in CYCLES {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("cycle", cycle);
            p.set_b("extras", true);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("earth-cycles").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for title in ["", "\u{0}", "<script>", "\u{1f600}", &"x".repeat(5000)] {
            for cycle in CYCLES {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("title", title);
                    p.set_s("cycle", cycle);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg) && !svg.contains("<script>"), "{title:?}");
                }
            }
        }
    }
}
