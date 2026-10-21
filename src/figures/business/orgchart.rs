//! An organisation chart from an indented list of roles: each role is a box, joined to the role it
//! reports to. The structure is the question; the number of levels, the largest span of control
//! and the number of roles are the answers, so the student copy (Show answers off) leaves them as
//! blank lines and can also leave the boxes empty for the role titles to be filled in.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const HIDE: [&str; 2] = ["Nothing", "Role titles"];
const MAX_NODES: usize = 30;
const MAX_DEPTH: usize = 6;
const MAX_LEAVES: usize = 10;
const MAX_SPEC: usize = 1500;
const INK: &str = "#111827";
const DEFAULT: &str = "Managing Director\n  Finance Manager\n    Accountant\n    Payroll Clerk\n  Operations Manager\n    Production Supervisor\n    Warehouse Supervisor\n  Sales Manager\n    Sales Representative";

pub fn def() -> FigureDef {
    FigureDef {
        id: "org-chart",
        subject: Subject::Business,
        title: "Organisation chart",
        blurb: "An organisation chart from an indented list of roles, with the levels, span of control and number of roles worked out, or with the role titles and answers left blank for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("org-chart", "hide") => !p.b(SHOW_ANSWERS),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "org-chart" => vec![
            Example { name: "Three levels", params: vec![] },
            Example { name: "A small shop", params: vec![("roles", s("Owner\n  Shop Manager\n    Cashier\n    Stock Assistant\n  Bookkeeper"))] },
            Example { name: "A flat structure", params: vec![("roles", s("Principal\n  Teacher A\n  Teacher B\n  Teacher C\n  Teacher D\n  Teacher E"))] },
            Example { name: "Blank role titles", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDE[1]))] },
            Example { name: "Blank answers only", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("roles", "Roles, indented under who they report to", DEFAULT).hint("One role per line; indent a role under its manager with spaces. One role at the top; up to 30 roles, 6 levels"),
        ParamDef::choice("hide", "Leave blank for students", &HIDE, HIDE[0]).hint("Used when Show answers is off"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the levels, the span of control and the number of roles blank, and the role titles too if chosen"),
    ]
}

#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub depth: usize,
    pub children: Vec<usize>,
}

pub fn parse(spec: &str) -> Result<Vec<Node>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The roles are limited to {MAX_SPEC} characters"));
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for line in spec.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indent: usize = line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum();
        let name: String = line.trim().trim_start_matches(['-', '*', '\u{2022}']).trim().chars().take(28).collect();
        if name.is_empty() {
            continue;
        }
        while stack.last().map_or(false, |(ind, _)| *ind >= indent) {
            stack.pop();
        }
        let depth = stack.len();
        if depth >= MAX_DEPTH {
            return Err(format!("A chart is limited to {MAX_DEPTH} levels"));
        }
        let idx = nodes.len();
        if let Some(&(_, parent)) = stack.last() {
            nodes[parent].children.push(idx);
        } else if idx > 0 {
            return Err("Only one role can be at the top: indent the others under it".into());
        }
        nodes.push(Node { name, depth, children: Vec::new() });
        stack.push((indent, idx));
        if nodes.len() > MAX_NODES {
            return Err(format!("A chart is limited to {MAX_NODES} roles"));
        }
    }
    if nodes.is_empty() {
        return Err("Give some roles, one per line, indented under their manager".into());
    }
    Ok(nodes)
}

/// Slots (columns) for the leaves left to right, each role centred over its children.
pub fn layout(nodes: &[Node]) -> Vec<f64> {
    fn go(nodes: &[Node], i: usize, next: &mut f64, x: &mut [f64]) {
        if nodes[i].children.is_empty() {
            x[i] = *next;
            *next += 1.0;
        } else {
            for &c in &nodes[i].children {
                go(nodes, c, next, x);
            }
            let (a, b) = (nodes[i].children[0], *nodes[i].children.last().unwrap());
            x[i] = (x[a] + x[b]) / 2.0;
        }
    }
    let mut x = vec![0.0; nodes.len()];
    go(nodes, 0, &mut 0.0, &mut x);
    x
}

/// The role with the most direct reports and how many it has.
pub fn widest_span(nodes: &[Node]) -> (usize, usize) {
    nodes.iter().enumerate().fold((0, 0), |best, (i, n)| if n.children.len() > best.1 { (i, n.children.len()) } else { best })
}

fn wrap(name: &str) -> Vec<String> {
    if name.chars().count() <= 15 || !name.contains(' ') {
        return vec![name.to_string()];
    }
    let mid = name.chars().count() / 2;
    let mut best = (usize::MAX, 0);
    for (i, c) in name.char_indices() {
        if c == ' ' {
            let d = (name[..i].chars().count() as isize - mid as isize).unsigned_abs();
            if d < best.0 {
                best = (d, i);
            }
        }
    }
    vec![name[..best.1].to_string(), name[best.1 + 1..].to_string()]
}

fn render(p: &Params) -> String {
    let nodes = match parse(p.s("roles")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let leaves = nodes.iter().filter(|n| n.children.is_empty()).count();
    if leaves > MAX_LEAVES {
        return chart::message(&format!("The chart is too wide: at most {MAX_LEAVES} roles can sit at the bottom of their branches"));
    }
    let answers = p.b(SHOW_ANSWERS);
    let blank_titles = !answers && p.s("hide") == HIDE[1];
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (bw, bh, vgap, slot) = (126.0, 46.0, 52.0, 142.0);
    let levels = nodes.iter().map(|n| n.depth).max().unwrap() + 1;
    let xs = layout(&nodes);
    let width = (leaves as f64 * slot + 60.0).max(560.0);
    let h = top + 40.0 + levels as f64 * (bh + vgap) + 150.0;
    let mut d = Doc::new(width, h);
    if !title.is_empty() {
        chart::text(&mut d, width / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let x_of = |i: usize| 30.0 + (xs[i] + 0.5) * slot + (width - 60.0 - leaves as f64 * slot) / 2.0;
    let y_of = |depth: usize| top + 30.0 + depth as f64 * (bh + vgap);
    // connectors first
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
        let (a, b) = (x_of(n.children[0]), x_of(*n.children.last().unwrap()));
        d.line(a.min(px), my, b.max(px), my, INK, 1.6);
        for &c in &n.children {
            d.line(x_of(c), my, x_of(c), y_of(nodes[c].depth), INK, 1.6);
        }
    }
    let tints = ["#dbe7fb", "#e1f2e3", "#fdeccf", "#f3dcf0", "#dcecf0", "#ececec"];
    for (i, n) in nodes.iter().enumerate() {
        let (cx, y) = (x_of(i), y_of(n.depth));
        let fill = if print { if n.depth % 2 == 0 { "#e8e8e8" } else { "#f7f7f7" } } else { tints[n.depth % 6] };
        d.raw(&format!(r##"<rect x="{:.2}" y="{y:.2}" width="{bw}" height="{bh}" rx="6" fill="{fill}" stroke="{INK}" stroke-width="1.6"/>"##, cx - bw / 2.0));
        if blank_titles {
            continue;
        }
        let lines = wrap(&n.name);
        for (k, l) in lines.iter().enumerate() {
            let ty = y + bh / 2.0 + fs * 0.35 - (lines.len() as f64 - 1.0) * fs * 0.6 + k as f64 * fs * 1.2;
            chart::text(&mut d, cx, ty, fs * 0.92, "middle", l);
        }
    }
    // what is asked
    let (wi, ws) = widest_span(&nodes);
    let mut y = y_of(levels - 1) + bh + 50.0;
    let items = [
        ("Levels in the hierarchy:", levels.to_string()),
        ("Largest span of control:", format!("{} ({} direct reports)", nodes[wi].name, ws)),
        ("Number of roles:", nodes.len().to_string()),
    ];
    for (label, val) in items {
        chart::text(&mut d, 40.0, y, fs, "start", label);
        if answers {
            chart::text(&mut d, 40.0 + 230.0, y, fs, "start", &val);
        } else {
            d.line(40.0 + 230.0, y + 3.0, 40.0 + 470.0, y + 3.0, INK, 1.0);
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
    fn indentation_makes_the_tree() {
        let n = parse("A\n  B\n    C\n  D\n").unwrap();
        assert_eq!(n.len(), 4);
        assert_eq!(n[0].children, vec![1, 3]);
        assert_eq!(n[1].children, vec![2]);
        assert_eq!((n[2].depth, n[3].depth), (2, 1));
        // tabs, dashes and uneven indents
        let n = parse("A\n\t- B\n\t\t- C\n   * D").unwrap();
        assert_eq!(n[0].children, vec![1, 3]);
        assert_eq!(n[1].children, vec![2]);
    }

    #[test]
    fn leaves_take_slots_and_parents_centre() {
        let n = parse("A\n B\n  C\n  D\n E").unwrap();
        let x = layout(&n);
        assert_eq!((x[2], x[3], x[4]), (0.0, 1.0, 2.0));
        assert_eq!(x[1], 0.5);
        assert_eq!(x[0], 1.25);
        assert_eq!(widest_span(&n), (0, 2));
    }

    #[test]
    fn key_answers_the_questions() {
        let svg = render(&defaults());
        assert!(svg.contains(">3<") && svg.contains(">9<") && svg.contains("Managing Director (3 direct reports)"));
        assert!(svg.contains(">Accountant<") && svg.contains(">Production<") && svg.contains(">Supervisor<"), "long titles are on two lines");
    }

    #[test]
    fn student_copy_blanks_answers_and_optionally_titles() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Accountant<") && !student.contains("direct reports") && key.contains("direct reports"));
        assert_eq!(size_of(&student), size_of(&key));
        let mut p = defaults();
        p.set_s("hide", HIDE[1]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(!student.contains("Accountant") && key.contains("Accountant"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_charts_are_explained() {
        for (spec, needle) in [("", "Give some roles"), ("A\nB", "Only one role"), ("  A\nB", "Only one role"), ("A\n B\n  C\n   D\n    E\n     F\n      G", "limited to 6 levels")] {
            let mut p = defaults();
            p.set_s("roles", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let mut p = defaults();
        p.set_s("roles", &format!("Top\n{}", " x\n".repeat(31)));
        assert!(render(&p).contains("limited to 30"));
        p.set_s("roles", &format!("Top\n{}", " x\n".repeat(11)));
        assert!(render(&p).contains("too wide"));
        p.set_s("roles", &"x\n ".repeat(900));
        assert!(render(&p).contains("limited to 1500"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("org-chart").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("Only one") && !svg.contains("too wide"), "{}", ex.name);
        }
        for spec in ["\u{0}", "<b>x</b>", "nan", "-\n-\n-", "A\n\u{0}", &"w ".repeat(400)] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("roles", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
