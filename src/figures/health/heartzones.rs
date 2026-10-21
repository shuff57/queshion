//! Heart-rate training zones: five bands from a maximum heart rate (220 minus age, or worked from
//! the resting rate with the heart-rate reserve method). The age (and resting rate) are the
//! question; the maximum and the beats-per-minute ranges are the answers, so the student copy
//! (Show answers off) draws the same bars and leaves the ranges blank.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
/// (name, low %, high %, how it feels)
const ZONES: [(&str, f64, f64, &str); 5] = [
    ("Very light", 50.0, 60.0, "Easy, can chat freely"),
    ("Light", 60.0, 70.0, "Comfortable, burns fat"),
    ("Moderate", 70.0, 80.0, "Breathing deeper, can talk"),
    ("Hard", 80.0, 90.0, "Short sentences only"),
    ("Maximum", 90.0, 100.0, "All out, only for moments"),
];

pub fn def() -> FigureDef {
    FigureDef {
        id: "heart-zones",
        subject: Subject::Health,
        title: "Heart-rate zones",
        blurb: "Five heart-rate training zones worked out from age (and optionally resting heart rate), with the maximum and the beats per minute of each zone, or blank ranges for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    Some(match id {
        "heart-zones" => vec![
            Example { name: "A 16 year old", params: vec![] },
            Example { name: "With resting heart rate", params: vec![("resting", Value::I(60))] },
            Example { name: "An adult", params: vec![("age", Value::I(45)), ("resting", Value::I(70))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::int("age", "Age (years)", 5, 100, 16),
        ParamDef::int("resting", "Resting heart rate (0 to use the simple method)", 0, 120, 0).hint("With a resting rate, each zone is worked as resting + share of (maximum - resting)"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the bars and the given age and leaves the maximum and the ranges blank for students"),
    ]
}

/// The heart rate for `pct` percent of effort: simple (share of the maximum) or by heart-rate reserve.
pub fn rate_at(max: f64, resting: f64, pct: f64) -> f64 {
    if resting > 0.0 { resting + pct / 100.0 * (max - resting) } else { pct / 100.0 * max }
}

fn render(p: &Params) -> String {
    let age = p.i("age").clamp(5, 100) as f64;
    let resting = p.i("resting").clamp(0, 120) as f64;
    let max = 220.0 - age;
    if resting >= max {
        return chart::message("The resting heart rate must be below the maximum heart rate");
    }
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (980.0, top + 500.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let method = if resting > 0.0 { format!("Heart-rate reserve method, resting rate {} beats a minute", resting as i64) } else { "Simple method: a share of the maximum heart rate".to_string() };
    chart::text(&mut d, 40.0, top + 30.0, fs, "start", &format!("Age: {}", age as i64));
    chart::text(&mut d, 40.0, top + 30.0 + fs * 1.8, fs, "start", &method);
    chart::text(&mut d, 40.0, top + 30.0 + fs * 3.6, fs, "start", "Maximum heart rate:");
    if answers {
        chart::text(&mut d, 40.0 + 170.0, top + 30.0 + fs * 3.6, fs, "start", &format!("220 \u{2212} {} = {} beats a minute", age as i64, max as i64));
    } else {
        d.line(40.0 + 170.0, top + 33.0 + fs * 3.6, 40.0 + 420.0, top + 33.0 + fs * 3.6, INK, 1.0);
    }
    let greens = ["#cfe8d0", "#a9d6ab", "#f2dc8f", "#f0b07a", "#e07a7a"];
    let greys = ["#f2f2f2", "#e0e0e0", "#cecece", "#bcbcbc", "#aaaaaa"];
    let (x0, bar_max) = (230.0, 380.0);
    let y0 = top + 150.0;
    chart::text(&mut d, 40.0, y0 - 14.0, fs, "start", "Zone");
    chart::text(&mut d, x0, y0 - 14.0, fs, "start", "Effort");
    chart::text(&mut d, x0 + bar_max + 30.0, y0 - 14.0, fs, "start", "Beats a minute");
    for (i, (name, lo, hi, feel)) in ZONES.iter().enumerate() {
        let y = y0 + (4 - i) as f64 * 66.0;
        chart::text(&mut d, 40.0, y + 22.0, fs * 1.05, "start", name);
        chart::text(&mut d, 40.0, y + 42.0, fs * 0.85, "start", &format!("{}\u{2013}{}% effort", *lo as i64, *hi as i64));
        let len = 210.0 + (bar_max - 210.0) * i as f64 / 4.0;
        d.rect(x0, y, len, 50.0, if print { greys[i] } else { greens[i] }, INK, 1.6);
        chart::text(&mut d, x0 + 12.0, y + 31.0, fs * 0.9, "start", feel);
        let (a, b) = (rate_at(max, resting, *lo).round() as i64, rate_at(max, resting, *hi).round() as i64);
        if answers {
            chart::text(&mut d, x0 + bar_max + 30.0, y + 31.0, fs * 1.1, "start", &format!("{a} to {b}"));
        } else {
            d.line(x0 + bar_max + 30.0, y + 36.0, x0 + bar_max + 180.0, y + 36.0, INK, 1.0);
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
    fn rates_follow_both_methods() {
        assert_eq!(rate_at(200.0, 0.0, 60.0), 120.0);
        assert_eq!(rate_at(200.0, 60.0, 60.0), 144.0);
        assert_eq!(rate_at(200.0, 60.0, 100.0), 200.0);
    }

    #[test]
    fn key_works_the_ranges() {
        let svg = render(&defaults());
        // age 16: max 204; 50% = 102, 60% = 122.4, 80% = 163.2
        assert!(svg.contains("220 \u{2212} 16 = 204 beats a minute") && svg.contains(">102 to 122<") && svg.contains(">163 to 184<") && svg.contains(">184 to 204<"));
        let mut p = defaults();
        p.set_i("resting", 60);
        // 60 + 0.5*144 = 132; 60 + 0.6*144 = 146.4
        assert!(render(&p).contains(">132 to 146<"));
    }

    #[test]
    fn student_copy_blanks_the_ranges() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Age: 16<") && !student.contains("102 to") && key.contains("102 to") && student.contains(">Moderate<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn a_resting_rate_above_the_maximum_is_explained() {
        let mut p = defaults();
        p.set_i("age", 100);
        p.set_i("resting", 120);
        assert!(render(&p).contains("must be below"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("heart-zones").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("must be below"), "{}", ex.name);
        }
        for age in [i64::MIN, -5, 0, 5, 100, 1_000_000, i64::MAX] {
            for resting in [i64::MIN, 0, 60, 500, i64::MAX] {
                let mut p = defaults();
                p.set_i("age", age);
                p.set_i("resting", resting);
                assert!(sane(&render(&p)), "{age} {resting}");
            }
        }
    }
}
