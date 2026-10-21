//! Production possibilities frontier: a bowed-out or straight frontier from the most of each good
//! that can be made, with named points classed as inside (attainable but inefficient), on
//! (efficient) or outside (unattainable), and the opportunity cost of moving between two points.
//! The frontier and the points are the question; the classes and the opportunity cost are the
//! answers, so the student copy (Show answers off) leaves a blank line after each.
use super::{draw_axes, n2, nice_max, plane};
use crate::figures::chart;
use crate::figures::svg::{self, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const SHAPES: [&str; 2] = ["Bowed out (rising cost)", "Straight line (constant cost)"];
const MAX_POINTS: usize = 6;
const DEFAULT_POINTS: &str = "A 40 60\nB 60 30\nC 30 30\nD 70 70";

pub fn def() -> FigureDef {
    FigureDef {
        id: "ppf",
        subject: Subject::Economics,
        title: "Production possibilities frontier",
        blurb: "A production possibilities frontier with named points classed as inside, on or outside it, and the opportunity cost of moving between two points, or blank lines for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("ppf", "to") => !p.s("from").trim().is_empty(),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "ppf" => vec![
            Example { name: "Points inside, on and outside", params: vec![] },
            Example { name: "Opportunity cost of a move", params: vec![("from", s("A")), ("to", s("B"))] },
            Example { name: "Constant opportunity cost", params: vec![("shape", s(SHAPES[1])), ("points", s("A 20 80\nB 60 40\nC 80 10")), ("from", s("A")), ("to", s("B"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false)), ("from", s("A")), ("to", s("B"))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("x_name", "Good on the x axis", "Wheat"),
        ParamDef::text("y_name", "Good on the y axis", "Cloth"),
        ParamDef::float("x_max", "Most x if only x is made", 1.0, 1.0e6, 1.0, 80.0),
        ParamDef::float("y_max", "Most y if only y is made", 1.0, 1.0e6, 1.0, 80.0),
        ParamDef::choice("shape", "Shape", &SHAPES, SHAPES[0]),
        ParamDef::long("points", "Points", DEFAULT_POINTS).hint("One per line: a name, then x and y, like A 40 60 (up to six)"),
        ParamDef::text("from", "Opportunity cost: from point", "").hint("A point name; leave empty for no opportunity cost"),
        ParamDef::text("to", "Opportunity cost: to point", ""),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off draws the frontier and the points and leaves a blank line for each point's class and for the opportunity cost"),
    ]
}

struct Pt {
    name: String,
    x: f64,
    y: f64,
}

fn parse_points(spec: &str) -> Result<Vec<Pt>, String> {
    let mut out = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [name, x, y] = parts[..] else { return Err(format!("\u{201c}{line}\u{201d} should be a name and two numbers, like A 40 60")) };
        let num = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && (0.0..=1.0e6).contains(v));
        let (Some(x), Some(y)) = (num(x), num(y)) else { return Err(format!("\u{201c}{line}\u{201d} needs two numbers from 0 up")) };
        if out.len() >= MAX_POINTS {
            return Err(format!("A figure is limited to {MAX_POINTS} points"));
        }
        out.push(Pt { name: name.chars().take(8).collect(), x, y });
    }
    Ok(out)
}

/// How far a point is from the frontier: below 1 inside it, 1 on it, above 1 outside it.
pub fn reach(shape: &str, a: f64, b: f64, x: f64, y: f64) -> f64 {
    if shape == SHAPES[1] { x / a + y / b } else { (x / a).powi(2) + (y / b).powi(2) }
}

pub fn class(r: f64) -> &'static str {
    if r > 1.0 + 0.01 {
        "outside: unattainable"
    } else if r < 1.0 - 0.01 {
        "inside: attainable, not efficient"
    } else {
        "on the frontier: efficient"
    }
}

fn render(p: &Params) -> String {
    let (a, b) = (p.f("x_max"), p.f("y_max"));
    if !(a.is_finite() && b.is_finite() && (1.0..=1.0e6).contains(&a) && (1.0..=1.0e6).contains(&b)) {
        return chart::message("Enter the most of each good as a number above 0");
    }
    let pts = match parse_points(p.s("points")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let shape = SHAPES.iter().copied().find(|s| *s == p.s("shape")).unwrap_or(SHAPES[0]);
    let (xn, yn): (String, String) = (p.s("x_name").trim().chars().take(24).collect(), p.s("y_name").trim().chars().take(24).collect());
    let (xn, yn) = (if xn.is_empty() { "Good X".to_string() } else { xn }, if yn.is_empty() { "Good Y".to_string() } else { yn });
    let (from, to) = (p.s("from").trim(), p.s("to").trim());
    let find = |n: &str| pts.iter().find(|q| q.name.eq_ignore_ascii_case(n));
    let cost = if from.is_empty() {
        None
    } else {
        match (find(from), find(to)) {
            (Some(f), Some(t)) => Some((f, t)),
            _ => return chart::message("Name two of the points for the opportunity cost, or leave them empty"),
        }
    };
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let title = p.s("title").trim();
    let xmax = nice_max(pts.iter().fold(a, |m, q| m.max(q.x)) * 1.1);
    let ymax = nice_max(pts.iter().fold(b, |m, q| m.max(q.y)) * 1.1);
    let rows = pts.len() + usize::from(cost.is_some());
    let (mut d, f) = plane(xmax, ymax, 14.0 + rows as f64 * fs * 1.75);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, f.w / 2.0, 16.0, fs * 1.2, "middle", title);
    }
    draw_axes(&mut d, &f, p.b("numbers"), fs, &xn, &yn);
    let curve: Vec<(f64, f64)> = if shape == SHAPES[1] {
        vec![(f.px(0.0), f.py(b)), (f.px(a), f.py(0.0))]
    } else {
        (0..=60).map(|i| {
            let t = std::f64::consts::FRAC_PI_2 * i as f64 / 60.0;
            (f.px(a * t.cos()), f.py(b * t.sin()))
        }).collect()
    };
    d.polyline(&curve, fills.line(0), 3.0);
    for q in &pts {
        d.circle(f.px(q.x), f.py(q.y), 4.8, "#111827", "#fff", 1.4);
        chart::text(&mut d, f.px(q.x) + 8.0, f.py(q.y) - 7.0, fs * 1.05, "start", &q.name);
    }
    let x_val = f.left() + 150.0 * k.clamp(1.0, 1.3);
    let mut y = f.h + 20.0;
    for q in &pts {
        chart::text(&mut d, f.left(), y, fs, "start", &format!("Point {}:", q.name));
        if answers {
            chart::text(&mut d, x_val, y, fs, "start", class(reach(shape, a, b, q.x, q.y)));
        } else {
            d.line(x_val, y + 2.0, f.right(), y + 2.0, "#111827", 1.0);
        }
        y += fs * 1.75;
    }
    if let Some((from, to)) = cost {
        chart::text(&mut d, f.left(), y, fs, "start", &format!("{} to {}:", from.name, to.name));
        if answers {
            let (dx, dy) = (to.x - from.x, to.y - from.y);
            let t = if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
                "no move".to_string()
            } else if dx.abs() < 1e-9 || dy.abs() < 1e-9 || dx * dy > 0.0 {
                format!("{} {} and {} {}: not a trade-off along the frontier", if dx >= 0.0 { "gain" } else { "lose" }, n2(dx.abs()), if dy >= 0.0 { "gain" } else { "lose" }, n2(dy.abs()))
            } else {
                let (gain, give, g, l) = if dx > 0.0 { (&xn, &yn, dx, -dy) } else { (&yn, &xn, dy, -dx) };
                format!("1 {} costs {} {}  ({} {} for {} {})", gain, n2(l / g), give, n2(g), gain, n2(l), give)
            };
            chart::text(&mut d, x_val, y, fs, "start", &t);
        } else {
            d.line(x_val, y + 2.0, f.right(), y + 2.0, "#111827", 1.0);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn points_are_classed_against_the_frontier() {
        // (x/80)^2 + (y/80)^2: A (40,60) 0.8125 and C (30,30) inside; D (70,70) 1.53 outside
        assert_eq!(class(reach(SHAPES[0], 80.0, 80.0, 40.0, 60.0)), "inside: attainable, not efficient");
        assert_eq!(class(reach(SHAPES[0], 80.0, 80.0, 70.0, 70.0)), "outside: unattainable");
        assert_eq!(class(reach(SHAPES[0], 80.0, 80.0, 80.0, 0.0)), "on the frontier: efficient");
        assert_eq!(class(reach(SHAPES[1], 80.0, 80.0, 40.0, 40.0)), "on the frontier: efficient");
        assert_eq!(class(reach(SHAPES[1], 80.0, 80.0, 60.0, 60.0)), "outside: unattainable");
    }

    #[test]
    fn key_writes_each_class() {
        let svg = render(&defaults());
        assert_eq!(svg.matches("inside: attainable, not efficient").count(), 3, "A, B and C");
        assert!(svg.contains(">Point A:<") && svg.contains("outside: unattainable"));
    }

    #[test]
    fn opportunity_cost_is_the_slope_between_points() {
        let mut p = defaults();
        p.set_s("shape", SHAPES[1]);
        p.set_s("points", "A 20 80\nB 60 40");
        p.set_s("from", "A");
        p.set_s("to", "b");
        // gain 40 wheat, give up 40 cloth: 1 wheat costs 1 cloth
        assert!(render(&p).contains("1 Wheat costs 1 Cloth  (40 Wheat for 40 Cloth)"));
        p.set_s("from", "B");
        p.set_s("to", "A");
        assert!(render(&p).contains("1 Cloth costs 1 Wheat"));
        p.set_s("points", "A 20 80\nB 60 90");
        assert!(render(&p).contains("not a trade-off"));
    }

    #[test]
    fn student_copy_has_blank_lines() {
        let mut p = defaults();
        p.set_s("from", "A");
        p.set_s("to", "B");
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains(">Point A:<") && !student.contains("unattainable") && !student.contains("costs"));
        assert!(key.contains("costs"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        for (k, v, needle) in [
            ("points", "A 40", "name and two numbers"),
            ("points", "A x y", "two numbers"),
            ("points", "A 1 -1", "two numbers"),
            ("from", "Z", "Name two of the points"),
        ] {
            let mut p = defaults();
            p.set_s(k, v);
            if k == "from" {
                p.set_s("to", "A");
            }
            assert!(render(&p).contains(needle), "{k} {v}");
        }
        let many: String = (0..7).map(|i| format!("P{i} 1 1\n")).collect();
        let mut p = defaults();
        p.set_s("points", &many);
        assert!(render(&p).contains("limited to 6"));
        let mut p = defaults();
        p.set_f("x_max", 0.0);
        assert!(render(&p).contains("above 0"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("ppf").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for pts in ["", "\u{0}", "A 1e300 1", "A 1e999 1", "\u{1f600} 5 5", "A 0 0\nB 0 0"] {
            for shape in SHAPES {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("points", pts);
                    p.set_s("shape", shape);
                    p.set_s("from", "A");
                    p.set_s("to", "B");
                    p.set_b(SHOW_ANSWERS, answers);
                    assert!(sane(&render(&p)), "{pts:?}");
                }
            }
        }
        for v in [0.0, -1.0, 1.0e12, f64::NAN] {
            let mut p = defaults();
            p.set_f("y_max", v);
            assert!(sane(&render(&p)));
        }
    }
}
