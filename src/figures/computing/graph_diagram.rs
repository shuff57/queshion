//! Graph: nodes and edges from a short text spec, directed or not, weighted or not, drawn round a
//! circle or as layers from the first node. The answers a worksheet asks for (adjacency list or
//! matrix, degrees, breadth- or depth-first order, the shortest path) are worked out under the
//! drawing; the student copy (Show answers off) keeps the heading and leaves the room to fill in.
use super::graph_draw::{self as gd, NodeStyle};
use crate::figures::chart::{self, INK};
use crate::figures::graph::{Graph, Layout};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const DEFAULT_SPEC: &str = "nodes A, B, C, D, E\nA - B : 4\nA - C : 2\nB - D : 5\nC - D : 8\nD - E : 3";
const ANSWERS: [&str; 7] = ["None", "Adjacency list", "Adjacency matrix", "Degrees", "Breadth-first order", "Depth-first order", "Shortest path"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "graph-diagram",
        subject: Subject::Computing,
        title: "Graph",
        blurb: "A graph of nodes and edges from a short text spec (directed or not, weighted or not), with the adjacency list or matrix, degrees, traversal order or shortest path worked out underneath.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("graph-diagram", "start") => matches!(p.s("answer"), "Breadth-first order" | "Depth-first order" | "Shortest path"),
        ("graph-diagram", "end") => p.s("answer") == "Shortest path",
        ("graph-diagram", "mark") => matches!(p.s("answer"), "Breadth-first order" | "Depth-first order" | "Shortest path"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "graph-diagram" => vec![
            Example { name: "Weighted graph", params: vec![] },
            Example {
                name: "Shortest path",
                params: vec![("answer", s("Shortest path")), ("start", s("A")), ("end", s("E"))],
            },
            Example {
                name: "Breadth-first search",
                params: vec![("spec", s("nodes A, B, C, D, E, F\nA - B\nA - C\nB - D\nB - E\nC - F")), ("weights", Value::B(false)), ("answer", s("Breadth-first order")), ("start", s("A"))],
            },
            Example {
                name: "Directed graph",
                params: vec![("spec", s("A -> B\nA -> C\nB -> D\nC -> D\nD -> A")), ("weights", Value::B(false)), ("answer", s("Adjacency matrix"))],
            },
            Example {
                name: "Network on a circle",
                params: vec![("spec", s("nodes P1, P2, P3, P4, P5, P6\nP1 - P2\nP2 - P3\nP3 - P4\nP4 - P5\nP5 - P6\nP6 - P1\nP1 - P4")), ("weights", Value::B(false)), ("answer", s("Degrees"))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Graph", DEFAULT_SPEC).hint("nodes A, B, C lists nodes; A - B is an edge, A -> B a directed one, A - B : 4 a weight. Add node A @ 2,1 to place a node"),
        ParamDef::choice("layout", "Layout", &["Circle", "Layers from the first node"], "Circle"),
        ParamDef::bool("weights", "Show edge weights", true),
        ParamDef::choice("answer", "Work out", &ANSWERS, "None").hint("Worked out under the graph. Show answers off leaves the heading and the room for students to fill in"),
        ParamDef::text("start", "Start node", "").hint("Empty means the first node"),
        ParamDef::text("end", "End node", "").hint("Empty means the last node"),
        ParamDef::bool("mark", "Mark it on the graph", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the worked-out answer and the marks on the graph, for students to work out"),
    ]
}

fn message(msg: &str) -> String {
    let mut d = Doc::new(520.0, 110.0);
    chart::text(&mut d, 260.0, 60.0, 15.0, "middle", msg);
    d.finish()
}

fn node_named(g: &Graph, name: &str, default: usize) -> Result<usize, String> {
    let n = name.trim();
    if n.is_empty() {
        return Ok(default);
    }
    g.index(n).ok_or_else(|| format!("{n} is not in the graph"))
}

/// What the "Work out" choice says, as a caption, the rows of its answer, and the nodes and
/// edges to mark (node order for a traversal, edge ids for a path).
struct Work {
    caption: String,
    rows: Vec<String>,
    visit: Vec<usize>,
    path: Vec<usize>,
}

fn weight_text(w: f64) -> String {
    crate::figures::chart::fmt(w)
}

fn work(g: &Graph, p: &Params) -> Work {
    let mut w = Work { caption: String::new(), rows: Vec::new(), visit: Vec::new(), path: Vec::new() };
    let name = |i: usize| g.nodes[i].name.clone();
    let any_dir = g.edges.iter().any(|e| e.directed);
    match p.s("answer") {
        "Adjacency list" => {
            w.caption = "Adjacency list".into();
            for i in 0..g.nodes.len() {
                let outs: Vec<String> = g
                    .edges
                    .iter()
                    .filter_map(|e| if e.a == i { Some((e.b, e.weight)) } else if !e.directed && e.b == i { Some((e.a, e.weight)) } else { None })
                    .map(|(n, wt)| match wt {
                        Some(x) if p.b("weights") => format!("{}({})", name(n), weight_text(x)),
                        _ => name(n),
                    })
                    .collect();
                let mut sorted = outs;
                sorted.sort_by_key(|s| g.index(s.split('(').next().unwrap_or("")).unwrap_or(usize::MAX));
                w.rows.push(format!("{}: {}", name(i), if sorted.is_empty() { "\u{2014}".to_string() } else { sorted.join(", ") }));
            }
        }
        "Degrees" => {
            w.caption = if any_dir { "Degrees (in and out)".into() } else { "Degrees".into() };
            for (i, (din, dout)) in g.degrees().into_iter().enumerate() {
                w.rows.push(if any_dir { format!("{}: in {din}, out {dout}", name(i)) } else { format!("{}: {din}", name(i)) });
            }
        }
        a @ ("Breadth-first order" | "Depth-first order") => {
            let start = match node_named(g, p.s("start"), 0) {
                Ok(s) => s,
                Err(e) => {
                    w.caption = e;
                    return w;
                }
            };
            w.caption = format!("{} from {}", a, name(start));
            w.visit = if a.starts_with("Breadth") { g.bfs(start) } else { g.dfs(start) };
            let mut line = w.visit.iter().map(|i| name(*i)).collect::<Vec<_>>().join(", ");
            let missed: Vec<String> = (0..g.nodes.len()).filter(|i| !w.visit.contains(i)).map(name).collect();
            if !missed.is_empty() {
                line += &format!("   (not reached: {})", missed.join(", "));
            }
            w.rows.push(line);
        }
        "Shortest path" => {
            let last = g.nodes.len().saturating_sub(1);
            let (a, b) = match (node_named(g, p.s("start"), 0), node_named(g, p.s("end"), last)) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => {
                    w.caption = e;
                    return w;
                }
            };
            w.caption = format!("Shortest path from {} to {}", name(a), name(b));
            match g.shortest_path(a, b) {
                Some((path, total)) => {
                    w.rows.push(format!("{}   (total {})", path.iter().map(|i| name(*i)).collect::<Vec<_>>().join(" \u{2192} "), weight_text(total)));
                    w.path = path;
                }
                None => w.rows.push("There is no path".into()),
            }
        }
        _ => {}
    }
    w
}

fn render(p: &Params) -> String {
    let g = match Graph::parse(p.s("spec")) {
        Ok(g) => g,
        Err(e) => return message(&e),
    };
    if g.nodes.is_empty() {
        return message("Add some nodes and edges to draw a graph");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let title = p.s("title").trim();
    let n = g.nodes.len();

    let layout = if p.s("layout").starts_with("Layers") { Layout::Layers } else { Layout::Circle };
    let grid = g.layout(layout);
    let radii: Vec<f64> = g.nodes.iter().map(|x| gd::node_radius(&x.name, fs)).collect();
    let rmax = radii.iter().copied().fold(0.0, f64::max);
    let cell = ((rmax * 2.0 + 56.0).max(96.0), (rmax * 2.0 + 44.0).max(84.0));
    let top = if title.is_empty() { 20.0 } else { 38.0 + fs * 1.3 };
    let margin = (rmax + 24.0, rmax + 20.0);
    let (pts, dw, dh) = gd::to_px(&grid, cell, margin);

    // what to work out and mark
    let wk = work(&g, p);
    let marked = answers && p.b("mark");
    let visit_no = |i: usize| wk.visit.iter().position(|v| *v == i).map(|k| k + 1);
    let on_path_edge = |e: &crate::figures::graph::Edge| wk.path.windows(2).any(|w| (w[0], w[1]) == (e.a, e.b) || (!e.directed && (w[0], w[1]) == (e.b, e.a)));
    let path_node = |i: usize| wk.path.contains(&i);

    // the answer block: its caption, then rows, or the matrix table
    let want_matrix = p.s("answer") == "Adjacency matrix";
    let cellw = (gd::text_w("00.0", fs) + 12.0).max(fs * 2.4);
    let mat_w = (n + 1) as f64 * cellw;
    let row_h = fs * 1.65;
    let block_h = if p.s("answer") == "None" {
        0.0
    } else if want_matrix {
        22.0 + row_h + (n + 1) as f64 * row_h
    } else {
        22.0 + row_h * (1 + wk.rows.len()) as f64
    };
    let text_wid = wk.rows.iter().chain(std::iter::once(&wk.caption)).map(|r| gd::text_w(r, fs)).fold(0.0, f64::max);
    let w = (dw.max(if want_matrix { mat_w } else { text_wid } + 48.0)).max(300.0);
    let h = top + dh + block_h + 16.0;
    let ox = (w - dw) / 2.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let at = |i: usize| (pts[i].0 + ox, pts[i].1 + top);

    // edges: a pair of opposite arrows is drawn side by side
    let (hi_col, lo_col) = if print { ("#000", "#000") } else { ("#d97706", INK) };
    for e in &g.edges {
        let hot = marked && on_path_edge(e);
        let (col, sw) = if hot { (hi_col, if print { 4.2 } else { 3.6 }) } else { (lo_col, 2.0) };
        let reverse = e.directed && g.edges.iter().any(|f| f.directed && (f.a, f.b) == (e.b, e.a));
        let mid = gd::edge(&mut d, at(e.a), radii[e.a], at(e.b), radii[e.b], e.directed, if reverse { 6.0 } else { 0.0 }, col, sw, "");
        if let (Some(wt), true) = (e.weight, p.b("weights")) {
            gd::tag(&mut d, mid, fs * 0.85, &weight_text(wt));
        }
    }
    for i in 0..n {
        let hot = marked && (path_node(i) || visit_no(i).is_some());
        let (fill, stroke, sw) = match (hot, print) {
            (true, true) => ("#d9d9d9", "#000", 3.4),
            (true, false) => ("#ffe9a8", "#d97706", 2.8),
            (false, true) => ("#fff", "#000", 2.0),
            (false, false) => ("#eef3fc", INK, 2.0),
        };
        let c = at(i);
        gd::node(&mut d, c, radii[i], &g.nodes[i].name, fs, &NodeStyle { fill, stroke, sw, text: INK });
        if let (true, Some(no)) = (marked, visit_no(i)) {
            let (bx, by) = (c.0 + radii[i] * 0.72, c.1 - radii[i] * 0.72);
            d.circle(bx, by, fs * 0.62, if print { "#000" } else { "#d97706" }, "#fff", 1.5);
            gd::text_fill(&mut d, bx, by + fs * 0.27, fs * 0.8, "#fff", &no.to_string());
        }
    }

    // the answer block
    let mut y = top + dh;
    if p.s("answer") != "None" {
        y += 16.0;
        let x0 = (w - if want_matrix { mat_w } else { text_wid }) / 2.0;
        y = gd::lines(&mut d, x0.max(24.0), y - row_h * 0.6, fs, &[wk.caption.clone()]) + row_h * 0.0;
        // heading in bold would need another font: underline it instead
        d.line(x0.max(24.0), y + 3.0, x0.max(24.0) + gd::text_w(&wk.caption, fs), y + 3.0, INK, 1.0);
        if want_matrix {
            let m = g.matrix();
            let tx = (w - mat_w) / 2.0;
            let ty = y + 8.0;
            for r in 0..=n {
                for c in 0..=n {
                    let head = r == 0 || c == 0;
                    let fill = if head { if print { "#e5e5e5" } else { "#eef1f6" } } else { "#fff" };
                    d.rect(tx + c as f64 * cellw, ty + r as f64 * row_h, cellw, row_h, fill, INK, 1.2);
                    let text = match (r, c) {
                        (0, 0) => None,
                        (0, c) => Some(g.nodes[c - 1].name.clone()),
                        (r, 0) => Some(g.nodes[r - 1].name.clone()),
                        (r, c) if answers => Some(m[r - 1][c - 1].map_or("0".to_string(), |x| if p.b("weights") { weight_text(x) } else { "1".into() })),
                        _ => None,
                    };
                    if let Some(t) = text {
                        chart::text(&mut d, tx + (c as f64 + 0.5) * cellw, ty + (r as f64 + 0.5) * row_h + fs * 0.35, fs, "middle", &t);
                    }
                }
            }
        } else if answers {
            let x0 = (w - text_wid) / 2.0;
            gd::lines(&mut d, x0.max(24.0), y + 6.0, fs, &wk.rows);
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
    fn default_draws_every_node_edge_and_weight() {
        let svg = render(&defaults());
        assert_eq!(svg.matches("<circle").count(), 5);
        for w in ["4", "2", "5", "8", "3"] {
            assert!(svg.contains(&format!(">{w}<")), "weight {w}");
        }
        let mut p = defaults();
        p.set_b("weights", false);
        assert!(!render(&p).contains(">8<"));
    }

    #[test]
    fn each_answer_is_worked_out_correctly() {
        let mut p = defaults();
        let text = |p: &Params| render(p);
        p.set_s("answer", "Shortest path");
        p.set_s("start", "A");
        p.set_s("end", "E");
        assert!(text(&p).contains("A \u{2192} B \u{2192} D \u{2192} E") && text(&p).contains("(total 12)"));
        p.set_s("answer", "Breadth-first order");
        assert!(text(&p).contains(">A, B, C, D, E<"));
        p.set_s("answer", "Depth-first order");
        assert!(text(&p).contains(">A, B, D, C, E<"), "B, then D, then back to C");
        p.set_s("answer", "Adjacency list");
        let s = text(&p);
        assert!(s.contains(">A: B(4), C(2)<") && s.contains(">E: D(3)<"), "{s}");
        p.set_s("answer", "Degrees");
        assert!(text(&p).contains(">D: 3<"));
        p.set_s("spec", "A -> B\nB -> C");
        assert!(text(&p).contains(">B: in 1, out 1<"));
        p.set_s("answer", "Adjacency matrix");
        p.set_s("spec", "A - B : 7\nB - C");
        let s = text(&p);
        assert!(s.contains(">7<") && s.contains(">1<") && s.contains(">0<"));
    }

    #[test]
    fn unreachable_nodes_and_missing_paths_are_said_so() {
        let mut p = defaults();
        p.set_s("spec", "A - B\nnodes C");
        p.set_s("answer", "Breadth-first order");
        assert!(render(&p).contains("not reached: C"));
        p.set_s("answer", "Shortest path");
        p.set_s("start", "A");
        p.set_s("end", "C");
        assert!(render(&p).contains("There is no path"));
        p.set_s("end", "Z");
        assert!(render(&p).contains("Z is not in the graph"));
    }

    #[test]
    fn student_copy_keeps_the_graph_and_the_heading_only() {
        let def = def();
        let mut p = defaults();
        p.set_s("answer", "Breadth-first order");
        p.set_s("start", "A");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(student.contains("Breadth-first order from A") && key.contains("Breadth-first order from A"));
        assert!(key.contains(">A, B, C, D, E<") && !student.contains(">A, B, C, D, E<"));
        assert!(key.contains("#d97706") && !student.contains("#d97706"), "no marks on the student graph");
        assert_eq!(size(&student), size(&key), "same size, so the copies can be swapped");
        // the matrix keeps its empty grid for the student
        p.set_s("answer", "Adjacency matrix");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert_eq!(student.matches("<rect").count(), key.matches("<rect").count());
        assert!(key.matches(">0<").count() > 0 && student.matches(">0<").count() == 0, "the student grid is empty");
        assert_eq!(size(&student), size(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("answer", "Shortest path");
        p.set_s("start", "A");
        p.set_s("end", "E");
        let svg = render(&p);
        assert!(!svg.contains("#d97706") && !svg.contains("#eef3fc"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn layouts_and_examples_render() {
        for layout in ["Circle", "Layers from the first node"] {
            let mut p = defaults();
            p.set_s("layout", layout);
            assert_eq!(render(&p).matches("<circle").count(), 5, "{layout}");
        }
        for ex in examples("graph-diagram").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<circle"), "{} draws a graph, not a message", ex.name);
            assert!(!svg.contains("NaN"), "{}", ex.name);
        }
        let mut p = defaults();
        p.set_s("spec", "A -> B\nB -> A");
        assert!(render(&p).contains("<polygon") || render(&p).contains("<path"), "directed edges have arrow heads");
        assert!(relevant("graph-diagram", "end", &{ let mut q = defaults(); q.set_s("answer", "Shortest path"); q }) && !relevant("graph-diagram", "end", &defaults()));
    }

    #[test]
    fn hostile_input_never_panics_or_prints_nan() {
        let huge: String = (0..60).map(|i| format!("n{i} - n{}\n", (i + 1) % 60)).collect();
        for spec in ["", "\u{0}", "A", "nodes A\nnode A @ 1000,-1000", "A - B\nnode A @ 0,0\nnode B @ 0,0", "A - B : 1e9\nB - C : -5", &huge, "nodes A, B, C\n", "aaaaaaaaaaaa - bbbbbbbbbbbb : 123456789"] {
            for answer in ANSWERS {
                for layout in ["Circle", "Layers from the first node"] {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_s("spec", spec);
                        p.set_s("answer", answer);
                        p.set_s("layout", layout);
                        p.set_s("start", "q");
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{spec:?} {answer}");
                        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{spec:?} {answer}");
                    }
                }
            }
        }
    }
}
