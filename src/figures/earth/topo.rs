//! Topographic contours from a small grid of heights: the heights are given at the grid points and
//! the contour lines, found by marching squares with linear interpolation, are the answer. The
//! student copy (Show answers off) shows the numbered grid only, ready for contours to be drawn.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_N: usize = 12;
const MAX_SPEC: usize = 2000;
const INK: &str = "#111827";
const DEFAULT_GRID: &str = "10 12 15 18 20 19 16\n12 18 25 32 34 30 22\n15 26 38 48 50 42 28\n17 30 46 62 66 52 30\n16 27 40 52 55 44 26\n13 20 28 34 36 30 20\n10 13 17 20 21 18 14";

pub fn def() -> FigureDef {
    FigureDef {
        id: "topo-contours",
        subject: Subject::EarthScience,
        title: "Topographic contours",
        blurb: "Contour lines drawn from a grid of heights at a chosen interval, with the highest point marked, or only the numbered grid for students to draw the contours.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "topo-contours" => vec![
            Example { name: "A hill", params: vec![] },
            Example { name: "A hill with a valley", params: vec![("grid", s("5 8 14 20 28\n9 16 26 34 40\n14 22 24 30 44\n20 24 16 28 46\n26 30 22 32 50")), ("interval", Value::F(10.0))] },
            Example { name: "A steep slope (close contours)", params: vec![("grid", s("0 10 40 90\n0 15 50 95\n0 20 60 100\n0 25 70 100")), ("interval", Value::F(20.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("grid", "Heights", DEFAULT_GRID).hint("One row per line, heights separated by spaces or commas; every row the same length, 2 to 12 across and down"),
        ParamDef::float("interval", "Contour interval", 0.001, 1.0e6, 1.0, 10.0),
        ParamDef::text("unit", "Unit of height", "m"),
        ParamDef::bool("labels", "Label the contour lines", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off shows the numbered grid only: no contour lines, labels or highest point"),
    ]
}

fn parse(spec: &str) -> Result<Vec<Vec<f64>>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The heights are limited to {MAX_SPEC} characters"));
    }
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut row = Vec::new();
        for tok in line.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()) {
            match tok.parse::<f64>() {
                Ok(v) if v.is_finite() && v.abs() <= 1.0e6 => row.push(v),
                _ => return Err(format!("\u{201c}{}\u{201d} is not a height", tok.chars().take(20).collect::<String>())),
            }
        }
        if row.len() > MAX_N {
            return Err(format!("A row is limited to {MAX_N} heights"));
        }
        rows.push(row);
        if rows.len() > MAX_N {
            return Err(format!("The grid is limited to {MAX_N} rows"));
        }
    }
    if rows.len() < 2 || rows[0].len() < 2 {
        return Err("Give at least a 2 by 2 grid of heights".into());
    }
    if rows.iter().any(|r| r.len() != rows[0].len()) {
        return Err("Every row needs the same number of heights".into());
    }
    Ok(rows)
}

/// The contour segments at `level` in grid units (column, row), found cell by cell.
pub fn segments(g: &[Vec<f64>], level: f64) -> Vec<((f64, f64), (f64, f64))> {
    let mut out = Vec::new();
    for r in 0..g.len() - 1 {
        for c in 0..g[0].len() - 1 {
            let (tl, tr, br, bl) = (g[r][c], g[r][c + 1], g[r + 1][c + 1], g[r + 1][c]);
            let (cx, cy) = (c as f64, r as f64);
            let t = |a: f64, b: f64| if (b - a).abs() < 1e-12 { 0.5 } else { ((level - a) / (b - a)).clamp(0.0, 1.0) };
            // crossing points on the top, right, bottom and left edges
            let top = (cx + t(tl, tr), cy);
            let right = (cx + 1.0, cy + t(tr, br));
            let bottom = (cx + t(bl, br), cy + 1.0);
            let left = (cx, cy + t(tl, bl));
            let code = usize::from(tl >= level) | usize::from(tr >= level) << 1 | usize::from(br >= level) << 2 | usize::from(bl >= level) << 3;
            let centre_high = (tl + tr + br + bl) / 4.0 >= level;
            let mut push = |a, b| out.push((a, b));
            match code {
                1 | 14 => push(left, top),
                2 | 13 => push(top, right),
                3 | 12 => push(left, right),
                4 | 11 => push(right, bottom),
                6 | 9 => push(top, bottom),
                7 | 8 => push(left, bottom),
                5 => {
                    if centre_high {
                        push(left, bottom);
                        push(top, right);
                    } else {
                        push(left, top);
                        push(bottom, right);
                    }
                }
                10 => {
                    if centre_high {
                        push(left, top);
                        push(bottom, right);
                    } else {
                        push(left, bottom);
                        push(top, right);
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn num(v: f64) -> String {
    chart::fmt((v * 100.0).round() / 100.0)
}

fn render(p: &Params) -> String {
    let g = match parse(p.s("grid")) {
        Ok(g) => g,
        Err(e) => return chart::message(&e),
    };
    let interval = p.f("interval");
    if !(interval.is_finite() && interval > 0.0 && interval <= 1.0e6) {
        return chart::message("The contour interval must be a number above 0");
    }
    let (rows, cols) = (g.len(), g[0].len());
    let (lo, hi) = g.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let k0 = (lo / interval).ceil() as i64;
    let k1 = (hi / interval).floor() as i64;
    if k1 - k0 > 60 {
        return chart::message("That interval would draw more than 60 contour lines: use a larger interval");
    }
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let unit: String = p.s("unit").trim().chars().take(6).collect();
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let cell = (520.0 / (cols.max(rows) as f64 - 1.0)).min(90.0);
    let (gw, gh) = (cell * (cols as f64 - 1.0), cell * (rows as f64 - 1.0));
    let (ox, oy) = (50.0, 36.0 + top);
    let (w, h) = (gw + 100.0, gh + 120.0 + top);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    d.rect(ox, oy, gw, gh, if print { "#fff" } else { "#f6f3ea" }, INK, 1.8);
    let px = |c: f64, r: f64| (ox + c * cell, oy + r * cell);
    // faint grid lines through the points
    for c in 0..cols {
        d.line(ox + c as f64 * cell, oy, ox + c as f64 * cell, oy + gh, "#d8d4c8", 0.8);
    }
    for r in 0..rows {
        d.line(ox, oy + r as f64 * cell, ox + gw, oy + r as f64 * cell, "#d8d4c8", 0.8);
    }
    let mut labels: Vec<((f64, f64), f64)> = Vec::new();
    if answers {
        for k in k0..=k1 {
            let level = k as f64 * interval;
            let segs = segments(&g, level);
            let major = k % 5 == 0;
            let colour = if print { "#111827" } else { "#8a4b1f" };
            for (a, b) in &segs {
                let (p0, p1) = (px(a.0, a.1), px(b.0, b.1));
                d.line(p0.0, p0.1, p1.0, p1.1, colour, if major { 2.6 } else { 1.6 });
            }
            if let Some((a, b)) = segs.get(segs.len() / 2) {
                let (p0, p1) = (px(a.0, a.1), px(b.0, b.1));
                labels.push((((p0.0 + p1.0) / 2.0, (p0.1 + p1.1) / 2.0), level));
            }
        }
        if p.b("labels") {
            for ((x, y), level) in &labels {
                let t = num(*level);
                let tw = t.chars().count() as f64 * fs * 0.62 + 8.0;
                d.rect(x - tw / 2.0, y - fs * 0.62, tw, fs * 1.25, if print { "#fff" } else { "#f6f3ea" }, "none", 0.0);
                chart::text(&mut d, *x, y + fs * 0.36, fs * 0.92, "middle", &t);
            }
        }
    }
    // the given heights, at every grid point
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = px(c as f64, r as f64);
            d.circle(x, y, 2.6, INK, INK, 0.5);
            chart::text(&mut d, x + 5.0, y - 5.0, fs * 0.82, "start", &num(g[r][c]));
        }
    }
    if answers {
        let mut best = (0, 0);
        for r in 0..rows {
            for c in 0..cols {
                if g[r][c] > g[best.0][best.1] {
                    best = (r, c);
                }
            }
        }
        let (x, y) = px(best.1 as f64, best.0 as f64);
        d.raw(&format!(r##"<path d="M{:.1},{:.1} l-9,14 l18,0Z" fill="#fff" stroke="{INK}" stroke-width="1.6"/>"##, x, y - 18.0));
        chart::text(&mut d, ox, oy + gh + 32.0, fs, "start", &format!("\u{25b3} Highest point: {} {unit} (row {}, column {}); contour interval {} {unit}", num(g[best.0][best.1]), best.0 + 1, best.1 + 1, num(interval)).replace("  ", " "));
    } else {
        chart::text(&mut d, ox, oy + gh + 32.0, fs, "start", &format!("Contour interval: {} {unit}", num(interval)));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn a_slope_gives_one_straight_contour() {
        // heights rise left to right: 0 10 / 0 10; the 5 contour is a vertical line down the middle
        let g = vec![vec![0.0, 10.0], vec![0.0, 10.0]];
        let s = segments(&g, 5.0);
        assert_eq!(s, vec![((0.5, 0.0), (0.5, 1.0))]);
        assert!(segments(&g, 20.0).is_empty() && segments(&g, -5.0).is_empty());
    }

    #[test]
    fn a_peak_gives_a_closed_loop() {
        let g = vec![vec![0.0, 0.0, 0.0], vec![0.0, 10.0, 0.0], vec![0.0, 0.0, 0.0]];
        let s = segments(&g, 5.0);
        assert_eq!(s.len(), 4, "one segment per cell round the peak");
        // every end meets another end, so the loop is closed
        for (a, b) in &s {
            assert_eq!(s.iter().filter(|(c, e)| c == a || e == a).count(), 2);
            assert_eq!(s.iter().filter(|(c, e)| c == b || e == b).count(), 2);
        }
    }

    #[test]
    fn a_saddle_keeps_both_diagonals_apart() {
        let g = vec![vec![10.0, 0.0], vec![0.0, 10.0]];
        assert_eq!(segments(&g, 5.0).len(), 2);
    }

    #[test]
    fn key_draws_contours_labels_and_the_summit() {
        let svg = render(&defaults());
        assert!(svg.contains("Highest point: 66 m (row 4, column 5)") && svg.contains(">50<") && svg.contains("<line"));
    }

    #[test]
    fn student_copy_has_the_grid_only() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">66<") && student.contains("Contour interval: 10 m") && !student.contains("Highest point"));
        assert!(student.matches("<line").count() < key.matches("<line").count() && !student.contains("#8a4b1f"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_grids_are_explained() {
        for (spec, needle) in [("", "2 by 2"), ("1 2", "2 by 2"), ("1 2\n3", "same number"), ("1 x\n3 4", "not a height"), ("1 2\n3 NaN", "not a height"), ("1 2\n3 1e9", "not a height")] {
            let mut p = defaults();
            p.set_s("grid", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let wide: String = (0..13).map(|i| format!("{i} ")).collect();
        let mut p = defaults();
        p.set_s("grid", &format!("{wide}\n{wide}"));
        assert!(render(&p).contains("limited to 12"));
        let mut p = defaults();
        p.set_f("interval", 0.0);
        assert!(render(&p).contains("above 0"));
        let mut p = defaults();
        p.set_f("interval", 0.01);
        assert!(render(&p).contains("more than 60"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("topo-contours").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "5 5\n5 5", "0 0\n0 1e6", "-5 -5\n-5 5", "\u{1f600} 1\n2 3"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("grid", spec);
                p.set_b(SHOW_ANSWERS, answers);
                assert!(sane(&render(&p)), "{spec:?}");
            }
        }
        for v in [f64::NAN, -1.0, 1.0e12, f64::INFINITY] {
            let mut p = defaults();
            p.set_f("interval", v);
            assert!(sane(&render(&p)));
        }
    }
}
