//! Logic circuit: gates and wires drawn from a text spec (or from a boolean expression), with an
//! optional truth table, the expression, and an input row that lights up the wires. The student
//! copy (Show answers off) keeps the diagram and the table's inputs and hides what a student
//! works out: the output column, the values on the wires and the expression.
use crate::figures::chart::{self, INK};
use crate::figures::logic::{Circuit, Gate, Kind, MAX_TABLE_INPUTS};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const HALF_ADDER: &str = "in A, B\ng1 = XOR(A, B)\ng2 = AND(A, B)\nout S = g1\nout C = g2";
const FULL_ADDER: &str = "in A, B, Cin\nx1 = XOR(A, B)\na1 = AND(A, B)\na2 = AND(x1, Cin)\ns = XOR(x1, Cin)\nc = OR(a1, a2)\nout Sum = s\nout Cout = c";
const MUX: &str = "in A, B, S\nn = NOT(S)\na1 = AND(A, n)\na2 = AND(B, S)\ny = OR(a1, a2)\nout Y = y";
const XOR_NAND: &str = "in A, B\nn1 = NAND(A, B)\nn2 = NAND(A, n1)\nn3 = NAND(B, n1)\nn4 = NAND(n2, n3)\nout Q = n4";

pub fn def() -> FigureDef {
    FigureDef {
        id: "logic-circuit",
        subject: Subject::Computing,
        title: "Logic Circuit",
        blurb: "A logic circuit of AND, OR, NOT, NAND, NOR and XOR gates from a short text spec or a boolean expression, with a truth table, the expression and an input row that lights up the wires.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("logic-circuit", "spec") => p.s("source") != "Boolean expression",
        ("logic-circuit", "expr") => p.s("source") == "Boolean expression",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "logic-circuit" => vec![
            Example { name: "Half adder", params: vec![] },
            Example { name: "Full adder", params: vec![("spec", s(FULL_ADDER))] },
            Example { name: "2-to-1 multiplexer", params: vec![("spec", s(MUX)), ("try", s("101"))] },
            Example { name: "XOR from NAND gates", params: vec![("spec", s(XOR_NAND)), ("symbols", s("Boxes (IEC)"))] },
            Example {
                name: "From an expression",
                params: vec![("source", s("Boolean expression")), ("expr", s("Q = (A OR B) AND NOT C")), ("title", s("Q = (A OR B) AND NOT C"))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("source", "Build the circuit from", &["Gates and wires", "Boolean expression"], "Gates and wires")
            .hint("A boolean expression is turned into gates for you"),
        ParamDef::long("spec", "Circuit", HALF_ADDER)
            .hint("in A, B lists the inputs; g1 = AND(A, B) is a gate (AND OR NOT NAND NOR XOR XNOR BUF); out Q = g1 is an output. Add @x,y to place a part"),
        ParamDef::long("expr", "Expression", "Q = (A OR B) AND NOT C")
            .hint("One per line, such as S = A XOR B. Words AND OR XOR NOT, or the symbols & | ^ !"),
        ParamDef::choice("symbols", "Gate symbols", &["Curved (ANSI)", "Boxes (IEC)"], "Curved (ANSI)"),
        ParamDef::choice("table", "Truth table", &["None", "Truth table"], "Truth table"),
        ParamDef::bool("show_expr", "Show the expression", true),
        ParamDef::text("try", "Light up the inputs", "").hint("Type 0s and 1s, one per input in order (such as 101), to show the values on the wires. Empty shows none"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the output column, the expression and the values inside the circuit, for students to work out"),
    ]
}

fn build(p: &Params) -> Result<Circuit, String> {
    if p.s("source") == "Boolean expression" {
        Circuit::from_exprs(p.s("expr"))
    } else {
        Circuit::parse(p.s("spec"))
    }
}

/// Words of `msg` set in lines of about `width` characters.
fn wrap(msg: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for w in msg.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + w.chars().count() <= width => {
                l.push(' ');
                l.push_str(w);
            }
            _ => lines.push(w.to_string()),
        }
    }
    lines.truncate(6);
    lines
}

fn message(msg: &str) -> String {
    let lines = wrap(msg, 56);
    let h = 50.0 + 22.0 * lines.len() as f64;
    let mut d = Doc::new(520.0, h);
    for (i, l) in lines.iter().enumerate() {
        chart::text(&mut d, 260.0, 42.0 + 22.0 * i as f64, 15.0, "middle", l);
    }
    d.finish()
}

/// Size of a gate body (without the output bubble) and where its pins sit.
pub(crate) struct Dim {
    pub w: f64,
    pub h: f64,
    pub bubble: bool,
    /// How far the input pins stand left of the body (the extra curve of an XOR).
    pub back: f64,
}

pub(crate) const BUB: f64 = 5.0;
/// World pixels per grid unit across and down: the spacing the figure and the canvas both use.
pub(crate) const COL_W: f64 = 136.0;
pub(crate) const ROW_H: f64 = 76.0;

pub(crate) fn dim(g: Gate, n: usize, iec: bool) -> Dim {
    let one = matches!(g, Gate::Not | Gate::Buf);
    let h = if one { 36.0 } else { 40.0 + n.saturating_sub(2) as f64 * 14.0 };
    let w = match (one, iec) {
        (true, true) => 36.0,
        (true, false) => 38.0,
        (false, true) => 48.0,
        (false, false) => 54.0,
    };
    let bubble = matches!(g, Gate::Not | Gate::Nand | Gate::Nor | Gate::Xnor);
    let back = if !iec && matches!(g, Gate::Xor | Gate::Xnor) { 7.0 } else { 0.0 };
    Dim { w, h, bubble, back }
}

impl Dim {
    pub fn total(&self) -> f64 { self.w + self.back + if self.bubble { 2.0 * BUB } else { 0.0 } }
}

/// The input pin `k` of `n` and the output pin of a gate whose whole symbol is centred at (x, y).
pub(crate) fn pins(dm: &Dim, n: usize, x: f64, y: f64) -> (Vec<(f64, f64)>, (f64, f64)) {
    let left = x - dm.total() / 2.0;
    let ins = (0..n).map(|k| (left, y - dm.h / 2.0 + dm.h * (k as f64 + 0.5) / n as f64)).collect();
    (ins, (x + dm.total() / 2.0, y))
}

fn symbol(d: &mut Doc, g: Gate, dm: &Dim, n: usize, iec: bool, x: f64, y: f64, fs: f64) {
    let (w, h) = (dm.w, dm.h);
    // body centre: the whole symbol is centred on x, the back curve and bubble sit either side
    let bx = x + (dm.back - if dm.bubble { 2.0 * BUB } else { 0.0 }) / 2.0;
    let (l, r, t, b) = (bx - w / 2.0, bx + w / 2.0, y - h / 2.0, y + h / 2.0);
    let sw = 2.0;
    if iec {
        d.rect(l, t, w, h, "#fff", INK, sw);
        let label = match g {
            Gate::And | Gate::Nand => "&",
            Gate::Or | Gate::Nor => "\u{2265}1",
            Gate::Xor | Gate::Xnor => "=1",
            Gate::Not | Gate::Buf => "1",
        };
        chart::text(d, bx, y + fs * 0.35, fs * 1.05, "middle", label);
    } else {
        match g {
            Gate::And | Gate::Nand => {
                d.path(&format!("M {l:.2} {t:.2} H {bx:.2} A {:.2} {:.2} 0 0 1 {bx:.2} {b:.2} H {l:.2} Z", w / 2.0, h / 2.0), "#fff", INK, sw);
            }
            Gate::Or | Gate::Nor | Gate::Xor | Gate::Xnor => {
                d.path(&format!("M {l:.2} {t:.2} Q {bx:.2} {t:.2} {r:.2} {y:.2} Q {bx:.2} {b:.2} {l:.2} {b:.2} Q {:.2} {y:.2} {l:.2} {t:.2} Z", l + 14.0), "#fff", INK, sw);
                if dm.back > 0.0 {
                    let o = dm.back;
                    d.path(&format!("M {:.2} {t:.2} Q {:.2} {y:.2} {:.2} {b:.2}", l - o, l - o + 14.0, l - o), "none", INK, sw);
                }
            }
            Gate::Not | Gate::Buf => {
                d.poly(&[(l, t), (r, y), (l, b)], "#fff", INK, sw);
            }
        }
        // input leads reach the curved back of an OR-type gate
        if matches!(g, Gate::Or | Gate::Nor | Gate::Xor | Gate::Xnor) {
            let (ins, _) = pins(dm, n, x, y);
            for (px, py) in ins {
                d.line(px, py, l + 6.0, py, INK, sw);
            }
        }
    }
    if dm.bubble {
        d.circle(r + BUB, y, BUB, "#fff", INK, sw);
    }
}

fn render(p: &Params) -> String {
    let circ = match build(p) {
        Ok(c) => c,
        Err(e) => return message(&e),
    };
    if circ.nodes.is_empty() {
        return message("Add some inputs and gates to draw a circuit");
    }
    let iec = p.s("symbols").starts_with("Boxes");
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let title = p.s("title").trim();

    // diagram geometry
    let (cw, rh) = (COL_W, ROW_H);
    let (npos, opos, routes) = circ.layout();
    let all: Vec<(f64, f64)> = npos.iter().chain(opos.iter()).copied().collect();
    let min_x = all.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
    let max_x = all.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = all.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
    let max_y = all.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
    let name_w = |s: &str| s.chars().count() as f64 * fs * 0.62;
    let left_pad = 22.0 + circ.nodes.iter().filter(|n| matches!(n.kind, Kind::Input)).map(|n| name_w(&n.name)).fold(0.0, f64::max);
    let right_pad = 22.0 + circ.outputs.iter().map(|o| name_w(&o.name)).fold(0.0, f64::max);
    let gate_pad = 34.0;
    let top_pad = if title.is_empty() { 26.0 } else { 38.0 + fs * 1.4 };
    let dw = left_pad + right_pad + (max_x - min_x) * cw + 2.0 * gate_pad;
    let dh = (max_y - min_y) * rh + 70.0;
    let at = |g: (f64, f64)| (left_pad + gate_pad + (g.0 - min_x) * cw, top_pad + 35.0 + (g.1 - min_y) * rh);

    // the lit input row
    let n_in = circ.inputs().len();
    let try_text: Vec<bool> = p.s("try").trim().chars().filter(|c| matches!(c, '0' | '1')).map(|c| c == '1').collect();
    let lit = !try_text.is_empty() && n_in > 0;
    let bits: Vec<bool> = (0..n_in).map(|i| try_text.get(i).copied().unwrap_or(false)).collect();
    let (vals, out_vals) = if lit { circ.eval(&bits) } else { (vec![false; circ.nodes.len()], vec![false; circ.outputs.len()]) };

    // table geometry
    let want_table = p.s("table") == "Truth table";
    let table = if want_table { circ.truth_table() } else { Ok(Vec::new()) };
    let n_out = circ.outputs.len();
    let cols = n_in + n_out;
    let colw = circ.inputs().iter().map(|i| name_w(&circ.nodes[*i].name)).chain(circ.outputs.iter().map(|o| name_w(&o.name))).fold(0.0, f64::max).max(fs * 1.0) + 22.0;
    let trh = fs * 1.9;
    let show_table = want_table && cols > 0;
    let table_w = cols as f64 * colw;
    let (table_h, note) = match (&table, show_table) {
        (Ok(rows), true) => ((rows.len() + 1) as f64 * trh, false),
        (Err(_), true) => (fs * 2.0, true),
        _ => (0.0, false),
    };
    let exprs: Vec<String> = if p.b("show_expr") { (0..n_out).map(|i| format!("{} = {}", circ.outputs[i].name, circ.expr(i))).collect() } else { Vec::new() };
    let expr_h = if exprs.is_empty() { 0.0 } else { 14.0 + exprs.len() as f64 * fs * 1.7 };

    let w = (dw.max(table_w + 40.0)).max(320.0);
    let ox = (w - dw) / 2.0;
    let h = top_pad + dh + if show_table { 18.0 + table_h } else { 0.0 } + expr_h + 20.0;

    let mut d = Doc::new(w, h);
    if title.is_empty() {
        // nothing
    } else {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    let (lit_on, lit_off) = if print { ("#000", "#000") } else { ("#1a8a3a", "#6b7280") };
    let wire_style = |v: Option<bool>| -> (&str, f64, &str) {
        match v {
            None => (INK, 2.0, ""),
            Some(true) => (lit_on, if print { 3.4 } else { 2.8 }, ""),
            Some(false) => (lit_off, if print { 1.4 } else { 2.0 }, if print { "5 4" } else { "" }),
        }
    };

    // where each node's output leaves and each gate input arrives
    let geom: Vec<(Option<Dim>, (f64, f64))> = (0..circ.nodes.len())
        .map(|i| {
            let (x, y) = at(npos[i]);
            let (x, y) = (x + ox, y);
            match &circ.nodes[i].kind {
                Kind::Gate(g, a) => (Some(dim(*g, a.len(), iec)), (x, y)),
                Kind::Input => (None, (x, y)),
            }
        })
        .collect();
    let out_pin = |i: usize| -> (f64, f64) {
        match &geom[i].0 {
            Some(dm) => {
                let n = match &circ.nodes[i].kind { Kind::Gate(_, a) => a.len(), _ => 1 };
                pins(dm, n, geom[i].1 .0, geom[i].1 .1).1
            }
            None => geom[i].1,
        }
    };
    // wires: from the source pin to the reading pin, the source node, and which pin reads it
    struct Wire {
        a: (f64, f64),
        b: (f64, f64),
        src: usize,
        key: (usize, usize),
        auto: bool,
    }
    let n_nodes = circ.nodes.len();
    let mut wires: Vec<Wire> = Vec::new();
    for (i, n) in circ.nodes.iter().enumerate() {
        if let (Kind::Gate(_, a), Some(dm)) = (&n.kind, &geom[i].0) {
            let (ins, _) = pins(dm, a.len(), geom[i].1 .0, geom[i].1 .1);
            for (k, src) in a.iter().enumerate() {
                let auto = n.pos.is_none() && circ.nodes[*src].pos.is_none();
                wires.push(Wire { a: out_pin(*src), b: ins[k], src: *src, key: (i, k), auto });
            }
        }
    }
    let out_pts: Vec<(f64, f64)> = opos.iter().map(|q| { let (x, y) = at(*q); (x + ox, y) }).collect();
    for (k, o) in circ.outputs.iter().enumerate() {
        let auto = o.pos.is_none() && circ.nodes[o.src].pos.is_none();
        wires.push(Wire { a: out_pin(o.src), b: out_pts[k], src: o.src, key: (n_nodes + k, 0), auto });
    }
    let fanout = |src: usize| wires.iter().filter(|w| w.src == src).count();
    // a student sees the input values but not what the gates make of them
    let known = |src: usize| lit && (answers || matches!(circ.nodes[src].kind, Kind::Input));
    let col_x = |level: f64| ox + left_pad + gate_pad + (level - min_x) * cw;
    for pass in [false, true] {
        for wr in &wires {
            let (a, b, src) = (wr.a, wr.b, wr.src);
            let v = known(src).then_some(vals[src]);
            if v.unwrap_or(false) != pass {
                continue;
            }
            let (col, sw, dash) = wire_style(v);
            // each source gets its own lane in the gaps between the columns, so wires from different
            // sources never run down the same vertical
            let lane = ((src % 5) as f64 - 2.0) * 6.0;
            let mut path = format!("M {:.2} {:.2}", a.0, a.1);
            let mut junction = None;
            if (a.1 - b.1).abs() < 0.01 && !routes.contains_key(&wr.key) {
                path += &format!(" H {:.2}", b.0);
            } else if wr.auto {
                // the wire steps through the gap after each column it crosses, on the row of each stop
                let src_level = npos[src].0;
                let stops: Vec<f64> = routes.get(&wr.key).map(|r| r.iter().map(|q| top_pad + 35.0 + (q.1 - min_y) * rh).collect()).unwrap_or_default();
                for (h, ty) in stops.iter().copied().chain(std::iter::once(b.1)).enumerate() {
                    let gx = col_x(src_level + h as f64) + cw / 2.0 + lane;
                    let gx = gx.min(b.0 - 8.0).max(a.0 + 8.0);
                    if junction.is_none() && (a.1 - ty).abs() >= 0.01 {
                        junction = Some(gx);
                    }
                    path += &format!(" H {gx:.2} V {ty:.2}");
                }
                path += &format!(" H {:.2}", b.0);
            } else {
                let mx = (b.0 - (cw - 70.0).max(40.0) / 2.0).max(a.0 + 10.0).min(b.0 - 8.0);
                path += &format!(" H {mx:.2} V {:.2} H {:.2}", b.1, b.0);
                junction = Some(mx);
            }
            if dash.is_empty() {
                d.path(&path, "none", col, sw);
            } else {
                d.raw(&format!(r##"<path d="{path}" fill="none" stroke="{col}" stroke-width="{sw}" stroke-dasharray="{dash}"/>"##));
            }
            if let (Some(jx), true) = (junction, fanout(src) > 1) {
                d.circle(jx, a.1, 3.2, col, col, 1.0);
            }
        }
    }
    // input labels and values
    for (i, n) in circ.nodes.iter().enumerate() {
        if matches!(n.kind, Kind::Input) {
            let (x, y) = geom[i].1;
            chart::text(&mut d, x - 10.0, y + fs * 0.35, fs * 1.1, "end", &n.name);
            d.circle(x, y, 3.2, "#fff", INK, 1.6);
            if lit {
                chart::text(&mut d, x + 6.0, y - 7.0, fs * 0.85, "start", if vals[i] { "1" } else { "0" });
            }
        }
    }
    for (i, n) in circ.nodes.iter().enumerate() {
        if let (Kind::Gate(g, a), Some(dm)) = (&n.kind, &geom[i].0) {
            symbol(&mut d, *g, dm, a.len(), iec, geom[i].1 .0, geom[i].1 .1, fs);
        }
    }
    for (k, o) in circ.outputs.iter().enumerate() {
        let (x, y) = out_pts[k];
        d.circle(x, y, 3.2, "#fff", INK, 1.6);
        chart::text(&mut d, x + 10.0, y + fs * 0.35, fs * 1.1, "start", &o.name);
        if lit && answers {
            chart::text(&mut d, x - 6.0, y - 8.0, fs * 0.85, "end", if out_vals[k] { "1" } else { "0" });
        }
    }

    // truth table
    let mut y = top_pad + dh + 18.0;
    if show_table {
        match &table {
            Ok(rows) => {
                let tx = (w - table_w) / 2.0;
                let names: Vec<&str> = circ.inputs().iter().map(|i| circ.nodes[*i].name.as_str()).chain(circ.outputs.iter().map(|o| o.name.as_str())).collect();
                let mut y0 = y;
                for (c, name) in names.iter().enumerate() {
                    d.rect(tx + c as f64 * colw, y0, colw, trh, if print { "#e5e5e5" } else { "#f0e9db" }, INK, 1.4);
                    chart::text(&mut d, tx + (c as f64 + 0.5) * colw, y0 + trh * 0.5 + fs * 0.35, fs, "middle", name);
                }
                y0 += trh;
                for (ins, outs) in rows {
                    let is_lit = lit && *ins == bits;
                    for c in 0..cols {
                        let fill = if is_lit { if print { "#d9d9d9" } else { "#fff3bf" } } else { "#fff" };
                        d.rect(tx + c as f64 * colw, y0, colw, trh, fill, INK, 1.2);
                        let text = if c < n_in {
                            Some(ins[c])
                        } else if answers {
                            Some(outs[c - n_in])
                        } else {
                            None
                        };
                        if let Some(v) = text {
                            chart::text(&mut d, tx + (c as f64 + 0.5) * colw, y0 + trh * 0.5 + fs * 0.35, fs, "middle", if v { "1" } else { "0" });
                        }
                    }
                    y0 += trh;
                }
                // a heavier rule between the inputs and the outputs
                if n_in > 0 && n_out > 0 {
                    let xx = tx + n_in as f64 * colw;
                    d.line(xx, y, xx, y0, INK, 2.6);
                }
                y = y0;
            }
            Err(_) if note => {
                chart::text(&mut d, w / 2.0, y + fs * 1.1, fs, "middle", &format!("The truth table is limited to {MAX_TABLE_INPUTS} inputs"));
                y += table_h;
            }
            Err(_) => {}
        }
    }
    if !exprs.is_empty() {
        y += 14.0;
        for e in &exprs {
            y += fs * 1.7;
            // the student copy keeps the room, for the student to write the expression in
            if answers {
                chart::text(&mut d, w / 2.0, y - fs * 0.4, fs * 1.05, "middle", e);
            }
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
    fn default_draws_a_half_adder_with_its_table_and_expressions() {
        let svg = render(&defaults());
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert!(svg.contains(">S = A XOR B<") && svg.contains(">C = A AND B<"));
        assert_eq!(svg.matches("<path").count() >= 4, true);
        // table: header + 4 rows of 4 cells
        assert_eq!(svg.matches("<rect").count(), 1 + 4 * 5);
    }

    #[test]
    fn student_copy_hides_what_is_worked_out() {
        let def = def();
        let mut p = defaults();
        p.set_s("try", "11");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(!student.contains("S = A XOR B") && !student.contains("C = A AND B"));
        assert!(key.contains("S = A XOR B"));
        assert_eq!(size(&student), size(&key), "same size, so the copies can be swapped");
        let digits = |s: &str| s.matches(">0<").count() + s.matches(">1<").count();
        // key: 8 input cells + 8 output cells + 2 input values + 2 output values; student: inputs only
        assert_eq!(digits(&key), 8 + 8 + 2 + 2);
        assert_eq!(digits(&student), 8 + 2);
        assert!(key.contains("#1a8a3a"), "lit wires are green in colour");
        assert!(!student.contains("#6b7280"), "a student's gate wires are not lit");
    }

    #[test]
    fn lit_row_is_highlighted_and_follows_the_inputs() {
        let mut p = defaults();
        p.set_s("try", "10");
        let svg = render(&p);
        assert_eq!(svg.matches("#fff3bf").count(), 4, "one row of four cells");
        p.set_s("try", "");
        assert!(!render(&p).contains("#fff3bf"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("try", "10");
        let svg = render(&p);
        assert!(!svg.contains("#1a8a3a") && !svg.contains("#fff3bf") && svg.contains("stroke-dasharray"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn both_symbol_styles_draw_every_gate() {
        let spec = "in A, B\ng1 = AND(A, B)\ng2 = OR(A, B)\ng3 = NOT(A)\ng4 = NAND(A, B)\ng5 = NOR(A, B)\ng6 = XOR(A, B)\ng7 = XNOR(A, B)\ng8 = BUF(A)\ng9 = AND(A, B, g1, g2)\nout Q = g9";
        for style in ["Curved (ANSI)", "Boxes (IEC)"] {
            let mut p = defaults();
            p.set_s("spec", spec);
            p.set_s("symbols", style);
            let svg = render(&p);
            assert!(!svg.contains("NaN") && svg.contains("<path") || svg.contains("<rect"), "{style}");
            assert_eq!(svg.matches("<circle").count() >= 4 + 4 + 1 + 2, true, "bubbles and terminals: {style}");
        }
        let mut p = defaults();
        p.set_s("spec", spec);
        p.set_s("symbols", "Boxes (IEC)");
        let svg = render(&p);
        assert!(svg.contains(">&amp;<") && svg.contains(">\u{2265}1<") && svg.contains(">=1<"));
    }

    #[test]
    fn expression_source_builds_the_same_circuit() {
        let mut p = defaults();
        p.set_s("source", "Boolean expression");
        p.set_s("expr", "Q = (A OR B) AND NOT C");
        let svg = render(&p);
        assert!(svg.contains(">Q = (A OR B) AND NOT C<"), "{svg}");
        assert!(relevant("logic-circuit", "expr", &p) && !relevant("logic-circuit", "spec", &p));
        assert!(!relevant("logic-circuit", "expr", &defaults()));
    }

    #[test]
    fn a_bad_spec_is_explained_in_the_figure() {
        for spec in ["in A\ng = AND(A)", "in A\ng1 = AND(A, g2)\ng2 = NOT(g1)", "nonsense", "in A\nout Q = nope"] {
            let mut p = defaults();
            p.set_s("spec", spec);
            let svg = render(&p);
            assert!(svg.starts_with("<svg") && !svg.contains("<rect x") && svg.contains("<text"), "{spec}");
        }
        let mut p = defaults();
        p.set_s("spec", "");
        assert!(render(&p).contains("Add some inputs"));
    }

    #[test]
    fn six_inputs_draw_without_a_table() {
        let mut p = defaults();
        p.set_s("spec", "in A, B, C, D, E, F\ng = AND(A, B, C, D)\nh = OR(g, E, F)\nout Q = h");
        let svg = render(&p);
        assert!(svg.contains("limited to 5 inputs") && svg.contains("<path"));
    }

    #[test]
    fn examples_all_render_and_are_correct() {
        for ex in examples("logic-circuit").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                if let Value::S(s) = v {
                    p.set_s(k, s);
                }
            }
            let svg = render(&p);
            assert!(svg.starts_with("<svg") && !svg.contains("NaN"), "{}", ex.name);
            assert!(svg.contains("<path"), "{} draws gates (not an error message)", ex.name);
        }
        // the adders really add
        let c = Circuit::parse(FULL_ADDER).unwrap();
        for (ins, outs) in c.truth_table().unwrap() {
            let sum = ins.iter().filter(|b| **b).count();
            assert_eq!((outs[0], outs[1]), (sum % 2 == 1, sum >= 2), "{ins:?}");
        }
        let m = Circuit::parse(MUX).unwrap();
        for (ins, outs) in m.truth_table().unwrap() {
            assert_eq!(outs[0], if ins[2] { ins[1] } else { ins[0] }, "{ins:?}");
        }
        let x = Circuit::parse(XOR_NAND).unwrap();
        for (ins, outs) in x.truth_table().unwrap() {
            assert_eq!(outs[0], ins[0] != ins[1]);
        }
    }

    #[test]
    fn hostile_input_never_panics_or_prints_nan() {
        let huge = format!("in A\n{}", (0..80).map(|i| format!("g{i} = NOT(A)\n")).collect::<String>());
        for spec in ["", "\u{0}", "in A @ 1,1", "in A\ng = NOT(A) @ 999,-999\nout Q = g @ 1000,1000", "in A\nout Q = A", "out Q = Q", &huge, "in A, B\ng = AND(A, B) @ 0,0\nh = OR(A, B) @ 0,0\nout Q = g\nout R = h"] {
            for answers in [true, false] {
                for table in ["None", "Truth table"] {
                    for try_text in ["", "1", "abc", "1111111"] {
                        let mut p = defaults();
                        p.set_s("spec", spec);
                        p.set_b(SHOW_ANSWERS, answers);
                        p.set_s("table", table);
                        p.set_s("try", try_text);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{spec:?}");
                        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{spec:?}");
                    }
                }
            }
        }
        let mut p = defaults();
        p.set_s("source", "Boolean expression");
        for e in ["", "(((", "A AND", &"A OR ".repeat(500), "\u{1f600}"] {
            p.set_s("expr", e);
            assert!(render(&p).starts_with("<svg"));
        }
    }
}
