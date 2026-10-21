//! A loan amortisation table: the repayment each period, the interest, the part that reduces the
//! loan and the balance left, for the first rows of the schedule. The amount, the rate and the
//! term are the question; the payment and every cell of the table are the answers, so the student
//! copy (Show answers off) leaves them as empty cells.
use crate::figures::accounting::{currency_param, money, symbol};
use crate::figures::chart;
use crate::figures::geography::trim;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const PER_YEAR: [&str; 4] = ["12", "4", "2", "1"];
const MAX_PERIODS: usize = 480;
const INK: &str = "#111827";

pub fn def() -> FigureDef {
    FigureDef {
        id: "loan-amortisation",
        subject: Subject::Business,
        title: "Loan amortisation table",
        blurb: "A loan repayment schedule from the amount, interest rate and term, with the payment, interest, principal and balance of each period worked out, or empty cells for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "loan-amortisation" => vec![
            Example { name: "A two-year loan", params: vec![] },
            Example { name: "A car loan, first year", params: vec![("principal", Value::F(18000.0)), ("rate", Value::F(7.5)), ("years", Value::F(5.0))] },
            Example { name: "Quarterly repayments", params: vec![("per_year", s("4")), ("years", Value::F(3.0)), ("rows", Value::I(12))] },
            Example { name: "Interest-free", params: vec![("rate", Value::F(0.0)), ("principal", Value::F(1200.0)), ("years", Value::F(1.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("principal", "Amount borrowed", 1.0, 1.0e9, 500.0, 20000.0),
        ParamDef::float("rate", "Interest rate per year (%)", 0.0, 100.0, 0.25, 6.0),
        ParamDef::float("years", "Term (years)", 0.25, 40.0, 0.25, 2.0),
        ParamDef::choice("per_year", "Repayments per year", &PER_YEAR, PER_YEAR[0]),
        ParamDef::int("rows", "Rows to show", 1, 24, 12),
        currency_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the given figures and leaves the payment and every cell of the table for students"),
    ]
}

/// The repayment each period that clears `principal` over `n` periods at `r` per period, to the cent.
pub fn payment(principal: f64, r: f64, n: usize) -> f64 {
    let raw = if r.abs() < 1e-12 { principal / n as f64 } else { principal * r / (1.0 - (1.0 + r).powi(-(n as i32))) };
    (raw * 100.0).round() / 100.0
}

#[derive(Debug, PartialEq)]
pub struct Row {
    pub payment: f64,
    pub interest: f64,
    pub principal: f64,
    pub balance: f64,
}

fn cents(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// The whole schedule; the last payment is adjusted so that the balance ends at exactly zero.
pub fn schedule(principal: f64, r: f64, n: usize) -> Vec<Row> {
    let pay = payment(principal, r, n);
    let mut bal = cents(principal);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let interest = cents(bal * r);
        let (pmt, princ) = if i + 1 == n { (cents(bal + interest), bal) } else { (pay, cents(pay - interest)) };
        bal = cents(bal - princ);
        out.push(Row { payment: pmt, interest, principal: princ, balance: bal });
    }
    out
}

fn render(p: &Params) -> String {
    let (principal, rate, years) = (p.f("principal"), p.f("rate"), p.f("years"));
    if ![principal, rate, years].iter().all(|v| v.is_finite()) {
        return chart::message("Give numbers for the amount, the rate and the term");
    }
    let principal = principal.clamp(1.0, 1.0e9);
    let ppy: f64 = p.s("per_year").parse().unwrap_or(12.0);
    let n = ((years.clamp(0.25, 40.0) * ppy).round() as usize).clamp(1, MAX_PERIODS);
    let r = rate.clamp(0.0, 100.0) / 100.0 / ppy;
    let sched = schedule(principal, r, n);
    let shown = (p.i("rows").clamp(1, 24) as usize).min(n);
    let answers = p.b(SHOW_ANSWERS);
    let sym = symbol(p).to_string();
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let rowh = 28.0;
    let (w, h) = (860.0, top + 170.0 + (shown as f64 + 3.0) * rowh + 80.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let x0 = 40.0;
    let period = match ppy as u32 { 12 => "month", 4 => "quarter", 2 => "half-year", _ => "year" };
    let given = [
        format!("Amount borrowed: {}", money(principal, &sym, false)),
        format!("Interest rate: {}% a year, charged each {period} on the balance", trim(rate.clamp(0.0, 100.0), 3)),
        format!("Term: {} years, paid in {n} equal repayments", trim(years.clamp(0.25, 40.0), 2)),
    ];
    for (i, g) in given.iter().enumerate() {
        chart::text(&mut d, x0, top + 34.0 + i as f64 * fs * 1.8, fs, "start", g);
    }
    let total_int: f64 = sched.iter().map(|r| r.interest).sum();
    let total_paid: f64 = sched.iter().map(|r| r.payment).sum();
    let mut y = top + 34.0 + 3.0 * fs * 1.8 + 6.0;
    for (label, val) in [("Repayment each period:", money(payment(principal, r, n), &sym, true)), ("Total interest over the loan:", money(total_int, &sym, true)), ("Total repaid:", money(total_paid, &sym, true))] {
        chart::text(&mut d, x0, y, fs, "start", label);
        if answers {
            chart::text(&mut d, x0 + 240.0, y, fs, "start", &val);
        } else {
            d.line(x0 + 240.0, y + 3.0, x0 + 400.0, y + 3.0, INK, 1.0);
        }
        y += fs * 1.8;
    }
    // the table
    let cw = [70.0, 160.0, 150.0, 150.0, 170.0];
    let sum = |c: usize| cw[..c].iter().sum::<f64>();
    let ty = y + 40.0;
    d.rect(x0, ty - 20.0, cw.iter().sum(), rowh, "none", INK, 1.2);
    for (c, hd) in ["Period", "Repayment", "Interest", "Principal", "Balance"].iter().enumerate() {
        chart::text(&mut d, x0 + sum(c) + cw[c] / 2.0, ty, fs, "middle", hd);
    }
    // the opening balance
    d.rect(x0, ty + rowh - 20.0, cw.iter().sum(), rowh, "none", INK, 1.0);
    chart::text(&mut d, x0 + cw[0] / 2.0, ty + rowh, fs, "middle", "0");
    chart::text(&mut d, x0 + sum(4) + cw[4] / 2.0, ty + rowh, fs, "middle", &money(principal, &sym, true));
    for (i, row) in sched.iter().take(shown).enumerate() {
        let yy = ty + (i as f64 + 2.0) * rowh;
        d.rect(x0, yy - 20.0, cw.iter().sum(), rowh, "none", INK, 1.0);
        chart::text(&mut d, x0 + cw[0] / 2.0, yy, fs, "middle", &(i + 1).to_string());
        if answers {
            for (c, v) in [(1, row.payment), (2, row.interest), (3, row.principal), (4, row.balance)] {
                chart::text(&mut d, x0 + sum(c) + cw[c] / 2.0, yy, fs, "middle", &money(v, &sym, true));
            }
        }
    }
    // the vertical rules stop above the "more repayments" row, which is one cell
    let end = ty + (shown as f64 + 2.0) * rowh;
    if shown < n {
        d.rect(x0, end - 20.0, cw.iter().sum(), rowh, "none", INK, 1.0);
        chart::text(&mut d, x0 + 10.0, end, fs * 0.95, "start", &format!("\u{2026} {} more repayment{}", n - shown, if n - shown == 1 { "" } else { "s" }));
    }
    for c in 1..5 {
        d.line(x0 + sum(c), ty - 20.0, x0 + sum(c), end - 20.0, INK, 1.0);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn the_payment_formula_matches_known_loans() {
        // 20 000 over 24 months at 0.5% a month
        assert_eq!(payment(20000.0, 0.005, 24), 886.41);
        assert_eq!(payment(1200.0, 0.0, 12), 100.0);
        assert_eq!(payment(100000.0, 0.06 / 12.0, 360), 599.55);
    }

    #[test]
    fn the_schedule_clears_the_loan() {
        for (p, r, n) in [(20000.0, 0.005, 24), (1200.0, 0.0, 12), (999.99, 0.0123, 7), (1.0, 0.5, 1), (5e8, 0.01, 480)] {
            let s = schedule(p, r, n);
            assert_eq!(s.len(), n);
            assert_eq!(s.last().unwrap().balance, 0.0, "{p} {r} {n}");
            for w in s.windows(2) {
                assert!(w[1].balance <= w[0].balance + 1e-9);
            }
            let princ: f64 = s.iter().map(|r| r.principal).sum();
            assert!((princ - p).abs() < 0.011 * n as f64, "{princ} {p}");
        }
        let s = schedule(20000.0, 0.005, 24);
        assert_eq!(s[0].interest, 100.0);
        assert_eq!(s[0].principal, 786.41);
        assert_eq!(s[0].balance, 19213.59);
    }

    #[test]
    fn key_fills_the_table() {
        let svg = render(&defaults());
        for t in [">$886.41<", ">$100.00<", ">$786.41<", ">$19,213.59<", ">Repayment<", ">\u{2026} 12 more repayments<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn student_copy_leaves_cells_empty() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains("Amount borrowed: $20,000<") && student.contains(">$20,000.00<") && !student.contains("886.41") && key.contains("886.41"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn a_short_loan_shows_every_row() {
        let mut p = defaults();
        p.set_f("years", 0.25);
        p.set_s("per_year", "12");
        let svg = render(&p);
        assert!(!svg.contains("more repayment"), "three payments, all shown");
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("loan-amortisation").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300, 1e-300] {
            for key in ["principal", "rate", "years"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_f(key, v);
                    p.set_b(SHOW_ANSWERS, answers);
                    p.set_i("rows", 9999);
                    assert!(sane(&render(&p)), "{key} {v}");
                }
            }
        }
    }
}
