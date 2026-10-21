//! Financial statements: an income statement (revenue less expenses, net income) or a balance
//! sheet (assets against liabilities and equity), laid out in the standard two-column form from
//! a list of accounts and amounts. The accounts and amounts are what a student is given; the
//! section totals, the net income or the totals of each side and the check are the answers, so
//! the student copy (Show answers off) leaves them as empty boxes.
use super::{cents_param, clip, currency_param, digits, money, parse_amount, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_SECTIONS: usize = 6;
const MAX_ITEMS: usize = 10;
const MAX_LINES: usize = 30;
const MAX_SPEC: usize = 2500;
const KINDS: [&str; 2] = ["Income statement", "Balance sheet"];
const IS_SPEC: &str = "Revenue\nSales 52000\nService fees 3000\nExpenses\nWages 21000\nRent 8400\nUtilities 2350";
const BS_SPEC: &str = "Assets\nCash 4300\nAccounts receivable 2100\nEquipment 1800\nLiabilities\nAccounts payable 900\nEquity\nCapital 5000\nRetained earnings 2300";

pub fn def() -> FigureDef {
    FigureDef {
        id: "financial-statement",
        subject: Subject::Accounting,
        title: "Income statement and balance sheet",
        blurb: "An income statement or a balance sheet in the standard layout from a list of accounts and amounts, with the totals and the net income or the balance check, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("financial-statement", "is_spec") => p.s("kind") == KINDS[0],
        ("financial-statement", "bs_spec") => p.s("kind") == KINDS[1],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "financial-statement" => vec![
            Example { name: "Income statement", params: vec![("business", s("Bright Print Co."))] },
            Example { name: "Balance sheet", params: vec![("kind", s(KINDS[1])), ("business", s("Bright Print Co.")), ("period", s("At 31 December"))] },
            Example { name: "A net loss", params: vec![("is_spec", s("Revenue\nSales 9000\nExpenses\nWages 8000\nRent 3500"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Statement", &KINDS, KINDS[0]),
        ParamDef::long("is_spec", "Accounts", IS_SPEC).hint("A heading line (Revenue), then Account amount lines. The first section is revenue; every later section is an expense"),
        ParamDef::long("bs_spec", "Accounts", BS_SPEC).hint("A heading line (Assets), then Account amount lines. The first section is assets; every later section is liabilities or equity"),
        ParamDef::text("business", "Business name", "Bright Print Co."),
        ParamDef::text("period", "Period", "For the year ended 31 December").hint("For a balance sheet write the date instead, such as At 31 December"),
        currency_param(),
        cents_param(),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the accounts and amounts and leaves the totals, the net income and the check empty for students"),
    ]
}

struct Section {
    name: String,
    items: Vec<(String, f64)>,
}

fn parse(spec: &str) -> Result<Vec<Section>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The accounts are limited to {MAX_SPEC} characters"));
    }
    let mut out: Vec<Section> = Vec::new();
    let mut lines = 0;
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        lines += 1;
        if lines > MAX_LINES {
            return Err(format!("A statement is limited to {MAX_LINES} lines"));
        }
        let amount = line.rsplit_once(char::is_whitespace).and_then(|(n, a)| parse_amount(a).map(|v| (n.trim(), v)));
        match amount {
            Some((name, v)) => {
                let sec = out.last_mut().ok_or("Start with a heading line such as Revenue")?;
                if sec.items.len() >= MAX_ITEMS {
                    return Err(format!("{} has more than {MAX_ITEMS} accounts", sec.name));
                }
                sec.items.push((clip(name, 30), v));
            }
            None => {
                if out.len() >= MAX_SECTIONS {
                    return Err(format!("A statement is limited to {MAX_SECTIONS} sections"));
                }
                out.push(Section { name: clip(line, 30), items: vec![] });
            }
        }
    }
    Ok(out)
}

fn render(p: &Params) -> String {
    let income = p.s("kind") == KINDS[0];
    let secs = match parse(p.s(if income { "is_spec" } else { "bs_spec" })) {
        Ok(s) => s,
        Err(e) => return chart::message(&e),
    };
    if secs.is_empty() || secs.iter().all(|s| s.items.is_empty()) {
        return chart::message("Add a heading and some accounts: Revenue, then Sales 5000");
    }
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (sym, cents) = (symbol(p), p.b("cents"));
    let (business, period) = (clip(p.s("business"), 40), clip(p.s("period"), 44));
    let w = 560.0;
    let (item_x, total_x) = (w - 200.0, w - 40.0);
    let row = fs * 1.85;
    let total = |s: &Section| s.items.iter().map(|i| i.1).sum::<f64>();
    let _ = print;

    // the rows, top to bottom: (kind, label, indent, amount or none, in which column)
    enum R {
        Head(String),
        Item(String, f64),
        Total(String, f64),
        Grand(String, f64),
        Gap,
    }
    let mut rows: Vec<R> = Vec::new();
    let mut result_text: Option<String> = None;
    if income {
        let rev = total(&secs[0]);
        let exp: f64 = secs.iter().skip(1).map(total).sum();
        for (i, s) in secs.iter().enumerate() {
            rows.push(R::Head(s.name.clone()));
            for (n, v) in &s.items {
                rows.push(R::Item(n.clone(), *v));
            }
            rows.push(R::Total(format!("Total {}", s.name.to_lowercase()), total(s)));
            if i + 1 < secs.len() {
                rows.push(R::Gap);
            }
        }
        if secs.len() > 2 {
            rows.push(R::Total("Total expenses".into(), exp));
        }
        rows.push(R::Gap);
        let net = rev - exp;
        rows.push(R::Grand(if net < 0.0 && answers { "Net loss".into() } else { "Net income".into() }, net));
    } else {
        let assets = total(&secs[0]);
        let right: f64 = secs.iter().skip(1).map(total).sum();
        for (i, s) in secs.iter().enumerate() {
            rows.push(R::Head(s.name.clone()));
            for (n, v) in &s.items {
                rows.push(R::Item(n.clone(), *v));
            }
            if i == 0 {
                rows.push(R::Grand("Total assets".into(), assets));
            } else {
                rows.push(R::Total(format!("Total {}", s.name.to_lowercase()), total(s)));
            }
            if i + 1 < secs.len() {
                rows.push(R::Gap);
            }
        }
        rows.push(R::Gap);
        rows.push(R::Grand("Total liabilities and equity".into(), right));
        let diff = assets - right;
        result_text = Some(if diff.abs() < 0.005 { "Assets equal liabilities plus equity".into() } else { format!("Out of balance by {}: {} are larger", money(diff.abs(), sym, cents), if diff > 0.0 { "assets" } else { "liabilities and equity" }) });
    }
    let head_h = if business.is_empty() { 0.0 } else { fs * 1.9 } + fs * 1.9 + if period.is_empty() { 0.0 } else { fs * 1.7 } + 8.0;
    let rows_h: f64 = rows.iter().map(|r| if matches!(r, R::Gap) { row * 0.45 } else { row }).sum();
    let h = 14.0 + head_h + rows_h + row * 1.6 + 10.0;
    let mut d = Doc::new(w, h);
    let mut y = 14.0;
    if !business.is_empty() {
        chart::text(&mut d, w / 2.0, y + fs * 1.1, fs * 1.25, "middle", &business);
        y += fs * 1.9;
    }
    chart::text(&mut d, w / 2.0, y + fs * 1.1, fs * 1.15, "middle", if income { "Income statement" } else { "Balance sheet" });
    y += fs * 1.9;
    if !period.is_empty() {
        chart::text(&mut d, w / 2.0, y + fs * 0.9, fs, "middle", &period);
        y += fs * 1.7;
    }
    y += 8.0;
    d.line(30.0, y, w - 30.0, y, INK, 1.6);
    let mut grand_seen = 0;
    for r in &rows {
        match r {
            R::Gap => y += row * 0.45,
            R::Head(t) => {
                d.raw(&format!(
                    r##"<text x="30" y="{:.2}" font-size="{fs}" font-weight="bold" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
                    y + row * 0.72,
                    svg::esc(t)
                ));
                y += row;
            }
            R::Item(n, v) => {
                chart::text(&mut d, 46.0, y + row * 0.72, fs, "start", n);
                chart::text(&mut d, item_x, y + row * 0.72, fs, "end", &digits(*v, cents));
                y += row;
            }
            R::Total(n, v) => {
                chart::text(&mut d, 46.0, y + row * 0.72, fs, "start", n);
                d.line(item_x - 60.0, y, item_x + 2.0, y, INK, 1.1);
                if answers {
                    chart::text(&mut d, total_x, y + row * 0.72, fs, "end", &digits(*v, cents));
                } else {
                    d.rect(total_x - 90.0, y + 3.0, 90.0, row - 6.0, "#fff", INK, 1.0);
                }
                y += row;
            }
            R::Grand(n, v) => {
                d.raw(&format!(
                    r##"<text x="30" y="{:.2}" font-size="{fs}" font-weight="bold" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
                    y + row * 0.72,
                    svg::esc(n)
                ));
                d.line(total_x - 96.0, y, total_x + 2.0, y, INK, 1.4);
                if answers {
                    chart::text(&mut d, total_x, y + row * 0.72, fs, "end", &money(*v, sym, cents));
                } else {
                    d.rect(total_x - 90.0, y + 3.0, 90.0, row - 6.0, "#fff", INK, 1.0);
                }
                d.line(total_x - 96.0, y + row - 2.0, total_x + 2.0, y + row - 2.0, INK, 1.2);
                d.line(total_x - 96.0, y + row + 1.0, total_x + 2.0, y + row + 1.0, INK, 1.2);
                y += row;
                grand_seen += 1;
            }
        }
    }
    let _ = grand_seen;
    if let Some(t) = result_text {
        let cy = y + row * 1.2;
        if answers {
            chart::text(&mut d, 30.0, cy, fs, "start", &t);
        } else {
            chart::text(&mut d, 30.0, cy, fs, "start", "Check:");
            d.line(30.0 + fs * 3.6, cy + 2.0, w - 30.0, cy + 2.0, INK, 1.0);
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
    fn spec_parses_sections_and_accounts() {
        let s = parse(IS_SPEC).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].name.as_str(), s[0].items.len(), s[1].items.len()), ("Revenue", 2, 3));
        assert_eq!(s[0].items[0], ("Sales".to_string(), 52000.0));
        let t = parse("Assets\nAccounts receivable 2,100.50").unwrap();
        assert_eq!(t[0].items[0], ("Accounts receivable".to_string(), 2100.5));
    }

    #[test]
    fn income_statement_works_out_net_income() {
        let svg = render(&defaults());
        // revenue 55000, expenses 21000 + 8400 + 2350 = 31750, net 23250
        for t in [">55,000<", ">31,750<", ">$23,250<", ">Net income<", ">Total revenue<", ">Bright Print Co.<"] {
            assert!(svg.contains(t), "{t}");
        }
        let mut p = defaults();
        p.set_s("is_spec", "Revenue\nSales 9000\nExpenses\nWages 8000\nRent 3500");
        let svg = render(&p);
        assert!(svg.contains(">Net loss<") && svg.contains("($2,500)"));
    }

    #[test]
    fn balance_sheet_checks_the_two_sides() {
        let mut p = defaults();
        p.set_s("kind", KINDS[1]);
        let svg = render(&p);
        // assets 8200; liabilities 900 + equity 7300 = 8200
        assert_eq!(svg.matches(">$8,200<").count(), 2);
        assert!(svg.contains("Assets equal liabilities plus equity"));
        p.set_s("bs_spec", "Assets\nCash 5000\nLiabilities\nLoan 1000\nEquity\nCapital 3000");
        assert!(render(&p).contains("Out of balance by $1,000: assets are larger"));
    }

    #[test]
    fn student_copy_leaves_the_totals_empty() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">52,000<") && student.contains(">Wages<"));
        assert!(!student.contains(">55,000<") && !student.contains("23,250") && key.contains("23,250"));
        assert_eq!(size_of(&student), size_of(&key));
        let mut p = defaults();
        p.set_s("kind", KINDS[1]);
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(student.contains("Check:") && !student.contains("Assets equal") && key.contains("Assets equal"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [("Sales 5000", "Start with a heading"), ("", "Add a heading"), ("Revenue", "Add a heading")] {
            let mut p = defaults();
            p.set_s("is_spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let many: String = (0..12).map(|i| format!("A{i} 1\n")).collect();
        let mut p = defaults();
        p.set_s("is_spec", &format!("Revenue\n{many}"));
        assert!(render(&p).contains("more than 10 accounts"));
        let sections: String = (0..7).map(|i| format!("S{i}\nx 1\n")).collect();
        p.set_s("is_spec", &sections);
        assert!(render(&p).contains("limited to 6 sections"));
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
        for ex in examples("financial-statement").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<line") && !svg.contains("NaN"), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "Revenue\nSales 999999999999\nExpenses\nX 999999999999", "A\nB\nC", "x\ny 1\nz 2\nw 3\nv 4", "\u{1f600}\n\u{1f600} 5"] {
            for kind in KINDS {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("kind", kind);
                    p.set_s("is_spec", spec);
                    p.set_s("bs_spec", spec);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?} {kind}");
                }
            }
        }
    }
}
