//! Graphic organisers: a T-chart, a KWL chart and a Venn diagram. The items are listed mixed up
//! under "Items to sort"; where each belongs is the answer, so the student copy (Show answers off)
//! leaves the columns or the circles empty.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use super::{lines_of, shuffled, wrap};

const KINDS: [&str; 3] = ["T-chart", "KWL chart", "Venn diagram"];
const INK: &str = "#111827";
const MAX_EACH: usize = 8;

pub fn def() -> FigureDef {
    FigureDef {
        id: "organiser",
        subject: Subject::Language,
        title: "Graphic organiser",
        blurb: "A T-chart, KWL chart or Venn diagram with the items sorted into place, or empty columns or circles with the items listed mixed up for students to sort.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    if fig != "organiser" {
        return true;
    }
    let kind = p.s("kind");
    match key {
        "title_a" | "title_b" => kind != KINDS[1],
        "items_c" => kind != KINDS[0],
        "hide" => false,
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "organiser" => vec![
            Example { name: "T-chart: facts and opinions", params: vec![] },
            Example { name: "KWL chart", params: vec![("kind", s(KINDS[1])), ("items_a", s("Frogs are amphibians\nThey lay eggs in water")), ("items_b", s("How do they breathe?\nWhy do they croak?")), ("items_c", s("Tadpoles breathe with gills\nMales croak to attract mates"))] },
            Example { name: "Venn: two stories", params: vec![("kind", s(KINDS[2])), ("title_a", s("Fable")), ("title_b", s("Myth")), ("items_a", s("Has a moral\nAnimals speak")), ("items_b", s("Explains nature\nGods and heroes")), ("items_c", s("Passed on by telling\nShort tale"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Organiser", &KINDS, KINDS[0]),
        ParamDef::text("title_a", "Heading of the first column or circle", "Fact"),
        ParamDef::text("title_b", "Heading of the second", "Opinion"),
        ParamDef::long("items_a", "First column (T-chart, Venn) or Know (KWL)", "Water boils at 100 degrees\nThe moon orbits the Earth").hint("One item per line, up to 8"),
        ParamDef::long("items_b", "Second column or circle, or Want to know (KWL)", "Summer is the best season\nMaths is boring"),
        ParamDef::long("items_c", "Both circles (Venn) or Learned (KWL)", ""),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the empty organiser and the mixed-up items and leaves it for students to sort"),
    ]
}

fn render(p: &Params) -> String {
    let kind = p.s("kind");
    let kind = KINDS.iter().find(|k| **k == kind).copied().unwrap_or(KINDS[0]);
    let mut groups: Vec<Vec<String>> = vec![lines_of(p.s("items_a"), 40), lines_of(p.s("items_b"), 40), if kind == KINDS[0] { Vec::new() } else { lines_of(p.s("items_c"), 40) }];
    if groups.iter().any(|g| g.len() > MAX_EACH) {
        return chart::message(&format!("Each box is limited to {MAX_EACH} items"));
    }
    if kind == KINDS[0] {
        groups.truncate(2);
    }
    if groups.iter().all(|g| g.is_empty()) {
        return chart::message("Give some items in at least one box");
    }
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (ta, tb) = (p.s("title_a").trim().chars().take(24).collect::<String>(), p.s("title_b").trim().chars().take(24).collect::<String>());
    let heads: Vec<String> = match kind {
        k if k == KINDS[1] => vec!["Know".into(), "Want to know".into(), "Learned".into()],
        k if k == KINDS[2] => vec![ta, tb, "Both".into()],
        _ => vec![ta, tb],
    };
    // the items in a fixed mixed-up order, numbered
    let mut all: Vec<(usize, &String)> = Vec::new();
    for (g, v) in groups.iter().enumerate() {
        for t in v {
            all.push((g, t));
        }
    }
    let n = all.len();
    let order = shuffled(n);
    let mut number = vec![0usize; n];
    for (pos, &orig) in order.iter().enumerate() {
        number[orig] = pos + 1;
    }
    let list_rows = (n + 1) / 2;
    let w = 900.0;
    let area_h = if kind == KINDS[2] { 470.0 } else { 80.0 + MAX_EACH as f64 * fs * 1.8 };
    let h = top + 40.0 + area_h + 50.0 + list_rows as f64 * fs * 1.9 + 30.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let y0 = top + 30.0;
    let tints = if print { ["#eeeeee", "#dddddd", "#cccccc"] } else { ["#cfe0fa", "#fdeccf", "#d8efd9"] };
    let mut offs = vec![0usize; groups.len()];
    for g in 1..groups.len() {
        offs[g] = offs[g - 1] + groups[g - 1].len();
    }
    // where the items of group g are written (x anchor, first baseline, width in characters)
    let mut spots: Vec<(f64, f64, usize)> = Vec::new();
    if kind == KINDS[2] {
        let (r, cy) = (230.0, y0 + 235.0);
        d.raw(&format!(r##"<circle cx="330" cy="{cy}" r="{r}" fill="{}" fill-opacity="0.75" stroke="{INK}" stroke-width="2"/>"##, tints[0]));
        d.raw(&format!(r##"<circle cx="570" cy="{cy}" r="{r}" fill="{}" fill-opacity="0.75" stroke="{INK}" stroke-width="2"/>"##, tints[1]));
        chart::text(&mut d, 235.0, y0 + 85.0, fs * 1.15, "middle", &heads[0]);
        chart::text(&mut d, 665.0, y0 + 85.0, fs * 1.15, "middle", &heads[1]);
        chart::text(&mut d, 450.0, y0 + 85.0, fs * 1.05, "middle", &heads[2]);
        spots = vec![(235.0, y0 + 135.0, 22), (665.0, y0 + 135.0, 22), (450.0, y0 + 135.0, 14)];
    } else {
        let cols = groups.len();
        let cw = (w - 60.0) / cols as f64;
        for g in 0..cols {
            let x = 30.0 + g as f64 * cw;
            d.rect(x, y0, cw, 40.0, tints[g], INK, 1.6);
            chart::text(&mut d, x + cw / 2.0, y0 + 26.0, fs * 1.15, "middle", &heads[g]);
            d.rect(x, y0 + 40.0, cw, area_h - 40.0, "none", INK, 1.6);
            spots.push((x + cw / 2.0, y0 + 72.0, ((cw - 24.0) / (fs * 0.55)) as usize));
        }
    }
    if answers {
        for (g, v) in groups.iter().enumerate() {
            let (x, y, chars) = spots[g];
            let mut yy = y;
            for (k, t) in v.iter().enumerate() {
                let label = format!("{}. {}", number[offs[g] + k], t);
                for l in wrap(&label, chars, 2) {
                    chart::text(&mut d, x, yy, fs * 0.95, "middle", &l);
                    yy += fs * 1.35;
                }
                yy += fs * 0.35;
            }
        }
    }
    let ly = y0 + area_h + 44.0;
    chart::text(&mut d, 30.0, ly - 12.0, fs * 1.05, "start", "Items to sort");
    for (pos, &orig) in order.iter().enumerate() {
        let (col, row) = (pos / list_rows, pos % list_rows);
        chart::text(&mut d, 30.0 + col as f64 * 430.0, ly + 10.0 + row as f64 * fs * 1.9, fs * 0.95, "start", &format!("{}. {}", pos + 1, all[orig].1));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }
    fn with(kind: &str) -> Params { let mut p = defaults(); p.set_s("kind", kind); p.set_s("items_c", "shared one\nshared two"); p }

    #[test]
    fn each_kind_draws_and_sorts() {
        for k in KINDS {
            let (student, key) = crate::figures::answer_pair(&def(), &with(k)).unwrap();
            assert!(sane(&key) && sane(&student), "{k}");
            assert_eq!(size_of(&student), size_of(&key), "{k}");
            assert!(key.contains("Items to sort") && key.contains("Water boils"), "{k}");
            let count = |s: &str| s.matches("Water boils").count();
            assert_eq!((count(&key), count(&student)), (2, 1), "{k}");
        }
        assert!(render(&with(KINDS[1])).contains(">Want to know<"));
        assert!(render(&with(KINDS[2])).contains(">Both<") && render(&with(KINDS[2])).matches("<circle").count() == 2);
        assert!(!render(&with(KINDS[0])).contains("shared one"));
    }

    #[test]
    fn bad_input_is_explained() {
        let mut p = defaults();
        p.set_s("items_a", &"x\n".repeat(9));
        assert!(render(&p).contains("limited to 8"));
        p.set_s("items_a", "");
        p.set_s("items_b", "");
        assert!(render(&p).contains("Give some items"));
    }

    #[test]
    fn controls_follow_the_kind() {
        let p = with(KINDS[1]);
        assert!(!relevant("organiser", "title_a", &p) && relevant("organiser", "items_c", &p));
        assert!(!relevant("organiser", "items_c", &defaults()) && relevant("organiser", "title_a", &defaults()));
        assert!(relevant("histogram", "title_a", &p));
    }

    #[test]
    fn print_style_is_grey() {
        for k in KINDS {
            let mut p = with(k);
            p.set_s("style", "Print");
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("organiser").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("limited to") && !svg.contains("Give some"), "{}", ex.name);
        }
        for k in KINDS {
            for spec in ["\u{0}", "<b>x</b>", "nan", &"w".repeat(5000), "a\n\n b "] {
                for answers in [true, false] {
                    let mut p = with(k);
                    p.set_s("items_a", spec);
                    p.set_s("title_a", "<i>");
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg) && !svg.contains("<b>") && !svg.contains("<i>"), "{k} {spec:?}");
                }
            }
        }
    }
}
