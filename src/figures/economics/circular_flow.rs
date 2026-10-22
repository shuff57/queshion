//! The circular flow of income. The two-sector model draws households and firms linked by the
//! product market and the factor market, with the real flows (goods, services and factors) as an
//! inner loop and the money flows (spending, revenue, factor payments, income) as an outer loop.
//! The three-sector model adds government and draws the money flows between households, firms and
//! government. The sectors and arrows are the question; what a student can be asked to supply is
//! either the flow labels or the sector names (Hide in student copy), so with Show answers off those
//! are left as empty boxes of the same size.
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MODELS: [&str; 2] = ["Two sectors with markets", "Three sectors: money flows"];
const HIDES: [&str; 2] = ["Flow labels", "Sector names"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "circular-flow",
        subject: Subject::Economics,
        title: "Circular flow of income",
        blurb: "The circular flow diagram: households, firms and the product and factor markets with real and money flows, or households, firms and government with money flows, with the flow labels or the sector names left blank for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("circular-flow", "real") | ("circular-flow", "money") => p.s("model") == MODELS[0],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "circular-flow" => vec![
            Example { name: "Two sectors, both loops", params: vec![] },
            Example { name: "Money flows only", params: vec![("real", Value::B(false))] },
            Example { name: "With government", params: vec![("model", s(MODELS[1]))] },
            Example { name: "Blank flow labels", params: vec![("show_answers", Value::B(false))] },
            Example { name: "Blank sector names", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDES[1]))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("model", "Model", &MODELS, MODELS[0]),
        ParamDef::bool("real", "Show the real flows (goods, services, factors)", true),
        ParamDef::bool("money", "Show the money flows", true),
        ParamDef::choice("hide", "Leave blank in the student copy", &HIDES, HIDES[0]),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the flow labels (or the sector names) as empty boxes for students to fill in"),
    ]
}

/// Splits a label into at most two balanced lines.
fn lines(s: &str) -> Vec<String> {
    if s.chars().count() <= 14 {
        return vec![s.to_string()];
    }
    let words: Vec<&str> = s.split(' ').collect();
    let mut best = (usize::MAX, 1);
    for i in 1..words.len() {
        let (a, b) = (words[..i].join(" ").chars().count(), words[i..].join(" ").chars().count());
        if a.max(b) < best.0 {
            best = (a.max(b), i);
        }
    }
    if words.len() < 2 {
        return vec![s.to_string()];
    }
    vec![words[..best.1].join(" "), words[best.1..].join(" ")]
}

struct Ctx {
    fs: f64,
    show_labels: bool,
    print: bool,
    /// Label boxes wait until every arrow is drawn, so one box never hides another's text.
    pending: std::cell::RefCell<Vec<((f64, f64), String)>>,
}

/// Draws the waiting label boxes: the text when labels show, an empty box otherwise.
fn flush(d: &mut Doc, c: &Ctx) {
    let (bw, bh) = (132.0, 38.0);
    for (at, label) in c.pending.borrow().iter() {
        if c.show_labels {
            d.rect(at.0 - bw / 2.0, at.1 - bh / 2.0, bw, bh, "#fff", "none", 0.0);
            let ls = lines(label);
            let y0 = at.1 + c.fs * 0.35 - (ls.len() as f64 - 1.0) * c.fs * 0.6;
            for (i, l) in ls.iter().enumerate() {
                chart::text(d, at.0, y0 + i as f64 * c.fs * 1.2, c.fs, "middle", l);
            }
        } else {
            d.rect(at.0 - bw / 2.0, at.1 - bh / 2.0, bw, bh, "#fff", INK, 1.2);
        }
    }
}

/// An arrow along a polyline with a label box centred at `at`: boxed and empty for a student copy.
fn flow(d: &mut Doc, c: &Ctx, pts: &[(f64, f64)], at: (f64, f64), label: &str, real: bool) {
    let stroke = if c.print { "#111827" } else if real { "#2f6fb5" } else { "#c0612b" };
    let dash = if real { "" } else { r#" stroke-dasharray="9 5""# };
    let mut path = format!("M{:.1},{:.1}", pts[0].0, pts[0].1);
    for (x, y) in &pts[1..pts.len() - 1] {
        path.push_str(&format!(" L{x:.1},{y:.1}"));
    }
    // stop the line short of the tip so the head sits cleanly
    let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (ux, uy) = (dx / len, dy / len);
    path.push_str(&format!(" L{:.1},{:.1}", b.0 - ux * 9.0, b.1 - uy * 9.0));
    d.raw(&format!(r##"<path d="{path}" fill="none" stroke="{stroke}" stroke-width="2.6"{dash}/>"##));
    let (nx, ny) = (-uy, ux);
    d.poly(&[(b.0, b.1), (b.0 - ux * 12.0 + nx * 6.0, b.1 - uy * 12.0 + ny * 6.0), (b.0 - ux * 12.0 - nx * 6.0, b.1 - uy * 12.0 - ny * 6.0)], stroke, "none", 0.0);
    c.pending.borrow_mut().push((at, label.to_string()));
}

fn node(d: &mut Doc, c: &Ctx, cx: f64, cy: f64, w: f64, h: f64, name: &str, show_name: bool) {
    d.rect(cx - w / 2.0, cy - h / 2.0, w, h, if c.print { "#e5e5e5" } else { "#f4eee2" }, INK, 2.0);
    if show_name {
        let ls = lines(name);
        let y0 = cy + c.fs * 0.38 - (ls.len() as f64 - 1.0) * c.fs * 0.6;
        for (i, l) in ls.iter().enumerate() {
            d.raw(&format!(
                r##"<text x="{cx:.1}" y="{:.1}" font-size="{}" font-weight="bold" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
                y0 + i as f64 * c.fs * 1.2,
                c.fs * 1.05,
                svg::esc(l)
            ));
        }
    }
}

fn render(p: &Params) -> String {
    let two = p.s("model") != MODELS[1];
    let (real, money) = (p.b("real"), p.b("money"));
    if two && !real && !money {
        return chart::message("Show the real flows, the money flows, or both");
    }
    let answers = p.b(SHOW_ANSWERS);
    let hide_names = p.s("hide") == HIDES[1];
    let c = Ctx { fs: 13.0 * svg::label_factor(p.s("label_size")), show_labels: answers || hide_names, print: p.s("style") == "Print", pending: Default::default() };
    let show_names = answers || !hide_names;
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = if two { (800.0, 520.0 + top) } else { (720.0, 470.0 + top) };
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, c.fs * 1.3, "middle", title);
    }
    if two {
        let y = |v: f64| v + top;
        // nodes: households top, firms bottom, factor market left, product market right
        let (hx, fx, mx_l, mx_r) = (400.0, 400.0, 100.0, 700.0);
        if money {
            // outer loop, clockwise: households -> product market -> firms -> factor market -> households
            flow(&mut d, &c, &[(490.0, y(48.0)), (700.0, y(48.0)), (700.0, y(205.0))], (595.0, y(48.0)), "Consumption spending", false);
            flow(&mut d, &c, &[(700.0, y(335.0)), (700.0, y(472.0)), (490.0, y(472.0))], (595.0, y(472.0)), "Sales revenue", false);
            flow(&mut d, &c, &[(310.0, y(472.0)), (100.0, y(472.0)), (100.0, y(335.0))], (205.0, y(472.0)), "Payments for factors", false);
            flow(&mut d, &c, &[(100.0, y(205.0)), (100.0, y(48.0)), (310.0, y(48.0))], (205.0, y(48.0)), "Income", false);
        }
        if real {
            // inner loop, counter-clockwise: households -> factor market -> firms -> product market -> households
            flow(&mut d, &c, &[(350.0, y(74.0)), (350.0, y(215.0)), (175.0, y(215.0))], (262.0, y(215.0)), "Factors of production", true);
            flow(&mut d, &c, &[(175.0, y(305.0)), (370.0, y(305.0)), (370.0, y(446.0))], (272.0, y(305.0)), "Factors of production", true);
            flow(&mut d, &c, &[(430.0, y(446.0)), (430.0, y(305.0)), (625.0, y(305.0))], (528.0, y(305.0)), "Goods and services", true);
            flow(&mut d, &c, &[(625.0, y(215.0)), (450.0, y(215.0)), (450.0, y(74.0))], (538.0, y(215.0)), "Goods and services", true);
        }
        node(&mut d, &c, hx, y(48.0), 180.0, 52.0, "Households", show_names);
        node(&mut d, &c, fx, y(472.0), 180.0, 52.0, "Firms", show_names);
        node(&mut d, &c, mx_l, y(260.0), 150.0, 130.0, "Factor market", show_names);
        node(&mut d, &c, mx_r, y(260.0), 150.0, 130.0, "Product market", show_names);
        flush(&mut d, &c);
        if real && money {
            // a key for the two kinds of flow, in the middle of the diagram
            let ky = y(262.0);
            let stroke = if c.print { "#111827" } else { "#2f6fb5" };
            let stroke2 = if c.print { "#111827" } else { "#c0612b" };
            d.raw(&format!(r##"<line x1="330" y1="{:.1}" x2="372" y2="{:.1}" stroke="{stroke}" stroke-width="2.6"/>"##, ky - 9.0, ky - 9.0));
            chart::text(&mut d, 380.0, ky - 4.0, c.fs * 0.9, "start", "Real flows");
            d.raw(&format!(r##"<line x1="330" y1="{:.1}" x2="372" y2="{:.1}" stroke="{stroke2}" stroke-width="2.6" stroke-dasharray="9 5"/>"##, ky + 14.0, ky + 14.0));
            chart::text(&mut d, 380.0, ky + 19.0, c.fs * 0.9, "start", "Money flows");
        }
    } else {
        let y = |v: f64| v + top;
        let g = (360.0, y(60.0));
        let hh = (120.0, y(380.0));
        let ff = (600.0, y(380.0));
        let (bw, bh) = (170.0, 60.0);
        // two arrows between each pair, offset either side of the line between their centres
        let pair = |d: &mut Doc, a: (f64, f64), b: (f64, f64), l_ab: &str, l_ba: &str| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = (dx * dx + dy * dy).sqrt();
            let (ux, uy) = (dx / len, dy / len);
            let (nx, ny) = (-uy, ux);
            let off = 24.0;
            let edge = |c: (f64, f64), sgn: f64, o: f64| {
                // the point where the line offset by o along n, leaving centre c along sgn*u, exits the box
                let (ox, oy) = (nx * o, ny * o);
                let (vx, vy) = (ux * sgn, uy * sgn);
                let sx = if vx.abs() > 1e-9 { ((vx.signum() * bw / 2.0) - ox) / vx } else { f64::INFINITY };
                let sy = if vy.abs() > 1e-9 { ((vy.signum() * bh / 2.0) - oy) / vy } else { f64::INFINITY };
                let s = sx.min(sy);
                (c.0 + ox + vx * s, c.1 + oy + vy * s)
            };
            let (p0, p1) = (edge(a, 1.0, off), edge(b, -1.0, off));
            let at = |f: f64, p0: (f64, f64), p1: (f64, f64)| (p0.0 + (p1.0 - p0.0) * f, p0.1 + (p1.1 - p0.1) * f);
            flow(d, &c, &[p0, p1], at(0.62, p0, p1), l_ab, false);
            let (q0, q1) = (edge(b, -1.0, -off), edge(a, 1.0, -off));
            // the reverse arrow runs from b to a along the other side
            let (r0, r1) = (edge(b, -1.0, -off), edge(a, 1.0, -off));
            let _ = (q0, q1);
            flow(d, &c, &[r0, r1], at(0.62, r0, r1), l_ba, false);
        };
        pair(&mut d, hh, ff, "Consumption spending", "Wages, rent, interest and profit");
        pair(&mut d, g, hh, "Transfer payments", "Taxes");
        pair(&mut d, ff, g, "Taxes", "Subsidies and purchases");
        node(&mut d, &c, g.0, g.1, bw, bh, "Government", show_names);
        node(&mut d, &c, hh.0, hh.1, bw, bh, "Households", show_names);
        node(&mut d, &c, ff.0, ff.1, bw, bh, "Firms", show_names);
        flush(&mut d, &c);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn labels_wrap_into_two_balanced_lines() {
        assert_eq!(lines("Income"), vec!["Income"]);
        assert_eq!(lines("Wages, rent, interest and profit"), vec!["Wages, rent,", "interest and profit"]);
        assert_eq!(lines("Factors of production"), vec!["Factors of", "production"]);
        assert_eq!(lines("Supercalifragilistic expialidocious"), vec!["Supercalifragilistic", "expialidocious"]);
        assert_eq!(lines("Antidisestablishmentarianism"), vec!["Antidisestablishmentarianism"]);
    }

    #[test]
    fn two_sector_key_names_every_sector_and_flow() {
        let svg = render(&defaults());
        for t in [">Households<", ">Firms<", ">Factor market<", ">Product market<", ">Income<", ">Sales revenue<", ">production<", ">services<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert_eq!(svg.matches("stroke-dasharray=\"9 5\"").count(), 5, "four money flows and the key");
    }

    #[test]
    fn the_loops_can_be_shown_separately() {
        let mut p = defaults();
        p.set_b("real", false);
        let svg = render(&p);
        assert!(svg.contains(">Income<") && !svg.contains(">services<"));
        p.set_b("real", true);
        p.set_b("money", false);
        let svg = render(&p);
        assert!(!svg.contains(">Income<") && svg.contains(">services<"));
        p.set_b("real", false);
        assert!(render(&p).contains("both"));
    }

    #[test]
    fn three_sector_adds_government() {
        let mut p = defaults();
        p.set_s("model", MODELS[1]);
        let svg = render(&p);
        for t in [">Government<", ">Taxes<", ">Households<", ">Transfer<", ">payments<", ">Subsidies<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn student_copy_blanks_what_was_chosen() {
        for model in MODELS {
            let mut p = defaults();
            p.set_s("model", model);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert!(student.contains(">Households<") && !student.contains("Income") && !student.contains("Taxes") && key.contains("Households"));
            assert_eq!(size_of(&student), size_of(&key));
            p.set_s("hide", HIDES[1]);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert!(!student.contains(">Households<") && (student.contains("Consumption") || student.contains("Wages")), "flow labels stay when the names are hidden");
            assert_eq!(size_of(&student), size_of(&key));
        }
    }

    #[test]
    fn print_style_is_grey() {
        for model in MODELS {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("model", model);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("circular-flow").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for title in ["", "\u{0}", "<script>", "\u{1f600}", &"x".repeat(5000)] {
            for model in MODELS {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("title", title);
                    p.set_s("model", model);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg) && !svg.contains("<script>"), "{title:?}");
                }
            }
        }
    }
}
