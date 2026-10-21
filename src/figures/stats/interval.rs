//! Confidence interval on a number line: for a mean (σ known, or σ unknown with the t
//! distribution) or a proportion, drawn as an interval around the point estimate with the
//! working underneath. The sample facts are what the student is given; the interval, margin and
//! working are the answers, so the student copy (Show answers off) keeps the number line and the
//! estimate and hides the rest.
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const KINDS: [&str; 3] = ["Mean, \u{3c3} known", "Mean, \u{3c3} unknown (t)", "Proportion"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "confidence-interval",
        subject: Subject::Math,
        title: "Confidence interval",
        blurb: "A confidence interval for a mean or a proportion on a number line, with the margin of error and the working, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("confidence-interval", "sd") => p.s("kind") != KINDS[2],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "confidence-interval" => vec![
            Example { name: "Mean with the t distribution", params: vec![] },
            Example {
                name: "Poll proportion, 95%",
                params: vec![("kind", s(KINDS[2])), ("estimate", Value::F(0.46)), ("n", Value::I(850)), ("x_title", s("Proportion in favour"))],
            },
            Example {
                name: "Known \u{3c3}, 99%, claimed value",
                params: vec![("kind", s(KINDS[0])), ("sd", Value::F(12.0)), ("n", Value::I(40)), ("level", s("99%")), ("mark", s("50"))],
            },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Interval for", &KINDS, KINDS[1]),
        ParamDef::float("estimate", "Sample mean or proportion", -1.0e6, 1.0e6, 0.5, 52.0),
        ParamDef::float("sd", "Standard deviation (\u{3c3} or s)", 0.0001, 1.0e6, 0.5, 8.0),
        ParamDef::int("n", "Sample size (n)", 2, 1_000_000, 30),
        ParamDef::choice("level", "Confidence level", &["90%", "95%", "99%"], "95%"),
        ParamDef::text("mark", "Mark a value (optional)", "").hint("For example a claimed mean; the working then says whether it is inside the interval"),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Number line title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the number line and the estimate and hides the interval, the margin of error and the working"),
    ]
}

fn lgamma(x: f64) -> f64 {
    const C: [f64; 9] = [0.999_999_999_999_809_9, 676.520_368_121_885_1, -1_259.139_216_722_402_8, 771.323_428_777_653_1, -176.615_029_162_140_6, 12.507_343_278_686_905, -0.138_571_095_265_720_12, 9.984_369_578_019_572e-6, 1.505_632_735_149_311_6e-7];
    let x = x - 1.0;
    let t = x + 7.5;
    let a = C[0] + (1..9).map(|i| C[i] / (x + i as f64)).sum::<f64>();
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The density's constant factor for `nu` degrees of freedom, worked out once per quantile.
fn t_norm(nu: f64) -> f64 {
    (lgamma((nu + 1.0) / 2.0) - lgamma(nu / 2.0)).exp() / (nu * std::f64::consts::PI).sqrt()
}

fn t_pdf(t: f64, nu: f64, norm: f64) -> f64 {
    norm * (1.0 + t * t / nu).powf(-(nu + 1.0) / 2.0)
}

/// P(T <= t) for t >= 0, by Simpson's rule from 0.
fn t_cdf_pos(t: f64, nu: f64, norm: f64) -> f64 {
    let steps = 400usize;
    let h = t / steps as f64;
    let mut s = t_pdf(0.0, nu, norm) + t_pdf(t, nu, norm);
    for i in 1..steps {
        s += t_pdf(h * i as f64, nu, norm) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    0.5 + s * h / 3.0
}

/// The t value with `upper` probability below it (0.5 < upper < 1), found by bisection.
pub fn t_crit(upper: f64, nu: f64) -> f64 {
    let norm = t_norm(nu);
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    while t_cdf_pos(hi, nu, norm) < upper && hi < 1.0e3 {
        hi *= 2.0;
    }
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if t_cdf_pos(mid, nu, norm) < upper { lo = mid } else { hi = mid }
    }
    (lo + hi) / 2.0
}

pub fn z_crit(level: &str) -> f64 {
    match level {
        "90%" => 1.644_854,
        "99%" => 2.575_829,
        _ => 1.959_964,
    }
}

struct Ci {
    crit: f64,
    se: f64,
    margin: f64,
    lo: f64,
    hi: f64,
}

fn interval(kind: &str, est: f64, sd: f64, n: i64, level: &str) -> Result<Ci, String> {
    if !est.is_finite() {
        return Err("Enter the estimate as a number".into());
    }
    let n = n.max(2) as f64;
    let (crit, se) = if kind == KINDS[2] {
        if !(0.0..=1.0).contains(&est) {
            return Err("A proportion must be between 0 and 1".into());
        }
        (z_crit(level), (est * (1.0 - est) / n).sqrt())
    } else {
        if !(sd.is_finite() && sd > 0.0) {
            return Err("The standard deviation must be above 0".into());
        }
        let crit = if kind == KINDS[0] {
            z_crit(level)
        } else {
            let upper = 1.0 - (1.0 - level.trim_end_matches('%').parse::<f64>().unwrap_or(95.0) / 100.0) / 2.0;
            t_crit(upper, n - 1.0)
        };
        (crit, sd / n.sqrt())
    };
    let margin = crit * se;
    if !(margin.is_finite() && margin > 0.0) {
        return Err("The margin of error is 0, so there is no interval to draw".into());
    }
    Ok(Ci { crit, se, margin, lo: est - margin, hi: est + margin })
}

fn r3(v: f64) -> String {
    chart::fmt((v * 1000.0).round() / 1000.0)
}

fn render(p: &Params) -> String {
    let (kind, level) = (p.s("kind"), p.s("level"));
    let est = p.f("estimate");
    let n = p.i("n");
    let ci = match interval(kind, est, p.f("sd"), n, level) {
        Ok(c) => c,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (title, x_title) = (p.s("title").trim(), p.s("x_title").trim());
    let mark: Option<f64> = p.s("mark").trim().parse::<f64>().ok().filter(|v| v.is_finite());

    // the line spans the interval with room either side, and the marked value if there is one
    let (mut a0, mut a1) = (est - ci.margin * 2.0, est + ci.margin * 2.0);
    if let Some(m) = mark {
        a0 = a0.min(m - ci.margin * 0.4);
        a1 = a1.max(m + ci.margin * 0.4);
    }
    let step = chart::nice_step(a1 - a0, 6.0);
    let (w, base_h) = (640.0, 300.0);
    let top = if title.is_empty() { 18.0 } else { 34.0 + fs * 1.3 };
    let h = base_h + (k - 1.0) * 40.0 + top + if x_title.is_empty() { 0.0 } else { fs * 1.6 };
    let f = chart::Frame { w, h, l: 36.0, r: 36.0, t: top, b: 0.0, x0: a0, x1: a1, y0: 0.0, y1: 1.0 };
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    // what the student is given
    let given = if kind == KINDS[2] {
        format!("Sample proportion = {}, n = {n}, {level} confidence", r3(est))
    } else if kind == KINDS[0] {
        format!("Sample mean = {}, \u{3c3} = {}, n = {n}, {level} confidence", r3(est), r3(p.f("sd")))
    } else {
        format!("Sample mean = {}, s = {}, n = {n}, {level} confidence", r3(est), r3(p.f("sd")))
    };
    chart::text(&mut d, w / 2.0, top + fs * 0.6, fs * 1.05, "middle", &given);

    let y = top + 120.0 + (k - 1.0) * 10.0;
    d.line(f.left() - 10.0, y, f.right() + 10.0, y, INK, 2.2);
    for (v, label) in chart::ticks(a0, a1, step) {
        let x = f.px(v);
        d.line(x, y - 6.0, x, y + 6.0, INK, 1.6);
        chart::text(&mut d, x, y + 10.0 + fs, fs, "middle", &label);
    }
    if !x_title.is_empty() {
        chart::text(&mut d, w / 2.0, y + 14.0 + fs * 2.6, fs * 1.1, "middle", x_title);
    }
    let (xl, xh, xe) = (f.px(ci.lo), f.px(ci.hi), f.px(est));
    if answers {
        // the interval: a filled bar with end caps, the estimate as a dot in the middle
        d.rect(xl, y - 13.0, xh - xl, 26.0, &fills.series(0), INK, 1.6);
        d.line(xl, y - 22.0, xl, y + 22.0, INK, 2.6);
        d.line(xh, y - 22.0, xh, y + 22.0, INK, 2.6);
        chart::text(&mut d, xl, y - 30.0, fs, "middle", &r3(ci.lo));
        chart::text(&mut d, xh, y - 30.0, fs, "middle", &r3(ci.hi));
    }
    d.circle(xe, y, 6.0, INK, INK, 1.0);
    chart::text(&mut d, xe, y - 30.0 - if answers { fs * 1.25 } else { 0.0 }, fs, "middle", &format!("estimate {}", r3(est)));
    if let Some(m) = mark {
        let xm = f.px(m);
        d.raw(&format!(r#"<line x1="{xm:.2}" y1="{:.2}" x2="{xm:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.8" stroke-dasharray="6 4"/>"#, y - 56.0, y + 6.0));
        chart::text(&mut d, xm, y - 62.0, fs * 0.9, "middle", &format!("claim {}", r3(m)));
    }
    // the working, one line per step
    let wy = y + 14.0 + fs * 2.6 + if x_title.is_empty() { 0.0 } else { fs * 1.6 };
    let (crit_name, se_text) = if kind == KINDS[2] {
        ("z*", format!("\u{221a}({} \u{d7} {} \u{f7} {n})", r3(est), r3(1.0 - est)))
    } else {
        (if kind == KINDS[0] { "z*" } else { "t*" }, format!("{} \u{f7} \u{221a}{n}", r3(p.f("sd"))))
    };
    let lines = [
        format!("{crit_name} = {}  ({level}{})", r3(ci.crit), if kind == KINDS[1] { format!(", {} degrees of freedom", n.max(2) - 1) } else { String::new() }),
        format!("margin of error = {crit_name} \u{d7} {se_text} = {}", r3(ci.margin)),
        format!("interval = {} \u{b1} {} = ({}, {})", r3(est), r3(ci.margin), r3(ci.lo), r3(ci.hi)),
    ];
    let _ = ci.se;
    for (i, l) in lines.iter().enumerate() {
        let yy = wy + fs * 1.7 * i as f64;
        if answers {
            chart::text(&mut d, 36.0, yy, fs, "start", l);
        } else {
            let label = l.split(" = ").next().unwrap_or("");
            chart::text(&mut d, 36.0, yy, fs, "start", &format!("{label} ="));
            d.line(36.0 + label.chars().count() as f64 * fs * 0.58 + fs * 1.6, yy + 2.0, w - 36.0, yy + 2.0, INK, 1.0);
        }
    }
    if let Some(m) = mark {
        let yy = wy + fs * 1.7 * 3.0;
        if answers {
            let inside = m >= ci.lo && m <= ci.hi;
            chart::text(&mut d, 36.0, yy, fs, "start", &format!("{} is {} the interval, so it is {} a plausible value", r3(m), if inside { "inside" } else { "outside" }, if inside { "still" } else { "not" }));
        } else {
            chart::text(&mut d, 36.0, yy, fs, "start", &format!("Is {} a plausible value?", r3(m)));
        }
    }
    let _ = h;
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn t_values_match_the_table() {
        for (upper, nu, want) in [(0.975, 9.0, 2.262157), (0.975, 29.0, 2.045230), (0.995, 14.0, 2.976843), (0.95, 4.0, 2.131847), (0.975, 1.0, 12.7062)] {
            let got = t_crit(upper, nu);
            assert!((got - want).abs() < 2e-4 * want, "t({upper}, {nu}) = {got} vs {want}");
        }
        assert!((lgamma(5.0) - 24f64.ln()).abs() < 1e-10 && (lgamma(0.5) - std::f64::consts::PI.sqrt().ln()).abs() < 1e-10);
        assert!((t_crit(0.975, 1.0e5) - 1.96).abs() < 0.01, "approaches z");
    }

    #[test]
    fn intervals_are_worked_out() {
        let ci = interval(KINDS[0], 52.0, 8.0, 30, "95%").unwrap();
        assert!((ci.margin - 1.959964 * 8.0 / 30f64.sqrt()).abs() < 1e-9);
        let ci = interval(KINDS[1], 52.0, 8.0, 30, "95%").unwrap();
        assert!((ci.margin - 2.04523 * 8.0 / 30f64.sqrt()).abs() < 1e-3);
        let ci = interval(KINDS[2], 0.46, 0.0, 850, "95%").unwrap();
        assert!((ci.margin - 1.959964 * (0.46f64 * 0.54 / 850.0).sqrt()).abs() < 1e-9);
        assert!(interval(KINDS[2], 1.5, 0.0, 10, "95%").is_err());
        assert!(interval(KINDS[0], 5.0, 0.0, 10, "95%").is_err());
        assert!(interval(KINDS[1], f64::NAN, 1.0, 10, "95%").is_err());
    }

    #[test]
    fn default_shows_the_interval_and_working() {
        let svg = render(&defaults());
        assert!(svg.contains("t* = 2.045") && svg.contains("(49.013, 54.987)"), "working shown");
        assert!(svg.contains(">estimate 52<") && svg.contains("29 degrees of freedom"));
    }

    #[test]
    fn marked_value_is_judged() {
        let mut p = defaults();
        p.set_s("mark", "48");
        assert!(render(&p).contains("48 is outside the interval"));
        p.set_s("mark", "53");
        assert!(render(&p).contains("53 is inside the interval"));
        p.set_s("mark", "words");
        assert!(!render(&p).contains("plausible"));
    }

    #[test]
    fn student_copy_hides_the_interval() {
        let def = def();
        let mut p = defaults();
        p.set_s("mark", "50");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(key.contains("margin of error = t*") && !student.contains("2.045") && !student.contains("49.0"));
        assert!(student.contains(">estimate 52<") && student.contains("Sample mean = 52"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("confidence-interval").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    Value::I(i) => p.set_i(k, *i),
                    Value::F(x) => p.set_f(k, *x),
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<circle") && !svg.contains("NaN"), "{}", ex.name);
        }
        for kind in KINDS {
            for est in [0.0, 0.5, 1.0, -3.0, 1.0e6, f64::NAN] {
                for sd in [0.0, 1.0e-4, 1.0e6, f64::INFINITY] {
                    for n in [-5, 0, 2, 1_000_000] {
                        for answers in [true, false] {
                            let mut p = defaults();
                            p.set_s("kind", kind);
                            p.set_f("estimate", est);
                            p.set_f("sd", sd);
                            p.set_i("n", n);
                            p.set_s("mark", "1e308");
                            p.set_b(SHOW_ANSWERS, answers);
                            let svg = render(&p);
                            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{kind} {est} {sd} {n}");
                        }
                    }
                }
            }
        }
    }
}
