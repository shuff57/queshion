//! Accounting figures (the Accounting tab): T-accounts, journal entries, trial balances,
//! financial statements, the accounting equation, depreciation schedules, bank reconciliations and
//! cash-flow statements. Original designs on the shared pieces (`svg::Doc`, `show_answers`, print rules). Every
//! figure reads money the same way and writes it with the same formatter, so a student copy and
//! its key are always the same size.
use super::{Example, FigureDef, ParamDef, Params};

mod bank_rec;
mod cash_flow;
mod depreciation;
mod equation;
mod journal;
mod statements;
mod t_account;
mod trial_balance;

pub fn defs() -> Vec<FigureDef> {
    vec![t_account::def(), journal::def(), trial_balance::def(), statements::def(), equation::def(), depreciation::def(), bank_rec::def(), cash_flow::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    t_account::relevant(fig, key, p) && journal::relevant(fig, key, p) && trial_balance::relevant(fig, key, p) && statements::relevant(fig, key, p) && equation::relevant(fig, key, p) && depreciation::relevant(fig, key, p) && bank_rec::relevant(fig, key, p) && cash_flow::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    t_account::examples(id).or_else(|| journal::examples(id)).or_else(|| trial_balance::examples(id)).or_else(|| statements::examples(id)).or_else(|| equation::examples(id)).or_else(|| depreciation::examples(id)).or_else(|| bank_rec::examples(id)).or_else(|| cash_flow::examples(id))
}

pub const MAX_AMOUNT: f64 = 1.0e12;

pub fn currency_param() -> ParamDef {
    ParamDef::choice("currency", "Currency symbol", &["$", "\u{a3}", "\u{20ac}", "None"], "$")
}

pub fn cents_param() -> ParamDef {
    ParamDef::bool("cents", "Always show cents", false)
}

/// The symbol to write for a `currency` param value ("None" is no symbol).
pub fn symbol(p: &Params) -> &str {
    match p.s("currency") {
        "None" => "",
        s => s,
    }
}

/// A typed amount: digits with optional commas, a leading currency symbol and a decimal part.
/// Negative numbers and brackets are refused (a debit or credit is never negative); None for
/// words, empty text, NaN or anything above `MAX_AMOUNT`.
pub fn parse_amount(s: &str) -> Option<f64> {
    let t = s.trim().trim_start_matches(['$', '\u{a3}', '\u{20ac}']).replace(',', "");
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let v: f64 = t.parse().ok()?;
    (v.is_finite() && (0.0..=MAX_AMOUNT).contains(&v)).then_some(v)
}

/// Thousands separators, and two decimals when `cents` is on or the amount has a fraction.
pub fn digits(v: f64, cents: bool) -> String {
    let dec = if cents || (v - v.round()).abs() > 1e-9 { 2 } else { 0 };
    let s = format!("{:.*}", dec, v.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), None), |(a, b)| (a, Some(b)));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

/// An amount with its symbol; negatives in brackets, the accountant's way.
pub fn money(v: f64, sym: &str, cents: bool) -> String {
    let d = format!("{sym}{}", digits(v, cents));
    if v < -1e-9 { format!("({d})") } else { d }
}

/// The text up to `n` characters (a name that is too long is cut, never overflows a column).
pub fn clip(s: &str, n: usize) -> String {
    s.trim().chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_are_read_and_written() {
        for (t, want) in [("1200", 1200.0), ("1,200.50", 1200.5), ("$5,000", 5000.0), (" 0.5 ", 0.5), ("\u{a3}3", 3.0)] {
            assert_eq!(parse_amount(t), Some(want), "{t}");
        }
        for t in ["", "x", "-5", "(5)", "1e5", "1.2.3", "$", "9999999999999", "1 000"] {
            assert_eq!(parse_amount(t), None, "{t:?}");
        }
        assert_eq!(digits(1234567.0, false), "1,234,567");
        assert_eq!(digits(1234.5, false), "1,234.50");
        assert_eq!(digits(12.0, true), "12.00");
        assert_eq!(digits(999.0, false), "999");
        assert_eq!(money(5000.0, "$", false), "$5,000");
        assert_eq!(money(-300.0, "$", false), "($300)");
        assert_eq!(money(0.0, "", false), "0");
    }

    #[test]
    fn clip_cuts_long_names() {
        assert_eq!(clip("  Accounts receivable  ", 8), "Accounts");
        assert_eq!(clip("", 4), "");
    }
}
