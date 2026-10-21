//! Statistics figures (Math tab): histogram, and the other data plots that follow it. Original
//! designs on the shared pieces: `chart` (frame, ticks, axes), `data` (typed, pasted, CSV or
//! generated numbers), `Fills` (colour or black-and-white hatching) and `show_answers` (the
//! student copy keeps the axes and hides what the student is asked to draw).
use super::chart::{self, AxesOpts, Frame, INK};
use super::data;
use super::svg::{self, Doc, Fills};
use super::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value};

mod dot;
mod normal;
mod prob_tree;
mod scatter;
mod stem;
mod twoway;
mod venn;

pub fn defs() -> Vec<FigureDef> {
    vec![dot::def(), normal::def(), prob_tree::def(), scatter::def(), stem::def(), twoway::def(), venn::def(), FigureDef {
        id: "histogram",
        subject: Subject::Math,
        title: "Histogram",
        blurb: "A histogram of a data list, with bins chosen for you or set by hand; frequency, relative frequency or density.",
        params: histogram_params,
        render: histogram_render,
    }]
    .into_iter()
    .rev()
    .collect()
}

/// Params that only matter in some states of the figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("histogram", "bin_width" | "bin_start") => !p.b("auto_bins"),
        ("histogram", "y_max") => !p.b("auto_y"),
        _ => scatter::relevant(fig, key, p) && normal::relevant(fig, key, p) && dot::relevant(fig, key, p) && stem::relevant(fig, key, p) && twoway::relevant(fig, key, p) && prob_tree::relevant(fig, key, p) && venn::relevant(fig, key, p),
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "histogram" => vec![
            Example { name: "Test scores", params: vec![] },
            Example {
                name: "Skewed incomes",
                params: vec![
                    ("data", s(&data::column_text(&data::generate(data::Dist::SkewedRight, 60, 4, 52.0, 18.0, 0)))),
                    ("x_title", s("Income (thousands of dollars)")),
                ],
            },
            Example { name: "Relative frequency", params: vec![("measure", s("Relative frequency")), ("values", Value::B(true))] },
        ],
        _ => return scatter::examples(id).or_else(|| normal::examples(id)).or_else(|| dot::examples(id)).or_else(|| stem::examples(id)).or_else(|| twoway::examples(id)).or_else(|| prob_tree::examples(id)).or_else(|| venn::examples(id)),
    })
}

// ───────────────────────── histogram ─────────────────────────

const MAX_BINS: usize = 200;

fn histogram_params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("data", "Numbers", 1, "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93"),
        ParamDef::bool("auto_bins", "Choose the bins automatically", true),
        ParamDef::float("bin_width", "Bin width", 0.01, 10000.0, 0.5, 10.0),
        ParamDef::float("bin_start", "First bin starts at", -10000.0, 10000.0, 0.5, 50.0),
        ParamDef::choice("closed", "A value on a bin edge goes", &["In the bin to its right", "In the bin to its left"], "In the bin to its right"),
        ParamDef::choice("measure", "Bar height", &["Frequency", "Relative frequency", "Density"], "Frequency"),
        ParamDef::bool("auto_y", "Choose the height scale automatically", true),
        ParamDef::float("y_max", "Height scale top", 0.0001, 1_000_000.0, 1.0, 10.0),
        ParamDef::bool("values", "Write the value above each bar", false),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", ""),
        ParamDef::text("y_title", "Vertical axis title (blank = the bar height)", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the bars and values, leaving the axes for students to draw on"),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bin {
    pub lo: f64,
    pub hi: f64,
    pub n: usize,
}

/// A bin width of 1, 2 or 5 times a power of ten that gives about Sturges' number of bins.
pub fn auto_width(values: &[f64]) -> f64 {
    let (lo, hi) = values.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    if values.is_empty() || !(hi > lo) {
        return 1.0;
    }
    chart::nice_step(hi - lo, (1.0 + (values.len() as f64).log2()).ceil())
}

/// Where automatic bins begin so the smallest value is inside the first bin.
pub fn auto_start(values: &[f64], width: f64, left_closed: bool) -> f64 {
    let min = values.iter().cloned().fold(f64::MAX, f64::min);
    let q = (min / width * 1e9).round() / 1e9;
    if left_closed { q.floor() * width } else { (q.ceil() - 1.0) * width }
}

/// The bins from `start` in steps of `width`, with each value counted once. Values below
/// `start` are left out. None when the width is not usable or there would be over [`MAX_BINS`].
pub fn bins(values: &[f64], start: f64, width: f64, left_closed: bool) -> Option<Vec<Bin>> {
    if !(width.is_finite() && width > 0.0 && start.is_finite()) {
        return None;
    }
    let idx = |v: f64| -> i64 {
        let q = ((v - start) / width * 1e9).round() / 1e9;
        (if left_closed { q.floor() } else { q.ceil() - 1.0 }) as i64
    };
    let top = values.iter().map(|v| idx(*v)).max()?;
    if top < 0 {
        return Some(Vec::new());
    }
    if top as usize >= MAX_BINS {
        return None;
    }
    let mut out: Vec<Bin> = (0..=top).map(|i| Bin { lo: start + i as f64 * width, hi: start + (i + 1) as f64 * width, n: 0 }).collect();
    for v in values {
        let i = idx(*v);
        if i >= 0 {
            out[i as usize].n += 1;
        }
    }
    Some(out)
}

fn measure_values(bins: &[Bin], measure: &str) -> Vec<f64> {
    let total: usize = bins.iter().map(|b| b.n).sum();
    let total = total.max(1) as f64;
    bins.iter()
        .map(|b| match measure {
            "Relative frequency" => b.n as f64 / total,
            "Density" => b.n as f64 / (total * (b.hi - b.lo)),
            _ => b.n as f64,
        })
        .collect()
}

fn histogram_render(p: &Params) -> String {
    let values = data::numbers(p.s("data"));
    if values.is_empty() {
        return chart::message("Enter some data");
    }
    let left_closed = p.s("closed") != "In the bin to its left";
    let (width, start) = if p.b("auto_bins") {
        let w = auto_width(&values);
        (w, auto_start(&values, w, left_closed))
    } else {
        (p.f("bin_width"), p.f("bin_start"))
    };
    let Some(bs) = bins(&values, start, width, left_closed) else {
        return chart::message("Too many bars: make the bins wider");
    };
    if bs.is_empty() {
        return chart::message("No data at or above the first bin");
    }
    let measure = p.s("measure");
    let heights = measure_values(&bs, measure);
    let hmax = heights.iter().cloned().fold(0.0, f64::max);
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;

    // the vertical scale: nice ticks up to a top that holds the tallest bar
    let (ystep, ytop) = if p.b("auto_y") {
        let mut step = chart::nice_step(hmax.max(1e-9), 5.0);
        if measure == "Frequency" {
            step = step.max(1.0);
        }
        (step, (hmax / step - 1e-9).ceil().max(1.0) * step)
    } else {
        let top = p.f("y_max").max(1e-9);
        (chart::nice_step(top, 5.0), top)
    };
    let yticks = chart::ticks(0.0, ytop, ystep);
    let y_title = match p.s("y_title").trim() {
        "" => measure,
        t => t,
    };
    let x_title = p.s("x_title").trim();
    let title = p.s("title").trim();

    // every edge is a tick; a long run is thinned so the labels never touch
    let every = (bs.len() as f64 / 12.0).ceil().max(1.0) as usize;
    let mut xticks: Vec<chart::Tick> = bs.iter().enumerate().filter(|(i, _)| i % every == 0).map(|(_, b)| (b.lo, chart::fmt(b.lo))).collect();
    if (bs.len() % every == 0) || every == 1 {
        let last = bs.last().unwrap();
        xticks.push((last.hi, chart::fmt(last.hi)));
    }

    let label_chars = yticks.iter().map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let w = 640.0;
    let h = 420.0 + (k - 1.0) * 60.0;
    let frame = Frame {
        w,
        h,
        l: 34.0 + fs * 1.4 + label_chars * fs * 0.62,
        r: 28.0,
        t: (if title.is_empty() { 24.0 } else { 34.0 + fs * 1.4 }) + if p.b("values") && p.b(super::SHOW_ANSWERS) { fs * 1.1 } else { 0.0 },
        b: 22.0 + fs * 1.9 + if x_title.is_empty() { 0.0 } else { fs * 1.7 },
        x0: bs[0].lo,
        x1: bs.last().unwrap().hi,
        y0: 0.0,
        y1: ytop,
    };

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    chart::axes(&mut d, &frame, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: p.b("grid"), x_title, y_title, fs });
    if p.b(super::SHOW_ANSWERS) {
        for (b, v) in bs.iter().zip(&heights) {
            let (x0, x1, y) = (frame.px(b.lo), frame.px(b.hi), frame.py(v.min(ytop)));
            d.rect(x0, y, x1 - x0, frame.bottom() - y, &fills.series(0), INK, 1.6);
        }
        // the axes are drawn again over the bars' feet so the baseline stays crisp
        d.line(frame.left(), frame.bottom(), frame.right(), frame.bottom(), INK, 2.0);
        if p.b("values") {
            for (b, v) in bs.iter().zip(&heights) {
                if *v > 0.0 {
                    let text = if measure == "Frequency" { b.n.to_string() } else { chart::fmt((v * 1000.0).round() / 1000.0) };
                    let (cx, cy) = ((frame.px(b.lo) + frame.px(b.hi)) / 2.0, frame.py(v.min(ytop)) - 6.0);
                    chart::text(&mut d, cx, cy, fs * 0.9, "middle", &text);
                }
            }
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults(id: &str) -> Params {
        let def = defs().into_iter().find(|d| d.id == id).unwrap();
        Params::defaults(&(def.params)())
    }

    #[test]
    fn every_value_lands_in_exactly_one_bin() {
        let v = [1.0, 2.0, 2.0, 3.0, 4.0, 4.0, 4.0, 9.0];
        for left in [true, false] {
            let w = 2.0;
            let b = bins(&v, auto_start(&v, w, left), w, left).unwrap();
            assert_eq!(b.iter().map(|b| b.n).sum::<usize>(), v.len(), "left_closed={left}");
        }
    }

    #[test]
    fn edge_values_follow_the_closed_side() {
        let v = [10.0, 20.0, 20.0, 30.0];
        let right = bins(&v, 10.0, 10.0, true).unwrap();
        assert_eq!(right.iter().map(|b| b.n).collect::<Vec<_>>(), [1, 2, 1], "[10,20) [20,30) [30,40)");
        let left = bins(&v, 0.0, 10.0, false).unwrap();
        assert_eq!(left.iter().map(|b| b.n).collect::<Vec<_>>(), [1, 2, 1], "(0,10] (10,20] (20,30]");
        assert_eq!((left[0].lo, left[0].hi), (0.0, 10.0));
    }

    #[test]
    fn float_noise_does_not_move_a_value_to_the_next_bin() {
        let v = [0.1 + 0.2, 0.3];
        let b = bins(&v, 0.0, 0.1, true).unwrap();
        assert_eq!(b.iter().map(|b| b.n).sum::<usize>(), 2);
        assert_eq!(b.iter().filter(|b| b.n == 2).count(), 1, "0.1+0.2 and 0.3 share a bin: {b:?}");
    }

    #[test]
    fn bad_widths_and_huge_bin_counts_are_refused() {
        let v = [0.0, 1.0];
        assert!(bins(&v, 0.0, 0.0, true).is_none());
        assert!(bins(&v, 0.0, -1.0, true).is_none());
        assert!(bins(&v, f64::NAN, 1.0, true).is_none());
        assert!(bins(&[0.0, 1e9], 0.0, 1.0, true).is_none(), "a billion bars");
        assert_eq!(bins(&[1.0], 5.0, 1.0, true).unwrap(), vec![], "all below the first bin");
    }

    #[test]
    fn automatic_bins_are_sensible() {
        let v = data::generate(data::Dist::Normal, 100, 2, 50.0, 10.0, 1);
        let w = auto_width(&v);
        let n = bins(&v, auto_start(&v, w, true), w, true).unwrap().len();
        assert!((4..=20).contains(&n), "{n} bins of {w}");
        assert_eq!(auto_width(&[5.0, 5.0]), 1.0, "no spread");
        assert_eq!(auto_width(&[]), 1.0);
    }

    #[test]
    fn heights_by_measure() {
        let b = vec![Bin { lo: 0.0, hi: 2.0, n: 1 }, Bin { lo: 2.0, hi: 4.0, n: 3 }];
        assert_eq!(measure_values(&b, "Frequency"), [1.0, 3.0]);
        assert_eq!(measure_values(&b, "Relative frequency"), [0.25, 0.75]);
        let dens = measure_values(&b, "Density");
        assert!((dens.iter().map(|d| d * 2.0).sum::<f64>() - 1.0).abs() < 1e-12, "density bars have area 1");
    }

    #[test]
    fn default_histogram_draws_one_bar_per_bin() {
        let p = defaults("histogram");
        let svg = histogram_render(&p);
        let values = data::numbers(p.s("data"));
        let w = auto_width(&values);
        let nb = bins(&values, auto_start(&values, w, true), w, true).unwrap().len();
        assert_eq!(svg.matches("<rect").count() - 1, nb, "one rect per bin, plus the page background");
    }

    #[test]
    fn student_copy_has_axes_but_no_bars_or_values() {
        let def = defs().remove(0);
        let mut p = defaults("histogram");
        p.set_b("values", true);
        let (student, key) = super::super::answer_pair(&def, &p).unwrap();
        let bars = |s: &str| s.matches("<rect").count();
        assert_eq!(bars(&student), 1, "only the page background");
        assert!(bars(&key) > 3);
        assert!(student.contains("Frequency"), "the axis title stays");
        assert!(key.matches("<text").count() > student.matches("<text").count(), "values are answers too");
    }

    #[test]
    fn print_style_uses_patterns_not_colour() {
        let mut p = defaults("histogram");
        p.set_s("style", "Print");
        let svg = histogram_render(&p);
        assert!(svg.contains("url(#vs-hatch0)") && !svg.contains("#4f7cd4"));
        assert!(super::super::to_png(&svg, 1.0).is_ok());
    }

    #[test]
    fn odd_input_never_panics_and_says_so() {
        let mut p = defaults("histogram");
        for text in ["", "abc", "5", "5, 5, 5", "-1e308, 1e308", "1, 2, 3, 1000000000", "0.001, 0.002"] {
            p.set_s("data", text);
            let svg = histogram_render(&p);
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?}");
        }
        p.set_s("data", "1, 2, 3");
        p.set_b("auto_bins", false);
        p.set_f("bin_start", 0.0);
        p.set_f("bin_width", 0.0001);
        assert!(histogram_render(&p).contains("Too many bars"));
        p.set_f("bin_width", 1.0);
        p.set_f("bin_start", 100.0);
        assert!(histogram_render(&p).contains("No data at or above"));
    }

    #[test]
    fn relevance_follows_the_automatic_switches() {
        let mut p = defaults("histogram");
        assert!(!relevant("histogram", "bin_width", &p) && !relevant("histogram", "y_max", &p));
        p.set_b("auto_bins", false);
        p.set_b("auto_y", false);
        assert!(relevant("histogram", "bin_width", &p) && relevant("histogram", "bin_start", &p) && relevant("histogram", "y_max", &p));
        assert!(relevant("scatterplot", "bin_width", &p), "other figures are untouched");
    }
}
