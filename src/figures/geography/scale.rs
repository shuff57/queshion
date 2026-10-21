//! A map scale: a ratio such as 1 : 50 000, a scale bar drawn to it and a few measured map
//! distances worked out as ground distances. The lines are drawn at their true length (one
//! centimetre is 37.8 units, the size of a CSS centimetre) so a ruler held to the page agrees with
//! the numbers. The ratio and the measurements are the question; the workings and the ground
//! distances are the answers, so the student copy (Show answers off) leaves them as empty boxes.
use super::trim;
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_LINES: usize = 5;
const INK: &str = "#111827";
/// Drawing units in one centimetre.
pub const CM: f64 = 37.795;
const DEFAULT: &str = "3.4\n5.8\n8.2";

pub fn def() -> FigureDef {
    FigureDef {
        id: "map-scale",
        subject: Subject::Geography,
        title: "Map scale and distances",
        blurb: "A map scale (1 : 50 000) with a scale bar drawn to it and measured map distances turned into ground distances, with the workings shown, or left empty for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "map-scale" => vec![
            Example { name: "1 : 50 000", params: vec![] },
            Example { name: "Town map, 1 : 10 000", params: vec![("ratio", Value::I(10000)), ("lengths", s("2.5\n6.4\n11.2\n4"))] },
            Example { name: "Atlas map, 1 : 5 000 000", params: vec![("ratio", Value::I(5000000)), ("lengths", s("1.2\n3.7\n6.5"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::int("ratio", "Scale 1 : ", 100, 100_000_000, 50_000).hint("1 cm on the map is this many cm on the ground"),
        ParamDef::long("lengths", "Measured map distances (cm)", DEFAULT).hint("One per line, 0.1 to 25 cm; up to 5"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the scale, the bar and the measured lines and leaves the workings and the ground distances empty"),
    ]
}

fn parse(spec: &str) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for tok in spec.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|t| !t.is_empty()) {
        match tok.parse::<f64>() {
            Ok(x) if x.is_finite() && (0.1..=25.0).contains(&x) => v.push(x),
            _ => return Err(format!("\u{201c}{}\u{201d} is not a distance from 0.1 to 25 cm", tok.chars().take(20).collect::<String>())),
        }
        if v.len() > MAX_LINES {
            return Err(format!("A figure is limited to {MAX_LINES} distances"));
        }
    }
    if v.is_empty() {
        return Err("Give at least one measured distance in cm".into());
    }
    Ok(v)
}

/// Digits grouped in threes with a space, keeping any decimals.
pub fn grouped(v: f64) -> String {
    let s = trim(v, 3);
    let (neg, s) = match s.strip_prefix('\u{2212}') { Some(r) => ("\u{2212}", r.to_string()), None => ("", s) };
    let (int, frac) = match s.split_once('.') { Some((a, b)) => (a.to_string(), format!(".{b}")), None => (s.clone(), String::new()) };
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push('\u{a0}');
        }
        out.push(c);
    }
    format!("{neg}{out}{frac}")
}

/// A ground distance in cm as metres below 1 km, kilometres from 1 km.
pub fn ground(cm: f64) -> String {
    let km = cm / 100_000.0;
    if km < 1.0 { format!("{} m", trim(cm / 100.0, 2)) } else { format!("{} km", trim(km, 3)) }
}

/// The scale bar's length in km: the largest 1, 2 or 5 times a power of ten that fits in 10 cm.
pub fn bar_km(ratio: f64) -> f64 {
    let max = 10.0 * ratio / 100_000.0;
    let mut best = 0.0;
    let mut m = 1e-6;
    while m < 1e6 {
        for f in [1.0, 2.0, 5.0] {
            if f * m <= max * (1.0 + 1e-9) {
                best = f * m;
            }
        }
        m *= 10.0;
    }
    best
}

fn render(p: &Params) -> String {
    let ratio = p.i("ratio").clamp(100, 100_000_000) as f64;
    let lens = match parse(p.s("lengths")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let w = 1000.0;
    let h = top + 34.0 + 56.0 + 40.0 + lens.len() as f64 * 44.0 + 56.0 + (lens.len() as f64 + 1.0) * 34.0 + 30.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let x0 = 40.0;
    let mut y = top + 34.0;
    chart::text(&mut d, x0, y, fs * 1.3, "start", &format!("Scale 1 : {}", grouped(ratio)));
    // 1 cm on the map
    chart::text(&mut d, x0 + 260.0, y, fs, "start", "1 cm on the map represents");
    let x = x0 + 260.0 + fs * 13.5;
    if answers {
        chart::text(&mut d, x, y, fs, "start", &ground(ratio));
    } else {
        d.line(x, y + 3.0, x + 120.0, y + 3.0, INK, 1.0);
    }
    // the scale bar
    y += 56.0;
    let lk = bar_km(ratio);
    let (unit, scale) = if lk < 1.0 { ("m", 1000.0) } else { ("km", 1.0) };
    let bar_px = lk * 100_000.0 / ratio * CM;
    let parts = 5;
    for i in 0..parts {
        let fill = if i % 2 == 0 { INK } else { "#fff" };
        d.rect(x0 + bar_px / parts as f64 * i as f64, y - 8.0, bar_px / parts as f64, 8.0, fill, INK, 1.2);
    }
    for i in 0..=parts {
        let xx = x0 + bar_px / parts as f64 * i as f64;
        d.line(xx, y - 8.0, xx, y - 14.0, INK, 1.2);
        chart::text(&mut d, xx, y - 18.0, fs * 0.9, "middle", &trim(lk * scale / parts as f64 * i as f64, 3));
    }
    chart::text(&mut d, x0 + bar_px + 14.0, y, fs * 0.9, "start", unit);
    chart::text(&mut d, x0, y + fs * 1.6, fs * 0.9, "start", "Scale bar");
    // the measured lines, each at its true length
    y += 40.0;
    for (i, l) in lens.iter().enumerate() {
        y += 44.0;
        let len = l * CM;
        d.line(x0, y, x0 + len, y, INK, 3.0);
        d.line(x0, y - 8.0, x0, y + 8.0, INK, 2.0);
        d.line(x0 + len, y - 8.0, x0 + len, y + 8.0, INK, 2.0);
        chart::text(&mut d, x0 + len + 14.0, y + fs * 0.35, fs, "start", &format!("{}: {} cm", i + 1, trim(*l, 3)));
    }
    // the working
    y += 56.0;
    let cw = [60.0, 130.0, 560.0, 190.0];
    let sum = |c: usize| cw[..c].iter().sum::<f64>();
    d.rect(x0, y - 22.0, cw.iter().sum(), 34.0, "none", INK, 1.2);
    for (c, hd) in ["Line", "Map (cm)", "Working", "Ground distance"].iter().enumerate() {
        chart::text(&mut d, x0 + sum(c) + cw[c] / 2.0, y, fs, "middle", hd);
    }
    let table_top = y - 22.0;
    for (i, l) in lens.iter().enumerate() {
        y += 34.0;
        d.rect(x0, y - 22.0, cw.iter().sum(), 34.0, "none", INK, 1.0);
        chart::text(&mut d, x0 + cw[0] / 2.0, y, fs, "middle", &(i + 1).to_string());
        chart::text(&mut d, x0 + sum(1) + cw[1] / 2.0, y, fs, "middle", &trim(*l, 3));
        if answers {
            let g = l * ratio;
            chart::text(&mut d, x0 + sum(2) + 10.0, y, fs, "start", &format!("{} \u{d7} {} = {} cm", trim(*l, 3), grouped(ratio), grouped(g)));
            chart::text(&mut d, x0 + sum(3) + cw[3] / 2.0, y, fs, "middle", &ground(g));
        }
    }
    for c in 1..4 {
        d.line(x0 + sum(c), table_top, x0 + sum(c), y + 12.0, INK, 1.0);
    }
    let _ = print;
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn grouping_and_units() {
        assert_eq!(grouped(170000.0), "170\u{a0}000");
        assert_eq!(grouped(50000.0), "50\u{a0}000");
        assert_eq!(grouped(12.5), "12.5");
        assert_eq!(grouped(1234567.0), "1\u{a0}234\u{a0}567");
        assert_eq!(ground(170000.0), "1.7 km");
        assert_eq!(ground(85000.0), "850 m");
        assert_eq!(ground(100000.0), "1 km");
        assert_eq!(ground(1.0), "0.01 m");
    }

    #[test]
    fn the_bar_fits_in_ten_centimetres_and_is_not_tiny() {
        for ratio in [100.0, 1000.0, 25000.0, 50000.0, 63360.0, 5.0e6, 1.0e8] {
            let cm = bar_km(ratio) * 100_000.0 / ratio;
            assert!(cm <= 10.0 + 1e-6 && cm >= 3.9, "{ratio}: {cm}");
        }
        assert_eq!(bar_km(50000.0), 5.0);
        assert_eq!(bar_km(10000.0), 1.0);
    }

    #[test]
    fn key_works_the_distances() {
        let svg = render(&defaults());
        assert!(svg.contains("3.4 \u{d7} 50\u{a0}000 = 170\u{a0}000 cm") && svg.contains(">1.7 km<") && svg.contains(">2.9 km<") && svg.contains(">500 m<"));
        assert!(svg.contains("Scale 1 : 50\u{a0}000"));
    }

    #[test]
    fn student_copy_leaves_the_workings_empty() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">3.4<"));
        assert!(!student.contains("170") && !student.contains("500 m"));
        assert!(student.contains("Scale 1 : 50\u{a0}000"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        for (spec, needle) in [("", "at least one"), ("a", "not a distance"), ("0", "not a distance"), ("26", "not a distance"), ("1 1 1 1 1 1", "limited to 5")] {
            let mut p = defaults();
            p.set_s("lengths", spec);
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
        for ex in examples("map-scale").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("not a distance"), "{}", ex.name);
        }
        for spec in ["nan", "1e999", "<b>", "\u{0}", "0.1\n25\n0.1\n25\n0.1"] {
            for ratio in [-5, 0, 100, 100_000_000, i64::MAX] {
                let mut p = defaults();
                p.set_s("lengths", spec);
                p.set_i("ratio", ratio);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?} {ratio}");
            }
        }
    }
}
