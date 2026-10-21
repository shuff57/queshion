//! A family tree from an indented list: each line is a person or a couple ("Anna + Ben"), with
//! their children indented underneath. The tree is the question; the number of generations, the
//! people in the tree and the descendants of the first couple are the answers, so the student copy
//! (Show answers off) leaves them blank and can also empty the name boxes.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use super::wrap;

const HIDE: [&str; 2] = ["Nothing", "Names"];
const MAX_NODES: usize = 24;
const MAX_DEPTH: usize = 5;
const MAX_LEAVES: usize = 8;
const INK: &str = "#111827";
const DEFAULT: &str = "George + Martha\n  John + Anne\n    Sam\n    Jo + Pat\n  Mary + Tom\n    Lucy\n  Peter";

pub fn def() -> FigureDef {
    FigureDef {
        id: "family-tree",
        subject: Subject::Language,
        title: "Family tree",
        blurb: "A family tree of couples and their children from an indented list, with the generations, number of people and descendants worked out, or with blank name boxes and answers for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("family-tree", "hide") => !p.b(SHOW_ANSWERS),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "family-tree" => vec![
            Example { name: "Three generations", params: vec![] },
            Example { name: "A small family", params: vec![("people", s("Rosa + Luis\n  Ana\n  Marco"))] },
            Example { name: "Single parent", params: vec![("people", s("Dina\n  Kai\n    Mia\n    Noor\n  Esi"))] },
            Example { name: "Blank names", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDE[1]))] },
            Example { name: "Blank answers only", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("people", "People, children indented under their parents", DEFAULT).hint("One person per line, or a couple as Anna + Ben; indent children under their parents. One person or couple at the top; up to 24 lines, 5 generations"),
        ParamDef::choice("hide", "Leave blank for students", &HIDE, HIDE[0]).hint("Used when Show answers is off"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the questions blank, and the names too if chosen"),
    ]
}

#[derive(Debug)]
pub struct Node {
    pub names: Vec<String>,
    pub depth: usize,
    pub children: Vec<usize>,
}

pub fn parse(spec: &str) -> Result<Vec<Node>, String> {
    if spec.chars().count() > 1500 {
        return Err("The people are limited to 1500 characters".into());
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for line in spec.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indent: usize = line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum();
        let names: Vec<String> = line.trim().split('+').map(|n| n.trim().chars().take(16).collect::<String>()).filter(|n| !n.is_empty()).take(2).collect();
        if names.is_empty() {
            continue;
        }
        while stack.last().map_or(false, |(ind, _)| *ind >= indent) {
            stack.pop();
        }
        let depth = stack.len();
        if depth >= MAX_DEPTH {
            return Err(format!("A tree is limited to {MAX_DEPTH} generations"));
        }
        let idx = nodes.len();
        if let Some(&(_, parent)) = stack.last() {
            nodes[parent].children.push(idx);
        } else if idx > 0 {
            return Err("Only one person or couple can be at the top: indent the others under them".into());
        }
        nodes.push(Node { names, depth, children: Vec::new() });
        stack.push((indent, idx));
        if nodes.len() > MAX_NODES {
            return Err(format!("A tree is limited to {MAX_NODES} lines"));
        }
    }
    if nodes.is_empty() {
        return Err("Give some people, one per line, with children indented under their parents".into());
    }
    Ok(nodes)
}

/// Leaf slots left to right, each parent centred over its children.
pub fn layout(nodes: &[Node]) -> Vec<f64> {
    fn go(nodes: &[Node], i: usize, next: &mut f64, x: &mut [f64]) {
        if nodes[i].children.is_empty() {
            x[i] = *next;
            *next += 1.0;
        } else {
            for &c in &nodes[i].children {
                go(nodes, c, next, x);
            }
            x[i] = (x[nodes[i].children[0]] + x[*nodes[i].children.last().unwrap()]) / 2.0;
        }
    }
    let mut x = vec![0.0; nodes.len()];
    go(nodes, 0, &mut 0.0, &mut x);
    x
}

/// (generations, people in all, descendants of the first person or couple).
pub fn counts(nodes: &[Node]) -> (usize, usize, usize) {
    (nodes.iter().map(|n| n.depth).max().unwrap_or(0) + 1, nodes.iter().map(|n| n.names.len()).sum(), nodes.len() - 1)
}

fn render(p: &Params) -> String {
    let nodes = match parse(p.s("people")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let leaves = nodes.iter().filter(|n| n.children.is_empty()).count();
    if leaves > MAX_LEAVES {
        return chart::message(&format!("The tree is too wide: at most {MAX_LEAVES} people can be at the ends of branches"));
    }
    let answers = p.b(SHOW_ANSWERS);
    let blank_names = !answers && p.s("hide") == HIDE[1];
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (bw, bh, vgap) = (116.0, 40.0, 56.0);
    let couple = nodes.iter().any(|n| n.names.len() == 2);
    let slot = if couple { 2.0 * bw + 40.0 } else { bw + 30.0 };
    let xs = layout(&nodes);
    let width = (leaves as f64 * slot + 60.0).max(560.0);
    let levels = nodes.iter().map(|n| n.depth).max().unwrap() + 1;
    let h = top + 40.0 + levels as f64 * (bh + vgap) + 150.0;
    let mut d = Doc::new(width, h);
    if !title.is_empty() {
        chart::text(&mut d, width / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let x_of = |i: usize| 30.0 + (xs[i] + 0.5) * slot + (width - 60.0 - leaves as f64 * slot) / 2.0;
    let y_of = |depth: usize| top + 30.0 + depth as f64 * (bh + vgap);
    let node_w = |n: &Node| if n.names.len() == 2 { 2.0 * bw + 14.0 } else { bw };
    for (i, n) in nodes.iter().enumerate() {
        if n.children.is_empty() {
            continue;
        }
        let (px, py) = (x_of(i), y_of(n.depth) + bh);
        let my = py + vgap / 2.0;
        if n.children.len() == 1 {
            d.line(px, py, x_of(n.children[0]), y_of(nodes[n.children[0]].depth), INK, 1.6);
            continue;
        }
        d.line(px, py, px, my, INK, 1.6);
        d.line(x_of(n.children[0]).min(px), my, x_of(*n.children.last().unwrap()).max(px), my, INK, 1.6);
        for &c in &n.children {
            d.line(x_of(c), my, x_of(c), y_of(nodes[c].depth), INK, 1.6);
        }
    }
    let tints = ["#dbe7fb", "#e1f2e3", "#fdeccf", "#f3dcf0", "#dcecf0"];
    for (i, n) in nodes.iter().enumerate() {
        let (cx, y) = (x_of(i), y_of(n.depth));
        let fill = if print { if n.depth % 2 == 0 { "#e8e8e8" } else { "#f7f7f7" } } else { tints[n.depth % 5] };
        let total = node_w(n);
        for (k, name) in n.names.iter().enumerate() {
            let x = if n.names.len() == 2 { cx - total / 2.0 + k as f64 * (bw + 14.0) } else { cx - bw / 2.0 };
            d.raw(&format!(r##"<rect x="{x:.2}" y="{y:.2}" width="{bw}" height="{bh}" rx="6" fill="{fill}" stroke="{INK}" stroke-width="1.6"/>"##));
            if !blank_names {
                let lines = wrap(name, 14, 2);
                for (j, l) in lines.iter().enumerate() {
                    chart::text(&mut d, x + bw / 2.0, y + bh / 2.0 + fs * 0.35 - (lines.len() as f64 - 1.0) * fs * 0.6 + j as f64 * fs * 1.2, fs * 0.95, "middle", l);
                }
            }
        }
        if n.names.len() == 2 {
            // the marriage line joining the two boxes
            d.line(cx - 7.0, y + bh / 2.0, cx + 7.0, y + bh / 2.0, INK, 2.4);
        }
    }
    let (gens, people, desc) = counts(&nodes);
    let mut y = y_of(levels - 1) + bh + 50.0;
    for (label, val) in [("Generations in the tree:", gens), ("People in the tree:", people), ("Descendants of the first couple (or person):", desc)] {
        chart::text(&mut d, 40.0, y, fs, "start", label);
        if answers {
            chart::text(&mut d, 40.0 + 340.0, y, fs, "start", &val.to_string());
        } else {
            d.line(40.0 + 340.0, y + 3.0, 40.0 + 460.0, y + 3.0, INK, 1.0);
        }
        y += fs * 1.9;
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn couples_and_children_parse() {
        let n = parse("A + B\n  C + D\n    E\n  F").unwrap();
        assert_eq!(n[0].names, vec!["A", "B"]);
        assert_eq!(n[0].children, vec![1, 3]);
        assert_eq!(n[2].depth, 2);
        let n = parse("A + B + C").unwrap();
        assert_eq!(n[0].names.len(), 2, "a third name is ignored");
        assert_eq!(parse("+ +\nA").unwrap()[0].names, vec!["A"]);
    }

    #[test]
    fn the_counts_are_right() {
        let n = parse(DEFAULT).unwrap();
        // George+Martha; John+Anne, Sam, Jo+Pat; Mary+Tom, Lucy; Peter: 11 people, 6 descendant lines
        assert_eq!(counts(&n), (3, 11, 6));
        let n = parse("Solo").unwrap();
        assert_eq!(counts(&n), (1, 1, 0));
    }

    #[test]
    fn key_answers_and_student_blanks() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains(">11<") && key.contains(">Sam<") && !student.contains(">11<") && student.contains(">Sam<"));
        assert_eq!(size_of(&student), size_of(&key));
        let mut p = defaults();
        p.set_s("hide", HIDE[1]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(!student.contains(">Sam<") && key.contains(">Sam<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_trees_are_explained() {
        for (spec, needle) in [("", "Give some people"), ("A\nB", "Only one person"), ("A\n B\n  C\n   D\n    E\n     F", "limited to 5 generations")] {
            let mut p = defaults();
            p.set_s("people", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let mut p = defaults();
        p.set_s("people", &format!("Top\n{}", " x\n".repeat(25)));
        assert!(render(&p).contains("limited to 24"));
        p.set_s("people", &format!("Top\n{}", " x\n".repeat(9)));
        assert!(render(&p).contains("too wide"));
        p.set_s("people", &"x\n".repeat(900));
        assert!(render(&p).contains("1500"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("family-tree").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("Only one") && !svg.contains("too wide"), "{}", ex.name);
        }
        for spec in ["\u{0}", "<b>x</b>", "nan + nan", "+", "A +\n +B", &"w ".repeat(400)] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("people", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
