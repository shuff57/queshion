//! Dot plot: one dot for each value, stacked above a number line. The dots are what a student is
//! asked to draw, so the student copy (Show answers off) keeps the number line and hides the dots
//! and the mean/median marks.
use std::collections::BTreeMap;

use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "dot-plot",
        subject: Subject::Math,
        title: "Dot Plot",
        blurb: "A dot plot of a data list: one dot per value, stacked above a number line, with optional mean and median marks.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("dot-plot", "axis_min" | "axis_max" | "axis_step") => !p.b("axis_auto"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "dot-plot" => vec![
            Example { name: "Shoe sizes", params: vec![] },
            Example {
                name: "Two peaks",
                params: vec![
                    ("data", s(&data::column_text(&data::generate(data::Dist::Bimodal, 30, 6, 20.0, 5.0, 0)))),
                    ("x_title", s("Minutes to get to school")),
                    ("marks", s("Mean and median")),
                ],
            },
            Example {
                name: "Grouped measurements",
                params: vec![
                    ("data", s(&data::column_text(&data::generate(data::Dist::Normal, 40, 2, 50.0, 6.0, 1)))),
                    ("group", Value::F(2.0)),
                    ("x_title", s("Height of seedling (mm)")),
                ],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("data", "Numbers", 1, "5, 6, 6, 7, 7, 7, 8, 8, 8, 8, 9, 9, 9, 10, 10, 11, 12"),
        ParamDef::float("group", "Group size", 0.0, 1000.0, 0.5, 0.0).hint("Values are rounded to the nearest multiple of this, so close values share a stack. 0 keeps exact values"),
        ParamDef::bool("axis_auto", "Choose the number line automatically", true),
        ParamDef::float("axis_min", "Number line minimum", -10000.0, 10000.0, 1.0, 0.0),
        ParamDef::float("axis_max", "Number line maximum", -10000.0, 10000.0, 1.0, 12.0),
        ParamDef::float("axis_step", "Number line step", 0.001, 10000.0, 0.5, 1.0),
        ParamDef::choice("dot_size", "Dot size", &["Small", "Medium", "Large"], "Medium"),
        ParamDef::choice("marks", "Mark", &["None", "Mean", "Median", "Mean and median"], "None"),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the dots and marks, leaving the number line for students to plot on"),
    ]
}

/// The values stacked: each distinct (grouped) value with how many dots it gets, in order.
/// `group` > 0 rounds every value to the nearest multiple of it first.
pub fn stacks(values: &[f64], group: f64) -> Vec<(f64, usize)> {
    let mut m: BTreeMap<i64, (f64, usize)> = BTreeMap::new();
    for v in values {
        let (key, shown) = if group > 0.0 && group.is_finite() {
            let k = (v / group).round();
            (k as i64, k * group)
        } else {
            // exact values: values that agree to nine places are the same stack
            ((v * 1e9).round() as i64, *v)
        };
        m.entry(key).or_insert((shown, 0)).1 += 1;
    }
    m.into_values().collect()
}

pub fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 })
}

fn render(p: &Params) -> String {
    let values = data::numbers(p.s("data"));
    if values.is_empty() {
        return chart::message("Enter some data");
    }
    let stacks = stacks(&values, p.f("group"));
    let (lo, hi) = (stacks.first().unwrap().0, stacks.last().unwrap().0);
    let (a0, a1, step) = if p.b("axis_auto") {
        chart::axis_range(lo, hi, 8.0)
    } else {
        let (a, b) = (p.f("axis_min"), p.f("axis_max"));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        (a, if b > a { b } else { a + 1.0 }, p.f("axis_step").max(1e-9))
    };
    if !(a0.is_finite() && a1.is_finite() && step.is_finite()) {
        return chart::message("Check the number line settings");
    }
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (title, x_title) = (p.s("title").trim(), p.s("x_title").trim());
    let plot_h = 190.0;
    let top = if title.is_empty() { 26.0 } else { 36.0 + fs * 1.4 };
    let bottom = 24.0 + fs * 1.9 + if x_title.is_empty() { 0.0 } else { fs * 1.7 };
    let (w, h) = (640.0, top + plot_h + bottom);
    let f = chart::Frame { w, h, l: 36.0, r: 36.0, t: top, b: bottom, x0: a0, x1: a1, y0: 0.0, y1: 1.0 };

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    d.line(f.left() - 10.0, f.bottom(), f.right() + 10.0, f.bottom(), INK, 2.4);
    for (v, label) in chart::ticks(a0, a1, step) {
        let x = f.px(v);
        d.line(x, f.bottom() - 6.0, x, f.bottom() + 8.0, INK, 1.8);
        chart::text(&mut d, x, f.bottom() + 12.0 + fs, fs, "middle", &label);
    }
    if !x_title.is_empty() {
        chart::text(&mut d, w / 2.0, h - 10.0, fs * 1.1, "middle", x_title);
    }

    if p.b(SHOW_ANSWERS) {
        // the mean / median marks go down first so the dots sit on top of their dashed lines
        let marks = p.s("marks");
        let mut lines: Vec<(&str, f64)> = Vec::new();
        if matches!(marks, "Mean" | "Mean and median") {
            lines.extend(mean(&values).map(|m| ("mean", m)));
        }
        if matches!(marks, "Median" | "Mean and median") {
            lines.extend(median(&values).map(|m| ("median", m)));
        }
        lines.retain(|(_, v)| *v >= a0 && *v <= a1);
        lines.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let label_y = f.top() + fs * 1.2;
        for (i, (name, v)) in lines.iter().enumerate() {
            let x = f.px(*v);
            // labels that would overprint each other go either side of their lines
            let near_next = lines.get(i + 1).is_some_and(|n| f.px(n.1) - x < 130.0);
            let near_prev = i > 0 && x - f.px(lines[i - 1].1) < 130.0;
            let (anchor, dx) = if near_next { ("end", -7.0) } else if near_prev { ("start", 7.0) } else { ("middle", 0.0) };
            d.raw(&format!(r#"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.8" stroke-dasharray="6 4"/>"#, f.bottom(), label_y - fs * 0.35));
            chart::text(&mut d, x + dx, label_y, fs, anchor, &format!("{name} = {}", chart::fmt((v * 100.0).round() / 100.0)));
        }
        // dot radius: the chosen size, shrunk so neighbouring stacks do not touch and the tallest
        // stack fits the plot
        let base: f64 = match p.s("dot_size") {
            "Small" => 4.5,
            "Large" => 9.0,
            _ => 6.5,
        };
        let min_gap = stacks.windows(2).map(|w| f.px(w[1].0) - f.px(w[0].0)).fold(f64::MAX, f64::min);
        let tallest = stacks.iter().map(|s| s.1).max().unwrap_or(1) as f64;
        let r = base.min(min_gap * 0.45).min((plot_h - 10.0) / (tallest * 2.2)).max(0.8);
        let pitch = r * 2.2;
        for (v, count) in &stacks {
            let x = f.px(*v);
            for i in 0..*count {
                let y = f.bottom() - 4.0 - r - i as f64 * pitch;
                if y < f.top() {
                    break;
                }
                d.circle(x, y, r, &fills.series(0), INK, 1.3);
            }
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn stacks_count_each_value() {
        assert_eq!(stacks(&[3.0, 1.0, 3.0, 2.0, 3.0], 0.0), vec![(1.0, 1), (2.0, 1), (3.0, 3)]);
        assert!(stacks(&[], 0.0).is_empty());
        let near = stacks(&[0.1 + 0.2, 0.3], 0.0);
        assert_eq!(near.len(), 1, "float noise does not split a stack: {near:?}");
    }

    #[test]
    fn grouping_rounds_to_the_nearest_multiple() {
        let s = stacks(&[1.2, 1.4, 2.9, 3.1, 7.6], 2.0);
        assert_eq!(s, vec![(2.0, 3), (4.0, 1), (8.0, 1)], "1.2, 1.4 and 2.9 round to 2; 3.1 to 4; 7.6 to 8");
    }

    #[test]
    fn mean_and_median() {
        assert_eq!(mean(&[]), None);
        assert_eq!(mean(&[1.0, 2.0, 6.0]), Some(3.0));
        assert_eq!(median(&[5.0, 1.0, 3.0]), Some(3.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn default_draws_one_dot_per_value() {
        let p = defaults();
        let n = data::numbers(p.s("data")).len();
        assert_eq!(render(&p).matches("<circle").count(), n);
    }

    #[test]
    fn student_copy_keeps_the_number_line_only() {
        let def = def();
        let mut p = defaults();
        p.set_s("marks", "Mean and median");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert_eq!(student.matches("<circle").count(), 0);
        assert!(!student.contains("mean =") && !student.contains("median ="));
        assert!(student.contains(">12<") && student.contains(">5<"), "the number line is labelled");
        assert!(key.contains("mean = ") && key.contains("median = 8"));
        let size = |s: &str| s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string();
        assert_eq!(size(&student), size(&key), "same size, so the copies can be swapped");
    }

    #[test]
    fn print_style_is_grey() {
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
    fn manual_number_line_is_used() {
        let mut p = defaults();
        p.set_b("axis_auto", false);
        p.set_f("axis_min", 0.0);
        p.set_f("axis_max", 20.0);
        p.set_f("axis_step", 5.0);
        let svg = render(&p);
        assert!(svg.contains(">20<") && svg.contains(">15<") && !svg.contains(">12<"));
        assert!(relevant("dot-plot", "axis_step", &p) && !relevant("dot-plot", "axis_step", &defaults()));
    }

    #[test]
    fn a_tall_stack_and_tight_values_still_fit() {
        let mut p = defaults();
        p.set_s("data", &vec!["5"; 400].join(", "));
        let svg = render(&p);
        assert!(svg.contains("<circle"), "a 400-high stack is drawn, smaller");
        p.set_s("data", "1, 1.01, 1.02, 1.03, 2");
        assert!(render(&p).contains("<circle"));
    }

    #[test]
    fn odd_input_never_panics_or_prints_nan() {
        let mut p = defaults();
        p.set_s("marks", "Mean and median");
        for text in ["", "abc", "7", "7, 7, 7", "-1e12, 1e12", "1e12, 1e12", "0.0000001, 0.0000002", "1, 2, 3, 1000000000"] {
            for group in [0.0, 0.001, 5.0, 1e9] {
                p.set_s("data", text);
                p.set_f("group", group);
                for answers in [true, false] {
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?} {group}");
                    assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text:?} {group}: {svg}");
                }
            }
        }
        p.set_s("data", "1, 2");
        p.set_b("axis_auto", false);
        for (lo, hi, st) in [(5.0, 5.0, 1.0), (10.0, 0.0, 2.0), (0.0, 1e12, 1e-9), (f64::NAN, 1.0, 1.0)] {
            p.set_f("axis_min", lo);
            p.set_f("axis_max", hi);
            p.set_f("axis_step", st);
            let svg = render(&p);
            assert!(svg.starts_with("<svg") && !svg.contains("NaN") && !svg.contains("inf"), "{lo} {hi} {st}");
        }
    }

    #[test]
    fn examples_render() {
        for e in examples("dot-plot").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            assert!(render(&p).contains("<circle"), "{}", e.name);
        }
    }
}
