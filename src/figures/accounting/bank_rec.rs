//! Bank reconciliation: the balance per the bank statement and the balance per the cash book side
//! by side, each carried to an adjusted balance by its own list of additions and deductions, with a
//! check that the two agree. The opening balances and the adjustments are what a student is given;
//! the two adjusted balances and the check are the answers, so the student copy (Show answers off)
//! leaves them as empty boxes and a blank line.
use super::{cents_param, clip, currency_param, digits, money, parse_amount, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ADJ: usize = 6;
const MAX_SPEC: usize = 2000;
const DEFAULT_SPEC: &str = "bank 5200\nbank + Deposits in transit 800\nbank - Outstanding cheques 750\nbook 5250\nbook + Interest earned 15\nbook - Bank charges 35\nbook - Dishonoured cheque 400\nbook + Direct credit 420";

pub fn def() -> FigureDef {
    FigureDef {
        id: "bank-reconciliation",
        subject: Subject::Accounting,
        title: "Bank reconciliation",
        blurb: "A bank reconciliation with the bank statement and the cash book side by side, each adjusted to the same balance, or with the adjusted balances and the check left empty for students.",
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
        "bank-reconciliation" => vec![
            Example { name: "Statement and cash book", params: vec![] },
            Example {
                name: "Timing differences only",
                params: vec![("spec", s("bank 4800\nbank + Deposit in transit 1200\nbank - Cheque 114 outstanding 350\nbank - Cheque 117 outstanding 90\nbook 5560")), ("title", s("Reconciliation at 30 June"))],
            },
            Example { name: "Does not reconcile", params: vec![("spec", s("bank 3000\nbank + Deposit in transit 500\nbook 3200"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Balances and adjustments", DEFAULT_SPEC).hint("bank 5200 and book 5250 give the two balances; bank + Name 800 adds to one side and book - Name 35 deducts from it"),
        ParamDef::bool("check", "Write the check", true),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the balances and adjustments and leaves the adjusted balances and the check empty for students"),
    ]
}

#[derive(Default)]
struct Side {
    opening: Option<f64>,
    adj: Vec<(bool, String, f64)>,
}

impl Side {
    fn adjusted(&self) -> f64 {
        self.opening.unwrap_or(0.0) + self.adj.iter().map(|(add, _, v)| if *add { *v } else { -*v }).sum::<f64>()
    }
}

fn parse(spec: &str) -> Result<(Side, Side), String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The adjustments are limited to {MAX_SPEC} characters"));
    }
    let (mut bank, mut book) = (Side::default(), Side::default());
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (head, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let side = match head.to_ascii_lowercase().trim_end_matches(':') {
            "bank" => &mut bank,
            "book" => &mut book,
            _ => return Err(format!("\u{201c}{line}\u{201d} should start with bank or book")),
        };
        let rest = rest.trim().trim_start_matches(':').trim();
        if let Some(v) = parse_amount(rest) {
            if side.opening.is_some() {
                return Err(format!("The {} balance is given twice", head.trim_end_matches(':').to_lowercase()));
            }
            side.opening = Some(v);
        } else if let Some(sign) = rest.chars().next().filter(|c| *c == '+' || *c == '-' || *c == '\u{2212}') {
            let body = rest[sign.len_utf8()..].trim();
            let (name, amt) = body.rsplit_once(char::is_whitespace).ok_or_else(|| format!("\u{201c}{line}\u{201d} needs a name and an amount, like bank + Deposit 800"))?;
            let v = parse_amount(amt).ok_or_else(|| format!("\u{201c}{amt}\u{201d} is not an amount"))?;
            if side.adj.len() >= MAX_ADJ {
                return Err(format!("Each side is limited to {MAX_ADJ} adjustments"));
            }
            side.adj.push((sign == '+', clip(name, 26), v));
        } else {
            return Err(format!("\u{201c}{line}\u{201d} should be a balance (bank 5200) or an adjustment (bank + Name 800)"));
        }
    }
    if bank.opening.is_none() || book.opening.is_none() {
        return Err("Give both balances: bank 5200 and book 5250".into());
    }
    Ok((bank, book))
}

fn render(p: &Params) -> String {
    let (bank, book) = match parse(p.s("spec")) {
        Ok(x) => x,
        Err(e) => return chart::message(&e),
    };
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (sym, cents) = (symbol(p), p.b("cents"));
    let title = p.s("title").trim();
    let aw = 316.0;
    let w = 20.0 * 3.0 + aw * 2.0;
    let row = fs * 1.85;
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 1.3 };
    let rows = bank.adj.len().max(book.adj.len()) + 2;
    let check = p.b("check");
    let h = top + row * (rows as f64 + 1.0) + if check { row * 2.2 } else { 0.0 } + 14.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let head = if print { "#e5e5e5" } else { "#f4eee2" };
    let (b1, b2) = (bank.adjusted(), book.adjusted());
    for (i, (side, name, adj_name)) in [(&bank, "Bank statement", "Adjusted bank balance"), (&book, "Cash book", "Adjusted cash book balance")].into_iter().enumerate() {
        let x0 = 20.0 + i as f64 * (aw + 20.0);
        d.rect(x0, top, aw, row, head, INK, 1.6);
        chart::text(&mut d, x0 + aw / 2.0, top + row * 0.68, fs * 1.05, "middle", name);
        let amt_x = x0 + aw - 10.0;
        let mut y = top + row;
        chart::text(&mut d, x0 + 10.0, y + row * 0.68, fs, "start", "Balance");
        chart::text(&mut d, amt_x, y + row * 0.68, fs, "end", &digits(side.opening.unwrap_or(0.0), cents));
        y += row;
        for (add, nm, v) in &side.adj {
            chart::text(&mut d, x0 + 10.0, y + row * 0.68, fs * 0.92, "start", &format!("{} {nm}", if *add { "Add:" } else { "Less:" }));
            chart::text(&mut d, amt_x, y + row * 0.68, fs, "end", &digits(*v, cents));
            y += row;
        }
        // the adjusted balance sits on the same line on both sides so they can be compared
        let yt = top + row * (rows as f64);
        d.line(x0 + aw - 120.0, yt, x0 + aw - 4.0, yt, INK, 1.2);
        chart::text(&mut d, x0 + 10.0, yt + row * 0.68, fs * 0.92, "start", adj_name);
        if answers {
            chart::text(&mut d, amt_x, yt + row * 0.68, fs, "end", &money(if i == 0 { b1 } else { b2 }, sym, cents));
        } else {
            d.rect(amt_x - 104.0, yt + 4.0, 104.0, row - 8.0, "#fff", INK, 1.0);
        }
        d.line(x0 + aw - 120.0, yt + row - 2.0, x0 + aw - 4.0, yt + row - 2.0, INK, 1.2);
        d.line(x0 + aw - 120.0, yt + row + 1.0, x0 + aw - 4.0, yt + row + 1.0, INK, 1.2);
        d.rect(x0, top, aw, row * (rows as f64 + 1.0), "none", INK, 1.8);
        let _ = y;
    }
    if check {
        let cy = top + row * (rows as f64 + 1.0) + row * 1.3;
        if answers {
            let diff = b1 - b2;
            let t = if diff.abs() < 0.005 { format!("Reconciled: both balances are {}", money(b1, sym, cents)) } else { format!("Not reconciled: the bank side is {} {} the cash book side", money(diff.abs(), sym, cents), if diff > 0.0 { "above" } else { "below" }) };
            chart::text(&mut d, 20.0, cy, fs, "start", &t);
        } else {
            chart::text(&mut d, 20.0, cy, fs, "start", "Do the balances agree?");
            d.line(20.0 + fs * 13.0, cy + 2.0, w - 20.0, cy + 2.0, INK, 1.0);
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
    fn spec_parses_balances_and_adjustments() {
        let (bank, book) = parse(DEFAULT_SPEC).unwrap();
        assert_eq!((bank.opening, bank.adj.len(), book.opening, book.adj.len()), (Some(5200.0), 2, Some(5250.0), 4));
        assert_eq!(bank.adj[1], (false, "Outstanding cheques".to_string(), 750.0));
        // bank: 5200 + 800 - 750 = 5250; book: 5250 + 15 - 35 - 400 + 420 = 5250
        assert_eq!(bank.adjusted(), 5250.0);
    }

    #[test]
    fn default_reconciles() {
        let (_, book) = parse(DEFAULT_SPEC).unwrap();
        assert_eq!(book.adjusted(), 5250.0);
        let svg = render(&defaults());
        assert!(svg.contains("Reconciled: both balances are $5,250"));
        let mut p = defaults();
        p.set_s("spec", "bank 5300\nbook 5200");
        assert!(render(&p).contains("Not reconciled: the bank side is $100 above the cash book side"));
        p.set_s("spec", "bank 5000\nbank + Deposit in transit 500\nbook 5400\nbook + Interest 100");
        let svg = render(&p);
        assert_eq!(svg.matches(">$5,500<").count(), 2, "both adjusted balances");
        assert!(svg.contains("Reconciled: both balances are $5,500"));
    }

    #[test]
    fn student_copy_leaves_the_adjusted_balances_empty() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">Add: Deposits in transit<") && student.contains(">5,200<") && student.contains(">Adjusted bank balance<"));
        assert!(!student.contains("$5,250") && key.contains("$5,250") && student.contains("Do the balances agree?"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [
            ("bank 5\nbook", "should be a balance"),
            ("bank 5", "Give both balances"),
            ("cash 5\nbook 6", "should start with bank or book"),
            ("bank 5\nbank 6\nbook 1", "given twice"),
            ("bank 5\nbook 1\nbank + Deposit", "needs a name and an amount"),
            ("bank 5\nbook 1\nbank + Deposit abc", "not an amount"),
            ("bank 5\nbook 1\nbank xyz", "should be a balance"),
            ("", "Give both balances"),
        ] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let many: String = (0..7).map(|i| format!("bank + A{i} 1\n")).collect();
        let mut p = defaults();
        p.set_s("spec", &format!("bank 1\nbook 1\n{many}"));
        assert!(render(&p).contains("limited to 6"));
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
        for ex in examples("bank-reconciliation").unwrap() {
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
        for spec in ["", "\u{0}", "bank 0\nbook 0", "bank 999999999999\nbook 0.001\nbank - X 999999999999", "bank : 5\nbook : 5", "bank 1\nbook 1\nbank + \u{1f600} 1"] {
            for answers in [true, false] {
                for check in [true, false] {
                    let mut p = defaults();
                    p.set_s("spec", spec);
                    p.set_b("check", check);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?}");
                }
            }
        }
    }
}
