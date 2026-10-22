//! The distributions behind the test figures: normal (from `normal`), Student t and chi-square,
//! with cdf, pdf and quantiles. The special functions are the standard series and continued
//! fractions (log-gamma by Lanczos, the incomplete beta and gamma functions), good to about 1e-10,
//! far more than a figure needs.
use super::normal;

/// ln Γ(x) for x > 0 (Lanczos, g = 7).
pub fn lgamma(x: f64) -> f64 {
    const G: [f64; 9] = [0.999_999_999_999_809_9, 676.520_368_121_885_1, -1_259.139_216_722_402_8, 771.323_428_777_653_1, -176.615_029_162_140_6, 12.507_343_278_686_905, -0.138_571_095_265_720_12, 9.984_369_578_019_572e-6, 1.505_632_735_149_311_6e-7];
    if x < 0.5 {
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - lgamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularised incomplete beta function I_x(a, b).
pub fn betainc(a: f64, b: f64, x: f64) -> f64 {
    if !(x > 0.0) {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front = (lgamma(a + b) - lgamma(a) - lgamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * betacf(a, b, x) / a
    } else {
        1.0 - front * betacf(b, a, 1.0 - x) / b
    }
}

fn betacf(a: f64, b: f64, x: f64) -> f64 {
    let tiny = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny { d = tiny; }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..400 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny { d = tiny; }
        c = 1.0 + aa / c;
        if c.abs() < tiny { c = tiny; }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny { d = tiny; }
        c = 1.0 + aa / c;
        if c.abs() < tiny { c = tiny; }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-14 {
            break;
        }
    }
    h
}

/// The regularised lower incomplete gamma function P(a, x).
pub fn gammainc(a: f64, x: f64) -> f64 {
    if !(x > 0.0) {
        return 0.0;
    }
    let front = (-x + a * x.ln() - lgamma(a)).exp();
    if x < a + 1.0 {
        let (mut sum, mut term, mut ap) = (1.0 / a, 1.0 / a, a);
        for _ in 0..1000 {
            ap += 1.0;
            term *= x / ap;
            sum += term;
            if term.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        sum * front
    } else {
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny { d = tiny; }
            c = b + an / c;
            if c.abs() < tiny { c = tiny; }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-14 {
                break;
            }
        }
        1.0 - front * h
    }
}

pub fn t_cdf(t: f64, df: f64) -> f64 {
    if t.is_nan() {
        return f64::NAN;
    }
    let tail = 0.5 * betainc(df / 2.0, 0.5, df / (df + t * t));
    if t > 0.0 { 1.0 - tail } else { tail }
}

pub fn t_pdf(t: f64, df: f64) -> f64 {
    (lgamma((df + 1.0) / 2.0) - lgamma(df / 2.0)).exp() / (df * std::f64::consts::PI).sqrt() * (1.0 + t * t / df).powf(-(df + 1.0) / 2.0)
}

pub fn chisq_cdf(x: f64, df: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    gammainc(df / 2.0, x / 2.0)
}

pub fn chisq_pdf(x: f64, df: f64) -> f64 {
    if x <= 0.0 {
        return if df < 2.0 { f64::INFINITY } else if df == 2.0 { 0.5 } else { 0.0 };
    }
    ((df / 2.0 - 1.0) * x.ln() - x / 2.0 - (df / 2.0) * 2f64.ln() - lgamma(df / 2.0)).exp()
}

/// The three distributions a test figure can use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dist {
    Z,
    T(f64),
    Chi(f64),
}

impl Dist {
    pub fn cdf(self, x: f64) -> f64 {
        match self {
            Dist::Z => normal::cdf(x),
            Dist::T(df) => t_cdf(x, df),
            Dist::Chi(df) => chisq_cdf(x, df),
        }
    }
    pub fn pdf(self, x: f64) -> f64 {
        match self {
            Dist::Z => normal::pdf(x),
            Dist::T(df) => t_pdf(x, df),
            Dist::Chi(df) => chisq_pdf(x, df),
        }
    }
    /// The value with probability `p` below it, by bisection.
    pub fn quantile(self, p: f64) -> f64 {
        let p = p.clamp(1e-12, 1.0 - 1e-12);
        let (mut lo, mut hi) = match self {
            Dist::Chi(_) => (0.0, 1.0),
            _ => (-1.0, 1.0),
        };
        while self.cdf(hi) < p && hi < 1e7 {
            hi *= 2.0;
        }
        if !matches!(self, Dist::Chi(_)) {
            while self.cdf(lo) > p && lo > -1e7 {
                lo *= 2.0;
            }
        }
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if self.cdf(mid) < p { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() <= tol * b.abs().max(1.0) }

    #[test]
    fn log_gamma_matches_factorials() {
        for (x, want) in [(1.0, 0.0), (2.0, 0.0), (5.0, 24f64.ln()), (10.0, 362880f64.ln()), (0.5, std::f64::consts::PI.sqrt().ln())] {
            assert!(close(lgamma(x), want, 1e-10), "lgamma({x})");
        }
    }

    #[test]
    fn t_critical_values_match_the_table() {
        for (df, p, want) in [(1.0, 0.975, 12.7062), (5.0, 0.975, 2.5706), (10.0, 0.975, 2.2281), (30.0, 0.975, 2.0423), (10.0, 0.95, 1.8125), (100.0, 0.975, 1.9840)] {
            assert!((Dist::T(df).quantile(p) - want).abs() < 5e-4, "t df {df} p {p}: {}", Dist::T(df).quantile(p));
        }
        assert!((Dist::T(10.0).cdf(0.0) - 0.5).abs() < 1e-12);
        assert!((Dist::T(7.0).cdf(1.3) + Dist::T(7.0).cdf(-1.3) - 1.0).abs() < 1e-12);
        // a large df is the normal curve
        assert!((Dist::T(1e6).quantile(0.975) - 1.95996).abs() < 1e-3);
    }

    #[test]
    fn chi_square_critical_values_match_the_table() {
        for (df, p, want) in [(1.0, 0.95, 3.8415), (2.0, 0.95, 5.9915), (4.0, 0.95, 9.4877), (10.0, 0.95, 18.3070), (5.0, 0.99, 15.0863), (30.0, 0.95, 43.7730)] {
            assert!((Dist::Chi(df).quantile(p) - want).abs() < 5e-4, "chi df {df}: {}", Dist::Chi(df).quantile(p));
        }
        assert!(close(chisq_cdf(2.0, 2.0), 1.0 - (-1.0f64).exp(), 1e-12), "df 2 is exponential");
        assert_eq!(chisq_cdf(-1.0, 3.0), 0.0);
    }

    #[test]
    fn densities_integrate_to_one() {
        let area = |d: Dist, lo: f64, hi: f64| { let n = 20000; let h = (hi - lo) / n as f64; (0..n).map(|i| d.pdf(lo + (i as f64 + 0.5) * h) * h).sum::<f64>() };
        assert!((area(Dist::T(5.0), -60.0, 60.0) - 1.0).abs() < 2e-3);
        assert!((area(Dist::Chi(6.0), 0.0, 80.0) - 1.0).abs() < 1e-4);
        assert!((area(Dist::Z, -9.0, 9.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn odd_arguments_do_not_panic() {
        for d in [Dist::Z, Dist::T(1.0), Dist::Chi(1.0)] {
            for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1e300, 1e300] {
                let _ = d.cdf(x);
                let _ = d.pdf(x);
            }
            for p in [0.0, 1.0, -3.0, 7.0, f64::NAN] {
                let _ = d.quantile(p);
            }
        }
    }
}
