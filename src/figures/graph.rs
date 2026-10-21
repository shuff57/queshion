//! The models behind the graph and tree figures: a graph of named nodes and edges written as
//! text (`A - B`, `A -> C : 4`), a tree written as parent lines (`A: B, C`) or grown from a list
//! of values (a binary search tree), with traversals, adjacency tables, degrees and the layouts
//! that place them. Pure code, no drawing, so the typed spec, the canvas editor and the figure
//! agent share one reading.
//!
//! Graph spec, one item per line (`#` starts a comment, `@x,y` places a node in grid units):
//! ```text
//! nodes A, B, C, D
//! node E @ 3,1
//! A - B
//! B -> C : 4
//! ```
use std::collections::{HashMap, VecDeque};

pub const MAX_NODES: usize = 40;
pub const MAX_EDGES: usize = 120;
pub const MAX_SPEC: usize = 4000;
pub const MAX_NAME: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    /// Where the spec placed it (grid units); None means the automatic layout.
    pub pos: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub directed: bool,
    pub weight: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

fn valid_name(s: &str) -> Result<(), String> {
    if s.is_empty() || !s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(format!("\u{201c}{s}\u{201d} is not a usable name (letters, digits and _)"));
    }
    if s.chars().count() > MAX_NAME {
        return Err(format!("\u{201c}{s}\u{201d} is longer than {MAX_NAME} characters"));
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

fn fmt_num(v: f64) -> String {
    crate::figures::data::fmt_num(v)
}

impl Graph {
    pub fn index(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    fn add_node(&mut self, name: &str) -> Result<usize, String> {
        if let Some(i) = self.index(name) {
            return Ok(i);
        }
        valid_name(name)?;
        if self.nodes.len() >= MAX_NODES {
            return Err(format!("More than {MAX_NODES} nodes"));
        }
        self.nodes.push(Node { name: name.to_string(), pos: None });
        Ok(self.nodes.len() - 1)
    }

    /// Reads the spec. Errors say which line and why.
    pub fn parse(spec: &str) -> Result<Graph, String> {
        if spec.len() > MAX_SPEC {
            return Err(format!("The spec is longer than {MAX_SPEC} characters"));
        }
        let mut g = Graph::default();
        for (ln, raw) in spec.lines().enumerate() {
            let at = |m: String| format!("Line {}: {m}", ln + 1);
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (body, pos) = match line.rsplit_once('@') {
                Some((a, p)) => (a.trim(), Some(parse_pos(p).map_err(&at)?)),
                None => (line, None),
            };
            let lower = body.to_ascii_lowercase();
            if let Some(rest) = ["nodes ", "node "].iter().find_map(|p| lower.starts_with(p).then(|| &body[p.len()..])) {
                let names: Vec<&str> = rest.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
                if pos.is_some() && names.len() != 1 {
                    return Err(at("only a single node can be placed: node A @ 1,2".into()));
                }
                for n in names {
                    let i = g.add_node(n).map_err(&at)?;
                    if pos.is_some() {
                        g.nodes[i].pos = pos;
                    }
                }
                continue;
            }
            if pos.is_some() {
                return Err(at("only a node line can have a position".into()));
            }
            // an edge: A - B, A -> B, optionally ": weight"
            let (ends, weight) = match body.split_once(':') {
                Some((e, w)) => {
                    let w = w.trim();
                    (e.trim(), Some(w.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1e9).ok_or_else(|| at(format!("\u{201c}{w}\u{201d} is not a usable weight")))?))
                }
                None => (body, None),
            };
            let (l, r, directed) = match ends.find("->") {
                Some(k) => (&ends[..k], &ends[k + 2..], true),
                None => match ends.find('-') {
                    Some(k) => (&ends[..k], &ends[k + 1..], false),
                    None => return Err(at("expected \u{201c}nodes A, B\u{201d}, \u{201c}A - B\u{201d} or \u{201c}A -> B : 3\u{201d}".into())),
                },
            };
            let (l, r) = (l.trim(), r.trim());
            let (a, b) = (g.add_node(l).map_err(&at)?, g.add_node(r).map_err(&at)?);
            if a == b {
                return Err(at(format!("{l} cannot connect to itself")));
            }
            if g.edges.iter().any(|e| e.directed == directed && ((e.a, e.b) == (a, b) || (!directed && (e.a, e.b) == (b, a)))) {
                return Err(at(format!("{l} and {r} are already connected")));
            }
            if g.edges.len() >= MAX_EDGES {
                return Err(at(format!("More than {MAX_EDGES} edges")));
            }
            g.edges.push(Edge { a, b, directed, weight });
        }
        Ok(g)
    }

    /// The spec text for this graph (parses back to an equal graph).
    pub fn to_spec(&self) -> String {
        let pos = |p: Option<(f64, f64)>| p.map_or(String::new(), |(x, y)| format!(" @ {},{}", fmt_num(x), fmt_num(y)));
        let mut s = String::new();
        // every node in its declared order: runs of unplaced nodes share a line
        let mut run: Vec<&str> = Vec::new();
        for n in &self.nodes {
            match n.pos {
                None => run.push(n.name.as_str()),
                Some(_) => {
                    if !run.is_empty() {
                        s += &format!("nodes {}\n", run.join(", "));
                        run.clear();
                    }
                    s += &format!("node {}{}\n", n.name, pos(n.pos));
                }
            }
        }
        if !run.is_empty() {
            s += &format!("nodes {}\n", run.join(", "));
        }
        for e in &self.edges {
            s += &format!("{} {} {}{}\n", self.nodes[e.a].name, if e.directed { "->" } else { "-" }, self.nodes[e.b].name, e.weight.map_or(String::new(), |w| format!(" : {}", fmt_num(w))));
        }
        s
    }

    /// Nodes reachable in one step from `i`, in node order.
    pub fn neighbours(&self, i: usize) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .edges
            .iter()
            .filter_map(|e| if e.a == i { Some(e.b) } else if !e.directed && e.b == i { Some(e.a) } else { None })
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Breadth-first visiting order from `start` (neighbours taken in node order).
    pub fn bfs(&self, start: usize) -> Vec<usize> {
        if start >= self.nodes.len() {
            return Vec::new();
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut q = VecDeque::from([start]);
        seen[start] = true;
        let mut out = Vec::new();
        while let Some(n) = q.pop_front() {
            out.push(n);
            for m in self.neighbours(n) {
                if !seen[m] {
                    seen[m] = true;
                    q.push_back(m);
                }
            }
        }
        out
    }

    /// Depth-first (pre-order) visiting order from `start`, neighbours taken in node order.
    pub fn dfs(&self, start: usize) -> Vec<usize> {
        if start >= self.nodes.len() {
            return Vec::new();
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut out = Vec::new();
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            if seen[n] {
                continue;
            }
            seen[n] = true;
            out.push(n);
            for m in self.neighbours(n).into_iter().rev() {
                if !seen[m] {
                    stack.push(m);
                }
            }
        }
        out
    }

    /// (in, out) degree of every node; an undirected edge counts once for each end, as both.
    pub fn degrees(&self) -> Vec<(usize, usize)> {
        let mut d = vec![(0, 0); self.nodes.len()];
        for e in &self.edges {
            d[e.a].1 += 1;
            d[e.b].0 += 1;
            if !e.directed {
                d[e.a].0 += 1;
                d[e.b].1 += 1;
            }
        }
        d
    }

    /// Adjacency matrix: the weight (1 when unweighted) of the edge from row to column, None if no edge.
    pub fn matrix(&self) -> Vec<Vec<Option<f64>>> {
        let n = self.nodes.len();
        let mut m = vec![vec![None; n]; n];
        for e in &self.edges {
            let w = e.weight.unwrap_or(1.0);
            m[e.a][e.b] = Some(w);
            if !e.directed {
                m[e.b][e.a] = Some(w);
            }
        }
        m
    }

    /// Whether every node can reach every other (following edges both ways).
    pub fn connected(&self) -> bool {
        if self.nodes.is_empty() {
            return true;
        }
        let mut und = self.clone();
        for e in &mut und.edges {
            e.directed = false;
        }
        und.bfs(0).len() == self.nodes.len()
    }

    /// The cheapest path from `a` to `b` by edge weights (1 for unweighted), as node indices and total.
    /// Negative weights are not supported (None).
    pub fn shortest_path(&self, a: usize, b: usize) -> Option<(Vec<usize>, f64)> {
        let n = self.nodes.len();
        if a >= n || b >= n || self.edges.iter().any(|e| e.weight.is_some_and(|w| w < 0.0)) {
            return None;
        }
        let mut dist = vec![f64::INFINITY; n];
        let mut prev = vec![usize::MAX; n];
        let mut done = vec![false; n];
        dist[a] = 0.0;
        for _ in 0..n {
            let Some(u) = (0..n).filter(|i| !done[*i] && dist[*i].is_finite()).min_by(|x, y| dist[*x].total_cmp(&dist[*y])) else { break };
            done[u] = true;
            for e in &self.edges {
                let w = e.weight.unwrap_or(1.0);
                let next = if e.a == u { Some(e.b) } else if !e.directed && e.b == u { Some(e.a) } else { None };
                if let Some(v) = next.filter(|v| dist[u] + w < dist[*v]) {
                    dist[v] = dist[u] + w;
                    prev[v] = u;
                }
            }
        }
        if !dist[b].is_finite() {
            return None;
        }
        let mut path = vec![b];
        while *path.last().unwrap() != a {
            path.push(prev[*path.last().unwrap()]);
        }
        path.reverse();
        Some((path, dist[b]))
    }

    /// Node positions in grid units for a layout: `Circle` spaces them round a ring; `Layers`
    /// is a tree-style drawing from node 0 (breadth-first levels downward, a node under the
    /// middle of the ones it leads to), unreached nodes starting further trees to the right.
    /// A position written in the spec always wins.
    pub fn layout(&self, mode: Layout) -> Vec<(f64, f64)> {
        let n = self.nodes.len();
        let auto: Vec<(f64, f64)> = match mode {
            Layout::Circle => {
                let r = (n as f64 * 0.42).max(1.0);
                (0..n).map(|i| {
                    let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / n.max(1) as f64;
                    (r + r * a.cos(), r + r * a.sin())
                }).collect()
            }
            Layout::Layers => self.layers(),
        };
        (0..n).map(|i| self.nodes[i].pos.unwrap_or(auto[i])).collect()
    }

    fn layers(&self) -> Vec<(f64, f64)> {
        let n = self.nodes.len();
        let mut parent = vec![usize::MAX; n];
        let mut depth = vec![0usize; n];
        let mut seen = vec![false; n];
        let mut kids: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut roots = Vec::new();
        for r in 0..n {
            if seen[r] {
                continue;
            }
            roots.push(r);
            seen[r] = true;
            let mut q = VecDeque::from([r]);
            while let Some(u) = q.pop_front() {
                // an undirected tree edge can be walked from either end
                for m in self.neighbours(u) {
                    if !seen[m] {
                        seen[m] = true;
                        parent[m] = u;
                        depth[m] = depth[u] + 1;
                        kids[u].push(m);
                        q.push_back(m);
                    }
                }
            }
        }
        let mut x = vec![0.0f64; n];
        let mut next = 0.0;
        fn place(u: usize, kids: &[Vec<usize>], x: &mut [f64], next: &mut f64) {
            if kids[u].is_empty() {
                x[u] = *next;
                *next += 1.0;
            } else {
                for k in &kids[u] {
                    place(*k, kids, x, next);
                }
                x[u] = (x[kids[u][0]] + x[*kids[u].last().unwrap()]) / 2.0;
            }
        }
        for r in roots {
            place(r, &kids, &mut x, &mut next);
        }
        let _ = parent;
        (0..n).map(|i| (x[i], depth[i] as f64)).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Circle,
    Layers,
}

// ---------------------------------------------------------------------------------------------

/// A rooted tree (node 0 is the root): a binary search tree grown from values, or any tree
/// written as parent lines.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Tree {
    pub labels: Vec<String>,
    /// Children of each node in order. In a binary tree (`binary`) the slots are [left, right]
    /// and a missing one is None, so a lone child sits on its own side.
    pub kids: Vec<Vec<Option<usize>>>,
    pub binary: bool,
}

impl Tree {
    /// A binary search tree from values, inserted in order (a repeat is ignored).
    pub fn bst(values: &[f64]) -> Tree {
        let mut t = Tree { binary: true, ..Default::default() };
        let mut vals: Vec<f64> = Vec::new();
        for v in values.iter().copied().filter(|v| v.is_finite()).take(MAX_NODES) {
            if vals.is_empty() {
                vals.push(v);
                t.labels.push(fmt_num(v));
                t.kids.push(vec![]);
                continue;
            }
            let mut cur = 0;
            loop {
                if v == vals[cur] {
                    break;
                }
                let side = usize::from(v > vals[cur]);
                if t.kids[cur].is_empty() {
                    t.kids[cur] = vec![None, None];
                }
                match t.kids[cur][side] {
                    Some(c) => cur = c,
                    None => {
                        vals.push(v);
                        t.labels.push(fmt_num(v));
                        t.kids.push(vec![]);
                        let id = vals.len() - 1;
                        t.kids[cur][side] = Some(id);
                        break;
                    }
                }
            }
        }
        // a node with two empty slots is a leaf
        for k in &mut t.kids {
            if k.iter().all(Option::is_none) {
                k.clear();
            }
        }
        t
    }

    /// Reads parent lines: `A: B, C` (a `_` is an empty slot, which makes it a binary tree).
    /// The root is the first parent that is nobody's child.
    pub fn parse(spec: &str) -> Result<Tree, String> {
        if spec.len() > MAX_SPEC {
            return Err(format!("The spec is longer than {MAX_SPEC} characters"));
        }
        let mut t = Tree::default();
        let mut idx: HashMap<String, usize> = HashMap::new();
        let mut get = |t: &mut Tree, idx: &mut HashMap<String, usize>, name: &str| -> Result<usize, String> {
            if let Some(i) = idx.get(name) {
                return Ok(*i);
            }
            valid_name(name)?;
            if t.labels.len() >= MAX_NODES {
                return Err(format!("More than {MAX_NODES} nodes"));
            }
            t.labels.push(name.to_string());
            t.kids.push(vec![]);
            idx.insert(name.to_string(), t.labels.len() - 1);
            Ok(t.labels.len() - 1)
        };
        for (ln, raw) in spec.lines().enumerate() {
            let at = |m: String| format!("Line {}: {m}", ln + 1);
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (p, rest) = line.split_once(':').ok_or_else(|| at("expected \u{201c}parent: child, child\u{201d}".into()))?;
            let pi = get(&mut t, &mut idx, p.trim()).map_err(&at)?;
            let items: Vec<Option<String>> = rest.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| if s == "_" { None } else { Some(s.to_string()) }).collect();
            if items.is_empty() {
                return Err(at(format!("{} has no children listed", p.trim())));
            }
            let mut ids = Vec::new();
            for it in &items {
                ids.push(match it {
                    None => None,
                    Some(c) => Some(get(&mut t, &mut idx, c).map_err(&at)?),
                });
            }
            if !t.kids[pi].is_empty() {
                return Err(at(format!("{} already has its children listed", p.trim())));
            }
            t.kids[pi] = ids;
        }
        if t.labels.is_empty() {
            return Ok(t);
        }
        t.binary = t.kids.iter().any(|k| k.iter().any(Option::is_none));
        if t.binary && t.kids.iter().any(|k| k.len() > 2) {
            return Err("A tree using _ for empty slots is binary: at most two children each".into());
        }
        // one parent each, no cycles, one root
        let mut parent = vec![usize::MAX; t.labels.len()];
        for (i, k) in t.kids.iter().enumerate() {
            for c in k.iter().flatten() {
                if parent[*c] != usize::MAX {
                    return Err(format!("{} has two parents ({} and {})", t.labels[*c], t.labels[parent[*c]], t.labels[i]));
                }
                parent[*c] = i;
            }
        }
        let roots: Vec<usize> = (0..t.labels.len()).filter(|i| parent[*i] == usize::MAX).collect();
        if roots.len() != 1 {
            return Err(if roots.is_empty() { "The tree has a loop: no node is the root".into() } else { format!("A tree has one root, but {} have no parent", roots.iter().map(|r| t.labels[*r].clone()).collect::<Vec<_>>().join(", ")) });
        }
        // make the root node 0
        let r = roots[0];
        if r != 0 {
            let n = t.labels.len();
            let map: Vec<usize> = (0..n).map(|i| if i == r { 0 } else if i < r { i + 1 } else { i }).collect();
            let mut labels = vec![String::new(); n];
            let mut kids = vec![Vec::new(); n];
            for i in 0..n {
                labels[map[i]] = t.labels[i].clone();
                kids[map[i]] = t.kids[i].iter().map(|c| c.map(|c| map[c])).collect();
            }
            t.labels = labels;
            t.kids = kids;
        }
        // every node reachable from the root (a loop away from it would leave some out)
        if t.preorder().len() != t.labels.len() {
            return Err("Part of the tree is a loop that does not hang from the root".into());
        }
        Ok(t)
    }

    pub fn to_spec(&self) -> String {
        let mut s = String::new();
        for (i, k) in self.kids.iter().enumerate() {
            if !k.is_empty() {
                s += &format!("{}: {}\n", self.labels[i], k.iter().map(|c| c.map_or("_".to_string(), |c| self.labels[c].clone())).collect::<Vec<_>>().join(", "));
            }
        }
        s
    }

    fn children(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        self.kids[i].iter().flatten().copied()
    }

    pub fn preorder(&self) -> Vec<usize> {
        if self.labels.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut stack = vec![0];
        while let Some(n) = stack.pop() {
            if out.contains(&n) {
                continue;
            }
            out.push(n);
            let kids: Vec<usize> = self.children(n).collect();
            stack.extend(kids.into_iter().rev());
        }
        out
    }

    pub fn postorder(&self) -> Vec<usize> {
        fn go(t: &Tree, n: usize, out: &mut Vec<usize>, depth: usize) {
            if depth > 200 {
                return;
            }
            for c in t.children(n) {
                go(t, c, out, depth + 1);
            }
            out.push(n);
        }
        let mut out = Vec::new();
        if !self.labels.is_empty() {
            go(self, 0, &mut out, 0);
        }
        out
    }

    /// Left, node, right: only for a binary tree (None otherwise).
    pub fn inorder(&self) -> Option<Vec<usize>> {
        if !self.binary && self.kids.iter().any(|k| k.len() > 2) {
            return None;
        }
        fn go(t: &Tree, n: usize, out: &mut Vec<usize>, depth: usize) {
            if depth > 200 {
                return;
            }
            let k = &t.kids[n];
            if let Some(Some(l)) = k.first() {
                go(t, *l, out, depth + 1);
            }
            out.push(n);
            if let Some(Some(r)) = k.get(1) {
                go(t, *r, out, depth + 1);
            }
        }
        let mut out = Vec::new();
        if !self.labels.is_empty() {
            go(self, 0, &mut out, 0);
        }
        Some(out)
    }

    pub fn level_order(&self) -> Vec<usize> {
        if self.labels.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut q = VecDeque::from([0]);
        while let Some(n) = q.pop_front() {
            if out.contains(&n) {
                continue;
            }
            out.push(n);
            q.extend(self.children(n));
        }
        out
    }

    /// Depth of every node (the root is 0).
    pub fn depths(&self) -> Vec<usize> {
        let mut d = vec![0; self.labels.len()];
        for n in self.level_order() {
            for c in self.children(n) {
                d[c] = d[n] + 1;
            }
        }
        d
    }

    /// Height: the number of edges on the longest path from the root down.
    pub fn height(&self) -> usize {
        self.depths().into_iter().max().unwrap_or(0)
    }

    /// Positions in grid units: depth down, leaves across (an empty slot of a binary tree
    /// takes a place so a lone child sits to its own side), each parent centred over its children.
    pub fn layout(&self) -> Vec<(f64, f64)> {
        let n = self.labels.len();
        let mut x = vec![0.0f64; n];
        let mut next = 0.0;
        fn place(t: &Tree, u: usize, x: &mut [f64], next: &mut f64, depth: usize) {
            if depth > 200 {
                return;
            }
            if t.kids[u].is_empty() {
                x[u] = *next;
                *next += 1.0;
                return;
            }
            let (mut first, mut last) = (None, 0.0);
            for slot in &t.kids[u] {
                let cx = match slot {
                    Some(c) => {
                        place(t, *c, x, next, depth + 1);
                        x[*c]
                    }
                    None => {
                        *next += 1.0;
                        *next - 1.0
                    }
                };
                first.get_or_insert(cx);
                last = cx;
            }
            x[u] = (first.unwrap_or(0.0) + last) / 2.0;
        }
        if n > 0 {
            place(self, 0, &mut x, &mut next, 0);
        }
        let d = self.depths();
        (0..n).map(|i| (x[i], d[i] as f64)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: &str = "nodes A, B, C, D, E\nA - B\nA - C\nB - D\nC - D\nD -> E : 2.5\n";

    fn names(g: &Graph, v: &[usize]) -> String { v.iter().map(|i| g.nodes[*i].name.as_str()).collect::<Vec<_>>().join("") }

    #[test]
    fn parse_and_round_trip() {
        let g = Graph::parse(G).unwrap();
        assert_eq!(g.nodes.len(), 5);
        assert_eq!(g.edges.len(), 5);
        assert_eq!(g.edges[4], Edge { a: 3, b: 4, directed: true, weight: Some(2.5) });
        assert_eq!(Graph::parse(&g.to_spec()).unwrap(), g);
        let placed = Graph::parse("node A @ 1.5,2\nA - B").unwrap();
        assert_eq!(placed.nodes[0].pos, Some((1.5, 2.0)));
        assert_eq!(Graph::parse(&placed.to_spec()).unwrap(), placed, "the order of the nodes survives");
        let mixed = Graph::parse("nodes A, B\nnode C @ 1,1\nnodes D").unwrap();
        assert_eq!(Graph::parse(&mixed.to_spec()).unwrap(), mixed);
    }

    #[test]
    fn edges_name_their_own_nodes_and_comments_are_ignored() {
        let g = Graph::parse("# a path\nX - Y  # first\nY - 7").unwrap();
        assert_eq!(g.nodes.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["X", "Y", "7"]);
    }

    #[test]
    fn malformed_specs_say_where_and_why() {
        for (spec, needle) in [
            ("A - A", "itself"),
            ("A - B\nB - A", "already connected"),
            ("A -> B\nA -> B", "already connected"),
            ("A - B : x", "weight"),
            ("A - B : 1e99", "weight"),
            ("nonsense", "Line 1"),
            ("A - B @ 1,1", "only a node line"),
            ("nodes A, B @ 1,1", "single node"),
            ("node A @ x", "@x,y"),
            ("node a!", "usable name"),
            ("node AAAAAAAAAAAAAAAAAAAA", "longer than"),
        ] {
            let e = Graph::parse(spec).unwrap_err();
            assert!(e.contains(needle), "{spec:?} -> {e}");
        }
        // a directed edge each way is two different edges
        assert!(Graph::parse("A -> B\nB -> A").is_ok());
    }

    #[test]
    fn size_caps_hold() {
        let big: String = (0..MAX_NODES + 1).map(|i| format!("node n{i}\n")).collect();
        assert!(Graph::parse(&big).unwrap_err().contains("More than"));
        let many: String = (0..MAX_EDGES + 1).map(|i| format!("a{} - b{}\n", i % 20, (i / 20) + 20)).collect();
        assert!(Graph::parse(&many).is_err());
        assert!(Graph::parse(&"#".repeat(MAX_SPEC + 1)).is_err());
        assert_eq!(Graph::parse("").unwrap(), Graph::default());
    }

    #[test]
    fn traversals_take_neighbours_in_node_order() {
        let g = Graph::parse(G).unwrap();
        assert_eq!(names(&g, &g.bfs(0)), "ABCDE");
        assert_eq!(names(&g, &g.dfs(0)), "ABDCE", "B, then D, whose neighbours are B, C, E in order");
        assert_eq!(names(&g, &g.dfs(2)), "CABDE");
        assert!(g.bfs(99).is_empty());
        // directed edges are one way
        let d = Graph::parse("A -> B\nC -> B").unwrap();
        assert_eq!(names(&d, &d.bfs(0)), "AB");
        assert_eq!(names(&d, &d.bfs(1)), "B");
    }

    #[test]
    fn degrees_matrix_and_connectivity() {
        let g = Graph::parse(G).unwrap();
        assert_eq!(g.degrees(), vec![(2, 2), (2, 2), (2, 2), (2, 3), (1, 0)]);
        let m = g.matrix();
        assert_eq!(m[0][1], Some(1.0));
        assert_eq!(m[1][0], Some(1.0));
        assert_eq!(m[3][4], Some(2.5));
        assert_eq!(m[4][3], None, "the directed edge is one way");
        assert!(g.connected());
        assert!(!Graph::parse("A - B\nnodes C").unwrap().connected());
        assert!(Graph::default().connected());
    }

    #[test]
    fn shortest_path_by_weight() {
        let g = Graph::parse("A - B : 1\nB - C : 1\nA - C : 5\nC -> D : 2").unwrap();
        let (p, d) = g.shortest_path(0, 3).unwrap();
        assert_eq!((names(&g, &p), d), ("ABCD".to_string(), 4.0));
        assert!(g.shortest_path(3, 0).is_none(), "D cannot reach back");
        assert!(g.shortest_path(0, 99).is_none());
        let neg = Graph::parse("A - B : -1").unwrap();
        assert!(neg.shortest_path(0, 1).is_none());
    }

    #[test]
    fn layouts_place_every_node_and_the_spec_wins() {
        let g = Graph::parse(G).unwrap();
        for mode in [Layout::Circle, Layout::Layers] {
            let l = g.layout(mode);
            assert_eq!(l.len(), 5);
            assert!(l.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
        }
        let l = g.layout(Layout::Layers);
        assert_eq!(l[0].1, 0.0, "the first node is the root of the layers");
        assert!(l[1].1 == 1.0 && l[2].1 == 1.0 && l[3].1 == 2.0);
        let two = Graph::parse("A - B\nC - D").unwrap().layout(Layout::Layers);
        assert!(two[2].0 > two[1].0, "a second component starts to the right");
        let placed = Graph::parse("node A @ 9,9\nA - B").unwrap();
        assert_eq!(placed.layout(Layout::Circle)[0], (9.0, 9.0));
        assert!(Graph::default().layout(Layout::Circle).is_empty());
    }

    #[test]
    fn bst_grows_by_insertion_and_ignores_repeats() {
        let t = Tree::bst(&[8.0, 3.0, 10.0, 1.0, 6.0, 14.0, 4.0, 7.0, 13.0, 6.0]);
        assert_eq!(t.labels.len(), 9);
        let l = |v: &[usize]| v.iter().map(|i| t.labels[*i].clone()).collect::<Vec<_>>().join(" ");
        assert_eq!(l(&t.inorder().unwrap()), "1 3 4 6 7 8 10 13 14", "in-order of a BST is sorted");
        assert_eq!(l(&t.preorder()), "8 3 1 6 4 7 10 14 13");
        assert_eq!(l(&t.postorder()), "1 4 7 6 3 13 14 10 8");
        assert_eq!(l(&t.level_order()), "8 3 10 1 6 14 4 7 13");
        assert_eq!(t.height(), 3);
        let again = Tree::parse(&t.to_spec()).unwrap();
        assert_eq!(again.to_spec(), t.to_spec());
        assert_eq!(again.inorder().map(|v| v.iter().map(|i| again.labels[*i].clone()).collect::<Vec<_>>()), t.inorder().map(|v| v.iter().map(|i| t.labels[*i].clone()).collect::<Vec<_>>()));
        assert!(Tree::bst(&[]).labels.is_empty());
        assert_eq!(Tree::bst(&[f64::NAN, 5.0]).labels, vec!["5"]);
    }

    #[test]
    fn tree_spec_round_trips_and_finds_the_root() {
        let t = Tree::parse("B: D, E\nA: B, C\nC: F").unwrap();
        assert_eq!(t.labels[0], "A", "the root is node 0");
        assert!(!t.binary);
        assert_eq!(Tree::parse(&t.to_spec()).unwrap().to_spec(), t.to_spec());
        assert_eq!(t.preorder().iter().map(|i| t.labels[*i].as_str()).collect::<String>(), "ABDECF");
        assert_eq!(t.level_order().iter().map(|i| t.labels[*i].as_str()).collect::<String>(), "ABCDEF");
        let b = Tree::parse("A: _, B\nB: C, _").unwrap();
        assert!(b.binary);
        assert_eq!(b.inorder().unwrap().iter().map(|i| b.labels[*i].as_str()).collect::<String>(), "ACB");
        assert_eq!(Tree::parse(&b.to_spec()).unwrap(), b);
        assert!(Tree::parse("A: B, C, D").unwrap().inorder().is_none());
    }

    #[test]
    fn bad_trees_are_explained() {
        for (spec, needle) in [
            ("A: B\nC: B", "two parents"),
            ("A: B\nB: A", "loop"),
            ("A: B\nC: D", "one root"),
            ("A:", "no children"),
            ("A B", "parent: child"),
            ("A: B\nA: C", "already has"),
            ("A: _, B, C", "binary"),
            ("A: B\nB: C\nC: B", "two parents"),
            ("R: A\nA: B\nB: C\nC: A", "two parents"),
        ] {
            let e = Tree::parse(spec).unwrap_err();
            assert!(e.contains(needle), "{spec:?} -> {e}");
        }
        assert!(Tree::parse("").unwrap().labels.is_empty());
    }

    #[test]
    fn tree_layout_centres_parents_and_gives_a_lone_child_its_side() {
        let t = Tree::bst(&[4.0, 2.0, 6.0]);
        let l = t.layout();
        assert_eq!(l[0], (0.5, 0.0), "the root is over its two children");
        assert!(l[1].0 < l[2].0);
        let lone = Tree::bst(&[4.0, 6.0]).layout();
        assert!(lone[1].0 > lone[0].0, "a right child sits to the right: {lone:?}");
        let left = Tree::bst(&[4.0, 2.0]).layout();
        assert!(left[1].0 < left[0].0);
        let wide = Tree::parse("A: B, C, D\nB: E, F").unwrap().layout();
        assert!(wide.iter().all(|p| p.0.is_finite()));
        let mut xs: Vec<_> = wide.iter().map(|p| (p.0 * 100.0) as i64 + (p.1 * 100000.0) as i64).collect();
        xs.sort();
        xs.dedup();
        assert_eq!(xs.len(), 6, "no two nodes share a spot");
    }

    #[test]
    fn a_long_chain_does_not_overflow_the_stack() {
        let spec: String = (0..39).map(|i| format!("n{i}: n{}\n", i + 1)).collect();
        let t = Tree::parse(&spec).unwrap();
        assert_eq!(t.height(), 39);
        assert_eq!(t.layout().len(), 40);
        let vals: Vec<f64> = (0..300).map(|i| i as f64).collect();
        assert_eq!(Tree::bst(&vals).labels.len(), MAX_NODES);
    }
}
