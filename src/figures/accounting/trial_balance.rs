//! Trial balance: the account balances from the ledger, sorted into debit and credit columns with
//! the totals and a check that they agree. The balances are what a student is given; the columns,
//! totals and the difference are the answers, so the student copy (Show answers off) leaves the
//! two columns, both totals and the difference empty.
use super::{cents_param, clip, currency_param, digits, money, parse_amount, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ACCOUNTS: usize = 20;
const MAX_SPEC: usize = 2500;
const DEFAULT_SPEC: &str = "Cash Dr 4300\nAccounts receivable Dr 2100\nEquipment Dr 1800\nAccounts payable Cr 900\nCapital Cr 5000\nSales Cr 3350\nWages Dr 650\nRent Dr 400";

pub fn def() -> FigureDef {
    FigureDef {
        id: "trial-balance",
        subject: Subject::Accounting,
        title: "Trial balance",
        blurb: "A trial balance: ledger balances sorted into debit and credit columns with the totals and whether they agree, or with the columns and totals left empty for students.",
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
        "trial-balance" => vec![
            Example { name: "Eight accounts, balanced", params: vec![] },
            Example {
                name: "Out of balance",
                params: vec![("spec", s("Cash Dr 1200\nInventory Dr 800\nCapital Cr 1500\nSales Cr 450\nPurchases Dr 300\nExpenses Dr 250")), ("title", s("Trial balance at 31 March"))],
            },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Balances", DEFAULT_SPEC).hint("One account per line: Name Dr 500 or Name Cr 500 (the side the balance is on)"),
        ParamDef::bool("totals", "Write the totals and the difference", true),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the balances and leaves the debit and credit columns, the totals and the difference empty"),
    ]
}

struct Row {
    name: String,
    debit: bool,
    amount: f64,
}

fn parse(spec: &str) -> Result<Vec<Row>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The balances are limited to {MAX_SPEC} characters"));
    }
    let mut out = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(format!("\u{201c}{line}\u{201d} should be an account, Dr or Cr, and an amount"));
        }
        let amount = parse_amount(parts[parts.len() - 1]).ok_or_else(|| format!("\u{201c}{}\u{201d} is not an amount", parts[parts.len() - 1]))?;
        let side = parts[parts.len() - 2].to_ascii_lowercase();
        if side != "dr" && side != "cr" {
            return Err(format!("\u{201c}{}\u{201d} should be Dr or Cr", parts[parts.len() - 2]));
        }
        if out.len() >= MAX_ACCOUNTS {
            return Err(format!("A trial balance is limited to {MAX_ACCOUNTS} accounts"));
        }
        out.push(Row { name: clip(&parts[..parts.len() - 2].join(" "), 28), debit: side == "dr", amount });
    }
    Ok(out)
}

fn render(p: &Params) -> String {
    let rows = match parse(p.s("spec")) {
        Ok(r) => r,
        Err(e) => return chart::message(&e),
    };
    if rows.is_empty() {
        return chart::message("Add some balances: Cash Dr 4300");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (sym, cents) = (symbol(p), p.b("cents"));
    let title = p.s("title").trim();
    let totals = p.b("totals");
    let cols = [250.0, 110.0, 110.0, 110.0];
    let (x0, tw) = (20.0, cols.iter().sum::<f64>());
    let w = tw + 40.0;
    let row = fs * 1.9;
    let top = if title.is_empty() { 12.0 } else { 38.0 + fs * 1.3 };
    let n = rows.len() as f64;
    let table_h = row * (n + 1.0) + if totals { row * 1.3 } else { 0.0 };
    let h = top + table_h + if totals { row * 1.6 } else { 0.0 } + 14.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let xs: Vec<f64> = cols.iter().scan(x0, |a, c| { let v = *a; *a += c; Some(v) }).collect();
    let head = if print { "#e5e5e5" } else { "#f4eee2" };
    d.rect(x0, top, tw, row, head, INK, 1.6);
    for (i, t) in ["Account", "Balance", "Debit", "Credit"].iter().enumerate() {
        let (x, anchor) = if i == 0 { (xs[i] + 10.0, "start") } else { (xs[i] + cols[i] - 10.0, "end") };
        chart::text(&mut d, x, top + row * 0.68, fs, anchor, t);
    }
    let (mut dr, mut cr) = (0.0, 0.0);
    for (i, r) in rows.iter().enumerate() {
        let y = top + row * (i as f64 + 1.0);
        if r.debit { dr += r.amount } else { cr += r.amount }
        chart::text(&mut d, xs[0] + 10.0, y + row * 0.68, fs, "start", &r.name);
        chart::text(&mut d, xs[1] + cols[1] - 10.0, y + row * 0.68, fs, "end", &format!("{} {}", digits(r.amount, cents), if r.debit { "Dr" } else { "Cr" }));
        if answers {
            let col = if r.debit { 2 } else { 3 };
            chart::text(&mut d, xs[col] + cols[col] - 10.0, y + row * 0.68, fs, "end", &digits(r.amount, cents));
        }
        if i > 0 {
            d.line(x0, y, x0 + tw, y, "#c9ced6", 0.8);
        }
    }
    let bottom = top + row * (n + 1.0);
    let end = bottom + if totals { row * 1.3 } else { 0.0 };
    for x in xs.iter().skip(1) {
        d.line(*x, top, *x, end, INK, 1.2);
    }
    d.line(x0, bottom, x0 + tw, bottom, INK, 1.6);
    d.rect(x0, top, tw, end - top, "none", INK, 1.8);
    if totals {
        chart::text(&mut d, xs[0] + 10.0, bottom + row * 0.85, fs, "start", "Totals");
        for (col, v) in [(2, dr), (3, cr)] {
            if answers {
                chart::text(&mut d, xs[col] + cols[col] - 10.0, bottom + row * 0.85, fs, "end", &money(v, sym, cents));
            } else {
                d.rect(xs[col] + 12.0, bottom + 6.0, cols[col] - 22.0, row * 1.3 - 12.0, "#fff", INK, 1.0);
            }
        }
        let cy = end + row * 1.0;
        if answers {
            let diff = dr - cr;
            let text = if diff.abs() < 0.005 { "The totals agree".to_string() } else { format!("Out of balance by {}: {} exceed {}", money(diff.abs(), sym, cents), if diff > 0.0 { "debits" } else { "credits" }, if diff > 0.0 { "credits" } else { "debits" }) };
            chart::text(&mut d, x0, cy, fs, "start", &text);
        } else {
            chart::text(&mut d, x0, cy, fs, "start", "Difference:");
            d.line(x0 + fs * 6.0, cy + 2.0, x0 + tw, cy + 2.0, INK, 1.0);
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
    fn spec_parses_names_sides_and_amounts() {
        let r = parse(DEFAULT_SPEC).unwrap();
        assert_eq!(r.len(), 8);
        assert_eq!((r[1].name.as_str(), r[1].debit, r[1].amount), ("Accounts receivable", true, 2100.0));
        assert!(!r[3].debit);
    }

    #[test]
    fn default_totals_agree() {
        let svg = render(&defaults());
        // debits 4300 + 2100 + 1800 + 650 + 400 = 9250; credits 900 + 5000 + 3350 = 9250
        assert_eq!(svg.matches(">$9,250<").count(), 2, "both totals");
        assert!(svg.contains("The totals agree"));
        let mut p = defaults();
        p.set_s("spec", "Cash Dr 1200\nInventory Dr 800\nCapital Cr 1500\nSales Cr 450\nPurchases Dr 300\nExpenses Dr 250");
        let svg = render(&p);
        assert!(svg.contains("Out of balance by $600: debits exceed credits"));
    }

    #[test]
    fn student_copy_leaves_the_columns_empty() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">4,300 Dr<") && student.contains(">Cash<"));
        assert!(key.contains(">4,300<") && !student.contains(">4,300<"), "the columns are the answer");
        assert!(!student.contains("Out of balance") && student.contains("Difference:"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [("Cash 500", "should be an account"), ("Cash Dr abc", "not an amount"), ("Cash Db 500", "should be Dr or Cr"), ("", "Add some balances")] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let many: String = (0..21).map(|i| format!("A{i} Dr 1\n")).collect();
        let mut p = defaults();
        p.set_s("spec", &many);
        assert!(render(&p).contains("limited to 20"));
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
        for ex in examples("trial-balance").unwrap() {
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
        for spec in ["", "\u{0}", "a b c", "Cash Dr 999999999999\nX Cr 0.001", "\u{1f600} Dr 5", "Cash Dr 1\nCash Dr 1"] {
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
