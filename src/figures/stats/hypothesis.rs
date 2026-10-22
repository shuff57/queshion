//! The rejection region of a hypothesis test: a normal, Student t or chi-square curve with the
//! significance level shaded in the tail(s) and the test statistic marked. The test set-up (the
//! distribution, α, the tail and the statistic) is the question; the critical value, the p-value
//! and the decision are the answers, so the student copy (Show answers off) keeps the curve and
//! the statistic and leaves the shading and those three readouts for students.
use super::dist::Dist;
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const DISTS: [&str; 3] = ["Normal (z)", "Student t", "Chi-square"];
const TAILS: [&str; 3] = ["Two-tailed", "Right-tailed", "Left-tailed"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "rejection-region",
        subject: Subject::Math,
        title: "Rejection Region",
        blurb: "A hypothesis test on a normal, t or chi-square curve: the significance level shaded in the tail, the critical value, the test statistic, the p-value and the decision, or blanks for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("rejection-region", "df") => p.s("dist") != DISTS[0],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "rejection-region" => vec![
            Example { name: "Two-tailed z test", params: vec![] },
            Example { name: "Right-tailed t test", params: vec![("dist", s(DISTS[1])), ("df", Value::I(15)), ("tail", s(TAILS[1])), ("stat", Value::F(1.9))] },
            Example { name: "Left-tailed z test, 1%", params: vec![("tail", s(TAILS[2])), ("alpha", Value::F(0.01)), ("stat", Value::F(-1.8))] },
            Example { name: "Chi-square test", params: vec![("dist", s(DISTS[2])), ("df", Value::I(4)), ("tail", s(TAILS[1])), ("stat", Value::F(11.2))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("dist", "Test statistic follows", &DISTS, DISTS[0]),
        ParamDef::int("df", "Degrees of freedom", 1, 200, 10),
        ParamDef::choice("tail", "Alternative hypothesis", &TAILS, TAILS[0]).hint("Two-tailed puts half of α in each tail"),
        ParamDef::float("alpha", "Significance level (α)", 0.001, 0.5, 0.005, 0.05),
        ParamDef::float("stat", "Test statistic", -10000.0, 10000.0, 0.1, 2.1),
        ParamDef::bool("show_stat", "Mark the test statistic", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the curve and the test statistic and leaves the shading, critical value, p-value and decision for students"),
    ]
}

fn dist_of(p: &Params) -> Dist {
    let df = p.i("df").clamp(1, 200) as f64;
    match p.s("dist") {
        "Student t" => Dist::T(df),
        "Chi-square" => Dist::Chi(df),
        _ => Dist::Z,
    }
}

/// The critical values in ascending order: one for a one-tailed test, two for a two-tailed one.
pub fn critical(d: Dist, tail: &str, alpha: f64) -> Vec<f64> {
    match tail {
        "Right-tailed" => vec![d.quantile(1.0 - alpha)],
        "Left-tailed" => vec![d.quantile(alpha)],
        _ => vec![d.quantile(alpha / 2.0), d.quantile(1.0 - alpha / 2.0)],
    }
}

/// The p-value of a statistic for the tail.
pub fn p_value(d: Dist, tail: &str, stat: f64) -> f64 {
    let lower = d.cdf(stat);
    let upper = match d {
        Dist::Chi(_) => 1.0 - lower,
        _ => d.cdf(-stat),
    };
    let p = match tail {
        "Right-tailed" => upper,
        "Left-tailed" => lower,
        _ => 2.0 * lower.min(upper),
    };
    p.clamp(0.0, 1.0)
}

fn num(v: f64) -> String {
    chart::fmt((v * 1000.0).round() / 1000.0)
}

fn render(p: &Params) -> String {
    let (alpha, stat) = (p.f("alpha"), p.f("stat"));
    if !alpha.is_finite() {
        return chart::message("Enter the significance level as a number");
    }
    let alpha = alpha.clamp(0.001, 0.5);
    let show_stat = p.b("show_stat");
    if show_stat && !stat.is_finite() {
        return chart::message("Enter the test statistic as a number");
    }
    let dist = dist_of(p);
    let tail = p.s("tail");
    let crit = critical(dist, tail, alpha);
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let fs = 14.0 * svg::label_factor(p.s("label_size"));
    let (title, x_title) = (p.s("title").trim(), p.s("x_title").trim());
    let chi = matches!(dist, Dist::Chi(_));
    // the plotted range
    let cmax = crit.iter().fold(0.0f64, |m, c| m.max(c.abs()));
    let (x0, x1) = if chi {
        let df = if let Dist::Chi(df) = dist { df } else { 1.0 };
        let mut hi = (df + 4.0 * (2.0 * df).sqrt()).max(cmax * 1.3);
        if show_stat && stat > 0.0 && stat * 1.15 <= hi * 2.0 {
            hi = hi.max(stat * 1.15);
        }
        (0.0, hi)
    } else {
        let mut r = 4.0f64.max(cmax * 1.25);
        if show_stat && stat.abs() * 1.15 <= 12.0 {
            r = r.max(stat.abs() * 1.15);
        }
        (-r.min(12.0), r.min(12.0))
    };
    let samples = 300;
    let xs = |i: usize| x0 + (x1 - x0) * i as f64 / samples as f64;
    let ymax = (1..=samples).map(|i| dist.pdf(xs(i))).filter(|y| y.is_finite()).fold(0.0f64, f64::max).max(1e-9);
    let y1 = ymax * 1.12;

    let mut rows: Vec<(String, String)> = Vec::new();
    let crit_text = match tail {
        "Two-tailed" if !chi && (crit[0] + crit[1]).abs() < 1e-6 => format!("±{}", num(crit[1])),
        "Two-tailed" => format!("{} and {}", num(crit[0]), num(crit[1])),
        _ => num(crit[0]),
    };
    rows.push(("Critical value:".into(), crit_text));
    if show_stat {
        let pv = p_value(dist, tail, stat);
        rows.push(("p-value:".into(), super::normal::prob_text(pv)));
        let reject = pv < alpha;
        rows.push(("Decision:".into(), if reject { "Reject H0: the statistic is in the rejection region, p below α".into() } else { "Fail to reject H0: the statistic is not in the rejection region".into() }));
    }
    let top = 14.0 + if title.is_empty() { 0.0 } else { fs * 1.8 } + fs * 1.9 * (1.0 + rows.len() as f64) + 14.0;
    let bottom = 26.0 + fs * 4.6 + if x_title.is_empty() { 0.0 } else { fs * 1.7 };
    let (w, h) = (700.0, top + 280.0 + bottom);
    let f = chart::Frame { w, h, l: 28.0, r: 28.0, t: top, b: bottom, x0, x1, y0: 0.0, y1 };
    let clip = |y: f64| y.min(y1);

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    let mut y = 14.0;
    if !title.is_empty() {
        y += fs * 1.2;
        chart::text(&mut d, w / 2.0, y, fs * 1.3, "middle", title);
        y += fs * 0.6;
    }
    y += fs * 1.2;
    let what = match dist {
        Dist::Z => "Standard normal".to_string(),
        Dist::T(df) => format!("Student t, {} degrees of freedom", df as i64),
        Dist::Chi(df) => format!("Chi-square, {} degrees of freedom", df as i64),
    };
    chart::text(&mut d, w / 2.0, y, fs, "middle", &format!("{what} · α = {} · {}", num(alpha), tail.to_lowercase()));
    for (label, value) in &rows {
        y += fs * 1.9;
        chart::text(&mut d, 60.0, y, fs, "start", label);
        let vx = 60.0 + 140.0 * (fs / 14.0);
        if answers {
            chart::text(&mut d, vx, y, fs, "start", value);
        } else {
            d.line(vx, y + 3.0, vx + 150.0, y + 3.0, INK, 1.0);
        }
    }

    // the rejection region (an answer), then the curve
    if answers {
        let bands: Vec<(f64, f64)> = match tail {
            "Right-tailed" => vec![(crit[0], x1)],
            "Left-tailed" => vec![(x0, crit[0])],
            _ => vec![(x0, crit[0]), (crit[1], x1)],
        };
        for (lo, hi) in bands {
            let (lo, hi) = (lo.max(x0), hi.min(x1));
            if lo >= hi {
                continue;
            }
            let steps = 80;
            let mut pts = vec![(f.px(lo), f.bottom())];
            for i in 0..=steps {
                let x = lo + (hi - lo) * i as f64 / steps as f64;
                pts.push((f.px(x), f.py(clip(dist.pdf(x)))));
            }
            pts.push((f.px(hi), f.bottom()));
            d.poly(&pts, &fills.series(0), "none", 0.0);
        }
    }
    let curve: Vec<(f64, f64)> = (0..=samples).map(|i| { let x = xs(i); let v = dist.pdf(x); (f.px(x), f.py(clip(if v.is_finite() { v } else { y1 }))) }).collect();
    d.polyline(&curve, INK, 2.6);
    d.line(f.left() - 8.0, f.bottom(), f.right() + 8.0, f.bottom(), INK, 2.2);
    if answers {
        let share = if tail == "Two-tailed" { alpha / 2.0 } else { alpha };
        let label = if tail == "Two-tailed" { "α/2" } else { "α" };
        for (i, c) in crit.iter().enumerate() {
            if *c < x0 || *c > x1 {
                continue;
            }
            let (px, py) = (f.px(*c), f.py(clip(dist.pdf(*c))));
            d.raw(&format!(r#"<line x1="{px:.2}" y1="{:.2}" x2="{px:.2}" y2="{py:.2}" stroke="{INK}" stroke-width="1.6" stroke-dasharray="5 4"/>"#, f.bottom()));
            // under the axis, below the tick labels, so the statistic's arrow never crosses them
            let name = if chi { "χ²*" } else if matches!(dist, Dist::T(_)) { "t*" } else { "z*" };
            chart::text(&mut d, px, f.bottom() + 10.0 + fs * 2.5, fs * 0.95, "middle", &format!("{name} = {}", num(*c)));
            chart::text(&mut d, px, f.bottom() + 10.0 + fs * 3.7, fs * 0.9, "middle", &format!("{label} = {}", num(share)));
            let _ = i;
        }
    }
    // the observed statistic (given)
    if show_stat && stat >= x0 && stat <= x1 {
        let sx = f.px(stat);
        d.line(sx, f.bottom(), sx, f.top() + fs * 1.6, INK, 2.2);
        d.poly(&[(sx - 6.0, f.top() + fs * 1.6 + 10.0), (sx + 6.0, f.top() + fs * 1.6 + 10.0), (sx, f.top() + fs * 1.6)], INK, INK, 1.0);
        chart::text(&mut d, sx, f.top() + fs * 1.0, fs * 0.95, "middle", &format!("test statistic = {}", num(stat)));
    }
    let step = chart::nice_step(x1 - x0, 8.0);
    for (v, label) in chart::ticks(x0, x1, step) {
        let x = f.px(v);
        d.line(x, f.bottom(), x, f.bottom() + 7.0, INK, 1.8);
        chart::text(&mut d, x, f.bottom() + 10.0 + fs, fs, "middle", &label);
    }
    if !x_title.is_empty() {
        chart::text(&mut d, w / 2.0, h - 10.0, fs * 1.1, "middle", x_title);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn critical_values_and_p_values_match_the_tables() {
        let c = critical(Dist::Z, "Two-tailed", 0.05);
        assert!((c[1] - 1.95996).abs() < 1e-3 && (c[0] + 1.95996).abs() < 1e-3);
        assert!((critical(Dist::Z, "Right-tailed", 0.05)[0] - 1.64485).abs() < 1e-3);
        assert!((critical(Dist::Z, "Left-tailed", 0.01)[0] + 2.32635).abs() < 1e-3);
        assert!((critical(Dist::T(10.0), "Two-tailed", 0.05)[1] - 2.2281).abs() < 5e-4);
        assert!((critical(Dist::Chi(4.0), "Right-tailed", 0.05)[0] - 9.4877).abs() < 5e-4);
        let two = critical(Dist::Chi(4.0), "Two-tailed", 0.05);
        assert!((two[0] - 0.4844).abs() < 5e-4 && (two[1] - 11.1433).abs() < 5e-4);
        assert!((p_value(Dist::Z, "Two-tailed", 1.96) - 0.04999).abs() < 1e-3);
        assert!((p_value(Dist::Z, "Right-tailed", 1.645) - 0.05).abs() < 1e-3);
        assert!((p_value(Dist::Z, "Left-tailed", -2.326) - 0.01).abs() < 1e-3);
        assert!((p_value(Dist::Chi(4.0), "Right-tailed", 9.4877) - 0.05).abs() < 1e-4);
        assert!((p_value(Dist::T(10.0), "Two-tailed", 2.2281) - 0.05).abs() < 1e-3);
    }

    #[test]
    fn key_gives_the_decision() {
        let svg = render(&defaults());
        // z = 2.1 two-tailed: p = 0.0357 < 0.05
        assert!(svg.contains("±1.96") && svg.contains("0.0357") && svg.contains("Reject H0"), "{svg}");
        let mut p = defaults();
        p.set_f("stat", 1.0);
        assert!(render(&p).contains("Fail to reject H0"));
    }

    #[test]
    fn student_copy_keeps_the_curve_and_the_statistic_only() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains("test statistic = 2.1") && !student.contains("±1.96") && !student.contains("Reject") && student.contains(">Decision:<"));
        assert_eq!(student.matches("<polygon").count() + 2, key.matches("<polygon").count(), "the key shades two tails");
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn controls_follow_the_distribution() {
        let mut p = defaults();
        assert!(!relevant("rejection-region", "df", &p));
        p.set_s("dist", "Student t");
        assert!(relevant("rejection-region", "df", &p));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("rejection-region").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for dist in DISTS {
            for tail in TAILS {
                for k in ["alpha", "stat"] {
                    for v in [f64::NAN, f64::INFINITY, -1e300, 1e300, 0.0, 1e-9] {
                        for df in [1, 200, i64::MAX] {
                            let mut p = defaults();
                            p.set_s("dist", dist);
                            p.set_s("tail", tail);
                            p.set_i("df", df);
                            p.set_f(k, v);
                            assert!(sane(&render(&p)), "{dist} {tail} {k} {v} {df}");
                        }
                    }
                }
            }
        }
        let mut p = defaults();
        p.set_s("title", "<b>&");
        assert!(sane(&render(&p)) && !render(&p).contains("<b>"));
    }
}
