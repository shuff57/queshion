//! A SWOT grid: strengths and weaknesses (inside the business), opportunities and threats
//! (outside it), each a list of points. The points are listed numbered in a mixed-up order;
//! which box each belongs in is the answer, so the student copy (Show answers off) leaves the four
//! boxes empty for the numbers to be sorted into them.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const NAMES: [&str; 4] = ["Strengths", "Weaknesses", "Opportunities", "Threats"];
const KEYS: [&str; 4] = ["strengths", "weaknesses", "opportunities", "threats"];
const INK: &str = "#111827";
const MAX_EACH: usize = 6;
const MAX_LEN: usize = 44;
const DEFAULTS: [&str; 4] = [
    "Loyal local customers\nFresh, high-quality ingredients",
    "Small kitchen limits output\nNo online ordering",
    "New office block opening nearby\nGrowing demand for catering",
    "A national chain opens in town\nRising flour prices",
];

pub fn def() -> FigureDef {
    FigureDef {
        id: "swot",
        subject: Subject::Business,
        title: "SWOT analysis",
        blurb: "A SWOT grid of strengths, weaknesses, opportunities and threats, with the numbered points sorted into their boxes, or empty boxes for students to sort the points into.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "swot" => vec![
            Example { name: "A bakery", params: vec![] },
            Example { name: "A school", params: vec![("strengths", s("Experienced teachers\nStrong exam results\nGood sports facilities")), ("weaknesses", s("Ageing buildings\nLimited parking")), ("opportunities", s("New housing nearby\nGrant for a science lab")), ("threats", s("Falling local birth rate\nNew free school opening"))] },
            Example { name: "One point per box", params: vec![("strengths", s("Strong brand")), ("weaknesses", s("High prices")), ("opportunities", s("New markets")), ("threats", s("Competitors"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("strengths", "Strengths (inside, helpful)", DEFAULTS[0]).hint("One point per line, up to 6"),
        ParamDef::long("weaknesses", "Weaknesses (inside, harmful)", DEFAULTS[1]),
        ParamDef::long("opportunities", "Opportunities (outside, helpful)", DEFAULTS[2]),
        ParamDef::long("threats", "Threats (outside, harmful)", DEFAULTS[3]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the grid and the numbered list of points and leaves the four boxes empty for students to sort the points into"),
    ]
}

/// The points of each quadrant, trimmed and cut to length.
fn points(p: &Params) -> Result<[Vec<String>; 4], String> {
    let mut out: [Vec<String>; 4] = Default::default();
    for (q, key) in KEYS.iter().enumerate() {
        let v: Vec<String> = p.s(key).lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| l.chars().take(MAX_LEN).collect()).collect();
        if v.len() > MAX_EACH {
            return Err(format!("{} is limited to {MAX_EACH} points", NAMES[q]));
        }
        out[q] = v;
    }
    if out.iter().all(|v| v.is_empty()) {
        return Err("Give some points in at least one box".into());
    }
    Ok(out)
}

/// The order the points are listed in for sorting: a fixed shuffle that mixes the boxes up.
pub fn shuffled(n: usize) -> Vec<usize> {
    let gcd = |mut a: usize, mut b: usize| { while b != 0 { (a, b) = (b, a % b); } a };
    let k = [11usize, 7, 5, 3].into_iter().find(|k| gcd(*k, n.max(1)) == 1).unwrap_or(1);
    (0..n).map(|j| (j * k + 1) % n.max(1)).collect()
}

fn render(p: &Params) -> String {
    let pts = match points(p) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    // number the points in their box order, then list them in mixed-up order
    let mut all: Vec<(usize, &String)> = Vec::new();
    for (q, v) in pts.iter().enumerate() {
        for t in v {
            all.push((q, t));
        }
    }
    let n = all.len();
    let order = shuffled(n);
    // number each point by its place in the mixed-up list
    let mut number = vec![0usize; n];
    for (pos, &orig) in order.iter().enumerate() {
        number[orig] = pos + 1;
    }
    let list_rows = (n + 1) / 2;
    let (qw, qh) = (420.0, 40.0 + MAX_EACH as f64 * fs * 1.7);
    let h = top + 70.0 + 2.0 * qh + 40.0 + list_rows as f64 * fs * 1.9 + 30.0;
    let w = 60.0 + 2.0 * qw + 40.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (gx, gy) = (70.0, top + 50.0);
    let fills = if print { ["#f2f2f2", "#e2e2e2", "#f2f2f2", "#e2e2e2"] } else { ["#dff0e1", "#fbdcd7", "#dbe8fa", "#fdeccf"] };
    chart::text(&mut d, gx + qw / 2.0, gy - 14.0, fs * 1.05, "middle", "Helpful");
    chart::text(&mut d, gx + qw * 1.5 + 10.0, gy - 14.0, fs * 1.05, "middle", "Harmful");
    chart::vtext(&mut d, gx - 20.0, gy + qh / 2.0, fs * 1.05, "Internal");
    chart::vtext(&mut d, gx - 20.0, gy + qh * 1.5, fs * 1.05, "External");
    // quadrant order: S top-left, W top-right, O bottom-left, T bottom-right
    let pos = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
    let mut idx = 0;
    for (q, (cx, cy)) in pos.iter().enumerate() {
        let (x, y) = (gx + cx * (qw + 10.0), gy + cy * qh);
        d.rect(x, y, qw, qh, fills[q], INK, 1.8);
        chart::text(&mut d, x + 12.0, y + 26.0, fs * 1.2, "start", NAMES[q]);
        for (k, t) in pts[q].iter().enumerate() {
            let num = number[idx + k];
            if answers {
                chart::text(&mut d, x + 14.0, y + 52.0 + k as f64 * fs * 1.7, fs, "start", &format!("{num}. {t}"));
            }
        }
        idx += pts[q].len();
    }
    // the list
    let ly = gy + 2.0 * qh + 44.0;
    chart::text(&mut d, gx, ly - 14.0, fs * 1.05, "start", "Points to sort");
    for (pos_i, &orig) in order.iter().enumerate() {
        let (col, row) = (pos_i / list_rows, pos_i % list_rows);
        let text = format!("{}. {}", pos_i + 1, all[orig].1);
        chart::text(&mut d, gx + col as f64 * (qw + 10.0), ly + 8.0 + row as f64 * fs * 1.9, fs * 0.95, "start", &text);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn the_shuffle_is_a_permutation_that_mixes() {
        for n in 1..=24 {
            let mut s = shuffled(n);
            s.sort();
            assert_eq!(s, (0..n).collect::<Vec<_>>(), "{n}");
        }
        assert_eq!(shuffled(0), Vec::<usize>::new());
        assert_ne!(shuffled(8), (0..8).collect::<Vec<_>>());
        assert_ne!(shuffled(2), vec![0, 1], "even two points are swapped");
    }

    #[test]
    fn key_places_each_numbered_point_in_its_box() {
        let svg = render(&defaults());
        for name in NAMES {
            assert!(svg.contains(&format!(">{name}<")), "{name}");
        }
        let pts = points(&defaults()).unwrap();
        let order = shuffled(8);
        // the first strength is item 0 and appears in the list at position order.iter().position(0)+1
        let num = order.iter().position(|&o| o == 0).unwrap() + 1;
        assert!(svg.contains(&format!(">{num}. {}<", pts[0][0])));
        // each point appears twice in the key (box and list) and once in the student copy
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert_eq!(key.matches("Loyal local customers").count(), 2);
        assert_eq!(student.matches("Loyal local customers").count(), 1);
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn the_student_list_does_not_group_the_boxes() {
        let svg = render(&{ let mut p = defaults(); p.set_b(SHOW_ANSWERS, false); p });
        let first = svg.find(">1. ").unwrap();
        let second = svg.find(">2. ").unwrap();
        assert!(first < second);
        assert!(!svg.contains(">1. Loyal local customers<") || !svg.contains(">2. Fresh, high-quality ingredients<"), "the first two points are not both strengths in order");
    }

    #[test]
    fn too_many_or_no_points_are_explained() {
        let mut p = defaults();
        p.set_s("strengths", &"x\n".repeat(7));
        assert!(render(&p).contains("Strengths is limited to 6"));
        for k in KEYS {
            p.set_s(k, "");
        }
        assert!(render(&p).contains("Give some points"));
        p.set_s("strengths", "only one");
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
        for ex in examples("swot").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("limited to") && !svg.contains("Give some"), "{}", ex.name);
        }
        for spec in ["\u{0}", "<b>x</b>", "nan", &"w".repeat(5000), "a\n\n\n b \n"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("strengths", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
