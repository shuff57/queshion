//! A gear train: gears in a row meshing, drawn to the size of their tooth counts. The tooth counts
//! and the input speed are the question; the gear ratio, the speed of every gear and the direction
//! each turns are the answers, so the student copy (Show answers off) draws the same gears with
//! only the input direction and blank lines for the rest.
use crate::figures::chart;
use crate::figures::earth::polar;
use crate::figures::geography::{rows, trim};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";

pub fn def() -> FigureDef {
    FigureDef {
        id: "gear-train",
        subject: Subject::Health,
        title: "Gear train",
        blurb: "A train of two to four meshing gears drawn from their tooth counts, with the gear ratio, the speed and the direction of each gear worked out, or left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "gear-train" => vec![
            Example { name: "Two gears, slowing down", params: vec![] },
            Example { name: "Speeding up", params: vec![("teeth", s("40, 16")), ("rpm", Value::F(60.0))] },
            Example { name: "Idler gear in the middle", params: vec![("teeth", s("20, 30, 40"))] },
            Example { name: "Four gears", params: vec![("teeth", s("16, 32, 12, 36")), ("rpm", Value::F(240.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("teeth", "Teeth on each gear, driver first", 1, "20\n40").hint("One number per gear in order, from the gear that is turned to the last gear (2 to 4 gears, 8 to 60 teeth). A comma or a new line separates them"),
        ParamDef::float("rpm", "Speed of the first gear (rev/min)", 1.0, 100000.0, 10.0, 120.0),
        ParamDef::bool("clockwise", "First gear turns clockwise", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the gears and the first gear's speed and leaves the ratio, the other speeds and the directions for students"),
    ]
}

/// The speed of gear `i` for a first gear turning at `rpm`: speed times first teeth over its own.
pub fn speed(teeth: &[f64], rpm: f64, i: usize) -> f64 {
    rpm * teeth[0] / teeth[i]
}

fn gear_points(cx: f64, cy: f64, r: f64, teeth: usize, h: f64, phase: f64) -> Vec<(f64, f64)> {
    let step = 360.0 / teeth as f64;
    let mut pts = Vec::with_capacity(teeth * 4);
    for k in 0..teeth {
        let a = phase + k as f64 * step;
        for (da, rad) in [(0.0, r - h), (0.12, r + h), (0.38, r + h), (0.5, r - h)] {
            pts.push(polar(cx, cy, rad, a + da * step));
        }
    }
    pts
}

fn render(p: &Params) -> String {
    let rpm = p.f("rpm");
    if !rpm.is_finite() {
        return chart::message("Give the speed as a number");
    }
    let rpm = rpm.clamp(1.0, 100000.0);
    let spec = p.s("teeth").replace(',', "\n");
    let teeth = match rows(&spec, 1, 2, 4, 8.0, 60.0, "a tooth count") {
        Ok(v) => v.into_iter().map(|r| r[0]).collect::<Vec<f64>>(),
        Err(e) => return chart::message(&e),
    };
    if teeth.iter().any(|t| t.fract() != 0.0) {
        return chart::message("Tooth counts are whole numbers");
    }
    let n = teeth.len();
    let answers = p.b(SHOW_ANSWERS);
    let cw0 = p.b("clockwise");
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let radii0: Vec<f64> = teeth.iter().map(|t| t * 3.0).collect();
    let total: f64 = radii0.iter().map(|r| 2.0 * r).sum();
    let sc = (860.0 / total).min(1.0);
    let radii: Vec<f64> = radii0.iter().map(|r| r * sc).collect();
    let tooth_h = (3.4 * sc).max(2.5);
    let maxr = radii.iter().cloned().fold(0.0, f64::max);
    let w = 940.0;
    let cy = top + 60.0 + maxr + tooth_h;
    let h = cy + maxr + tooth_h + 330.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let used: f64 = radii.iter().map(|r| 2.0 * r).sum();
    let mut x = (w - used) / 2.0;
    let fills = if print { ["#e6e6e6", "#d2d2d2", "#bebebe", "#dcdcdc"] } else { ["#cfe0fa", "#f7d9a8", "#cfe8d0", "#f0c4d4"] };
    let mut centres = Vec::new();
    for i in 0..n {
        let cx = x + radii[i];
        centres.push(cx);
        let count = teeth[i] as usize;
        // a gear and the next one fit tooth to gap at the point where they touch
        let step = 360.0 / count as f64;
        let phase = if i % 2 == 0 { -0.25 * step } else { 180.0 - 0.75 * step };
        d.poly(&gear_points(cx, cy, radii[i], count, tooth_h, phase), fills[i % 4], INK, 1.8);
        d.circle(cx, cy, radii[i] * 0.16, "#fff", INK, 1.6);
        chart::text(&mut d, cx, cy + radii[i] * 0.5 + fs, fs, "middle", &format!("{count} teeth"));
        x += 2.0 * radii[i];
    }
    // the turn of each gear: the first is given, the others are answers
    for i in 0..n {
        if i > 0 && !answers {
            continue;
        }
        let cw = cw0 == (i % 2 == 0);
        let (cx, r) = (centres[i], radii[i] * 0.38);
        let pts: Vec<(f64, f64)> = (0..=14).map(|k| { let a = if cw { 150.0 - 11.0 * k as f64 } else { 30.0 + 11.0 * k as f64 }; polar(cx, cy - radii[i] * 0.1, r, a) }).collect();
        d.polyline(&pts[..14], INK, 2.4);
        d.arrow(pts[13].0, pts[13].1, pts[14].0, pts[14].1, INK, 2.4);
    }
    // the working
    let ry = cy + maxr + tooth_h + 50.0;
    chart::text(&mut d, 40.0, ry, fs * 1.05, "start", &format!("First gear: {} teeth, {} rev/min, {}", teeth[0] as usize, trim(rpm, 3), if cw0 { "clockwise" } else { "anticlockwise" }));
    let mut y = ry + fs * 2.2;
    let line = |d: &mut Doc, label: String, val: String, y: &mut f64| {
        chart::text(d, 40.0, *y, fs, "start", &label);
        if answers {
            chart::text(d, 40.0 + 330.0, *y, fs, "start", &val);
        } else {
            d.line(40.0 + 330.0, *y + 3.0, 40.0 + 560.0, *y + 3.0, INK, 1.0);
        }
        *y += fs * 1.9;
    };
    let ratio = teeth[n - 1] / teeth[0];
    line(&mut d, "Gear ratio (last teeth \u{f7} first teeth):".into(), format!("{} : 1", trim(ratio, 3)), &mut y);
    for i in 1..n {
        line(&mut d, format!("Speed of gear {}:", i + 1), format!("{} rev/min", trim(speed(&teeth, rpm, i), 3)), &mut y);
    }
    let same = (n - 1) % 2 == 0;
    line(&mut d, "Last gear turns:".into(), format!("{} (the same way as the first)", if cw0 == same { "clockwise" } else { "anticlockwise" }).replace(" (the same way as the first)", if same { " (same way as the first)" } else { " (opposite to the first)" }), &mut y);
    line(&mut d, "Does the last gear speed up or slow down?".into(), if ratio > 1.0 { "slows down".into() } else if ratio < 1.0 { "speeds up".into() } else { "same speed".into() }, &mut y);
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn speeds_follow_the_teeth() {
        let t = [20.0, 40.0, 10.0];
        assert_eq!(speed(&t, 120.0, 0), 120.0);
        assert_eq!(speed(&t, 120.0, 1), 60.0);
        assert_eq!(speed(&t, 120.0, 2), 240.0);
    }

    #[test]
    fn key_works_the_train() {
        let svg = render(&defaults());
        for t in [">2 : 1<", ">60 rev/min<", "opposite to the first", "slows down", ">20 teeth<", ">40 teeth<"] {
            assert!(svg.contains(t), "{t}");
        }
        let mut p = defaults();
        p.set_s("teeth", "20, 30, 40");
        let svg = render(&p);
        assert!(svg.contains("same way as the first") && svg.contains(">80 rev/min<") && svg.contains(">2 : 1<") && svg.contains(">60 rev/min<"));
    }

    #[test]
    fn student_copy_keeps_the_gears_only() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">20 teeth<") && !student.contains("rev/min<") && !student.contains("slows down") && key.contains("slows down"));
        assert!(student.contains("First gear: 20 teeth, 120 rev/min, clockwise"));
        assert!(key.matches("<polygon").count() > student.matches("<polygon").count(), "the key draws the other turn arrows");
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        for (spec, needle) in [("", "Give 2 rows"), ("20", "Give 2 rows"), ("20, 5", "tooth count"), ("20, 61", "tooth count"), ("20, 30.5", "whole"), ("a, b", "tooth count")] {
            let mut p = defaults();
            p.set_s("teeth", spec);
            let svg = render(&p);
            assert!(svg.contains(needle), "{spec:?}: {}", &svg[svg.len().saturating_sub(200)..]);
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
        for ex in examples("gear-train").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("Give") && !svg.contains("8 to 60"), "{}", ex.name);
        }
        for v in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300] {
            let mut p = defaults();
            p.set_f("rpm", v);
            assert!(sane(&render(&p)), "{v}");
        }
        for spec in ["60, 60, 60, 60", "8, 8, 8, 8", "\u{0}", "nan, 1e999", "60, 8"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("teeth", spec);
                p.set_b(SHOW_ANSWERS, answers);
                assert!(sane(&render(&p)), "{spec:?}");
            }
        }
    }
}
