//! Court and field layouts with numbered positions: a basketball half-court, a soccer pitch with a
//! chosen formation, and a volleyball court. The numbers in the circles are given; what each
//! position is called is the answer, so the student copy (Show answers off) leaves the list of
//! names blank.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const SPORTS: [&str; 3] = ["Basketball", "Soccer", "Volleyball"];
const FORMATIONS: [&str; 3] = ["4-4-2", "4-3-3", "3-5-2"];
const INK: &str = "#111827";

pub fn def() -> FigureDef {
    FigureDef {
        id: "court-layout",
        subject: Subject::Health,
        title: "Court and field layout",
        blurb: "A basketball half-court, soccer pitch (with a formation) or volleyball court with numbered player positions, named in a list, or a blank list for students to name them.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("court-layout", "formation") => p.s("sport") == SPORTS[1],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "court-layout" => vec![
            Example { name: "Basketball positions", params: vec![] },
            Example { name: "Soccer 4-4-2", params: vec![("sport", s(SPORTS[1]))] },
            Example { name: "Soccer 4-3-3", params: vec![("sport", s(SPORTS[1])), ("formation", s(FORMATIONS[1]))] },
            Example { name: "Soccer 3-5-2", params: vec![("sport", s(SPORTS[1])), ("formation", s(FORMATIONS[2]))] },
            Example { name: "Volleyball rotation", params: vec![("sport", s(SPORTS[2]))] },
            Example { name: "Blank for students", params: vec![("sport", s(SPORTS[1])), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("sport", "Sport", &SPORTS, SPORTS[0]),
        ParamDef::choice("formation", "Soccer formation", &FORMATIONS, FORMATIONS[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the court and the numbered circles and leaves the names of the positions blank for students"),
    ]
}

/// A position: its name and where it stands, as fractions of the playing area.
pub struct Spot {
    pub name: &'static str,
    pub x: f64,
    pub y: f64,
}

const fn sp(name: &'static str, x: f64, y: f64) -> Spot {
    Spot { name, x, y }
}

pub fn spots(sport: &str, formation: &str) -> Vec<Spot> {
    match sport {
        "Soccer" => {
            let mut v = vec![sp("Goalkeeper", 0.04, 0.5)];
            match formation {
                "4-3-3" => v.extend([
                    sp("Left back", 0.2, 0.15), sp("Left centre-back", 0.2, 0.38), sp("Right centre-back", 0.2, 0.62), sp("Right back", 0.2, 0.85),
                    sp("Left midfielder", 0.4, 0.22), sp("Central midfielder", 0.37, 0.5), sp("Right midfielder", 0.4, 0.78),
                    sp("Left winger", 0.62, 0.15), sp("Centre forward", 0.66, 0.5), sp("Right winger", 0.62, 0.85),
                ]),
                "3-5-2" => v.extend([
                    sp("Left centre-back", 0.2, 0.2), sp("Central centre-back", 0.17, 0.5), sp("Right centre-back", 0.2, 0.8),
                    sp("Left wing-back", 0.4, 0.07), sp("Left central midfielder", 0.38, 0.3), sp("Defensive midfielder", 0.33, 0.5), sp("Right central midfielder", 0.38, 0.7), sp("Right wing-back", 0.4, 0.93),
                    sp("Left striker", 0.64, 0.36), sp("Right striker", 0.64, 0.64),
                ]),
                _ => v.extend([
                    sp("Left back", 0.2, 0.15), sp("Left centre-back", 0.2, 0.38), sp("Right centre-back", 0.2, 0.62), sp("Right back", 0.2, 0.85),
                    sp("Left midfielder", 0.4, 0.12), sp("Left central midfielder", 0.4, 0.38), sp("Right central midfielder", 0.4, 0.62), sp("Right midfielder", 0.4, 0.88),
                    sp("Left striker", 0.64, 0.36), sp("Right striker", 0.64, 0.64),
                ]),
            }
            v
        }
        "Volleyball" => vec![
            sp("Right back (server)", 0.17, 0.82), sp("Right front", 0.38, 0.82), sp("Middle front", 0.38, 0.5), sp("Left front", 0.38, 0.18), sp("Left back", 0.17, 0.18), sp("Middle back", 0.17, 0.5),
        ],
        _ => vec![
            sp("Point guard", 0.5, 0.42), sp("Shooting guard", 0.8, 0.58), sp("Small forward", 0.2, 0.58), sp("Power forward", 0.68, 0.84), sp("Center", 0.32, 0.84),
        ],
    }
}

fn draw_court(d: &mut Doc, sport: &str, x0: f64, y0: f64, ww: f64, hh: f64, fill: &str) {
    let line = |d: &mut Doc, a: f64, b: f64, c: f64, e: f64| { d.line(a, b, c, e, "#fff", 3.0); };
    d.rect(x0, y0, ww, hh, fill, "#fff", 4.0);
    match sport {
        "Soccer" => {
            let (cx, cy) = (x0 + ww / 2.0, y0 + hh / 2.0);
            let m = ww / 105.0;
            line(d, cx, y0, cx, y0 + hh);
            d.circle(cx, cy, 9.15 * m, "none", "#fff", 3.0);
            d.circle(cx, cy, 3.0, "#fff", "#fff", 1.0);
            for side in [0.0, 1.0] {
                let sgn = if side == 0.0 { 1.0 } else { -1.0 };
                let gx = if side == 0.0 { x0 } else { x0 + ww };
                let (pa_w, pa_d, ga_w, ga_d) = (40.3 * m, 16.5 * m, 18.3 * m, 5.5 * m);
                d.rect(if side == 0.0 { gx } else { gx - pa_d }, cy - pa_w / 2.0, pa_d, pa_w, "none", "#fff", 3.0);
                d.rect(if side == 0.0 { gx } else { gx - ga_d }, cy - ga_w / 2.0, ga_d, ga_w, "none", "#fff", 3.0);
                d.circle(gx + sgn * 11.0 * m, cy, 3.0, "#fff", "#fff", 1.0);
                d.rect(if side == 0.0 { gx - 14.0 } else { gx }, cy - 3.66 * m, 14.0, 7.32 * m, "none", "#fff", 3.0);
            }
        }
        "Volleyball" => {
            let cx = x0 + ww / 2.0;
            d.line(cx, y0 - 14.0, cx, y0 + hh + 14.0, "#fff", 6.0);
            line(d, cx - ww / 6.0, y0, cx - ww / 6.0, y0 + hh);
            line(d, cx + ww / 6.0, y0, cx + ww / 6.0, y0 + hh);
        }
        _ => {
            // a half court with the basket at the bottom: 15 m wide, 14 m deep
            let m = ww / 15.0;
            let (cx, by) = (x0 + ww / 2.0, y0 + hh - 1.575 * m);
            let key_w = 4.9 * m;
            d.rect(cx - key_w / 2.0, y0 + hh - 5.8 * m, key_w, 5.8 * m, "none", "#fff", 3.0);
            let ft = y0 + hh - 5.8 * m;
            d.path(&format!("M {:.2} {:.2} A {:.2} {:.2} 0 0 1 {:.2} {:.2}", cx - 1.8 * m, ft, 1.8 * m, 1.8 * m, cx + 1.8 * m, ft), "none", "#fff", 3.0);
            let r = 6.75 * m;
            let off = 6.6 * m;
            let up = (r * r - off * off).max(0.0).sqrt();
            line(d, cx - off, y0 + hh, cx - off, by - up);
            line(d, cx + off, y0 + hh, cx + off, by - up);
            d.path(&format!("M {:.2} {:.2} A {r:.2} {r:.2} 0 0 1 {:.2} {:.2}", cx - off, by - up, cx + off, by - up), "none", "#fff", 3.0);
            d.line(cx - 0.9 * m, y0 + hh - 1.2 * m, cx + 0.9 * m, y0 + hh - 1.2 * m, "#fff", 4.0);
            d.circle(cx, by, 0.23 * m, "none", "#fff", 3.0);
            d.path(&format!("M {:.2} {:.2} A {:.2} {:.2} 0 0 1 {:.2} {:.2}", cx - 4.0 * m, y0, 4.0 * m, 4.0 * m, cx + 4.0 * m, y0).replace("0 0 1", "0 0 0"), "none", "#fff", 3.0);
        }
    }
}

fn render(p: &Params) -> String {
    let sport = p.s("sport");
    let sport = SPORTS.iter().find(|s| **s == sport).copied().unwrap_or(SPORTS[0]);
    let formation = p.s("formation");
    let formation = FORMATIONS.iter().find(|s| **s == formation).copied().unwrap_or(FORMATIONS[0]);
    let list = spots(sport, formation);
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (cw, ch) = match sport {
        "Soccer" => (840.0, 544.0),
        "Volleyball" => (720.0, 360.0),
        _ => (495.0, 462.0),
    };
    let w = 960.0f64.max(cw + 120.0);
    let rows = (list.len() + 1) / 2;
    let h = top + 40.0 + ch + 50.0 + rows as f64 * fs * 2.0 + 40.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (x0, y0) = ((w - cw) / 2.0, top + 30.0);
    let (green, ring, num_fill) = if print { ("#d9d9d9", "#fff", "#fff") } else { ("#4f9d5d", "#fff", "#fff") };
    let court_fill = match sport { "Basketball" => if print { "#d9d9d9" } else { "#d9a566" }, "Volleyball" => if print { "#d9d9d9" } else { "#5b9bd5" }, _ => green };
    draw_court(&mut d, sport, x0, y0, cw, ch, court_fill);
    if sport == "Volleyball" {
        chart::text(&mut d, x0 + cw / 2.0, y0 - 18.0, fs * 0.9, "middle", "net");
    }
    // volleyball and soccer players stand in the left half; the numbers are fixed
    let (px0, pw) = match sport { "Soccer" => (x0, cw), "Volleyball" => (x0, cw), _ => (x0, cw) };
    for (i, s) in list.iter().enumerate() {
        let (x, y) = (px0 + s.x * pw, y0 + s.y * ch);
        d.circle(x, y, 17.0, num_fill, INK, 2.0);
        chart::text(&mut d, x, y + fs * 0.35, fs, "middle", &(i + 1).to_string());
    }
    let _ = ring;
    let ly = y0 + ch + 44.0;
    for (i, s) in list.iter().enumerate() {
        let (col, row) = (i / rows, i % rows);
        let x = 60.0 + col as f64 * (w / 2.0 - 20.0);
        let y = ly + row as f64 * fs * 2.0;
        chart::text(&mut d, x, y, fs, "start", &format!("{}.", i + 1));
        if answers {
            chart::text(&mut d, x + 28.0, y, fs, "start", s.name);
        } else {
            d.line(x + 28.0, y + 3.0, x + 270.0, y + 3.0, INK, 1.0);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }
    fn with(sport: &str, f: &str) -> Params { let mut p = defaults(); p.set_s("sport", sport); p.set_s("formation", f); p }

    #[test]
    fn each_layout_has_the_right_number_of_positions() {
        assert_eq!(spots("Basketball", "").len(), 5);
        assert_eq!(spots("Volleyball", "").len(), 6);
        for f in FORMATIONS {
            let s = spots("Soccer", f);
            assert_eq!(s.len(), 11, "{f}");
            // outfield counts match the name of the formation
            let nums: Vec<usize> = f.split('-').map(|x| x.parse().unwrap()).collect();
            let xs: Vec<f64> = s.iter().skip(1).map(|p| p.x).collect();
            assert_eq!(nums.iter().sum::<usize>(), xs.len());
            for sp in &s {
                assert!((0.0..=1.0).contains(&sp.x) && (0.0..=1.0).contains(&sp.y), "{}", sp.name);
            }
        }
    }

    #[test]
    fn positions_do_not_overlap_each_other() {
        for (sport, f) in [("Basketball", ""), ("Volleyball", ""), ("Soccer", "4-4-2"), ("Soccer", "4-3-3"), ("Soccer", "3-5-2")] {
            let (cw, ch) = match sport { "Soccer" => (840.0, 544.0), "Volleyball" => (720.0, 360.0), _ => (495.0, 462.0) };
            let s = spots(sport, f);
            for i in 0..s.len() {
                for j in i + 1..s.len() {
                    let (dx, dy) = ((s[i].x - s[j].x) * cw, (s[i].y - s[j].y) * ch);
                    assert!((dx * dx + dy * dy).sqrt() > 40.0, "{sport} {f}: {} and {}", s[i].name, s[j].name);
                }
            }
        }
    }

    #[test]
    fn key_names_positions_and_student_leaves_them_blank() {
        for sport in SPORTS {
            let (student, key) = crate::figures::answer_pair(&def(), &with(sport, "4-3-3")).unwrap();
            let first = spots(sport, "4-3-3")[0].name;
            assert!(key.contains(&format!(">{first}<")) && !student.contains(&format!(">{first}<")), "{sport}");
            assert_eq!(size_of(&student), size_of(&key), "{sport}");
        }
    }

    #[test]
    fn controls_follow_the_sport() {
        assert!(relevant("court-layout", "formation", &with("Soccer", "4-4-2")));
        assert!(!relevant("court-layout", "formation", &defaults()));
    }

    #[test]
    fn print_style_is_grey() {
        for sport in SPORTS {
            let mut p = with(sport, "4-4-2");
            p.set_s("style", "Print");
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("court-layout").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in ["", "\u{0}", "<b>", "nan", "Golf"] {
            let mut p = defaults();
            p.set_s("sport", v);
            p.set_s("formation", v);
            p.set_s("title", v);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("<b>"), "{v:?}");
        }
    }
}
