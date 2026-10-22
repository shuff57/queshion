//! An experimental design flow: a sample is randomly split into a treatment group and a control
//! group, both are measured and the results compared. What the experiment is about is the
//! question; the role of each step (random assignment, control group, independent and dependent
//! variable) is the answer, so the student copy (Show answers off) leaves those labels blank.
use crate::figures::chart;
use crate::figures::language::wrap;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const MAX_GROUPS: usize = 4;

pub fn def() -> FigureDef {
    FigureDef {
        id: "experiment-flow",
        subject: Subject::Art,
        title: "Experimental design",
        blurb: "A flow chart of a controlled experiment, from the sample through random assignment to a treatment and a control group, then measuring and comparing, with the role of each step as answers.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "experiment-flow" => vec![
            Example { name: "Sleep and test scores", params: vec![] },
            Example { name: "Plant food", params: vec![("sample", s("40 bean plants")), ("groups", s("Fertiliser in the water\nPlant food and fertiliser\nPlain water")), ("measure", s("Height after 3 weeks")), ("question", s("Does fertiliser make beans grow taller?"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("question", "Question being tested", "Does more sleep raise test scores?"),
        ParamDef::text("sample", "Sample", "60 students"),
        ParamDef::long("groups", "What each group gets (the control group last)", "8 hours of sleep\nTheir usual sleep").hint("One per line, 2 to 4 groups; every group but the last is a treatment group and the last is the control group"),
        ParamDef::text("measure", "What is measured", "Score on a test"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the boxes and leaves the role labels (such as control group) blank for students"),
    ]
}

fn cut(s: &str, n: usize) -> String {
    s.trim().chars().take(n).collect()
}

fn render(p: &Params) -> String {
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (980.0, top + 560.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let fill = if print { "#ececec" } else { "#f0e9db" };
    let fill2 = if print { "#d6d6d6" } else { "#f7d9a8" };
    let q = cut(p.s("question"), 90);
    chart::text(&mut d, 30.0, top + 30.0, fs * 1.05, "start", &q);
    let groups = crate::figures::language::lines_of(p.s("groups"), 60);
    if groups.len() < 2 {
        return chart::message("Give at least two groups, with the control group last");
    }
    let groups: Vec<String> = groups.into_iter().take(MAX_GROUPS).collect();
    let n = groups.len();
    let bw = 220.0;
    let bh = 70.0;
    let bx = |d: &mut Doc, x: f64, y: f64, text: &str, f: &str| {
        d.rect(x, y, bw, bh, f, INK, 2.0);
        let lines = wrap(text, 24, 3);
        for (k, l) in lines.iter().enumerate() {
            chart::text(d, x + bw / 2.0, y + bh / 2.0 + fs * 0.35 - (lines.len() as f64 - 1.0) * fs * 0.62 + k as f64 * fs * 1.25, fs, "middle", l);
        }
    };
    let label = |d: &mut Doc, x: f64, y: f64, text: &str| {
        if answers {
            chart::text(d, x, y, fs * 0.9, "middle", text);
        } else {
            d.line(x - 70.0, y + 3.0, x + 70.0, y + 3.0, INK, 1.0);
        }
    };
    let (cx, y1) = (490.0, top + 70.0);
    bx(&mut d, cx - bw / 2.0, y1, &cut(p.s("sample"), 60), fill);
    label(&mut d, cx + 180.0, y1 + bh / 2.0 - 6.0, "The sample");
    // random assignment splits the sample into the groups
    let ysplit = y1 + bh + 62.0;
    d.arrow(cx, y1 + bh, cx, ysplit - 22.0, INK, 2.2);
    d.circle(cx, ysplit, 22.0, "#fff", INK, 2.0);
    chart::text(&mut d, cx, ysplit + fs * 0.35, fs * 0.9, "middle", "?");
    label(&mut d, cx + 180.0, ysplit - 6.0, "Random assignment");
    let span = match n { 2 => 250.0, 3 => 330.0, _ => 360.0 };
    let gx = |i: usize| if n == 1 { cx } else { cx - span + 2.0 * span * i as f64 / (n as f64 - 1.0) };
    let y2 = ysplit + 90.0;
    d.line(gx(0), ysplit, cx - 22.0, ysplit, INK, 2.2);
    d.line(cx + 22.0, ysplit, gx(n - 1), ysplit, INK, 2.2);
    let mut treat = 0;
    for (i, g) in groups.iter().enumerate() {
        let x = gx(i);
        let from = if (x - cx).abs() < 1.0 { ysplit + 22.0 } else { ysplit };
        d.arrow(x, from, x, y2 - 2.0, INK, 2.2);
        let control = i == n - 1;
        bx(&mut d, x - bw / 2.0, y2, g, if control { fill } else { fill2 });
        let name = if control { "Control group".to_string() } else if n > 2 { treat += 1; format!("Treatment group {treat}") } else { "Treatment group".to_string() };
        label(&mut d, x, y2 + bh + 22.0, &name);
        d.line(x, y2 + bh + 30.0, x, y2 + bh + 56.0, INK, 2.2);
    }
    label(&mut d, gx(0) + 100.0, y2 - 24.0, "Independent variable");
    // all groups are measured
    let y3 = y2 + bh + 76.0;
    d.line(gx(0), y2 + bh + 56.0, gx(n - 1), y2 + bh + 56.0, INK, 2.2);
    d.arrow(cx, y2 + bh + 56.0, cx, y3 + 10.0, INK, 2.2);
    bx(&mut d, cx - bw / 2.0, y3 + 12.0, &format!("Measure: {}", cut(p.s("measure"), 50)), fill);
    label(&mut d, cx + 180.0, y3 + 12.0 + bh / 2.0 - 6.0, "Dependent variable");
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn key_labels_roles_student_keeps_the_boxes() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        for t in [">Control group<", ">Random assignment<", ">Dependent variable<", ">Independent variable<"] {
            assert!(key.contains(t) && !student.contains(t), "{t}");
        }
        for t in ["60 students", "8 hours of sleep", "usual sleep", "Does more sleep"] {
            assert!(student.contains(t) && key.contains(t), "{t}");
        }
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn several_treatment_groups() {
        let mut p = defaults();
        p.set_s("groups", "A\nB\nC\nD");
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        for t in [">Treatment group 1<", ">Treatment group 2<", ">Treatment group 3<", ">Control group<"] {
            assert!(key.contains(t) && !student.contains(t), "{t}");
        }
        assert_eq!(size_of(&student), size_of(&key));
        p.set_s("groups", "only one");
        assert!(render(&p).contains("at least two groups"));
        p.set_s("groups", &"g\n".repeat(9));
        assert!(sane(&render(&p)) && !render(&p).contains("Treatment group 4"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("experiment-flow").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "<b>x</b>", "nan", &"w".repeat(500), "&"] {
            for answers in [true, false] {
                let mut p = defaults();
                for k in ["question", "sample", "groups", "measure"] {
                    p.set_s(k, spec);
                }
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
