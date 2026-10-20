//! Normal curve with a shaded region (below, above, between or outside two bounds, given as values
//! or z-scores) and the area under it. The shaded region is the question the student is given;
//! the area is the answer, so the student copy (Show answers off) keeps the shading and hides
//! the probability readout.
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "normal-curve",
        subject: Subject::Math,
        title: "Normal Curve",
        blurb: "A bell curve from a mean and standard deviation, with a shaded region and its area as a probability.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("normal-curve", "a") => matches!(p.s("region"), "Above" | "Between" | "Outside"),
        ("normal-curve", "b") => matches!(p.s("region"), "Below" | "Between" | "Outside"),
        ("normal-curve", "by" | "show_prob") => p.s("region") != "None",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "normal-curve" => vec![
            Example { name: "Within one standard deviation", params: vec![] },
            Example {
                name: "Standard normal, 95%",
                params: vec![("mean", Value::F(0.0)), ("sd", Value::F(1.0)), ("by", s("z-scores")), ("a", Value::F(-1.96)), ("b", Value::F(1.96)), ("ticks", s("z-scores"))],
            },
            Example {
                name: "Scores above a cutoff",
                params: vec![("mean", Value::F(72.0)), ("sd", Value::F(8.0)), ("region", s("Above")), ("a", Value::F(85.0)), ("x_title", s("Exam score"))],
            },
            Example {
                name: "Both tails",
                params: vec![("region", s("Outside")), ("a", Value::F(70.0)), ("b", Value::F(130.0)), ("ticks", s("Values and z-scores"))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("mean", "Mean", -10000.0, 10000.0, 1.0, 100.0),
        ParamDef::float("sd", "Standard deviation", 0.001, 10000.0, 1.0, 15.0),
        ParamDef::choice("region", "Shade", &["None", "Below", "Above", "Between", "Outside"], "Between").hint("Below and Above use one bound; Between shades the middle; Outside shades both tails"),
        ParamDef::choice("by", "Bounds are", &["x values", "z-scores"], "x values"),
        ParamDef::float("a", "Lower bound", -10000.0, 10000.0, 0.5, 85.0),
        ParamDef::float("b", "Upper bound", -10000.0, 10000.0, 0.5, 115.0),
        ParamDef::bool("show_prob", "Write the area as a probability", true),
        ParamDef::choice("ticks", "Axis labels", &["Values", "z-scores", "Values and z-scores"], "Values"),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the probability and keeps the shaded region, for students to find the area"),
    ]
}

// ───────────────────────── the maths ─────────────────────────

/// The error function: a series for small arguments, a continued fraction for the tails (both
/// good to about 1e-12).
pub fn erf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let ax = x.abs();
    let v = if ax <= 3.0 {
        let (mut term, mut sum) = (ax, ax);
        for n in 1..120 {
            term *= -ax * ax / n as f64;
            let add = term / (2 * n + 1) as f64;
            sum += add;
            if add.abs() < 1e-17 * sum.abs() {
                break;
            }
        }
        sum * 2.0 / std::f64::consts::PI.sqrt()
    } else {
        1.0 - erfc_tail(ax)
    };
    if x < 0.0 { -v } else { v }
}

/// erfc for x > 3 (continued fraction, evaluated from the tail inward).
fn erfc_tail(x: f64) -> f64 {
    if x > 27.0 {
        return 0.0;
    }
    let mut t = x;
    for k in (1..=60).rev() {
        t = x + (k as f64 / 2.0) / t;
    }
    (-x * x).exp() / (std::f64::consts::PI.sqrt() * t)
}

/// P(Z <= z) for the standard normal.
pub fn cdf(z: f64) -> f64 {
    if z.is_nan() {
        return f64::NAN;
    }
    if z < -3.0 * std::f64::consts::SQRT_2 {
        return 0.5 * erfc_tail(-z / std::f64::consts::SQRT_2);
    }
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

pub fn pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// The z-intervals shaded for `region` with bounds `a`, `b` (swapped if given the wrong way
/// round), kept inside the plotted range [lo, hi].
pub fn intervals(region: &str, a: f64, b: f64, lo: f64, hi: f64) -> Vec<(f64, f64)> {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let raw: Vec<(f64, f64)> = match region {
        "Below" => vec![(lo, b)],
        "Above" => vec![(a, hi)],
        "Between" => vec![(a, b)],
        "Outside" => vec![(lo, a), (b, hi)],
        _ => vec![],
    };
    raw.into_iter().map(|(x, y)| (x.max(lo), y.min(hi))).filter(|(x, y)| x < y).collect()
}

/// The probability the region covers (z-scores `za` <= `zb` already ordered by the caller of `render`).
pub fn probability(region: &str, za: f64, zb: f64) -> f64 {
    let (za, zb) = if za <= zb { (za, zb) } else { (zb, za) };
    match region {
        "Below" => cdf(zb),
        "Above" => cdf(-za),
        "Between" => cdf(zb) - cdf(za),
        "Outside" => cdf(za) + cdf(-zb),
        _ => 0.0,
    }
}

/// A probability as text: four decimals, but never "0.0000" or "1.0000" for a value that is not
/// exactly 0 or 1.
pub fn prob_text(p: f64) -> String {
    if !p.is_finite() {
        return "?".into();
    }
    let p = p.clamp(0.0, 1.0);
    if p == 0.0 {
        "0".into()
    } else if p == 1.0 {
        "1".into()
    } else if p < 0.00005 {
        "< 0.0001".into()
    } else if p > 0.99995 {
        "> 0.9999".into()
    } else {
        format!("{p:.4}")
    }
}

// ───────────────────────── drawing ─────────────────────────

const RANGE_SD: f64 = 4.0;

fn render(p: &Params) -> String {
    let (mean, sd) = (p.f("mean"), p.f("sd"));
    if !(mean.is_finite() && sd.is_finite() && sd > 0.0) {
        return chart::message("The standard deviation must be above 0");
    }
    let region = p.s("region");
    let by_z = p.s("by") == "z-scores";
    let to_z = |v: f64| if by_z { v } else { (v - mean) / sd };
    let (ra, rb) = (p.f("a"), p.f("b"));
    if !(ra.is_finite() && rb.is_finite()) {
        return chart::message("Enter the bounds as numbers");
    }
    let (za, zb) = (to_z(ra), to_z(rb));
    let (za, zb) = if za <= zb { (za, zb) } else { (zb, za) };
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (title, x_title) = (p.s("title").trim(), p.s("x_title").trim());
    let two_line = p.s("ticks") == "Values and z-scores";
    let readout = region != "None" && p.b("show_prob");

    let (w, h) = (640.0, 360.0 + (k - 1.0) * 50.0 + if two_line { fs * 1.2 } else { 0.0 });
    let top = if title.is_empty() { 22.0 } else { 32.0 + fs * 1.4 } + if readout { fs * 2.2 } else { 0.0 };
    let bottom = 26.0 + fs * if two_line { 3.2 } else { 2.0 } + if x_title.is_empty() { 0.0 } else { fs * 1.7 };
    let f = chart::Frame { w, h, l: 28.0, r: 28.0, t: top, b: bottom, x0: -RANGE_SD, x1: RANGE_SD, y0: 0.0, y1: pdf(0.0) * 1.12 };

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }

    // shaded regions under the curve (z on the x axis internally: -4 .. 4)
    for (lo, hi) in intervals(region, za, zb, -RANGE_SD, RANGE_SD) {
        let steps = (((hi - lo) / (2.0 * RANGE_SD)) * 160.0).ceil().max(2.0) as usize;
        let mut pts = vec![(f.px(lo), f.bottom())];
        for i in 0..=steps {
            let z = lo + (hi - lo) * i as f64 / steps as f64;
            pts.push((f.px(z), f.py(pdf(z))));
        }
        pts.push((f.px(hi), f.bottom()));
        d.poly(&pts, &fills.series(0), "none", 0.0);
    }
    // the curve itself, and the baseline
    let curve: Vec<(f64, f64)> = (0..=160).map(|i| {
        let z = -RANGE_SD + 2.0 * RANGE_SD * i as f64 / 160.0;
        (f.px(z), f.py(pdf(z)))
    }).collect();
    d.polyline(&curve, INK, 2.6);
    d.line(f.left() - 8.0, f.bottom(), f.right() + 8.0, f.bottom(), INK, 2.2);

    // bound markers: a dashed line up to the curve, labelled with the bound as given
    if region != "None" {
        let marks: Vec<f64> = match region {
            "Below" => vec![zb],
            "Above" => vec![za],
            _ => vec![za, zb],
        };
        for z in marks {
            if z.abs() > RANGE_SD {
                continue;
            }
            let (x, y) = (f.px(z), f.py(pdf(z)));
            d.raw(&format!(r#"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{y:.2}" stroke="{INK}" stroke-width="1.6" stroke-dasharray="5 4"/>"#, f.bottom()));
            let value = if by_z { z } else { mean + z * sd };
            let text = if by_z { format!("z = {}", chart::fmt((value * 1000.0).round() / 1000.0)) } else { chart::fmt((value * 1000.0).round() / 1000.0) };
            // up and outward from the curve point: the curve falls away on both sides of the
            // mean, so the corner above and away from the middle is clear of it
            if z < 0.0 {
                chart::text(&mut d, x - 7.0, y - 7.0, fs * 0.95, "end", &text);
            } else {
                chart::text(&mut d, x + 7.0, y - 7.0, fs * 0.95, "start", &text);
            }
        }
    }

    // axis ticks at the mean and each standard deviation
    for kk in -3..=3 {
        let x = f.px(kk as f64);
        d.line(x, f.bottom(), x, f.bottom() + 7.0, INK, 1.8);
        let value = chart::fmt(mean + kk as f64 * sd);
        let zed = chart::fmt(kk as f64);
        let (line1, line2) = match p.s("ticks") {
            "z-scores" => (zed, None),
            "Values and z-scores" => (value, Some(format!("z = {zed}"))),
            _ => (value, None),
        };
        chart::text(&mut d, x, f.bottom() + 10.0 + fs, fs, "middle", &line1);
        if let Some(l2) = line2 {
            chart::text(&mut d, x, f.bottom() + 10.0 + fs * 2.2, fs * 0.9, "middle", &l2);
        }
    }
    if !x_title.is_empty() {
        chart::text(&mut d, w / 2.0, h - 10.0, fs * 1.1, "middle", x_title);
    }

    // the answer: the probability under the shaded region
    if readout && p.b(SHOW_ANSWERS) {
        let prob = prob_text(probability(region, za, zb));
        // the bounds as the user gave them (not recomputed from z, which can overflow)
        let (va, vb) = if ra <= rb { (ra, rb) } else { (rb, ra) };
        let var = if by_z { "Z" } else { "X" };
        let num = |v: f64| chart::fmt((v * 1000.0).round() / 1000.0);
        let event = match region {
            "Below" => format!("{var} < {}", num(vb)),
            "Above" => format!("{var} > {}", num(va)),
            "Between" => format!("{} < {var} < {}", num(va), num(vb)),
            _ => format!("{var} < {} or {var} > {}", num(va), num(vb)),
        };
        chart::text(&mut d, w / 2.0, top - fs * 0.9, fs * 1.15, "middle", &format!("P({event}) = {prob}"));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn erf_matches_known_values() {
        for (x, want) in [(0.0, 0.0), (0.5, 0.5204998778130465), (1.0, 0.8427007929497149), (2.0, 0.9953222650189527), (-1.0, -0.8427007929497149), (3.0, 0.9999779095030014), (4.0, 0.9999999845827421)] {
            assert!((erf(x) - want).abs() < 1e-12, "erf({x}) = {} vs {want}", erf(x));
        }
        assert!(erf(f64::NAN).is_nan());
        assert_eq!(erf(40.0), 1.0);
    }

    #[test]
    fn cdf_matches_the_normal_table() {
        for (z, want) in [(0.0, 0.5), (1.0, 0.8413447460685429), (1.96, 0.9750021048517795), (-1.0, 0.15865525393145707), (3.0, 0.9986501019683699), (-5.0, 2.866515718791939e-7)] {
            let got = cdf(z);
            assert!((got - want).abs() < 1e-12 * want.max(1e-3) + 1e-14, "cdf({z}) = {got} vs {want}");
        }
        assert!(cdf(-40.0) >= 0.0 && cdf(-40.0) < 1e-300);
        assert!(cdf(f64::NAN).is_nan());
        assert!((cdf(1.234) + cdf(-1.234) - 1.0).abs() < 1e-14);
    }

    #[test]
    fn regions_partition_the_curve() {
        let (za, zb) = (-0.7, 1.3);
        let between = probability("Between", za, zb);
        assert!((between + probability("Outside", za, zb) - 1.0).abs() < 1e-12);
        assert!((probability("Below", 0.7, 0.7) + probability("Above", 0.7, 0.7) - 1.0).abs() < 1e-12);
        assert!((probability("Below", 0.0, 0.0) - 0.5).abs() < 1e-12);
        assert!((probability("Between", -1.0, 1.0) - 0.6826894921370859).abs() < 1e-12);
        assert_eq!(probability("None", 0.0, 1.0), 0.0);
        assert_eq!(probability("Between", 1.0, -1.0), probability("Between", -1.0, 1.0), "bounds given backwards");
    }

    #[test]
    fn shaded_intervals_follow_the_region() {
        assert_eq!(intervals("Between", -1.0, 1.0, -4.0, 4.0), vec![(-1.0, 1.0)]);
        assert_eq!(intervals("Below", -9.0, 1.0, -4.0, 4.0), vec![(-4.0, 1.0)]);
        assert_eq!(intervals("Above", -1.0, 9.0, -4.0, 4.0), vec![(-1.0, 4.0)]);
        assert_eq!(intervals("Outside", -1.0, 1.0, -4.0, 4.0), vec![(-4.0, -1.0), (1.0, 4.0)]);
        assert!(intervals("None", -1.0, 1.0, -4.0, 4.0).is_empty());
        assert!(intervals("Between", 6.0, 8.0, -4.0, 4.0).is_empty(), "entirely off the plot");
        assert_eq!(intervals("Between", 1.0, -1.0, -4.0, 4.0), vec![(-1.0, 1.0)], "swapped bounds");
    }

    #[test]
    fn probabilities_read_sensibly() {
        assert_eq!(prob_text(0.0), "0");
        assert_eq!(prob_text(1.0), "1");
        assert_eq!(prob_text(0.6826894), "0.6827");
        assert_eq!(prob_text(0.00001), "< 0.0001");
        assert_eq!(prob_text(0.999999), "> 0.9999");
        assert_eq!(prob_text(0.3), "0.3000");
        assert_eq!(prob_text(f64::NAN), "?", "an unusable probability is not printed as NaN");
    }

    #[test]
    fn default_shades_one_region_and_writes_the_area() {
        let svg = render(&defaults());
        assert_eq!(svg.matches("<polygon").count(), 1);
        assert!(svg.contains("P(85 &lt; X &lt; 115) = 0.6827"), "{svg}");
    }

    #[test]
    fn z_score_bounds_are_labelled_as_z() {
        let mut p = defaults();
        p.set_s("by", "z-scores");
        p.set_f("a", -1.96);
        p.set_f("b", 1.96);
        let svg = render(&p);
        assert!(svg.contains("z = 1.96") && svg.contains("P(") && svg.contains("0.9500"), "{svg}");
    }

    #[test]
    fn outside_shades_both_tails() {
        let mut p = defaults();
        p.set_s("region", "Outside");
        assert_eq!(render(&p).matches("<polygon").count(), 2);
    }

    #[test]
    fn student_copy_keeps_the_shading_and_hides_the_area() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert_eq!(student.matches("<polygon").count(), 1, "the question is the shaded region");
        assert!(!student.contains("P(") && !student.contains("0.68"), "the probability is the answer");
        assert!(key.contains("P("));
        // the two copies are the same size, so they can be swapped in a document
        let size = |s: &str| s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string();
        assert_eq!(size(&student), size(&key));
    }

    #[test]
    fn print_style_is_hatched_and_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(svg.contains("url(#vs-hatch0)") && !svg.contains("#4f7cd4"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn odd_input_never_panics_or_prints_nan() {
        let mut p = defaults();
        let nan = f64::NAN;
        for (mean, sd, a, b) in [(100.0, 0.0, 85.0, 115.0), (100.0, -3.0, 85.0, 115.0), (nan, 15.0, 85.0, 115.0), (100.0, nan, 85.0, 115.0), (100.0, 15.0, nan, 115.0), (100.0, 15.0, 85.0, f64::INFINITY),
                                (1e300, 1e-300, -1e300, 1e300), (0.0, 1e-9, -5.0, 5.0), (0.0, 1e9, 1e12, -1e12), (0.0, 1.0, 50.0, 60.0), (0.0, 1.0, -60.0, -50.0)] {
            p.set_f("mean", mean);
            p.set_f("sd", sd);
            p.set_f("a", a);
            p.set_f("b", b);
            for region in ["None", "Below", "Above", "Between", "Outside"] {
                for by in ["x values", "z-scores"] {
                    p.set_s("region", region);
                    p.set_s("by", by);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{mean} {sd} {a} {b} {region}");
                    assert!(!svg.contains("NaN") && !svg.contains("inf"), "{mean} {sd} {a} {b} {region} {by}: {svg}");
                }
            }
        }
    }

    #[test]
    fn relevance_follows_the_region() {
        let mut p = defaults();
        assert!(relevant("normal-curve", "a", &p) && relevant("normal-curve", "b", &p));
        p.set_s("region", "Below");
        assert!(!relevant("normal-curve", "a", &p) && relevant("normal-curve", "b", &p));
        p.set_s("region", "Above");
        assert!(relevant("normal-curve", "a", &p) && !relevant("normal-curve", "b", &p));
        p.set_s("region", "None");
        assert!(!relevant("normal-curve", "by", &p) && !relevant("normal-curve", "show_prob", &p));
        assert!(relevant("histogram", "a", &p));
    }

    #[test]
    fn examples_render() {
        for e in examples("normal-curve").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            assert!(render(&p).contains("<polygon"), "{}", e.name);
        }
    }
}
