//! A plot arc (Freytag's pyramid): the rise and fall of a story in five stages. The events of a
//! story are given in order, one per stage; the stage each belongs to is the answer, so the student
//! copy (Show answers off) leaves the stage names as blank lines under an empty arc.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use super::wrap;

const STAGES: [&str; 5] = ["Exposition", "Rising action", "Climax", "Falling action", "Resolution"];
const INK: &str = "#111827";
const DEFAULT: &str = "Cinderella lives with her unkind stepfamily\nShe is forbidden to go to the ball, but her fairy godmother helps\nAt the ball the prince falls in love with her\nShe flees at midnight and loses a slipper\nThe prince finds her and they marry";

pub fn def() -> FigureDef {
    FigureDef {
        id: "plot-arc",
        subject: Subject::Language,
        title: "Plot arc",
        blurb: "A plot arc of five stages (exposition, rising action, climax, falling action, resolution) with a story's events placed along it, or blank stage names for students to label.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "plot-arc" => vec![
            Example { name: "A fairy tale", params: vec![] },
            Example { name: "Three little pigs", params: vec![("events", s("Three pigs leave home to build houses\nThe wolf blows down the straw and stick houses\nThe wolf cannot blow down the brick house\nThe wolf climbs down the chimney\nThe wolf runs away and the pigs are safe"))] },
            Example { name: "Only the climax", params: vec![("events", s("\n\nThe duel on the bridge"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("events", "What happens at each stage, in order", DEFAULT).hint("One line per stage, in story order: exposition, rising action, climax, falling action, resolution. Leave a line empty to skip a stage"),
        ParamDef::text("story", "Story title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the arc and the events and leaves the stage names blank for students"),
    ]
}

fn render(p: &Params) -> String {
    let raw = p.s("events");
    if raw.chars().count() > 1500 {
        return chart::message("The events are limited to 1500 characters");
    }
    // keep blank lines: they hold a stage's place
    let mut events: Vec<String> = raw.lines().take(5).map(|l| l.trim().chars().take(160).collect()).collect();
    events.resize(5, String::new());
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let story = p.s("story").trim();
    let top = if story.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (1000.0, top + 500.0);
    let mut d = Doc::new(w, h);
    if !story.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", story);
    }
    let base = top + 330.0;
    let pts = [(40.0, base - 10.0), (210.0, base - 14.0), (500.0, top + 40.0), (790.0, base - 40.0), (960.0, base - 10.0)];
    // the arc: a flat start, the climb, the descent, a flat end
    let fill = if print { "#ececec" } else { "#f5f0e6" };
    let mut area: Vec<(f64, f64)> = pts.to_vec();
    area.push((960.0, base));
    area.push((40.0, base));
    d.poly(&area, fill, "none", 0.0);
    d.polyline(&pts, INK, 3.2);
    d.line(40.0, base, 960.0, base, INK, 1.6);
    // dividers between the stages and the dot of each
    let cuts = [210.0, 380.0, 620.0, 790.0];
    for x in cuts {
        d.raw(&format!(r##"<line x1="{x}" y1="{:.1}" x2="{x}" y2="{base}" stroke="#9ca3af" stroke-width="1.2" stroke-dasharray="4 4"/>"##, top + 20.0));
    }
    let centre = [125.0, 295.0, 500.0, 705.0, 875.0];
    let at = |x: f64| -> f64 {
        let mut prev = pts[0];
        for &q in &pts[1..] {
            if x <= q.0 {
                return prev.1 + (q.1 - prev.1) * (x - prev.0) / (q.0 - prev.0);
            }
            prev = q;
        }
        pts[4].1
    };
    let widths = [170.0, 170.0, 240.0, 170.0, 170.0];
    let edges = [40.0, 210.0, 380.0, 620.0, 790.0, 960.0];
    for i in 0..5 {
        let (cx, cy) = (centre[i], at(centre[i]));
        if !events[i].is_empty() {
            d.circle(cx, cy, 7.0, "#fff", INK, 2.4);
        }
        // the stage name, or a blank line to write it on
        let (x0, x1) = (edges[i], edges[i + 1]);
        let ly = base + 34.0;
        if answers {
            chart::text(&mut d, cx, ly, fs * 1.05, "middle", STAGES[i]);
        } else {
            d.line(x0 + 14.0, ly + 3.0, x1 - 14.0, ly + 3.0, INK, 1.0);
        }
        // the event, in a box
        let (bx, bw) = (x0 + 6.0, widths[i] - 12.0);
        d.rect(bx, base + 52.0, bw, 100.0, "none", INK, 1.2);
        let chars = ((bw - 16.0) / (fs * 0.55)) as usize;
        for (k, l) in wrap(&events[i], chars, 5).iter().enumerate() {
            chart::text(&mut d, cx, base + 74.0 + k as f64 * fs * 1.25, fs * 0.9, "middle", l);
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
    fn key_names_the_stages() {
        let svg = render(&defaults());
        for s in STAGES {
            assert!(svg.contains(&format!(">{s}<")), "{s}");
        }
        assert!(svg.contains("Cinderella lives"));
    }

    #[test]
    fn student_copy_blanks_the_names_only() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(!student.contains(">Climax<") && key.contains(">Climax<") && student.contains("Cinderella lives"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn blank_lines_skip_stages_and_long_text_is_wrapped() {
        let mut p = defaults();
        p.set_s("events", "\n\nOnly the climax, which is a very long event that goes on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on");
        let svg = render(&p);
        assert!(sane(&svg) && svg.contains("Only the climax,") && svg.contains('\u{2026}'));
        assert_eq!(svg.matches("<circle").count(), 1);
        p.set_s("events", &"x".repeat(1501));
        assert!(render(&p).contains("limited to 1500"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("plot-arc").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "<b>x</b>", "nan", &"w ".repeat(500), "\n\n\n\n\n\n\n\n", "a\nb\nc\nd\ne\nf\ng"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("events", spec);
                p.set_s("story", "<script>");
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>") && !svg.contains("<script>"), "{spec:?}");
            }
        }
    }
}
