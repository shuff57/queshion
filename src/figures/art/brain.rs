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

type Pt = (f64, f64);

/// A smooth curve through control points (Catmull-Rom), closed or open.
fn spline(ctrl: &[Pt], closed: bool, per: usize) -> Vec<Pt> {
    let n = ctrl.len();
    let at = |i: isize| -> Pt {
        if closed { ctrl[i.rem_euclid(n as isize) as usize] } else { ctrl[i.clamp(0, n as isize - 1) as usize] }
    };
    let segs = if closed { n } else { n - 1 };
    let mut out = Vec::new();
    for i in 0..segs as isize {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 0..per {
            let t = k as f64 / per as f64;
            let f = |a: f64, b: f64, c: f64, d: f64| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t + (3.0 * b - a - 3.0 * c + d) * t * t * t);
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    if !closed {
        out.push(ctrl[n - 1]);
    }
    out
}

fn nearest(path: &[Pt], p: Pt) -> usize {
    let d = |q: &Pt| (q.0 - p.0).powi(2) + (q.1 - p.1).powi(2);
    (0..path.len()).min_by(|&a, &b| d(&path[a]).total_cmp(&d(&path[b]))).unwrap_or(0)
}

/// The points of a closed path from index `i` forward to index `j`, wrapping round.
fn arc(path: &[Pt], i: usize, j: usize) -> Vec<Pt> {
    let n = path.len();
    let mut v = vec![path[i]];
    let mut k = i;
    while k != j {
        k = (k + 1) % n;
        v.push(path[k]);
    }
    v
}

fn shift(v: &[Pt], c: Pt) -> Vec<Pt> {
    v.iter().map(|p| (p.0 + c.0, p.1 + c.1)).collect()
}

/// The outline of the cerebrum from the side, front on the left: a rounded frontal pole, a long
/// top, the occipital pole, and the underside with the temporal lobe bulging below and pointing
/// forward.
pub fn outline(c: Pt) -> Vec<Pt> {
    let ctrl = [(-236.0, 12.0), (-228.0, -45.0), (-196.0, -95.0), (-140.0, -132.0), (-65.0, -150.0), (20.0, -154.0), (105.0, -144.0), (170.0, -114.0), (214.0, -65.0), (236.0, -5.0), (228.0, 48.0), (196.0, 80.0), (150.0, 92.0), (95.0, 98.0), (40.0, 106.0), (-20.0, 118.0), (-80.0, 122.0), (-132.0, 112.0), (-166.0, 92.0), (-172.0, 66.0), (-196.0, 52.0), (-226.0, 38.0)];
    spline(&shift(&ctrl, c), true, 10)
}

/// The curves that divide the lobes: lateral fissure, central sulcus and parieto-occipital line.
pub fn dividers(c: Pt) -> [Vec<Pt>; 3] {
    let f = spline(&shift(&[(-180.0, 58.0), (-120.0, 44.0), (-55.0, 32.0), (15.0, 10.0), (75.0, -8.0), (120.0, -30.0)], c), false, 10);
    let cc = spline(&shift(&[(12.0, -154.0), (2.0, -112.0), (-18.0, -72.0), (-28.0, -36.0), (-44.0, 10.0)], c), false, 10);
    let pp = spline(&shift(&[(128.0, -134.0), (112.0, -92.0), (120.0, -45.0), (138.0, 5.0), (158.0, 50.0), (178.0, 92.0)], c), false, 10);
    [f, cc, pp]
}

/// The four lobes as polygons: frontal, parietal, occipital, temporal. Each is bounded by the
/// outline and the curved dividers, which are snapped onto the outline and onto each other.
pub fn lobes(c: Pt) -> [Vec<Pt>; 4] {
    let o = outline(c);
    let [mut f, mut cc, mut pp] = dividers(c);
    let (i_f, i_c, i_p0) = (nearest(&o, f[0]), nearest(&o, cc[0]), nearest(&o, pp[0]));
    let i_p1 = nearest(&o, *pp.last().unwrap());
    f[0] = o[i_f];
    cc[0] = o[i_c];
    pp[0] = o[i_p0];
    let last = pp.len() - 1;
    pp[last] = o[i_p1];
    let m = nearest(&pp, *f.last().unwrap());
    *f.last_mut().unwrap() = pp[m];
    let j = nearest(&f, *cc.last().unwrap()).min(f.len() - 2);
    *cc.last_mut().unwrap() = f[j];

    let mut frontal = arc(&o, i_f, i_c);
    frontal.extend(cc[1..].iter());
    frontal.extend(f[..j].iter().rev());

    let mut parietal = arc(&o, i_c, i_p0);
    parietal.extend(pp[1..=m].iter());
    parietal.extend(f[j..].iter().rev());
    parietal.extend(cc[..cc.len() - 1].iter().rev());

    let mut occipital = arc(&o, i_p0, i_p1);
    occipital.extend(pp[..last].iter().rev());

    let mut temporal = f.clone();
    temporal.extend(pp[m + 1..].iter());
    temporal.extend(arc(&o, i_p1, i_f).iter().skip(1));
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
    let at = |x: f64, y: f64| (c.0 + x, c.1 + y);
    let ink2 = if print { "#666666" } else { "#7a6a8a" };
    let mut anchors: Vec<(f64, f64)> = vec![at(-110.0, -30.0), at(60.0, -85.0), at(185.0, -20.0), at(-45.0, 80.0)];
    if count == 6 {
        // brain stem behind, leaning back, then the cerebellum with its folds
        let stem = [at(-8.0, 60.0), at(46.0, 66.0), at(78.0, 150.0), at(82.0, 240.0), at(36.0, 240.0), at(30.0, 160.0)];
        d.poly(&spline(&stem, true, 6), tint(5), INK, 2.0);
        let (cb, rx, ry) = (at(150.0, 112.0), 78.0, 46.0);
        let pts: Vec<(f64, f64)> = (0..48)
            .map(|k| {
                let a = k as f64 * std::f64::consts::TAU / 48.0;
                (cb.0 + rx * a.cos(), cb.1 - ry * a.sin() * if a.sin() < 0.0 { 0.85 } else { 1.0 })
            })
            .collect();
        d.poly(&pts, tint(4), INK, 2.0);
        for t in -2..=2 {
            let dy = t as f64 * 11.0;
            let w = rx * (1.0 - (dy / ry).powi(2)).max(0.0).sqrt() * 0.92;
            let line: Vec<(f64, f64)> = (0..=10).map(|k| { let x = -w + 2.0 * w * k as f64 / 10.0; (cb.0 + x, cb.1 + dy + 3.0 * (x / w.max(1.0) * 3.0).sin()) }).collect();
            d.polyline(&line, ink2, 1.2);
        }
        anchors.push(at(150.0, 128.0));
        anchors.push(at(58.0, 200.0));
    }
    let regions = lobes(c);
    for (i, poly) in regions.iter().enumerate() {
        d.poly(poly, tint(i), INK, 2.0);
    }
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
            assert!(r.iter().all(|p| p.0.abs() <= 245.0 && p.1.abs() <= 160.0 && p.0.is_finite() && p.1.is_finite()), "lobe {i}");
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
