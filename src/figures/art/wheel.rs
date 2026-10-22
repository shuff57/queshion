//! A twelve-colour wheel (artist's red-yellow-blue wheel) with an optional colour scheme marked on
//! it. The wheel is the question; the colour names, the kind of each colour and the scheme are the
//! answers, so the student copy (Show answers off) draws the same wheel with blank lines.
use crate::figures::chart;
use crate::figures::earth::polar;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const NAMES: [&str; 12] = ["Yellow", "Yellow-orange", "Orange", "Red-orange", "Red", "Red-violet", "Violet", "Blue-violet", "Blue", "Blue-green", "Green", "Yellow-green"];
const HEX: [&str; 12] = ["#fde72e", "#fcc22b", "#f7931e", "#ef5a26", "#e0222a", "#b3195f", "#702a8c", "#4a3e9a", "#1f5bb5", "#16928f", "#2ca24a", "#a4cc3a"];
const SCHEMES: [&str; 5] = ["None", "Complementary", "Triadic", "Analogous", "Split complementary"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "colour-wheel",
        subject: Subject::Art,
        title: "Colour wheel",
        blurb: "A twelve-colour wheel with primary, secondary and tertiary colours, and an optional colour scheme marked on it, or blank names for students to fill in.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "colour-wheel" => vec![
            Example { name: "The wheel", params: vec![] },
            Example { name: "Complementary", params: vec![("scheme", s(SCHEMES[1])), ("base", Value::I(8))] },
            Example { name: "Triadic", params: vec![("scheme", s(SCHEMES[2])), ("base", Value::I(0))] },
            Example { name: "Analogous", params: vec![("scheme", s(SCHEMES[3])), ("base", Value::I(4))] },
            Example { name: "Blank for students", params: vec![("scheme", s(SCHEMES[1])), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("scheme", "Colour scheme to mark", &SCHEMES, SCHEMES[0]),
        ParamDef::int("base", "Starting colour (0 is yellow, going round the wheel)", 0, 11, 0).hint("Counts the twelve colours from yellow, through orange, red, violet, blue and green"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the wheel and leaves the colour names and the scheme blank for students"),
    ]
}

/// The wheel positions that make up a scheme from `base`.
pub fn scheme_steps(scheme: &str, base: usize) -> Vec<usize> {
    let offs: &[usize] = match scheme {
        "Complementary" => &[0, 6],
        "Triadic" => &[0, 4, 8],
        "Analogous" => &[0, 1, 11],
        "Split complementary" => &[0, 5, 7],
        _ => &[],
    };
    offs.iter().map(|o| (base + o) % 12).collect()
}

/// Primary, secondary or tertiary, by position on the wheel.
pub fn kind(i: usize) -> &'static str {
    match i % 12 {
        0 | 4 | 8 => "primary",
        2 | 6 | 10 => "secondary",
        _ => "tertiary",
    }
}

fn render(p: &Params) -> String {
    let base = p.i("base").clamp(0, 11) as usize;
    let scheme = p.s("scheme").to_string();
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (900.0, top + 560.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (cx, cy, r_out, r_in) = (300.0, top + 270.0, 200.0, 60.0);
    let steps = scheme_steps(&scheme, base);
    for i in 0..12 {
        // yellow at the top, going clockwise
        let a0 = 90.0 - i as f64 * 30.0 + 15.0;
        let a1 = a0 - 30.0;
        let mut pts = vec![polar(cx, cy, r_in, a0)];
        for k in 0..=6 {
            pts.push(polar(cx, cy, r_out, a0 + (a1 - a0) * k as f64 / 6.0));
        }
        pts.push(polar(cx, cy, r_in, a1));
        let fill = if print { ["#f2f2f2", "#d8d8d8", "#b4b4b4", "#8e8e8e"][i % 4].to_string() } else { HEX[i].to_string() };
        d.poly(&pts, &fill, "#fff", 2.0);
        let (x, y) = polar(cx, cy, r_out + 20.0, 90.0 - i as f64 * 30.0);
        if answers {
            let anchor = if x > cx + 5.0 { "start" } else if x < cx - 5.0 { "end" } else { "middle" };
            chart::text(&mut d, x, y + fs * 0.35, fs * 0.9, anchor, NAMES[i]);
        }
    }
    d.circle(cx, cy, r_in, "#fff", INK, 1.5);
    chart::text(&mut d, cx, cy + fs * 0.35, fs, "middle", "12 colours");
    if answers && steps.len() > 1 {
        let pts: Vec<(f64, f64)> = steps.iter().map(|i| polar(cx, cy, r_out - 25.0, 90.0 - *i as f64 * 30.0)).collect();
        d.poly(&pts, "none", INK, 2.4);
        for q in pts {
            d.circle(q.0, q.1, 7.0, "#fff", INK, 2.0);
        }
    }
    // the working
    let (tx, mut y) = (600.0, top + 60.0);
    chart::text(&mut d, tx, y - 20.0, fs * 1.05, "start", "Name each colour");
    for i in 0..12 {
        chart::text(&mut d, tx, y + 4.0, fs * 0.9, "start", &format!("{}.", i + 1));
        if answers {
            chart::text(&mut d, tx + 28.0, y + 4.0, fs * 0.9, "start", &format!("{} ({})", NAMES[i], kind(i)));
        } else {
            d.line(tx + 28.0, y + 7.0, tx + 250.0, y + 7.0, INK, 1.0);
        }
        y += fs * 2.0;
    }
    chart::text(&mut d, tx, y + 14.0, fs * 0.85, "start", "Colour 1 is yellow at the top; they number clockwise.");
    if scheme != "None" {
        chart::text(&mut d, tx, y + 44.0, fs, "start", &format!("Scheme marked: {}", if answers { scheme.as_str() } else { "" }));
        if !answers {
            d.line(tx + 130.0, y + 47.0, tx + 280.0, y + 47.0, INK, 1.0);
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
    fn schemes_pick_wheel_positions() {
        assert_eq!(scheme_steps("Complementary", 8), vec![8, 2]);
        assert_eq!(scheme_steps("Triadic", 0), vec![0, 4, 8]);
        assert_eq!(scheme_steps("Analogous", 0), vec![0, 1, 11]);
        assert_eq!(scheme_steps("Split complementary", 10), vec![10, 3, 5]);
        assert!(scheme_steps("None", 3).is_empty());
        assert_eq!((kind(0), kind(2), kind(1)), ("primary", "secondary", "tertiary"));
    }

    #[test]
    fn key_names_colours_student_does_not() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains(">Red-orange<") && key.contains("Blue (primary)") && !student.contains("Red-orange") && student.contains(">12 colours<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn scheme_is_an_answer() {
        let mut p = defaults();
        p.set_s("scheme", "Triadic");
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(key.contains("Scheme marked: Triadic") && !student.contains("Triadic"));
        assert!(key.matches("<circle").count() > student.matches("<circle").count());
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("scheme", "Triadic");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("colour-wheel").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [-5, 99, i64::MAX, i64::MIN] {
            let mut p = defaults();
            p.set_i("base", v);
            p.set_s("scheme", "Triadic");
            assert!(sane(&render(&p)), "{v}");
        }
        let mut p = defaults();
        p.set_s("scheme", "<b>nan");
        p.set_s("title", "<b>&");
        assert!(sane(&render(&p)));
    }
}
