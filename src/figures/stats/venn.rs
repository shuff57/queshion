//! Venn diagram of two or three sets: a value in every region (counts, or short elements), an
//! optional shaded region, and the set totals worked out underneath. The student copy (Show
//! answers off) keeps the circles and any shading and leaves the regions and totals empty.
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const SHADES: [&str; 11] = ["None", "A", "B", "C", "A and B", "A or B", "A only", "B only", "Not A", "Not B", "Outside"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "venn-diagram",
        subject: Subject::Math,
        title: "Venn diagram",
        blurb: "A two- or three-set Venn diagram with a count in every region, an optional shaded region and the set totals worked out, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("venn-diagram", "c_name") => p.s("sets") == "3",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "venn-diagram" => vec![
            Example { name: "Two sets: a class survey", params: vec![] },
            Example {
                name: "Three sets",
                params: vec![("sets", s("3")), ("c_name", s("Art")), ("counts", s("8, 6, 5, 4, 3, 2, 1, 7"))],
            },
            Example { name: "Shade the overlap, blank regions", params: vec![("shade", s("A and B")), ("show_answers", Value::B(false))] },
            Example {
                name: "Elements instead of counts",
                params: vec![("a_name", s("Even")), ("b_name", s("Prime")), ("counts", s("4 6 8, 3 5 7, 2, 1 9")), ("totals", Value::B(false))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("sets", "Sets", &["2", "3"], "2"),
        ParamDef::text("a_name", "Set A", "Music"),
        ParamDef::text("b_name", "Set B", "Sport"),
        ParamDef::text("c_name", "Set C", "Art"),
        ParamDef::text("counts", "Region values", "12, 9, 5, 4").hint("Separated by commas. Two sets: A only, B only, both, neither. Three sets: A only, B only, C only, A and B only, A and C only, B and C only, all three, none. Numbers or short text such as 2 4 6"),
        ParamDef::choice("shade", "Shade", &SHADES, "None"),
        ParamDef::bool("totals", "Work out the totals underneath", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the circles and the shading and leaves the region values and totals empty for students"),
    ]
}

/// Whether the region with membership `mask` (A = 1, B = 2, C = 4) is inside the shade.
fn shaded(shade: &str, mask: usize) -> bool {
    let (a, b, c) = (mask & 1 != 0, mask & 2 != 0, mask & 4 != 0);
    match shade {
        "A" => a,
        "B" => b,
        "C" => c,
        "A and B" => a && b,
        "A or B" => a || b,
        "A only" => a && !b && !c,
        "B only" => b && !a && !c,
        "Not A" => !a,
        "Not B" => !b,
        "Outside" => mask == 0,
        _ => false,
    }
}

fn clip_defs(n: usize, c: &[(f64, f64)], r: f64, rect: (f64, f64, f64, f64)) -> String {
    let mut s = String::new();
    for i in 0..n {
        let (cx, cy) = c[i];
        let ring = format!("M{:.2},{:.2} a{r},{r} 0 1,0 {:.2},0 a{r},{r} 0 1,0 {:.2},0 Z", cx - r, cy, 2.0 * r, -2.0 * r);
        s.push_str(&format!(r##"<clipPath id="vn-in{i}"><path d="{ring}"/></clipPath>"##));
        s.push_str(&format!(
            r##"<clipPath id="vn-out{i}"><path clip-rule="evenodd" d="M{:.2},{:.2} h{:.2} v{:.2} h{:.2} Z {ring}"/></clipPath>"##,
            rect.0, rect.1, rect.2, rect.3, -rect.2
        ));
    }
    s
}

fn render(p: &Params) -> String {
    let n = if p.s("sets") == "3" { 3 } else { 2 };
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let title = p.s("title").trim();
    let clip = |s: &str, m: usize| -> String { s.trim().chars().take(m).collect() };
    let names = [clip(p.s("a_name"), 12), clip(p.s("b_name"), 12), clip(p.s("c_name"), 12)];
    let names: Vec<String> = names.iter().enumerate().map(|(i, s)| if s.is_empty() { ["A", "B", "C"][i].to_string() } else { s.clone() }).collect();

    // the values, in the order the hint gives, stored by membership mask
    let order: &[usize] = if n == 2 { &[1, 2, 3, 0] } else { &[1, 2, 4, 3, 5, 6, 7, 0] };
    let given: Vec<String> = p.s("counts").split(',').map(|s| clip(s, 14)).collect();
    if given.len() > order.len() {
        return chart::message(&format!("{n} sets have {} regions; there are {} values", order.len(), given.len()));
    }
    let mut vals: Vec<String> = vec![String::new(); 8];
    for (g, m) in given.iter().zip(order) {
        vals[*m] = g.clone();
    }
    let nums: Option<Vec<f64>> = (0..8)
        .map(|m| if m >= (1 << n) { Some(0.0) } else if vals[m].is_empty() { Some(0.0) } else { vals[m].parse::<f64>().ok().filter(|v| v.is_finite()) })
        .collect();

    let (rect, centres, r, label_at, name_at, bottom): ((f64, f64, f64, f64), Vec<(f64, f64)>, f64, Vec<(usize, f64, f64)>, Vec<(f64, f64, &str)>, f64) = if n == 2 {
        (
            (20.0, 20.0, 480.0, 280.0),
            vec![(205.0, 160.0), (315.0, 160.0)],
            105.0,
            vec![(1, 150.0, 165.0), (2, 370.0, 165.0), (3, 260.0, 165.0), (0, 48.0, 284.0)],
            vec![(150.0, 44.0, "middle"), (370.0, 44.0, "middle")],
            300.0,
        )
    } else {
        (
            (20.0, 20.0, 480.0, 360.0),
            vec![(205.0, 150.0), (315.0, 150.0), (260.0, 245.0)],
            95.0,
            vec![(1, 160.0, 130.0), (2, 360.0, 130.0), (4, 260.0, 300.0), (3, 260.0, 112.0), (5, 212.0, 210.0), (6, 308.0, 210.0), (7, 260.0, 178.0), (0, 48.0, 364.0)],
            vec![(140.0, 44.0, "middle"), (380.0, 44.0, "middle"), (352.0, 338.0, "start")],
            380.0,
        )
    };
    let totals = p.b("totals");
    let lines = 5;
    let top = if title.is_empty() { 0.0 } else { 14.0 + fs * 1.5 };
    let foot = if totals { 24.0 + lines as f64 * fs * 1.7 } else { 10.0 };
    let (w, h) = (520.0, top + bottom + foot);
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.2, fs * 1.3, "middle", title);
    }
    let sh = p.s("shade");
    let c: Vec<(f64, f64)> = centres.iter().map(|(x, y)| (*x, y + top)).collect();
    let rect = (rect.0, rect.1 + top, rect.2, rect.3);
    d.raw(&clip_defs(n, &c, r, rect));
    // shading: each shaded region is the universe clipped to the circles it is in and out of
    for mask in 0..(1usize << n) {
        if !shaded(sh, mask) {
            continue;
        }
        let (mut open, mut close) = (String::new(), String::new());
        for i in 0..n {
            open.push_str(&format!(r##"<g clip-path="url(#vn-{}{i})">"##, if mask & (1 << i) != 0 { "in" } else { "out" }));
            close.push_str("</g>");
        }
        d.raw(&format!(
            r##"{open}<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>{close}"##,
            rect.0, rect.1, rect.2, rect.3, fills.series(0)
        ));
    }
    d.rect(rect.0, rect.1, rect.2, rect.3, "none", INK, 2.0);
    for (cx, cy) in &c {
        d.circle(*cx, *cy, r, "none", INK, 2.4);
    }
    chart::text(&mut d, rect.0 + 12.0, rect.1 + fs * 1.3, fs, "start", "U");
    for (i, (x, y, a)) in name_at.iter().enumerate() {
        chart::text(&mut d, *x, y + top, fs * 1.1, a, &names[i]);
    }
    if answers {
        for (m, x, y) in &label_at {
            if !vals[*m].is_empty() {
                chart::text(&mut d, *x, y + top + fs * 0.35, fs * 1.05, "middle", &vals[*m]);
            }
        }
    }
    if totals {
        let tot = |f: &dyn Fn(usize) -> bool| -> Option<String> {
            nums.as_ref().map(|v| chart::fmt(((0..(1usize << n)).filter(|m| f(*m)).map(|m| v[m]).sum::<f64>() * 1000.0).round() / 1000.0))
        };
        let rows: Vec<(String, Option<String>)> = if n == 2 {
            vec![
                (format!("n({}) =", names[0]), tot(&|m| m & 1 != 0)),
                (format!("n({}) =", names[1]), tot(&|m| m & 2 != 0)),
                (format!("n({} and {}) =", names[0], names[1]), tot(&|m| m & 3 == 3)),
                (format!("n({} or {}) =", names[0], names[1]), tot(&|m| m & 3 != 0)),
                ("n(all) =".to_string(), tot(&|_| true)),
            ]
        } else {
            vec![
                (format!("n({}) =", names[0]), tot(&|m| m & 1 != 0)),
                (format!("n({}) =", names[1]), tot(&|m| m & 2 != 0)),
                (format!("n({}) =", names[2]), tot(&|m| m & 4 != 0)),
                ("n(all three) =".to_string(), tot(&|m| m == 7)),
                ("n(all) =".to_string(), tot(&|_| true)),
            ]
        };
        let lab_w = rows.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(8) as f64 * fs * 0.5 + 6.0;
        for (i, (l, v)) in rows.iter().enumerate() {
            let y = top + bottom + 14.0 + fs * 1.7 * (i as f64 + 0.8);
            chart::text(&mut d, 28.0, y, fs, "start", l);
            match (answers, v) {
                (true, Some(v)) => chart::text(&mut d, 28.0 + lab_w, y, fs, "start", v),
                (true, None) => {}
                _ => {
                    d.line(28.0 + lab_w, y + 2.0, 28.0 + lab_w + 90.0, y + 2.0, INK, 1.0);
                }
            }
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn two_sets_show_regions_and_totals() {
        let svg = render(&defaults());
        for t in [">Music<", ">Sport<", ">12<", ">9<", ">5<", ">4<"] {
            assert!(svg.contains(t), "{t}");
        }
        // 12 + 5 = 17 in Music, 9 + 5 = 14 in Sport, 5 in both, 26 in all
        for t in [">17<", ">14<", ">26<", ">30<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert_eq!(svg.matches("<circle").count(), 2);
    }

    #[test]
    fn three_sets_total_by_membership() {
        let mut p = defaults();
        p.set_s("sets", "3");
        p.set_s("counts", "8, 6, 5, 4, 3, 2, 1, 7");
        let svg = render(&p);
        // A = 8 + 4 + 3 + 1 = 16, B = 6 + 4 + 2 + 1 = 13, C = 5 + 3 + 2 + 1 = 11, all = 36
        for t in [">16<", ">13<", ">11<", ">36<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert_eq!(svg.matches("<circle").count(), 3);
    }

    #[test]
    fn student_copy_keeps_the_circles_and_shading_only() {
        let def = def();
        let mut p = defaults();
        p.set_s("shade", "A and B");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(key.contains(">12<") && !student.contains(">12<") && !student.contains(">26<"));
        assert!(student.contains("vn-in0") && student.contains(">Music<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn shading_follows_the_region() {
        assert!(shaded("A and B", 3) && !shaded("A and B", 1));
        assert!(shaded("Outside", 0) && !shaded("Outside", 4));
        assert!(shaded("A only", 1) && !shaded("A only", 3) && !shaded("A only", 5));
        assert!(shaded("Not A", 2) && !shaded("Not A", 1));
        assert!(!shaded("None", 7));
        let mut p = defaults();
        assert!(!render(&p).contains("<g clip-path"));
        p.set_s("shade", "A or B");
        assert_eq!(render(&p).matches("<g clip-path").count(), 3 * 2, "three shaded regions, two clips each");
    }

    #[test]
    fn too_many_values_and_words_are_handled() {
        let mut p = defaults();
        p.set_s("counts", "1, 2, 3, 4, 5");
        assert!(render(&p).contains("there are 5 values"));
        p.set_s("counts", "4 6 8, 3 5 7, 2, 1 9");
        let svg = render(&p);
        assert!(svg.contains(">4 6 8<") && !svg.contains("NaN"));
        p.set_s("counts", "1, x");
        assert!(render(&p).starts_with("<svg"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("shade", "A or B");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("venn-diagram").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<circle") && !svg.contains("NaN"), "{}", ex.name);
        }
        for counts in ["", "\u{0}", ",,,", "nan, inf, -1, 1e308", "a very very long value indeed, b, c, d", "1e308, 1e308, 1e308, 1e308"] {
            for sets in ["2", "3", "9"] {
                for shade in SHADES {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_s("counts", counts);
                        p.set_s("sets", sets);
                        p.set_s("shade", shade);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{counts:?} {sets} {shade}");
                    }
                }
            }
        }
    }
}
