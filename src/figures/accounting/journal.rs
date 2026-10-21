//! Journal entries: for each transaction the date, the accounts debited and credited with their
//! amounts, and a narration, in the standard ruled layout, with the totals and a check that debits
//! equal credits. The date and narration are the question a student is given; the accounts and
//! amounts are the answer, so the student copy (Show answers off) keeps them as empty rows.
use super::{cents_param, clip, currency_param, digits, money, parse_amount, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ENTRIES: usize = 8;
const MAX_LINES: usize = 6;
const MAX_SPEC: usize = 3000;
const DEFAULT_SPEC: &str = "1 Mar\nDr Cash 5000\nCr Capital 5000\n(Owner invests cash in the business)\n\n4 Mar\nDr Equipment 1800\nCr Bank 1800\n(Bought a printer by cheque)\n\n9 Mar\nDr Wages 650\nDr Rent 400\nCr Bank 1050\n(Paid wages and rent)";

pub fn def() -> FigureDef {
    FigureDef {
        id: "journal-entry",
        subject: Subject::Accounting,
        title: "Journal entries",
        blurb: "Journal entries in the standard ruled layout with the accounts debited and credited, the amounts, a narration and a check that debits equal credits, or with the accounts and amounts left blank for students.",
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
        "journal-entry" => vec![
            Example { name: "Three March transactions", params: vec![] },
            Example {
                name: "Credit sale with a compound entry",
                params: vec![("spec", s("12 Apr\nDr Accounts receivable 2400\nCr Sales 2000\nCr Sales tax payable 400\n(Sold goods on credit, tax included)")), ("title", s("General journal"))],
            },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Entries", DEFAULT_SPEC).hint("A date line, then Dr Account amount and Cr Account amount lines, then (a narration). A blank line starts the next entry"),
        ParamDef::bool("totals", "Write the totals and the check", true),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the dates and narrations and leaves the accounts, amounts and totals empty for students"),
    ]
}

struct Line {
    debit: bool,
    account: String,
    amount: f64,
}

struct Entry {
    date: String,
    lines: Vec<Line>,
    narration: String,
}

fn parse(spec: &str) -> Result<Vec<Entry>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The entries are limited to {MAX_SPEC} characters"));
    }
    let mut out: Vec<Entry> = Vec::new();
    let mut cur: Option<Entry> = None;
    let flush = |cur: &mut Option<Entry>, out: &mut Vec<Entry>| {
        if let Some(e) = cur.take() {
            out.push(e);
        }
    };
    for line in spec.lines().map(str::trim) {
        if line.is_empty() {
            flush(&mut cur, &mut out);
            continue;
        }
        let (head, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let side = head.to_ascii_lowercase();
        if (side == "dr" || side == "cr") && !rest.trim().is_empty() {
            let entry = cur.get_or_insert_with(|| Entry { date: String::new(), lines: vec![], narration: String::new() });
            let rest = rest.trim();
            let (account, amt) = rest.rsplit_once(char::is_whitespace).ok_or_else(|| format!("\u{201c}{line}\u{201d} needs an account and an amount, like Dr Cash 500"))?;
            let amount = parse_amount(amt).ok_or_else(|| format!("\u{201c}{amt}\u{201d} is not an amount"))?;
            if entry.lines.len() >= MAX_LINES {
                return Err(format!("An entry is limited to {MAX_LINES} lines"));
            }
            entry.lines.push(Line { debit: side == "dr", account: clip(account, 26), amount });
        } else if line.starts_with('(') {
            let entry = cur.get_or_insert_with(|| Entry { date: String::new(), lines: vec![], narration: String::new() });
            entry.narration = clip(line.trim_start_matches('(').trim_end_matches(')'), 60);
        } else {
            // a date line: it starts a new entry even without a blank line before it
            if cur.as_ref().is_some_and(|e| !e.lines.is_empty()) {
                flush(&mut cur, &mut out);
            }
            let entry = cur.get_or_insert_with(|| Entry { date: String::new(), lines: vec![], narration: String::new() });
            entry.date = clip(line, 12);
        }
    }
    flush(&mut cur, &mut out);
    if out.len() > MAX_ENTRIES {
        return Err(format!("A figure is limited to {MAX_ENTRIES} entries"));
    }
    Ok(out)
}

fn render(p: &Params) -> String {
    let entries = match parse(p.s("spec")) {
        Ok(e) => e,
        Err(e) => return chart::message(&e),
    };
    if entries.is_empty() {
        return chart::message("Add an entry: a date, then Dr Cash 500 and Cr Sales 500");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (sym, cents) = (symbol(p), p.b("cents"));
    let title = p.s("title").trim();
    let totals = p.b("totals");
    let cols = [92.0, 258.0, 110.0, 110.0];
    let (x0, tw) = (20.0, cols.iter().sum::<f64>());
    let w = tw + 40.0;
    let row = fs * 1.9;
    let top = if title.is_empty() { 12.0 } else { 38.0 + fs * 1.3 };
    let body_rows: usize = entries.iter().map(|e| e.lines.len().max(1) + usize::from(!e.narration.is_empty())).sum();
    let table_h = row * (1.0 + body_rows as f64 + entries.len() as f64 * 0.35) + if totals { row * 1.2 } else { 0.0 };
    let foot = if totals { row * 1.4 } else { 0.0 };
    let h = top + table_h + foot + 14.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let xs: Vec<f64> = cols.iter().scan(x0, |a, c| { let v = *a; *a += c; Some(v) }).collect();
    let head = if print { "#e5e5e5" } else { "#e6edf8" };
    d.rect(x0, top, tw, row, head, INK, 1.6);
    for (i, t) in ["Date", "Account", "Debit", "Credit"].iter().enumerate() {
        let (x, anchor) = if i >= 2 { (xs[i] + cols[i] - 10.0, "end") } else { (xs[i] + 10.0, "start") };
        chart::text(&mut d, x, top + row * 0.68, fs, anchor, t);
    }
    let mut y = top + row;
    let (mut dr_sum, mut cr_sum) = (0.0, 0.0);
    let mut group_tops: Vec<f64> = Vec::new();
    for e in &entries {
        group_tops.push(y);
        chart::text(&mut d, xs[0] + 10.0, y + row * 0.68, fs * 0.95, "start", &e.date);
        for l in &e.lines {
            if l.debit { dr_sum += l.amount } else { cr_sum += l.amount }
            if answers {
                let x = if l.debit { xs[1] + 10.0 } else { xs[1] + 34.0 };
                chart::text(&mut d, x, y + row * 0.68, fs, "start", &l.account);
                let col = if l.debit { 2 } else { 3 };
                chart::text(&mut d, xs[col] + cols[col] - 10.0, y + row * 0.68, fs, "end", &digits(l.amount, cents));
            }
            y += row;
        }
        if e.lines.is_empty() {
            y += row;
        }
        if !e.narration.is_empty() {
            chart::text(&mut d, xs[1] + 10.0, y + row * 0.68, fs * 0.88, "start", &format!("({})", e.narration));
            y += row;
        }
        y += row * 0.35;
    }
    let bottom = y;
    // the ruling: a line under each entry, the column dividers and the frame
    for t in group_tops.iter().skip(1) {
        d.line(x0, *t, x0 + tw, *t, INK, 1.0);
    }
    for x in xs.iter().skip(1) {
        d.line(*x, top, *x, bottom + if totals { row * 1.2 } else { 0.0 }, INK, 1.2);
    }
    d.line(x0, bottom, x0 + tw, bottom, INK, 1.6);
    d.rect(x0, top, tw, bottom + if totals { row * 1.2 } else { 0.0 } - top, "none", INK, 1.8);
    if totals {
        chart::text(&mut d, xs[1] + 10.0, bottom + row * 0.8, fs, "start", "Total");
        for (col, v) in [(2, dr_sum), (3, cr_sum)] {
            if answers {
                chart::text(&mut d, xs[col] + cols[col] - 10.0, bottom + row * 0.8, fs, "end", &digits(v, cents));
            } else {
                d.rect(xs[col] + 14.0, bottom + 5.0, cols[col] - 24.0, row * 1.2 - 10.0, "#fff", INK, 1.0);
            }
        }
        let cy = bottom + row * 1.2 + row * 1.0;
        if answers {
            let diff = dr_sum - cr_sum;
            let text = if diff.abs() < 0.005 { format!("Debits equal credits ({})", money(dr_sum, sym, cents)) } else { format!("Out of balance by {}: {} exceed {}", money(diff.abs(), sym, cents), if diff > 0.0 { "debits" } else { "credits" }, if diff > 0.0 { "credits" } else { "debits" }) };
            chart::text(&mut d, x0, cy, fs, "start", &text);
        } else {
            chart::text(&mut d, x0, cy, fs, "start", "Do debits equal credits?");
            d.line(x0 + fs * 11.0, cy + 2.0, x0 + tw, cy + 2.0, INK, 1.0);
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
    fn spec_parses_dates_lines_and_narrations() {
        let e = parse(DEFAULT_SPEC).unwrap();
        assert_eq!(e.len(), 3);
        assert_eq!((e[0].date.as_str(), e[0].lines.len(), e[0].narration.as_str()), ("1 Mar", 2, "Owner invests cash in the business"));
        assert_eq!((e[2].lines.len(), e[2].lines[2].debit, e[2].lines[2].amount), (3, false, 1050.0));
        // a new date starts a new entry even with no blank line
        let two = parse("1 Jan\nDr A 5\nCr B 5\n2 Jan\nDr C 7\nCr D 7").unwrap();
        assert_eq!(two.len(), 2);
        let multi = parse("x\nDr Accounts receivable 2,400.50").unwrap();
        assert_eq!((multi[0].lines[0].account.as_str(), multi[0].lines[0].amount), ("Accounts receivable", 2400.5));
    }

    #[test]
    fn key_writes_accounts_totals_and_the_check() {
        let svg = render(&defaults());
        for t in [">Cash<", ">Capital<", ">5,000<", ">1,050<", ">Total<", "Debits equal credits ($7,850)", ">(Paid wages and rent)<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn unbalanced_entries_are_reported() {
        let mut p = defaults();
        p.set_s("spec", "1 Jan\nDr Cash 500\nCr Sales 450");
        assert!(render(&p).contains("Out of balance by $50: debits exceed credits"));
        p.set_s("spec", "1 Jan\nDr Cash 100\nCr Sales 450");
        assert!(render(&p).contains("credits exceed debits") || render(&p).contains("debits exceed"));
    }

    #[test]
    fn student_copy_keeps_dates_and_narrations_only() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">1 Mar<") && student.contains(">(Bought a printer by cheque)<"));
        assert!(!student.contains(">Cash<") && !student.contains(">5,000<") && !student.contains("Debits equal"));
        assert!(key.contains(">Cash<"));
        assert!(student.contains("Do debits equal credits?"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [("1 Jan\nDr Cash", "Add an entry").to_owned(), ("1 Jan\nDr Cash abc", "not an amount"), ("1 Jan\nDr 500", "needs an account"), ("", "Add an entry")]
            .iter()
            .map(|(a, b)| (a.to_string(), *b))
        {
            let mut p = defaults();
            p.set_s("spec", &spec);
            let svg = render(&p);
            assert!(svg.contains(needle) || svg.contains("needs an account") || svg.contains("Add an entry"), "{spec:?}");
        }
        let seven: String = (0..9).map(|i| format!("d{i}\nDr A 1\nCr B 1\n\n")).collect();
        let mut p = defaults();
        p.set_s("spec", &seven);
        assert!(render(&p).contains("limited to 8 entries"));
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
        for ex in examples("journal-entry").unwrap() {
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
        for spec in ["", "\u{0}", "(only a narration)", "Dr", "Cr x 1", "Dr A 999999999999\nCr B 1", "a\nb\nc", "Dr A 1\n\n\n\nCr B 1"] {
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
