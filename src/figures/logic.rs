//! The model behind the logic-circuit figure: a combinational circuit of named inputs, gates and
//! outputs, the text spec it is written in, evaluation, truth tables, the boolean expression a
//! diagram stands for, and the reverse (an expression to a diagram). Pure code, no drawing, so the
//! typed spec, the canvas editor and the figure agent all share one reading of a circuit.
//!
//! Spec, one item per line (`#` starts a comment, `@x,y` optionally places an item in grid units):
//! ```text
//! in A, B
//! g1 = XOR(A, B)
//! g2 = AND(A, B) @ 2,1
//! out S = g1
//! out C = g2
//! ```
use std::collections::HashMap;

pub const MAX_NODES: usize = 40;
pub const MAX_SPEC: usize = 4000;
pub const MAX_NAME: usize = 12;
/// A truth table past this many inputs is not a printable figure (2^5 = 32 rows).
pub const MAX_TABLE_INPUTS: usize = 5;

/// Grid points a level-skipping wire passes through; see [`Circuit::layout`].
pub type Routes = HashMap<(usize, usize), Vec<(f64, f64)>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gate {
    And,
    Or,
    Not,
    Nand,
    Nor,
    Xor,
    Xnor,
    Buf,
}

impl Gate {
    pub const ALL: [Gate; 8] = [Gate::And, Gate::Or, Gate::Not, Gate::Nand, Gate::Nor, Gate::Xor, Gate::Xnor, Gate::Buf];

    pub fn name(self) -> &'static str {
        match self {
            Gate::And => "AND",
            Gate::Or => "OR",
            Gate::Not => "NOT",
            Gate::Nand => "NAND",
            Gate::Nor => "NOR",
            Gate::Xor => "XOR",
            Gate::Xnor => "XNOR",
            Gate::Buf => "BUF",
        }
    }

    pub fn from_name(s: &str) -> Option<Gate> {
        Gate::ALL.into_iter().find(|g| g.name().eq_ignore_ascii_case(s))
    }

    /// Allowed number of inputs, inclusive.
    pub fn arity(self) -> (usize, usize) {
        match self {
            Gate::Not | Gate::Buf => (1, 1),
            _ => (2, 4),
        }
    }

    pub fn apply(self, v: &[bool]) -> bool {
        let all = v.iter().all(|b| *b);
        let any = v.iter().any(|b| *b);
        let odd = v.iter().filter(|b| **b).count() % 2 == 1;
        match self {
            Gate::And => all,
            Gate::Or => any,
            Gate::Not => !v.first().copied().unwrap_or(false),
            Gate::Nand => !all,
            Gate::Nor => !any,
            Gate::Xor => odd,
            Gate::Xnor => !odd,
            Gate::Buf => v.first().copied().unwrap_or(false),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Input,
    Gate(Gate, Vec<usize>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub kind: Kind,
    /// Where the spec placed it (grid units); None means the automatic layout.
    pub pos: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub name: String,
    pub src: usize,
    pub pos: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Circuit {
    pub nodes: Vec<Node>,
    pub outputs: Vec<Output>,
    /// Gate evaluation order (every gate after the nodes it reads).
    order: Vec<usize>,
}

fn valid_name(s: &str) -> Result<(), String> {
    let mut ch = s.chars();
    let ok = ch.next().is_some_and(|c| c.is_alphabetic() || c == '_') && ch.all(|c| c.is_alphanumeric() || c == '_');
    if !ok {
        return Err(format!("\u{201c}{s}\u{201d} is not a usable name (letters, digits and _, starting with a letter)"));
    }
    if s.chars().count() > MAX_NAME {
        return Err(format!("\u{201c}{s}\u{201d} is longer than {MAX_NAME} characters"));
    }
    if Gate::from_name(s).is_some() || matches!(s.to_ascii_uppercase().as_str(), "IN" | "OUT" | "INPUTS") {
        return Err(format!("\u{201c}{s}\u{201d} is a reserved word"));
    }
    Ok(())
}

fn parse_pos(s: &str) -> Result<(f64, f64), String> {
    let (x, y) = s.split_once(',').ok_or_else(|| format!("a position is @x,y, not \u{201c}@{s}\u{201d}"))?;
    let n = |t: &str| t.trim().parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1000.0);
    match (n(x), n(y)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(format!("a position is @x,y with numbers, not \u{201c}@{s}\u{201d}")),
    }
}

/// A line split from its optional trailing `@x,y`.
fn split_pos(line: &str) -> Result<(&str, Option<(f64, f64)>), String> {
    match line.rsplit_once('@') {
        Some((a, p)) => Ok((a.trim(), Some(parse_pos(p)?))),
        None => Ok((line.trim(), None)),
    }
}

impl Circuit {
    pub fn inputs(&self) -> Vec<usize> {
        (0..self.nodes.len()).filter(|i| matches!(self.nodes[*i].kind, Kind::Input)).collect()
    }

    fn name_taken(&self, n: &str) -> bool {
        self.nodes.iter().any(|x| x.name == n) || self.outputs.iter().any(|x| x.name == n)
    }

    /// Reads the spec. Errors say which line and why; a circuit with a loop is refused.
    pub fn parse(spec: &str) -> Result<Circuit, String> {
        if spec.len() > MAX_SPEC {
            return Err(format!("The spec is longer than {MAX_SPEC} characters"));
        }
        // (name, gate, arg names, pos, line) collected first so gates may be listed in any order
        type Pending = (String, Gate, Vec<String>, Option<(f64, f64)>, usize);
        let mut c = Circuit::default();
        let mut gates: Vec<Pending> = Vec::new();
        let mut outs: Vec<(String, String, Option<(f64, f64)>, usize)> = Vec::new();
        for (ln, raw) in spec.lines().enumerate() {
            let ln = ln + 1;
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let at = |m: String| format!("Line {ln}: {m}");
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = ["inputs ", "input ", "in "].iter().find_map(|p| lower.starts_with(p).then(|| &line[p.len()..])) {
                let (rest, pos) = split_pos(rest).map_err(&at)?;
                let names: Vec<&str> = rest.split(|ch: char| ch == ',' || ch.is_whitespace()).filter(|s| !s.is_empty()).collect();
                if pos.is_some() && names.len() != 1 {
                    return Err(at("only a single input can be placed: in A @ 0,1".into()));
                }
                for n in names {
                    valid_name(n).map_err(&at)?;
                    if c.name_taken(n) || gates.iter().any(|g| g.0 == n) || outs.iter().any(|o| o.0 == n) {
                        return Err(at(format!("\u{201c}{n}\u{201d} is already used")));
                    }
                    c.nodes.push(Node { name: n.into(), kind: Kind::Input, pos });
                }
            } else if lower.starts_with("out ") || lower.starts_with("output ") {
                let rest = line.split_once(char::is_whitespace).map_or("", |x| x.1);
                let (body, pos) = split_pos(rest).map_err(&at)?;
                let (name, src) = body
                    .split_once("<-")
                    .or_else(|| body.split_once('='))
                    .ok_or_else(|| at("an output is \u{201c}out Q = g1\u{201d}".into()))?;
                let (name, src) = (name.trim(), src.trim());
                valid_name(name).map_err(&at)?;
                if c.name_taken(name) || gates.iter().any(|g| g.0 == name) || outs.iter().any(|o| o.0 == name) {
                    return Err(at(format!("\u{201c}{name}\u{201d} is already used")));
                }
                outs.push((name.into(), src.into(), pos, ln));
            } else {
                let (body, pos) = split_pos(line).map_err(&at)?;
                let (name, call) = body.split_once('=').ok_or_else(|| at("expected \u{201c}in A, B\u{201d}, \u{201c}g1 = AND(A, B)\u{201d} or \u{201c}out Q = g1\u{201d}".into()))?;
                let name = name.trim();
                valid_name(name).map_err(&at)?;
                if c.name_taken(name) || gates.iter().any(|g| g.0 == name) || outs.iter().any(|o| o.0 == name) {
                    return Err(at(format!("\u{201c}{name}\u{201d} is already used")));
                }
                let call = call.trim();
                let (gname, args) = call.split_once('(').ok_or_else(|| at("a gate is written GATE(A, B)".into()))?;
                let args = args.trim().strip_suffix(')').ok_or_else(|| at("the gate is missing its closing )".into()))?;
                let gate = Gate::from_name(gname.trim()).ok_or_else(|| at(format!("\u{201c}{}\u{201d} is not a gate (AND OR NOT NAND NOR XOR XNOR BUF)", gname.trim())))?;
                let args: Vec<String> = args.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                let (lo, hi) = gate.arity();
                if args.len() < lo || args.len() > hi {
                    let want = if lo == hi { format!("{lo} input") } else { format!("{lo} to {hi} inputs") };
                    return Err(at(format!("{} takes {want}, not {}", gate.name(), args.len())));
                }
                gates.push((name.into(), gate, args, pos, ln));
            }
            if c.nodes.len() + gates.len() > MAX_NODES {
                return Err(at(format!("More than {MAX_NODES} inputs and gates")));
            }
        }
        // give every gate a node, then resolve what each one reads
        let mut index: HashMap<String, usize> = c.nodes.iter().enumerate().map(|(i, n)| (n.name.clone(), i)).collect();
        for g in &gates {
            index.insert(g.0.clone(), c.nodes.len());
            c.nodes.push(Node { name: g.0.clone(), kind: Kind::Input, pos: g.3 });
        }
        for g in &gates {
            let mut ids = Vec::new();
            for a in &g.2 {
                ids.push(*index.get(a).ok_or_else(|| format!("Line {}: {} reads \u{201c}{a}\u{201d}, which is not defined", g.4, g.0))?);
            }
            c.nodes[index[&g.0]].kind = Kind::Gate(g.1, ids);
        }
        for (name, src, pos, ln) in outs {
            let s = *index.get(&src).ok_or_else(|| format!("Line {ln}: {name} reads \u{201c}{src}\u{201d}, which is not defined"))?;
            c.outputs.push(Output { name, src: s, pos });
        }
        c.order = c.topo_order()?;
        Ok(c)
    }

    fn topo_order(&self) -> Result<Vec<usize>, String> {
        // 0 = unseen, 1 = on the current path, 2 = done
        let mut state = vec![0u8; self.nodes.len()];
        let mut order = Vec::new();
        for start in 0..self.nodes.len() {
            if state[start] != 0 {
                continue;
            }
            let mut stack = vec![(start, 0usize)];
            state[start] = 1;
            while let Some((n, k)) = stack.pop() {
                let args: &[usize] = match &self.nodes[n].kind {
                    Kind::Gate(_, a) => a,
                    Kind::Input => &[],
                };
                if k < args.len() {
                    stack.push((n, k + 1));
                    let a = args[k];
                    match state[a] {
                        0 => {
                            state[a] = 1;
                            stack.push((a, 0));
                        }
                        1 => return Err(format!("{} feeds back into itself: a loop is not a combinational circuit", self.nodes[a].name)),
                        _ => {}
                    }
                } else {
                    state[n] = 2;
                    if matches!(self.nodes[n].kind, Kind::Gate(..)) {
                        order.push(n);
                    }
                }
            }
        }
        Ok(order)
    }

    // ----- editing (the canvas): each edit keeps the circuit valid or is refused -----

    /// A name not yet used by any input, gate or output: `base`, else `base2`, `base3` ...
    pub fn fresh_name(&self, base: &str) -> String {
        if !self.name_taken(base) {
            return base.to_string();
        }
        (2..).map(|k| format!("{base}{k}")).find(|n| !self.name_taken(n)).unwrap_or_else(|| base.to_string())
    }

    /// The next free input letter: A, B, C ... then X1, X2 ...
    pub fn fresh_input_name(&self) -> String {
        ('A'..='Z').map(|c| c.to_string()).find(|n| !self.name_taken(n)).unwrap_or_else(|| self.fresh_name("X"))
    }

    fn room(&self) -> Result<(), String> {
        if self.nodes.len() >= MAX_NODES { Err(format!("A circuit is limited to {MAX_NODES} inputs and gates")) } else { Ok(()) }
    }

    pub fn add_input(&mut self, pos: Option<(f64, f64)>) -> Result<usize, String> {
        self.room()?;
        let name = self.fresh_input_name();
        self.nodes.push(Node { name, kind: Kind::Input, pos });
        self.order = self.topo_order()?;
        Ok(self.nodes.len() - 1)
    }

    /// A gate reading `srcs` (a gate has to read something, so it is made with its inputs wired).
    pub fn add_gate(&mut self, g: Gate, srcs: Vec<usize>, pos: Option<(f64, f64)>) -> Result<usize, String> {
        self.room()?;
        let (lo, hi) = g.arity();
        if srcs.len() < lo || srcs.len() > hi || srcs.iter().any(|s| *s >= self.nodes.len()) {
            return Err(format!("{} takes {lo} to {hi} inputs", g.name()));
        }
        let name = self.fresh_name("g1");
        // g1, g2, ...: the first free number
        let name = if name == "g1" { name } else { (1..).map(|k| format!("g{k}")).find(|n| !self.name_taken(n)).unwrap() };
        self.nodes.push(Node { name, kind: Kind::Gate(g, srcs), pos });
        self.order = self.topo_order()?;
        Ok(self.nodes.len() - 1)
    }

    pub fn add_output(&mut self, src: usize, pos: Option<(f64, f64)>) -> Result<usize, String> {
        if src >= self.nodes.len() {
            return Err("That part is gone".into());
        }
        let name = self.fresh_name("Q");
        self.outputs.push(Output { name, src, pos });
        Ok(self.outputs.len() - 1)
    }

    /// Whether `a` reads `b`, directly or through other gates (or is `b`).
    pub fn depends_on(&self, a: usize, b: usize) -> bool {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![a];
        while let Some(n) = stack.pop() {
            if n == b {
                return true;
            }
            if std::mem::replace(&mut seen[n], true) {
                continue;
            }
            if let Kind::Gate(_, args) = &self.nodes[n].kind {
                stack.extend(args.iter().copied());
            }
        }
        false
    }

    /// Wire `src` into input `pin` of `gate`, replacing what was there. Refused if it would make
    /// a loop. `pin` one past the last appends (while the gate has room).
    pub fn set_pin(&mut self, gate: usize, pin: usize, src: usize) -> Result<(), String> {
        if gate >= self.nodes.len() || src >= self.nodes.len() {
            return Err("That part is gone".into());
        }
        if self.depends_on(src, gate) {
            return Err("That wire would make a loop".into());
        }
        let Kind::Gate(g, args) = &mut self.nodes[gate].kind else { return Err("Only a gate has inputs to wire".into()) };
        if pin < args.len() {
            args[pin] = src;
        } else if args.len() < g.arity().1 {
            args.push(src);
        } else {
            return Err(format!("{} takes at most {} inputs", g.name(), g.arity().1));
        }
        self.order = self.topo_order()?;
        Ok(())
    }

    pub fn set_output_src(&mut self, out: usize, src: usize) -> Result<(), String> {
        if out >= self.outputs.len() || src >= self.nodes.len() {
            return Err("That part is gone".into());
        }
        self.outputs[out].src = src;
        Ok(())
    }

    /// Removes the nodes in `gone` and every gate left with too few inputs, and the outputs that
    /// read them. Returns how many nodes went.
    pub fn delete_nodes(&mut self, gone: &[usize]) -> usize {
        let n = self.nodes.len();
        let mut dead = vec![false; n];
        for g in gone {
            if *g < n {
                dead[*g] = true;
            }
        }
        loop {
            let mut changed = false;
            for i in 0..n {
                if dead[i] {
                    continue;
                }
                if let Kind::Gate(g, a) = &self.nodes[i].kind {
                    if a.iter().filter(|s| !dead[**s]).count() < g.arity().0 {
                        dead[i] = true;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let remap: Vec<Option<usize>> = {
            let mut next = 0;
            dead.iter().map(|d| if *d { None } else { next += 1; Some(next - 1) }).collect()
        };
        let removed = dead.iter().filter(|d| **d).count();
        let old = std::mem::take(&mut self.nodes);
        for (i, mut node) in old.into_iter().enumerate() {
            if dead[i] {
                continue;
            }
            if let Kind::Gate(_, a) = &mut node.kind {
                *a = a.iter().filter_map(|s| remap[*s]).collect();
            }
            self.nodes.push(node);
        }
        self.outputs.retain_mut(|o| match remap[o.src] {
            Some(s) => {
                o.src = s;
                true
            }
            None => false,
        });
        self.order = self.topo_order().unwrap_or_default();
        removed
    }

    pub fn delete_output(&mut self, k: usize) {
        if k < self.outputs.len() {
            self.outputs.remove(k);
        }
    }

    /// Cuts the wire into input `pin` of `gate` (a gate left with too few inputs goes too).
    pub fn disconnect(&mut self, gate: usize, pin: usize) -> usize {
        let Some(Node { kind: Kind::Gate(g, args), .. }) = self.nodes.get_mut(gate) else { return 0 };
        if pin >= args.len() {
            return 0;
        }
        if args.len() - 1 >= g.arity().0 {
            args.remove(pin);
            self.order = self.topo_order().unwrap_or_default();
            0
        } else {
            self.delete_nodes(&[gate])
        }
    }

    /// Renames an input or gate (`Ok` only for a usable, unused name).
    pub fn rename_node(&mut self, i: usize, name: &str) -> Result<(), String> {
        let name = name.trim();
        valid_name(name)?;
        if self.nodes.get(i).is_some_and(|n| n.name == name) {
            return Ok(());
        }
        if self.name_taken(name) {
            return Err(format!("\u{201c}{name}\u{201d} is already used"));
        }
        self.nodes.get_mut(i).ok_or("That part is gone")?.name = name.to_string();
        Ok(())
    }

    pub fn rename_output(&mut self, k: usize, name: &str) -> Result<(), String> {
        let name = name.trim();
        valid_name(name)?;
        if self.outputs.get(k).is_some_and(|o| o.name == name) {
            return Ok(());
        }
        if self.name_taken(name) {
            return Err(format!("\u{201c}{name}\u{201d} is already used"));
        }
        self.outputs.get_mut(k).ok_or("That part is gone")?.name = name.to_string();
        Ok(())
    }

    /// The spec text for this circuit (parses back to an equal circuit).
    pub fn to_spec(&self) -> String {
        let pos = |p: Option<(f64, f64)>| p.map_or(String::new(), |(x, y)| format!(" @ {},{}", crate::figures::data::fmt_num(x), crate::figures::data::fmt_num(y)));
        let mut s = String::new();
        let ins: Vec<&str> = self.nodes.iter().filter(|n| matches!(n.kind, Kind::Input) && n.pos.is_none()).map(|n| n.name.as_str()).collect();
        if !ins.is_empty() {
            s += &format!("in {}\n", ins.join(", "));
        }
        for n in self.nodes.iter().filter(|n| matches!(n.kind, Kind::Input) && n.pos.is_some()) {
            s += &format!("in {}{}\n", n.name, pos(n.pos));
        }
        for n in &self.nodes {
            if let Kind::Gate(g, a) = &n.kind {
                let args: Vec<&str> = a.iter().map(|i| self.nodes[*i].name.as_str()).collect();
                s += &format!("{} = {}({}){}\n", n.name, g.name(), args.join(", "), pos(n.pos));
            }
        }
        for o in &self.outputs {
            s += &format!("out {} = {}{}\n", o.name, self.nodes[o.src].name, pos(o.pos));
        }
        s
    }

    /// Every node's value for the given input values (in the order of `inputs()`; missing ones
    /// read as 0), and every output's value.
    pub fn eval(&self, inputs: &[bool]) -> (Vec<bool>, Vec<bool>) {
        let mut v = vec![false; self.nodes.len()];
        for (k, i) in self.inputs().into_iter().enumerate() {
            v[i] = inputs.get(k).copied().unwrap_or(false);
        }
        for &n in &self.order {
            if let Kind::Gate(g, a) = &self.nodes[n].kind {
                let args: Vec<bool> = a.iter().map(|i| v[*i]).collect();
                v[n] = g.apply(&args);
            }
        }
        let outs = self.outputs.iter().map(|o| v[o.src]).collect();
        (v, outs)
    }

    /// All input rows (first input is the most significant bit, rows count up from all zeros)
    /// with their output values. Refused past `MAX_TABLE_INPUTS` inputs.
    #[allow(clippy::type_complexity)]
    pub fn truth_table(&self) -> Result<Vec<(Vec<bool>, Vec<bool>)>, String> {
        let n = self.inputs().len();
        if n > MAX_TABLE_INPUTS {
            return Err(format!("A truth table is limited to {MAX_TABLE_INPUTS} inputs ({n} here)"));
        }
        Ok((0..1u32 << n)
            .map(|row| {
                let ins: Vec<bool> = (0..n).map(|k| row >> (n - 1 - k) & 1 == 1).collect();
                let (_, outs) = self.eval(&ins);
                (ins, outs)
            })
            .collect())
    }

    /// The boolean expression output `k` stands for (gates written out, inputs by name). Very
    /// large shared circuits are cut short with an ellipsis rather than growing without bound.
    pub fn expr(&self, k: usize) -> String {
        let Some(o) = self.outputs.get(k) else { return String::new() };
        let mut budget = 400usize;
        self.expr_node(o.src, 0, &mut budget)
    }

    fn expr_node(&self, n: usize, min_prec: u8, budget: &mut usize) -> String {
        if *budget == 0 {
            return "\u{2026}".into();
        }
        *budget -= 1;
        let wrap = |s: String, p: u8| if p < min_prec { format!("({s})") } else { s };
        match &self.nodes[n].kind {
            Kind::Input => self.nodes[n].name.clone(),
            Kind::Gate(g, a) => {
                let join = |op: &str, p: u8, budget: &mut usize| -> String { a.iter().map(|i| self.expr_node(*i, p + 1, budget)).collect::<Vec<_>>().join(&format!(" {op} ")) };
                match g {
                    Gate::Buf => self.expr_node(a[0], min_prec, budget),
                    Gate::Not => format!("NOT {}", self.expr_node(a[0], 5, budget)),
                    Gate::And => wrap(join("AND", 3, budget), 3),
                    Gate::Or => wrap(join("OR", 1, budget), 1),
                    Gate::Xor => wrap(join("XOR", 2, budget), 2),
                    Gate::Nand => format!("NOT ({})", join("AND", 3, budget)),
                    Gate::Nor => format!("NOT ({})", join("OR", 1, budget)),
                    Gate::Xnor => format!("NOT ({})", join("XOR", 2, budget)),
                }
            }
        }
    }

    /// Where each node and output sits, in grid units: the spec's `@x,y` where given, else by
    /// level (inputs at 0, each gate one past the deepest thing it reads, outputs last) with the
    /// rows ordered to keep wires short. The third result is, for a wire that skips levels, the
    /// grid points it passes through (keyed by the reading node, or `nodes.len() + k` for output
    /// `k`, and the input pin), so it can be routed between the gates rather than through them.
    pub fn layout(&self) -> (Vec<(f64, f64)>, Vec<(f64, f64)>, Routes) {
        let n = self.nodes.len();
        let m = self.outputs.len();
        let mut level = vec![0usize; n];
        for &g in &self.order {
            if let Kind::Gate(_, a) = &self.nodes[g].kind {
                level[g] = a.iter().map(|i| level[*i]).max().unwrap_or(0) + 1;
            }
        }
        let deepest = self.outputs.iter().map(|o| level[o.src]).chain(level.iter().copied()).max().unwrap_or(0);
        let out_level = deepest + 1;
        // items: nodes, then outputs, then one dummy per level a long wire crosses
        let mut item_level: Vec<usize> = level.clone();
        item_level.extend(std::iter::repeat(out_level).take(m));
        let mut parents: Vec<Vec<usize>> = (0..n + m)
            .map(|i| match (i < n).then(|| &self.nodes[i].kind) {
                Some(Kind::Gate(_, a)) => a.clone(),
                Some(Kind::Input) => Vec::new(),
                None => vec![self.outputs[i - n].src],
            })
            .collect();
        let mut chains: Vec<((usize, usize), Vec<usize>)> = Vec::new();
        for c in 0..n + m {
            for k in 0..parents[c].len() {
                let src = parents[c][k];
                let mut prev = src;
                let mut chain = Vec::new();
                for l in item_level[src] + 1..item_level[c] {
                    item_level.push(l);
                    parents.push(vec![prev]);
                    prev = item_level.len() - 1;
                    chain.push(prev);
                }
                parents[c][k] = prev;
                if !chain.is_empty() {
                    chains.push(((c, k), chain));
                }
            }
        }
        let total = item_level.len();
        let mut by_level: Vec<Vec<usize>> = vec![Vec::new(); out_level + 1];
        for i in 0..total {
            by_level[item_level[i]].push(i);
        }
        let widest = by_level.iter().map(Vec::len).max().unwrap_or(1) as f64;
        let mut rows = vec![0.0f64; total];
        let place = |lvl: &Vec<usize>, rows: &mut Vec<f64>| {
            let off = (widest - lvl.len() as f64) / 2.0;
            for (r, i) in lvl.iter().enumerate() {
                rows[*i] = r as f64 + off;
            }
        };
        for lvl in &by_level {
            place(lvl, &mut rows);
        }
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); total];
        for (c, ps) in parents.iter().enumerate() {
            for p in ps {
                children[*p].push(c);
            }
        }
        let avg = |ids: &[usize], rows: &[f64]| ids.iter().map(|i| rows[*i]).sum::<f64>() / ids.len().max(1) as f64;
        // inputs and outputs keep the order they were written in; the levels between them are
        // sorted by where their neighbours sit, down, up, then down again
        for sweep in 0..3 {
            let order: Vec<usize> = if sweep == 1 { (1..=deepest).rev().collect() } else { (1..=deepest).collect() };
            for lvl in order {
                let mut items = by_level[lvl].clone();
                let key = |i: usize, rows: &[f64]| if sweep == 1 { avg(&children[i], rows) } else { avg(&parents[i], rows) };
                let keys: Vec<f64> = items.iter().map(|i| key(*i, &rows)).collect();
                let mut idx: Vec<usize> = (0..items.len()).collect();
                idx.sort_by(|a, b| keys[*a].partial_cmp(&keys[*b]).unwrap_or(std::cmp::Ordering::Equal));
                items = idx.into_iter().map(|i| items[i]).collect();
                place(&items, &mut rows);
                by_level[lvl] = items;
            }
        }
        let node_pos = (0..n).map(|i| self.nodes[i].pos.unwrap_or((level[i] as f64, rows[i]))).collect();
        let out_pos = (0..m).map(|k| self.outputs[k].pos.unwrap_or((out_level as f64, rows[n + k]))).collect();
        let routes = chains.into_iter().map(|(key, ch)| (key, ch.into_iter().map(|d| (item_level[d] as f64, rows[d])).collect())).collect();
        (node_pos, out_pos, routes)
    }

    /// A circuit for boolean expressions, one per line as `Q = A AND (B OR NOT C)` (a line with
    /// no `=` is an output named Q, Q2, ...). Words AND OR XOR NOT, or the symbols & | ^ ! ~ and
    /// + (or) * (and). Names that are never on the left of an `=` become the inputs.
    pub fn from_exprs(text: &str) -> Result<Circuit, String> {
        if text.len() > MAX_SPEC {
            return Err(format!("The expression text is longer than {MAX_SPEC} characters"));
        }
        let mut defs: Vec<(String, Vec<Tok>)> = Vec::new();
        for (ln, raw) in text.lines().flat_map(|l| l.split(';')).enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (name, body) = match line.split_once('=') {
                Some((l, r)) => (l.trim().to_string(), r),
                None => (if defs.is_empty() { "Q".to_string() } else { format!("Q{}", defs.len() + 1) }, line),
            };
            valid_name(&name).map_err(|e| format!("Expression {}: {e}", ln + 1))?;
            if defs.iter().any(|d| d.0 == name) {
                return Err(format!("Expression {}: \u{201c}{name}\u{201d} is defined twice", ln + 1));
            }
            defs.push((name, tokenize(body).map_err(|e| format!("Expression {}: {e}", ln + 1))?));
        }
        if defs.is_empty() {
            return Err("Type an expression such as Q = A AND B".into());
        }
        let mut asts = Vec::new();
        for (name, toks) in &defs {
            let mut p = ExprParser { t: toks, i: 0, depth: 0 };
            let e = p.or().map_err(|e| format!("{name}: {e}"))?;
            if p.i < toks.len() {
                return Err(format!("{name}: unexpected {}", toks[p.i].show()));
            }
            asts.push(e);
        }
        let defined: Vec<&str> = defs.iter().map(|d| d.0.as_str()).collect();
        let mut input_names: Vec<String> = Vec::new();
        for a in &asts {
            a.idents(&mut |n| {
                if !defined.contains(&n) && !input_names.iter().any(|x| x == n) {
                    input_names.push(n.to_string());
                }
            });
        }
        for n in &input_names {
            valid_name(n)?;
        }
        let mut c = Circuit::default();
        for n in &input_names {
            c.nodes.push(Node { name: n.clone(), kind: Kind::Input, pos: None });
        }
        let mut b = Builder { c, defs: &defined, asts: &asts, built: vec![None; defs.len()], active: vec![false; defs.len()], memo: HashMap::new(), next: 1 };
        let mut srcs = Vec::new();
        for k in 0..defs.len() {
            srcs.push(b.def(k)?);
        }
        let mut c = b.c;
        for (k, (name, _)) in defs.iter().enumerate() {
            c.outputs.push(Output { name: name.clone(), src: srcs[k], pos: None });
        }
        if c.nodes.len() > MAX_NODES {
            return Err(format!("That needs more than {MAX_NODES} inputs and gates"));
        }
        c.order = c.topo_order()?;
        Ok(c)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Id(String),
    And,
    Or,
    Xor,
    Not,
    Open,
    Close,
}

impl Tok {
    fn show(&self) -> String {
        match self {
            Tok::Id(s) => s.clone(),
            Tok::And => "AND".into(),
            Tok::Or => "OR".into(),
            Tok::Xor => "XOR".into(),
            Tok::Not => "NOT".into(),
            Tok::Open => "(".into(),
            Tok::Close => ")".into(),
        }
    }
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let ch: Vec<char> = s.chars().collect();
    let mut t = Vec::new();
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => { t.push(Tok::Open); i += 1 }
            ')' => { t.push(Tok::Close); i += 1 }
            '&' | '*' | '\u{b7}' | '\u{2227}' => { t.push(Tok::And); i += 1 }
            '|' | '+' | '\u{2228}' => { t.push(Tok::Or); i += 1 }
            '^' | '\u{2295}' => { t.push(Tok::Xor); i += 1 }
            '!' | '~' | '\u{ac}' => { t.push(Tok::Not); i += 1 }
            c if c.is_alphabetic() || c == '_' => {
                let st = i;
                while i < ch.len() && (ch[i].is_alphanumeric() || ch[i] == '_') {
                    i += 1;
                }
                let w: String = ch[st..i].iter().collect();
                t.push(match w.to_ascii_uppercase().as_str() {
                    "AND" => Tok::And,
                    "OR" => Tok::Or,
                    "XOR" => Tok::Xor,
                    "NOT" => Tok::Not,
                    _ => Tok::Id(w),
                });
            }
            c => return Err(format!("unexpected \u{201c}{c}\u{201d}")),
        }
        if t.len() > 400 {
            return Err("that expression is too long".into());
        }
    }
    Ok(t)
}

#[derive(Clone, Debug)]
enum Ast {
    Id(String),
    Not(Box<Ast>),
    Bin(Gate, Box<Ast>, Box<Ast>),
}

impl Ast {
    fn idents(&self, f: &mut dyn FnMut(&str)) {
        match self {
            Ast::Id(n) => f(n),
            Ast::Not(a) => a.idents(f),
            Ast::Bin(_, a, b) => {
                a.idents(f);
                b.idents(f);
            }
        }
    }
}

struct ExprParser<'a> {
    t: &'a [Tok],
    i: usize,
    depth: usize,
}

impl ExprParser<'_> {
    fn peek(&self) -> Option<&Tok> { self.t.get(self.i) }

    fn level(&mut self, op: Tok, g: Gate, next: fn(&mut Self) -> Result<Ast, String>) -> Result<Ast, String> {
        let mut a = next(self)?;
        while self.peek() == Some(&op) {
            self.i += 1;
            let b = next(self)?;
            a = Ast::Bin(g, Box::new(a), Box::new(b));
        }
        Ok(a)
    }

    fn or(&mut self) -> Result<Ast, String> { self.level(Tok::Or, Gate::Or, Self::xor) }
    fn xor(&mut self) -> Result<Ast, String> { self.level(Tok::Xor, Gate::Xor, Self::and) }
    fn and(&mut self) -> Result<Ast, String> { self.level(Tok::And, Gate::And, Self::not) }

    fn not(&mut self) -> Result<Ast, String> {
        self.depth += 1;
        if self.depth > 60 {
            return Err("nested too deeply".into());
        }
        let r = match self.peek() {
            Some(Tok::Not) => {
                self.i += 1;
                Ok(Ast::Not(Box::new(self.not()?)))
            }
            Some(Tok::Open) => {
                self.i += 1;
                let e = self.or()?;
                if self.peek() != Some(&Tok::Close) {
                    return Err("missing )".into());
                }
                self.i += 1;
                Ok(e)
            }
            Some(Tok::Id(n)) => {
                let n = n.clone();
                self.i += 1;
                Ok(Ast::Id(n))
            }
            Some(t) => Err(format!("unexpected {}", t.show())),
            None => Err("the expression ends too soon".into()),
        };
        self.depth -= 1;
        r
    }
}

struct Builder<'a> {
    c: Circuit,
    defs: &'a [&'a str],
    asts: &'a [Ast],
    built: Vec<Option<usize>>,
    active: Vec<bool>,
    memo: HashMap<(Gate, Vec<usize>), usize>,
    next: usize,
}

impl Builder<'_> {
    fn def(&mut self, k: usize) -> Result<usize, String> {
        if let Some(n) = self.built[k] {
            return Ok(n);
        }
        if self.active[k] {
            return Err(format!("{} is defined in terms of itself", self.defs[k]));
        }
        self.active[k] = true;
        let n = self.build(&self.asts[k].clone())?;
        self.active[k] = false;
        self.built[k] = Some(n);
        Ok(n)
    }

    fn gate(&mut self, g: Gate, args: Vec<usize>) -> Result<usize, String> {
        if let Some(n) = self.memo.get(&(g, args.clone())) {
            return Ok(*n);
        }
        let mut name;
        loop {
            name = format!("g{}", self.next);
            self.next += 1;
            if !self.c.name_taken(&name) && !self.defs.contains(&name.as_str()) {
                break;
            }
        }
        if self.c.nodes.len() >= MAX_NODES {
            return Err(format!("That needs more than {MAX_NODES} inputs and gates"));
        }
        self.c.nodes.push(Node { name, kind: Kind::Gate(g, args.clone()), pos: None });
        let id = self.c.nodes.len() - 1;
        self.memo.insert((g, args), id);
        Ok(id)
    }

    fn build(&mut self, a: &Ast) -> Result<usize, String> {
        match a {
            Ast::Id(n) => match self.defs.iter().position(|d| d == n) {
                Some(k) => self.def(k),
                None => Ok(self.c.nodes.iter().position(|x| &x.name == n && matches!(x.kind, Kind::Input)).expect("inputs were collected from the same text")),
            },
            Ast::Not(x) => {
                let x = self.build(x)?;
                self.gate(Gate::Not, vec![x])
            }
            Ast::Bin(g, x, y) => {
                let (x, y) = (self.build(x)?, self.build(y)?);
                let args = vec![x, y];
                self.gate(*g, args)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: &str = "in A, B\ng1 = XOR(A, B)\ng2 = AND(A, B)\nout S = g1\nout C = g2\n";

    fn table(c: &Circuit) -> Vec<Vec<bool>> { c.truth_table().unwrap().into_iter().map(|r| r.1).collect() }

    #[test]
    fn half_adder_table() {
        let c = Circuit::parse(HALF).unwrap();
        assert_eq!(table(&c), vec![vec![false, false], vec![true, false], vec![true, false], vec![false, true]]);
    }

    #[test]
    fn spec_round_trips_with_positions() {
        let c = Circuit::parse("in A, B\ng1 = NAND(A, B) @ 2.5,1\nout Q = g1 @ 4,1").unwrap();
        let again = Circuit::parse(&c.to_spec()).unwrap();
        assert_eq!(c, again);
        assert_eq!(again.nodes[2].pos, Some((2.5, 1.0)));
    }

    #[test]
    fn gates_may_be_listed_before_what_they_read_and_comments_are_ignored() {
        let c = Circuit::parse("# fwd\nout Q = g2\ng2 = NOT(g1)  # inverter\ng1 = OR(A, B)\nin A B\n").unwrap();
        assert_eq!(table(&c).iter().map(|r| r[0]).collect::<Vec<_>>(), vec![true, false, false, false]);
    }

    #[test]
    fn every_gate_matches_its_definition() {
        for g in Gate::ALL {
            let (lo, _) = g.arity();
            for row in 0..1u32 << lo {
                let v: Vec<bool> = (0..lo).map(|k| row >> k & 1 == 1).collect();
                let want = match g {
                    Gate::And => v[0] && v[1],
                    Gate::Or => v[0] || v[1],
                    Gate::Not => !v[0],
                    Gate::Nand => !(v[0] && v[1]),
                    Gate::Nor => !(v[0] || v[1]),
                    Gate::Xor => v[0] != v[1],
                    Gate::Xnor => v[0] == v[1],
                    Gate::Buf => v[0],
                };
                assert_eq!(g.apply(&v), want, "{g:?} {v:?}");
            }
        }
        assert!(Gate::Xor.apply(&[true, true, true]), "three-input xor is odd parity");
    }

    #[test]
    fn malformed_specs_say_where_and_why() {
        for (spec, needle) in [
            ("in A\ng = AND(A)", "AND takes 2 to 4"),
            ("in A\ng = FOO(A, A)", "not a gate"),
            ("in A\ng = AND(A, B)\nout Q = g", "not defined"),
            ("in A\nout Q = g", "not defined"),
            ("in A, A", "already used"),
            ("in A\nnonsense", "Line 2"),
            ("in 9x", "not a usable name"),
            ("in AND", "reserved"),
            ("in A\ng = AND(A, A", "closing"),
            ("in A\ng = NOT(A) @ x", "@x,y"),
            ("in A, B @ 1,1", "only a single input"),
            ("in A\ng = NOT(A) @ 1e9,0", "@x,y"),
            ("in AAAAAAAAAAAAAAAAAAAA", "longer than"),
        ] {
            let e = Circuit::parse(spec).unwrap_err();
            assert!(e.contains(needle), "{spec:?} -> {e}");
        }
    }

    #[test]
    fn loops_are_refused() {
        let e = Circuit::parse("in A\ng1 = AND(A, g2)\ng2 = NOT(g1)\nout Q = g2").unwrap_err();
        assert!(e.contains("loop"), "{e}");
        assert!(Circuit::parse("in A\ng = NOT(g)").unwrap_err().contains("loop"));
    }

    #[test]
    fn size_caps_hold() {
        let big: String = (0..MAX_NODES + 1).map(|i| format!("in x{i}\n")).collect();
        assert!(Circuit::parse(&big).unwrap_err().contains("More than"));
        assert!(Circuit::parse(&"#".repeat(MAX_SPEC + 1)).is_err());
        assert_eq!(Circuit::parse("").unwrap(), Circuit::default());
        assert!(Circuit::parse("\u{0}\u{1}\u{ffff} = ??").is_err());
    }

    #[test]
    fn table_is_capped() {
        let c = Circuit::parse("in A, B, C, D, E, F\ng = AND(A, B, C, D)\nout Q = g").unwrap();
        assert!(c.truth_table().unwrap_err().contains("limited to 5"));
        let c = Circuit::parse("in A, B, C, D, E\ng = AND(A, B, C, D)\nout Q = g").unwrap();
        assert_eq!(c.truth_table().unwrap().len(), 32);
    }

    #[test]
    fn expression_of_a_diagram() {
        let c = Circuit::parse(HALF).unwrap();
        assert_eq!((c.expr(0), c.expr(1)), ("A XOR B".to_string(), "A AND B".to_string()));
        let c = Circuit::parse("in A, B, C\ng1 = NAND(A, B)\ng2 = OR(g1, C)\ng3 = NOT(g2)\nout Q = g3").unwrap();
        assert_eq!(c.expr(0), "NOT (NOT (A AND B) OR C)");
        assert_eq!(c.expr(5), "");
    }

    #[test]
    fn expression_text_stays_bounded_on_a_deeply_shared_circuit() {
        let mut s = String::from("in A\ng0 = NOT(A)\n");
        for i in 1..MAX_NODES - 2 {
            s += &format!("g{i} = XOR(g{}, g{})\n", i - 1, i - 1);
        }
        s += &format!("out Q = g{}\n", MAX_NODES - 3);
        let c = Circuit::parse(&s).unwrap();
        assert!(c.expr(0).chars().count() < 4000);
    }

    /// For an expression, the diagram built from it gives the same answers as the expression
    /// itself read straight off, and the expression read back off the diagram means the same.
    #[test]
    fn expressions_become_diagrams_that_agree_with_them() {
        for (text, f) in [
            ("Q = A AND B OR NOT C", (|a: bool, b: bool, c: bool| (a && b) || !c) as fn(bool, bool, bool) -> bool),
            ("Q = (A | B) & !C", |a, b, c| (a || b) && !c),
            ("A ^ B ^ C", |a, b, c| a ^ b ^ c),
            ("Q = ~(A * B) + C", |a, b, c| !(a && b) || c),
            ("Q = NOT NOT A XOR (B AND C)", |a, b, c| a ^ (b && c)),
        ] {
            let circ = Circuit::from_exprs(text).unwrap();
            assert_eq!(circ.inputs().len(), 3, "{text}");
            for (ins, outs) in circ.truth_table().unwrap() {
                assert_eq!(outs[0], f(ins[0], ins[1], ins[2]), "{text} {ins:?}");
            }
            // read back and rebuild
            let back = Circuit::from_exprs(&format!("Q = {}", circ.expr(0))).unwrap();
            assert_eq!(table(&circ), table(&back), "{text} -> {}", circ.expr(0));
            // and the spec round trips
            assert_eq!(Circuit::parse(&circ.to_spec()).unwrap(), circ);
        }
    }

    #[test]
    fn several_expressions_share_inputs_and_gates() {
        let c = Circuit::from_exprs("S = A XOR B\nC = A AND B").unwrap();
        assert_eq!(c.inputs().len(), 2);
        assert_eq!(table(&c), table(&Circuit::parse(HALF).unwrap()));
        let c = Circuit::from_exprs("X = A AND B; Y = X OR C").unwrap();
        assert_eq!(c.inputs().len(), 3, "an output used by a later line is not an input");
    }

    #[test]
    fn bad_expressions_are_explained() {
        for (t, needle) in [
            ("", "Type an expression"),
            ("Q = A AND", "ends too soon"),
            ("Q = (A AND B", "missing )"),
            ("Q = A B", "unexpected B"),
            ("Q = A $ B", "unexpected"),
            ("Q = A AND B\nQ = A", "twice"),
            ("X = Y\nY = X", "itself"),
        ] {
            let e = Circuit::from_exprs(t).unwrap_err();
            assert!(e.contains(needle), "{t:?} -> {e}");
        }
        assert!(Circuit::from_exprs(&"A AND ".repeat(300)).is_err());
        let deep = format!("Q = {}A{}", "(".repeat(100), ")".repeat(100));
        assert!(Circuit::from_exprs(&deep).unwrap_err().contains("deeply"));
    }

    #[test]
    fn editing_keeps_the_circuit_valid() {
        let mut c = Circuit::parse(HALF).unwrap();
        // a new input, a gate reading it, an output
        let c0 = c.add_input(Some((0.0, 2.0))).unwrap();
        assert_eq!(c.nodes[c0].name, "D", "C is taken by an output");
        let g = c.add_gate(Gate::Or, vec![0, c0], Some((2.0, 2.0))).unwrap();
        assert_eq!(c.nodes[g].name, "g3");
        let o = c.add_output(g, None).unwrap();
        assert_eq!(c.outputs[o].name, "Q");
        // the spec reads back to the same circuit (the parts may be listed in a new order)
        let back = Circuit::parse(&c.to_spec()).unwrap();
        assert_eq!(Circuit::parse(&back.to_spec()).unwrap(), back);
        assert_eq!(back.nodes.iter().find(|n| n.name == "D").unwrap().pos, Some((0.0, 2.0)), "an input's place is kept");
        assert_eq!(table(&back), table(&c));
        // wiring replaces a pin, a loop is refused, and nothing changed by the refusal
        let h = c.add_gate(Gate::Not, vec![g], None).unwrap();
        let before = c.clone();
        let e = c.set_pin(g, 0, h).unwrap_err();
        assert!(e.contains("loop"), "{e}");
        assert_eq!(c, before);
        c.set_pin(g, 1, 1).unwrap();
        assert_eq!(c.expr(o), "A OR B");
        // one past the end appends, up to the gate's limit
        c.set_pin(g, 2, c0).unwrap();
        assert_eq!(c.expr(o), "A OR B OR D");
        c.set_pin(g, 3, c0).unwrap();
        assert!(c.set_pin(g, 4, c0).unwrap_err().contains("at most"));
        assert!(c.set_pin(0, 0, 1).unwrap_err().contains("Only a gate"));
    }

    #[test]
    fn deleting_takes_dependents_with_it() {
        let mut c = Circuit::parse(HALF).unwrap();
        // A feeds both gates, each of which needs two inputs: everything but B goes, with its outputs
        let gone = c.delete_nodes(&[0]);
        assert_eq!(gone, 3);
        assert_eq!(c.nodes.len(), 1);
        assert!(c.outputs.is_empty());
        assert_eq!(Circuit::parse(&c.to_spec()).unwrap(), c);
        // a gate with spare inputs keeps going
        let mut c = Circuit::parse("in A, B, C\ng = AND(A, B, C)\nout Q = g").unwrap();
        assert_eq!(c.delete_nodes(&[2]), 1);
        assert_eq!(c.expr(0), "A AND B");
        // cutting one wire of a two-input gate removes the gate; of a three-input gate does not
        let mut c = Circuit::parse("in A, B, C\ng = AND(A, B, C)\nh = OR(A, g)\nout Q = h").unwrap();
        assert_eq!(c.disconnect(3, 2), 0);
        assert_eq!(c.expr(0), "A OR A AND B");
        assert_eq!(c.disconnect(4, 1), 1, "OR left with one input goes");
        assert!(c.outputs.is_empty());
        // out of range is a no-op
        assert_eq!(c.delete_nodes(&[99]), 0);
        c.delete_output(9);
    }

    #[test]
    fn renaming_checks_names() {
        let mut c = Circuit::parse(HALF).unwrap();
        c.rename_node(0, "X").unwrap();
        assert!(c.rename_node(1, "X").unwrap_err().contains("already used"));
        assert!(c.rename_node(1, "9").is_err());
        assert!(c.rename_output(0, "g1").unwrap_err().contains("already used"));
        c.rename_output(0, "Sum").unwrap();
        assert_eq!(c.expr(0), "X XOR B");
        assert_eq!(Circuit::parse(&c.to_spec()).unwrap(), c);
        let mut full = Circuit::default();
        for _ in 0..MAX_NODES {
            full.add_input(None).unwrap();
        }
        assert!(full.add_input(None).unwrap_err().contains("limited to"));
        assert_eq!(full.nodes[26].name, "X2");
    }

    #[test]
    fn layout_levels_and_overrides() {
        let c = Circuit::parse("in A, B\ng1 = AND(A, B)\ng2 = NOT(g1) @ 9,9\nout Q = g2").unwrap();
        let (n, o, _) = c.layout();
        assert_eq!(n[0].0, 0.0);
        assert_eq!(n[2].0, 1.0);
        assert_eq!(n[3], (9.0, 9.0), "the spec's position wins");
        assert!(o[0].0 > n[2].0);
    }

    #[test]
    fn a_wire_that_skips_levels_is_routed_through_its_own_row() {
        // Cout needs the adder's second level; Sum is ready earlier, so its wire to the outputs skips one
        let c = Circuit::parse("in A, B, C\nx = XOR(A, B)\na = AND(A, B)\nb = AND(x, C)\ns = XOR(x, C)\nco = OR(a, b)\nout Sum = s\nout Cout = co").unwrap();
        let (n, o, routes) = c.layout();
        assert_eq!(n.len(), c.nodes.len());
        assert!(!routes.is_empty(), "a is read two levels later, so it passes the middle column");
        // no gate or dummy shares a spot with another in its column
        let mut spots: Vec<(i64, i64)> = n.iter().chain(o.iter()).chain(routes.values().flatten()).map(|p| (p.0 as i64, (p.1 * 100.0) as i64)).collect();
        let before = spots.len();
        spots.sort();
        spots.dedup();
        assert_eq!(spots.len(), before, "{spots:?}");
    }
}
