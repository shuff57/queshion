//! Data structures: an array, stack, queue or linked list drawn from a list of values, with an
//! optional list of operations ("push 5, pop") worked out into the structure that results. The
//! student copy (Show answers off) draws the starting structure and an empty dashed box where the
//! result goes, the same size as the key's, so the two copies swap.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const KINDS: [&str; 4] = ["Array", "Stack", "Queue", "Linked list"];
const MAX_ITEMS: usize = 14;
const MAX_TOKEN: usize = 8;

pub fn def() -> FigureDef {
    FigureDef {
        id: "data-structure",
        subject: Subject::Computing,
        title: "Array, stack, queue, list",
        blurb: "An array, stack, queue or linked list drawn from a list of values, with operations such as push and pop worked out into the structure that results.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("data-structure", "ops") => p.s("kind") != "Array",
        ("data-structure", "indices") => p.s("kind") == "Array",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "data-structure" => vec![
            Example { name: "Array with indices", params: vec![] },
            Example { name: "Stack: push and pop", params: vec![("kind", s("Stack")), ("values", s("3, 8")), ("ops", s("push 5, push 9, pop, push 1"))] },
            Example { name: "Queue: enqueue and dequeue", params: vec![("kind", s("Queue")), ("values", s("A, B, C")), ("ops", s("dequeue, enqueue D, enqueue E, dequeue"))] },
            Example { name: "Linked list", params: vec![("kind", s("Linked list")), ("values", s("12, 7, 30")), ("ops", s("addfirst 4, removelast, addlast 9"))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Structure", &KINDS, "Array"),
        ParamDef::text("values", "Values", "7, 3, 9, 4, 1").hint("From the start (array), bottom to top (stack), front to back (queue) or head to tail (list)"),
        ParamDef::text("ops", "Operations", "").hint("Stack: push 5, pop. Queue: enqueue 5, dequeue. List: addfirst 5, addlast 5, removefirst, removelast. Leave empty for just the structure"),
        ParamDef::bool("indices", "Show array indices", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off draws the starting structure and an empty box where the result goes"),
    ]
}

fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).take(MAX_ITEMS).map(|t| t.chars().take(MAX_TOKEN).collect()).collect()
}

/// Applies each operation in turn; an impossible one (pop on empty, a full structure, an
/// unknown word) is skipped and reported.
fn apply(kind: &str, start: &[String], ops: &str) -> (Vec<String>, Vec<String>) {
    let mut v = start.to_vec();
    let mut notes = Vec::new();
    for op in ops.split(',').map(str::trim).filter(|o| !o.is_empty()).take(24) {
        let mut it = op.split_whitespace();
        let word = it.next().unwrap_or("").to_lowercase();
        let arg: String = it.next().unwrap_or("").chars().take(MAX_TOKEN).collect();
        let needs_arg = matches!(word.as_str(), "push" | "enqueue" | "addfirst" | "addlast");
        if needs_arg && arg.is_empty() {
            notes.push(format!("{word} needs a value"));
            continue;
        }
        let ok = match (kind, word.as_str()) {
            ("Stack", "push") | ("Queue", "enqueue") | ("Linked list", "addlast") => (v.len() < MAX_ITEMS).then(|| v.push(arg.clone())),
            ("Stack", "pop") | ("Linked list", "removelast") => v.pop().map(|_| ()),
            ("Queue", "dequeue") | ("Linked list", "removefirst") => (!v.is_empty()).then(|| {
                v.remove(0);
            }),
            ("Linked list", "addfirst") => (v.len() < MAX_ITEMS).then(|| v.insert(0, arg.clone())),
            _ => {
                notes.push(format!("{word} is not a {} operation", kind.to_lowercase()));
                continue;
            }
        };
        if ok.is_none() {
            notes.push(format!("{op}: {}", if v.len() >= MAX_ITEMS { "full" } else { "empty" }));
        }
    }
    (v, notes)
}

struct Style {
    cw: f64,
    ch: f64,
    fs: f64,
    print: bool,
}

impl Style {
    fn fill(&self, hot: bool) -> &'static str {
        match (hot, self.print) {
            (true, true) => "#d9d9d9",
            (true, false) => "#ffe9a8",
            (false, true) => "#fff",
            (false, false) => "#eef3fc",
        }
    }

    fn cell(&self, d: &mut Doc, x: f64, y: f64, w: f64, text: &str, hot: bool) {
        d.rect(x, y, w, self.ch, self.fill(hot), INK, 2.0);
        gd::text_fill(d, x + w / 2.0, y + self.ch / 2.0 + self.fs * 0.35, self.fs, INK, text);
    }

    fn label(&self, d: &mut Doc, x: f64, y: f64, anchor: &str, s: &str) {
        chart::text(d, x, y, self.fs * 0.85, anchor, s);
    }

    fn node_w(&self) -> f64 {
        self.cw + 30.0
    }
}

const LIST_GAP: f64 = 28.0;

/// The size a structure of `n` items takes (so the key and student copies agree).
fn size(kind: &str, n: usize, st: &Style, indices: bool) -> (f64, f64) {
    let n = n.max(1) as f64;
    let lab = st.fs * 1.7;
    match kind {
        "Stack" => (st.cw + 110.0, n * st.ch + 14.0),
        "Queue" => (n * st.cw + 20.0, st.ch + lab * 2.0),
        "Linked list" => (n * (st.node_w() + LIST_GAP) + 60.0, st.ch + lab + 6.0),
        _ => (n * st.cw + 20.0, st.ch + if indices { lab } else { 0.0 } + 6.0),
    }
}

fn draw(d: &mut Doc, kind: &str, items: &[String], x: f64, y: f64, st: &Style, indices: bool) {
    let lab = st.fs * 1.7;
    let n = items.len();
    if n == 0 {
        st.label(d, x + 10.0, y + st.ch / 2.0 + st.fs * 0.3, "start", "(empty)");
        return;
    }
    match kind {
        "Stack" => {
            for (i, t) in items.iter().enumerate() {
                let cy = y + 7.0 + (n - 1 - i) as f64 * st.ch;
                st.cell(d, x + 10.0, cy, st.cw, t, i == n - 1);
            }
            let ty = y + 7.0 + st.ch / 2.0;
            d.arrow(x + st.cw + 50.0, ty, x + st.cw + 18.0, ty, INK, 2.0);
            st.label(d, x + st.cw + 54.0, ty + st.fs * 0.3, "start", "top");
        }
        "Queue" => {
            for (i, t) in items.iter().enumerate() {
                st.cell(d, x + 10.0 + i as f64 * st.cw, y + lab, st.cw, t, i == 0);
            }
            st.label(d, x + 10.0 + st.cw / 2.0, y + lab - 6.0, "middle", "front");
            st.label(d, x + 10.0 + (n as f64 - 0.5) * st.cw, y + lab + st.ch + lab * 0.8, "middle", "back");
        }
        "Linked list" => {
            for (i, t) in items.iter().enumerate() {
                let nx = x + 10.0 + i as f64 * (st.node_w() + LIST_GAP);
                let hot = i == 0;
                d.rect(nx, y + lab, st.cw, st.ch, st.fill(hot), INK, 2.0);
                gd::text_fill(d, nx + st.cw / 2.0, y + lab + st.ch / 2.0 + st.fs * 0.35, st.fs, INK, t);
                d.rect(nx + st.cw, y + lab, 30.0, st.ch, st.fill(false), INK, 2.0);
                let (px, py) = (nx + st.cw + 15.0, y + lab + st.ch / 2.0);
                d.circle(px, py, 3.0, INK, INK, 1.0);
                if i + 1 < n {
                    d.arrow(px, py, nx + st.node_w() + LIST_GAP - 2.0, py, INK, 2.0);
                } else {
                    d.line(px, py, px + LIST_GAP - 4.0, py, INK, 2.0);
                    st.label(d, px + LIST_GAP, py + st.fs * 0.3, "start", "null");
                }
            }
            st.label(d, x + 10.0 + st.cw / 2.0, y + lab - 6.0, "middle", "head");
        }
        _ => {
            for (i, t) in items.iter().enumerate() {
                st.cell(d, x + 10.0 + i as f64 * st.cw, y, st.cw, t, false);
                if indices {
                    st.label(d, x + 10.0 + (i as f64 + 0.5) * st.cw, y + st.ch + lab * 0.8, "middle", &i.to_string());
                }
            }
        }
    }
}

fn render(p: &Params) -> String {
    let kind = KINDS.iter().copied().find(|k| *k == p.s("kind")).unwrap_or("Array");
    let items = tokens(p.s("values"));
    if items.is_empty() {
        return chart::message("Add some values to draw");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 16.0 * k;
    let ops = if kind == "Array" { "" } else { p.s("ops").trim() };
    let (after, notes) = apply(kind, &items, ops);
    let longest = items.iter().chain(after.iter()).map(|t| t.chars().count()).max().unwrap_or(1);
    let st = Style { cw: (longest as f64 * fs * 0.62 + 24.0).max(52.0), ch: fs * 2.6, fs, print };
    let indices = p.b("indices");
    let title = p.s("title").trim();
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 0.4 };
    let row = fs * 1.65;
    let (bw, bh) = size(kind, items.len(), &st, indices);
    let (aw, ah) = size(kind, after.len(), &st, indices);
    let caption = |s: &str| gd::text_w(s, fs) + 40.0;
    let ops_line = format!("Operations: {ops}");
    let mut w = bw.max(if ops.is_empty() { 0.0 } else { aw.max(caption(&ops_line)) }) + 20.0;
    w = w.max(300.0);
    let mut h = top + bh + 10.0;
    if !ops.is_empty() {
        h += row * 2.0 + ah + 10.0 + notes.len() as f64 * row;
    }
    let mut d = Doc::new(w, h + 8.0);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let center = |iw: f64| (w - iw) / 2.0;
    draw(&mut d, kind, &items, center(bw), top, &st, indices);
    if !ops.is_empty() {
        let mut y = top + bh + 10.0;
        let x0 = 24.0;
        y = gd::lines(&mut d, x0, y, fs, &[ops_line.clone()]);
        y = gd::lines(&mut d, x0, y, fs, &["Result:".to_string()]);
        let ay = y + 4.0;
        if answers {
            draw(&mut d, kind, &after, center(aw), ay, &st, indices);
            let mut ny = ay + ah + 6.0;
            for n in &notes {
                ny = gd::lines(&mut d, x0, ny - row * 0.4, fs * 0.9, &[format!("Skipped: {n}")]);
            }
        } else {
            d.raw(&format!(
                r##"<rect x="{:.2}" y="{ay:.2}" width="{:.2}" height="{ah:.2}" fill="none" stroke="{INK}" stroke-width="1.5" stroke-dasharray="6 5"/>"##,
                center(aw) + 6.0,
                aw - 12.0
            ));
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    fn with(kind: &str, values: &str, ops: &str) -> Params {
        let mut p = defaults();
        p.set_s("kind", kind);
        p.set_s("values", values);
        p.set_s("ops", ops);
        p
    }

    #[test]
    fn operations_are_worked_out() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(apply("Stack", &s(&["3", "8"]), "push 5, push 9, pop, push 1").0, s(&["3", "8", "5", "1"]));
        assert_eq!(apply("Queue", &s(&["A", "B", "C"]), "dequeue, enqueue D").0, s(&["B", "C", "D"]));
        assert_eq!(apply("Linked list", &s(&["12", "7"]), "addfirst 4, removelast, addlast 9").0, s(&["4", "12", "9"]));
        let (v, notes) = apply("Stack", &s(&["1"]), "pop, pop, dequeue, push");
        assert!(v.is_empty() && notes.len() == 3, "{notes:?}");
        assert!(notes[0].contains("empty") && notes[1].contains("not a stack") && notes[2].contains("needs a value"));
    }

    #[test]
    fn each_kind_draws_its_marks() {
        let arr = render(&defaults());
        assert!(arr.contains(">0<") && arr.contains(">4<") && arr.contains(">7<"));
        let mut p = defaults();
        p.set_b("indices", false);
        assert!(!render(&p).contains(">4<") || !render(&p).contains(">0<"));
        assert!(render(&with("Stack", "1, 2", "")).contains(">top<"));
        let q = render(&with("Queue", "1, 2", ""));
        assert!(q.contains(">front<") && q.contains(">back<"));
        let l = render(&with("Linked list", "1, 2", ""));
        assert!(l.contains(">head<") && l.contains(">null<"));
    }

    #[test]
    fn student_copy_leaves_the_result_empty_at_the_same_size() {
        let def = def();
        let p = with("Stack", "3, 8", "push 5, push 9, pop, push 1");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(student.contains(">8<") && student.contains("Operations: push 5"), "the start is shown");
        assert!(student.contains("stroke-dasharray") && !key.contains("stroke-dasharray"));
        assert!(key.matches(">1<").count() > student.matches(">1<").count());
        assert_eq!(size_of(&student), size_of(&key));
        assert!(!student.contains(">top<") || key.matches(">top<").count() > student.matches(">top<").count());
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = with("Linked list", "1, 2, 3", "addfirst 0");
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#ffe9a8") && !svg.contains("#eef3fc"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("data-structure").unwrap() {
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
        let many: String = (0..80).map(|i| format!("{i} ")).collect();
        let pushes: String = (0..60).map(|_| "push 1, ").collect();
        for kind in KINDS {
            for values in ["", "x", &many, "averyveryverylongvalue, 2", "\u{0}", ",,,"] {
                for ops in ["", "pop", &pushes, "push", "enqueue 1, dequeue, dequeue", ",,,", "frobnicate 3"] {
                    for answers in [true, false] {
                        let mut p = with(kind, values, ops);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{kind} {values:?} {ops:?}");
                    }
                }
            }
        }
    }
}
