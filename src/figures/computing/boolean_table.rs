//! Truth table and Karnaugh map for a boolean function, given as an expression or as a list of
//! minterms (with optional don't-cares). The simplified sum of products is worked out with
//! Quine-McCluskey and its groups are drawn on the map. The student copy (Show answers off) keeps
//! the inputs and the empty cells and drops the outputs, the groups and the simplified form.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::logic::Circuit;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_VARS: usize = 5;
const MAX_KMAP_VARS: usize = 4;
const SHOWS: [&str; 3] = ["Truth table", "Karnaugh map", "Both"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "boolean-table",
        subject: Subject::Computing,
        title: "Truth table and Karnaugh map",
        blurb: "A truth table and Karnaugh map for a boolean function given as an expression or as minterms, with the simplified sum of products and its groups worked out, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("boolean-table", "expr") => p.s("source") == "Expression",
        ("boolean-table", "vars" | "minterms" | "dontcares") => p.s("source") != "Expression",
        ("boolean-table", "output") => p.s("source") == "Expression",
        ("boolean-table", "groups") => p.s("show") != "Truth table",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "boolean-table" => vec![
            Example { name: "Expression to table and map", params: vec![] },
            Example {
                name: "Four-variable minterms",
                params: vec![("source", s("Minterms")), ("vars", s("A, B, C, D")), ("minterms", s("0, 2, 5, 7, 8, 10, 13, 15"))],
            },
            Example {
                name: "With don't-cares",
                params: vec![("source", s("Minterms")), ("vars", s("A, B, C, D")), ("minterms", s("1, 3, 7, 11, 15")), ("dontcares", s("0, 2, 5"))],
            },
            Example { name: "Truth table only", params: vec![("expr", s("S = A XOR B XOR C\nCout = (A AND B) OR (C AND (A XOR B))")), ("show", s("Truth table"))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("source", "Function given as", &["Expression", "Minterms"], "Expression"),
        ParamDef::long("expr", "Expression", "Q = (A AND B) OR (NOT A AND C)").hint("One per line, such as S = A XOR B. Words AND OR XOR NOT, or the symbols & | ^ !"),
        ParamDef::text("vars", "Variables", "A, B, C").hint("Up to five names, most significant first"),
        ParamDef::text("minterms", "Minterms (output 1)", "1, 3, 6, 7").hint("Row numbers, counting the first variable as the top bit"),
        ParamDef::text("dontcares", "Don't-cares", "").hint("Row numbers where the output does not matter (shown as X)"),
        ParamDef::int("output", "Output for the map", 1, 8, 1).hint("Which output of an expression with several the Karnaugh map shows"),
        ParamDef::choice("show", "Show", &SHOWS, "Both"),
        ParamDef::bool("row_numbers", "Number the rows", false),
        ParamDef::bool("groups", "Draw the groups on the map", true),
        ParamDef::bool("simplify", "Show the simplified expression", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the inputs and leaves the outputs, the map's cells, the groups and the simplified form empty for students"),
    ]
}

struct Func {
    vars: Vec<String>,
    /// Output name and, per row (first variable the top bit), Some(value) or None for a don't-care.
    outs: Vec<(String, Vec<Option<bool>>)>,
}

fn list(s: &str) -> Vec<&str> {
    s.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).collect()
}

fn numbers(s: &str, limit: usize, what: &str) -> Result<Vec<usize>, String> {
    let mut v = Vec::new();
    for t in list(s).into_iter().take(64) {
        let n: usize = t.trim_start_matches('m').parse().map_err(|_| format!("{what}: \u{201c}{t}\u{201d} is not a row number"))?;
        if n >= limit {
            return Err(format!("{what}: row {n} does not exist (rows run 0 to {})", limit - 1));
        }
        v.push(n);
    }
    Ok(v)
}

fn build(p: &Params) -> Result<Func, String> {
    if p.s("source") == "Expression" {
        let c = Circuit::from_exprs(p.s("expr"))?;
        let vars: Vec<String> = c.inputs().into_iter().map(|i| c.nodes[i].name.clone()).collect();
        if vars.is_empty() {
            return Err("The expression has no variables".into());
        }
        let table = c.truth_table()?;
        let outs = c.outputs.iter().enumerate().map(|(k, o)| (o.name.clone(), table.iter().map(|(_, r)| Some(r[k])).collect())).collect();
        return Ok(Func { vars, outs });
    }
    let vars: Vec<String> = list(p.s("vars")).into_iter().map(String::from).collect();
    if vars.is_empty() || vars.len() > MAX_VARS {
        return Err(format!("Give between 1 and {MAX_VARS} variable names"));
    }
    for (i, v) in vars.iter().enumerate() {
        if v.chars().count() > 8 || !v.chars().next().is_some_and(char::is_alphabetic) || !v.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(format!("\u{201c}{v}\u{201d} is not a usable variable name"));
        }
        if vars[..i].contains(v) {
            return Err(format!("{v} is listed twice"));
        }
    }
    let rows = 1usize << vars.len();
    let ones = numbers(p.s("minterms"), rows, "Minterms")?;
    let dcs = numbers(p.s("dontcares"), rows, "Don't-cares")?;
    if let Some(m) = ones.iter().find(|m| dcs.contains(m)) {
        return Err(format!("Row {m} is both a minterm and a don't-care"));
    }
    let col = (0..rows).map(|r| if dcs.contains(&r) { None } else { Some(ones.contains(&r)) }).collect();
    Ok(Func { vars, outs: vec![("F".into(), col)] })
}

/// Prime implicants as (value, dashes): a dash bit means the variable is left out.
fn primes(n: usize, ones: &[usize]) -> Vec<(u32, u32)> {
    let _ = n;
    let mut cur: Vec<(u32, u32)> = ones.iter().map(|m| (*m as u32, 0)).collect();
    cur.sort_unstable();
    cur.dedup();
    let mut out = Vec::new();
    while !cur.is_empty() {
        let mut used = vec![false; cur.len()];
        let mut next: Vec<(u32, u32)> = Vec::new();
        for i in 0..cur.len() {
            for j in i + 1..cur.len() {
                let (a, b) = (cur[i], cur[j]);
                if a.1 == b.1 && (a.0 ^ b.0).count_ones() == 1 {
                    used[i] = true;
                    used[j] = true;
                    next.push((a.0 & b.0, a.1 | (a.0 ^ b.0)));
                }
            }
        }
        for (i, t) in cur.iter().enumerate() {
            if !used[i] {
                out.push(*t);
            }
        }
        next.sort_unstable();
        next.dedup();
        cur = next;
    }
    out.sort_unstable();
    out
}

fn covers(t: (u32, u32), m: usize) -> bool {
    (m as u32 & !t.1) == t.0
}

/// A smallest sum of products: the essential primes, then the fewest further primes that cover
/// the rest (an exact search), preferring fewer literals when several covers are as small.
/// `ones` includes the don't-cares (they may be grouped); `must` are the rows that need covering.
fn minimize(n: usize, ones: &[usize], must: &[usize]) -> Vec<(u32, u32)> {
    let ps = primes(n, ones);
    let mut chosen: Vec<(u32, u32)> = Vec::new();
    for m in must {
        let c: Vec<&(u32, u32)> = ps.iter().filter(|t| covers(**t, *m)).collect();
        if c.len() == 1 && !chosen.contains(c[0]) {
            chosen.push(*c[0]);
        }
    }
    let left: Vec<usize> = must.iter().copied().filter(|m| !chosen.iter().any(|t| covers(*t, *m))).collect();
    let lits = |t: &(u32, u32)| n as u32 - t.1.count_ones();
    // cost: number of terms, then literals
    fn search(ps: &[(u32, u32)], left: &[usize], picked: &mut Vec<(u32, u32)>, best: &mut Option<(usize, u32, Vec<(u32, u32)>)>, lits: &dyn Fn(&(u32, u32)) -> u32, budget: &mut u32) {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let cost = (picked.len(), picked.iter().map(lits).sum::<u32>());
        if best.as_ref().is_some_and(|b| (b.0, b.1) <= cost) {
            return;
        }
        let Some(m) = left.first() else {
            let mut v = picked.clone();
            v.sort_unstable();
            if best.as_ref().is_none_or(|b| cost < (b.0, b.1) || (cost == (b.0, b.1) && v < b.2)) {
                *best = Some((cost.0, cost.1, v));
            }
            return;
        };
        for t in ps.iter().filter(|t| covers(**t, *m)) {
            let rest: Vec<usize> = left.iter().copied().filter(|x| !covers(*t, *x)).collect();
            picked.push(*t);
            search(ps, &rest, picked, best, lits, budget);
            picked.pop();
        }
    }
    let mut best = None;
    let mut budget = 200_000u32;
    search(&ps, &left, &mut Vec::new(), &mut best, &lits, &mut budget);
    match best {
        Some((_, _, v)) => chosen.extend(v),
        None => {
            // search budget spent without a cover (cannot happen for small maps): greedy fallback
            let mut left = left;
            while let Some(t) = ps.iter().filter(|t| !chosen.contains(t)).max_by_key(|t| left.iter().filter(|m| covers(**t, **m)).count()) {
                if !left.iter().any(|m| covers(*t, *m)) {
                    break;
                }
                chosen.push(*t);
                left.retain(|m| !covers(*t, *m));
            }
        }
    }
    chosen.sort_unstable();
    chosen.dedup();
    chosen
}

fn term_text(t: (u32, u32), vars: &[String]) -> String {
    let n = vars.len();
    let sep = if vars.iter().all(|v| v.chars().count() == 1) { "" } else { "\u{b7}" };
    let lits: Vec<String> = (0..n)
        .filter(|i| t.1 >> (n - 1 - i) & 1 == 0)
        .map(|i| if t.0 >> (n - 1 - i) & 1 == 1 { vars[i].clone() } else { format!("{}'", vars[i]) })
        .collect();
    if lits.is_empty() { "1".into() } else { lits.join(sep) }
}

fn sop(f: &Func, k: usize) -> (String, Vec<(u32, u32)>) {
    let n = f.vars.len();
    let col = &f.outs[k].1;
    let must: Vec<usize> = (0..col.len()).filter(|r| col[*r] == Some(true)).collect();
    let allowed: Vec<usize> = (0..col.len()).filter(|r| col[*r] != Some(false)).collect();
    if must.is_empty() {
        return ("0".into(), vec![]);
    }
    let t = minimize(n, &allowed, &must);
    (t.iter().map(|x| term_text(*x, &f.vars)).collect::<Vec<_>>().join(" + "), t)
}

fn gray(i: usize) -> usize {
    i ^ (i >> 1)
}

fn bits(v: usize, k: usize) -> String {
    (0..k).rev().map(|b| if v >> b & 1 == 1 { '1' } else { '0' }).collect()
}

fn val(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "1",
        Some(false) => "0",
        None => "X",
    }
}

fn render(p: &Params) -> String {
    let f = match build(p) {
        Ok(f) => f,
        Err(e) => return chart::message(&e),
    };
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 16.0 * k;
    let n = f.vars.len();
    let show = p.s("show");
    let want_table = show != "Karnaugh map";
    let kmap_ok = n >= 2 && n <= MAX_KMAP_VARS;
    let want_map = show != "Truth table" && kmap_ok;
    let out_k = (p.i("output").max(1) as usize - 1).min(f.outs.len() - 1);
    let rows = 1usize << n;
    let title = p.s("title").trim();
    let head = if print { "#e5e5e5" } else { "#eef1f6" };
    let ink_fill = if print { "#f3f3f3" } else { "#ffe9a8" };
    let row_h = fs * 1.9;
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 0.4 };

    // truth table geometry
    let num_w = if p.b("row_numbers") { fs * 2.4 } else { 0.0 };
    let in_w: Vec<f64> = f.vars.iter().map(|v| (v.chars().count() as f64 * fs * 0.7 + 22.0).max(46.0)).collect();
    let out_w: Vec<f64> = f.outs.iter().map(|(o, _)| (o.chars().count() as f64 * fs * 0.7 + 22.0).max(52.0)).collect();
    let table_w = num_w + in_w.iter().sum::<f64>() + out_w.iter().sum::<f64>();
    let table_h = row_h * (rows + 1) as f64;

    // Karnaugh map geometry
    let rb = n / 2;
    let cb = n - rb;
    let (nr, nc) = (1usize << rb.max(1), 1usize << cb);
    let cell = (fs * 2.9).max(48.0);
    let lab_w = fs * 0.75 * rb.max(1) as f64 + 40.0;
    let lab_h = fs * 2.4;
    let map_w = lab_w + nc as f64 * cell;
    let map_h = lab_h + nr as f64 * cell;
    let (sop_text, terms) = sop(&f, out_k);
    let sop_line = format!("{} = {}", f.outs[out_k].0, sop_text);
    let sop_on = p.b("simplify") && want_map;
    let sop_h = if sop_on { fs * 3.0 } else { 0.0 };
    let gap = 44.0;
    let body_w = (if want_table { table_w } else { 0.0 }) + if want_table && want_map { gap } else { 0.0 } + if want_map { map_w.max(if sop_on { gd::text_w(&sop_line, fs) + 8.0 } else { 0.0 }) } else { 0.0 };
    let w = (body_w + 48.0).max(300.0);
    let body_h = (if want_table { table_h } else { 0.0 }).max(if want_map { map_h + sop_h } else { 0.0 });
    let note = if show != "Truth table" && !kmap_ok { fs * 2.2 } else { 0.0 };
    let h = top + body_h + note + 18.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let x0 = (w - body_w) / 2.0;

    if want_table {
        let mut x = x0;
        let mut hdr: Vec<(f64, String, bool)> = Vec::new();
        if num_w > 0.0 {
            hdr.push((num_w, "#".into(), false));
        }
        hdr.extend(f.vars.iter().zip(&in_w).map(|(v, w)| (*w, v.clone(), false)));
        hdr.extend(f.outs.iter().zip(&out_w).map(|((o, _), w)| (*w, o.clone(), true)));
        for (cw, t, _) in &hdr {
            d.rect(x, top, *cw, row_h, head, INK, 1.2);
            gd::text_fill(&mut d, x + cw / 2.0, top + row_h / 2.0 + fs * 0.35, fs, INK, t);
            x += cw;
        }
        for r in 0..rows {
            let y = top + row_h * (r + 1) as f64;
            let mut x = x0;
            if num_w > 0.0 {
                d.rect(x, y, num_w, row_h, head, INK, 1.2);
                gd::text_fill(&mut d, x + num_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs * 0.85, INK, &r.to_string());
                x += num_w;
            }
            for (i, w) in in_w.iter().enumerate() {
                d.rect(x, y, *w, row_h, "#fff", INK, 1.2);
                gd::text_fill(&mut d, x + w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, if r >> (n - 1 - i) & 1 == 1 { "1" } else { "0" });
                x += w;
            }
            for (j, w) in out_w.iter().enumerate() {
                let v = f.outs[j].1[r];
                d.rect(x, y, *w, row_h, if answers && v == Some(true) { ink_fill } else { "#fff" }, INK, 1.2);
                if answers {
                    gd::text_fill(&mut d, x + w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, val(v));
                }
                x += w;
            }
        }
    }

    if want_map {
        let mx = x0 + if want_table { table_w + gap } else { 0.0 } + (body_w - if want_table { table_w + gap } else { 0.0 } - map_w) / 2.0;
        let my = top;
        let row_vars: String = f.vars[..rb].join("");
        let col_vars: String = f.vars[rb..].join("");
        // corner label: row variables below the diagonal, column variables above
        d.line(mx, my, mx + lab_w, my + lab_h, INK, 1.2);
        chart::text(&mut d, mx + lab_w - 6.0, my + fs * 1.0, fs * 0.85, "end", &col_vars);
        chart::text(&mut d, mx + 6.0, my + lab_h - 6.0, fs * 0.85, "start", &row_vars);
        for c in 0..nc {
            let x = mx + lab_w + c as f64 * cell;
            d.rect(x, my, cell, lab_h, head, INK, 1.2);
            gd::text_fill(&mut d, x + cell / 2.0, my + lab_h / 2.0 + fs * 0.3, fs * 0.9, INK, &bits(gray(c), cb));
        }
        let rbits = rb.max(1);
        for r in 0..nr {
            let y = my + lab_h + r as f64 * cell;
            d.rect(mx, y, lab_w, cell, head, INK, 1.2);
            gd::text_fill(&mut d, mx + lab_w / 2.0, y + cell / 2.0 + fs * 0.3, fs * 0.9, INK, &bits(gray(r), rbits));
        }
        let cell_row = |m: usize| (0..nr).find(|r| gray(*r) == m >> cb).unwrap_or(0);
        let cell_col = |m: usize| (0..nc).find(|c| gray(*c) == m & (nc - 1)).unwrap_or(0);
        for m in 0..rows {
            let (r, c) = (cell_row(m), cell_col(m));
            let (x, y) = (mx + lab_w + c as f64 * cell, my + lab_h + r as f64 * cell);
            let v = f.outs[out_k].1[m];
            d.rect(x, y, cell, cell, if answers && v == Some(true) { ink_fill } else { "#fff" }, INK, 1.2);
            if answers {
                gd::text_fill(&mut d, x + cell / 2.0, y + cell / 2.0 + fs * 0.35, fs, INK, val(v));
            }
        }
        if answers && p.b("groups") {
            let colors = if print { ["#000"; 6] } else { ["#2f6fe0", "#d9480f", "#2b8a3e", "#862e9c", "#e67700", "#0b7285"] };
            for (gi, t) in terms.iter().enumerate() {
                let mut rs: Vec<usize> = Vec::new();
                let mut cs: Vec<usize> = Vec::new();
                for m in 0..rows {
                    if covers(*t, m) {
                        rs.push(cell_row(m));
                        cs.push(cell_col(m));
                    }
                }
                rs.sort_unstable();
                rs.dedup();
                cs.sort_unstable();
                cs.dedup();
                let runs = |v: &[usize], len: usize| -> Vec<(usize, usize, bool, bool)> {
                    let mut out: Vec<(usize, usize)> = Vec::new();
                    for x in v {
                        match out.last_mut() {
                            Some(l) if l.1 + 1 == *x => l.1 = *x,
                            _ => out.push((*x, *x)),
                        }
                    }
                    // a group that leaves one edge of the map and comes back on the other is
                    // drawn as two loops, each left open on the edge it continues over
                    let wrapped = out.len() > 1 && out[0].0 == 0 && out[out.len() - 1].1 == len - 1;
                    let last = out.len() - 1;
                    out.iter().enumerate().map(|(i, (a, b))| (*a, *b, wrapped && i == 0, wrapped && i == last)).collect()
                };
                let inset = 4.0 + 2.5 * (gi % 3) as f64;
                for (r0, r1, r_open0, r_open1) in runs(&rs, nr) {
                    for (c0, c1, c_open0, c_open1) in runs(&cs, nc) {
                        let (x, y) = (mx + lab_w + c0 as f64 * cell + inset, my + lab_h + r0 as f64 * cell + inset);
                        let (gw, gh) = ((c1 - c0 + 1) as f64 * cell - inset * 2.0, (r1 - r0 + 1) as f64 * cell - inset * 2.0);
                        let dash = if print && gi % 2 == 1 { " stroke-dasharray=\"6 4\"" } else { "" };
                        let rr = cell * 0.25;
                        if !(r_open0 || r_open1 || c_open0 || c_open1) {
                            d.raw(&format!(
                                r##"<rect x="{x:.2}" y="{y:.2}" width="{gw:.2}" height="{gh:.2}" rx="{rr:.1}" fill="none" stroke="{}" stroke-width="2.4"{dash}/>"##,
                                colors[gi % 6]
                            ));
                        } else {
                            let (l, r, t, b) = (x, x + gw, y, y + gh);
                            let (has_t, has_b, has_l, has_r) = (!r_open0, !r_open1, !c_open0, !c_open1);
                            let mut path = String::new();
                            let ins = |on: bool| if on { rr } else { 0.0 };
                            if has_t {
                                path.push_str(&format!("M{:.2},{t:.2} H{:.2} ", l + ins(has_l), r - ins(has_r)));
                            }
                            if has_b {
                                path.push_str(&format!("M{:.2},{b:.2} H{:.2} ", l + ins(has_l), r - ins(has_r)));
                            }
                            if has_l {
                                path.push_str(&format!("M{l:.2},{:.2} V{:.2} ", t + ins(has_t), b - ins(has_b)));
                            }
                            if has_r {
                                path.push_str(&format!("M{r:.2},{:.2} V{:.2} ", t + ins(has_t), b - ins(has_b)));
                            }
                            if has_t && has_l {
                                path.push_str(&format!("M{l:.2},{:.2} A{rr:.2},{rr:.2} 0 0 1 {:.2},{t:.2} ", t + rr, l + rr));
                            }
                            if has_t && has_r {
                                path.push_str(&format!("M{:.2},{t:.2} A{rr:.2},{rr:.2} 0 0 1 {r:.2},{:.2} ", r - rr, t + rr));
                            }
                            if has_b && has_r {
                                path.push_str(&format!("M{r:.2},{:.2} A{rr:.2},{rr:.2} 0 0 1 {:.2},{b:.2} ", b - rr, r - rr));
                            }
                            if has_b && has_l {
                                path.push_str(&format!("M{:.2},{b:.2} A{rr:.2},{rr:.2} 0 0 1 {l:.2},{:.2} ", l + rr, b - rr));
                            }
                            d.raw(&format!(
                                r##"<path d="{path}" fill="none" stroke="{}" stroke-width="2.4" stroke-linecap="round"{dash}/>"##,
                                colors[gi % 6]
                            ));
                        }
                    }
                }
            }
        }
        if sop_on {
            let y = my + map_h + fs * 1.6;
            let sx = mx + map_w / 2.0;
            if answers {
                chart::text(&mut d, sx, y, fs, "middle", &sop_line);
            } else {
                chart::text(&mut d, sx, y, fs, "middle", &format!("{} =", f.outs[out_k].0));
            }
        }
    }
    if show != "Truth table" && !kmap_ok {
        chart::text(&mut d, w / 2.0, top + body_h + fs * 1.4, fs * 0.9, "middle", "A Karnaugh map needs 2 to 4 variables");
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    fn minterms(vars: &str, ones: &str, dc: &str) -> Params {
        let mut p = defaults();
        p.set_s("source", "Minterms");
        p.set_s("vars", vars);
        p.set_s("minterms", ones);
        p.set_s("dontcares", dc);
        p
    }

    fn simplified(vars: &str, ones: &str, dc: &str) -> String {
        let f = build(&minterms(vars, ones, dc)).unwrap();
        sop(&f, 0).0
    }

    #[test]
    fn quine_mccluskey_finds_small_covers() {
        assert_eq!(simplified("A, B", "1, 3", ""), "B");
        assert_eq!(simplified("A, B, C", "1, 3, 6, 7", ""), "A'C + AB");
        assert_eq!(simplified("A, B, C", "0, 1, 2, 3, 4, 5, 6, 7", ""), "1");
        assert_eq!(simplified("A, B, C", "", ""), "0");
        // the four corners of a 4-variable map are one group: B'D'
        assert_eq!(simplified("A, B, C, D", "0, 2, 8, 10", ""), "B'D'");
        // don't-cares may be grouped but need not be covered
        // A'D + CD is just as small; ties are broken the same way every time
        assert_eq!(simplified("A, B, C, D", "1, 3, 7, 11, 15", "0, 2, 5"), "A'B' + CD");
        assert_eq!(simplified("A, B", "3", ""), "AB");
        assert_eq!(simplified("X1, X2", "3", ""), "X1\u{b7}X2");
    }

    #[test]
    fn the_cover_is_never_larger_than_any_other_cover() {
        // 0, 1, 2, 5, 6, 7 on three variables is a six-cycle: no cover has fewer than three terms
        let f = build(&minterms("A, B, C", "0, 1, 2, 5, 6, 7", "")).unwrap();
        assert_eq!(sop(&f, 0).1.len(), 3);
        // a function whose first-found prime is not part of any smallest cover
        let f = build(&minterms("A, B, C, D", "0, 1, 2, 5, 6, 7, 8, 9, 10, 14", "")).unwrap();
        let n = sop(&f, 0).1.len();
        assert!(n <= 4, "{n}");
    }

    #[test]
    fn wrapped_groups_are_open_where_they_continue() {
        // the four corners of a 4-variable map: one group split over all four corners
        let mut p = minterms("A, B, C, D", "0, 2, 8, 10", "");
        p.set_s("show", "Both");
        let svg = render(&p);
        assert!(svg.matches("<path d=\"M").count() >= 4, "four open corner pieces");
        // a group that does not wrap stays a closed rounded box
        let mut q = minterms("A, B, C", "1, 3", "");
        q.set_s("show", "Both");
        assert!(!render(&q).contains("<path d=\"M"), "closed boxes stay rectangles");
    }

    #[test]
    fn every_cover_agrees_with_the_function() {
        for mask in 0u32..256 {
            let ones: Vec<usize> = (0..8).filter(|i| mask >> i & 1 == 1).collect();
            let f = Func { vars: vec!["A".into(), "B".into(), "C".into()], outs: vec![("F".into(), (0..8).map(|r| Some(ones.contains(&r))).collect())] };
            let (_, t) = sop(&f, 0);
            for r in 0..8 {
                assert_eq!(t.iter().any(|x| covers(*x, r)), ones.contains(&r), "{ones:?} row {r}");
            }
        }
    }

    #[test]
    fn default_draws_table_map_and_groups() {
        let svg = render(&defaults());
        assert!(svg.contains(">Q<") && svg.contains(">00<") && svg.contains(">01<") && svg.contains(">11<") && svg.contains(">10<"));
        assert!(svg.contains(">Q = A'C + AB<"), "{}", svg.split("<text").filter(|t| t.contains("Q =")).collect::<String>());
        assert!(svg.contains("stroke-width=\"2.4\""), "groups are drawn");
    }

    #[test]
    fn student_copy_keeps_inputs_and_drops_outputs() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(key.contains("A'C + AB") && !student.contains("A'C + AB"));
        assert!(student.contains(">Q =<") && !student.contains("stroke-width=\"2.4\"") && key.contains("stroke-width=\"2.4\""));
        assert!(key.matches(">1<").count() > student.matches(">1<").count());
        assert_eq!(key.matches("<rect").count() - student.matches("<rect").count(), 2, "the key adds the two group loops");
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        for (p, needle) in [
            (minterms("A, B", "5", ""), "does not exist"),
            (minterms("A, B", "x", ""), "not a row number"),
            (minterms("A, A", "1", ""), "twice"),
            (minterms("A, B", "1", "1"), "both"),
            (minterms("", "1", ""), "between 1 and 5"),
            (minterms("A, B, C, D, E, F", "1", ""), "between 1 and 5"),
            (minterms("1A, B", "1", ""), "not a usable"),
        ] {
            assert!(render(&p).contains(needle), "{needle}");
        }
        let mut p = defaults();
        p.set_s("expr", "Q = A AND");
        assert!(!render(&p).contains("NaN"));
    }

    #[test]
    fn map_limits_and_other_views() {
        let big = minterms("A, B, C, D, E", "1, 2, 31", "");
        assert!(render(&big).contains("needs 2 to 4 variables"));
        let mut p = minterms("A", "1", "");
        p.set_s("show", "Both");
        assert!(render(&p).contains("needs 2 to 4"));
        let mut p = defaults();
        p.set_s("show", "Truth table");
        p.set_b("row_numbers", true);
        let svg = render(&p);
        assert!(svg.contains(">#<") && svg.contains(">7<") && !svg.contains("stroke-width=\"2.4\""));
        let mut p = defaults();
        p.set_s("expr", "S = A XOR B\nC = A AND B");
        p.set_i("output", 2);
        assert!(render(&p).contains(">C = AB<"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = minterms("A, B, C, D", "0, 2, 5, 7, 8, 10, 13, 15", "");
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#ffe9a8") && !svg.contains("#2f6fe0") && !svg.contains("#eef1f6"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("boolean-table").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                if let Value::S(s) = v {
                    p.set_s(k, s);
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        for source in ["Expression", "Minterms"] {
            for text in ["", "\u{0}", "99999999999", "-1", "1, 1, 1", "A AND", "Q = (((A", "1e9", ",,,", "m3, m4"] {
                for show in SHOWS {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_s("source", source);
                        p.set_s("expr", text);
                        p.set_s("minterms", text);
                        p.set_s("dontcares", text);
                        p.set_s("show", show);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{source} {text:?}");
                    }
                }
            }
        }
    }
}
