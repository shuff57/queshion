//! The Lorenz curve and Gini coefficient from the incomes of equal-sized groups of the
//! population (lowest to highest): a table of incomes with each group's share and the cumulative
//! shares, the curve against the line of equality, the area between them and the Gini coefficient.
//! The incomes are the question; the cumulative columns, the curve, the shading and the Gini are
//! the answers, so the student copy (Show answers off) leaves the cumulative cells empty and draws
//! only the axes and the line of equality, ready for the curve to be plotted.
use super::n2;
use crate::figures::chart::{self, AxesOpts, Frame, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_GROUPS: usize = 10;
const DEFAULT_INCOMES: &str = "2\n5\n9\n16\n30";

pub fn def() -> FigureDef {
    FigureDef {
        id: "lorenz",
        subject: Subject::Economics,
        title: "Lorenz curve and Gini coefficient",
        blurb: "A Lorenz curve and Gini coefficient from the incomes of equal-sized population groups, with the cumulative shares worked out, or the incomes and the line of equality only for students to plot.",
        params,
        render,
    }
}

pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "lorenz" => vec![
            Example { name: "Five quintiles", params: vec![] },
            Example { name: "Perfect equality", params: vec![("incomes", s("10\n10\n10\n10\n10"))] },
            Example { name: "Very unequal", params: vec![("incomes", s("1\n1\n2\n4\n92")), ("title", s("A very unequal economy"))] },
            Example { name: "Ten deciles", params: vec![("incomes", s("1 2 3 4 5 6 8 10 14 27"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("incomes", "Group incomes, lowest to highest", DEFAULT_INCOMES).hint("Numbers separated by spaces, commas or new lines, from 2 to 10 groups of equal size; they are put in order for you"),
        ParamDef::text("group", "What a group is called", "Quintile").hint("The table's first column: Quintile, Decile, Group"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the incomes and the line of equality and leaves the cumulative columns, the curve, the shading and the Gini empty for students"),
    ]
}

fn parse(spec: &str) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for tok in spec.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|t| !t.is_empty()) {
        match tok.parse::<f64>() {
            Ok(x) if x.is_finite() && (0.0..=1.0e12).contains(&x) => v.push(x),
            _ => return Err(format!("\u{201c}{}\u{201d} is not an income from 0 up", tok.chars().take(20).collect::<String>())),
        }
        if v.len() > MAX_GROUPS {
            return Err(format!("A figure is limited to {MAX_GROUPS} groups"));
        }
    }
    if v.len() < 2 {
        return Err("Give at least two group incomes".into());
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.iter().sum::<f64>() <= 0.0 {
        return Err("The incomes add up to 0: give some income".into());
    }
    Ok(v)
}

/// Cumulative income shares (0..1) of the groups, starting at 0.
pub fn cumulative(inc: &[f64]) -> Vec<f64> {
    let total: f64 = inc.iter().sum();
    let mut out = vec![0.0];
    let mut acc = 0.0;
    for x in inc {
        acc += x;
        out.push(acc / total);
    }
    out
}

/// The Gini coefficient: one minus the area under the Lorenz curve, doubled (trapezoids over equal-sized groups).
pub fn gini(inc: &[f64]) -> f64 {
    let c = cumulative(inc);
    let n = inc.len() as f64;
    1.0 - c.windows(2).map(|w| (w[0] + w[1]) / n).sum::<f64>()
}

fn render(p: &Params) -> String {
    let inc = match parse(p.s("incomes")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let n = inc.len();
    let cum = cumulative(&inc);
    let total: f64 = inc.iter().sum();
    let g = gini(&inc);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 13.0 * k;
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let title = p.s("title").trim();
    let group: String = p.s("group").trim().chars().take(12).collect();
    let group = if group.is_empty() { "Group".to_string() } else { group };
    let top = if title.is_empty() { 0.0 } else { 26.0 };
    let (w, ph) = (940.0, 440.0);
    let h = ph + top + 64.0;
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 20.0, fs * 1.25, "middle", title);
    }
    // the plot, a square on the left
    let f = Frame { w: 480.0, h: ph + top, l: 78.0, r: 22.0, t: 22.0 + top, b: 62.0, x0: 0.0, x1: 100.0, y0: 0.0, y1: 100.0 };
    let t10 = chart::ticks(0.0, 100.0, 20.0);
    if answers {
        let mut pts: Vec<(f64, f64)> = cum.iter().enumerate().map(|(i, c)| (f.px(100.0 * i as f64 / n as f64), f.py(c * 100.0))).collect();
        pts.push((f.px(100.0), f.py(100.0)));
        d.poly(&pts, &fills.series(0), "none", 0.0);
    }
    chart::axes(&mut d, &f, &AxesOpts { xticks: &t10, yticks: &t10, grid: true, x_title: "Cumulative % of population", y_title: "Cumulative % of income", fs });
    d.line(f.px(0.0), f.py(0.0), f.px(100.0), f.py(100.0), "#6b7280", 2.0);
    chart::text(&mut d, f.px(58.0), f.py(66.0), fs, "end", "Line of equality");
    if answers {
        let pts: Vec<(f64, f64)> = cum.iter().enumerate().map(|(i, c)| (f.px(100.0 * i as f64 / n as f64), f.py(c * 100.0))).collect();
        d.polyline(&pts, fills.line(1), 3.0);
        for (x, y) in &pts {
            d.circle(*x, *y, 3.8, "#111827", "#fff", 1.2);
        }
        chart::text(&mut d, f.px(62.0), f.py(14.0), fs * 1.05, "start", "Lorenz curve");
    }
    // the table on the right
    let cols = [72.0, 60.0, 70.0, 92.0, 100.0];
    let tx = 520.0;
    let tw: f64 = cols.iter().sum();
    let row = fs * 2.05;
    let ty = 34.0 + top;
    let xs: Vec<f64> = cols.iter().scan(tx, |a, c| { let v = *a; *a += c; Some(v) }).collect();
    d.rect(tx, ty, tw, row * 1.5, if fills.is_print() { "#e5e5e5" } else { "#e6edf8" }, INK, 1.6);
    for (i, t) in [group.as_str(), "Income", "Share %", "Cum. pop. %", "Cum. income %"].iter().enumerate() {
        let (lines, x) = if i == 0 { (vec![*t], xs[0] + 8.0) } else { (t.splitn(2, ' ').collect::<Vec<_>>(), xs[i] + cols[i] - 8.0) };
        let anchor = if i == 0 { "start" } else { "end" };
        for (j, l) in lines.iter().enumerate() {
            chart::text(&mut d, x, ty + row * 0.62 + j as f64 * fs * 1.1 - if lines.len() == 2 { fs * 0.35 } else { 0.0 }, fs * 0.85, anchor, l);
        }
    }
    let body = ty + row * 1.5;
    for (i, x) in inc.iter().enumerate() {
        let y = body + row * i as f64;
        chart::text(&mut d, xs[0] + 8.0, y + row * 0.68, fs, "start", &(i + 1).to_string());
        chart::text(&mut d, xs[1] + cols[1] - 8.0, y + row * 0.68, fs, "end", &n2(*x));
        if answers {
            chart::text(&mut d, xs[2] + cols[2] - 8.0, y + row * 0.68, fs, "end", &n2(x / total * 100.0));
            chart::text(&mut d, xs[3] + cols[3] - 8.0, y + row * 0.68, fs, "end", &n2(100.0 * (i + 1) as f64 / n as f64));
            chart::text(&mut d, xs[4] + cols[4] - 8.0, y + row * 0.68, fs, "end", &n2(cum[i + 1] * 100.0));
        }
        d.line(tx, y, tx + tw, y, "#c9ced6", 0.8);
    }
    let bottom = body + row * n as f64;
    chart::text(&mut d, xs[0] + 8.0, bottom + row * 0.68, fs, "start", "Total");
    chart::text(&mut d, xs[1] + cols[1] - 8.0, bottom + row * 0.68, fs, "end", &n2(total));
    for x in xs.iter().skip(1) {
        d.line(*x, ty, *x, bottom + row, INK, 1.2);
    }
    d.line(tx, bottom, tx + tw, bottom, INK, 1.6);
    d.rect(tx, ty, tw, bottom + row - ty, "none", INK, 1.8);
    let gy = bottom + row * 2.0;
    if answers {
        chart::text(&mut d, tx, gy, fs * 1.15, "start", &format!("Gini coefficient = {}", n2(g)));
    } else {
        chart::text(&mut d, tx, gy, fs * 1.15, "start", "Gini coefficient =");
        d.line(tx + fs * 9.8, gy + 2.0, tx + 190.0, gy + 2.0, INK, 1.0);
    }
    chart::text(&mut d, tx, gy + fs * 1.7, fs * 0.9, "start", "= area between the curves \u{f7} area under the line of equality");
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn cumulative_shares_end_at_one() {
        let c = cumulative(&[2.0, 5.0, 9.0, 16.0, 30.0]);
        assert_eq!(c.len(), 6);
        assert_eq!(c[0], 0.0);
        assert!((c[5] - 1.0).abs() < 1e-12);
        assert!((c[1] - 2.0 / 62.0).abs() < 1e-12);
    }

    #[test]
    fn gini_of_equal_incomes_is_zero_and_of_one_person_owning_all_approaches_the_top() {
        assert!(gini(&[10.0; 5]).abs() < 1e-12);
        assert!((gini(&[0.0, 0.0, 0.0, 0.0, 100.0]) - 0.8).abs() < 1e-12, "1 - 1/n for n groups");
        assert!((gini(&[0.0, 100.0]) - 0.5).abs() < 1e-12);
        // 2, 5, 9, 16, 30: the area under the curve is 0.28387, so the Gini is 1 - 2 * 0.28387
        assert!((gini(&[2.0, 5.0, 9.0, 16.0, 30.0]) - 0.43226).abs() < 1e-4);
    }

    #[test]
    fn key_shows_the_table_and_gini() {
        let svg = render(&defaults());
        assert!(svg.contains("Gini coefficient = 0.43"));
        assert!(svg.contains(">3.23<") && svg.contains(">100<") && svg.contains(">Lorenz curve<"));
    }

    #[test]
    fn incomes_are_put_in_order() {
        let a = parse("30 2 16 5 9").unwrap();
        assert_eq!(a, vec![2.0, 5.0, 9.0, 16.0, 30.0]);
        assert_eq!(parse("1,2;3").unwrap().len(), 3);
    }

    #[test]
    fn student_copy_has_the_equality_line_only() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Line of equality<") && student.contains(">Gini coefficient =<") && student.contains(">30<"));
        assert!(!student.contains("Lorenz curve") && !student.contains(">3.23<") && !student.contains("Gini coefficient = 0") && key.contains("Lorenz curve"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_incomes_are_explained() {
        for (spec, needle) in [("", "at least two"), ("5", "at least two"), ("5 x", "not an income"), ("-1 5", "not an income"), ("0 0", "add up to 0"), ("1 2 3 4 5 6 7 8 9 10 11", "limited to 10"), ("NaN 5", "not an income"), ("inf 5", "not an income")] {
            let mut p = defaults();
            p.set_s("incomes", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("lorenz").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "1e300 1e300", "0 1e12", "1e-300 1e-300", "\u{1f600}", "1 1 1 1 1 1 1 1 1 1"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("incomes", spec);
                p.set_b(SHOW_ANSWERS, answers);
                assert!(sane(&render(&p)), "{spec:?}");
            }
        }
    }
}
