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

/// The outline of the cerebrum from the side: front on the left, flatter underneath, with a gentle
/// ripple so it reads as folded.
fn outline(c: (f64, f64)) -> Vec<(f64, f64)> {
    (0..120)
        .map(|k| {
            let a = (k as f64 * 3.0).to_radians();
            let flat = if a.sin() < 0.0 { 0.78 } else { 1.0 };
            let r = 1.0 + 0.022 * (13.0 * a).sin();
            (c.0 + 235.0 * r * a.cos(), c.1 - 150.0 * flat * r * a.sin())
        })
        .collect()
}

/// The part of `poly` on one side of the line through `a` and `b` (`side` is 1.0 or -1.0).
fn half(poly: &[(f64, f64)], a: (f64, f64), b: (f64, f64), side: f64) -> Vec<(f64, f64)> {
    let f = |p: (f64, f64)| side * ((b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0));
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        let (fp, fq) = (f(p), f(q));
        if fp >= 0.0 {
            out.push(p);
        }
        if (fp >= 0.0) != (fq >= 0.0) {
            let t = fp / (fp - fq);
            out.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
        }
    }
    out
}

fn centroid(poly: &[(f64, f64)]) -> (f64, f64) {
    let n = poly.len().max(1) as f64;
    (poly.iter().map(|p| p.0).sum::<f64>() / n, poly.iter().map(|p| p.1).sum::<f64>() / n)
}

/// The four lobes as polygons: frontal, parietal, occipital, temporal.
pub fn lobes(c: (f64, f64)) -> [Vec<(f64, f64)>; 4] {
    let o = outline(c);
    // the lateral fissure rises from the front underside to the back; the central sulcus crosses the top
    let (l1, l2) = ((c.0 - 260.0, c.1 + 60.0), (c.0 + 120.0, c.1 - 5.0));
    let (c1, c2) = ((c.0 + 20.0, c.1 - 170.0), (c.0 - 50.0, c.1 + 30.0));
    let (o1, o2) = ((c.0 + 120.0, c.1 - 170.0), (c.0 + 150.0, c.1 + 90.0));
    let above = half(&o, l1, l2, -1.0);
    let frontal = half(&above, c1, c2, 1.0);
    let rest = half(&above, c1, c2, -1.0);
    let parietal = half(&rest, o1, o2, 1.0);
    let occipital = half(&o, o1, o2, -1.0);
    let temporal = half(&half(&o, l1, l2, 1.0), o1, o2, 1.0);
    [frontal, parietal, occipital, temporal]
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
    let tint = |i: usize| -> &'static str {
        if print { ["#eeeeee", "#d6d6d6", "#bdbdbd", "#a4a4a4", "#e2e2e2", "#8c8c8c"][i] } else { ["#f4c7c3", "#fbe3a1", "#c7e3b8", "#bcd8f0", "#e0c8ec", "#c9c9c9"][i] }
    };
    let mut extra: Vec<(f64, f64)> = Vec::new();
    if count == 6 {
        let cb = (c.0 + 135.0, c.1 + 135.0);
        let pts: Vec<(f64, f64)> = (0..36)
            .map(|k| {
                let a = (k as f64 * 10.0).to_radians();
                (cb.0 + 85.0 * a.cos(), cb.1 - 45.0 * a.sin() * if a.sin() < 0.0 { 0.9 } else { 1.0 } + 2.5 * (9.0 * a).sin())
            })
            .collect();
        let stem = [(c.0 + 5.0, c.1 + 60.0), (c.0 + 70.0, c.1 + 80.0), (c.0 + 66.0, c.1 + 235.0), (c.0 + 26.0, c.1 + 235.0)];
        d.poly(&stem, tint(5), INK, 2.0);
        d.poly(&pts, tint(4), INK, 2.0);
        extra.push((cb.0 + 25.0, cb.1 + 22.0));
        extra.push((c.0 + 46.0, c.1 + 190.0));
    }
    let regions = lobes(c);
    for (i, poly) in regions.iter().enumerate() {
        d.poly(poly, tint(i), INK, 2.0);
    }
    let mut anchors: Vec<(f64, f64)> = regions.iter().map(|r| centroid(r)).collect();
    anchors.extend(extra);
    for (i, a) in anchors.iter().enumerate() {
        d.circle(a.0, a.1, 14.0, "#fff", INK, 1.8);
        chart::text(&mut d, a.0, a.1 + fs * 0.35, fs, "middle", &(i + 1).to_string());
    }
    chart::text(&mut d, c.0 - 235.0, c.1 - 175.0, fs * 0.85, "start", "Front");
    chart::text(&mut d, c.0 + 235.0, c.1 - 175.0, fs * 0.85, "end", "Back");
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
    fn lobes_are_real_regions_inside_the_outline() {
        let l = lobes((0.0, 0.0));
        for (i, r) in l.iter().enumerate() {
            assert!(r.len() >= 4, "lobe {i}");
            assert!(r.iter().all(|p| p.0.abs() <= 245.0 && p.1.abs() <= 160.0), "lobe {i}");
        }
        let area = |r: &Vec<(f64, f64)>| (0..r.len()).map(|i| r[i].0 * r[(i + 1) % r.len()].1 - r[(i + 1) % r.len()].0 * r[i].1).sum::<f64>().abs() / 2.0;
        assert!(l.iter().all(|r| area(r) > 4000.0), "every lobe is big enough to number");
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
