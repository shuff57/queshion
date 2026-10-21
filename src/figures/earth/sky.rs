//! The Moon's phases and the Earth's seasons as orbit diagrams. The phases diagram puts the Moon
//! at eight places round the Earth with the lit half facing the Sun, and beside each a small
//! picture of the phase as seen from Earth. The seasons diagram puts the Earth at four places
//! round the Sun with its axis tilted the same way each time. The drawings are the question; the
//! phase names, or the dates and seasons, are the answers, so the student copy (Show answers
//! off) leaves them as empty boxes.
use super::polar;
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MODES: [&str; 2] = ["Moon phases", "Seasons"];
const HEMISPHERES: [&str; 2] = ["Northern hemisphere", "Southern hemisphere"];
const INK: &str = "#111827";
const TILT: f64 = 23.5;
/// Phase names counted anticlockwise from the position between Earth and Sun (new moon).
const PHASES: [&str; 8] = ["New moon", "Waxing crescent", "First quarter", "Waxing gibbous", "Full moon", "Waning gibbous", "Last quarter", "Waning crescent"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "moon-seasons",
        subject: Subject::EarthScience,
        title: "Moon phases and seasons",
        blurb: "The Moon's eight phases or the Earth's four seasons as an orbit diagram with the lit half shaded, with the phase names or the dates and seasons left blank for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("moon-seasons", "hemisphere") => p.s("mode") == MODES[1],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "moon-seasons" => vec![
            Example { name: "Moon phases", params: vec![] },
            Example { name: "Seasons (northern hemisphere)", params: vec![("mode", s(MODES[1]))] },
            Example { name: "Seasons (southern hemisphere)", params: vec![("mode", s(MODES[1])), ("hemisphere", s(HEMISPHERES[1]))] },
            Example { name: "Blank phase names", params: vec![("show_answers", Value::B(false))] },
            Example { name: "Blank seasons", params: vec![("mode", s(MODES[1])), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Diagram", &MODES, MODES[0]),
        ParamDef::choice("hemisphere", "Seasons for", &HEMISPHERES, HEMISPHERES[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the phase names (or the dates and seasons) as empty boxes for students"),
    ]
}

/// The fraction of the Moon's disc lit as seen from Earth when it is `deg` round its orbit,
/// counted anticlockwise from east (full moon) with the Sun to the west.
pub fn lit_fraction(deg: f64) -> f64 {
    (1.0 + deg.to_radians().cos()) / 2.0
}

/// The path of the lit part of a disc seen from Earth: waxing is lit on the right, waning on the left.
pub fn phase_path(cx: f64, cy: f64, r: f64, f: f64, waxing: bool) -> String {
    if f <= 0.001 {
        return String::new();
    }
    if f >= 0.999 {
        return format!("M{:.2},{cy:.2} m-{r:.2},0 a{r:.2},{r:.2} 0 1 0 {:.2},0 a{r:.2},{r:.2} 0 1 0 -{:.2},0Z", cx, 2.0 * r, 2.0 * r);
    }
    let rx = r * (1.0 - 2.0 * f).abs();
    let (limb, term) = if waxing { (1, if f < 0.5 { 0 } else { 1 }) } else { (0, if f < 0.5 { 1 } else { 0 }) };
    format!("M{cx:.2},{:.2} A{r:.2},{r:.2} 0 0 {limb} {cx:.2},{:.2} A{rx:.2},{r:.2} 0 0 {term} {cx:.2},{:.2}Z", cy - r, cy + r, cy - r)
}

struct Style {
    dark: &'static str,
    lit: &'static str,
    sun: &'static str,
    earth_dark: &'static str,
    earth_lit: &'static str,
}

fn label_box(d: &mut Doc, x: f64, y: f64, w: f64, h: f64, lines: &[&str], show: bool, fs: f64) {
    if show {
        for (i, l) in lines.iter().enumerate() {
            let yy = y + fs * 0.35 - (lines.len() as f64 - 1.0) * fs * 0.6 + i as f64 * fs * 1.2;
            chart::text(d, x, yy, fs, "middle", l);
        }
    } else {
        d.rect(x - w / 2.0, y - h / 2.0, w, h, "#fff", INK, 1.2);
    }
}

fn render(p: &Params) -> String {
    let phases = p.s("mode") != MODES[1];
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let st = if print {
        Style { dark: "#555555", lit: "#f2f2f2", sun: "#d9d9d9", earth_dark: "#6a6a6a", earth_lit: "#e5e5e5" }
    } else {
        Style { dark: "#3a3f4b", lit: "#fbf3c6", sun: "#f6c744", earth_dark: "#2a4d7a", earth_lit: "#8fc1ea" }
    };
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (880.0, 690.0 + top);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    if phases {
        let (cx, cy) = (450.0, 345.0 + top);
        // sunlight from the west
        for dy in [-70.0, -35.0, 0.0, 35.0, 70.0] {
            d.arrow(14.0, cy + dy, 84.0, cy + dy, if print { "#777" } else { "#e0a800" }, 2.6);
        }
        chart::text(&mut d, 49.0, cy - 96.0, fs, "middle", "Sunlight");
        d.circle(cx, cy, 150.0, "none", "#9ca3af", 1.6);
        d.raw(&format!(r##"<circle cx="{cx}" cy="{cy}" r="150" fill="none" stroke="#9ca3af" stroke-width="1.6" stroke-dasharray="7 6"/>"##));
        d.circle(cx, cy, 34.0, st.earth_dark, INK, 1.8);
        d.raw(&format!(r##"<path d="M{cx},{:.1} A34,34 0 0 0 {cx},{:.1}Z" fill="{}"/>"##, cy - 34.0, cy + 34.0, st.earth_lit));
        chart::text(&mut d, cx, cy + 54.0, fs, "middle", "Earth");
        for (i, name) in PHASES.iter().enumerate() {
            // position round the orbit: new moon is between the Earth and the Sun (west, 180 degrees)
            let deg = 180.0 + 45.0 * i as f64;
            let (mx, my) = polar(cx, cy, 150.0, deg);
            d.circle(mx, my, 16.0, st.dark, INK, 1.6);
            // the half of the Moon facing the Sun is the west half
            d.raw(&format!(r##"<path d="M{mx:.1},{:.1} A16,16 0 0 0 {mx:.1},{:.1}Z" fill="{}"/>"##, my - 16.0, my + 16.0, st.lit));
            d.circle(mx, my, 16.0, "none", INK, 1.6);
            // as seen from Earth
            let f = lit_fraction(deg);
            let waxing = i < 4;
            let (ix, iy) = polar(cx, cy, 215.0, deg);
            d.circle(ix, iy, 19.0, st.dark, INK, 1.6);
            let path = phase_path(ix, iy, 19.0, f, waxing);
            if !path.is_empty() {
                d.raw(&format!(r##"<path d="{path}" fill="{}"/>"##, st.lit));
            }
            d.circle(ix, iy, 19.0, "none", INK, 1.6);
            let (lx, ly) = polar(cx, cy, 262.0, deg);
            let anchor_w = 130.0;
            let (tx, anchor) = if lx < cx - 20.0 { (lx - 4.0, "end") } else if lx > cx + 20.0 { (lx + 4.0, "start") } else { (lx, "middle") };
            let _ = anchor;
            let box_cx = if lx < cx - 20.0 { tx - anchor_w / 2.0 } else if lx > cx + 20.0 { tx + anchor_w / 2.0 } else { tx };
            let box_cy = if (lx - cx).abs() <= 20.0 { if ly < cy { ly - 12.0 } else { ly + 12.0 } } else { ly };
            let ls: Vec<&str> = if name.contains(' ') && name.len() > 12 { name.splitn(2, ' ').collect() } else { vec![*name] };
            label_box(&mut d, box_cx, box_cy, anchor_w, 34.0, &ls, answers, fs);
        }
        chart::text(&mut d, cx, h - 14.0, fs * 0.9, "middle", "The Moon orbits anticlockwise as seen from above the Earth's north pole. Inner discs: the Moon in its orbit; outer discs: how it looks from Earth.");
    } else {
        let south = p.s("hemisphere") == HEMISPHERES[1];
        let (cx, cy) = (440.0, 345.0 + top);
        d.circle(cx, cy, 46.0, st.sun, INK, 1.8);
        chart::text(&mut d, cx, cy + 5.0, fs, "middle", "Sun");
        d.raw(&format!(r##"<ellipse cx="{cx}" cy="{cy}" rx="310" ry="190" fill="none" stroke="#9ca3af" stroke-width="1.6" stroke-dasharray="7 6"/>"##));
        // the Earth at four places: the north pole tilts toward the Sun on the left
        let spots = [
            ("June solstice", "Northern summer", "Northern winter", (cx - 310.0, cy)),
            ("September equinox", "Northern autumn", "Northern spring", (cx, cy + 190.0)),
            ("December solstice", "Northern winter", "Northern summer", (cx + 310.0, cy)),
            ("March equinox", "Northern spring", "Northern autumn", (cx, cy - 190.0)),
        ];
        let r = 34.0;
        for (name, n_season, s_season, (ex, ey)) in spots {
            let season = if south { s_season.replace("Northern", "Southern") } else { n_season.to_string() };
            // the lit half faces the Sun
            let psi = (-(cy - ey)).atan2(cx - ex).to_degrees();
            d.circle(ex, ey, r, st.earth_dark, INK, 1.8);
            let (a, b) = (polar(ex, ey, r, psi - 90.0), polar(ex, ey, r, psi + 90.0));
            d.raw(&format!(r##"<path d="M{:.1},{:.1} A{r},{r} 0 0 0 {:.1},{:.1}Z" fill="{}"/>"##, a.0, a.1, b.0, b.1, st.earth_lit));
            d.circle(ex, ey, r, "none", INK, 1.8);
            // the axis, tilted 23.5 degrees toward the right of the page
            let t = TILT.to_radians();
            let (ax, ay) = (t.sin(), -t.cos());
            d.line(ex - ax * (r + 16.0), ey - ay * (r + 16.0), ex + ax * (r + 16.0), ey + ay * (r + 16.0), INK, 2.0);
            chart::text(&mut d, ex + ax * (r + 28.0), ey + ay * (r + 24.0), fs * 0.9, "middle", "N");
            let (by, below) = if ey < cy - 10.0 { (ey - r - 58.0, false) } else { (ey + r + 50.0, true) };
            let _ = below;
            label_box(&mut d, ex, by, 170.0, 42.0, &[name, &season], answers, fs);
        }
        chart::text(&mut d, w / 2.0, h - 14.0, fs * 0.9, "middle", "The Earth orbits anticlockwise as seen from above the north pole, and its axis always points the same way in space.");
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn the_lit_fraction_follows_the_phase() {
        assert!(lit_fraction(180.0) < 1e-9, "new moon");
        assert!((lit_fraction(270.0) - 0.5).abs() < 1e-9, "first quarter");
        assert!((lit_fraction(0.0) - 1.0).abs() < 1e-9, "full moon");
        assert!((lit_fraction(90.0) - 0.5).abs() < 1e-9, "last quarter");
        assert!(lit_fraction(225.0) < 0.5 && lit_fraction(315.0) > 0.5);
    }

    #[test]
    fn phase_paths_are_empty_for_new_and_a_circle_for_full() {
        assert!(phase_path(0.0, 0.0, 10.0, 0.0, true).is_empty());
        assert!(phase_path(0.0, 0.0, 10.0, 1.0, true).contains('a'));
        let crescent = phase_path(0.0, 0.0, 10.0, 0.25, true);
        assert!(crescent.contains("A5.00,10.00"), "the terminator is half as wide as the disc: {crescent}");
        assert_ne!(phase_path(0.0, 0.0, 10.0, 0.25, true), phase_path(0.0, 0.0, 10.0, 0.25, false), "waxing and waning are mirrored");
    }

    #[test]
    fn key_names_the_phases_and_seasons() {
        let svg = render(&defaults());
        assert!(svg.matches("<path").count() >= 16, "a lit half for the Earth and each Moon, and seven visible phases");
        for t in [">New moon<", ">Full moon<", ">First<", ">Last quarter<", ">Sunlight<"] {
            assert!(svg.contains(t), "{t}");
        }
        let mut p = defaults();
        p.set_s("mode", MODES[1]);
        let svg = render(&p);
        for t in [">June solstice<", ">Northern summer<", ">March equinox<", ">Northern spring<", ">December solstice<", ">Northern winter<"] {
            assert!(svg.contains(t), "{t}");
        }
        p.set_s("hemisphere", HEMISPHERES[1]);
        let svg = render(&p);
        assert!(svg.contains(">Southern winter<") && !svg.contains(">Northern summer<"));
    }

    #[test]
    fn student_copy_has_empty_boxes() {
        for mode in MODES {
            let mut p = defaults();
            p.set_s("mode", mode);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert!(!student.contains("moon<") && !student.contains("solstice") && !student.contains("quarter") && !student.contains("equinox"), "{mode}");
            assert!(student.matches("<rect").count() > key.matches("<rect").count(), "{mode}");
            assert_eq!(size_of(&student), size_of(&key), "{mode}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        for mode in MODES {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("mode", mode);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("moon-seasons").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for title in ["", "\u{0}", "<script>", &"x".repeat(5000)] {
            for mode in MODES {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("title", title);
                    p.set_s("mode", mode);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg) && !svg.contains("<script>"), "{title:?}");
                }
            }
        }
    }
}
