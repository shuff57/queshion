//! A chi-square test of independence worked in tables: the observed counts with their totals, the
//! expected counts (row total × column total ÷ grand total), each cell's contribution
//! (observed − expected)² ÷ expected, and the χ², degrees of freedom, critical value, p-value and
//! decision. The observed table is the question; every other number is an answer, so the student
//! copy (Show answers off) is the same set of tables with empty cells.
use super::dist::{chisq_cdf, Dist};
use super::normal::prob_text;
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_SIDE: usize = 5;
const MAX_COUNT: f64 = 1.0e6;

pub fn def() -> FigureDef {
    FigureDef {
        id: "chi-square-table",
        subject: Subject::Math,
        title: "Chi-Square Table",
        blurb: "A chi-square test of independence worked in tables: observed counts, expected counts, each cell's contribution, then the statistic, degrees of freedom, p-value and decision, or empty cells for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "chi-square-table" => vec![
            Example { name: "Two by three", params: vec![] },
            Example { name: "Two by two", params: vec![("observed", s("45 15\n30 30")), ("row_names", s("Treated, Untreated")), ("col_names", s("Better, Not better"))] },
            Example { name: "Independent (no association)", params: vec![("observed", s("20 30\n40 60"))] },
            Example { name: "Three by three", params: vec![("observed", s("30 14 6\n22 25 13\n8 21 31")), ("row_names", s("Grade 9, Grade 10, Grade 11")), ("col_names", s("Bus, Walk, Car"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("observed", "Observed counts: one row per line", "30 20 10\n20 25 35").hint("Whole numbers in a grid of 2 to 5 rows and 2 to 5 columns; a space or comma between numbers"),
        ParamDef::text("row_names", "Row names (comma separated)", "Group A, Group B"),
        ParamDef::text("col_names", "Column names (comma separated)", "Agree, Unsure, Disagree"),
        ParamDef::float("alpha", "Significance level (α)", 0.001, 0.5, 0.005, 0.05),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the observed table and leaves the expected counts, contributions, χ², p-value and decision empty for students"),
    ]
}

pub fn parse(spec: &str) -> Result<Vec<Vec<f64>>, String> {
    if spec.chars().count() > 300 {
        return Err("The counts are limited to 300 characters".into());
    }
    let mut g: Vec<Vec<f64>> = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let row: Result<Vec<f64>, String> = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .map(|t| t.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0 && *v <= MAX_COUNT && v.fract() == 0.0).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a whole count from 0", t.chars().take(12).collect::<String>())))
            .collect();
        g.push(row?);
        if g.len() > MAX_SIDE {
            return Err(format!("The table is limited to {MAX_SIDE} rows"));
        }
    }
    if g.len() < 2 {
        return Err("Give at least two rows of counts".into());
    }
    let cols = g[0].len();
    if cols < 2 {
        return Err("Give at least two columns of counts".into());
    }
    if cols > MAX_SIDE {
        return Err(format!("The table is limited to {MAX_SIDE} columns"));
    }
    if g.iter().any(|r| r.len() != cols) {
        return Err("Every row needs the same number of counts".into());
    }
    Ok(g)
}

pub struct Worked {
    pub row_totals: Vec<f64>,
    pub col_totals: Vec<f64>,
    pub total: f64,
    pub expected: Vec<Vec<f64>>,
    pub contrib: Vec<Vec<f64>>,
    pub chi2: f64,
    pub df: f64,
}

/// The working of the test; `None` when a row or column has no counts (the expected counts would
/// be zero).
pub fn work(g: &[Vec<f64>]) -> Option<Worked> {
    let row_totals: Vec<f64> = g.iter().map(|r| r.iter().sum()).collect();
    let col_totals: Vec<f64> = (0..g[0].len()).map(|c| g.iter().map(|r| r[c]).sum()).collect();
    let total: f64 = row_totals.iter().sum();
    if row_totals.iter().chain(col_totals.iter()).any(|t| *t == 0.0) {
        return None;
    }
    let expected: Vec<Vec<f64>> = row_totals.iter().map(|r| col_totals.iter().map(|c| r * c / total).collect()).collect();
    let contrib: Vec<Vec<f64>> = g.iter().zip(&expected).map(|(o, e)| o.iter().zip(e).map(|(o, e)| (o - e) * (o - e) / e).collect()).collect();
    let chi2 = contrib.iter().flatten().sum();
    let df = ((g.len() - 1) * (g[0].len() - 1)) as f64;
    Some(Worked { row_totals, col_totals, total, expected, contrib, chi2, df })
}

/// P(χ² ≥ x) for `df` degrees of freedom.
pub fn p_value(chi2: f64, df: f64) -> f64 {
    (1.0 - chisq_cdf(chi2, df)).clamp(0.0, 1.0)
}

fn names(s: &str, n: usize, fallback: &str, len: usize) -> Vec<String> {
    let given: Vec<String> = s.split(',').map(|t| t.trim().chars().take(len).collect::<String>()).collect();
    (0..n).map(|i| given.get(i).filter(|t| !t.is_empty()).cloned().unwrap_or_else(|| format!("{fallback} {}", i + 1))).collect()
}

fn render(p: &Params) -> String {
    let g = match parse(p.s("observed")) {
        Ok(g) => g,
        Err(e) => return chart::message(&e),
    };
    let Some(r) = work(&g) else {
        return chart::message("Every row and every column needs at least one count");
    };
    let alpha = p.f("alpha");
    if !alpha.is_finite() {
        return chart::message("Enter the significance level as a number");
    }
    let alpha = alpha.clamp(0.001, 0.5);
    let (nr, nc) = (g.len(), g[0].len());
    let rows = names(p.s("row_names"), nr, "Row", 14);
    let cols = names(p.s("col_names"), nc, "Column", 12);
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 32.0 };
    let (lw, cw, ch) = (96.0, 70.0, 32.0);
    let obs_w = lw + (nc as f64 + 1.0) * cw;
    let exp_w = lw + nc as f64 * cw;
    let table_h = (nr as f64 + 2.0) * ch;
    let (x1, x2) = (24.0, 24.0 + obs_w + 36.0);
    let w = (x2 + exp_w + 24.0).max(x2 + 570.0);
    let row1 = top + 40.0;
    let row2 = row1 + table_h + 74.0;
    let h = row2 + table_h + 50.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let head = if print { "#e2e2e2" } else { "#dbe7fb" };
    let cell = |d: &mut Doc, x: f64, y: f64, wd: f64, fill: &str, text: &str, bold: bool| {
        d.rect(x, y, wd, ch, fill, INK, 1.4);
        if !text.is_empty() {
            chart::text(d, x + wd / 2.0, y + ch / 2.0 + fs * 0.35, if bold { fs } else { fs * 0.95 }, "middle", text);
        }
    };
    let two = |v: f64| format!("{v:.2}");
    // a table of rows x cols cells with headers, optionally with a totals row and column
    let table = |d: &mut Doc, x: f64, y: f64, label: &str, totals: bool, body: &dyn Fn(usize, usize) -> String, row_t: &dyn Fn(usize) -> String, col_t: &dyn Fn(usize) -> String, grand: &str| {
        chart::text(d, x, y - 12.0, fs * 1.05, "start", label);
        cell(d, x, y, lw, head, "", false);
        for (j, c) in cols.iter().enumerate() {
            cell(d, x + lw + j as f64 * cw, y, cw, head, c, true);
        }
        if totals {
            cell(d, x + lw + nc as f64 * cw, y, cw, head, "Total", true);
        }
        for (i, rn) in rows.iter().enumerate() {
            let yy = y + ch * (i as f64 + 1.0);
            cell(d, x, yy, lw, head, rn, true);
            for j in 0..nc {
                cell(d, x + lw + j as f64 * cw, yy, cw, "#fff", &body(i, j), false);
            }
            if totals {
                cell(d, x + lw + nc as f64 * cw, yy, cw, head, &row_t(i), true);
            }
        }
        if totals {
            let yy = y + ch * (nr as f64 + 1.0);
            cell(d, x, yy, lw, head, "Total", true);
            for j in 0..nc {
                cell(d, x + lw + j as f64 * cw, yy, cw, head, &col_t(j), true);
            }
            cell(d, x + lw + nc as f64 * cw, yy, cw, head, grand, true);
        }
    };
    let n0 = |v: f64| chart::fmt(v);
    table(&mut d, x1, row1, "Observed counts", true, &|i, j| n0(g[i][j]), &|i| n0(r.row_totals[i]), &|j| n0(r.col_totals[j]), &n0(r.total));
    table(&mut d, x2, row1, "Expected counts", false, &|i, j| if answers { two(r.expected[i][j]) } else { String::new() }, &|_| String::new(), &|_| String::new(), "");
    chart::text(&mut d, x2, row1 + (nr as f64 + 1.0) * ch + 22.0, fs * 0.85, "start", "row total × column total ÷ grand total");
    table(&mut d, x1, row2, "Contribution of each cell", false, &|i, j| if answers { two(r.contrib[i][j]) } else { String::new() }, &|_| String::new(), &|_| String::new(), "");
    chart::text(&mut d, x1, row2 + (nr as f64 + 1.0) * ch + 22.0, fs * 0.85, "start", "(observed − expected)² ÷ expected");

    // the results
    let dist = Dist::Chi(r.df);
    let crit = dist.quantile(1.0 - alpha);
    let pv = p_value(r.chi2, r.df);
    let reject = pv < alpha;
    let mut items: Vec<(String, String)> = vec![
        ("χ² = sum of contributions".into(), two(r.chi2)),
        ("Degrees of freedom (r − 1)(c − 1)".into(), n0(r.df)),
        (format!("Critical value at α = {}", chart::fmt(alpha)), two(crit)),
        ("p-value".into(), prob_text(pv)),
        ("Decision".into(), if reject { "Reject H0: the variables are associated".into() } else { "Fail to reject H0: no evidence of an association".into() }),
    ];
    let min_e = r.expected.iter().flatten().cloned().fold(f64::INFINITY, f64::min);
    if answers && min_e < 5.0 {
        items.push(("Check".into(), format!("smallest expected count is {} (below 5)", two(min_e))));
    }
    chart::text(&mut d, x2, row2 - 12.0, fs * 1.05, "start", "Results");
    let mut y = row2 + 12.0;
    for (label, value) in &items {
        chart::text(&mut d, x2, y + fs * 0.35, fs * 0.95, "start", label);
        let vx = x2 + 250.0;
        if answers || label == "Check" {
            chart::text(&mut d, vx, y + fs * 0.35, fs * 0.95, "start", value);
        } else {
            d.line(vx, y + fs * 0.35 + 3.0, vx + 120.0, y + fs * 0.35 + 3.0, INK, 1.0);
        }
        y += fs * 2.0 + 2.0;
    }
    let _ = (nr, nc);
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn grid_parses() {
        assert_eq!(parse("1 2\n3,4").unwrap(), vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        for (spec, needle) in [("", "at least two rows"), ("1 2", "at least two rows"), ("1\n2", "at least two columns"), ("1 2\n3", "same number"), ("1 x\n2 3", "not a whole count"), ("-1 2\n3 4", "not a whole count"), ("1.5 2\n3 4", "not a whole count"), ("1e9 1\n1 1", "not a whole count"), ("nan 1\n1 1", "not a whole count")] {
            assert!(parse(spec).unwrap_err().contains(needle), "{spec:?}");
        }
        assert!(parse(&"1 1\n".repeat(6)).unwrap_err().contains("5 rows"));
        assert!(parse("1 1 1 1 1 1\n1 1 1 1 1 1").unwrap_err().contains("5 columns"));
        assert!(parse(&"1 ".repeat(200)).unwrap_err().contains("300"));
    }

    #[test]
    fn the_working_matches_a_known_example() {
        // 2 x 2: 20 30 / 40 60 is exactly independent
        let r = work(&parse("20 30\n40 60").unwrap()).unwrap();
        assert!(r.chi2.abs() < 1e-12 && r.df == 1.0);
        assert_eq!((r.row_totals.clone(), r.col_totals.clone(), r.total), (vec![50.0, 100.0], vec![60.0, 90.0], 150.0));
        assert_eq!(r.expected[0][0], 20.0);
        // 2 x 3: 30 20 10 / 20 25 35 (N = 140): E = 21.43 19.29 19.29 / 28.57 25.71 25.71, chi2 = 13.87
        let r = work(&parse("30 20 10\n20 25 35").unwrap()).unwrap();
        assert!((r.chi2 - 13.870).abs() < 0.001, "{}", r.chi2);
        assert_eq!(r.df, 2.0);
        // a known textbook value: 45 15 / 30 30, chi2 = 8.0 (N = 120)
        let r = work(&parse("45 15\n30 30").unwrap()).unwrap();
        assert!((r.chi2 - 8.0).abs() < 1e-9 || (r.chi2 - 8.0).abs() < 0.05, "{}", r.chi2);
        assert!(work(&parse("0 0\n1 2").unwrap()).is_none());
        assert!(work(&parse("0 1\n0 2").unwrap()).is_none());
    }

    #[test]
    fn p_values_match_the_table() {
        assert!((p_value(3.8415, 1.0) - 0.05).abs() < 1e-4);
        assert!((p_value(5.9915, 2.0) - 0.05).abs() < 1e-4);
        assert_eq!(p_value(0.0, 3.0), 1.0);
    }

    #[test]
    fn key_has_the_numbers_student_does_not() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains(">21.43<") && key.contains("Reject H0") && key.contains("Critical value at α = 0.05"));
        assert!(student.contains(">Observed counts<") && student.contains(">140<") && !student.contains(">21.43<") && !student.contains("Reject H0"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn small_expected_counts_are_flagged_in_the_key() {
        let mut p = defaults();
        p.set_s("observed", "1 2\n3 1");
        assert!(render(&p).contains("below 5"));
        p.set_s("observed", "50 60\n70 80");
        assert!(!render(&p).contains("below 5"));
    }

    #[test]
    fn bad_input_is_explained() {
        let mut p = defaults();
        p.set_s("observed", "0 0\n1 2");
        assert!(render(&p).contains("at least one count"));
        p.set_s("observed", "x");
        assert!(sane(&render(&p)));
        p.set_s("observed", "1 2\n3 4");
        p.set_s("row_names", ",,,");
        p.set_s("col_names", "");
        assert!(render(&p).contains("Row 1") && render(&p).contains("Column 2"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("chi-square-table").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("at least") && !svg.contains("not a whole"), "{}", ex.name);
        }
        for v in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300] {
            let mut p = defaults();
            p.set_f("alpha", v);
            assert!(sane(&render(&p)), "{v}");
        }
        for spec in ["\u{0} 1\n1 1", "<b> 1\n1 1", "1000000 1\n1 1000000", "0 1\n1 0", &"9 ".repeat(5), "5 5 5 5 5\n5 5 5 5 5\n5 5 5 5 5\n5 5 5 5 5\n5 5 5 5 5"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("observed", spec);
                p.set_s("row_names", "<b>&");
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
