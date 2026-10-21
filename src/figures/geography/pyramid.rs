//! A population pyramid from the numbers of males and females in each age band: male bars to the
//! left, female bars to the right, the age bands in the middle, with a table of the figures. The
//! table is the question; the bars and the summary (total, share of females, widest band and the
//! shape of the base) are the answers, so the student copy (Show answers off) draws only the
//! empty axes and leaves the summary as blank lines.
use super::{rows, trim};
use crate::figures::chart::{self};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const WIDTHS: [&str; 2] = ["5 years", "10 years"];
const MAX_BANDS: usize = 21;
const INK: &str = "#111827";
const DEFAULT: &str = "7.9, 7.5\n7.8, 7.4\n7.4, 7.0\n6.8, 6.6\n6.4, 6.3\n6.1, 6.0\n5.5, 5.6\n5.1, 5.3\n4.6, 4.8\n3.9, 4.2\n3.0, 3.5\n2.1, 2.7\n1.4, 2.1\n0.8, 1.5\n0.4, 1.0\n0.2, 0.6";

pub fn def() -> FigureDef {
    FigureDef {
        id: "population-pyramid",
        subject: Subject::Geography,
        title: "Population pyramid",
        blurb: "A population pyramid from male and female counts for each age band, with a table of the figures and a summary of its shape, or empty axes for students to draw the pyramid.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "population-pyramid" => vec![
            Example { name: "Young, growing population", params: vec![("data", s("9, 9\n8, 8\n7, 7\n6, 6\n5, 5\n4, 4\n3, 3\n2, 2\n1, 1.5\n0.5, 1")), ("unit", s("%"))] },
            Example { name: "Ageing population", params: vec![("data", s("3.1, 3\n3.3, 3.2\n3.6, 3.5\n3.8, 3.7\n4, 4\n4.2, 4.1\n4.3, 4.3\n4.2, 4.3\n4, 4.2\n3.6, 3.9\n3.1, 3.6\n2.6, 3.2\n2, 2.8\n1.4, 2.2\n0.8, 1.6\n0.3, 0.8"))] },
            Example { name: "Ten-year bands", params: vec![("width", s(WIDTHS[1])), ("data", s("9, 8.5\n8, 8\n7.5, 7.5\n6.5, 6.8\n5, 5.5\n3.5, 4.5\n2, 3.5\n0.8, 1.6"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("data", "Males and females per age band, youngest first", 2, DEFAULT).hint("One row per band: males then females, from 2 to 21 bands; the first row is the youngest band"),
        ParamDef::choice("width", "Age band width", &WIDTHS, WIDTHS[0]),
        ParamDef::text("unit", "Unit of the counts", "million").hint("Shown after the totals and in the axis titles, such as million or thousand"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the table and the empty axes and leaves the bars and the summary for students"),
    ]
}

/// The labels of `n` age bands of `bw` years, youngest first; the last is open-ended when `n` reaches 21.
pub fn labels(n: usize, bw: usize) -> Vec<String> {
    (0..n).map(|i| if i + 1 == MAX_BANDS && n == MAX_BANDS { format!("{}+", i * bw) } else { format!("{}\u{2013}{}", i * bw, i * bw + bw - 1) }).collect()
}

/// What the shape of the base says: compares the youngest band's share with an even spread.
pub fn shape(m: &[f64], f: &[f64]) -> &'static str {
    let tot: f64 = m.iter().chain(f).sum();
    if tot <= 0.0 {
        return "no population";
    }
    let base = (m[0] + f[0]) / tot * m.len() as f64;
    if base > 1.25 {
        "wide base: a young, growing population"
    } else if base < 0.75 {
        "narrow base: an ageing, shrinking population"
    } else {
        "even sides: a stable population"
    }
}

fn render(p: &Params) -> String {
    let data = match rows(p.s("data"), 2, 2, MAX_BANDS, 0.0, 1.0e12, "a count from 0") {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let m: Vec<f64> = data.iter().map(|r| r[0]).collect();
    let f: Vec<f64> = data.iter().map(|r| r[1]).collect();
    let n = data.len();
    let tot: f64 = m.iter().chain(&f).sum();
    if tot <= 0.0 {
        return chart::message("The counts add up to 0: give some population");
    }
    let bw = if p.s("width") == WIDTHS[1] { 10 } else { 5 };
    let names = labels(n, bw);
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let unit: String = p.s("unit").trim().chars().take(40).collect();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let rowh = 22.0;
    let ph = n as f64 * rowh;
    let (w, h) = (980.0, top + 30.0 + ph + 70.0 + 130.0);
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let maxv = m.iter().chain(&f).cloned().fold(0.0, f64::max);
    let (_, hi, step) = chart::axis_range(0.0, maxv, 4.0);
    let hi = hi.max(step);
    let (half, gap, x0, y0) = (270.0, 64.0, 24.0, top + 40.0);
    let cx = x0 + half + gap / 2.0;
    let vx = |v: f64, left: bool| if left { cx - gap / 2.0 - v / hi * half } else { cx + gap / 2.0 + v / hi * half };
    // gridlines and ticks
    let mut t = 0.0;
    while t <= hi + 1e-9 {
        for left in [true, false] {
            let x = vx(t, left);
            d.line(x, y0, x, y0 + ph, chart::GRID, 1.0);
            chart::text(&mut d, x, y0 + ph + fs * 1.5, fs, "middle", &chart::fmt(t));
        }
        t += step;
    }
    d.rect(x0, y0, 2.0 * half + gap, ph, "none", INK, 1.5);
    chart::text(&mut d, vx(hi / 2.0, true), y0 + ph + fs * 3.2, fs * 1.05, "middle", &format!("Males ({unit})"));
    chart::text(&mut d, vx(hi / 2.0, false), y0 + ph + fs * 3.2, fs * 1.05, "middle", &format!("Females ({unit})"));
    for (i, name) in names.iter().enumerate() {
        let y = y0 + ph - (i as f64 + 1.0) * rowh;
        chart::text(&mut d, cx, y + rowh / 2.0 + fs * 0.35, fs * 0.9, "middle", name);
        if answers {
            for (left, v, k) in [(true, m[i], 0), (false, f[i], 1)] {
                let (a, b) = (vx(0.0, left), vx(v, left));
                d.rect(a.min(b), y + 2.0, (a - b).abs(), rowh - 4.0, &fills.series(k), INK, 1.0);
            }
        }
    }
    // the table, youngest band at the bottom to match the pyramid
    let tx = x0 + 2.0 * half + gap + 40.0;
    let cols = [70.0, 80.0, 80.0];
    let hy = y0 - rowh / 2.0 + fs * 0.35;
    d.rect(tx, y0 - rowh, cols.iter().sum(), rowh, "none", INK, 1.0);
    for (c, head) in ["Age", "Males", "Females"].iter().enumerate() {
        chart::text(&mut d, tx + cols[..c].iter().sum::<f64>() + cols[c] / 2.0, hy, fs, "middle", head);
    }
    for i in 0..n {
        let y = y0 + ph - (i as f64 + 1.0) * rowh;
        d.rect(tx, y, cols.iter().sum(), rowh, "none", "#9ca3af", 0.8);
        for (c, text) in [names[i].clone(), trim(m[i], 3), trim(f[i], 3)].iter().enumerate() {
            chart::text(&mut d, tx + cols[..c].iter().sum::<f64>() + cols[c] / 2.0, y + rowh / 2.0 + fs * 0.35, fs * 0.9, "middle", text);
        }
    }
    // the summary
    let mut y = y0 + ph + fs * 5.4 + 20.0;
    let widest = (0..n).max_by(|a, b| (m[*a] + f[*a]).partial_cmp(&(m[*b] + f[*b])).unwrap()).unwrap();
    let mut line = |d: &mut Doc, label: &str, val: &str| {
        chart::text(d, x0, y, fs, "start", label);
        if answers {
            chart::text(d, x0 + 270.0, y, fs, "start", val);
        } else {
            d.line(x0 + 270.0, y + 3.0, x0 + 560.0, y + 3.0, INK, 1.0);
        }
        y += fs * 1.9;
    };
    line(&mut d, "Total population:", &format!("{} {}", trim(tot, 3), unit));
    line(&mut d, "Share who are female:", &format!("{}%", trim(f.iter().sum::<f64>() / tot * 100.0, 1)));
    line(&mut d, "Largest age band:", &format!("{} ({}%)", names[widest], trim((m[widest] + f[widest]) / tot * 100.0, 1)));
    line(&mut d, "Shape of the pyramid:", shape(&m, &f));
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn labels_run_from_zero_and_the_last_of_21_is_open() {
        assert_eq!(labels(3, 5), vec!["0\u{2013}4", "5\u{2013}9", "10\u{2013}14"]);
        assert_eq!(labels(2, 10)[1], "10\u{2013}19");
        assert_eq!(labels(21, 5)[20], "100+");
    }

    #[test]
    fn the_base_says_the_shape() {
        assert!(shape(&[9.0, 5.0, 1.0], &[9.0, 5.0, 1.0]).starts_with("wide"));
        assert!(shape(&[1.0, 5.0, 9.0], &[1.0, 5.0, 9.0]).starts_with("narrow"));
        assert!(shape(&[3.0, 3.0, 3.0], &[3.0, 3.0, 3.0]).starts_with("even"));
        assert_eq!(shape(&[0.0, 0.0], &[0.0, 0.0]), "no population");
    }

    #[test]
    fn key_writes_the_summary() {
        let svg = render(&defaults());
        assert!(svg.contains("Total population:") && svg.contains("Shape of the pyramid:") && svg.contains("0\u{2013}4") && svg.contains("Males (million)"));
        assert!(svg.contains("wide base") || svg.contains("even sides") || svg.contains("narrow base"));
    }

    #[test]
    fn student_copy_has_the_table_but_no_bars() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">7.9<") && !student.contains("base") && !student.contains("even sides") && student.contains(">Share who are female:<") && !student.contains("Million"));
        assert!(student.matches("<rect").count() < key.matches("<rect").count());
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_data_is_explained() {
        for (spec, needle) in [("", "at least") , ("1, 2", "2 rows"), ("a, b", "not a count"), ("0, 0\n0, 0", "add up to 0"), ("-1, 2\n1, 1", "not a count")] {
            let mut p = defaults();
            p.set_s("data", spec);
            let svg = render(&p);
            assert!(svg.contains(needle) || (needle == "at least" && svg.contains("Give")), "{spec:?}");
        }
        let mut p = defaults();
        p.set_s("data", &"1, 1\n".repeat(22));
        assert!(render(&p).contains("limited to 21"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("population-pyramid").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("limited"), "{}", ex.name);
        }
        for spec in ["nan, 1\n1, 1", "1e999, 1\n1, 1", "<b>, 1\n1, 1", "1e11, 1e11\n1, 1", "0.0001, 0.0002\n0, 0", "\u{0}"] {
            for unit in ["", "<script>", &"x".repeat(5000)] {
                let mut p = defaults();
                p.set_s("data", spec);
                p.set_s("unit", unit);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<script>"), "{spec:?}");
            }
        }
    }
}
