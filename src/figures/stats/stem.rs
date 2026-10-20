//! Stem-and-leaf plot. Each value is rounded to the leaf unit and split into a stem (everything
//! but the last digit) and a leaf (the last digit). The leaves are what a student is asked to
//! fill in, so the student copy (Show answers off) keeps the stems, the bar and the key and
//! hides the leaves.
//!
//! The plot is text only (no fills), so it has no Print style: it is already black and white.
use std::collections::BTreeMap;

use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "stem-and-leaf",
        subject: Subject::Math,
        title: "Stem-and-Leaf Plot",
        blurb: "A stem-and-leaf plot of a data list, with automatic or chosen leaf unit, split stems and a key.",
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
        "stem-and-leaf" => vec![
            Example { name: "Test scores", params: vec![] },
            Example {
                name: "Decimals, split stems",
                params: vec![
                    ("data", s("1.2, 1.5, 1.6, 1.8, 2.0, 2.1, 2.1, 2.4, 2.6, 2.7, 2.9, 3.1, 3.3, 3.8, 4.2")),
                    ("leaf_unit", s("0.1")),
                    ("split", s("Split each stem in two")),
                ],
            },
            Example {
                name: "Below and above zero",
                params: vec![("data", s("-12, -9, -5, -3, 0, 2, 4, 4, 7, 9, 11, 15, 18, 22"))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("data", "Numbers", 1, "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93"),
        ParamDef::choice("leaf_unit", "Leaf unit", &["Auto", "0.01", "0.1", "1", "10", "100"], "Auto").hint("The place value of a leaf. With a leaf unit of 1, the number 63 is stem 6, leaf 3"),
        ParamDef::choice("split", "Stems", &["One row per stem", "Split each stem in two"], "One row per stem").hint("Splitting puts leaves 0 to 4 and 5 to 9 on separate rows, for crowded plots"),
        ParamDef::bool("order", "Put the leaves in order", true),
        ParamDef::bool("key", "Write the key", true),
        ParamDef::text("title", "Title", ""),
        svg::label_size_param(),
        answers_param().hint("Off hides the leaves and keeps the stems and key, for students to fill in"),
    ]
}

/// One row of the plot: the stem as text, and the leaf digits left to right.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub stem: String,
    pub leaves: Vec<u8>,
}

/// A value in leaf units, rounded.
fn scaled(v: f64, unit: f64) -> i64 {
    (v / unit).round() as i64
}

/// The smallest power of ten unit that gives at most `max_stems` stems.
pub fn auto_unit(values: &[f64], max_stems: f64) -> f64 {
    let (lo, hi) = values.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    if values.is_empty() {
        return 1.0;
    }
    if hi - lo <= 0.0 {
        // one value repeated: a unit a tenth of its size puts it on a one-digit stem
        let m = hi.abs();
        return if m == 0.0 { 1.0 } else { 10f64.powf(m.log10().floor() - 1.0) };
    }
    for k in -3..=12 {
        let unit = 10f64.powi(k);
        if (hi - lo) / (10.0 * unit) + 1.0 <= max_stems {
            return unit;
        }
    }
    1e12
}

/// A stem's position: stems run ... -1, -0, 0, 1 ... and `idx` gives them consecutive odd
/// numbers (-0 is -1, 0 is 1) so negatives sort before positives and "-0" before "0".
fn stem_idx(s: i64) -> i64 {
    let stem = s.abs() / 10;
    if s < 0 { -(2 * stem + 1) } else { 2 * stem + 1 }
}

fn stem_label(idx: i64) -> String {
    let n = (idx.abs() - 1) / 2;
    if idx < 0 { format!("\u{2212}{n}") } else { n.to_string() }
}

/// The plot's rows from the lowest stem to the highest, with empty stems in between kept (a gap
/// in the data is part of the picture). With `split`, each stem is two rows (leaves 0-4, 5-9).
/// None when there would be more than [`MAX_ROWS`] rows.
pub fn rows(values: &[f64], unit: f64, split: bool, ordered: bool) -> Option<Vec<Row>> {
    if !(unit.is_finite() && unit > 0.0) || values.is_empty() {
        return None;
    }
    // (stem idx, half) -> leaves in the order met
    let mut cells: BTreeMap<(i64, u8), Vec<(i64, u8)>> = BTreeMap::new();
    for v in values {
        let s = scaled(*v, unit);
        let leaf = (s.abs() % 10) as u8;
        let half = if split && leaf >= 5 { 1 } else { 0 };
        cells.entry((stem_idx(s), half)).or_default().push((s, leaf));
    }
    let (lo, hi) = (cells.keys().next().unwrap().0, cells.keys().last().unwrap().0);
    if (hi - lo) / 2 + 1 > MAX_ROWS as i64 {
        return None;
    }
    let halves: &[u8] = if split { &[0, 1] } else { &[0] };
    let mut out = Vec::new();
    let mut idx = lo;
    while idx <= hi {
        // a negative stem runs from its most negative leaves to its least, so its high half comes first
        let order: Vec<u8> = if idx < 0 { halves.iter().rev().cloned().collect() } else { halves.to_vec() };
        for half in order {
            let mut leaves = cells.remove(&(idx, half)).unwrap_or_default();
            if ordered {
                leaves.sort_by_key(|(s, _)| *s);
            }
            out.push(Row { stem: stem_label(idx), leaves: leaves.into_iter().map(|(_, l)| l).collect() });
        }
        idx += 2;
    }
    // an empty half-row at either end says nothing; gaps in the middle are kept
    while out.last().is_some_and(|r| r.leaves.is_empty()) {
        out.pop();
    }
    let lead = out.iter().take_while(|r| r.leaves.is_empty()).count();
    out.drain(..lead);
    Some(out)
}

pub const MAX_ROWS: usize = 40;
const MAX_LEAVES: usize = 60;

/// "Key: 6 | 3 means 63" from the first row that has a leaf, with the unit when it is not 1.
fn key_text(rows: &[Row], unit: f64) -> Option<String> {
    let row = rows.iter().find(|r| !r.leaves.is_empty())?;
    let leaf = row.leaves[0];
    let neg = row.stem.starts_with('\u{2212}');
    let stem: i64 = row.stem.trim_start_matches('\u{2212}').parse().ok()?;
    let units = (stem * 10 + leaf as i64) * if neg { -1 } else { 1 };
    let value = chart::fmt(units as f64 * unit);
    let mut s = format!("Key: {} | {leaf} means {value}", row.stem);
    if unit != 1.0 {
        s += &format!("  (leaf unit {})", chart::fmt(unit));
    }
    Some(s)
}

fn render(p: &Params) -> String {
    let values = data::numbers(p.s("data"));
    if values.is_empty() {
        return chart::message("Enter some data");
    }
    let unit = match p.s("leaf_unit") {
        "Auto" => auto_unit(&values, 12.0),
        u => u.parse().unwrap_or(1.0),
    };
    let Some(rows) = rows(&values, unit, p.s("split") != "One row per stem", p.b("order")) else {
        return chart::message("Too many stems: use a larger leaf unit");
    };
    if rows.iter().map(|r| r.leaves.len()).max().unwrap_or(0) > MAX_LEAVES {
        return chart::message("Too many leaves on one stem: use a smaller leaf unit or split the stems");
    }
    let k = svg::label_factor(p.s("label_size"));
    let fs = 17.0 * k;
    let (title, show_key) = (p.s("title").trim(), p.b("key"));
    let key = if show_key { key_text(&rows, unit) } else { None };

    let stem_w = rows.iter().map(|r| r.stem.chars().count()).max().unwrap_or(1) as f64 * fs * 0.62;
    let pitch = fs * 0.95;
    let widest = rows.iter().map(|r| r.leaves.len()).max().unwrap_or(0).max(3) as f64;
    let rowh = fs * 1.75;
    let top = if title.is_empty() { 22.0 } else { 34.0 + fs * 1.2 };
    let body = rowh * rows.len() as f64;
    let foot = if key.is_some() { fs * 2.6 } else { 16.0 };
    let key_w = key.as_ref().map_or(0.0, |k| k.chars().count() as f64 * fs * 0.85 * 0.58 + 80.0);
    let (w, h) = ((40.0 + stem_w + 22.0 + widest * pitch + 40.0).max(260.0).max(key_w), top + body + foot);

    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.1, fs * 1.15, "middle", title);
    }
    let bar_x = 40.0 + stem_w + 10.0;
    d.line(bar_x, top, bar_x, top + body, INK, 2.2);
    let answers = p.b(SHOW_ANSWERS);
    for (i, row) in rows.iter().enumerate() {
        let y = top + rowh * i as f64 + rowh * 0.72;
        chart::text(&mut d, bar_x - 10.0, y, fs, "end", &row.stem);
        if answers {
            for (j, leaf) in row.leaves.iter().enumerate() {
                chart::text(&mut d, bar_x + 14.0 + j as f64 * pitch, y, fs, "start", &leaf.to_string());
            }
        }
    }
    if let Some(key) = key {
        chart::text(&mut d, 40.0, top + body + fs * 1.7, fs * 0.85, "start", &key);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }
    fn strs(r: &[Row]) -> Vec<String> {
        r.iter().map(|r| format!("{}|{}", r.stem, r.leaves.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(""))).collect()
    }

    #[test]
    fn classic_two_digit_plot() {
        let r = rows(&[34.0, 12.0, 21.0, 15.0, 21.0], 1.0, false, true).unwrap();
        assert_eq!(strs(&r), ["1|25", "2|11", "3|4"]);
    }

    #[test]
    fn a_gap_in_the_data_keeps_its_empty_stem() {
        let r = rows(&[12.0, 35.0], 1.0, false, true).unwrap();
        assert_eq!(strs(&r), ["1|2", "2|", "3|5"]);
    }

    #[test]
    fn unordered_leaves_keep_the_order_they_were_given() {
        let r = rows(&[15.0, 12.0, 11.0], 1.0, false, false).unwrap();
        assert_eq!(strs(&r), ["1|521"]);
        assert_eq!(strs(&rows(&[15.0, 12.0, 11.0], 1.0, false, true).unwrap()), ["1|125"]);
    }

    #[test]
    fn split_stems_separate_low_and_high_leaves() {
        let r = rows(&[41.0, 45.0, 46.0, 49.0, 52.0], 1.0, true, true).unwrap();
        assert_eq!(strs(&r), ["4|1", "4|569", "5|2"], "empty rows at the ends are dropped");
    }

    #[test]
    fn decimals_use_the_leaf_unit() {
        let r = rows(&[1.2, 1.5, 2.0], 0.1, false, true).unwrap();
        assert_eq!(strs(&r), ["1|25", "2|0"]);
        assert_eq!(strs(&rows(&[120.0, 150.0, 200.0], 10.0, false, true).unwrap()), ["1|25", "2|0"], "unit 10");
    }

    #[test]
    fn negatives_have_a_minus_zero_stem_and_ascending_values() {
        let r = rows(&[-12.0, -5.0, 3.0, 14.0, -39.0, -32.0], 1.0, false, true).unwrap();
        assert_eq!(strs(&r), ["\u{2212}3|92", "\u{2212}2|", "\u{2212}1|2", "\u{2212}0|5", "0|3", "1|4"]);
    }

    #[test]
    fn every_value_appears_exactly_once() {
        let v = data::generate(data::Dist::Normal, 120, 4, 50.0, 20.0, 1);
        for unit in [0.1, 1.0, 10.0] {
            for split in [false, true] {
                if let Some(r) = rows(&v, unit, split, true) {
                    assert_eq!(r.iter().map(|r| r.leaves.len()).sum::<usize>(), v.len(), "unit {unit} split {split}");
                }
            }
        }
    }

    #[test]
    fn auto_unit_aims_for_a_dozen_stems_or_fewer() {
        let scores = data::numbers("52, 58, 61, 63, 93");
        assert_eq!(auto_unit(&scores, 12.0), 1.0);
        assert_eq!(auto_unit(&[0.12, 0.55, 0.9], 12.0), 0.01);
        assert_eq!(auto_unit(&[1200.0, 5400.0, 9100.0], 12.0), 100.0);
        assert_eq!(auto_unit(&[5.0, 5.0], 12.0), 0.1, "a single value sits on a one-digit stem");
        assert_eq!(auto_unit(&[0.0, 0.0], 12.0), 1.0);
        assert_eq!(auto_unit(&[], 12.0), 1.0);
    }

    #[test]
    fn the_key_reads_back_a_value() {
        let r = rows(&[63.0, 65.0], 1.0, false, true).unwrap();
        assert_eq!(key_text(&r, 1.0).unwrap(), "Key: 6 | 3 means 63");
        let r = rows(&[1.2, 1.5], 0.1, false, true).unwrap();
        assert_eq!(key_text(&r, 0.1).unwrap(), "Key: 1 | 2 means 1.2  (leaf unit 0.1)");
        let r = rows(&[-32.0], 1.0, false, true).unwrap();
        assert_eq!(key_text(&r, 1.0).unwrap(), "Key: \u{2212}3 | 2 means \u{2212}32");
    }

    #[test]
    fn default_draws_every_leaf() {
        let p = defaults();
        let svg = render(&p);
        let n = data::numbers(p.s("data")).len();
        let r = rows(&data::numbers(p.s("data")), 1.0, false, true).unwrap();
        let stems = r.len();
        // title-less: each leaf, each stem and the key are one <text>
        assert_eq!(svg.matches("<text").count(), n + stems + 1);
        assert!(svg.contains("Key: 5 | 2 means 52"));
    }

    #[test]
    fn student_copy_keeps_stems_bar_and_key() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        let r = rows(&data::numbers(defaults().s("data")), 1.0, false, true).unwrap();
        assert_eq!(student.matches("<text").count(), r.len() + 1, "stems and the key, no leaves");
        assert!(student.contains("<line") && student.contains("Key: "));
        assert!(key.matches("<text").count() > student.matches("<text").count());
        let size = |s: &str| s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string();
        assert_eq!(size(&student), size(&key), "same size, so the copies can be swapped");
    }

    #[test]
    fn rendering_is_black_and_white() {
        let mut p = defaults();
        p.set_s("title", "Quiz scores");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24), "no colour anywhere");
    }

    #[test]
    fn too_many_stems_or_leaves_say_so() {
        let mut p = defaults();
        p.set_s("data", "1, 100000");
        p.set_s("leaf_unit", "0.01");
        assert!(render(&p).contains("Too many stems"));
        p.set_s("data", &vec!["5"; 100].join(", "));
        p.set_s("leaf_unit", "Auto");
        assert!(render(&p).contains("Too many leaves"));
    }

    #[test]
    fn odd_input_never_panics_or_prints_nan() {
        let mut p = defaults();
        for text in ["", "abc", "0", "5", "-0.001", "5, 5, 5", "-1e12, 1e12", "1e12", "0.0000001, 0.0000002", "-5, -4, -3", "1, 2, 3, 1000000000"] {
            for unit in ["Auto", "0.01", "0.1", "1", "10", "100"] {
                for split in ["One row per stem", "Split each stem in two"] {
                    p.set_s("data", text);
                    p.set_s("leaf_unit", unit);
                    p.set_s("split", split);
                    for answers in [true, false] {
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?} {unit}");
                        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text:?} {unit}: {svg}");
                    }
                }
            }
        }
    }

    #[test]
    fn examples_render() {
        for e in examples("stem-and-leaf").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            let svg = render(&p);
            assert!(svg.contains("Key:"), "{}: {svg}", e.name);
        }
    }
}
