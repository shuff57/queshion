//! Binomial and Poisson probability bars with a shaded region (exactly k, at most k, at least k,
//! or between two values) and its probability. The shaded bars are the question the student is
//! given; the probability is the answer, so the student copy (Show answers off) keeps the
//! shading and hides the readout.
use super::normal::prob_text;
use crate::figures::chart::{self, AxesOpts, Frame, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const REGIONS: [&str; 5] = ["None", "Exactly", "At most", "At least", "Between"];
const POISSON_CAP: usize = 300;

pub fn def() -> FigureDef {
    FigureDef {
        id: "discrete-distribution",
        subject: Subject::Math,
        title: "Binomial and Poisson bars",
        blurb: "Probability bars for a binomial or Poisson distribution with a shaded region (exactly, at most, at least or between) and its probability.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("discrete-distribution", "n" | "p") => p.s("dist") == "Binomial",
        ("discrete-distribution", "lambda") => p.s("dist") == "Poisson",
        ("discrete-distribution", "a") => p.s("region") != "None",
        ("discrete-distribution", "b") => p.s("region") == "Between",
        ("discrete-distribution", "show_prob") => p.s("region") != "None",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "discrete-distribution" => vec![
            Example { name: "Ten coin flips, at most 3 heads", params: vec![] },
            Example {
                name: "Free throws, at least 8 of 10",
                params: vec![("p", Value::F(0.7)), ("region", s("At least")), ("a", Value::I(8)), ("x_title", s("Shots made"))],
            },
            Example {
                name: "Poisson arrivals",
                params: vec![("dist", s("Poisson")), ("lambda", Value::F(4.0)), ("region", s("Between")), ("a", Value::I(2)), ("b", Value::I(5)), ("x_title", s("Customers per minute"))],
            },
            Example { name: "Exactly 5, values on the bars", params: vec![("region", s("Exactly")), ("a", Value::I(5)), ("values", Value::B(true))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("dist", "Distribution", &["Binomial", "Poisson"], "Binomial"),
        ParamDef::int("n", "Trials (n)", 1, 60, 10),
        ParamDef::float("p", "Probability of success (p)", 0.0, 1.0, 0.05, 0.5),
        ParamDef::float("lambda", "Mean (\u{3bb})", 0.1, 30.0, 0.5, 4.0),
        ParamDef::choice("region", "Shade", &REGIONS, "At most").hint("Exactly k, at most k (k or fewer), at least k (k or more), or between two values inclusive"),
        ParamDef::int("a", "k (or lower value)", 0, 300, 3),
        ParamDef::int("b", "Upper value", 0, 300, 6),
        ParamDef::bool("show_prob", "Write the probability", true),
        ParamDef::bool("values", "Write the probability above each bar", false),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the probability and keeps the shaded bars, for students to work out"),
    ]
}

/// ln(k!) for k up to 400.
fn ln_fact(k: usize) -> f64 {
    (2..=k).map(|i| (i as f64).ln()).sum()
}

pub fn binomial_pmf(n: usize, p: f64, k: usize) -> f64 {
    if k > n {
        return 0.0;
    }
    if p <= 0.0 {
        return if k == 0 { 1.0 } else { 0.0 };
    }
    if p >= 1.0 {
        return if k == n { 1.0 } else { 0.0 };
    }
    (ln_fact(n) - ln_fact(k) - ln_fact(n - k) + k as f64 * p.ln() + (n - k) as f64 * (1.0 - p).ln()).exp()
}

pub fn poisson_pmf(lambda: f64, k: usize) -> f64 {
    (-lambda + k as f64 * lambda.ln() - ln_fact(k)).exp()
}

/// The values the region covers, ordered and clipped to 0..=top.
fn covered(region: &str, a: i64, b: i64, top: usize) -> Vec<usize> {
    let (a, b) = if region == "Between" && a > b { (b, a) } else { (a, b) };
    let (a, b) = (a.max(0) as usize, b.max(0) as usize);
    match region {
        "Exactly" => if a <= top { vec![a] } else { vec![] },
        "At most" => (0..=a.min(top)).collect(),
        "At least" => (a..=top).collect(),
        "Between" => (a..=b.min(top)).collect(),
        _ => vec![],
    }
}

fn render(p: &Params) -> String {
    let poisson = p.s("dist") == "Poisson";
    let (n, pr, lambda) = (p.i("n").clamp(1, 60) as usize, p.f("p"), p.f("lambda"));
    if !poisson && !(pr.is_finite() && (0.0..=1.0).contains(&pr)) {
        return chart::message("The probability must be between 0 and 1");
    }
    if poisson && !(lambda.is_finite() && lambda > 0.0 && lambda <= 30.0) {
        return chart::message("The mean must be above 0 and at most 30");
    }
    let pmf = |k: usize| if poisson { poisson_pmf(lambda, k) } else { binomial_pmf(n, pr, k) };
    // bars drawn; a Poisson tail past the last bar still counts in the probability
    let kmax = if poisson { ((lambda + 4.0 * lambda.sqrt() + 4.0).ceil() as usize).clamp(6, 60) } else { n };
    let sum_to = if poisson { POISSON_CAP } else { n };
    let region = p.s("region");
    let (ra, rb) = (p.i("a"), p.i("b"));
    let inside = covered(region, ra, rb, sum_to);
    let prob: f64 = inside.iter().map(|k| pmf(*k)).sum();
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (title, x_title) = (p.s("title").trim(), p.s("x_title").trim());
    let readout = region != "None" && p.b("show_prob");
    let (w, h) = (640.0, 360.0 + (k - 1.0) * 50.0);
    let top = if title.is_empty() { 22.0 } else { 32.0 + fs * 1.4 } + if readout { fs * 2.2 } else { 0.0 };
    let bottom = 26.0 + fs * 2.0 + if x_title.is_empty() { 0.0 } else { fs * 1.7 };
    let ymax = (0..=kmax).map(pmf).fold(0.0, f64::max).max(1e-6) * 1.15;
    let ystep = chart::nice_step(ymax, 5.0);
    let ytop = (ymax / ystep).ceil() * ystep;
    let yticks = chart::ticks(0.0, ytop, ystep);
    let label_chars = yticks.iter().map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let f = Frame { w, h, l: 44.0 + fs * 0.62 * label_chars, r: 24.0, t: top, b: bottom, x0: -0.5, x1: kmax as f64 + 0.5, y0: 0.0, y1: ytop };
    let every = (kmax / 20 + 1).max(1);
    let xticks: Vec<chart::Tick> = (0..=kmax).filter(|v| v % every == 0).map(|v| (v as f64, v.to_string())).collect();
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    chart::axes(&mut d, &f, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: false, x_title, y_title: "Probability", fs });
    let bw = (f.px(1.0) - f.px(0.0)) * 0.84;
    for kk in 0..=kmax {
        let v = pmf(kk);
        let (x, y) = (f.px(kk as f64) - bw / 2.0, f.py(v));
        let on = inside.contains(&kk);
        let fill = if on { fills.series(0) } else if fills.is_print() { "#fff".to_string() } else { "#dde6f5".to_string() };
        d.rect(x, y, bw, (f.bottom() - y).max(0.0), &fill, INK, 1.4);
        if p.b("values") && kmax <= 24 && v >= 0.0005 {
            chart::text(&mut d, f.px(kk as f64), y - 5.0, fs * 0.72, "middle", &format!("{v:.3}"));
        }
    }
    d.line(f.left(), f.bottom(), f.right(), f.bottom(), INK, 2.0);
    if readout && p.b(SHOW_ANSWERS) {
        let x = if poisson { "X" } else { "X" };
        let (lo, hi) = if ra <= rb { (ra, rb) } else { (rb, ra) };
        let event = match region {
            "Exactly" => format!("{x} = {ra}"),
            "At most" => format!("{x} \u{2264} {ra}"),
            "At least" => format!("{x} \u{2265} {ra}"),
            _ => format!("{lo} \u{2264} {x} \u{2264} {hi}"),
        };
        chart::text(&mut d, w / 2.0, top - fs * 0.9, fs * 1.15, "middle", &format!("P({event}) = {}", prob_text(prob)));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn pmfs_match_known_values() {
        assert!((binomial_pmf(10, 0.5, 5) - 0.24609375).abs() < 1e-12);
        assert!((binomial_pmf(10, 0.5, 0) - 0.0009765625).abs() < 1e-12);
        assert!((0..=10).map(|k| binomial_pmf(10, 0.3, k)).sum::<f64>() - 1.0 < 1e-12);
        assert_eq!(binomial_pmf(5, 0.0, 0), 1.0);
        assert_eq!(binomial_pmf(5, 1.0, 5), 1.0);
        assert_eq!(binomial_pmf(5, 0.5, 9), 0.0);
        assert!((poisson_pmf(4.0, 3) - 0.19536681481316454).abs() < 1e-12);
        assert!((0..POISSON_CAP).map(|k| poisson_pmf(12.5, k)).sum::<f64>() - 1.0 < 1e-12);
    }

    #[test]
    fn regions_pick_the_right_values() {
        assert_eq!(covered("At most", 3, 0, 10), vec![0, 1, 2, 3]);
        assert_eq!(covered("At least", 8, 0, 10), vec![8, 9, 10]);
        assert_eq!(covered("Exactly", 4, 0, 10), vec![4]);
        assert_eq!(covered("Between", 6, 2, 10), vec![2, 3, 4, 5, 6], "bounds the wrong way round");
        assert_eq!(covered("Between", 2, 99, 10), vec![2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert!(covered("Exactly", 20, 0, 10).is_empty() && covered("None", 1, 2, 10).is_empty());
        assert_eq!(covered("At most", -4, 0, 10), vec![0]);
    }

    #[test]
    fn default_shows_the_probability() {
        let svg = render(&defaults());
        assert!(svg.contains("P(X \u{2264} 3) = 0.1719"), "{}", &svg[..svg.len().min(0)]);
        assert_eq!(svg.matches("<rect").count() >= 11, true);
        let mut p = defaults();
        p.set_s("region", "At least");
        p.set_i("a", 8);
        assert!(render(&p).contains("P(X \u{2265} 8) = 0.0547"));
    }

    #[test]
    fn poisson_counts_the_tail_beyond_the_bars() {
        let mut p = defaults();
        p.set_s("dist", "Poisson");
        p.set_f("lambda", 4.0);
        p.set_s("region", "At least");
        p.set_i("a", 10);
        // 1 - P(X <= 9) = 0.0081
        assert!(render(&p).contains("= 0.0081"));
    }

    #[test]
    fn student_copy_keeps_the_shading_and_drops_the_probability() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(key.contains("P(X") && !student.contains("P(X"));
        assert_eq!(size_of(&student), size_of(&key));
        assert_eq!(student.matches("<rect").count(), key.matches("<rect").count());
    }

    #[test]
    fn bad_values_are_explained() {
        let mut p = defaults();
        p.set_f("p", 1.5);
        assert!(render(&p).contains("between 0 and 1"));
        let mut q = defaults();
        q.set_s("dist", "Poisson");
        q.set_f("lambda", f64::NAN);
        assert!(render(&q).contains("mean must be above 0"));
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
        for ex in examples("discrete-distribution").unwrap() {
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
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        for dist in ["Binomial", "Poisson"] {
            for region in REGIONS {
                for (a, b) in [(0, 0), (-5, 400), (300, 0), (3, 3)] {
                    for (pp, lam, nn) in [(0.0, 0.1, 1), (1.0, 30.0, 60), (f64::NAN, f64::INFINITY, -3), (0.5, 0.0, 0)] {
                        let mut p = defaults();
                        p.set_s("dist", dist);
                        p.set_s("region", region);
                        p.set_i("a", a);
                        p.set_i("b", b);
                        p.set_f("p", pp);
                        p.set_f("lambda", lam);
                        p.set_i("n", nn);
                        p.set_b("values", true);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{dist} {region} {a} {b} {pp} {lam} {nn}");
                    }
                }
            }
        }
    }
}
