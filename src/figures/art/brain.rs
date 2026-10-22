//! A simplified side view of the brain with its main regions numbered. What each region does is the
//! question; the names of the regions are the answers, so the student copy (Show answers off) keeps
//! the numbered regions and the list of functions and leaves the names blank. The outline is a
//! simple original drawing, not anatomy.
use crate::figures::chart;
use crate::figures::language::wrap;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const SHOWS: [&str; 2] = ["Lobes and parts", "Lobes only"];
// name, what it does
const REGIONS: [(&str, &str); 6] = [
    ("Frontal lobe", "planning, decisions, movement, personality"),
    ("Parietal lobe", "touch, body position, spatial sense"),
    ("Occipital lobe", "seeing: it processes what the eyes send"),
    ("Temporal lobe", "hearing, language, memory"),
    ("Cerebellum", "balance and coordinated movement"),
    ("Brain stem", "breathing, heart rate, sleep"),
];

pub fn def() -> FigureDef {
    FigureDef {
        id: "brain-regions",
        subject: Subject::Art,
        title: "Brain regions",
        blurb: "A simplified side view of the brain with the four lobes, the cerebellum and the brain stem numbered, with what each does, or blank names for students to fill in.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "brain-regions" => vec![
            Example { name: "Lobes and parts", params: vec![] },
            Example { name: "Lobes only", params: vec![("show", s(SHOWS[1]))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("show", "Regions to show", &SHOWS, SHOWS[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the numbered regions and what each does and leaves the names blank for students"),
    ]
}

fn sector(c: (f64, f64), rx: f64, ry: f64, a0: f64, a1: f64) -> Vec<(f64, f64)> {
    let mut pts = vec![c];
    let steps = 24;
    for k in 0..=steps {
        let a = a0 + (a1 - a0) * k as f64 / steps as f64;
        pts.push((c.0 + rx * a.to_radians().cos(), c.1 - ry * a.to_radians().sin()));
    }
    pts
}

fn render(p: &Params) -> String {
    let count = if p.s("show") == SHOWS[1] { 4 } else { 6 };
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (1100.0, top + 480.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let c = (290.0, top + 190.0);
    let (rx, ry) = (230.0, 160.0);
    let tint = |i: usize| -> &'static str {
        if print { ["#eeeeee", "#d6d6d6", "#bdbdbd", "#a4a4a4", "#e2e2e2", "#8c8c8c"][i] } else { ["#f4c7c3", "#fbe3a1", "#c7e3b8", "#bcd8f0", "#e0c8ec", "#c9c9c9"][i] }
    };
    let mut extra: Vec<(f64, f64)> = Vec::new();
    if count == 6 {
        let cb = (c.0 + 130.0, c.1 + 150.0);
        let mut pts = Vec::new();
        for k in 0..=36 {
            let a = (k as f64 * 10.0).to_radians();
            pts.push((cb.0 + 80.0 * a.cos(), cb.1 - 42.0 * a.sin()));
        }
        let stem = [(c.0 + 10.0, c.1 + 90.0), (c.0 + 70.0, c.1 + 100.0), (c.0 + 62.0, c.1 + 235.0), (c.0 + 22.0, c.1 + 235.0)];
        d.poly(&stem, tint(5), INK, 2.0);
        d.poly(&pts, tint(4), INK, 2.0);
        extra.push((cb.0 + 20.0, cb.1 + 22.0));
        extra.push((c.0 + 42.0, c.1 + 205.0));
    }
    // the front of the brain is on the left; angles run anticlockwise from the right
    let ranges = [(125.0, 235.0), (35.0, 125.0), (-40.0, 35.0), (235.0, 320.0)];
    for (i, (a0, a1)) in ranges.iter().enumerate() {
        d.poly(&sector(c, rx, ry, *a0, *a1), tint(i), INK, 2.0);
    }
    let mut anchors: Vec<(f64, f64)> = Vec::new();
    for (a0, a1) in ranges.iter() {
        let m = ((a0 + a1) / 2.0).to_radians();
        anchors.push((c.0 + rx * 0.58 * m.cos(), c.1 - ry * 0.58 * m.sin()));
    }
    anchors.extend(extra);
    for (i, a) in anchors.iter().enumerate() {
        d.circle(a.0, a.1, 14.0, "#fff", INK, 1.8);
        chart::text(&mut d, a.0, a.1 + fs * 0.35, fs, "middle", &(i + 1).to_string());
    }
    chart::text(&mut d, c.0 - rx, c.1 - ry - 12.0, fs * 0.85, "start", "Front");
    chart::text(&mut d, c.0 + rx, c.1 - ry - 12.0, fs * 0.85, "end", "Back");
    // the list
    let (tx, mut y) = (610.0, top + 60.0);
    chart::text(&mut d, tx, y - 24.0, fs * 1.05, "start", "Name each region");
    for i in 0..count {
        chart::text(&mut d, tx, y, fs, "start", &format!("{}.", i + 1));
        if answers {
            chart::text(&mut d, tx + 26.0, y, fs, "start", REGIONS[i].0);
        } else {
            d.line(tx + 26.0, y + 3.0, tx + 190.0, y + 3.0, INK, 1.0);
        }
        for (k, l) in wrap(REGIONS[i].1, 34, 2).iter().enumerate() {
            chart::text(&mut d, tx + 210.0, y + k as f64 * fs * 1.2, fs * 0.88, "start", l);
        }
        y += fs * 3.2;
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn key_names_regions_student_keeps_functions() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains(">Cerebellum<") && !student.contains(">Cerebellum<") && student.contains("balance and coordinated"));
        assert_eq!(size_of(&student), size_of(&key));
        assert_eq!(key.matches("<polygon").count(), 6);
    }

    #[test]
    fn lobes_only_drops_the_extra_parts() {
        let mut p = defaults();
        p.set_s("show", SHOWS[1]);
        let svg = render(&p);
        assert_eq!(svg.matches("<polygon").count(), 4);
        assert!(!svg.contains("Brain stem") && svg.contains("Temporal lobe"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("brain-regions").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        let mut p = defaults();
        p.set_s("show", "<b>nan");
        p.set_s("title", "<b>&");
        assert!(sane(&render(&p)));
    }
}
