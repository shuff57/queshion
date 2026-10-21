//! Entity-relationship diagram: entities as boxes with their attributes (keys underlined) and
//! relationships as lines with the cardinality at each end, from a short text spec. The student
//! copy (Show answers off) draws an empty box where each cardinality goes, and can also hide
//! the attribute lists.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::graph::{Edge, Graph, Layout, Node};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ENTITIES: usize = 10;
const MAX_ATTRS: usize = 8;
const MAX_RELS: usize = 14;
const MAX_SPEC: usize = 2500;
const MAX_NAME: usize = 16;
const DEFAULT_SPEC: &str = "entity Student: id*, name, year\nentity Course: code*, title, credits\nentity Teacher: id*, name\nStudent N:M Course : enrols in\nTeacher 1:N Course : teaches";
const CARDS: [&str; 6] = ["1", "N", "M", "0..1", "0..N", "1..N"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "er-diagram",
        subject: Subject::Computing,
        title: "Entity-relationship diagram",
        blurb: "Entities with their attributes and the relationships between them, with the cardinality at each end, from a short text spec; the cardinalities (and optionally the attributes) can be left empty for students.",
        params,
        render,
    }
}

pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "er-diagram" => vec![
            Example { name: "School", params: vec![] },
            Example {
                name: "Shop with foreign keys",
                params: vec![(
                    "spec",
                    s("entity Customer: id*, name, email\nentity Order: id*, date, customer_id^\nentity Product: id*, name, price\nentity Line: order_id^, product_id^, qty\nCustomer 1:N Order : places\nOrder 1:N Line : contains\nProduct 1:N Line : appears in"),
                )],
            },
            Example { name: "Blank cardinalities", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Diagram", DEFAULT_SPEC).hint("entity Name: id*, name, other^ (a * marks the key, a ^ a foreign key). Relationship: A 1:N B : label, using 1, N, M, 0..1, 0..N, 1..N at each end"),
        ParamDef::choice("layout", "Layout", &["Layers", "Circle"], "Layers"),
        ParamDef::bool("attributes", "Show attributes", true),
        ParamDef::bool("hide_attrs", "Hide the attributes in the student copy", false),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves an empty box where each cardinality goes (and hides the attributes if that is ticked)"),
    ]
}

struct Entity {
    name: String,
    attrs: Vec<(String, bool, bool)>,
}

struct Rel {
    a: usize,
    b: usize,
    ca: String,
    cb: String,
    label: String,
}

fn parse(spec: &str) -> Result<(Vec<Entity>, Vec<Rel>), String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("A diagram is limited to {MAX_SPEC} characters"));
    }
    let mut ents: Vec<Entity> = Vec::new();
    let mut raw: Vec<(String, String, String, String, String)> = Vec::new();
    for line in spec.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        if first.eq_ignore_ascii_case("entity") {
            let rest = line[first.len()..].trim();
            let (name, attrs) = rest.split_once(':').unwrap_or((rest, ""));
            let name = name.trim();
            if name.is_empty() || name.chars().count() > MAX_NAME || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(format!("\u{201c}{name}\u{201d} is not a usable entity name (up to {MAX_NAME} letters, digits or _)"));
            }
            if ents.iter().any(|e| e.name == name) {
                return Err(format!("Entity {name} is defined twice"));
            }
            if ents.len() >= MAX_ENTITIES {
                return Err(format!("A diagram is limited to {MAX_ENTITIES} entities"));
            }
            let mut list = Vec::new();
            for a in attrs.split(',').map(str::trim).filter(|a| !a.is_empty()) {
                let (n, key, fk) = if let Some(n) = a.strip_suffix('*') { (n, true, false) } else if let Some(n) = a.strip_suffix('^') { (n, false, true) } else { (a, false, false) };
                let n = n.trim();
                if n.is_empty() || n.chars().count() > 20 {
                    return Err(format!("\u{201c}{a}\u{201d} is not a usable attribute name"));
                }
                list.push((n.to_string(), key, fk));
            }
            if list.len() > MAX_ATTRS {
                return Err(format!("{name} has more than {MAX_ATTRS} attributes"));
            }
            ents.push(Entity { name: name.to_string(), attrs: list });
        } else {
            let (head, label) = line.split_once(" : ").map_or((line, ""), |(h, l)| (h, l));
            let parts: Vec<&str> = head.split_whitespace().collect();
            if parts.len() != 3 {
                return Err(format!("\u{201c}{line}\u{201d} is not an entity or a relationship (A 1:N B : label)"));
            }
            let (ca, cb) = parts[1].split_once(':').ok_or_else(|| format!("\u{201c}{}\u{201d} should be two cardinalities such as 1:N", parts[1]))?;
            for c in [ca, cb] {
                if !CARDS.contains(&c.to_uppercase().as_str()) {
                    return Err(format!("\u{201c}{c}\u{201d} is not a cardinality (1, N, M, 0..1, 0..N, 1..N)"));
                }
            }
            raw.push((parts[0].to_string(), parts[2].to_string(), ca.to_uppercase(), cb.to_uppercase(), label.trim().chars().take(24).collect()));
        }
    }
    let mut rels = Vec::new();
    for (a, b, ca, cb, label) in raw {
        let find = |n: &str| ents.iter().position(|e| e.name == n).ok_or_else(|| format!("{a} {ca}:{cb} {b}: there is no entity {n}"));
        let (ia, ib) = (find(&a)?, find(&b)?);
        if rels.len() >= MAX_RELS {
            return Err(format!("A diagram is limited to {MAX_RELS} relationships"));
        }
        rels.push(Rel { a: ia, b: ib, ca, cb, label });
    }
    Ok((ents, rels))
}

fn exit(c: (f64, f64), hw: f64, hh: f64, to: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (to.0 - c.0, to.1 - c.1);
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return c;
    }
    let t = (hw / dx.abs().max(1e-9)).min(hh / dy.abs().max(1e-9));
    (c.0 + dx * t, c.1 + dy * t)
}

fn render(p: &Params) -> String {
    let (ents, rels) = match parse(p.s("spec")) {
        Ok(x) => x,
        Err(e) => return chart::message(&e),
    };
    if ents.is_empty() {
        return chart::message("Add some entities to draw a diagram");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let title = p.s("title").trim();
    let want_attrs = p.b("attributes");
    let hide_attrs = !answers && p.b("hide_attrs");
    let row_h = fs * 1.65;

    // box sizes from the real attributes, so the student copy keeps the key's layout
    let dims: Vec<(f64, f64)> = ents
        .iter()
        .map(|e| {
            let aw = if want_attrs { e.attrs.iter().map(|(a, _, fk)| gd::text_w(a, fs) + if *fk { fs * 2.2 } else { 0.0 }).fold(0.0, f64::max) } else { 0.0 };
            ((gd::text_w(&e.name, fs).max(aw) + 34.0).max(110.0), row_h * 1.25 + if want_attrs { e.attrs.len() as f64 * row_h + 6.0 } else { 0.0 })
        })
        .collect();
    let (mw, mh) = (dims.iter().map(|d| d.0).fold(0.0, f64::max), dims.iter().map(|d| d.1).fold(0.0, f64::max));
    let g = Graph {
        nodes: ents.iter().map(|e| Node { name: e.name.clone(), pos: None }).collect(),
        edges: {
            let mut v: Vec<Edge> = Vec::new();
            for r in &rels {
                if r.a != r.b && !v.iter().any(|e| (e.a, e.b) == (r.a, r.b) || (e.a, e.b) == (r.b, r.a)) {
                    v.push(Edge { a: r.a, b: r.b, directed: false, weight: None });
                }
            }
            v
        },
    };
    let grid = g.layout(if p.s("layout") == "Circle" { Layout::Circle } else { Layout::Layers });
    let (pts, dw, dh) = gd::to_px(&grid, (mw + 110.0, mh + 112.0), (mw / 2.0 + 30.0, mh / 2.0 + 24.0));
    let top = if title.is_empty() { 6.0 } else { 34.0 + fs * 1.3 };
    let w = dw.max(300.0);
    let h = top + dh;
    let ox = (w - dw) / 2.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let at = |i: usize| (pts[i].0 + ox, pts[i].1 + top);

    // relationship lines first, under the boxes
    for (ri, r) in rels.iter().enumerate() {
        if r.a == r.b {
            continue;
        }
        let (ca, cb) = (at(r.a), at(r.b));
        // two relationships between the same pair run side by side
        let same = rels.iter().enumerate().filter(|(_, o)| (o.a, o.b) == (r.a, r.b) || (o.a, o.b) == (r.b, r.a)).map(|(j, _)| j).collect::<Vec<_>>();
        let slot = same.iter().position(|j| *j == ri).unwrap_or(0) as f64 - (same.len() as f64 - 1.0) / 2.0;
        let (dx, dy) = (cb.0 - ca.0, cb.1 - ca.1);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (nx, ny) = (-dy / len * slot * 26.0, dx / len * slot * 26.0);
        let (sa, sb) = ((ca.0 + nx, ca.1 + ny), (cb.0 + nx, cb.1 + ny));
        let p0 = exit(ca, dims[r.a].0 / 2.0, dims[r.a].1 / 2.0, sb);
        let p1 = exit(cb, dims[r.b].0 / 2.0, dims[r.b].1 / 2.0, sa);
        let (p0, p1) = ((p0.0 + nx, p0.1 + ny), (p1.0 + nx, p1.1 + ny));
        d.line(p0.0, p0.1, p1.0, p1.1, INK, 1.8);
        let (ux, uy) = ((p1.0 - p0.0) / len, (p1.1 - p0.1) / len);
        let mid = ((p0.0 + p1.0) / 2.0, (p0.1 + p1.1) / 2.0);
        if !r.label.is_empty() {
            gd::tag(&mut d, mid, fs * 0.9, &r.label);
        }
        // cardinality just off each end, to one side of the line
        for (end, dir, card) in [(p0, 1.0, &r.ca), (p1, -1.0, &r.cb)] {
            let c = (end.0 + ux * dir * 22.0 - uy * 12.0, end.1 + uy * dir * 22.0 + ux * 12.0);
            if answers {
                gd::tag(&mut d, c, fs * 0.95, card);
            } else {
                d.rect(c.0 - 15.0, c.1 - fs * 0.7, 30.0, fs * 1.4, "#fff", INK, 1.2);
            }
        }
    }
    let head = if print { "#e5e5e5" } else { "#dfe8f8" };
    for (i, e) in ents.iter().enumerate() {
        let (cx, cy) = at(i);
        let (bw, bh) = dims[i];
        let (x, y) = (cx - bw / 2.0, cy - bh / 2.0);
        d.rect(x, y, bw, bh, "#fff", INK, 1.8);
        d.rect(x, y, bw, row_h * 1.25, head, INK, 1.8);
        gd::text_fill(&mut d, cx, y + row_h * 0.9, fs, INK, &e.name);
        if want_attrs && !hide_attrs {
            for (j, (a, key, fk)) in e.attrs.iter().enumerate() {
                let ty = y + row_h * 1.25 + 4.0 + row_h * (j as f64 + 0.8);
                d.raw(&format!(
                    r##"<text x="{:.2}" y="{ty:.2}" font-size="{fs}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
                    x + 12.0,
                    svg::esc(a)
                ));
                if *key {
                    d.line(x + 12.0, ty + 3.0, x + 12.0 + gd::text_w(a, fs) * 0.92, ty + 3.0, INK, 1.2);
                }
                if *fk {
                    d.raw(&format!(
                        r##"<text x="{:.2}" y="{ty:.2}" font-size="{}" text-anchor="end" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="#6b7280">FK</text>"##,
                        x + bw - 10.0,
                        fs * 0.8
                    ));
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
    fn spec_parses_entities_keys_and_cardinalities() {
        let (e, r) = parse(DEFAULT_SPEC).unwrap();
        assert_eq!(e.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["Student", "Course", "Teacher"]);
        assert_eq!(e[0].attrs[0], ("id".to_string(), true, false));
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].ca.as_str(), r[0].cb.as_str(), r[0].label.as_str()), ("N", "M", "enrols in"));
        let (e, _) = parse("entity A: x^, y*").unwrap();
        assert_eq!((e[0].attrs[0].2, e[0].attrs[1].1), (true, true));
        let (_, r) = parse("entity A\nentity B\nA 0..1:n B").unwrap();
        assert_eq!((r[0].ca.as_str(), r[0].cb.as_str()), ("0..1", "N"));
    }

    #[test]
    fn default_draws_boxes_keys_and_cardinalities() {
        let svg = render(&defaults());
        for t in [">Student<", ">Course<", ">id<", ">enrols in<", ">M<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert!(svg.matches("<line").count() >= 2 + 3, "two relationship lines and underlined keys");
    }

    #[test]
    fn student_copy_blanks_the_cardinalities() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(key.contains(">N<") && !student.contains(">N<") && !student.contains(">M<"));
        assert!(student.contains(">enrols in<"));
        assert_eq!(size_of(&student), size_of(&key));
        let mut p = defaults();
        p.set_b("hide_attrs", true);
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(!student.contains(">credits<") && key.contains(">credits<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [
            ("entity A\nentity A", "twice"),
            ("entity A\nA 1:N B", "no entity B"),
            ("entity A\nentity B\nA 1:X B", "not a cardinality"),
            ("entity A\nentity B\nA 1-N B", "two cardinalities"),
            ("nonsense", "not an entity"),
            ("entity bad name: x", "not a usable"),
            ("", "Add some entities"),
        ] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#dfe8f8"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("er-diagram").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        let many: String = (0..14).map(|i| format!("entity E{i}: a*, b\n")).chain((0..14).map(|i| format!("E{i} 1:N E{}\n", (i + 1) % 14))).collect();
        for spec in ["", "\u{0}", "entity A", "entity A\nA 1:1 A", "entity A\nentity B\nA 1:N B\nB N:1 A\nA M:M B", &many] {
            for layout in ["Layers", "Circle"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("spec", spec);
                    p.set_s("layout", layout);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?}");
                }
            }
        }
    }
}
