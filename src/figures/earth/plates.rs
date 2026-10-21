//! Plate boundaries: a cross-section of a divergent or convergent boundary (ocean and continent,
//! two continents, or two oceans), or a map view of a transform boundary, with the plates'
//! motion shown by arrows. The drawing and the arrows are the question; the numbered features
//! are named in a list below, so the student copy (Show answers off) leaves each name as a blank
//! line.
use crate::figures::chart;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const KINDS: [&str; 5] = ["Divergent (mid-ocean ridge)", "Convergent: ocean and continent", "Convergent: continent and continent", "Convergent: ocean and ocean", "Transform (map view)"];
const INK: &str = "#111827";
const W: f64 = 760.0;
const H: f64 = 440.0;

pub fn def() -> FigureDef {
    FigureDef {
        id: "plate-boundaries",
        subject: Subject::EarthScience,
        title: "Plate boundaries",
        blurb: "A divergent, convergent or transform plate boundary with the plates' motion and numbered features, named in a list below, or with the names left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "plate-boundaries" => vec![
            Example { name: "Mid-ocean ridge", params: vec![] },
            Example { name: "Subduction under a continent", params: vec![("kind", s(KINDS[1]))] },
            Example { name: "Continents collide", params: vec![("kind", s(KINDS[2]))] },
            Example { name: "Island arc", params: vec![("kind", s(KINDS[3]))] },
            Example { name: "Transform fault", params: vec![("kind", s(KINDS[4]))] },
            Example { name: "Blank for students", params: vec![("kind", s(KINDS[1])), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Boundary", &KINDS, KINDS[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the drawing, the arrows and the numbered markers and leaves each feature's name as a blank line"),
    ]
}

type Pts = Vec<(f64, f64)>;

fn ellipse(d: &mut Doc, cx: f64, cy: f64, rx: f64, ry: f64, fill: &str) {
    d.raw(&format!(r##"<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="{fill}" stroke="{INK}" stroke-width="1.6"/>"##));
}

/// A band under a top profile: the top points, then the same points `thick` lower, reversed.
fn band(top: &[(f64, f64)], thick: f64) -> Pts {
    let mut p: Pts = top.to_vec();
    p.extend(top.iter().rev().map(|(x, y)| (*x, y + thick)));
    p
}

struct Palette {
    mantle: &'static str,
    crust: &'static str,
    slab: &'static str,
    water: &'static str,
    magma: &'static str,
}

fn render(p: &Params) -> String {
    let kind = KINDS.iter().position(|k| *k == p.s("kind")).unwrap_or(0);
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fills = Fills::new(p);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let pal = if print {
        Palette { mantle: "#d4d4d4", crust: "#8c8c8c", slab: "#6e6e6e", water: "#ffffff", magma: "#3c3c3c" }
    } else {
        Palette { mantle: "#e8a96a", crust: "#a58559", slab: "#7b6a55", water: "#b9d8f0", magma: "#d64a3a" }
    };
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let mut d = Doc::new(W, H + top + 200.0);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, W / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    d.raw(&format!(r##"<g transform="translate(0 {top})">"##));
    let features: Vec<(&str, f64, f64)>;
    let poly = |b: &mut Doc, pts: &[(f64, f64)], fill: &str| {
        b.poly(pts, fill, INK, 1.6);
    };
    let arrow = |b: &mut Doc, x1: f64, y1: f64, x2: f64, y2: f64| {
        b.arrow(x1, y1, x2, y2, INK, 3.0);
    };
    match kind {
        0 => {
            d.rect(0.0, 130.0, W, H - 130.0, pal.mantle, "none", 0.0);
            poly(&mut d, &[(0.0, 130.0), (W, 130.0), (W, 262.0), (460.0, 250.0), (410.0, 232.0), (380.0, 224.0), (350.0, 232.0), (300.0, 250.0), (0.0, 262.0)], pal.water);
            poly(&mut d, &band(&[(0.0, 262.0), (300.0, 250.0), (350.0, 232.0), (366.0, 227.0)], 62.0), pal.crust);
            poly(&mut d, &band(&[(394.0, 227.0), (410.0, 232.0), (460.0, 250.0), (W, 262.0)], 62.0), pal.crust);
            poly(&mut d, &[(366.0, 227.0), (394.0, 227.0), (440.0, 312.0), (320.0, 312.0)], pal.magma);
            arrow(&mut d, 330.0, 104.0, 150.0, 104.0);
            arrow(&mut d, 430.0, 104.0, 610.0, 104.0);
            arrow(&mut d, 380.0, 395.0, 380.0, 330.0);
            arrow(&mut d, 300.0, 360.0, 170.0, 360.0);
            arrow(&mut d, 460.0, 360.0, 590.0, 360.0);
            features = vec![("Mid-ocean ridge", 380.0, 224.0), ("Rising magma", 380.0, 280.0), ("New oceanic crust", 520.0, 270.0), ("Plate moving away", 200.0, 104.0), ("Mantle convection", 230.0, 360.0)];
        }
        1 => {
            d.rect(0.0, 200.0, W, H - 200.0, pal.mantle, "none", 0.0);
            poly(&mut d, &[(0.0, 130.0), (330.0, 130.0), (352.0, 130.0), (306.0, 296.0), (240.0, 250.0), (0.0, 250.0)], pal.water);
            poly(&mut d, &[(352.0, 130.0), (470.0, 120.0), (540.0, 100.0), (562.0, 56.0), (584.0, 100.0), (660.0, 118.0), (W, 120.0), (W, 252.0), (470.0, 252.0), (352.0, 250.0)], pal.crust);
            // the subducting oceanic plate: a band along a dipping centre line
            let slab = [(0.0, 262.0), (240.0, 268.0), (304.0, 304.0), (420.0, 384.0), (500.0, 450.0)];
            let mut s: Pts = slab.iter().map(|(x, y)| (*x, y - 24.0)).collect();
            s.extend(slab.iter().rev().map(|(x, y)| (*x, y + 24.0)));
            poly(&mut d, &s, pal.slab);
            ellipse(&mut d, 540.0, 236.0, 44.0, 26.0, pal.magma);
            poly(&mut d, &[(536.0, 232.0), (562.0, 70.0), (572.0, 232.0)], pal.magma);
            arrow(&mut d, 60.0, 104.0, 180.0, 104.0);
            arrow(&mut d, 700.0, 80.0, 640.0, 80.0);
            features = vec![("Oceanic plate", 110.0, 262.0), ("Trench", 305.0, 290.0), ("Continental plate", 690.0, 180.0), ("Subduction zone", 400.0, 372.0), ("Magma chamber", 540.0, 238.0), ("Volcano", 562.0, 66.0)];
        }
        2 => {
            d.rect(0.0, 200.0, W, H - 200.0, pal.mantle, "none", 0.0);
            let topline = [(0.0, 135.0), (250.0, 132.0), (310.0, 95.0), (345.0, 62.0), (380.0, 32.0), (415.0, 62.0), (450.0, 95.0), (510.0, 132.0), (W, 135.0)];
            let mut c: Pts = topline.to_vec();
            c.extend([(W, 215.0), (520.0, 215.0), (440.0, 300.0), (380.0, 322.0), (320.0, 300.0), (240.0, 215.0), (0.0, 215.0)]);
            poly(&mut d, &c, pal.crust);
            arrow(&mut d, 60.0, 112.0, 200.0, 112.0);
            arrow(&mut d, 700.0, 112.0, 560.0, 112.0);
            features = vec![("Fold mountains", 380.0, 52.0), ("Thickened crust", 380.0, 280.0), ("Continental plate", 110.0, 175.0), ("Continental plate", 660.0, 175.0), ("Plates colliding", 200.0, 112.0)];
        }
        3 => {
            d.rect(0.0, 230.0, W, H - 230.0, pal.mantle, "none", 0.0);
            poly(&mut d, &[(0.0, 130.0), (W, 130.0), (W, 262.0), (350.0, 264.0), (318.0, 300.0), (260.0, 258.0), (0.0, 252.0)], pal.water);
            let right = [(344.0, 262.0), (430.0, 252.0), (W, 256.0)];
            poly(&mut d, &band(&right, 54.0), pal.crust);
            poly(&mut d, &[(436.0, 252.0), (472.0, 102.0), (506.0, 252.0)], pal.crust);
            poly(&mut d, &[(540.0, 254.0), (572.0, 126.0), (606.0, 254.0)], pal.crust);
            let slab = [(0.0, 272.0), (270.0, 276.0), (330.0, 312.0), (440.0, 390.0), (520.0, 450.0)];
            let mut s: Pts = slab.iter().map(|(x, y)| (*x, y - 22.0)).collect();
            s.extend(slab.iter().rev().map(|(x, y)| (*x, y + 22.0)));
            poly(&mut d, &s, pal.slab);
            ellipse(&mut d, 500.0, 312.0, 50.0, 22.0, pal.magma);
            poly(&mut d, &[(486.0, 306.0), (472.0, 110.0), (500.0, 110.0), (516.0, 306.0)], pal.magma);
            arrow(&mut d, 60.0, 104.0, 170.0, 104.0);
            arrow(&mut d, 700.0, 104.0, 650.0, 104.0);
            features = vec![("Subducting oceanic plate", 120.0, 272.0), ("Trench", 322.0, 296.0), ("Overriding plate", 680.0, 296.0), ("Island arc volcano", 472.0, 112.0), ("Magma", 500.0, 314.0)];
        }
        _ => {
            // a map view: two plates sliding past each other, with a road broken by the fault
            d.rect(0.0, 0.0, W, H, "#f4f1e8", "none", 0.0);
            poly(&mut d, &[(60.0, 90.0), (360.0, 90.0), (360.0, 400.0), (60.0, 400.0)], pal.crust);
            poly(&mut d, &[(400.0, 90.0), (700.0, 90.0), (700.0, 400.0), (400.0, 400.0)], pal.slab);
            d.line(380.0, 70.0, 380.0, 420.0, INK, 4.0);
            d.raw(&format!(r##"<path d="M60,250 L360,250 L400,300 L700,300" fill="none" stroke="#fff" stroke-width="5" stroke-dasharray="14 8"/>"##));
            arrow(&mut d, 210.0, 330.0, 210.0, 150.0);
            arrow(&mut d, 550.0, 160.0, 550.0, 340.0);
            d.raw(&format!(r##"<path d="M380,338 l9,-26 l-23,16 l28,0 l-23,-16Z" fill="#fff" stroke="{INK}" stroke-width="1.6"/>"##));
            features = vec![("Transform fault", 380.0, 120.0), ("Plate moving north", 210.0, 170.0), ("Plate moving south", 550.0, 330.0), ("Road broken by the fault", 400.0, 296.0), ("Earthquake focus", 380.0, 325.0)];
        }
    }
    // numbered markers on the drawing
    for (i, (_, x, y)) in features.iter().enumerate() {
        d.circle(*x, *y, 12.0, "#fff", INK, 1.8);
        chart::text(&mut d, *x, y + fs * 0.36, fs, "middle", &(i + 1).to_string());
    }
    d.raw("</g>");
    // the list of features
    let ly = H + top + 26.0;
    let cols = 2;
    for (i, (name, _, _)) in features.iter().enumerate() {
        let (c, r) = (i % cols, i / cols);
        let x = 30.0 + c as f64 * 370.0;
        let y = ly + r as f64 * fs * 2.3;
        d.circle(x + 12.0, y - fs * 0.35, 12.0, "#fff", INK, 1.6);
        chart::text(&mut d, x + 12.0, y, fs, "middle", &(i + 1).to_string());
        if answers {
            chart::text(&mut d, x + 32.0, y, fs, "start", name);
        } else {
            d.line(x + 32.0, y + 3.0, x + 340.0, y + 3.0, INK, 1.0);
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
    fn every_boundary_names_its_features() {
        for (kind, names) in [
            (KINDS[0], vec!["Mid-ocean ridge", "Rising magma"]),
            (KINDS[1], vec!["Trench", "Volcano", "Subduction zone"]),
            (KINDS[2], vec!["Fold mountains", "Thickened crust"]),
            (KINDS[3], vec!["Island arc volcano", "Overriding plate"]),
            (KINDS[4], vec!["Transform fault", "Earthquake focus"]),
        ] {
            let mut p = defaults();
            p.set_s("kind", kind);
            let svg = render(&p);
            for n in names {
                assert!(svg.contains(&format!(">{n}<")), "{kind}: {n}");
            }
        }
    }

    #[test]
    fn student_copy_keeps_the_numbers_but_not_the_names() {
        for kind in KINDS {
            let mut p = defaults();
            p.set_s("kind", kind);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert!(student.contains(">1<") && student.contains(">5<"), "{kind}");
            assert!(!student.contains("Mid-ocean") && !student.contains("Trench") && !student.contains("fault") && !student.contains("Fold mountains"), "{kind}");
            assert!(student.len() < key.len());
            assert_eq!(size_of(&student), size_of(&key), "{kind}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        for kind in KINDS {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("kind", kind);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("plate-boundaries").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for title in ["", "\u{0}", "<script>", &"x".repeat(5000)] {
            for kind in KINDS {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("title", title);
                    p.set_s("kind", kind);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg) && !svg.contains("<script>"), "{title:?}");
                }
            }
        }
        let mut p = defaults();
        p.set_s("kind", "nonsense");
        assert!(sane(&render(&p)));
    }
}
