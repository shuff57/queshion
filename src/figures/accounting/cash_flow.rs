//! Cash-flow statement: the operating, investing and financing activities with their inflows and
//! outflows, the net cash from each, the net increase or decrease in cash, and the opening and
//! closing cash. The activities and the opening cash are what a student is given; every total
//! and the closing cash are the answers, so the student copy (Show answers off) leaves them as
//! empty boxes.
use super::{cents_param, clip, currency_param, digits, money, parse_amount, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_SECTIONS: usize = 4;
const MAX_ITEMS: usize = 8;
const MAX_LINES: usize = 26;
const MAX_SPEC: usize = 2500;
const DEFAULT_SPEC: &str = "Operating activities\nCash received from customers 61000\nCash paid to suppliers -34000\nCash paid to employees -14500\nInvesting activities\nPurchase of equipment -9000\nSale of old van 2500\nFinancing activities\nOwner's investment 10000\nLoan repayment -3000";

pub fn def() -> FigureDef {
    FigureDef {
        id: "cash-flow-statement",
        subject: Subject::Accounting,
        title: "Cash-flow statement",
        blurb: "A cash-flow statement with operating, investing and financing activities, the net increase or decrease in cash and the closing cash, or with every total left empty for students.",
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
        "cash-flow-statement" => vec![
            Example { name: "A growing business", params: vec![("business", s("Bright Print Co."))] },
            Example {
                name: "A fall in cash",
                params: vec![("spec", s("Operating activities\nCash from customers 30000\nCash to suppliers (28000)\nCash to employees (9000)\nInvesting activities\nNew machine (12000)")), ("opening", Value::F(15000.0))],
            },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("spec", "Activities", DEFAULT_SPEC).hint("A heading line, then Description amount lines: a plain or + amount is cash in, a - amount or one in (brackets) is cash out"),
        ParamDef::float("opening", "Opening cash", -1.0e12, 1.0e12, 100.0, 6000.0),
        ParamDef::text("closing_given", "Closing cash per the balance sheet (optional)", "").hint("If you give it, the statement checks that it agrees with the closing cash worked out"),
        ParamDef::text("business", "Business name", "Bright Print Co."),
        ParamDef::text("period", "Period", "For the year ended 31 December"),
        currency_param(),
        cents_param(),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the activities and the opening cash and leaves every total and the closing cash empty for students"),
    ]
}

struct Section {
    name: String,
    items: Vec<(String, f64)>,
}

/// An amount that may be signed: +500, -500, (500) or 500.
fn parse_signed(t: &str) -> Option<f64> {
    let t = t.trim();
    if let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        return parse_amount(inner).map(|v| -v);
    }
    match t.chars().next()? {
        '-' | '\u{2212}' => parse_amount(&t[t.chars().next()?.len_utf8()..]).map(|v| -v),
        '+' => parse_amount(&t[1..]),
        _ => parse_amount(t),
    }
}

fn parse(spec: &str) -> Result<Vec<Section>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The activities are limited to {MAX_SPEC} characters"));
    }
    let mut out: Vec<Section> = Vec::new();
    let mut lines = 0;
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        lines += 1;
        if lines > MAX_LINES {
            return Err(format!("A statement is limited to {MAX_LINES} lines"));
        }
        let amount = line.rsplit_once(char::is_whitespace).and_then(|(n, a)| parse_signed(a).map(|v| (n.trim(), v)));
        match amount {
            Some((name, v)) => {
                let sec = out.last_mut().ok_or("Start with a heading line such as Operating activities")?;
                if sec.items.len() >= MAX_ITEMS {
                    return Err(format!("{} has more than {MAX_ITEMS} lines", sec.name));
                }
                sec.items.push((clip(name, 36), v));
            }
            None => {
                if out.len() >= MAX_SECTIONS {
                    return Err(format!("A statement is limited to {MAX_SECTIONS} sections"));
                }
                out.push(Section { name: clip(line, 36), items: vec![] });
            }
        }
    }
    Ok(out)
}

/// A figure with negatives in brackets, as cash flow statements write outflows.
fn bracket(v: f64, cents: bool) -> String {
    if v < -1e-9 { format!("({})", digits(-v, cents)) } else { digits(v, cents) }
}

fn render(p: &Params) -> String {
    let secs = match parse(p.s("spec")) {
        Ok(s) => s,
        Err(e) => return chart::message(&e),
    };
    if secs.iter().all(|s| s.items.is_empty()) {
        return chart::message("Add a heading and some cash flows: Operating activities, then Cash from customers 5000");
    }
    let opening = p.f("opening");
    if !(opening.is_finite() && opening.abs() <= 1.0e12) {
        return chart::message("Enter the opening cash as a number");
    }
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (sym, cents) = (symbol(p), p.b("cents"));
    let (business, period) = (clip(p.s("business"), 40), clip(p.s("period"), 44));
    let given_closing = parse_signed(p.s("closing_given"));
    let w = 600.0;
    let (item_x, total_x) = (w - 200.0, w - 40.0);
    let row = fs * 1.85;
    let total = |s: &Section| s.items.iter().map(|i| i.1).sum::<f64>();
    let net: f64 = secs.iter().map(total).sum();
    let closing = opening + net;

    enum R {
        Head(String),
        Item(String, f64),
        Total(String, f64),
        Given(String, f64),
        Grand(String, f64),
        Gap,
    }
    let mut rows: Vec<R> = Vec::new();
    for s in &secs {
        rows.push(R::Head(s.name.clone()));
        for (n, v) in &s.items {
            rows.push(R::Item(n.clone(), *v));
        }
        let what = s.name.to_lowercase();
        rows.push(R::Total(format!("Net cash from {}", what.trim_end_matches(" activities").to_string() + " activities"), total(s)));
        rows.push(R::Gap);
    }
    rows.push(R::Grand("Net increase (decrease) in cash".into(), net));
    rows.push(R::Given("Cash at the start of the period".into(), opening));
    rows.push(R::Grand("Cash at the end of the period".into(), closing));
    let head_h = if business.is_empty() { 0.0 } else { fs * 1.9 } + fs * 1.9 + if period.is_empty() { 0.0 } else { fs * 1.7 } + 8.0;
    let rows_h: f64 = rows.iter().map(|r| if matches!(r, R::Gap) { row * 0.45 } else { row }).sum();
    let h = 14.0 + head_h + rows_h + if given_closing.is_some() { row * 2.0 } else { 0.0 } + row * 0.8 + 10.0;
    let mut d = Doc::new(w, h);
    let mut y = 14.0;
    if !business.is_empty() {
        chart::text(&mut d, w / 2.0, y + fs * 1.1, fs * 1.25, "middle", &business);
        y += fs * 1.9;
    }
    chart::text(&mut d, w / 2.0, y + fs * 1.1, fs * 1.15, "middle", "Statement of cash flows");
    y += fs * 1.9;
    if !period.is_empty() {
        chart::text(&mut d, w / 2.0, y + fs * 0.9, fs, "middle", &period);
        y += fs * 1.7;
    }
    y += 8.0;
    d.line(30.0, y, w - 30.0, y, INK, 1.6);
    let bold = |d: &mut Doc, y: f64, t: &str| {
        d.raw(&format!(
            r##"<text x="30" y="{:.2}" font-size="{fs}" font-weight="bold" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
            y + row * 0.72,
            svg::esc(t)
        ));
    };
    let boxed = |d: &mut Doc, y: f64| {
        d.rect(total_x - 96.0, y + 3.0, 96.0, row - 6.0, "#fff", INK, 1.0);
    };
    for r in &rows {
        match r {
            R::Gap => y += row * 0.45,
            R::Head(t) => {
                bold(&mut d, y, t);
                y += row;
            }
            R::Item(n, v) => {
                chart::text(&mut d, 46.0, y + row * 0.72, fs, "start", n);
                chart::text(&mut d, item_x, y + row * 0.72, fs, "end", &bracket(*v, cents));
                y += row;
            }
            R::Total(n, v) => {
                chart::text(&mut d, 46.0, y + row * 0.72, fs, "start", n);
                d.line(item_x - 70.0, y, item_x + 2.0, y, INK, 1.1);
                if answers {
                    chart::text(&mut d, total_x, y + row * 0.72, fs, "end", &bracket(*v, cents));
                } else {
                    boxed(&mut d, y);
                }
                y += row;
            }
            R::Given(n, v) => {
                chart::text(&mut d, 46.0, y + row * 0.72, fs, "start", n);
                chart::text(&mut d, total_x, y + row * 0.72, fs, "end", &money(*v, sym, cents));
                y += row;
            }
            R::Grand(n, v) => {
                bold(&mut d, y, n);
                d.line(total_x - 100.0, y, total_x + 2.0, y, INK, 1.4);
                if answers {
                    chart::text(&mut d, total_x, y + row * 0.72, fs, "end", &money(*v, sym, cents));
                } else {
                    boxed(&mut d, y);
                }
                d.line(total_x - 100.0, y + row - 2.0, total_x + 2.0, y + row - 2.0, INK, 1.2);
                d.line(total_x - 100.0, y + row + 1.0, total_x + 2.0, y + row + 1.0, INK, 1.2);
                y += row;
            }
        }
    }
    if let Some(g) = given_closing {
        let cy = y + row * 1.1;
        if answers {
            let diff = closing - g;
            let t = if diff.abs() < 0.005 { format!("Agrees with the balance sheet cash of {}", money(g, sym, cents)) } else { format!("Does not agree with the balance sheet cash of {}: difference {}", money(g, sym, cents), money(diff.abs(), sym, cents)) };
            chart::text(&mut d, 30.0, cy, fs, "start", &t);
        } else {
            chart::text(&mut d, 30.0, cy, fs, "start", "Agrees with the balance sheet?");
            d.line(30.0 + fs * 16.5, cy + 2.0, w - 30.0, cy + 2.0, INK, 1.0);
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
    fn signed_amounts_are_read() {
        for (t, want) in [("500", 500.0), ("+500", 500.0), ("-500", -500.0), ("(500)", -500.0), ("\u{2212}1,200.5", -1200.5), ("(1,000)", -1000.0)] {
            assert_eq!(parse_signed(t), Some(want), "{t}");
        }
        for t in ["", "x", "(", "--5", "(-5)", "()"] {
            assert_eq!(parse_signed(t), None, "{t:?}");
        }
    }

    #[test]
    fn default_works_out_each_section_and_the_closing_cash() {
        let svg = render(&defaults());
        // operating 61000 - 34000 - 14500 = 12500; investing -6500; financing 7000; net 13000; closing 19000
        for t in [">12,500<", ">(6,500)<", ">7,000<", ">$13,000<", ">$19,000<", ">$6,000<", ">Net cash from operating activities<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn closing_cash_is_checked_against_the_balance_sheet() {
        let mut p = defaults();
        p.set_s("closing_given", "19000");
        assert!(render(&p).contains("Agrees with the balance sheet cash of $19,000"));
        p.set_s("closing_given", "18500");
        assert!(render(&p).contains("Does not agree with the balance sheet cash of $18,500: difference $500"));
    }

    #[test]
    fn a_fall_in_cash_shows_in_brackets() {
        let mut p = defaults();
        p.set_s("spec", "Operating activities\nCash from customers 30000\nCash to suppliers (28000)\nInvesting activities\nNew machine (12000)");
        p.set_f("opening", 15000.0);
        let svg = render(&p);
        assert!(svg.contains("($10,000)") && svg.contains(">$5,000<"), "net decrease and closing cash");
    }

    #[test]
    fn student_copy_leaves_the_totals_empty() {
        let def = def();
        let mut p = defaults();
        p.set_s("closing_given", "19000");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(student.contains(">61,000<") && student.contains(">(34,000)<") && student.contains(">$6,000<"), "the activities and opening cash are the question");
        assert!(!student.contains("12,500") && !student.contains("19,000") && key.contains("12,500"));
        assert!(student.contains("Agrees with the balance sheet?"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_specs_are_explained() {
        for (spec, needle) in [("Cash from customers 5000", "Start with a heading"), ("", "Add a heading"), ("Operating", "Add a heading")] {
            let mut p = defaults();
            p.set_s("spec", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let many: String = (0..9).map(|i| format!("L{i} 1\n")).collect();
        let mut p = defaults();
        p.set_s("spec", &format!("Operating\n{many}"));
        assert!(render(&p).contains("more than 8 lines"));
        let mut p = defaults();
        p.set_f("opening", f64::NAN);
        assert!(render(&p).contains("opening cash"));
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
        for ex in examples("cash-flow-statement").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    Value::F(x) => p.set_f(k, *x),
                    Value::I(i) => p.set_i(k, *i),
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<line") && !svg.contains("NaN"), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "A\nB\nC\nD\nE", "x\ny 999999999999\nz -999999999999", "\u{1f600}\n\u{1f600} (5)", "Operating\nx 0"] {
            for given in ["", "5", "abc", "999999999999"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("spec", spec);
                    p.set_s("closing_given", given);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{spec:?} {given:?}");
                }
            }
        }
    }
}
