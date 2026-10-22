//! Maslow's hierarchy of needs as a pyramid. The levels and an example of each are the question;
//! the names of the levels are the answers, so the student copy (Show answers off) leaves the
//! pyramid bands empty and the examples beside them for students to match up.
use crate::figures::chart;
use crate::figures::language::{lines_of, wrap};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const MAX_LEVELS: usize = 7;
const DEFAULT: &str = "Physiological: food, water, sleep\nSafety: a home, a job, health\nLove and belonging: friends, family\nEsteem: respect, achievement\nSelf-actualisation: reaching your potential";

pub fn def() -> FigureDef {
    FigureDef {
        id: "maslow",
        subject: Subject::Art,
        title: "Hierarchy of needs",
        blurb: "A pyramid of needs (Maslow's by default, or your own levels) with an example beside each level, or empty bands for students to name.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "maslow" => vec![
            Example { name: "Maslow's five levels", params: vec![] },
            Example { name: "A study pyramid", params: vec![("levels", s("Remember: recall a fact\nUnderstand: explain it\nApply: use it\nAnalyse: compare ideas\nCreate: make something new"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("levels", "Levels, bottom first: name: example", DEFAULT).hint("One per line as name: example, from the base up; 2 to 7 levels. The example is optional"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the pyramid and the examples and leaves the level names blank for students"),
    ]
}

fn render(p: &Params) -> String {
    let lines = lines_of(p.s("levels"), 80);
    if lines.len() < 2 {
        return chart::message("Give at least two levels");
    }
    if lines.len() > MAX_LEVELS {
        return chart::message(&format!("A pyramid is limited to {MAX_LEVELS} levels"));
    }
    let levels: Vec<(String, String)> = lines
        .iter()
        .map(|l| match l.split_once(':') {
            Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
            None => (l.trim().to_string(), String::new()),
        })
        .collect();
    let n = levels.len();
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, bh) = (980.0, 84.0);
    let h = top + 40.0 + n as f64 * bh + 30.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (base_w, cx, y0) = (520.0, 300.0, top + 30.0);
    let apex_w = 170.0;
    let width_at = |y: f64| apex_w + (base_w - apex_w) * ((y - y0) / (n as f64 * bh));
    let tint = |i: usize| -> String {
        if print { ["#f4f4f4", "#e2e2e2", "#d0d0d0", "#bebebe", "#acacac", "#9a9a9a", "#888888"][i % 7].into() } else { ["#cfe8d0", "#cfe0fa", "#f7d9a8", "#f0c4d4", "#e3d4f4", "#f9efb0", "#d4e8ee"][i % 7].into() }
    };
    for i in 0..n {
        // level 0 is the base, at the bottom
        let (yt, yb) = (y0 + (n - 1 - i) as f64 * bh, y0 + (n - i) as f64 * bh);
        let (wt, wb) = (width_at(yt), width_at(yb));
        d.poly(&[(cx - wt / 2.0, yt), (cx + wt / 2.0, yt), (cx + wb / 2.0, yb), (cx - wb / 2.0, yb)], &tint(i), INK, 2.0);
        let ym = (yt + yb) / 2.0;
        if answers {
            for (k, l) in wrap(&levels[i].0, 22, 2).iter().enumerate() {
                chart::text(&mut d, cx, ym + fs * 0.35 + k as f64 * fs * 1.2 - if levels[i].0.len() > 22 { fs * 0.6 } else { 0.0 }, fs, "middle", l);
            }
        }
        if !levels[i].1.is_empty() {
            let (x1, x2): (f64, f64) = (cx + wb / 2.0 + 20.0, 600.0);
            d.line(cx + (wt + wb) / 4.0, ym, x2.max(x1) - 6.0, ym, "#9ca3af", 1.0);
            for (k, l) in wrap(&levels[i].1, 40, 2).iter().enumerate() {
                chart::text(&mut d, x2, ym + fs * 0.35 + k as f64 * fs * 1.25 - if levels[i].1.len() > 40 { fs * 0.6 } else { 0.0 }, fs * 0.95, "start", l);
            }
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
    fn key_names_levels_student_keeps_examples() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains(">Esteem<") && key.contains("food, water, sleep"));
        assert!(!student.contains(">Esteem<") && student.contains("food, water, sleep") && student.contains("respect, achievement"));
        assert_eq!(size_of(&student), size_of(&key));
        assert_eq!(key.matches("<polygon").count(), 5);
    }

    #[test]
    fn bad_input_is_explained() {
        let mut p = defaults();
        p.set_s("levels", "one");
        assert!(render(&p).contains("at least two"));
        p.set_s("levels", &"a: b\n".repeat(8));
        assert!(render(&p).contains("limited to 7"));
        p.set_s("levels", "a\nb");
        assert!(sane(&render(&p)));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("maslow").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("at least two"), "{}", ex.name);
        }
        for spec in ["\u{0}\n\u{0}", "<b>:<i>\n&:&", "nan:nan\n1e999:x", &"w".repeat(500), ":\n:", "a:\nb:"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("levels", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
