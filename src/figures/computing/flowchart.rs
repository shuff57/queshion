//! Flowchart: boxes, diamonds and rounded ends joined by arrows, from a short text spec
//! (`A: start Begin`, `B: decision n > 0?`, `A -> B`, `B -> C : yes`). Laid out in layers from the
//! first box, with loops drawn round the right-hand side. The student copy (Show answers off)
//! leaves the boxes named in "Blank in student copy" empty, at their full size, to fill in.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_STEPS: usize = 30;
const MAX_LINKS: usize = 60;
const MAX_SPEC: usize = 3000;
const MAX_ID: usize = 8;
const MAX_TEXT: usize = 40;
const DEFAULT_SPEC: &str = "S: start Start\nI: input Read a number n\nD: decision n > 0?\nP: output Print \"positive\"\nN: output Print \"not positive\"\nE: end End\nS -> I -> D\nD -> P : yes\nD -> N : no\nP -> E\nN -> E";

pub fn def() -> FigureDef {
    FigureDef {
        id: "flowchart",
        subject: Subject::Computing,
        title: "Flowchart",
        blurb: "A flowchart of start/end, process, input/output and decision boxes from a short text spec, laid out top to bottom, with chosen boxes left empty in the student copy.",
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
        "flowchart" => vec![
            Example { name: "Positive or not", params: vec![] },
            Example { name: "Fill in the decision", params: vec![("blank", s("D, P"))] },
            Example {
                name: "Loop: add up to 10",
                params: vec![(
                    "spec",
                    s("S: start Start\nA: process total = 0, i = 1\nC: decision i <= 10?\nB: process total = total + i\nD: process i = i + 1\nO: output Print total\nE: end End\nS -> A -> C\nC -> B : yes\nB -> D\nD -> C\nC -> O : no\nO -> E"),
                )],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Flowchart", DEFAULT_SPEC).hint("One box per line: A: start Begin (kinds: start, end, process, input, output, decision). Arrows: A -> B, or A -> B : yes to label one"),
        ParamDef::text("blank", "Blank in student copy", "").hint("Box ids to leave empty when Show answers is off, e.g. D, P"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the boxes named above empty, at full size, for students to fill in"),
    ]
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Start,
    End,
    Process,
    Input,
    Output,
    Decision,
}

fn kind_of(w: &str) -> Option<Kind> {
    Some(match w.to_lowercase().as_str() {
        "start" | "begin" => Kind::Start,
        "end" | "stop" => Kind::End,
        "process" | "step" | "do" => Kind::Process,
        "input" | "read" => Kind::Input,
        "output" | "print" => Kind::Output,
        "decision" | "if" | "test" => Kind::Decision,
        _ => return None,
    })
}

struct Step {
    id: String,
    kind: Kind,
    text: String,
}

struct Chart {
    steps: Vec<Step>,
    links: Vec<(usize, usize, String)>,
}

fn parse(spec: &str) -> Result<Chart, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("A flowchart is limited to {MAX_SPEC} characters"));
    }
    let mut steps: Vec<Step> = Vec::new();
    let mut raw_links: Vec<(String, String, String)> = Vec::new();
    for line in spec.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.contains("->") {
            let (path, label) = match line.split_once(':') {
                Some((a, b)) => (a, b.trim().chars().take(12).collect::<String>()),
                None => (line, String::new()),
            };
            let ids: Vec<&str> = path.split("->").map(str::trim).collect();
            for w in ids.windows(2) {
                raw_links.push((w[0].to_string(), w[1].to_string(), label.clone()));
            }
        } else if let Some((id, rest)) = line.split_once(':') {
            let id = id.trim();
            if id.is_empty() || id.chars().count() > MAX_ID || !id.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(format!("\u{201c}{id}\u{201d} is not a usable box id (up to {MAX_ID} letters, digits or _)"));
            }
            if steps.iter().any(|s| s.id == id) {
                return Err(format!("Box {id} is defined twice"));
            }
            let rest = rest.trim();
            let (kw, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            let kind = kind_of(kw).ok_or_else(|| format!("Box {id}: \u{201c}{kw}\u{201d} is not a kind (start, end, process, input, output, decision)"))?;
            if steps.len() >= MAX_STEPS {
                return Err(format!("A flowchart is limited to {MAX_STEPS} boxes"));
            }
            steps.push(Step { id: id.to_string(), kind, text: text.trim().chars().take(MAX_TEXT).collect() });
        } else {
            return Err(format!("\u{201c}{line}\u{201d} is not a box (A: process text) or an arrow (A -> B)"));
        }
    }
    let mut links = Vec::new();
    for (a, b, l) in raw_links {
        let find = |n: &str| steps.iter().position(|s| s.id == n).ok_or_else(|| format!("Arrow {a} -> {b}: there is no box {n}"));
        let (ia, ib) = (find(&a)?, find(&b)?);
        if links.iter().any(|(x, y, _)| (*x, *y) == (ia, ib)) {
            continue;
        }
        if links.len() >= MAX_LINKS {
            return Err(format!("A flowchart is limited to {MAX_LINKS} arrows"));
        }
        links.push((ia, ib, l));
    }
    Ok(Chart { steps, links })
}

fn wrap(text: &str) -> Vec<String> {
    if text.chars().count() <= 16 {
        return vec![text.to_string()];
    }
    let mid = text.chars().count() / 2;
    let split = text.char_indices().filter(|(_, c)| *c == ' ').min_by_key(|(i, _)| (text[..*i].chars().count() as i64 - mid as i64).abs());
    match split {
        Some((i, _)) => vec![text[..i].to_string(), text[i + 1..].to_string()],
        None => vec![text.to_string()],
    }
}

struct Shape {
    c: (f64, f64),
    hw: f64,
    hh: f64,
    kind: Kind,
}

impl Shape {
    /// Where the line from this shape's centre towards `to` leaves it.
    fn exit(&self, to: (f64, f64)) -> (f64, f64) {
        let (dx, dy) = (to.0 - self.c.0, to.1 - self.c.1);
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            return self.c;
        }
        let t = if self.kind == Kind::Decision {
            1.0 / (dx.abs() / self.hw + dy.abs() / self.hh)
        } else {
            (self.hw / dx.abs().max(1e-9)).min(self.hh / dy.abs().max(1e-9))
        };
        (self.c.0 + dx * t, self.c.1 + dy * t)
    }
}

/// Grid positions (x in box widths, y = layer) and which arrows are loops back up. Layers follow
/// the longest forward path from the first box, loops excluded, with end boxes pushed to the
/// bottom; within a layer a box sits under the middle of the boxes that lead to it.
fn layers(ch: &Chart) -> (Vec<(f64, f64)>, Vec<bool>) {
    let n = ch.steps.len();
    // loops: arrows that close a cycle in a depth-first walk from the first box
    let mut state = vec![0u8; n];
    let mut back = vec![false; ch.links.len()];
    let order: Vec<usize> = std::iter::once(0).chain(1..n).collect();
    for root in order {
        if state[root] != 0 {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        state[root] = 1;
        while let Some((u, i)) = stack.pop() {
            let outs: Vec<usize> = (0..ch.links.len()).filter(|k| ch.links[*k].0 == u).collect();
            if i < outs.len() {
                stack.push((u, i + 1));
                let k = outs[i];
                let v = ch.links[k].1;
                match state[v] {
                    0 => {
                        state[v] = 1;
                        stack.push((v, 0));
                    }
                    1 => back[k] = true,
                    _ => {}
                }
            } else {
                state[u] = 2;
            }
        }
    }
    let mut lvl = vec![0usize; n];
    for _ in 0..n {
        for (k, (a, b, _)) in ch.links.iter().enumerate() {
            if !back[k] && lvl[*b] < lvl[*a] + 1 {
                lvl[*b] = lvl[*a] + 1;
            }
        }
    }
    let deepest = (0..n).filter(|i| ch.steps[*i].kind != Kind::End).map(|i| lvl[i]).max().unwrap_or(0);
    for i in 0..n {
        if ch.steps[i].kind == Kind::End {
            lvl[i] = lvl[i].max(deepest + 1);
        }
    }
    let mut x = vec![0.0f64; n];
    let max_l = lvl.iter().copied().max().unwrap_or(0);
    let mut next_free = 0.0;
    for l in 0..=max_l {
        let mut row: Vec<(f64, usize)> = (0..n)
            .filter(|i| lvl[*i] == l)
            .map(|i| {
                let ps: Vec<f64> = ch.links.iter().enumerate().filter(|(k, (a, b, _))| !back[*k] && *b == i && lvl[*a] < l).map(|(_, (a, _, _))| x[*a]).collect();
                let want = if ps.is_empty() { next_free } else { ps.iter().sum::<f64>() / ps.len() as f64 };
                (want, i)
            })
            .collect();
        row.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut placed: Vec<f64> = Vec::new();
        for (want, _) in &row {
            placed.push(match placed.last() {
                Some(prev) => want.max(prev + 1.0),
                None => *want,
            });
        }
        let drift = if row.is_empty() { 0.0 } else { row.iter().zip(&placed).map(|((w, _), p)| p - w).sum::<f64>() / row.len() as f64 };
        for ((_, i), p) in row.iter().zip(&placed) {
            x[*i] = p - drift;
        }
        next_free = x.iter().zip(&lvl).filter(|(_, l2)| **l2 == l).map(|(v, _)| *v).fold(f64::NEG_INFINITY, f64::max) + 1.0;
        if !next_free.is_finite() {
            next_free = 0.0;
        }
    }
    ((0..n).map(|i| (x[i], lvl[i] as f64)).collect(), back)
}

fn render(p: &Params) -> String {
    let ch = match parse(p.s("spec")) {
        Ok(c) => c,
        Err(e) => return chart::message(&e),
    };
    if ch.steps.is_empty() {
        return chart::message("Add some boxes to draw a flowchart");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let title = p.s("title").trim();
    let blank: Vec<String> = p.s("blank").split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).map(String::from).collect();
    let n = ch.steps.len();

    // sizes from the real text, so the student copy keeps the key's layout
    let lines: Vec<Vec<String>> = ch.steps.iter().map(|s| wrap(&s.text)).collect();
    let dims: Vec<(f64, f64)> = ch
        .steps
        .iter()
        .zip(&lines)
        .map(|(s, l)| {
            let tw = l.iter().map(|x| gd::text_w(x, fs)).fold(0.0, f64::max);
            let th = fs * 1.3 * l.len().max(1) as f64;
            match s.kind {
                Kind::Decision => ((tw * 1.5 + 40.0).max(110.0), (th * 1.7 + 24.0).max(70.0)),
                Kind::Input | Kind::Output => ((tw + 56.0).max(110.0), (th + 24.0).max(46.0)),
                _ => ((tw + 36.0).max(100.0), (th + 24.0).max(46.0)),
            }
        })
        .collect();
    let (mw, mh) = (dims.iter().map(|d| d.0).fold(0.0, f64::max), dims.iter().map(|d| d.1).fold(0.0, f64::max));
    let (grid, back) = layers(&ch);
    let cell = (mw + 56.0, mh + 44.0);
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 1.3 };
    let (pts, dw, dh) = gd::to_px(&grid, cell, (mw / 2.0 + 20.0, mh / 2.0 + 10.0));
    // forward arrows whose straight line would cross another box go round the right side too
    let blocked: Vec<bool> = ch
        .links
        .iter()
        .enumerate()
        .map(|(li, (a, b, _))| {
            if back[li] || a == b {
                return false;
            }
            (0..=40).any(|t| {
                let f = t as f64 / 40.0;
                let (x, y) = (pts[*a].0 + (pts[*b].0 - pts[*a].0) * f, pts[*a].1 + (pts[*b].1 - pts[*a].1) * f);
                (0..n).any(|o| o != *a && o != *b && (x - pts[o].0).abs() < dims[o].0 / 2.0 + 4.0 && (y - pts[o].1).abs() < dims[o].1 / 2.0 + 4.0)
            })
        })
        .collect();
    let loops = back.iter().chain(blocked.iter()).any(|b| *b);
    let w = (dw + if loops { 90.0 } else { 0.0 }).max(300.0);
    let h = top + dh + 6.0;
    let ox = (w - dw - if loops { 90.0 } else { 0.0 }) / 2.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let shapes: Vec<Shape> = (0..n).map(|i| Shape { c: (pts[i].0 + ox, pts[i].1 + top), hw: dims[i].0 / 2.0, hh: dims[i].1 / 2.0, kind: ch.steps[i].kind }).collect();
    let right = shapes.iter().map(|s| s.c.0 + s.hw).fold(0.0, f64::max);

    // arrows first, under the boxes
    let mut lanes = 0usize;
    for (li, (a, b, label)) in ch.links.iter().enumerate() {
        let (sa, sb) = (&shapes[*a], &shapes[*b]);
        if back[li] || blocked[li] {
            let lane = right + 28.0 + 16.0 * (lanes % 3) as f64;
            lanes += 1;
            let p0 = (sa.c.0 + sa.hw, sa.c.1);
            let p3 = (sb.c.0 + sb.hw + 1.0, sb.c.1);
            d.polyline(&[p0, (lane, p0.1), (lane, p3.1), (p3.0 + 12.0, p3.1)], INK, 2.0);
            d.arrow(p3.0 + 14.0, p3.1, p3.0, p3.1, INK, 2.0);
            if !label.is_empty() {
                gd::tag(&mut d, ((p0.0 + lane) / 2.0, p0.1 - fs * 0.8), fs * 0.9, label);
            }
        } else {
            let (p0, p1) = (sa.exit(sb.c), sb.exit(sa.c));
            d.arrow(p0.0, p0.1, p1.0, p1.1, INK, 2.0);
            if !label.is_empty() {
                let (lx, ly) = (p0.0 + (p1.0 - p0.0) * 0.28, p0.1 + (p1.1 - p0.1) * 0.28);
                gd::tag(&mut d, (lx + 14.0 * (p1.0 - p0.0).signum().max(0.0) - 10.0, ly - fs * 0.2), fs * 0.9, label);
            }
        }
    }
    let fill = |kind: Kind| match (kind, print) {
        (Kind::Decision, true) => "#e5e5e5",
        (Kind::Decision, false) => "#ffe9a8",
        (Kind::Start | Kind::End, true) => "#d9d9d9",
        (Kind::Start | Kind::End, false) => "#dff3e4",
        (_, true) => "#fff",
        (_, false) => "#eef3fc",
    };
    for (i, s) in shapes.iter().enumerate() {
        let (cx, cy) = s.c;
        let f = fill(s.kind);
        match s.kind {
            Kind::Start | Kind::End => {
                d.raw(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{f}" stroke="{INK}" stroke-width="2"/>"##, cx - s.hw, cy - s.hh, s.hw * 2.0, s.hh * 2.0, s.hh));
            }
            Kind::Process => {
                d.rect(cx - s.hw, cy - s.hh, s.hw * 2.0, s.hh * 2.0, f, INK, 2.0);
            }
            Kind::Input | Kind::Output => {
                let sk = 16.0;
                d.poly(&[(cx - s.hw + sk, cy - s.hh), (cx + s.hw, cy - s.hh), (cx + s.hw - sk, cy + s.hh), (cx - s.hw, cy + s.hh)], f, INK, 2.0);
            }
            Kind::Decision => {
                d.poly(&[(cx, cy - s.hh), (cx + s.hw, cy), (cx, cy + s.hh), (cx - s.hw, cy)], f, INK, 2.0);
            }
        }
        let hidden = !answers && blank.iter().any(|b| *b == ch.steps[i].id);
        if !hidden {
            let l = &lines[i];
            let y0 = cy - (l.len() as f64 - 1.0) * fs * 0.65 + fs * 0.35;
            for (j, t) in l.iter().enumerate() {
                gd::text_fill(&mut d, cx, y0 + j as f64 * fs * 1.3, fs, INK, t);
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
    fn default_draws_every_box_and_label() {
        let svg = render(&defaults());
        for t in [">Start<", ">n &gt; 0?<", ">yes<", ">no<", ">End<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert_eq!(svg.matches("<polygon").count(), 3 + 1 + 6, "three slanted boxes, a diamond and six arrow heads");
    }

    #[test]
    fn spec_errors_are_explained() {
        for (spec, needle) in [
            ("A: start x\nA: end y", "twice"),
            ("A: dance x", "not a kind"),
            ("A: start x\nA -> B", "no box B"),
            ("nonsense", "not a box"),
            ("toolongidentifier: start x", "not a usable"),
            ("", "Add some boxes"),
        ] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
    }

    #[test]
    fn student_copy_blanks_the_named_boxes_at_full_size() {
        let def = def();
        let mut p = defaults();
        p.set_s("blank", "D, P");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(key.contains(">n &gt; 0?<") && !student.contains(">n &gt; 0?<"));
        assert!(student.contains(">Start<") && student.contains(">yes<"), "the other boxes and labels stay");
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn loops_are_routed_round_the_side() {
        let mut p = defaults();
        p.set_s("spec", "A: start s\nB: process x\nC: decision again?\nA -> B -> C\nC -> B : yes");
        let svg = render(&p);
        assert!(svg.contains(">yes<") && svg.contains("<polyline"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn arrows_that_skip_layers_go_round_the_boxes_between() {
        let mut p = defaults();
        p.set_s("spec", "A: start s\nB: process x\nC: process y\nD: end e\nA -> B -> C -> D\nA -> D : skip");
        let svg = render(&p);
        assert!(svg.contains(">skip<") && svg.contains("<polyline"), "the long arrow is routed, not straight");
        let mut q = defaults();
        q.set_s("spec", "A: start s\nB: process x\nC: end e\nA -> B -> C");
        assert!(!render(&q).contains("<polyline"), "short arrows stay straight");
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#ffe9a8") && !svg.contains("#dff3e4") && !svg.contains("#eef3fc"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("flowchart").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                if let Value::S(s) = v {
                    p.set_s(k, s);
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<polygon") && !svg.contains("NaN"), "{}", ex.name);
        }
        let big: String = (0..40).map(|i| format!("n{i}: process step {i}\n")).collect();
        let ring: String = (0..30).map(|i| format!("n{i}: process s\n")).chain((0..30).map(|i| format!("n{i} -> n{}\n", (i + 1) % 30))).collect();
        for spec in ["", "\u{0}", "A: start", "A: start x\nA -> A", "A: process a very very long piece of text that goes on and on and on past the limit\nA -> A", &big, &ring, "A: decision x\nB: end y\nA -> B : a long label that is cut\nB -> A"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("spec", spec);
                p.set_s("blank", "A, n3");
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?}");
            }
        }
    }
}
