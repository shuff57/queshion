//! Tree: a binary search tree grown from a list of values, or any tree written as parent lines
//! (`A: B, C`). Traversal orders, height and depths are worked out under the drawing, and can be
//! numbered on the nodes; the student copy (Show answers off) keeps the tree and the heading and
//! leaves the room to fill in.
use super::graph_draw::{self as gd, NodeStyle};
use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::graph::Tree;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const ANSWERS: [&str; 7] = ["None", "Pre-order", "In-order", "Post-order", "Level-order", "All four traversals", "Height and depths"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "tree-diagram",
        subject: Subject::Computing,
        title: "Tree",
        blurb: "A binary search tree grown from a list of values, or any tree written as parent lines, with the traversal orders, height and depths worked out underneath.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("tree-diagram", "values") => p.s("source").starts_with("Insert"),
        ("tree-diagram", "spec") => !p.s("source").starts_with("Insert"),
        ("tree-diagram", "mark") => matches!(p.s("answer"), "Pre-order" | "In-order" | "Post-order" | "Level-order"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "tree-diagram" => vec![
            Example { name: "Binary search tree", params: vec![] },
            Example { name: "In-order traversal", params: vec![("answer", s("In-order"))] },
            Example { name: "Sorted values (a lopsided tree)", params: vec![("values", s("1, 2, 3, 4, 5, 6, 7")), ("answer", s("Height and depths"))] },
            Example {
                name: "File folders",
                params: vec![("source", s("Parent lines")), ("spec", s("root: docs, pics, music\ndocs: report, notes\npics: trip, pets, school\nmusic: rock")), ("answer", s("Pre-order"))],
            },
            Example {
                name: "Binary tree with gaps",
                params: vec![("source", s("Parent lines")), ("spec", s("A: B, C\nB: D, _\nC: _, F\nF: G, H")), ("answer", s("All four traversals"))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("source", "Build the tree from", &["Insert values (binary search tree)", "Parent lines"], "Insert values (binary search tree)")
            .hint("Values are inserted in order: smaller to the left, larger to the right, repeats ignored"),
        ParamDef::data("values", "Values", 1, "8, 3, 10, 1, 6, 14, 4, 7, 13"),
        ParamDef::long("spec", "Tree", "A: B, C\nB: D, E\nC: F").hint("One line per parent: A: B, C. A _ is an empty slot, which makes it a binary tree. The root is the one node nobody lists as a child"),
        ParamDef::choice("answer", "Work out", &ANSWERS, "None").hint("Worked out under the tree. Show answers off leaves the heading and the room for students to fill in"),
        ParamDef::bool("mark", "Number the nodes in that order", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the worked-out answer and the numbers on the tree, for students to work out"),
    ]
}

fn message(msg: &str) -> String {
    let mut d = Doc::new(520.0, 110.0);
    chart::text(&mut d, 260.0, 60.0, 15.0, "middle", msg);
    d.finish()
}

fn build(p: &Params) -> Result<Tree, String> {
    if p.s("source").starts_with("Insert") {
        Ok(Tree::bst(&data::numbers(p.s("values"))))
    } else {
        Tree::parse(p.s("spec"))
    }
}

/// The caption and rows of the chosen answer, and the visiting order to number on the nodes.
fn work(t: &Tree, p: &Params) -> (String, Vec<String>, Vec<usize>) {
    let names = |v: &[usize]| v.iter().map(|i| t.labels[*i].clone()).collect::<Vec<_>>().join(", ");
    let one = |label: &str, v: Option<Vec<usize>>| -> (String, Vec<String>, Vec<usize>) {
        match v {
            Some(v) => (label.to_string(), vec![names(&v)], v),
            None => (label.to_string(), vec!["In-order needs a binary tree (at most two children each)".into()], vec![]),
        }
    };
    match p.s("answer") {
        "Pre-order" => one("Pre-order (node, then children)", Some(t.preorder())),
        "In-order" => one("In-order (left, node, right)", t.inorder()),
        "Post-order" => one("Post-order (children, then node)", Some(t.postorder())),
        "Level-order" => one("Level-order (row by row)", Some(t.level_order())),
        "All four traversals" => {
            let rows = vec![
                format!("Pre-order:   {}", names(&t.preorder())),
                format!("In-order:    {}", t.inorder().map_or("needs a binary tree".to_string(), |v| names(&v))),
                format!("Post-order:  {}", names(&t.postorder())),
                format!("Level-order: {}", names(&t.level_order())),
            ];
            ("Traversals".into(), rows, vec![])
        }
        "Height and depths" => {
            let d = t.depths();
            let mut rows = vec![format!("Height: {}", t.height())];
            rows.push(format!("Depths: {}", (0..t.labels.len()).map(|i| format!("{} = {}", t.labels[i], d[i])).collect::<Vec<_>>().join(", ")));
            ("Height and depths".into(), rows, vec![])
        }
        _ => (String::new(), vec![], vec![]),
    }
}

fn render(p: &Params) -> String {
    let t = match build(p) {
        Ok(t) => t,
        Err(e) => return message(&e),
    };
    if t.labels.is_empty() {
        return message("Add some values or parent lines to draw a tree");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let title = p.s("title").trim();
    let n = t.labels.len();

    let radii: Vec<f64> = t.labels.iter().map(|l| gd::node_radius(l, fs)).collect();
    let rmax = radii.iter().copied().fold(0.0, f64::max);
    let cell = ((rmax * 2.0 + 22.0).max(54.0), (rmax * 2.0 + 40.0).max(80.0));
    let top = if title.is_empty() { 14.0 } else { 34.0 + fs * 1.3 };
    let (pts, dw, dh) = gd::to_px(&t.layout(), cell, (rmax + 24.0, rmax + 16.0));

    let (caption, rows, order) = work(&t, p);
    let numbered = answers && p.b("mark") && !order.is_empty();
    let row_h = fs * 1.65;
    let text_wid = rows.iter().chain(std::iter::once(&caption)).map(|r| gd::text_w(r, fs)).fold(0.0, f64::max);
    let block_h = if caption.is_empty() { 0.0 } else { 22.0 + row_h * (1 + rows.len()) as f64 };
    let w = dw.max(text_wid + 48.0).max(300.0);
    let h = top + dh + block_h + 14.0;
    let ox = (w - dw) / 2.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let at = |i: usize| (pts[i].0 + ox, pts[i].1 + top);
    for (i, kids) in t.kids.iter().enumerate() {
        for c in kids.iter().flatten() {
            gd::edge(&mut d, at(i), radii[i], at(*c), radii[*c], false, 0.0, INK, 2.0, "");
        }
    }
    let (fill, stroke) = if print { ("#fff", "#000") } else { ("#eef3fc", INK) };
    for i in 0..n {
        let c = at(i);
        gd::node(&mut d, c, radii[i], &t.labels[i], fs, &NodeStyle { fill, stroke, sw: 2.0, text: INK });
        if let (true, Some(no)) = (numbered, order.iter().position(|v| *v == i)) {
            let (bx, by) = (c.0 + radii[i] * 0.74, c.1 - radii[i] * 0.74);
            d.circle(bx, by, fs * 0.62, if print { "#000" } else { "#d97706" }, "#fff", 1.5);
            gd::text_fill(&mut d, bx, by + fs * 0.27, fs * 0.8, "#fff", &(no + 1).to_string());
        }
    }
    if !caption.is_empty() {
        let x0 = ((w - text_wid) / 2.0).max(24.0);
        let y = top + dh + 16.0;
        let y = gd::lines(&mut d, x0, y - row_h * 0.6, fs, &[caption.clone()]);
        d.line(x0, y + 3.0, x0 + gd::text_w(&caption, fs), y + 3.0, INK, 1.0);
        if answers {
            gd::lines(&mut d, x0, y + 6.0, fs, &rows);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn default_draws_the_search_tree() {
        let svg = render(&defaults());
        assert_eq!(svg.matches("<circle").count(), 9, "one circle per value");
        let mut p = defaults();
        p.set_s("values", "5, 5, 5");
        assert_eq!(render(&p).matches("<circle").count(), 1, "repeats are ignored");
    }

    #[test]
    fn traversals_are_worked_out_and_numbered() {
        let mut p = defaults();
        p.set_s("answer", "In-order");
        let svg = render(&p);
        assert!(svg.contains(">1, 3, 4, 6, 7, 8, 10, 13, 14<"));
        assert_eq!(svg.matches("#d97706").count(), 9, "a numbered badge on each node");
        p.set_s("answer", "Pre-order");
        assert!(render(&p).contains(">8, 3, 1, 6, 4, 7, 10, 14, 13<"));
        p.set_s("answer", "Post-order");
        assert!(render(&p).contains(">1, 4, 7, 6, 3, 13, 14, 10, 8<"));
        p.set_s("answer", "Level-order");
        assert!(render(&p).contains(">8, 3, 10, 1, 6, 14, 4, 7, 13<"));
        p.set_s("answer", "Height and depths");
        let s = render(&p);
        assert!(s.contains(">Height: 3<") && s.contains("8 = 0") && s.contains("13 = 3"));
        p.set_s("answer", "All four traversals");
        assert!(render(&p).contains("Level-order:"));
    }

    #[test]
    fn in_order_of_a_general_tree_says_why_not() {
        let mut p = defaults();
        p.set_s("source", "Parent lines");
        p.set_s("spec", "A: B, C, D");
        p.set_s("answer", "In-order");
        assert!(render(&p).contains("needs a binary tree"));
        p.set_s("answer", "Pre-order");
        assert!(render(&p).contains(">A, B, C, D<"));
    }

    #[test]
    fn student_copy_keeps_the_tree_and_the_heading_only() {
        let def = def();
        let mut p = defaults();
        p.set_s("answer", "In-order");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(student.contains("In-order (left, node, right)"));
        assert!(!student.contains(">1, 3, 4, 6, 7, 8, 10, 13, 14<") && key.contains(">1, 3, 4, 6, 7, 8, 10, 13, 14<"));
        assert!(!student.contains("#d97706") && key.contains("#d97706"));
        assert_eq!(size(&student), size(&key), "same size, so the copies can be swapped");
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("answer", "Pre-order");
        let svg = render(&p);
        assert!(!svg.contains("#d97706") && !svg.contains("#eef3fc"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_bad_specs_are_explained() {
        for ex in examples("tree-diagram").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                if let Value::S(s) = v {
                    p.set_s(k, s);
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<circle") && !svg.contains("NaN"), "{}", ex.name);
        }
        let mut p = defaults();
        p.set_s("source", "Parent lines");
        for (spec, needle) in [("A: B\nC: B", "two parents"), ("A: B\nB: A", "loop"), ("nonsense", "parent: child"), ("", "Add some")] {
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        assert!(relevant("tree-diagram", "spec", &p) && !relevant("tree-diagram", "values", &p));
    }

    #[test]
    fn hostile_input_never_panics_or_prints_nan() {
        let chain: String = (0..39).map(|i| format!("n{i}: n{}\n", i + 1)).collect();
        for source in ["Insert values (binary search tree)", "Parent lines"] {
            for text in ["", "abc", "7", "1e12, -1e12, 0", "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15", "A: B, C, D, E, F", "A: _, _", "A: B\nB: _, C\nC: D, _", &chain, "A: B, C\nB: A"] {
                for answer in ANSWERS {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_s("source", source);
                        p.set_s("values", text);
                        p.set_s("spec", text);
                        p.set_s("answer", answer);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?}");
                        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text:?} {answer}");
                    }
                }
            }
        }
    }
}
