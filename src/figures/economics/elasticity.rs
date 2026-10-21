//! Price elasticity of demand from two points on a demand curve, worked out by the midpoint or the
//! simple (starting-point) formula, with the classification and the effect on total revenue. The
//! two points are the question; every working line and the verdict are the answers, so the
//! student copy (Show answers off) leaves a blank line after each step.
use super::{clip_line, dot, draw_axes, draw_line, guides, n2, nice_max, plane};
use crate::figures::chart;
use crate::figures::svg::{self, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const METHODS: [&str; 2] = ["Midpoint", "Starting point"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "elasticity",
        subject: Subject::Economics,
        title: "Price elasticity of demand",
        blurb: "Two points on a demand curve with the elasticity worked out step by step (midpoint or starting-point formula), the verdict and the effect on total revenue, or blank working lines for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "elasticity" => vec![
            Example { name: "Elastic demand (midpoint)", params: vec![] },
            Example { name: "Inelastic demand", params: vec![("p1", Value::F(10.0)), ("q1", Value::F(100.0)), ("p2", Value::F(12.0)), ("q2", Value::F(96.0))] },
            Example { name: "Starting-point formula", params: vec![("method", s(METHODS[1]))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("p1", "First price", 0.0, 1.0e6, 1.0, 8.0),
        ParamDef::float("q1", "First quantity", 0.0, 1.0e6, 1.0, 40.0),
        ParamDef::float("p2", "Second price", 0.0, 1.0e6, 1.0, 6.0),
        ParamDef::float("q2", "Second quantity", 0.0, 1.0e6, 1.0, 60.0),
        ParamDef::choice("method", "Formula", &METHODS, METHODS[0]),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the two points and leaves a blank line after each step of the working"),
    ]
}

/// The verdict for a size of elasticity.
pub fn verdict(e: f64) -> &'static str {
    if (e - 1.0).abs() < 0.005 {
        "unit elastic"
    } else if e > 1.0 {
        "elastic"
    } else {
        "inelastic"
    }
}

fn pct(n: &str, s: f64) -> String {
    format!("{n}: {}%", n2(s * 100.0))
}

fn render(p: &Params) -> String {
    let (p1, q1, p2, q2) = (p.f("p1"), p.f("q1"), p.f("p2"), p.f("q2"));
    if ![p1, q1, p2, q2].iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0e6) {
        return chart::message("Enter the prices and quantities as numbers");
    }
    if (p1 - p2).abs() < 1e-9 || (q1 - q2).abs() < 1e-9 {
        return chart::message("The two points need different prices and different quantities");
    }
    let mid = p.s("method") == METHODS[0];
    let (dq, dp) = (q2 - q1, p2 - p1);
    let (bq, bp) = if mid { ((q1 + q2) / 2.0, (p1 + p2) / 2.0) } else { (q1, p1) };
    if bq <= 0.0 || bp <= 0.0 {
        return chart::message("The starting quantity and price must be above 0");
    }
    let (pq, pp) = (dq / bq, dp / bp);
    let e = (pq / pp).abs();
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let title = p.s("title").trim();
    let xmax = nice_max(q1.max(q2) * 1.3);
    let ymax = nice_max(p1.max(p2) * 1.3);
    let rows = 5.0;
    let (mut d, f) = plane(xmax, ymax, 14.0 + rows * fs * 1.75);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, f.w / 2.0, 16.0, fs * 1.2, "middle", title);
    }
    draw_axes(&mut d, &f, p.b("numbers"), fs, "Quantity", "Price");
    // the demand line through the two points, a little beyond each
    let m = dp / dq;
    let k0 = p1 - m * q1;
    draw_line(&mut d, &f, m, k0, fills.line(0), fills.dash(0), 3.0);
    for (i, (q, pr)) in [(q1, p1), (q2, p2)].into_iter().enumerate() {
        guides(&mut d, &f, q, pr);
        dot(&mut d, &f, q, pr);
        let name = if i == 0 { "A" } else { "B" };
        chart::text(&mut d, f.px(q) + 9.0, f.py(pr) - 9.0, fs * 1.1, "start", name);
    }
    if let Some((qe, pe)) = clip_line(&f, m, k0).map(|x| x.1) {
        chart::text(&mut d, f.px(qe) - 4.0, f.py(pe) + fs * 1.3, fs * 1.1, "end", "D");
    }
    let (tr1, tr2) = (p1 * q1, p2 * q2);
    let lines: Vec<(String, String)> = vec![
        ("Change in quantity".into(), format!("({} \u{2212} {}) \u{f7} {} = {}", n2(q2), n2(q1), n2(bq), pct("", pq).trim_start_matches(": ").to_string())),
        ("Change in price".into(), format!("({} \u{2212} {}) \u{f7} {} = {}", n2(p2), n2(p1), n2(bp), pct("", pp).trim_start_matches(": ").to_string())),
        ("Elasticity".into(), format!("|{}% \u{f7} {}%| = {}", n2(pq * 100.0), n2(pp * 100.0), n2(e))),
        ("Demand is".into(), format!("{} (elasticity {} 1)", verdict(e), if (e - 1.0).abs() < 0.005 { "=" } else if e > 1.0 { ">" } else { "<" })),
        (
            "Total revenue".into(),
            format!("{} \u{2192} {}: {}", n2(tr1), n2(tr2), if (tr2 - tr1).abs() < 1e-9 { "unchanged" } else if tr2 > tr1 { "rises" } else { "falls" }),
        ),
    ];
    let x_label = f.left();
    let x_val = f.left() + 150.0 * k.max(1.0).min(1.3);
    for (i, (lab, val)) in lines.iter().enumerate() {
        let y = f.h + 20.0 + i as f64 * fs * 1.75;
        chart::text(&mut d, x_label, y, fs, "start", &format!("{lab}:"));
        if answers {
            chart::text(&mut d, x_val, y, fs, "start", val);
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
    fn midpoint_works_out_the_defaults() {
        let svg = render(&defaults());
        // dq 20 over 50 = 40%; dp -2 over 7 = -28.57%; e = 1.4
        for t in ["(60 \u{2212} 40) \u{f7} 50 = 40%", "(6 \u{2212} 8) \u{f7} 7 = \u{2212}28.57%", "= 1.4<", "elastic (elasticity &gt; 1)", "320 \u{2192} 360: rises"] {
            assert!(svg.contains(t) || svg.contains(&t.replace('>', "&gt;")), "{t}");
        }
    }

    #[test]
    fn starting_point_uses_the_first_point() {
        let mut p = defaults();
        p.set_s("method", METHODS[1]);
        // 20/40 = 50%; -2/8 = -25%; e = 2
        assert!(render(&p).contains("= 2<"));
    }

    #[test]
    fn verdicts_cover_the_three_cases() {
        assert_eq!((verdict(0.3), verdict(1.0), verdict(1.004), verdict(2.5)), ("inelastic", "unit elastic", "unit elastic", "elastic"));
        let mut p = defaults();
        p.set_f("p1", 10.0);
        p.set_f("q1", 100.0);
        p.set_f("p2", 12.0);
        p.set_f("q2", 96.0);
        let svg = render(&p);
        assert!(svg.contains("inelastic") && svg.contains("1,200") == false && svg.contains("falls") == false);
    }

    #[test]
    fn student_copy_is_blank_lines() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Elasticity:<") && !student.contains("= 1.4") && key.contains("= 1.4"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_points_are_explained() {
        for (k, v, needle) in [("p2", 8.0, "different prices"), ("q2", 40.0, "different quantities"), ("q1", f64::NAN, "as numbers"), ("p1", -1.0, "as numbers")] {
            let mut p = defaults();
            p.set_f(k, v);
            assert!(render(&p).contains(needle), "{k}");
        }
        let mut p = defaults();
        p.set_s("method", METHODS[1]);
        p.set_f("q1", 0.0);
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
        for ex in examples("elasticity").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [0.0, -5.0, 1.0e12, f64::NAN, f64::INFINITY, 1.0e-9] {
            for key in ["p1", "q1", "p2", "q2"] {
                for method in METHODS {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_f(key, v);
                        p.set_s("method", method);
                        p.set_b(SHOW_ANSWERS, answers);
                        assert!(sane(&render(&p)), "{key} {v}");
                    }
                }
            }
        }
    }
}
