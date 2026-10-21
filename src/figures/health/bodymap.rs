//! A body map: a simplified front view of the body with numbered organs. The numbers are given;
//! the organ names are the answer, so the student copy (Show answers off) leaves the list of names
//! blank.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const SHOWS: [&str; 3] = ["All organs", "Digestive system", "Breathing and circulation"];
const INK: &str = "#111827";

pub fn def() -> FigureDef {
    FigureDef {
        id: "body-map",
        subject: Subject::Health,
        title: "Body map",
        blurb: "A simplified front view of the body with numbered organs, named in a list, or a blank list for students to name them. Choose all organs, the digestive system, or breathing and circulation.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "body-map" => vec![
            Example { name: "All organs", params: vec![] },
            Example { name: "Digestive system", params: vec![("show", s(SHOWS[1]))] },
            Example { name: "Breathing and circulation", params: vec![("show", s(SHOWS[2]))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("show", "Organs to show", &SHOWS, SHOWS[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the body and the numbered organs and leaves the names blank for students"),
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Organ { Brain, Trachea, Lungs, Heart, Liver, Stomach, SmallIntestine, LargeIntestine, Kidneys }

impl Organ {
    fn name(self) -> &'static str {
        match self {
            Organ::Brain => "Brain", Organ::Trachea => "Windpipe (trachea)", Organ::Lungs => "Lungs", Organ::Heart => "Heart", Organ::Liver => "Liver",
            Organ::Stomach => "Stomach", Organ::SmallIntestine => "Small intestine", Organ::LargeIntestine => "Large intestine", Organ::Kidneys => "Kidneys",
        }
    }
    /// Where its number sits on the body.
    fn badge(self) -> (f64, f64) {
        match self {
            Organ::Brain => (150.0, 52.0), Organ::Trachea => (150.0, 128.0), Organ::Lungs => (106.0, 186.0), Organ::Heart => (166.0, 214.0), Organ::Liver => (108.0, 262.0),
            Organ::Stomach => (186.0, 258.0), Organ::SmallIntestine => (150.0, 352.0), Organ::LargeIntestine => (96.0, 352.0), Organ::Kidneys => (124.0, 298.0),
        }
    }
}

fn organs(show: &str) -> Vec<Organ> {
    use Organ::*;
    match show {
        "Digestive system" => vec![Liver, Stomach, SmallIntestine, LargeIntestine],
        "Breathing and circulation" => vec![Trachea, Lungs, Heart],
        _ => vec![Brain, Lungs, Heart, Liver, Stomach, Kidneys, SmallIntestine, LargeIntestine],
    }
}

fn render(p: &Params) -> String {
    let show = p.s("show");
    let show = SHOWS.iter().find(|s| **s == show).copied().unwrap_or(SHOWS[0]);
    let list = organs(show);
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (760.0, top + 660.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (ox, oy) = (60.0, top + 20.0);
    let t = |x: f64, y: f64| format!("{:.1} {:.1}", ox + x, oy + y);
    let (skin, line) = if print { ("#f4f4f4", "#555555") } else { ("#fbe3cf", "#8a6a55") };
    // the outline: head, neck, torso, arms and legs
    d.circle(ox + 150.0, oy + 60.0, 44.0, skin, line, 2.4);
    d.rect(ox + 136.0, oy + 98.0, 28.0, 28.0, skin, line, 2.4);
    d.path(&format!("M {} Q {} {} L {} Q {} {} L {} Q {} {} Z", t(62.0, 130.0), t(150.0, 108.0), t(238.0, 130.0), t(222.0, 320.0), t(232.0, 380.0), t(222.0, 432.0), t(78.0, 432.0), t(68.0, 380.0), t(78.0, 320.0)), skin, line, 2.4);
    for sgn in [-1.0, 1.0] {
        let m = |x: f64| 150.0 + sgn * (x - 150.0);
        let pts: Vec<(f64, f64)> = [(62.0, 134.0), (36.0, 140.0), (10.0, 330.0), (38.0, 338.0)].iter().map(|(x, y)| (ox + m(*x), oy + y)).collect();
        d.poly(&pts, skin, line, 2.4);
        let legs: Vec<(f64, f64)> = [(80.0, 432.0), (148.0, 432.0), (140.0, 610.0), (92.0, 610.0)].iter().map(|(x, y)| (ox + m(*x), oy + y)).collect();
        d.poly(&legs, skin, line, 2.4);
    }
    let tints = if print { ["#dddddd", "#cfcfcf", "#bfbfbf", "#d8d8d8", "#c6c6c6", "#e4e4e4", "#d2d2d2", "#bdbdbd", "#ededed"] } else { ["#f4b6c8", "#cfe0fa", "#e07a7a", "#b5785a", "#f0b07a", "#c9a0dc", "#f2c98a", "#9fd49f", "#cfd8dc"] };
    let has = |o: Organ| list.contains(&o);
    if has(Organ::Brain) {
        d.raw(&format!(r##"<ellipse cx="{:.1}" cy="{:.1}" rx="29" ry="22" fill="{}" stroke="{line}" stroke-width="2"/>"##, ox + 150.0, oy + 52.0, tints[0]));
    }
    if has(Organ::Trachea) {
        d.line(ox + 150.0, oy + 100.0, ox + 150.0, oy + 168.0, "#9aa5b1", 7.0);
    }
    if has(Organ::Lungs) {
        for x in [112.0, 188.0] {
            d.raw(&format!(r##"<ellipse cx="{:.1}" cy="{:.1}" rx="34" ry="56" fill="{}" stroke="{line}" stroke-width="2"/>"##, ox + x, oy + 196.0, tints[1]));
        }
    }
    if has(Organ::Heart) {
        d.path(&format!("M {} C {} {} {} C {} {} {} Z", t(162.0, 240.0), t(134.0, 220.0), t(150.0, 192.0), t(162.0, 208.0), t(174.0, 192.0), t(194.0, 220.0), t(162.0, 240.0)), tints[2], line, 2.0);
    }
    if has(Organ::Kidneys) {
        for x in [124.0, 176.0] {
            d.raw(&format!(r##"<ellipse cx="{:.1}" cy="{:.1}" rx="9" ry="15" fill="{}" stroke="{line}" stroke-width="2"/>"##, ox + x, oy + 294.0, tints[5]));
        }
    }
    if has(Organ::Liver) {
        d.raw(&format!(r##"<ellipse cx="{:.1}" cy="{:.1}" rx="42" ry="21" fill="{}" stroke="{line}" stroke-width="2"/>"##, ox + 118.0, oy + 266.0, tints[3]));
    }
    if has(Organ::Stomach) {
        d.raw(&format!(r##"<ellipse cx="{:.1}" cy="{:.1}" rx="28" ry="20" fill="{}" stroke="{line}" stroke-width="2" transform="rotate(-20 {:.1} {:.1})"/>"##, ox + 182.0, oy + 262.0, tints[4], ox + 182.0, oy + 262.0));
    }
    if has(Organ::LargeIntestine) {
        d.polyline(&[(ox + 98.0, oy + 386.0), (ox + 98.0, oy + 314.0), (ox + 202.0, oy + 314.0), (ox + 202.0, oy + 386.0)], tints[7], 14.0);
    }
    if has(Organ::SmallIntestine) {
        let mut pts = Vec::new();
        for i in 0..9 {
            pts.push((ox + 116.0 + (i % 2) as f64 * 68.0, oy + 326.0 + i as f64 * 7.0));
        }
        d.polyline(&pts, tints[6], 7.0);
    }
    // the numbers sit on their organs
    for (i, o) in list.iter().enumerate() {
        let (x, y) = o.badge();
        d.circle(ox + x, oy + y, 12.0, "#fff", INK, 1.8);
        chart::text(&mut d, ox + x, oy + y + fs * 0.35, fs * 0.9, "middle", &(i + 1).to_string());
    }
    // the list of names
    let (lx, ly) = (440.0, top + 120.0);
    chart::text(&mut d, lx, ly - 30.0, fs * 1.05, "start", "Name the organs");
    for (i, o) in list.iter().enumerate() {
        let y = ly + i as f64 * fs * 2.6;
        chart::text(&mut d, lx, y, fs, "start", &format!("{}.", i + 1));
        if answers {
            chart::text(&mut d, lx + 30.0, y, fs, "start", o.name());
        } else {
            d.line(lx + 30.0, y + 3.0, lx + 260.0, y + 3.0, INK, 1.0);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn each_view_lists_its_organs() {
        assert_eq!(organs(SHOWS[0]).len(), 8);
        assert_eq!(organs(SHOWS[1]).len(), 4);
        assert_eq!(organs(SHOWS[2]).len(), 3);
        assert_eq!(organs("Skeleton").len(), 8, "an unknown view falls back to all organs");
        let svg = render(&defaults());
        for n in ["Brain", "Lungs", "Heart", "Liver", "Stomach", "Kidneys", "Small intestine", "Large intestine"] {
            assert!(svg.contains(&format!(">{n}<")), "{n}");
        }
    }

    #[test]
    fn student_copy_blanks_the_names() {
        for show in SHOWS {
            let mut p = defaults();
            p.set_s("show", show);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            let first = organs(show)[0].name();
            assert!(key.contains(&format!(">{first}<")) && !student.contains(&format!(">{first}<")) && student.contains(">1<"), "{show}");
            assert_eq!(size_of(&student), size_of(&key), "{show}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        for show in SHOWS {
            let mut p = defaults();
            p.set_s("show", show);
            p.set_s("style", "Print");
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("body-map").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in ["", "\u{0}", "<b>", "nan"] {
            let mut p = defaults();
            p.set_s("show", v);
            p.set_s("title", v);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("<b>"), "{v:?}");
        }
    }
}
