//! The layers of the Earth: concentric rings drawn to scale (the thin crust is widened so it can be
//! seen, and labelled so) beside a table of each layer's depth, thickness and state. The rings
//! are the question; the student copy (Show answers off) leaves one column of the table, chosen
//! with Leave blank, as empty cells.
use super::polar;
use crate::figures::chart;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const VIEWS: [&str; 2] = ["Whole circle", "Cutaway"];
const HIDES: [&str; 3] = ["Names", "Thicknesses", "States"];
const INK: &str = "#111827";
const RADIUS: f64 = 6371.0;
/// (name, depth of the top, depth of the bottom, state) in km.
const LAYERS: [(&str, f64, f64, &str); 4] = [("Crust", 0.0, 35.0, "Solid"), ("Mantle", 35.0, 2890.0, "Solid, flows slowly"), ("Outer core", 2890.0, 5150.0, "Liquid"), ("Inner core", 5150.0, 6371.0, "Solid")];

pub fn def() -> FigureDef {
    FigureDef {
        id: "earth-layers",
        subject: Subject::EarthScience,
        title: "Layers of the Earth",
        blurb: "The crust, mantle, outer core and inner core as concentric rings with their depths, thicknesses and states, with names, thicknesses or states left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "earth-layers" => vec![
            Example { name: "Whole circle", params: vec![] },
            Example { name: "Cutaway", params: vec![("view", s(VIEWS[1]))] },
            Example { name: "Blank names", params: vec![("show_answers", Value::B(false))] },
            Example { name: "Blank states", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDES[2]))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("view", "View", &VIEWS, VIEWS[0]),
        ParamDef::bool("widen", "Widen the crust so it can be seen", true),
        ParamDef::choice("hide", "Leave blank in the student copy", &HIDES, HIDES[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the rings and the numbered labels and leaves the chosen column of the table empty"),
    ]
}

fn km(v: f64) -> String {
    let s = format!("{}", v.round() as i64);
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A ring from `r0` out to `r1`: a full annulus, or three quarters of one with the top-right quadrant cut away.
fn ring(r0: f64, r1: f64, cx: f64, cy: f64, cut: bool) -> String {
    if !cut {
        return if r0 <= 0.0 {
            format!("M{:.2},{cy:.2} m-{r1:.2},0 a{r1:.2},{r1:.2} 0 1 0 {:.2},0 a{r1:.2},{r1:.2} 0 1 0 -{:.2},0Z", cx, 2.0 * r1, 2.0 * r1)
        } else {
            format!(
                "M{:.2},{cy:.2} m-{r1:.2},0 a{r1:.2},{r1:.2} 0 1 0 {:.2},0 a{r1:.2},{r1:.2} 0 1 0 -{:.2},0Z M{:.2},{cy:.2} m-{r0:.2},0 a{r0:.2},{r0:.2} 0 1 1 {:.2},0 a{r0:.2},{r0:.2} 0 1 1 -{:.2},0Z",
                cx, 2.0 * r1, 2.0 * r1, cx, 2.0 * r0, 2.0 * r0
            )
        };
    }
    let (o90, o0) = (polar(cx, cy, r1, 90.0), polar(cx, cy, r1, 0.0));
    if r0 <= 0.0 {
        format!("M{:.2},{:.2} A{r1:.2},{r1:.2} 0 1 0 {:.2},{:.2} L{cx:.2},{cy:.2}Z", o90.0, o90.1, o0.0, o0.1)
    } else {
        let (i0, i90) = (polar(cx, cy, r0, 0.0), polar(cx, cy, r0, 90.0));
        format!("M{:.2},{:.2} A{r1:.2},{r1:.2} 0 1 0 {:.2},{:.2} L{:.2},{:.2} A{r0:.2},{r0:.2} 0 1 1 {:.2},{:.2}Z", o90.0, o90.1, o0.0, o0.1, i0.0, i0.1, i90.0, i90.1)
    }
}

fn render(p: &Params) -> String {
    let answers = p.b(SHOW_ANSWERS);
    let hide = HIDES.iter().position(|h| *h == p.s("hide")).unwrap_or(0);
    let cut = p.s("view") == VIEWS[1];
    let fills = Fills::new(p);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (1090.0, 470.0 + top);
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (cx, cy, rr) = (240.0, 235.0 + top, 200.0);
    let scale = rr / RADIUS;
    let widen = p.b("widen");
    // page radius of the bottom of each layer
    let base: Vec<f64> = LAYERS
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if widen {
                if i == 0 { rr * 0.93 } else { (RADIUS - l.2) / (RADIUS - LAYERS[0].2) * rr * 0.93 }
            } else {
                rr - l.2 * scale
            }
        })
        .collect();
    let colours = ["#8a7355", "#d9893f", "#e6c24a", "#d65f4a"];
    for i in 0..4 {
        let (r1, r0) = (if i == 0 { rr } else { base[i - 1] }, base[i].max(0.0));
        let fill = if fills.is_print() { fills.series(i) } else { colours[i].to_string() };
        d.path(&ring(r0, r1, cx, cy, cut), &fill, INK, 1.6);
    }
    // numbered markers on the rings
    for i in 0..4 {
        let (r1, r0) = (if i == 0 { rr } else { base[i - 1] }, base[i].max(0.0));
        let (x, y) = if i == 0 { polar(cx, cy, rr + 20.0, 225.0) } else { polar(cx, cy, (r0 + r1) / 2.0, 225.0) };
        let r = 11.0;
        d.circle(x, y, r, "#fff", INK, 1.6);
        chart::text(&mut d, x, y + fs * 0.36, fs, "middle", &(i + 1).to_string());
    }
    // the table
    let cols = [34.0, 112.0, 120.0, 130.0, 170.0];
    let tx = 490.0;
    let tw: f64 = cols.iter().sum();
    let row = fs * 2.5;
    let ty = 60.0 + top;
    let xs: Vec<f64> = cols.iter().scan(tx, |a, c| { let v = *a; *a += c; Some(v) }).collect();
    d.rect(tx, ty, tw, row, if fills.is_print() { "#e5e5e5" } else { "#f4eee2" }, INK, 1.6);
    for (i, t) in ["", "Layer", "Depth (km)", "Thickness (km)", "State"].iter().enumerate() {
        if !t.is_empty() {
            chart::text(&mut d, xs[i] + 8.0, ty + row * 0.62, fs * 0.9, "start", t);
        }
    }
    for (i, l) in LAYERS.iter().enumerate() {
        let y = ty + row * (i as f64 + 1.0);
        chart::text(&mut d, xs[0] + 8.0, y + row * 0.62, fs, "start", &(i + 1).to_string());
        let thick = l.2 - l.1;
        let cells = [
            (1, l.0.to_string(), hide == 0),
            (2, format!("{} \u{2013} {}", km(l.1), km(l.2)), false),
            (3, if i == 0 { "5 \u{2013} 70 (avg 35)".to_string() } else { km(thick) }, hide == 1),
            (4, l.3.to_string(), hide == 2),
        ];
        for (c, text, blank) in cells {
            if blank && !answers {
                d.rect(xs[c] + 5.0, y + 6.0, cols[c] - 10.0, row - 12.0, "#fff", INK, 1.0);
            } else {
                chart::text(&mut d, xs[c] + 8.0, y + row * 0.62, if c == 4 && l.3.len() > 12 { fs * 0.82 } else if c == 3 && i == 0 { fs * 0.85 } else { fs }, "start", &text);
            }
        }
        d.line(tx, y, tx + tw, y, "#c9ced6", 0.8);
    }
    let bottom = ty + row * 5.0;
    for x in xs.iter().skip(1) {
        d.line(*x, ty, *x, bottom, INK, 1.2);
    }
    d.rect(tx, ty, tw, bottom - ty, "none", INK, 1.8);
    if widen {
        chart::text(&mut d, tx, bottom + fs * 2.0, fs * 0.9, "start", "The crust is drawn about ten times thicker than its true scale.");
    }
    chart::text(&mut d, tx, bottom + fs * (if widen { 3.6 } else { 2.0 }), fs * 0.9, "start", &format!("Radius of the Earth: {} km", km(RADIUS)));
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn layers_tile_the_radius_without_gaps() {
        assert_eq!(LAYERS[0].1, 0.0);
        for w in LAYERS.windows(2) {
            assert_eq!(w[0].2, w[1].1, "{} meets {}", w[0].0, w[1].0);
        }
        assert_eq!(LAYERS[3].2, RADIUS);
        assert_eq!(km(6371.0), "6,371");
        assert_eq!(km(35.0), "35");
    }

    #[test]
    fn key_fills_the_table() {
        let svg = render(&defaults());
        for t in [">Crust<", ">Mantle<", ">Outer core<", ">Inner core<", ">2,855<", ">2,260<", ">1,221<", ">Liquid<", "35 \u{2013} 2,890"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn student_copy_blanks_the_chosen_column() {
        for (hide, gone, kept) in [(HIDES[0], ">Mantle<", ">2,855<"), (HIDES[1], ">2,855<", ">Mantle<"), (HIDES[2], ">Liquid<", ">Mantle<")] {
            let mut p = defaults();
            p.set_s("hide", hide);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert!(!student.contains(gone) && student.contains(kept) && key.contains(gone), "{hide}");
            assert_eq!(size_of(&student), size_of(&key));
        }
    }

    #[test]
    fn views_and_scale_options_draw() {
        for view in VIEWS {
            for widen in [true, false] {
                let mut p = defaults();
                p.set_s("view", view);
                p.set_b("widen", widen);
                let svg = render(&p);
                assert!(sane(&svg) && svg.matches("<path").count() >= 4, "{view} {widen}");
                assert_eq!(svg.contains("ten times thicker"), widen);
            }
        }
    }

    #[test]
    fn print_style_is_grey() {
        for view in VIEWS {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("view", view);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("earth-layers").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for title in ["", "\u{0}", "<script>", &"x".repeat(5000)] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("title", title);
                p.set_b(SHOW_ANSWERS, answers);
                assert!(sane(&render(&p)), "{title:?}");
            }
        }
    }
}
