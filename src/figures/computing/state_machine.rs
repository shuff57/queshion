//! State machine (finite automaton): states as circles, transitions as labelled arrows, a start
//! arrow and double circles for accepting states, from a short text spec. Under the drawing it can
//! trace an input string or give the transition table; the student copy (Show answers off) keeps
//! the headings and the empty table or trace line.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::graph::{Edge, Graph, Layout, Node};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_STATES: usize = 12;
const MAX_MOVES: usize = 40;
const MAX_SPEC: usize = 2000;
const MAX_NAME: usize = 8;
const MAX_INPUT: usize = 24;
const DEFAULT_SPEC: &str = "start q0\naccept q2\nq0 -> q0 : 0\nq0 -> q1 : 1\nq1 -> q1 : 1\nq1 -> q2 : 0\nq2 -> q2 : 0\nq2 -> q1 : 1";
const ANSWERS: [&str; 3] = ["None", "Trace an input string", "Transition table"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "state-machine",
        subject: Subject::Computing,
        title: "State machine",
        blurb: "A finite automaton from a short text spec: states, labelled transitions, a start arrow and accepting states, with an input string traced or the transition table worked out underneath.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("state-machine", "input") => p.s("answer") == "Trace an input string",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "state-machine" => vec![
            Example { name: "Strings ending in 10", params: vec![] },
            Example { name: "Trace an input", params: vec![("answer", s("Trace an input string")), ("input", s("0110"))] },
            Example { name: "Transition table", params: vec![("answer", s("Transition table"))] },
            Example {
                name: "Turnstile",
                params: vec![("spec", s("start Locked\nLocked -> Unlocked : coin\nLocked -> Locked : push\nUnlocked -> Locked : push\nUnlocked -> Unlocked : coin")), ("direction", s("Left to right"))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "State machine", DEFAULT_SPEC).hint("A -> B : 0, 1 is a transition on 0 or 1; start A marks the start state; accept B, C marks accepting states. Optional: states A, B, C to fix the order"),
        ParamDef::choice("direction", "Direction", &["Left to right", "Top to bottom"], "Left to right"),
        ParamDef::choice("answer", "Work out", &ANSWERS, "None").hint("Worked out under the diagram. Show answers off leaves the heading and the empty table or line for students"),
        ParamDef::text("input", "Input string", "0110").hint("Symbols separated by spaces, or written together when each is one character"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the table cells and the trace for students to work out"),
    ]
}

struct Machine {
    states: Vec<String>,
    start: usize,
    accept: Vec<usize>,
    moves: Vec<(usize, usize, String)>,
    alphabet: Vec<String>,
}

fn names(s: &str) -> Vec<&str> {
    s.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).collect()
}

fn valid(n: &str) -> Result<(), String> {
    if n.chars().count() > MAX_NAME || !n.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(format!("\u{201c}{n}\u{201d} is not a usable state name (up to {MAX_NAME} letters, digits or _)"));
    }
    Ok(())
}

fn parse(spec: &str) -> Result<Machine, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("A state machine is limited to {MAX_SPEC} characters"));
    }
    let mut states: Vec<String> = Vec::new();
    let mut start: Option<String> = None;
    let mut accept: Vec<String> = Vec::new();
    let mut raw: Vec<(String, String, String)> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let add = |states: &mut Vec<String>, n: &str| -> Result<usize, String> {
        valid(n)?;
        if let Some(i) = states.iter().position(|s| s == n) {
            return Ok(i);
        }
        if states.len() >= MAX_STATES {
            return Err(format!("A state machine is limited to {MAX_STATES} states"));
        }
        states.push(n.to_string());
        Ok(states.len() - 1)
    };
    for line in spec.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (head, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match head.to_lowercase().as_str() {
            "states" => {
                for n in names(rest) {
                    add(&mut states, n)?;
                }
            }
            "start" => {
                let n = names(rest);
                if n.len() != 1 {
                    return Err("start takes one state, such as: start q0".into());
                }
                valid(n[0])?;
                pending.push(n[0].to_string());
                start = Some(n[0].to_string());
            }
            "accept" | "accepting" | "final" => {
                for n in names(rest) {
                    valid(n)?;
                    pending.push(n.to_string());
                    accept.push(n.to_string());
                }
            }
            _ if line.contains("->") => {
                let (path, labels) = line.split_once(':').ok_or_else(|| format!("\u{201c}{line}\u{201d} needs a label after a colon, such as A -> B : 0"))?;
                let (a, b) = path.split_once("->").ok_or("expected A -> B : label")?;
                let (a, b) = (a.trim(), b.trim());
                add(&mut states, a)?;
                add(&mut states, b)?;
                let labels = names(labels);
                if labels.is_empty() {
                    return Err(format!("{a} -> {b} has no label"));
                }
                for l in labels {
                    raw.push((a.to_string(), b.to_string(), l.chars().take(8).collect()));
                }
            }
            _ => return Err(format!("\u{201c}{line}\u{201d} is not a state line (start, accept, states) or a transition (A -> B : 0)")),
        }
    }
    for n in &pending {
        add(&mut states, n)?;
    }
    if states.is_empty() {
        return Ok(Machine { states, start: 0, accept: vec![], moves: vec![], alphabet: vec![] });
    }
    let idx = |n: &str| states.iter().position(|s| s == n).unwrap_or(0);
    let start = start.map_or(0, |s| idx(&s));
    let mut moves: Vec<(usize, usize, String)> = Vec::new();
    for (a, b, l) in raw {
        let m = (idx(&a), idx(&b), l);
        if !moves.contains(&m) {
            moves.push(m);
        }
    }
    if moves.len() > MAX_MOVES {
        return Err(format!("A state machine is limited to {MAX_MOVES} transitions"));
    }
    let mut alphabet: Vec<String> = Vec::new();
    for (_, _, l) in &moves {
        if !alphabet.contains(l) {
            alphabet.push(l.clone());
        }
    }
    alphabet.sort();
    let accept = accept.iter().map(|n| idx(n)).collect();
    Ok(Machine { states, start, accept, moves, alphabet })
}

impl Machine {
    fn next(&self, from: &[usize], sym: &str) -> Vec<usize> {
        let mut out: Vec<usize> = self.moves.iter().filter(|(a, _, l)| l == sym && from.contains(a)).map(|(_, b, _)| *b).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    fn set_text(&self, s: &[usize]) -> String {
        match s.len() {
            0 => "\u{2205}".to_string(),
            1 => self.states[s[0]].clone(),
            _ => format!("{{{}}}", s.iter().map(|i| self.states[*i].as_str()).collect::<Vec<_>>().join(", ")),
        }
    }
}

fn symbols(input: &str, m: &Machine) -> Vec<String> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() > 1 || parts.iter().any(|p| m.alphabet.iter().any(|a| a == p)) {
        return parts.into_iter().take(MAX_INPUT).map(String::from).collect();
    }
    input.chars().filter(|c| !c.is_whitespace()).take(MAX_INPUT).map(|c| c.to_string()).collect()
}

fn trace(m: &Machine, input: &str) -> (String, String) {
    let syms = symbols(input, m);
    let mut cur = vec![m.start];
    let mut line = m.set_text(&cur);
    for s in &syms {
        cur = m.next(&cur, s);
        line += &format!(" \u{2192}({s}) {}", m.set_text(&cur));
        if cur.is_empty() {
            break;
        }
    }
    let ok = cur.iter().any(|c| m.accept.contains(c));
    (line, if ok { "Accepted".into() } else { "Rejected".into() })
}

fn render(p: &Params) -> String {
    let m = match parse(p.s("spec")) {
        Ok(m) => m,
        Err(e) => return chart::message(&e),
    };
    if m.states.is_empty() {
        return chart::message("Add some states and transitions to draw a machine");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let n = m.states.len();
    let title = p.s("title").trim();
    let horizontal = p.s("direction") != "Top to bottom";

    // layout: the start state first, then the rest, as layers
    let order: Vec<usize> = std::iter::once(m.start).chain((0..n).filter(|i| *i != m.start)).collect();
    let pos_of = |s: usize| order.iter().position(|o| *o == s).unwrap_or(0);
    let mut seen: Vec<(usize, usize)> = Vec::new();
    for (a, b, _) in &m.moves {
        if a != b && !seen.contains(&(pos_of(*a), pos_of(*b))) {
            seen.push((pos_of(*a), pos_of(*b)));
        }
    }
    let g = Graph {
        nodes: order.iter().map(|s| Node { name: m.states[*s].clone(), pos: None }).collect(),
        edges: seen.iter().map(|(a, b)| Edge { a: *a, b: *b, directed: true, weight: None }).collect(),
    };
    let grid0 = g.layout(Layout::Layers);
    let grid: Vec<(f64, f64)> = (0..n).map(|s| { let g = grid0[pos_of(s)]; if horizontal { (g.1, g.0) } else { g } }).collect();
    let radii: Vec<f64> = m.states.iter().map(|s| gd::node_radius(s, fs) + 3.0).collect();
    let rmax = radii.iter().copied().fold(0.0, f64::max);
    let cell = (rmax * 2.0 + 86.0, rmax * 2.0 + 70.0);
    let loops = m.moves.iter().any(|(a, b, _)| a == b);
    let (pts, dw0, dh0) = gd::to_px(&grid, cell, (rmax + 50.0, rmax + if loops { 54.0 } else { 40.0 }));
    let top = if title.is_empty() { 8.0 } else { 34.0 + fs * 1.3 };

    // the answer block
    let row_h = fs * 1.9;
    let answer = p.s("answer");
    let cols = m.alphabet.len();
    let col_w = (fs * 4.2).max(gd::text_w("{q10, q11}", fs) * 0.8);
    let first_w = (fs * 4.2).max(m.states.iter().map(|s| gd::text_w(s, fs)).fold(0.0, f64::max) + 36.0);
    let table_w = first_w + cols as f64 * col_w;
    let input_line = format!("Input: {}", symbols(p.s("input"), &m).join(" "));
    let (trace_line, verdict) = trace(&m, p.s("input"));
    let block_h = match answer {
        "Transition table" => 14.0 + row_h * (n + 1) as f64,
        "Trace an input string" => 14.0 + row_h * 3.0,
        _ => 0.0,
    };
    let w = dw0.max(table_w + 40.0).max(if answer == "Trace an input string" { gd::text_w(&trace_line, fs).max(gd::text_w(&input_line, fs)) + 48.0 } else { 0.0 }).max(300.0);
    let h = top + dh0 + block_h + 10.0;
    let ox = (w - dw0) / 2.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let at = |s: usize| (pts[s].0 + ox, pts[s].1 + top);

    // transitions, merged by (from, to) so their labels share one arrow
    let mut groups: Vec<((usize, usize), Vec<String>)> = Vec::new();
    for (a, b, l) in &m.moves {
        match groups.iter_mut().find(|(k, _)| *k == (*a, *b)) {
            Some((_, v)) => v.push(l.clone()),
            None => groups.push(((*a, *b), vec![l.clone()])),
        }
    }
    for ((a, b), labels) in &groups {
        let text = labels.join(", ");
        if a == b {
            let (c, r) = (at(*a), radii[*a]);
            let (x0, x1, y) = (c.0 - r * 0.6, c.0 + r * 0.6, c.1 - r * 0.8);
            let lift = r * 0.9 + 14.0;
            d.path(&format!("M {x0:.1} {y:.1} C {:.1} {:.1}, {:.1} {:.1}, {x1:.1} {y:.1}", x0 - 14.0, y - lift, x1 + 14.0, y - lift), "none", INK, 2.0);
            d.poly(&[(x1, y), (x1 + 6.5, y - 9.0), (x1 - 4.5, y - 10.5)], INK, INK, 0.5);
            gd::tag(&mut d, (c.0, c.1 - r * 0.8 - lift * 0.75 - 2.0), fs * 0.9, &text);
        } else {
            let reverse = groups.iter().any(|((x, y), _)| (*x, *y) == (*b, *a));
            let mid = gd::edge(&mut d, at(*a), radii[*a], at(*b), radii[*b], true, if reverse { 9.0 } else { 0.0 }, INK, 2.0, "");
            gd::tag(&mut d, (mid.0, mid.1 + if reverse { 0.0 } else { 0.0 }), fs * 0.9, &text);
        }
    }
    // states, accepting ones double
    let fill = if print { "#fff" } else { "#eef3fc" };
    for s in 0..n {
        let c = at(s);
        if m.accept.contains(&s) {
            d.circle(c.0, c.1, radii[s], "none", INK, 2.0);
        }
        let r = if m.accept.contains(&s) { radii[s] - 5.0 } else { radii[s] - 3.0 };
        gd::node(&mut d, c, r, &m.states[s], fs, &gd::NodeStyle { fill, stroke: INK, sw: 2.0, text: INK });
    }
    // the start arrow comes in from the side the layers grow from
    let sc = at(m.start);
    let (sx, sy) = if horizontal { (sc.0 - radii[m.start] - 38.0, sc.1) } else { (sc.0, sc.1 - radii[m.start] - 38.0) };
    d.arrow(sx, sy, sc.0 - if horizontal { radii[m.start] } else { 0.0 }, sc.1 - if horizontal { 0.0 } else { radii[m.start] }, INK, 2.0);

    // the answer block
    let y0 = top + dh0 + 10.0;
    match answer {
        "Transition table" => {
            let x0 = (w - table_w) / 2.0;
            let head = if print { "#e5e5e5" } else { "#eef1f6" };
            d.rect(x0, y0, first_w, row_h, head, INK, 1.2);
            for (c, a) in m.alphabet.iter().enumerate() {
                let x = x0 + first_w + c as f64 * col_w;
                d.rect(x, y0, col_w, row_h, head, INK, 1.2);
                gd::text_fill(&mut d, x + col_w / 2.0, y0 + row_h / 2.0 + fs * 0.35, fs, INK, a);
            }
            for s in 0..n {
                let y = y0 + row_h * (s + 1) as f64;
                let name = format!("{}{}{}", if s == m.start { "\u{2192} " } else { "" }, if m.accept.contains(&s) { "* " } else { "" }, m.states[s]);
                d.rect(x0, y, first_w, row_h, head, INK, 1.2);
                gd::text_fill(&mut d, x0 + first_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, &name);
                for (c, a) in m.alphabet.iter().enumerate() {
                    let x = x0 + first_w + c as f64 * col_w;
                    d.rect(x, y, col_w, row_h, "#fff", INK, 1.2);
                    if answers {
                        gd::text_fill(&mut d, x + col_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, &m.set_text(&m.next(&[s], a)));
                    }
                }
            }
        }
        "Trace an input string" => {
            let x0 = 24.0;
            let mut y = gd::lines(&mut d, x0, y0, fs, &[input_line.clone()]);
            y = gd::lines(&mut d, x0, y, fs, &["Path:".to_string()]);
            if answers {
                gd::lines(&mut d, x0 + 52.0, y - fs * 1.65, fs, &[trace_line.clone()]);
                gd::lines(&mut d, x0, y, fs, &[verdict.clone()]);
            } else {
                d.line(x0 + 52.0, y - 5.0, w - 24.0, y - 5.0, INK, 1.0);
                gd::lines(&mut d, x0, y, fs, &["Accepted or rejected:".to_string()]);
                d.line(x0 + gd::text_w("Accepted or rejected:", fs) + 8.0, y + row_h - 8.0, x0 + gd::text_w("Accepted or rejected:", fs) + 120.0, y + row_h - 8.0, INK, 1.0);
            }
        }
        _ => {}
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn spec_parses_with_start_accept_and_merged_labels() {
        let m = parse("start A\naccept B\nA -> B : 0, 1\nB -> B : x\nA -> B : 0").unwrap();
        assert_eq!(m.states, ["A", "B"]);
        assert_eq!((m.start, m.accept.clone()), (0, vec![1]));
        assert_eq!(m.moves.len(), 3, "a repeated transition is one");
        assert_eq!(m.alphabet, ["0", "1", "x"]);
        let m = parse("states X, Y, Z\nstart Z\nX -> Y : a").unwrap();
        assert_eq!(m.states, ["X", "Y", "Z"]);
        assert_eq!(m.start, 2);
    }

    #[test]
    fn tracing_runs_the_machine() {
        let m = parse(DEFAULT_SPEC).unwrap();
        let (line, verdict) = trace(&m, "0110");
        assert_eq!(line, "q0 \u{2192}(0) q0 \u{2192}(1) q1 \u{2192}(1) q1 \u{2192}(0) q2");
        assert_eq!(verdict, "Accepted");
        assert_eq!(trace(&m, "01").1, "Rejected");
        // missing transitions get stuck on the empty set; nondeterminism tracks sets
        let (l, v) = trace(&parse("start A\naccept B\nA -> B : a").unwrap(), "ab");
        assert!(l.ends_with("\u{2205}") && v == "Rejected", "{l}");
        let nfa = parse("start A\naccept C\nA -> A : a\nA -> B : a\nB -> C : b").unwrap();
        let (l, v) = trace(&nfa, "ab");
        assert!(l.contains("{A, B}") && v == "Accepted", "{l}");
        let words = parse("start A\naccept B\nA -> B : go").unwrap();
        assert_eq!(symbols("go go", &words), ["go", "go"]);
        assert_eq!(symbols("go", &words), ["go"]);
    }

    #[test]
    fn default_draws_states_accept_ring_and_start_arrow() {
        let svg = render(&defaults());
        for t in [">q0<", ">q1<", ">q2<", ">0<", ">1<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert_eq!(svg.matches("<circle").count(), 3 + 1, "three states and one accepting ring");
        assert!(svg.contains("<path"), "self loops are arcs");
    }

    #[test]
    fn answers_are_worked_out_and_hidden_for_students() {
        let def = def();
        let mut p = defaults();
        p.set_s("answer", "Transition table");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(key.contains(">q1<") && key.matches(">q2<").count() > student.matches(">q2<").count());
        assert_eq!(student.matches("<rect").count(), key.matches("<rect").count());
        assert_eq!(size_of(&student), size_of(&key));
        assert!(key.contains("\u{2192} q0") && key.contains("* q2"));
        p.set_s("answer", "Trace an input string");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(key.contains("Accepted") && !student.contains(">Accepted<") && student.contains("Input: 0 1 1 0"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [("A -> B", "needs a label"), ("start", "takes one state"), ("dance", "not a state line"), ("A -> B :", "has no label"), ("toolongstatename -> B : 1", "not a usable"), ("", "Add some states")] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("answer", "Transition table");
        let svg = render(&p);
        assert!(!svg.contains("#eef3fc") && !svg.contains("#eef1f6"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("state-machine").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                if let Value::S(s) = v {
                    p.set_s(k, s);
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<circle") && !svg.contains("NaN"), "{}", ex.name);
        }
        let many: String = (0..40).map(|i| format!("s{i} -> s{} : a\n", (i + 1) % 40)).collect();
        for spec in ["", "\u{0}", "start A", "A -> A : x", "A -> B : x\nB -> A : x\nA -> A : x, y, z", &many, "states A, B\nstart B\naccept A, B"] {
            for answer in ANSWERS {
                for dir in ["Left to right", "Top to bottom"] {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_s("spec", spec);
                        p.set_s("answer", answer);
                        p.set_s("direction", dir);
                        p.set_s("input", "x\u{0}y a b 1 0 1 0 1 0 1 0 1 0 1 0 1 0 1 0 1 0 1 0 1 0 1 0 1 0");
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?} {answer}");
                    }
                }
            }
        }
    }
}
