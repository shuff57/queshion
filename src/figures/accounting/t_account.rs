//! T-accounts: up to four ledger accounts with debits on the left and credits on the right, each
//! balanced off with the totals and the balance carried down. The entries are what a student is
//! given; the balancing figure, the totals and the balance are the answers, so the student copy
//! (Show answers off) leaves them as empty boxes and a blank balance line.
use super::{clip, cents_param, currency_param, digits, money, parse_amount, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ACCOUNTS: usize = 4;
const MAX_ENTRIES: usize = 12;
const MAX_SPEC: usize = 2500;
const DEFAULT_SPEC: &str = "account Cash\nDr 5000 Capital\nDr 800 Sales\nCr 1200 Rent\nCr 300 Wages\naccount Rent\nDr 1200 Cash";

pub fn def() -> FigureDef {
    FigureDef {
        id: "t-account",
        subject: Subject::Accounting,
        title: "T-accounts",
        blurb: "Ledger accounts as T-accounts with debits left and credits right, balanced off with the totals and the balance carried down, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("t-account", "descriptions") => true,
        ("t-account", "balance") => true,
        _ => {
            let _ = p;
            true
        }
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "t-account" => vec![
            Example { name: "Cash and rent", params: vec![] },
            Example {
                name: "One account",
                params: vec![("spec", s("account Bank\nDr 12000 Opening\nDr 2500 Sales\nCr 4000 Supplier\nCr 900 Wages\nCr 350 Rates")), ("title", s("Bank account"))],
            },
            Example { name: "Without descriptions", params: vec![("descriptions", Value::B(false))] },
            Example { name: "Blank balancing figures", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Accounts", DEFAULT_SPEC).hint("account Name, then Dr 500 text (debit, left) or Cr 200 text (credit, right) lines"),
        ParamDef::bool("descriptions", "Show the descriptions", true),
        ParamDef::bool("balance", "Balance off each account", true),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves empty boxes for the balancing figure and the totals, and a blank balance line"),
    ]
}

struct Entry {
    amount: f64,
    text: String,
}

struct Acct {
    name: String,
    dr: Vec<Entry>,
    cr: Vec<Entry>,
}

fn parse(spec: &str) -> Result<Vec<Acct>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The accounts are limited to {MAX_SPEC} characters"));
    }
    let mut out: Vec<Acct> = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (head, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        match head.to_ascii_lowercase().as_str() {
            "account" => {
                if rest.is_empty() {
                    return Err("An account needs a name: account Cash".into());
                }
                if out.len() >= MAX_ACCOUNTS {
                    return Err(format!("A figure is limited to {MAX_ACCOUNTS} accounts"));
                }
                out.push(Acct { name: clip(rest, 22), dr: vec![], cr: vec![] });
            }
            side @ ("dr" | "cr") => {
                let acct = out.last_mut().ok_or("Start with an account line: account Cash")?;
                let (amt, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                let amount = parse_amount(amt).ok_or_else(|| format!("\u{201c}{amt}\u{201d} is not an amount; a debit or credit is never negative"))?;
                let list = if side == "dr" { &mut acct.dr } else { &mut acct.cr };
                if list.len() >= MAX_ENTRIES {
                    return Err(format!("{} has more than {MAX_ENTRIES} entries on one side", acct.name));
                }
                list.push(Entry { amount, text: clip(text, 12) });
            }
            _ => return Err(format!("\u{201c}{line}\u{201d} should start with account, Dr or Cr")),
        }
    }
    Ok(out)
}

/// How an account balances off: the totals each side is brought to, the side that needs the
/// balancing figure (true = credit side) and its amount, and the balance that is carried down.
struct Balance {
    total: f64,
    on_credit_side: bool,
    figure: f64,
    carried_dr: bool,
}

fn balance(a: &Acct) -> Balance {
    let (dr, cr): (f64, f64) = (a.dr.iter().map(|e| e.amount).sum(), a.cr.iter().map(|e| e.amount).sum());
    Balance { total: dr.max(cr), on_credit_side: dr > cr, figure: (dr - cr).abs(), carried_dr: dr >= cr }
}

fn render(p: &Params) -> String {
    let accts = match parse(p.s("spec")) {
        Ok(a) => a,
        Err(e) => return chart::message(&e),
    };
    if accts.is_empty() {
        return chart::message("Add an account: account Cash");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (sym, cents) = (symbol(p), p.b("cents"));
    let title = p.s("title").trim();
    let balanced = p.b("balance");
    let descs = p.b("descriptions");

    let n = accts.len();
    let cols = if n == 1 { 1 } else { 2 };
    let aw = 320.0;
    let gap = 36.0;
    let w = 40.0 + cols as f64 * aw + (cols - 1) as f64 * gap;
    let row = fs * 1.75;
    let head = fs * 2.0;
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 1.3 };
    // every account in a row of the page is as tall as the tallest, so rows line up
    let rows_of = |a: &Acct| a.dr.len().max(a.cr.len()) + usize::from(balanced);
    let heights: Vec<f64> = accts.iter().map(|a| head + row * (rows_of(a) as f64 + 1.0) + row * 1.5 + 14.0).collect();
    let grid_rows = n.div_ceil(cols);
    let row_h: Vec<f64> = (0..grid_rows).map(|r| (0..cols).filter_map(|c| heights.get(r * cols + c)).cloned().fold(0.0, f64::max)).collect();
    let h = top + row_h.iter().sum::<f64>() + 10.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let head_fill = if print { "#e5e5e5" } else { "#f4eee2" };
    let mut y0 = top;
    for (r, rh) in row_h.iter().enumerate() {
        for c in 0..cols {
            let Some(a) = accts.get(r * cols + c) else { continue };
            let x0 = 20.0 + c as f64 * (aw + gap);
            let cx = x0 + aw / 2.0;
            let bal = balance(a);
            d.rect(x0, y0, aw, head, head_fill, INK, 1.6);
            chart::text(&mut d, cx, y0 + head * 0.68, fs * 1.1, "middle", &a.name);
            let body = y0 + head;
            let main_rows = rows_of(a);
            let tot_y = body + row * main_rows as f64;
            d.line(cx, body, cx, tot_y + row, INK, 1.8);
            d.line(x0, body, x0 + aw, body, INK, 1.8);
            let amt = |d: &mut Doc, x: f64, y: f64, anchor: &str, v: f64| chart::text(d, x, y, fs, anchor, &digits(v, cents));
            let desc = |d: &mut Doc, x: f64, y: f64, anchor: &str, t: &str| {
                if descs && !t.is_empty() {
                    chart::text(d, x, y, fs * 0.82, anchor, t);
                }
            };
            for (i, e) in a.dr.iter().enumerate() {
                let y = body + row * (i as f64 + 0.72);
                desc(&mut d, x0 + 8.0, y, "start", &e.text);
                amt(&mut d, cx - 10.0, y, "end", e.amount);
            }
            for (i, e) in a.cr.iter().enumerate() {
                let y = body + row * (i as f64 + 0.72);
                desc(&mut d, cx + 10.0, y, "start", &e.text);
                amt(&mut d, x0 + aw - 8.0, y, "end", e.amount);
            }
            if balanced {
                // the balancing figure goes on the side with the smaller total; an account
                // that already balances has none
                if bal.figure > 1e-9 {
                    let (list_len, x, anchor, dx) = if bal.on_credit_side { (a.cr.len(), x0 + aw - 8.0, "end", cx + 10.0) } else { (a.dr.len(), cx - 10.0, "end", x0 + 8.0) };
                    let y = body + row * (list_len as f64 + 0.72);
                    if answers {
                        desc(&mut d, dx, y, "start", "Balance c/d");
                        amt(&mut d, x, y, anchor, bal.figure);
                    }
                }
                // totals: ruled above and double-ruled below, on both sides
                for (xa, xb) in [(x0, cx), (cx, x0 + aw)] {
                    d.line(xa + 6.0, tot_y + 3.0, xb - 6.0, tot_y + 3.0, INK, 1.2);
                    d.line(xa + 6.0, tot_y + row - 4.0, xb - 6.0, tot_y + row - 4.0, INK, 1.2);
                    d.line(xa + 6.0, tot_y + row - 1.0, xb - 6.0, tot_y + row - 1.0, INK, 1.2);
                    if answers {
                        let right = xb > cx;
                        let x = if right { xb - 8.0 } else { xb - 10.0 };
                        chart::text(&mut d, x, tot_y + row * 0.75, fs, "end", &digits(bal.total, cents));
                    } else {
                        let right = xb > cx;
                        let x = if right { xb - 8.0 } else { xb - 10.0 };
                        d.rect(x - 74.0, tot_y + 6.0, 74.0, row - 12.0, "#fff", INK, 1.0);
                    }
                }
            } else {
                for (xa, xb, list) in [(x0, cx, &a.dr), (cx, x0 + aw, &a.cr)] {
                    let total: f64 = list.iter().map(|e| e.amount).sum();
                    let right = xb > cx;
                    let x = if right { xb - 8.0 } else { xb - 10.0 };
                    d.line(xa + 6.0, tot_y + 3.0, xb - 6.0, tot_y + 3.0, INK, 1.2);
                    if answers {
                        chart::text(&mut d, x, tot_y + row * 0.75, fs, "end", &digits(total, cents));
                    } else {
                        d.rect(x - 74.0, tot_y + 6.0, 74.0, row - 12.0, "#fff", INK, 1.0);
                    }
                }
            }
            // the balance carried down, below the account, written the same way in both copies
            let by = tot_y + row + row * 0.95;
            if answers {
                let side = if bal.carried_dr { "debit" } else { "credit" };
                let text = if bal.figure < 1e-9 { "Balance nil".to_string() } else { format!("Balance b/d {} {side}", money(bal.figure, sym, cents)) };
                chart::text(&mut d, cx, by, fs, "middle", &text);
            } else {
                chart::text(&mut d, x0 + 8.0, by, fs, "start", "Balance b/d");
                d.line(x0 + 8.0 + fs * 5.6, by + 2.0, x0 + aw - 8.0, by + 2.0, INK, 1.0);
            }
        }
        y0 += rh;
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn spec_parses_accounts_and_sides() {
        let a = parse(DEFAULT_SPEC).unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].name.as_str(), a[0].dr.len(), a[0].cr.len()), ("Cash", 2, 2));
        assert_eq!((a[0].dr[0].amount, a[0].dr[0].text.as_str()), (5000.0, "Capital"));
        assert_eq!(a[1].dr[0].amount, 1200.0);
        let b = parse("account X\nDR $1,200.50 big sale").unwrap();
        assert_eq!(b[0].dr[0].amount, 1200.5);
    }

    #[test]
    fn balancing_figure_goes_on_the_smaller_side() {
        // cash: debits 5800, credits 1500, so 4300 is carried down as a debit balance
        let a = parse(DEFAULT_SPEC).unwrap();
        let b = balance(&a[0]);
        assert_eq!((b.total, b.figure, b.on_credit_side, b.carried_dr), (5800.0, 4300.0, true, true));
        let r = balance(&a[1]);
        assert_eq!((r.total, r.figure, r.on_credit_side), (1200.0, 1200.0, true));
        let c = parse("account Z\nCr 700 x\nDr 200 y").unwrap();
        let b = balance(&c[0]);
        assert_eq!((b.total, b.figure, b.on_credit_side, b.carried_dr), (700.0, 500.0, false, false));
        let eq = parse("account E\nDr 5\nCr 5").unwrap();
        assert!(balance(&eq[0]).figure < 1e-9);
    }

    #[test]
    fn key_writes_the_totals_and_the_balance() {
        let svg = render(&defaults());
        for t in [">Cash<", ">Rent<", ">5,800<", ">4,300<", ">Balance c/d<", ">Balance b/d $4,300 debit<", ">Balance b/d $1,200 debit<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn student_copy_leaves_boxes_at_full_size() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(key.contains(">5,800<") && !student.contains(">5,800<") && !student.contains(">4,300<"));
        assert!(student.contains(">5,000<") && student.contains(">Capital<"), "the entries are the question");
        assert!(student.contains(">Balance b/d<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [
            ("Dr 5 x", "Start with an account"),
            ("account\nDr 5", "needs a name"),
            ("account A\nDr abc x", "not an amount"),
            ("account A\nDr -5 x", "not an amount"),
            ("account A\nbuy 5", "should start with"),
            ("", "Add an account"),
            ("account A\naccount B\naccount C\naccount D\naccount E", "limited to 4 accounts"),
        ] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
    }

    #[test]
    fn other_options_change_the_drawing() {
        let mut p = defaults();
        p.set_b("balance", false);
        let svg = render(&p);
        assert!(svg.contains(">5,800<") && svg.contains(">1,500<") && !svg.contains(">Balance c/d<"));
        p.set_b("balance", true);
        p.set_b("descriptions", false);
        assert!(!render(&p).contains(">Capital<"));
        p.set_s("currency", "\u{20ac}");
        p.set_b("cents", true);
        let svg = render(&p);
        assert!(svg.contains("\u{20ac}4,300.00 debit") && svg.contains(">5,800.00<"));
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
        for ex in examples("t-account").unwrap() {
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
        let many: String = (0..14).map(|i| format!("Dr {i} line\n")).collect();
        for spec in ["", "\u{0}", "account A", "account A\nDr 999999999999 x\nCr 1 y", &format!("account A\n{many}"), "account A\nDr 0.001 x", "account \u{1f600}\nCr 5 \u{1f600}"] {
            for bal in [true, false] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("spec", spec);
                    p.set_b("balance", bal);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?}");
                }
            }
        }
    }
}
