//! Recursion tree: the calls a recursive function makes, as a tree with each call's return value
//! under it. Fibonacci, factorial, sum, gcd and fast power. Base cases are shaded and the calls
//! can be numbered in the order they start or finish. The student copy (Show answers off) keeps
//! the calls and leaves the return values and numbers empty.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::graph::Tree;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_CALLS: usize = 80;
const FUNCS: [&str; 5] = ["fib(n)", "factorial(n)", "sum(n)", "gcd(a, b)", "power(x, n)"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "recursion-tree",
        subject: Subject::Computing,
        title: "Recursion tree",
        blurb: "The calls a recursive function makes, drawn as a tree with each call's return value under it, base cases shaded and the calls optionally numbered in order, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("recursion-tree", "b") => matches!(p.s("func"), "gcd(a, b)" | "power(x, n)"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "recursion-tree" => vec![
            Example { name: "fib(5)", params: vec![] },
            Example { name: "Calls in order", params: vec![("number", s("Order the calls start"))] },
            Example { name: "factorial(5)", params: vec![("func", s("factorial(n)")), ("a", Value::I(5))] },
            Example { name: "gcd(48, 18)", params: vec![("func", s("gcd(a, b)")), ("a", Value::I(48)), ("b", Value::I(18))] },
            Example { name: "power(2, 10)", params: vec![("func", s("power(x, n)")), ("a", Value::I(2)), ("b", Value::I(10))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("func", "Function", &FUNCS, "fib(n)").hint("fib(n) adds the two before; power(x, n) squares half the power"),
        ParamDef::int("a", "n (or a, or x)", 0, 60, 5),
        ParamDef::int("b", "b (or n for power)", 0, 40, 3),
        ParamDef::choice("number", "Number the calls", &["None", "Order the calls start", "Order the calls return"], "None"),
        ParamDef::bool("returns", "Show return values", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the calls and leaves the return values and numbers empty for students"),
    ]
}

/// A call tree: node 0 is the first call.
#[derive(Debug)]
struct Calls {
    label: Vec<String>,
    value: Vec<String>,
    base: Vec<bool>,
    kids: Vec<Vec<Option<usize>>>,
}

impl Calls {
    fn add(&mut self, label: String) -> usize {
        self.label.push(label);
        self.value.push(String::new());
        self.base.push(false);
        self.kids.push(vec![]);
        self.label.len() - 1
    }
}

type Res = Result<i128, String>;

fn go(func: &str, a: i64, b: i64, c: &mut Calls, parent: Option<usize>) -> Res {
    if c.label.len() >= MAX_CALLS {
        return Err(format!("That makes more than {MAX_CALLS} calls; try a smaller number"));
    }
    let label = match func {
        "gcd(a, b)" => format!("gcd({a}, {b})"),
        "power(x, n)" => format!("power({a}, {b})"),
        "factorial(n)" => format!("fact({a})"),
        "sum(n)" => format!("sum({a})"),
        _ => format!("fib({a})"),
    };
    let me = c.add(label);
    if let Some(p) = parent {
        c.kids[p].push(Some(me));
    }
    let v: i128 = match func {
        "gcd(a, b)" => {
            if b == 0 {
                c.base[me] = true;
                a as i128
            } else {
                go(func, b, a % b, c, Some(me))?
            }
        }
        "power(x, n)" => {
            if b == 0 {
                c.base[me] = true;
                1
            } else {
                let h = go(func, a, b / 2, c, Some(me))?;
                let sq = h.checked_mul(h).ok_or("The answer is too large to show")?;
                if b % 2 == 0 { sq } else { sq.checked_mul(a as i128).ok_or("The answer is too large to show")? }
            }
        }
        "factorial(n)" => {
            if a <= 1 {
                c.base[me] = true;
                1
            } else {
                go(func, a - 1, 0, c, Some(me))?.checked_mul(a as i128).ok_or("The answer is too large to show")?
            }
        }
        "sum(n)" => {
            if a <= 0 {
                c.base[me] = true;
                0
            } else {
                a as i128 + go(func, a - 1, 0, c, Some(me))?
            }
        }
        _ => {
            if a < 2 {
                c.base[me] = true;
                a as i128
            } else {
                go(func, a - 1, 0, c, Some(me))? + go(func, a - 2, 0, c, Some(me))?
            }
        }
    };
    c.value[me] = v.to_string();
    Ok(v)
}

fn build(p: &Params) -> Result<Calls, String> {
    let func = FUNCS.iter().copied().find(|f| *f == p.s("func")).unwrap_or("fib(n)");
    let (a, b) = (p.i("a"), p.i("b"));
    let mut c = Calls { label: vec![], value: vec![], base: vec![], kids: vec![] };
    if func == "power(x, n)" && b > 40 {
        return Err("n is limited to 40".into());
    }
    go(func, a, b, &mut c, None)?;
    Ok(c)
}

fn render(p: &Params) -> String {
    let c = match build(p) {
        Ok(c) => c,
        Err(e) => return chart::message(&e),
    };
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let n = c.label.len();
    let tree = Tree { labels: c.label.clone(), kids: c.kids.clone(), binary: false };
    let title = p.s("title").trim();
    let want_val = p.b("returns");
    let box_h = if want_val { fs * 3.4 } else { fs * 2.2 };
    let widths: Vec<f64> = c.label.iter().zip(&c.value).map(|(l, v)| gd::text_w(l, fs).max(gd::text_w(&format!("= {v}"), fs * 0.95)) + 26.0).collect();
    let wmax = widths.iter().copied().fold(60.0, f64::max);
    let (pts, dw, dh) = gd::to_px(&tree.layout(), (wmax + 14.0, box_h + 42.0), (wmax / 2.0 + 16.0, box_h / 2.0 + 14.0));
    let top = if title.is_empty() { 4.0 } else { 34.0 + fs * 1.3 };
    let w = dw.max(280.0);
    let h = top + dh;
    let ox = (w - dw) / 2.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let at = |i: usize| (pts[i].0 + ox, pts[i].1 + top);
    for (i, ks) in c.kids.iter().enumerate() {
        for ch in ks.iter().flatten() {
            let (a, b) = (at(i), at(*ch));
            d.line(a.0, a.1 + box_h / 2.0, b.0, b.1 - box_h / 2.0, INK, 1.8);
        }
    }
    // numbering: the order calls start is the index; the order they return is post-order
    let post = tree.postorder();
    let number = |i: usize| match p.s("number") {
        "Order the calls start" => Some(i + 1),
        "Order the calls return" => post.iter().position(|x| *x == i).map(|x| x + 1),
        _ => None,
    };
    for i in 0..n {
        let (cx, cy) = at(i);
        let bw = widths[i].max(60.0);
        let fill = match (c.base[i], print) {
            (true, true) => "#d9d9d9",
            (true, false) => "#dff3e4",
            (false, true) => "#fff",
            (false, false) => "#eef3fc",
        };
        d.raw(&format!(
            r##"<rect x="{:.2}" y="{:.2}" width="{bw:.2}" height="{box_h:.2}" rx="8" fill="{fill}" stroke="{INK}" stroke-width="1.8"/>"##,
            cx - bw / 2.0,
            cy - box_h / 2.0
        ));
        let ly = if want_val { cy - fs * 0.45 } else { cy + fs * 0.35 };
        gd::text_fill(&mut d, cx, ly, fs, INK, &c.label[i]);
        if want_val {
            d.line(cx - bw / 2.0 + 8.0, cy + fs * 0.1, cx + bw / 2.0 - 8.0, cy + fs * 0.1, "#9ca3af", 1.0);
            if answers {
                gd::text_fill(&mut d, cx, cy + fs * 1.15, fs * 0.95, if print { "#000" } else { "#b45309" }, &format!("= {}", c.value[i]));
            }
        }
        if let (true, Some(no)) = (answers, number(i)) {
            let (bx, by) = (cx - bw / 2.0, cy - box_h / 2.0);
            d.circle(bx, by, fs * 0.66, if print { "#000" } else { "#d97706" }, "#fff", 1.5);
            gd::text_fill(&mut d, bx, by + fs * 0.27, fs * 0.8, "#fff", &no.to_string());
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    fn with(func: &str, a: i64, b: i64) -> Params {
        let mut p = defaults();
        p.set_s("func", func);
        p.set_i("a", a);
        p.set_i("b", b);
        p
    }

    #[test]
    fn calls_and_values_are_right() {
        let c = build(&with("fib(n)", 5, 0)).unwrap();
        assert_eq!(c.label.len(), 15, "fib(5) makes 15 calls");
        assert_eq!((c.label[0].as_str(), c.value[0].as_str()), ("fib(5)", "5"));
        assert_eq!(c.base.iter().filter(|b| **b).count(), 8);
        let c = build(&with("factorial(n)", 5, 0)).unwrap();
        assert_eq!((c.label.len(), c.value[0].as_str()), (5, "120"));
        let c = build(&with("sum(n)", 4, 0)).unwrap();
        assert_eq!((c.label.len(), c.value[0].as_str()), (5, "10"));
        let c = build(&with("gcd(a, b)", 48, 18)).unwrap();
        assert_eq!(c.label, ["gcd(48, 18)", "gcd(18, 12)", "gcd(12, 6)", "gcd(6, 0)"]);
        assert_eq!(c.value[0], "6");
        let c = build(&with("power(x, n)", 2, 10)).unwrap();
        assert_eq!(c.label, ["power(2, 10)", "power(2, 5)", "power(2, 2)", "power(2, 1)", "power(2, 0)"]);
        assert_eq!(c.value[0], "1024");
        assert_eq!(build(&with("power(x, n)", 3, 5)).unwrap().value[0], "243");
        assert!(build(&with("fib(n)", 12, 0)).unwrap_err().contains("more than"));
        assert!(build(&with("power(x, n)", 60, 40)).unwrap_err().contains("too large"));
    }

    #[test]
    fn default_draws_every_call_with_values() {
        let svg = render(&defaults());
        assert_eq!(svg.matches("fib(").count(), 15);
        assert!(svg.contains(">= 5<") && svg.contains(">= 1<") && svg.contains(">= 0<"));
        let mut p = defaults();
        p.set_b("returns", false);
        assert!(!render(&p).contains(">= 5<"));
    }

    #[test]
    fn numbering_orders_are_start_and_return() {
        let mut p = with("factorial(n)", 3, 0);
        p.set_s("number", "Order the calls start");
        let start = render(&p);
        assert_eq!(start.matches("#d97706").count(), 3);
        p.set_s("number", "Order the calls return");
        let ret = render(&p);
        assert_ne!(start, ret);
        // the root returns last
        let t = Tree { labels: vec!["a".into(), "b".into(), "c".into()], kids: vec![vec![Some(1)], vec![Some(2)], vec![]], binary: false };
        assert_eq!(t.postorder(), [2, 1, 0]);
    }

    #[test]
    fn student_copy_keeps_the_calls_only() {
        let def = def();
        let mut p = defaults();
        p.set_s("number", "Order the calls start");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(student.matches("fib(").count() == 15 && !student.contains(">= 5<") && key.contains(">= 5<"));
        assert!(!student.contains("#d97706") && key.contains("#d97706"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("number", "Order the calls start");
        let svg = render(&p);
        assert!(!svg.contains("#dff3e4") && !svg.contains("#eef3fc") && !svg.contains("#b45309") && !svg.contains("#d97706"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("recursion-tree").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::I(i) => p.set_i(k, *i),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        for func in FUNCS {
            for (a, b) in [(0, 0), (1, 1), (60, 40), (-5, -5), (7, 0), (0, 7), (i64::MAX, i64::MAX), (12, 5)] {
                for number in ["None", "Order the calls start", "Order the calls return"] {
                    for answers in [true, false] {
                        let mut p = with(func, a, b);
                        p.set_s("number", number);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{func} {a} {b}");
                    }
                }
            }
        }
    }
}
