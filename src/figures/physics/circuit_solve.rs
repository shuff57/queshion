//! The arithmetic behind "Work out the circuit" on the circuit diagram: reading a typed value such
//! as "4.7 kΩ" or "12 V", solving the series or parallel circuit with ideal meters, and writing
//! a result back out with a sensible unit prefix. Pure functions, no drawing.

/// The value of a typed quantity: a number with an optional SI prefix (m, k, M, µ) and an
/// optional unit (Ω, ohm, V, A). None for a symbol such as "R", an empty box, or junk.
pub fn parse_qty(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && (i == 0 || matches!(s.as_bytes()[i - 1], b'e' | b'E'))) || ((c == 'e' || c == 'E') && i > 0 && s[i + 1..].starts_with(|d: char| d.is_ascii_digit() || d == '-' || d == '+'))))
        .map_or(s.len(), |(i, _)| i);
    let n: f64 = s[..end].parse().ok()?;
    let rest = s[end..].trim();
    let is_unit = |u: &str| matches!(u.trim(), "" | "Ω" | "ohm" | "ohms" | "Ohm" | "Ohms" | "V" | "A" | "v" | "a");
    // (multiply, divide): sub-unit prefixes divide so 10 µA is exactly 1e-5
    let (mult, div) = if is_unit(rest) {
        (1.0, 1.0)
    } else {
        let mut it = rest.chars();
        let p = it.next()?;
        let m = match p {
            'k' | 'K' => (1e3, 1.0),
            'M' => (1e6, 1.0),
            'm' => (1.0, 1e3),
            'µ' | 'μ' | 'u' => (1.0, 1e6),
            _ => return None,
        };
        if !is_unit(it.as_str()) {
            return None;
        }
        m
    };
    let v = n * mult / div;
    (v.is_finite() && v.abs() <= 1e12).then_some(v)
}

/// A result with three significant figures and a prefix that keeps the number in 1..1000
/// ("2.5 kΩ", "40 mA"). Zero is "0 unit".
pub fn fmt_qty(x: f64, unit: &str) -> String {
    if !x.is_finite() {
        return "?".into();
    }
    if x == 0.0 {
        return format!("0 {unit}");
    }
    let (mut v, mut prefix) = (x, "");
    for (limit, p, div) in [(1e9, "G", 1e9), (1e6, "M", 1e6), (1e3, "k", 1e3)] {
        if x.abs() >= limit {
            v = x / div;
            prefix = p;
            break;
        }
    }
    if prefix.is_empty() {
        for (limit, p, mul) in [(1.0, "", 1.0), (1e-3, "m", 1e3), (1e-6, "µ", 1e6), (1e-9, "n", 1e9)] {
            if x.abs() >= limit {
                v = x * mul;
                prefix = p;
                break;
            }
        }
        if prefix.is_empty() && x.abs() < 1e-9 {
            return format!("{x:.2e} {unit}");
        }
    }
    // three significant figures, trailing zeros dropped
    let decimals = (2 - v.abs().log10().floor() as i32).clamp(0, 9) as usize;
    let mut t = format!("{v:.decimals$}");
    if t.contains('.') {
        t = t.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    format!("{} {prefix}{unit}", t.replace('-', "\u{2212}"))
}

/// What the circuit works out to; None where a value is unknown (a symbol was typed) or the
/// circuit has no answer (a short across the battery).
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    pub r_eq: Option<f64>,
    pub i_total: Option<f64>,
    pub v_batt: Option<f64>,
    pub v: Vec<Option<f64>>,
    pub i: Vec<Option<f64>>,
}

/// Solves `r` (one entry per part) on a battery of `v_batt`, in series or in parallel. An open
/// switch stops all current. Meters are ideal.
pub fn solve(v_batt: Option<f64>, r: &[Option<f64>], parallel: bool, open: bool) -> Solved {
    let n = r.len();
    let r: Vec<Option<f64>> = r.iter().map(|x| x.filter(|v| *v >= 0.0)).collect();
    let all: Option<Vec<f64>> = r.iter().cloned().collect();
    let r_eq = all.as_ref().and_then(|a| {
        if parallel {
            if a.iter().any(|x| *x == 0.0) { Some(0.0) } else { Some(1.0 / a.iter().map(|x| 1.0 / x).sum::<f64>()) }
        } else {
            Some(a.iter().sum())
        }
    });
    if open {
        return Solved { r_eq, i_total: Some(0.0), v_batt, v: vec![Some(0.0); n], i: vec![Some(0.0); n] };
    }
    let (mut v, mut i) = (vec![None; n], vec![None; n]);
    let mut i_total = None;
    if let (Some(vb), Some(a)) = (v_batt, all.as_ref()) {
        if parallel {
            for k in 0..n {
                v[k] = Some(vb);
                i[k] = (a[k] > 0.0).then(|| vb / a[k]);
            }
            i_total = i.iter().cloned().sum::<Option<f64>>();
        } else if let Some(req) = r_eq.filter(|q| *q > 0.0) {
            let it = vb / req;
            i_total = Some(it);
            for k in 0..n {
                i[k] = Some(it);
                v[k] = Some(it * a[k]);
            }
        }
    }
    Solved { r_eq, i_total, v_batt, v, i }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Option<f64>, b: f64) -> bool { a.is_some_and(|a| (a - b).abs() < 1e-9 * b.abs().max(1.0)) }

    #[test]
    fn quantities_with_prefixes_and_units() {
        assert_eq!(parse_qty("2 Ω"), Some(2.0));
        assert_eq!(parse_qty("12 V"), Some(12.0));
        assert_eq!(parse_qty("4.7 kΩ"), Some(4700.0));
        assert_eq!(parse_qty("4.7k"), Some(4700.0));
        assert_eq!(parse_qty("40 mA"), Some(0.04));
        assert_eq!(parse_qty("2 MΩ"), Some(2e6));
        assert_eq!(parse_qty("10µA"), Some(1e-5));
        assert_eq!(parse_qty("1e3"), Some(1000.0));
        assert_eq!(parse_qty("220 ohm"), Some(220.0));
        assert_eq!(parse_qty("-5 V"), Some(-5.0));
    }

    #[test]
    fn symbols_and_junk_are_unknown() {
        for s in ["", "R", "x", "abc", "5 parsecs", "2 kg", "1e999", "999999999999999999 V", "--3"] {
            assert_eq!(parse_qty(s), None, "{s:?}");
        }
    }

    #[test]
    fn results_get_three_figures_and_a_prefix() {
        assert_eq!(fmt_qty(12.0, "Ω"), "12 Ω");
        assert_eq!(fmt_qty(2.0 / 3.0, "A"), "667 mA");
        assert_eq!(fmt_qty(4700.0, "Ω"), "4.7 kΩ");
        assert_eq!(fmt_qty(0.0, "V"), "0 V");
        assert_eq!(fmt_qty(1.0, "A"), "1 A");
        assert_eq!(fmt_qty(2.5e-5, "A"), "25 µA");
        assert_eq!(fmt_qty(-3.5, "V"), "\u{2212}3.5 V");
        assert_eq!(fmt_qty(f64::NAN, "V"), "?");
        assert_eq!(fmt_qty(f64::INFINITY, "V"), "?");
        assert_eq!(fmt_qty(123456.0, "Ω"), "123 kΩ");
        assert!(!fmt_qty(1e-30, "A").contains("NaN"));
        assert!(!fmt_qty(1e300, "A").contains("inf"));
    }

    #[test]
    fn series_circuit() {
        let s = solve(Some(12.0), &[Some(2.0), Some(4.0), Some(6.0)], false, false);
        assert!(near(s.r_eq, 12.0) && near(s.i_total, 1.0));
        assert!(near(s.v[0], 2.0) && near(s.v[1], 4.0) && near(s.v[2], 6.0));
        assert!(s.i.iter().all(|i| near(*i, 1.0)));
    }

    #[test]
    fn parallel_circuit() {
        let s = solve(Some(12.0), &[Some(2.0), Some(4.0), Some(6.0)], true, false);
        assert!(near(s.r_eq, 12.0 / 11.0));
        assert!(near(s.i_total, 6.0 + 3.0 + 2.0));
        assert!(s.v.iter().all(|v| near(*v, 12.0)));
        assert!(near(s.i[1], 3.0));
    }

    #[test]
    fn open_switch_stops_everything() {
        for parallel in [false, true] {
            let s = solve(Some(12.0), &[Some(2.0), Some(4.0)], parallel, true);
            assert_eq!(s.i_total, Some(0.0));
            assert!(s.v.iter().chain(s.i.iter()).all(|x| *x == Some(0.0)));
        }
    }

    #[test]
    fn unknowns_and_shorts_have_no_answer_not_a_wrong_one() {
        let s = solve(Some(12.0), &[Some(2.0), None], false, false);
        assert!(s.r_eq.is_none() && s.i_total.is_none() && s.v.iter().all(Option::is_none));
        let s = solve(None, &[Some(2.0)], false, false);
        assert!(near(s.r_eq, 2.0) && s.i_total.is_none());
        let s = solve(Some(12.0), &[Some(0.0)], false, false);
        assert!(near(s.r_eq, 0.0) && s.i_total.is_none(), "a series loop with no resistance has no current");
        let s = solve(Some(12.0), &[Some(0.0), Some(4.0)], true, false);
        assert!(near(s.r_eq, 0.0) && s.i_total.is_none(), "a short in parallel");
        let s = solve(Some(12.0), &[Some(-3.0)], false, false);
        assert!(s.r_eq.is_none(), "negative resistance is not a resistor");
    }

    #[test]
    fn huge_values_stay_finite_or_unknown() {
        let s = solve(Some(1e12), &[Some(1e-12), Some(1e12)], true, false);
        for x in s.v.iter().chain(s.i.iter()).chain([&s.i_total, &s.r_eq]) {
            assert!(x.map_or(true, f64::is_finite));
        }
    }
}
