//! Trace table: a few lines of pseudocode (assignments, print, one while or for loop) run step by
//! step into a table of every variable's value. The student copy (Show answers off) keeps the
//! program, the headings and the first rows, and leaves the rest of the cells empty.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_LINES: usize = 24;
const MAX_ROWS: usize = 40;
const MAX_STEPS: usize = 4000;
const MAX_VARS: usize = 8;
const MAX_EXPR: usize = 120;
const DEFAULT_PROGRAM: &str = "total = 0\nfor i = 1 to 5\n  total = total + i\nend\nprint total";

pub fn def() -> FigureDef {
    FigureDef {
        id: "trace-table",
        subject: Subject::Computing,
        title: "Trace table",
        blurb: "A short piece of pseudocode (assignments, print, one while or for loop) run step by step into a table of each variable's value, with the later rows left empty in the student copy.",
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
        "trace-table" => vec![
            Example { name: "Sum of 1 to 5", params: vec![] },
            Example {
                name: "While loop: halving",
                params: vec![("program", s("n = 100\ncount = 0\nwhile n > 10\n  n = n div 2\n  count = count + 1\nend\nprint count"))],
            },
            Example {
                name: "Every statement",
                params: vec![("rows", s("Every statement")), ("program", s("a = 3\nb = 4\nwhile a < 12\n  a = a + b\n  b = b - 1\nend"))],
            },
            Example { name: "Fibonacci", params: vec![("program", s("a = 0\nb = 1\nfor i = 1 to 7\n  print a\n  t = a + b\n  a = b\n  b = t\nend"))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("program", "Pseudocode", DEFAULT_PROGRAM).hint("x = expression; print x; one loop: while condition ... end, or for i = 1 to 5 [step 2] ... end. Operators + - * / div mod ( ), comparisons, and, or, not"),
        ParamDef::choice("rows", "One row per", &["Loop pass", "Every statement"], "Loop pass"),
        ParamDef::int("given", "Rows filled in for students", 0, 10, 1).hint("When Show answers is off, this many rows from the top keep their values and the rest are empty"),
        ParamDef::bool("numbers", "Number the program lines", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the program, the headings and the first rows and leaves the other cells empty"),
    ]
}

// ----- expressions -----

#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    Var(String),
    Un(&'static str, Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(&'static str),
}

const OPS: [&str; 18] = ["//", "<=", ">=", "==", "!=", "<>", "+", "-", "*", "/", "%", "(", ")", "<", ">", "=", ",", "!"];

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    if s.chars().count() > MAX_EXPR {
        return Err(format!("An expression is limited to {MAX_EXPR} characters"));
    }
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                i += 1;
            }
            let t: String = cs[st..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("\u{201c}{t}\u{201d} is not a number"))?));
        } else if c.is_alphabetic() || c == '_' {
            let st = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            out.push(Tok::Id(cs[st..i].iter().collect()));
        } else {
            let rest: String = cs[i..].iter().take(2).collect();
            let op = OPS.iter().find(|o| rest.starts_with(**o)).ok_or_else(|| format!("Unexpected \u{201c}{c}\u{201d}"))?;
            i += op.chars().count();
            out.push(Tok::Op(op));
        }
    }
    Ok(out)
}

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek_op(&self) -> Option<&'static str> {
        match self.t.get(self.i) {
            Some(Tok::Op(o)) => Some(o),
            _ => None,
        }
    }

    fn peek_word(&self) -> Option<String> {
        match self.t.get(self.i) {
            Some(Tok::Id(w)) => Some(w.to_lowercase()),
            _ => None,
        }
    }

    fn or(&mut self, d: usize) -> Result<Expr, String> {
        let mut l = self.and(d)?;
        while self.peek_word().as_deref() == Some("or") {
            self.i += 1;
            l = Expr::Bin("or", Box::new(l), Box::new(self.and(d)?));
        }
        Ok(l)
    }

    fn and(&mut self, d: usize) -> Result<Expr, String> {
        let mut l = self.not(d)?;
        while self.peek_word().as_deref() == Some("and") {
            self.i += 1;
            l = Expr::Bin("and", Box::new(l), Box::new(self.not(d)?));
        }
        Ok(l)
    }

    fn not(&mut self, d: usize) -> Result<Expr, String> {
        if self.peek_word().as_deref() == Some("not") || self.peek_op() == Some("!") {
            self.i += 1;
            return Ok(Expr::Un("not", Box::new(self.not(d + 1)?)));
        }
        self.cmp(d)
    }

    fn cmp(&mut self, d: usize) -> Result<Expr, String> {
        let l = self.add(d)?;
        if let Some(o) = self.peek_op().filter(|o| matches!(*o, "<" | "<=" | ">" | ">=" | "==" | "!=" | "<>")) {
            self.i += 1;
            let r = self.add(d)?;
            return Ok(Expr::Bin(if o == "<>" { "!=" } else { o }, Box::new(l), Box::new(r)));
        }
        Ok(l)
    }

    fn add(&mut self, d: usize) -> Result<Expr, String> {
        let mut l = self.mul(d)?;
        while let Some(o) = self.peek_op().filter(|o| matches!(*o, "+" | "-")) {
            self.i += 1;
            l = Expr::Bin(o, Box::new(l), Box::new(self.mul(d)?));
        }
        Ok(l)
    }

    fn mul(&mut self, d: usize) -> Result<Expr, String> {
        let mut l = self.unary(d)?;
        loop {
            let o = match (self.peek_op(), self.peek_word().as_deref()) {
                (Some(o @ ("*" | "/" | "%" | "//")), _) => if o == "%" { "mod" } else if o == "//" { "div" } else { o },
                (_, Some("mod")) => "mod",
                (_, Some("div")) => "div",
                _ => break,
            };
            self.i += 1;
            l = Expr::Bin(o, Box::new(l), Box::new(self.unary(d)?));
        }
        Ok(l)
    }

    fn unary(&mut self, d: usize) -> Result<Expr, String> {
        if d > 40 {
            return Err("That expression is nested too deeply".into());
        }
        if self.peek_op() == Some("-") {
            self.i += 1;
            return Ok(Expr::Un("neg", Box::new(self.unary(d + 1)?)));
        }
        match self.t.get(self.i).cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Expr::Num(n))
            }
            Some(Tok::Id(w)) => {
                self.i += 1;
                Ok(match w.to_lowercase().as_str() {
                    "true" => Expr::Num(1.0),
                    "false" => Expr::Num(0.0),
                    _ => Expr::Var(w),
                })
            }
            Some(Tok::Op("(")) => {
                self.i += 1;
                let e = self.or(d + 1)?;
                if self.peek_op() != Some(")") {
                    return Err("Missing a closing bracket".into());
                }
                self.i += 1;
                Ok(e)
            }
            Some(Tok::Op(o)) => Err(format!("Unexpected \u{201c}{o}\u{201d}")),
            None => Err("The expression ends too soon".into()),
        }
    }
}

fn expr(s: &str) -> Result<Expr, String> {
    let mut p = P { t: lex(s)?, i: 0 };
    let e = p.or(0)?;
    if p.i < p.t.len() {
        return Err(format!("Unexpected {}", match &p.t[p.i] {
            Tok::Num(n) => n.to_string(),
            Tok::Id(w) => w.clone(),
            Tok::Op(o) => o.to_string(),
        }));
    }
    Ok(e)
}

type Vars = Vec<(String, Option<f64>)>;

fn lookup(vars: &Vars, n: &str) -> Result<f64, String> {
    match vars.iter().find(|(k, _)| k == n) {
        Some((_, Some(v))) => Ok(*v),
        _ => Err(format!("{n} has no value yet")),
    }
}

fn eval(e: &Expr, vars: &Vars) -> Result<f64, String> {
    let b = |x: bool| if x { 1.0 } else { 0.0 };
    Ok(match e {
        Expr::Num(n) => *n,
        Expr::Var(n) => lookup(vars, n)?,
        Expr::Un("neg", a) => -eval(a, vars)?,
        Expr::Un(_, a) => b(eval(a, vars)? == 0.0),
        Expr::Bin(op, l, r) => {
            let (x, y) = (eval(l, vars)?, eval(r, vars)?);
            match *op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" | "div" | "mod" => {
                    if y == 0.0 {
                        return Err("Division by zero".into());
                    }
                    match *op {
                        "/" => x / y,
                        "div" => (x / y).floor(),
                        _ => x - y * (x / y).floor(),
                    }
                }
                "<" => b(x < y),
                "<=" => b(x <= y),
                ">" => b(x > y),
                ">=" => b(x >= y),
                "==" => b(x == y),
                "!=" => b(x != y),
                "and" => b(x != 0.0 && y != 0.0),
                _ => b(x != 0.0 || y != 0.0),
            }
        }
    })
}

// ----- the program -----

enum Stmt {
    Assign(String, Expr),
    Print(Vec<Expr>),
}

enum Head {
    While(Expr),
    For { var: String, from: Expr, to: Expr, step: Expr },
}

struct Line {
    no: usize,
    indent: usize,
    text: String,
}

struct Prog {
    lines: Vec<Line>,
    setup: Vec<(usize, Stmt)>,
    head: Option<(usize, Head)>,
    body: Vec<(usize, Stmt)>,
    after: Vec<(usize, Stmt)>,
}

fn stmt(text: &str) -> Result<Stmt, String> {
    let lower = text.to_lowercase();
    if lower == "print" || lower.starts_with("print ") || lower.starts_with("print(") || lower.starts_with("output ") {
        let rest = text.split_once(char::is_whitespace).map_or(&text[5.min(text.len())..], |(_, r)| r);
        let parts: Vec<&str> = rest.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Err("print needs something to show".into());
        }
        return Ok(Stmt::Print(parts.into_iter().map(expr).collect::<Result<_, _>>()?));
    }
    let (name, rhs) = text.split_once('=').ok_or_else(|| format!("\u{201c}{text}\u{201d} is not a statement (x = expression, print x, while ..., for ...)"))?;
    let name = name.trim();
    if rhs.starts_with('=') || name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(format!("\u{201c}{text}\u{201d} is not a statement (x = expression, print x, while ..., for ...)"));
    }
    Ok(Stmt::Assign(name.to_string(), expr(rhs)?))
}

fn parse(src: &str) -> Result<Prog, String> {
    let mut prog = Prog { lines: Vec::new(), setup: Vec::new(), head: None, body: Vec::new(), after: Vec::new() };
    #[derive(PartialEq)]
    enum At {
        Setup,
        Body,
        After,
    }
    let mut at = At::Setup;
    for (i, raw) in src.lines().enumerate() {
        let code = raw.split('#').next().unwrap_or("");
        let text = code.trim();
        if text.is_empty() {
            continue;
        }
        if prog.lines.len() >= MAX_LINES {
            return Err(format!("A program is limited to {MAX_LINES} lines"));
        }
        let no = prog.lines.len() + 1;
        let indent = (code.len() - code.trim_start().len()).min(12) / 2;
        prog.lines.push(Line { no, indent, text: text.to_string() });
        let wrap = |m: String| format!("Line {no}: {m}");
        let lower = text.to_lowercase();
        let first = lower.split_whitespace().next().unwrap_or("");
        match first {
            "while" | "for" => {
                if prog.head.is_some() {
                    return Err(wrap("only one loop is allowed".into()));
                }
                let rest = text[first.len()..].trim();
                let head = if first == "while" {
                    Head::While(expr(rest.trim_end_matches(':').trim_end_matches("do").trim()).map_err(&wrap)?)
                } else {
                    let (v, r) = rest.split_once('=').ok_or_else(|| wrap("a for loop reads for i = 1 to 5".into()))?;
                    let r = r.trim_end_matches(':');
                    let lo = r.to_lowercase();
                    let to = lo.find(" to ").ok_or_else(|| wrap("a for loop reads for i = 1 to 5".into()))?;
                    let (from, after_to) = (&r[..to], &r[to + 4..]);
                    let (hi, step) = match after_to.to_lowercase().find(" step ") {
                        Some(k) => (&after_to[..k], &after_to[k + 6..]),
                        None => (after_to, "1"),
                    };
                    let var = v.trim().to_string();
                    if var.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        return Err(wrap(format!("\u{201c}{var}\u{201d} is not a variable name")));
                    }
                    Head::For { var, from: expr(from).map_err(&wrap)?, to: expr(hi).map_err(&wrap)?, step: expr(step).map_err(&wrap)? }
                };
                prog.head = Some((no, head));
                at = At::Body;
            }
            "end" | "endwhile" | "endfor" | "next" => {
                if at != At::Body {
                    return Err(wrap("end without a loop".into()));
                }
                at = At::After;
            }
            _ => {
                let s = stmt(text).map_err(&wrap)?;
                match at {
                    At::Setup => prog.setup.push((no, s)),
                    At::Body => prog.body.push((no, s)),
                    At::After => prog.after.push((no, s)),
                }
            }
        }
        let _ = i;
    }
    Ok(prog)
}

struct Row {
    label: String,
    vals: Vec<Option<f64>>,
    out: String,
}

struct Trace {
    cols: Vec<String>,
    rows: Vec<Row>,
    note: Option<String>,
    has_output: bool,
}

fn run(prog: &Prog, per_statement: bool) -> Trace {
    let mut vars: Vars = Vec::new();
    // columns in order of first assignment
    let declare = |n: &str, vars: &mut Vars| {
        if !vars.iter().any(|(k, _)| k == n) {
            vars.push((n.to_string(), None));
        }
    };
    let assigns = |list: &[(usize, Stmt)], vars: &mut Vars| {
        for (_, st) in list {
            if let Stmt::Assign(n, _) = st {
                declare(n, vars);
            }
        }
    };
    assigns(&prog.setup, &mut vars);
    if let Some((_, Head::For { var, .. })) = &prog.head {
        declare(var, &mut vars);
    }
    assigns(&prog.body, &mut vars);
    assigns(&prog.after, &mut vars);
    vars.truncate(MAX_VARS);
    let has_output = prog.setup.iter().chain(&prog.body).chain(&prog.after).any(|(_, s)| matches!(s, Stmt::Print(_)));
    let mut t = Trace { cols: vars.iter().map(|(k, _)| k.clone()).collect(), rows: Vec::new(), note: None, has_output };
    let mut steps = 0usize;
    let mut out = String::new();

    // one statement; Err ends the trace with a note
    fn exec(s: &Stmt, vars: &mut Vars, out: &mut String) -> Result<(), String> {
        match s {
            Stmt::Assign(n, e) => {
                let v = eval(e, vars)?;
                if let Some(slot) = vars.iter_mut().find(|(k, _)| k == n) {
                    slot.1 = Some(v);
                }
            }
            Stmt::Print(es) => {
                let parts: Vec<String> = es.iter().map(|e| eval(e, vars).map(chart::fmt)).collect::<Result<_, _>>()?;
                if !out.is_empty() {
                    out.push_str(", ");
                }
                out.push_str(&parts.join(" "));
            }
        }
        Ok(())
    }

    let snap = |t: &mut Trace, label: String, vars: &Vars, out: &mut String| {
        t.rows.push(Row { label, vals: vars.iter().map(|(_, v)| *v).collect(), out: std::mem::take(out) });
    };
    macro_rules! stop {
        ($t:expr, $line:expr, $m:expr) => {{
            $t.note = Some(format!("Line {}: {}", $line, $m));
            return $t;
        }};
    }
    // the output column shows what each row printed, not a running total
    let full = |t: &Trace| t.rows.len() >= MAX_ROWS;

    // setup
    let setup_rows = per_statement;
    for (no, s) in &prog.setup {
        if let Err(e) = exec(s, &mut vars, &mut out) {
            stop!(t, no, e);
        }
        if setup_rows {
            snap(&mut t, format!("{no}"), &vars, &mut out);
        }
    }
    if !setup_rows {
        snap(&mut t, "start".into(), &vars, &mut out);
    }
    if let Some((hno, head)) = &prog.head {
        let (mut var_step, mut limit): (Option<(String, f64)>, Option<f64>) = (None, None);
        if let Head::For { var, from, to, step } = head {
            let (f, l, s) = match (eval(from, &vars), eval(to, &vars), eval(step, &vars)) {
                (Ok(f), Ok(l), Ok(s)) => (f, l, s),
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => stop!(t, hno, e),
            };
            if s == 0.0 {
                stop!(t, hno, "the step cannot be 0");
            }
            if let Some(slot) = vars.iter_mut().find(|(k, _)| k == var) {
                slot.1 = Some(f);
            }
            var_step = Some((var.clone(), s));
            limit = Some(l);
            if per_statement {
                snap(&mut t, format!("{hno}"), &vars, &mut out);
            }
        }
        let mut pass = 0usize;
        loop {
            steps += 1;
            if steps > MAX_STEPS || full(&t) {
                t.note = Some(format!("Stopped after {MAX_ROWS} rows"));
                return t;
            }
            let go = match (head, &var_step, limit) {
                (Head::While(c), _, _) => match eval(c, &vars) {
                    Ok(v) => v != 0.0,
                    Err(e) => stop!(t, hno, e),
                },
                (Head::For { var, .. }, Some((_, s)), Some(l)) => {
                    let v = lookup(&vars, var).unwrap_or(0.0);
                    if *s > 0.0 { v <= l } else { v >= l }
                }
                _ => false,
            };
            if !go {
                break;
            }
            pass += 1;
            for (no, s) in &prog.body {
                if let Err(e) = exec(s, &mut vars, &mut out) {
                    stop!(t, no, e);
                }
                if per_statement {
                    if full(&t) {
                        t.note = Some(format!("Stopped after {MAX_ROWS} rows"));
                        return t;
                    }
                    snap(&mut t, format!("{no}"), &vars, &mut out);
                }
            }
            if let Some((v, s)) = &var_step {
                let cur = lookup(&vars, v).unwrap_or(0.0);
                if let Some(slot) = vars.iter_mut().find(|(k, _)| k == v) {
                    slot.1 = Some(cur + s);
                }
                if per_statement {
                    snap(&mut t, format!("{hno}"), &vars, &mut out);
                }
            }
            if !per_statement {
                snap(&mut t, format!("{pass}"), &vars, &mut out);
            }
        }
    }
    for (no, s) in &prog.after {
        if let Err(e) = exec(s, &mut vars, &mut out) {
            stop!(t, no, e);
        }
        if per_statement {
            snap(&mut t, format!("{no}"), &vars, &mut out);
        }
    }
    if !per_statement && !prog.after.is_empty() {
        snap(&mut t, "after".into(), &vars, &mut out);
    }
    if !out.is_empty() {
        // printed after the last row (setup-only programs, or output with no row of its own)
        if let Some(last) = t.rows.last_mut() {
            if !last.out.is_empty() {
                last.out.push_str(", ");
            }
            last.out.push_str(&out);
        }
    }
    t
}

fn render(p: &Params) -> String {
    let prog = match parse(p.s("program")) {
        Ok(g) => g,
        Err(e) => return chart::message(&e),
    };
    if prog.lines.is_empty() {
        return chart::message("Write some pseudocode to trace");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let per = p.s("rows") == "Every statement";
    let t = run(&prog, per);
    let given = p.i("given").max(0) as usize;
    let title = p.s("title").trim();
    let numbered = p.b("numbers");

    // program listing
    let row_h = fs * 1.75;
    let list_w = prog.lines.iter().map(|l| gd::text_w(&l.text, fs) + l.indent as f64 * fs * 1.1).fold(0.0, f64::max) + if numbered { fs * 2.2 } else { 0.0 } + 28.0;
    let list_h = row_h * prog.lines.len() as f64 + 20.0;

    // table
    let label_head = if per { "Line" } else { "Pass" };
    let val_w = (fs * 4.0).max(t.cols.iter().map(|c| gd::text_w(c, fs) + 28.0).fold(0.0, f64::max));
    let lab_w = fs * 4.2;
    let out_w = if t.has_output { (fs * 5.0).max(t.rows.iter().map(|r| gd::text_w(&r.out, fs) + 24.0).fold(0.0, f64::max)) } else { 0.0 };
    let table_w = lab_w + val_w * t.cols.len() as f64 + out_w;
    let table_h = row_h * (t.rows.len() + 1) as f64;
    let note_h = if t.note.is_some() { row_h * 1.2 } else { 0.0 };
    let gap = 36.0;
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 0.4 };
    let w = (list_w + gap + table_w + 40.0).max(320.0);
    let h = top + list_h.max(table_h + note_h) + 16.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let x0 = (w - (list_w + gap + table_w)) / 2.0;
    let head = if print { "#e5e5e5" } else { "#eef1f6" };

    // the listing sits in a light box
    d.raw(&format!(
        r##"<rect x="{x0:.2}" y="{top:.2}" width="{list_w:.2}" height="{list_h:.2}" rx="6" fill="{}" stroke="{INK}" stroke-width="1.4"/>"##,
        if print { "#f6f6f6" } else { "#f7f8fb" }
    ));
    for (i, l) in prog.lines.iter().enumerate() {
        let y = top + 10.0 + row_h * (i + 1) as f64 - fs * 0.45;
        let mut x = x0 + 14.0;
        if numbered {
            d.raw(&format!(
                r##"<text x="{:.2}" y="{y:.2}" font-size="{}" text-anchor="end" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="#6b7280">{}</text>"##,
                x + fs * 1.2,
                fs * 0.85,
                l.no
            ));
            x += fs * 2.2;
        }
        d.raw(&format!(
            r##"<text x="{:.2}" y="{y:.2}" font-size="{fs}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
            x + l.indent as f64 * fs * 1.1,
            svg::esc(&l.text)
        ));
    }

    // the table
    let tx = x0 + list_w + gap;
    let mut x = tx;
    let mut heads: Vec<(f64, String)> = vec![(lab_w, label_head.to_string())];
    heads.extend(t.cols.iter().map(|c| (val_w, c.clone())));
    if t.has_output {
        heads.push((out_w, "Output".into()));
    }
    for (cw, name) in &heads {
        d.rect(x, top, *cw, row_h, head, INK, 1.2);
        gd::text_fill(&mut d, x + cw / 2.0, top + row_h / 2.0 + fs * 0.35, fs, INK, name);
        x += cw;
    }
    for (ri, r) in t.rows.iter().enumerate() {
        let y = top + row_h * (ri + 1) as f64;
        let shown = answers || ri < given;
        d.rect(tx, y, lab_w, row_h, head, INK, 1.2);
        gd::text_fill(&mut d, tx + lab_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, &r.label);
        for (ci, v) in r.vals.iter().enumerate() {
            let cx = tx + lab_w + ci as f64 * val_w;
            d.rect(cx, y, val_w, row_h, "#fff", INK, 1.2);
            if let (true, Some(v)) = (shown, v) {
                gd::text_fill(&mut d, cx + val_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, &chart::fmt(*v));
            }
        }
        if t.has_output {
            let cx = tx + lab_w + val_w * t.cols.len() as f64;
            d.rect(cx, y, out_w, row_h, "#fff", INK, 1.2);
            if shown {
                gd::text_fill(&mut d, cx + out_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, &r.out);
            }
        }
    }
    if let Some(n) = &t.note {
        chart::text(&mut d, tx, top + table_h + row_h * 0.9, fs * 0.9, "start", n);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    fn trace(src: &str, per: bool) -> Trace {
        run(&parse(src).unwrap(), per)
    }

    fn col(t: &Trace, name: &str) -> Vec<Option<f64>> {
        let c = t.cols.iter().position(|c| c == name).unwrap();
        t.rows.iter().map(|r| r.vals[c]).collect()
    }

    #[test]
    fn expressions_follow_the_usual_rules() {
        let v: Vars = vec![("x".into(), Some(7.0))];
        for (src, want) in [("1 + 2 * 3", 7.0), ("(1 + 2) * 3", 9.0), ("x div 2", 3.0), ("x mod 4", 3.0), ("-x + 10", 3.0), ("x // 2 + x % 2", 4.0), ("x > 3 and x < 10", 1.0), ("not x == 7", 0.0), ("2 - 3 - 4", -5.0), ("7 / 2", 3.5), ("-7 div 2", -4.0), ("-7 mod 3", 2.0), ("x <> 1 or false", 1.0)] {
            assert_eq!(eval(&expr(src).unwrap(), &v).unwrap(), want, "{src}");
        }
        assert!(eval(&expr("1 / 0").unwrap(), &v).unwrap_err().contains("zero"));
        assert!(eval(&expr("y").unwrap(), &v).unwrap_err().contains("no value"));
        for bad in ["", "1 +", "(1", "1 2", "$", "1 + * 2"] {
            assert!(expr(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_for_loop_traces_pass_by_pass() {
        let t = trace(DEFAULT_PROGRAM, false);
        assert_eq!(t.cols, ["total", "i"]);
        assert_eq!(t.rows.len(), 1 + 5 + 1, "start, five passes and the lines after the loop");
        assert_eq!(col(&t, "total"), [Some(0.0), Some(1.0), Some(3.0), Some(6.0), Some(10.0), Some(15.0), Some(15.0)]);
        assert_eq!(col(&t, "i"), [None, Some(2.0), Some(3.0), Some(4.0), Some(5.0), Some(6.0), Some(6.0)], "i has stepped past the end when the loop stops");
        assert_eq!((t.rows.last().unwrap().label.as_str(), t.rows.last().unwrap().out.as_str()), ("after", "15"));
        assert_eq!(t.rows[0].label, "start");
    }

    #[test]
    fn a_while_loop_and_statement_rows() {
        let t = trace("n = 100\ncount = 0\nwhile n > 10\n  n = n div 2\n  count = count + 1\nend", false);
        assert_eq!(col(&t, "n"), [Some(100.0), Some(50.0), Some(25.0), Some(12.0), Some(6.0)]);
        assert_eq!(col(&t, "count").last().unwrap(), &Some(4.0));
        let s = trace("a = 3\nb = 4\nwhile a < 12\n  a = a + b\n  b = b - 1\nend", true);
        assert_eq!(s.rows[0].label, "1");
        assert_eq!(s.rows.len(), 2 + 2 * 3, "two setup rows, then two statements for each of three passes: {:?}", col(&s, "a"));
        assert_eq!(col(&s, "a").last().unwrap(), &Some(12.0));
    }

    #[test]
    fn print_goes_in_the_output_column_and_a_step_can_count_down() {
        let t = trace("a = 0\nb = 1\nfor i = 1 to 4\n  print a\n  t = a + b\n  a = b\n  b = t\nend", false);
        let outs: Vec<&str> = t.rows.iter().map(|r| r.out.as_str()).collect();
        assert_eq!(outs, ["", "0", "1", "1", "2"]);
        let t = trace("for i = 3 to 1 step -1\n  print i\nend", false);
        assert_eq!(t.rows.iter().map(|r| r.out.clone()).collect::<Vec<_>>(), ["", "3", "2", "1"]);
    }

    #[test]
    fn runaway_and_broken_programs_stop_with_a_note() {
        let t = trace("i = 0\nwhile i >= 0\n  i = i + 1\nend", false);
        assert!(t.note.as_deref().is_some_and(|n| n.contains("Stopped after")) && t.rows.len() <= MAX_ROWS);
        let t = trace("x = 1 / 0", false);
        assert!(t.note.as_deref().is_some_and(|n| n.contains("Line 1") && n.contains("zero")));
        let t = trace("for i = 1 to 3 step 0\nend", false);
        assert!(t.note.as_deref().is_some_and(|n| n.contains("step")));
        for (src, needle) in [("while x < 3", "no value"), ("x = 1\nend", "end without"), ("for i = 1 to 2\nwhile x\nend", "only one loop"), ("hello", "not a statement"), ("for i = 1\nend", "for i = 1 to 5"), ("x == 3", "not a statement"), ("print", "needs something")] {
            let err = match parse(src) {
                Err(e) => e,
                Ok(p) => run(&p, false).note.unwrap_or_default(),
            };
            assert!(err.contains(needle), "{src:?}: {err}");
        }
    }

    #[test]
    fn student_copy_keeps_the_program_and_the_given_rows() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">total = total + i<") && student.contains(">start<"));
        assert!(key.contains(">15<") && !student.contains(">15<"));
        assert!(student.contains(">0<"), "the first row is given");
        assert_eq!(student.matches("<rect").count(), key.matches("<rect").count());
        assert_eq!(size_of(&student), size_of(&key));
        let mut p = defaults();
        p.set_i("given", 0);
        let (student, _) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(!student.contains(">0<") || student.matches(">0<").count() < key.matches(">0<").count());
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#eef1f6") && !svg.contains("#f7f8fb"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("trace-table").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                if let Value::S(s) = v {
                    p.set_s(k, s);
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        let long: String = (0..40).map(|i| format!("v{i} = {i}\n")).collect();
        for prog in ["", "\u{0}", "x = ((((((((((((((((((((((((((((((((((((((((((((((1))))))))))))))))))))))))))))))))))))))))))))))", &long, "x = 1e999", "for i = 1 to 1000000\n  x = i\nend", "x = 1\nwhile x\n  x = x\nend", "while 1 < 2\nend", "a = 1\nprint a, a + 1, a * 2"] {
            for rows in ["Loop pass", "Every statement"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("program", prog);
                    p.set_s("rows", rows);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{prog:?}");
                }
            }
        }
    }
}
