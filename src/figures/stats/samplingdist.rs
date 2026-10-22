//! The sampling distribution of the sample mean: the population curve and the narrower curve of
//! sample means (mean μ, standard error σ/√n), with an optional shaded region for a probability.
//! The population and the sample size are the question; the standard error and the probability
//! are the answers, so the student copy (Show answers off) keeps both curves and the shading and
//! leaves those two readouts blank.
use super::normal::{cdf, intervals, pdf, prob_text, probability};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "sampling-distribution",
        subject: Subject::Math,
        title: "Sampling Distribution",
        blurb: "The population curve beside the narrower curve of sample means for a sample size n, with the standard error and a shaded probability, or blank readouts for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("sampling-distribution", "a") => matches!(p.s("region"), "Above" | "Between" | "Outside"),
        ("sampling-distribution", "b") => matches!(p.s("region"), "Below" | "Between" | "Outside"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "sampling-distribution" => vec![
            Example { name: "Samples of 25", params: vec![] },
            Example { name: "A bigger sample narrows the curve", params: vec![("n", Value::I(100)), ("a", Value::F(102.0))] },
            Example { name: "Sample means between two values", params: vec![("region", s("Between")), ("a", Value::F(96.0)), ("b", Value::F(104.0))] },
            Example { name: "Just the sampling distribution", params: vec![("show_pop", Value::B(false)), ("n", Value::I(9)), ("region", s("None"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("mean", "Population mean (μ)", -10000.0, 10000.0, 1.0, 100.0),
        ParamDef::float("sd", "Population standard deviation (σ)", 0.001, 10000.0, 1.0, 15.0),
        ParamDef::int("n", "Sample size (n)", 1, 1000, 25),
        ParamDef::bool("show_pop", "Show the population curve", true),
        ParamDef::choice("region", "Shade sample means", &["None", "Below", "Above", "Between", "Outside"], "Above"),
        ParamDef::float("a", "Lower bound", -10000.0, 10000.0, 0.5, 104.0),
        ParamDef::float("b", "Upper bound", -10000.0, 10000.0, 0.5, 108.0),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps both curves and the shading and leaves the standard error and the probability blank for students"),
    ]
}

/// The standard error of the mean, σ/√n.
pub fn std_error(sd: f64, n: f64) -> f64 {
    sd / n.sqrt()
}

fn num(v: f64) -> String {
    chart::fmt((v * 1000.0).round() / 1000.0)
}

fn render(p: &Params) -> String {
    let (mean, sd) = (p.f("mean"), p.f("sd"));
    if !(mean.is_finite() && sd.is_finite() && sd > 0.0) {
        return chart::message("The standard deviation must be above 0");
    }
    if mean.abs() > 1.0e7 || sd > 1.0e7 {
        return chart::message("Use a smaller mean and standard deviation");
    }
    let n = p.i("n").clamp(1, 1000) as f64;
    let region = p.s("region");
    let (ra, rb) = (p.f("a"), p.f("b"));
    if !(ra.is_finite() && rb.is_finite()) {
        return chart::message("Enter the bounds as numbers");
    }
    let (ra, rb) = if ra <= rb { (ra, rb) } else { (rb, ra) };
    let se = std_error(sd, n);
    let show_pop = p.b("show_pop");
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (title, x_title) = (p.s("title").trim(), p.s("x_title").trim());
    let rows = if region == "None" { 2.0 } else { 3.0 };
    let top = 14.0 + if title.is_empty() { 0.0 } else { fs * 1.8 } + fs * 1.9 * rows + 14.0;
    let bottom = 26.0 + fs * 2.0 + if x_title.is_empty() { 0.0 } else { fs * 1.7 };
    let (w, h) = (700.0, top + 300.0 + bottom);
    let spread = if show_pop { sd } else { se } * 4.0;
    let (x0, x1) = (mean - spread, mean + spread);
    let peak = pdf(0.0) / se;
    let f = chart::Frame { w, h, l: 28.0, r: 28.0, t: top, b: bottom, x0, x1, y0: 0.0, y1: peak * 1.15 };

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    let mut y = 14.0;
    if !title.is_empty() {
        y += fs * 1.2;
        chart::text(&mut d, w / 2.0, y, fs * 1.3, "middle", title);
        y += fs * 0.6;
    }
    y += fs * 1.2;
    chart::text(&mut d, w / 2.0, y, fs, "middle", &format!("Population: mean {}, standard deviation {}. Samples of size n = {}.", num(mean), num(sd), n as i64));
    let readout = |d: &mut Doc, y: f64, label: &str, value: &str| {
        let x = 60.0;
        chart::text(d, x, y, fs, "start", label);
        let vx = x + 20.0 + 24.0 * fs * 0.56 * 1.0 + 13.0 * fs * 0.56;
        if answers {
            chart::text(d, vx, y, fs, "start", value);
        } else {
            d.line(vx, y + 3.0, vx + 150.0, y + 3.0, INK, 1.0);
        }
    };
    y += fs * 1.9;
    readout(&mut d, y, "Standard error of the mean, σ/sqrt(n) =", &format!("{}/sqrt({}) = {}", num(sd), n as i64, num(se)));
    if region != "None" {
        y += fs * 1.9;
        let (za, zb) = ((ra - mean) / se, (rb - mean) / se);
        let event = match region {
            "Below" => format!("x\u{304} < {}", num(rb)),
            "Above" => format!("x\u{304} > {}", num(ra)),
            "Between" => format!("{} < x\u{304} < {}", num(ra), num(rb)),
            _ => format!("x\u{304} < {} or x\u{304} > {}", num(ra), num(rb)),
        };
        readout(&mut d, y, &format!("P({event}) ="), &prob_text(probability(region, za, zb)));
    }

    // shading, then both curves
    let (zlo, zhi) = ((x0 - mean) / se, (x1 - mean) / se);
    let (za, zb) = ((ra - mean) / se, (rb - mean) / se);
    for (lo, hi) in intervals(region, za, zb, zlo, zhi) {
        let steps = 120;
        let mut pts = vec![(f.px(mean + lo * se), f.bottom())];
        for i in 0..=steps {
            let z = lo + (hi - lo) * i as f64 / steps as f64;
            pts.push((f.px(mean + z * se), f.py(pdf(z) / se)));
        }
        pts.push((f.px(mean + hi * se), f.bottom()));
        d.poly(&pts, &fills.series(0), "none", 0.0);
    }
    if show_pop {
        let pop: Vec<(f64, f64)> = (0..=200).map(|i| { let x = x0 + (x1 - x0) * i as f64 / 200.0; (f.px(x), f.py(pdf((x - mean) / sd) / sd)) }).collect();
        d.raw(&format!(r#"<polyline points="{}" fill="none" stroke="{INK}" stroke-width="1.8" stroke-dasharray="6 4"/>"#, pop.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ")));
    }
    let samp: Vec<(f64, f64)> = (0..=240).map(|i| { let x = x0 + (x1 - x0) * i as f64 / 240.0; (f.px(x), f.py(pdf((x - mean) / se) / se)) }).collect();
    d.polyline(&samp, INK, 2.6);
    d.line(f.left() - 8.0, f.bottom(), f.right() + 8.0, f.bottom(), INK, 2.2);
    // mean marker and axis ticks
    d.raw(&format!(r#"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.2" stroke-dasharray="3 4"/>"#, f.bottom(), f.py(peak), x = f.px(mean)));
    let step = chart::nice_step(x1 - x0, 7.0);
    for (v, label) in chart::ticks(x0, x1, step) {
        let x = f.px(v);
        d.line(x, f.bottom(), x, f.bottom() + 7.0, INK, 1.8);
        chart::text(&mut d, x, f.bottom() + 10.0 + fs, fs, "middle", &label);
    }
    if !x_title.is_empty() {
        chart::text(&mut d, w / 2.0, h - 10.0, fs * 1.1, "middle", x_title);
    }
    // labels for the two curves
    let lx = f.right() - 230.0;
    chart::text(&mut d, lx, f.top() + fs, fs * 0.95, "start", &format!("Sample means (n = {})", n as i64));
    d.line(lx - 34.0, f.top() + fs * 0.7, lx - 8.0, f.top() + fs * 0.7, INK, 2.6);
    if show_pop {
        chart::text(&mut d, lx, f.top() + fs * 2.4, fs * 0.95, "start", "Population");
        d.raw(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.8" stroke-dasharray="6 4"/>"#, lx - 34.0, f.top() + fs * 2.1, lx - 8.0, f.top() + fs * 2.1));
    }
    let _ = cdf;
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn the_standard_error_shrinks_with_n() {
        assert_eq!(std_error(15.0, 25.0), 3.0);
        assert_eq!(std_error(10.0, 100.0), 1.0);
        assert!(std_error(15.0, 100.0) < std_error(15.0, 25.0));
    }

    #[test]
    fn key_works_the_standard_error_and_the_probability() {
        let svg = render(&defaults());
        // sd 15, n 25: se 3; P(mean > 104) = P(z > 1.3333) = 0.0912
        assert!(svg.contains("15/sqrt(25) = 3") && svg.contains("0.0912"), "{svg}");
        assert!(svg.contains("Samples of size n = 25"));
    }

    #[test]
    fn student_copy_leaves_the_readouts_blank() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains("Standard error of the mean") && !student.contains("0.0912") && !student.contains("= 3<"));
        assert!(student.contains("Samples of size n = 25"));
        assert_eq!(size_of(&student), size_of(&key));
        assert_eq!(student.matches("<polygon").count(), key.matches("<polygon").count(), "the shading stays");
    }

    #[test]
    fn no_region_has_no_probability_row() {
        let mut p = defaults();
        p.set_s("region", "None");
        let svg = render(&p);
        assert!(!svg.contains("P(") && sane(&svg));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("sampling-distribution").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for k in ["mean", "sd", "a", "b"] {
            for v in [f64::NAN, f64::INFINITY, -1e300, 1e300, 0.0] {
                let mut p = defaults();
                p.set_f(k, v);
                assert!(sane(&render(&p)), "{k} {v}");
            }
        }
        for n in [i64::MIN, -5, 0, 1, 1000, i64::MAX] {
            let mut p = defaults();
            p.set_i("n", n);
            assert!(sane(&render(&p)), "{n}");
        }
        let mut p = defaults();
        p.set_s("region", "<b>nan");
        p.set_s("title", "<b>&");
        assert!(sane(&render(&p)));
    }
}
