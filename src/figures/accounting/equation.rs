//! The accounting equation: assets = liabilities + equity, with a list of transactions and the
//! effect of each on the three parts, and the totals at the end. The transactions are what a
//! student is given; the effects and the totals are the answers, so the student copy (Show
//! answers off) keeps the descriptions and leaves the effect cells and the check empty.
use super::{cents_param, clip, currency_param, digits, money, parse_amount, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ROWS: usize = 10;
const MAX_SPEC: usize = 2000;
const DEFAULT_SPEC: &str = "Owner invests cash: A +10000, E +10000\nBuy equipment on credit: A +3000, L +3000\nEarn service fees in cash: A +1800, E +1800\nPay part of the loan: A -1000, L -1000\nBuy supplies for cash: A +400, A -400";

pub fn def() -> FigureDef {
    FigureDef {
        id: "accounting-equation",
        subject: Subject::Accounting,
        title: "Accounting equation",
        blurb: "Assets = liabilities + equity: each transaction's effect on the three parts, one row at a time, with the totals and a check, or left empty for students.",
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
        "accounting-equation" => vec![
            Example { name: "Five transactions", params: vec![] },
            Example {
                name: "A year of a small shop",
                params: vec![("spec", s("Owner invests cash: A +20000, E +20000\nBuy inventory on credit: A +6000, L +6000\nSell goods for cash: A +9000, E +9000\nPay supplier: A -4000, L -4000\nPay rent: A -1200, E -1200")), ("title", s("Quick Mart"))],
            },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Transactions", DEFAULT_SPEC).hint("One per line: a description, a colon, then the effects as A +500, L -200, E +300 (A assets, L liabilities, E equity)"),
        ParamDef::bool("totals", "Write the totals and the check", true),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the transaction descriptions and leaves the effects, totals and check empty for students"),
    ]
}

struct Tx {
    text: String,
    /// The net change to assets, liabilities and equity.
    d: [f64; 3],
}

fn parse(spec: &str) -> Result<Vec<Tx>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The transactions are limited to {MAX_SPEC} characters"));
    }
    let mut out = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (text, effects) = line.rsplit_once(':').ok_or_else(|| format!("\u{201c}{line}\u{201d} needs a description, a colon and the effects, like Pay rent: A -500, E -500"))?;
        let mut d = [0.0; 3];
        for e in effects.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let (letter, amt) = e.split_once(char::is_whitespace).ok_or_else(|| format!("\u{201c}{e}\u{201d} should be a letter and an amount, like A +500"))?;
            let idx = match letter.to_ascii_uppercase().as_str() {
                "A" => 0,
                "L" => 1,
                "E" => 2,
                _ => return Err(format!("\u{201c}{letter}\u{201d} should be A, L or E")),
            };
            let amt = amt.trim();
            let (sign, num) = match amt.chars().next() {
                Some('-') | Some('\u{2212}') => (-1.0, &amt[amt.chars().next().map_or(0, char::len_utf8)..]),
                Some('+') => (1.0, &amt[1..]),
                _ => (1.0, amt),
            };
            let v = parse_amount(num).ok_or_else(|| format!("\u{201c}{amt}\u{201d} is not an amount"))?;
            d[idx] += sign * v;
        }
        if out.len() >= MAX_ROWS {
            return Err(format!("A figure is limited to {MAX_ROWS} transactions"));
        }
        out.push(Tx { text: clip(text, 40), d });
    }
    Ok(out)
}

fn signed(v: f64, cents: bool) -> String {
    if v.abs() < 1e-9 {
        String::new()
    } else if v > 0.0 {
        format!("+{}", digits(v, cents))
    } else {
        format!("\u{2212}{}", digits(-v, cents))
    }
}

fn render(p: &Params) -> String {
    let txs = match parse(p.s("spec")) {
        Ok(t) => t,
        Err(e) => return chart::message(&e),
    };
    if txs.is_empty() {
        return chart::message("Add a transaction: Pay rent: A -500, E -500");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (sym, cents) = (symbol(p), p.b("cents"));
    let title = p.s("title").trim();
    let totals = p.b("totals");
    let cols = [34.0, 270.0, 100.0, 110.0, 100.0];
    let (x0, tw) = (20.0, cols.iter().sum::<f64>());
    let w = tw + 40.0;
    let row = fs * 1.9;
    let top = if title.is_empty() { 12.0 } else { 38.0 + fs * 1.3 };
    let n = txs.len() as f64;
    let end_h = row * (n + 1.0) + if totals { row * 1.3 } else { 0.0 };
    let h = top + row * 1.0 + end_h + if totals { row * 1.8 } else { 0.0 } + 14.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let xs: Vec<f64> = cols.iter().scan(x0, |a, c| { let v = *a; *a += c; Some(v) }).collect();
    let head = if print { "#e5e5e5" } else { "#e6edf8" };
    // the equation itself, above the columns
    chart::text(&mut d, xs[2] + cols[2] / 2.0 - 8.0, top + row * 0.7, fs * 0.95, "middle", "Assets");
    chart::text(&mut d, xs[3], top + row * 0.7, fs, "middle", "=");
    chart::text(&mut d, xs[3] + cols[3] / 2.0, top + row * 0.7, fs * 0.95, "middle", "Liabilities");
    chart::text(&mut d, xs[4], top + row * 0.7, fs, "middle", "+");
    chart::text(&mut d, xs[4] + cols[4] / 2.0, top + row * 0.7, fs * 0.95, "middle", "Equity");
    let y0 = top + row;
    d.rect(x0, y0, tw, row, head, INK, 1.6);
    chart::text(&mut d, xs[0] + 10.0, y0 + row * 0.68, fs, "start", "#");
    chart::text(&mut d, xs[1] + 10.0, y0 + row * 0.68, fs, "start", "Transaction");
    for (i, t) in ["Change", "Change", "Change"].iter().enumerate() {
        chart::text(&mut d, xs[i + 2] + cols[i + 2] - 10.0, y0 + row * 0.68, fs, "end", t);
    }
    let mut sum = [0.0; 3];
    let mut bad_rows: Vec<usize> = Vec::new();
    for (i, t) in txs.iter().enumerate() {
        let y = y0 + row * (i as f64 + 1.0);
        chart::text(&mut d, xs[0] + 10.0, y + row * 0.68, fs, "start", &(i + 1).to_string());
        chart::text(&mut d, xs[1] + 10.0, y + row * 0.68, fs * 0.95, "start", &t.text);
        for c in 0..3 {
            sum[c] += t.d[c];
            if answers {
                chart::text(&mut d, xs[c + 2] + cols[c + 2] - 10.0, y + row * 0.68, fs, "end", &signed(t.d[c], cents));
            }
        }
        if (t.d[0] - t.d[1] - t.d[2]).abs() > 0.005 {
            bad_rows.push(i + 1);
        }
        if i > 0 {
            d.line(x0, y, x0 + tw, y, "#c9ced6", 0.8);
        }
    }
    let bottom = y0 + row * (n + 1.0);
    let end = bottom + if totals { row * 1.3 } else { 0.0 };
    for x in xs.iter().skip(1) {
        d.line(*x, y0, *x, end, INK, 1.2);
    }
    d.line(x0, bottom, x0 + tw, bottom, INK, 1.6);
    d.rect(x0, y0, tw, end - y0, "none", INK, 1.8);
    if totals {
        chart::text(&mut d, xs[1] + 10.0, bottom + row * 0.85, fs, "start", "Totals");
        for c in 0..3 {
            if answers {
                chart::text(&mut d, xs[c + 2] + cols[c + 2] - 10.0, bottom + row * 0.85, fs, "end", &money(sum[c], sym, cents));
            } else {
                d.rect(xs[c + 2] + 12.0, bottom + 6.0, cols[c + 2] - 22.0, row * 1.3 - 12.0, "#fff", INK, 1.0);
            }
        }
        let cy = end + row * 1.1;
        if answers {
            let t = if !bad_rows.is_empty() {
                format!("Transaction {} does not balance: the change in assets must equal liabilities plus equity", bad_rows.iter().map(usize::to_string).collect::<Vec<_>>().join(", "))
            } else if (sum[0] - sum[1] - sum[2]).abs() < 0.005 {
                format!("Assets {} = liabilities {} + equity {}", money(sum[0], sym, cents), money(sum[1], sym, cents), money(sum[2], sym, cents))
            } else {
                "The totals do not balance".to_string()
            };
            chart::text(&mut d, x0, cy, fs * if bad_rows.is_empty() { 1.0 } else { 0.9 }, "start", &t);
        } else {
            chart::text(&mut d, x0, cy, fs, "start", "Does the equation balance?");
            d.line(x0 + fs * 12.5, cy + 2.0, x0 + tw, cy + 2.0, INK, 1.0);
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
    fn spec_parses_effects_by_letter() {
        let t = parse(DEFAULT_SPEC).unwrap();
        assert_eq!(t.len(), 5);
        assert_eq!((t[0].text.as_str(), t[0].d), ("Owner invests cash", [10000.0, 0.0, 10000.0]));
        assert_eq!(t[3].d, [-1000.0, -1000.0, 0.0]);
        assert_eq!(t[4].d, [0.0, 0.0, 0.0], "an asset swap nets to nothing");
        let u = parse("x: a \u{2212}5, E 5").unwrap();
        assert_eq!(u[0].d, [-5.0, 0.0, 5.0]);
    }

    #[test]
    fn key_writes_effects_and_balances() {
        let svg = render(&defaults());
        // assets 10000 + 3000 + 1800 - 1000 = 13800; liabilities 3000 - 1000 = 2000; equity 11800
        for t in [">+10,000<", ">\u{2212}1,000<", ">$13,800<", ">$2,000<", ">$11,800<", "Assets $13,800 = liabilities $2,000 + equity $11,800"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn a_row_that_does_not_balance_is_named() {
        let mut p = defaults();
        p.set_s("spec", "Good: A +5, E +5\nBad: A +100, L +20");
        assert!(render(&p).contains("Transaction 2 does not balance"));
    }

    #[test]
    fn student_copy_keeps_only_the_descriptions() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">Owner invests cash<") && student.contains("Does the equation balance?"));
        assert!(!student.contains(">+10,000<") && !student.contains("13,800") && key.contains("13,800"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [("no colon", "needs a description"), ("x: A", "letter and an amount"), ("x: Q +5", "should be A, L or E"), ("x: A +abc", "not an amount"), ("", "Add a transaction")] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let many: String = (0..11).map(|i| format!("t{i}: A +1, E +1\n")).collect();
        let mut p = defaults();
        p.set_s("spec", &many);
        assert!(render(&p).contains("limited to 10"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("accounting-equation").unwrap() {
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
        for spec in ["", "\u{0}", ":", "x:", "x: A +999999999999, A -999999999999", "\u{1f600}: A +1", "x: A +0.001"] {
            for answers in [true, false] {
                for totals in [true, false] {
                    let mut p = defaults();
                    p.set_s("spec", spec);
                    p.set_b("totals", totals);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?}");
                }
            }
        }
    }
}
