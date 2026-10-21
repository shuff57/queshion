//! A map grid with numbered lines and some features placed on it, for four- and six-figure grid
//! references and compass bearings. The positions are the question; the references and the
//! bearings are the answers, so the student copy (Show answers off) draws the same map and
//! leaves the table cells as empty boxes.
use super::trim;
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_FEATURES: usize = 8;
const MAX_BEARINGS: usize = 4;
const INK: &str = "#111827";
const CELL: f64 = 46.0;
const DEFAULT: &str = "Church 2.4 6.7\nBridge 5.5 3.2\nSchool 7.3 6.1\nFarm 3.6 1.3\nStation 6.4 4.8";

pub fn def() -> FigureDef {
    FigureDef {
        id: "grid-reference",
        subject: Subject::Geography,
        title: "Grid references and bearings",
        blurb: "A map grid with numbered lines and features, with four- and six-figure grid references and compass bearings between features worked out, or empty boxes for students to fill in.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "grid-reference" => vec![
            Example { name: "Eight by eight", params: vec![] },
            Example { name: "Bigger map, higher line numbers", params: vec![("cols", Value::I(10)), ("rows", Value::I(9)), ("origin", Value::I(84)), ("features", s("Lighthouse 1.2 8.4\nHarbour 3.8 6.1\nWindmill 6.5 2.7\nCastle 8.9 7.3\nCampsite 4.4 0.9\nQuarry 9.2 1.6")), ("bearings", s("1 2\n3 4\n5 6\n2 3"))] },
            Example { name: "References only", params: vec![("bearings", s(""))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("features", "Features: name, then easting and northing", DEFAULT).hint("One per line: a name, then two numbers counted in grid squares from the bottom-left corner (3.4 is four tenths along the fourth square); up to 8"),
        ParamDef::long("bearings", "Bearings to find, from feature number to feature number", "1 3\n4 2").hint("One pair per line, such as 1 3 for the bearing from the first feature to the third; up to 4; leave empty for none"),
        ParamDef::int("cols", "Squares across", 4, 12, 8),
        ParamDef::int("rows", "Squares up", 4, 12, 8),
        ParamDef::int("origin", "Number of the first grid line", 0, 99, 10).hint("Both axes start at this number and wrap after 99, as on a real map"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the map and leaves the grid references and bearings as empty boxes"),
    ]
}

pub struct Feature {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

fn parse(spec: &str, cols: f64, rows: f64) -> Result<Vec<Feature>, String> {
    let mut v = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.len() < 3 {
            return Err(format!("\u{201c}{}\u{201d} needs a name and two numbers", line.chars().take(24).collect::<String>()));
        }
        let (xs, ys) = (toks[toks.len() - 2], toks[toks.len() - 1]);
        let (x, y) = match (xs.parse::<f64>(), ys.parse::<f64>()) {
            (Ok(x), Ok(y)) if x.is_finite() && y.is_finite() => (x, y),
            _ => return Err(format!("\u{201c}{}\u{201d} needs two numbers after the name", line.chars().take(24).collect::<String>())),
        };
        if !(0.0..cols).contains(&x) || !(0.0..rows).contains(&y) {
            return Err(format!("{} is off the map: use 0 up to {cols} across and 0 up to {rows} up", toks[..toks.len() - 2].join(" ").chars().take(20).collect::<String>()));
        }
        v.push(Feature { name: toks[..toks.len() - 2].join(" ").chars().take(16).collect(), x, y });
        if v.len() > MAX_FEATURES {
            return Err(format!("A figure is limited to {MAX_FEATURES} features"));
        }
    }
    if v.is_empty() {
        return Err("Give at least one feature, such as Church 2.4 6.7".into());
    }
    Ok(v)
}

fn pairs(spec: &str, n: usize) -> Result<Vec<(usize, usize)>, String> {
    let mut v = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let t: Vec<&str> = line.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).collect();
        let ok = |s: &str| s.parse::<usize>().ok().filter(|k| (1..=n).contains(k));
        match (t.as_slice(), t.first().and_then(|s| ok(s)), t.get(1).and_then(|s| ok(s))) {
            ([_, _], Some(a), Some(b)) => v.push((a - 1, b - 1)),
            _ => return Err(format!("\u{201c}{}\u{201d} is not a pair of feature numbers from 1 to {n}", line.chars().take(20).collect::<String>())),
        }
        if v.len() > MAX_BEARINGS {
            return Err(format!("A figure is limited to {MAX_BEARINGS} bearings"));
        }
    }
    Ok(v)
}

/// Two-digit grid line number of the square containing `v`, counting from `origin` and wrapping after 99.
pub fn line_no(origin: i64, v: f64) -> i64 {
    (origin + v.floor() as i64).rem_euclid(100)
}

pub fn four_figure(origin: i64, f: &Feature) -> String {
    format!("{:02}{:02}", line_no(origin, f.x), line_no(origin, f.y))
}

pub fn six_figure(origin: i64, f: &Feature) -> String {
    let tenth = |v: f64| ((v - v.floor()) * 10.0 + 1e-9).floor().min(9.0) as i64;
    format!("{:02}{} {:02}{}", line_no(origin, f.x), tenth(f.x), line_no(origin, f.y), tenth(f.y))
}

/// The compass bearing from `a` to `b` in whole degrees, clockwise from north (0 to 359); None when they coincide.
pub fn bearing(a: &Feature, b: &Feature) -> Option<i64> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return None;
    }
    Some((dx.atan2(dy).to_degrees().rem_euclid(360.0).round() as i64) % 360)
}

fn render(p: &Params) -> String {
    let (cols, rows) = (p.i("cols").clamp(4, 12), p.i("rows").clamp(4, 12));
    let origin = p.i("origin").clamp(0, 99);
    let feats = match parse(p.s("features"), cols as f64, rows as f64) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let br = match pairs(p.s("bearings"), feats.len()) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (mw, mh) = (cols as f64 * CELL, rows as f64 * CELL);
    let (mx, my) = (56.0, top + 30.0);
    let tx = mx + mw + 70.0;
    let table_h = 40.0 + (feats.len() as f64 + 1.0) * 30.0 + if br.is_empty() { 0.0 } else { 60.0 + (br.len() as f64 + 1.0) * 30.0 };
    let (w, h) = (tx + 450.0, (my + mh + 50.0).max(my + table_h + 20.0));
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    d.rect(mx, my, mw, mh, if print { "#f2f2f2" } else { "#f6f3e8" }, INK, 1.8);
    for i in 0..=cols {
        let x = mx + i as f64 * CELL;
        d.line(x, my, x, my + mh, "#9ca3af", 1.0);
        chart::text(&mut d, x, my - 8.0, fs * 0.9, "middle", &format!("{:02}", line_no(origin, i as f64)));
        chart::text(&mut d, x, my + mh + fs * 1.5, fs * 0.9, "middle", &format!("{:02}", line_no(origin, i as f64)));
    }
    for j in 0..=rows {
        let y = my + mh - j as f64 * CELL;
        d.line(mx, y, mx + mw, y, "#9ca3af", 1.0);
        chart::text(&mut d, mx - 8.0, y + fs * 0.3, fs * 0.9, "end", &format!("{:02}", line_no(origin, j as f64)));
        chart::text(&mut d, mx + mw + 8.0, y + fs * 0.3, fs * 0.9, "start", &format!("{:02}", line_no(origin, j as f64)));
    }
    let pos = |f: &Feature| (mx + f.x * CELL, my + mh - f.y * CELL);
    for (i, f) in feats.iter().enumerate() {
        let (x, y) = pos(f);
        d.circle(x, y, 3.0, INK, INK, 1.0);
        d.circle(x + 12.0, y - 12.0, 11.0, "#fff", INK, 1.6);
        chart::text(&mut d, x + 12.0, y - 12.0 + fs * 0.35, fs * 0.95, "middle", &(i + 1).to_string());
    }
    // the north arrow
    let (nx, ny) = (w - 24.0, my + 56.0);
    d.arrow(nx, ny, nx, my - 4.0, INK, 3.0);
    chart::text(&mut d, nx, ny + 18.0, fs, "middle", "N");
    // the feature table
    let cw = [34.0, 150.0, 90.0, 110.0];
    let mut y = my + 4.0;
    let sum = |c: usize| cw[..c].iter().sum::<f64>();
    d.rect(tx, y - 20.0, cw.iter().sum(), 30.0, "none", INK, 1.2);
    for (c, hd) in ["", "Feature", "4-figure", "6-figure"].iter().enumerate() {
        chart::text(&mut d, tx + sum(c) + cw[c] / 2.0, y, fs, "middle", hd);
    }
    for (i, f) in feats.iter().enumerate() {
        y += 30.0;
        d.rect(tx, y - 20.0, cw.iter().sum(), 30.0, "none", INK, 1.0);
        chart::text(&mut d, tx + cw[0] / 2.0, y, fs, "middle", &(i + 1).to_string());
        chart::text(&mut d, tx + sum(1) + 8.0, y, fs, "start", &f.name);
        for (c, val) in [(2, four_figure(origin, f)), (3, six_figure(origin, f))] {
            if answers {
                chart::text(&mut d, tx + sum(c) + cw[c] / 2.0, y, fs, "middle", &val);
            }
        }
    }
    for c in 1..4 {
        d.line(tx + sum(c), my - 16.0, tx + sum(c), y + 10.0, INK, 1.0);
    }
    if !br.is_empty() {
        y += 60.0;
        let bw = [180.0, 120.0];
        d.rect(tx, y - 20.0, bw[0] + bw[1], 30.0, "none", INK, 1.2);
        chart::text(&mut d, tx + bw[0] / 2.0, y, fs, "middle", "Bearing from \u{2026} to \u{2026}");
        chart::text(&mut d, tx + bw[0] + bw[1] / 2.0, y, fs, "middle", "Bearing");
        for (a, b) in &br {
            y += 30.0;
            d.rect(tx, y - 20.0, bw[0] + bw[1], 30.0, "none", INK, 1.0);
            d.line(tx + bw[0], y - 20.0, tx + bw[0], y + 10.0, INK, 1.0);
            chart::text(&mut d, tx + bw[0] / 2.0, y, fs, "middle", &format!("{} to {}", a + 1, b + 1));
            if answers {
                let t = bearing(&feats[*a], &feats[*b]).map(|v| format!("{v:03}\u{b0}")).unwrap_or_else(|| "same place".into());
                chart::text(&mut d, tx + bw[0] + bw[1] / 2.0, y, fs, "middle", &t);
            }
        }
        chart::text(&mut d, tx, y + 34.0, fs * 0.9, "start", "Bearings are measured clockwise from north.");
    }
    let _ = trim;
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }
    fn f(x: f64, y: f64) -> Feature { Feature { name: "p".into(), x, y } }

    #[test]
    fn references_name_the_square_and_the_tenths() {
        assert_eq!(four_figure(10, &f(2.4, 6.7)), "1216");
        assert_eq!(six_figure(10, &f(2.4, 6.7)), "124 167");
        assert_eq!(four_figure(98, &f(2.0, 0.5)), "0098");
        assert_eq!(six_figure(0, &f(0.0, 0.99)), "000 009");
        assert_eq!(line_no(95, 7.0), 2);
    }

    #[test]
    fn bearings_run_clockwise_from_north() {
        assert_eq!(bearing(&f(0.0, 0.0), &f(0.0, 1.0)), Some(0));
        assert_eq!(bearing(&f(0.0, 0.0), &f(1.0, 0.0)), Some(90));
        assert_eq!(bearing(&f(0.0, 0.0), &f(0.0, -1.0)), Some(180));
        assert_eq!(bearing(&f(0.0, 0.0), &f(-1.0, 0.0)), Some(270));
        assert_eq!(bearing(&f(0.0, 0.0), &f(1.0, 1.0)), Some(45));
        assert_eq!(bearing(&f(0.0, 0.0), &f(-0.001, 1.0)), Some(0), "359.94 rounds to 360, which is 000");
        assert_eq!(bearing(&f(1.0, 1.0), &f(1.0, 1.0)), None);
    }

    #[test]
    fn key_fills_the_table() {
        let svg = render(&defaults());
        assert!(svg.contains(">1216<") && svg.contains(">124 167<") && svg.contains("1 to 3") && svg.contains("clockwise from north"));
        assert!(svg.contains(">Church<"));
    }

    #[test]
    fn student_copy_leaves_boxes_empty() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Church<") && !student.contains(">1216<") && !student.contains("\u{b0}<") && key.contains("\u{b0}<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        for (field, spec, needle) in [
            ("features", "", "at least one"),
            ("features", "Church", "needs a name"),
            ("features", "Church a b", "two numbers"),
            ("features", "Church 9 1", "off the map"),
            ("features", "Church -1 1", "off the map"),
            ("features", "a 1 1\nb 1 1\nc 1 1\nd 1 1\ne 1 1\nf 1 1\ng 1 1\nh 1 1\ni 1 1", "limited to 8"),
            ("bearings", "1 9", "not a pair"),
            ("bearings", "1", "not a pair"),
            ("bearings", "1 2\n1 2\n1 2\n1 2\n1 2", "limited to 4"),
        ] {
            let mut p = defaults();
            p.set_s(field, spec);
            assert!(render(&p).contains(needle), "{field}: {spec:?}");
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
        for ex in examples("grid-reference").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("off the map") && !svg.contains("not a pair"), "{}", ex.name);
        }
        for spec in ["nan 1 1", "x 1e999 1", "<b>x</b> 1 1", "\u{0} 1 1", &"y 1 1\n".repeat(50), "a 3.9999999 3.9999999"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("features", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
        let mut p = defaults();
        p.set_i("cols", 1000);
        p.set_i("origin", -5);
        assert!(sane(&render(&p)));
    }
}
