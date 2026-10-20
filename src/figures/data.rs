//! Data entry for the figures that plot numbers (`Kind::Data`): reading what a teacher types,
//! pastes from a spreadsheet or opens from a CSV file, tidying it into the canonical text a
//! figure stores, and generating sample data from a seed.
//!
//! Canonical text: one column is `2, 4, 5` (the same text `math::nums` reads); two columns are
//! one `x, y` row per line. A figure reads it back with [`numbers`] / [`pairs`].

/// One pasted or typed cell split out of a line.
fn split_line(line: &str) -> Vec<String> {
    // a spreadsheet copy is tab separated; a CSV file is comma (or semicolon) separated, possibly
    // quoted; anything else is numbers typed with spaces
    let cells: Vec<String> = if line.contains('\t') {
        line.split('\t').map(|c| c.trim().to_string()).collect()
    } else if line.contains(',') || line.contains(';') {
        csv_cells(line)
    } else {
        line.split_whitespace().map(|c| c.to_string()).collect()
    };
    cells.into_iter().map(|c| c.trim_matches('"').trim().replace('\u{2212}', "-")).collect()
}

fn csv_cells(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' | ';' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    out.push(cur);
    out
}

fn as_number(cell: &str) -> Option<f64> {
    let c = cell.trim();
    // a percent sign from a spreadsheet is not part of the value
    let c = c.trim_end_matches('%');
    c.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= MAX_ABS)
}

/// The cells of `text` as rows: lines split at tabs, commas/semicolons or spaces.
pub fn parse_table(text: &str) -> Vec<Vec<String>> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(split_line).collect()
}

/// Largest magnitude read as a number. Beyond this a value is junk for any figure (and would
/// overflow the axis arithmetic), so it is skipped like a word.
pub const MAX_ABS: f64 = 1e12;

/// Most rows kept from one paste or file; the rest are dropped (a figure of more points than this
/// is unreadable, and a huge paste must not freeze the preview).
pub const MAX_ROWS: usize = 5000;

/// Result of reading `text` for a figure that wants `columns` numbers per row.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub rows: Vec<Vec<f64>>,
    /// Cells that were not numbers (a header, a stray word) and were left out.
    pub skipped: usize,
    /// More than [`MAX_ROWS`] rows were given; only the first ones are kept.
    pub truncated: bool,
}

/// Reads `text` as rows of `columns` numbers. With one column every number on every line counts
/// (a comma separated line or a spreadsheet column both work); with more, each line is a row, a
/// row whose first `columns` cells are not all numbers (a header) is skipped, and extra cells
/// to the right are ignored.
pub fn read(text: &str, columns: usize) -> Read {
    let columns = columns.max(1);
    let (mut rows, mut skipped, mut truncated) = (Vec::new(), 0, false);
    for cells in parse_table(text) {
        if rows.len() >= MAX_ROWS {
            truncated = true;
            break;
        }
        if columns == 1 {
            for c in cells.iter().filter(|c| !c.is_empty()) {
                match as_number(c) {
                    Some(v) => rows.push(vec![v]),
                    None => skipped += 1,
                }
            }
        } else {
            let nums: Vec<f64> = cells.iter().take(columns).filter_map(|c| as_number(c)).collect();
            if nums.len() == columns && cells.len() >= columns {
                rows.push(nums);
            } else {
                skipped += cells.iter().filter(|c| !c.is_empty()).count();
            }
        }
    }
    Read { rows, skipped, truncated }
}

/// The single column of `text`.
pub fn numbers(text: &str) -> Vec<f64> {
    read(text, 1).rows.into_iter().map(|r| r[0]).collect()
}

/// The (x, y) rows of `text`.
pub fn pairs(text: &str) -> Vec<(f64, f64)> {
    read(text, 2).rows.into_iter().map(|r| (r[0], r[1])).collect()
}

/// A value as short text: no trailing zeros, no `-0`.
pub fn fmt_num(v: f64) -> String {
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_string() }
}

/// `text` tidied into the canonical form for `columns`. Returns None when nothing in it is a
/// number, so the caller can leave what the user typed alone instead of wiping it.
pub fn canonical(text: &str, columns: usize) -> Option<String> {
    let r = read(text, columns);
    if r.rows.is_empty() {
        return None;
    }
    Some(if columns <= 1 {
        r.rows.iter().map(|row| fmt_num(row[0])).collect::<Vec<_>>().join(", ")
    } else {
        r.rows.iter().map(|row| row.iter().map(|v| fmt_num(*v)).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("\n")
    })
}

/// "9 values" / "5 pairs", plus a note of what was left out.
pub fn summary(text: &str, columns: usize) -> String {
    let r = read(text, columns);
    let unit = if columns <= 1 { ("value", "values") } else { ("pair", "pairs") };
    let n = r.rows.len();
    let mut s = format!("{n} {}", if n == 1 { unit.0 } else { unit.1 });
    if r.skipped > 0 {
        s.push_str(&format!(" ({} skipped: not numbers)", r.skipped));
    }
    if r.truncated {
        s.push_str(&format!(" (only the first {MAX_ROWS} are used)"));
    }
    s
}

// ───────────────────────── sample data ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dist {
    Normal,
    Uniform,
    SkewedRight,
    SkewedLeft,
    Bimodal,
}

impl Dist {
    pub const ALL: [Dist; 5] = [Dist::Normal, Dist::Uniform, Dist::SkewedRight, Dist::SkewedLeft, Dist::Bimodal];
    pub fn name(self) -> &'static str {
        match self {
            Dist::Normal => "Bell-shaped",
            Dist::Uniform => "Even spread",
            Dist::SkewedRight => "Skewed right",
            Dist::SkewedLeft => "Skewed left",
            Dist::Bimodal => "Two peaks",
        }
    }
}

/// Small seeded generator (splitmix64): the same seed always gives the same data, on every
/// platform, so a saved figure or a shared link reopens with the numbers it was made with.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self { Rng(seed) }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in (0, 1).
    pub fn unit(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    /// Standard normal (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let (a, b) = (self.unit(), self.unit());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

fn rounded(v: f64, decimals: u32) -> f64 {
    let m = 10f64.powi(decimals as i32);
    (v * m).round() / m
}

/// `n` values with the given mean and standard deviation, shaped by `dist`, rounded to
/// `decimals` places.
pub fn generate(dist: Dist, n: usize, seed: u64, mean: f64, sd: f64, decimals: u32) -> Vec<f64> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            let v = match dist {
                Dist::Normal => mean + sd * rng.normal(),
                Dist::Uniform => mean + sd * 3f64.sqrt() * (2.0 * rng.unit() - 1.0),
                // exponential has mean 1 and sd 1, so this keeps the asked-for mean and sd
                Dist::SkewedRight => mean + sd * (-rng.unit().ln() - 1.0),
                Dist::SkewedLeft => mean - sd * (-rng.unit().ln() - 1.0),
                Dist::Bimodal => {
                    let side = if rng.unit() < 0.5 { -1.0 } else { 1.0 };
                    mean + sd * (side * 1.2 + 0.5 * rng.normal()) / 1.3
                }
            };
            rounded(v, decimals)
        })
        .collect()
}

/// `n` (x, y) points: x bell-shaped around `x_mean` with sd `x_sd`, y = `intercept` + `slope` x
/// plus noise of sd `noise`.
pub fn generate_pairs(n: usize, seed: u64, x_mean: f64, x_sd: f64, slope: f64, intercept: f64, noise: f64, decimals: u32) -> Vec<(f64, f64)> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            let x = x_mean + x_sd * rng.normal();
            let y = intercept + slope * x + noise * rng.normal();
            (rounded(x, decimals), rounded(y, decimals))
        })
        .collect()
}

/// Canonical one-column text for `values`.
pub fn column_text(values: &[f64]) -> String {
    values.iter().map(|v| fmt_num(*v)).collect::<Vec<_>>().join(", ")
}

/// Canonical two-column text for `points`.
pub fn pairs_text(points: &[(f64, f64)]) -> String {
    points.iter().map(|(x, y)| format!("{}, {}", fmt_num(*x), fmt_num(*y))).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_list_and_spreadsheet_column_read_the_same() {
        assert_eq!(numbers("2, 4, 5.5; 7"), vec![2.0, 4.0, 5.5, 7.0]);
        assert_eq!(numbers("2\n4\n5.5\n7"), vec![2.0, 4.0, 5.5, 7.0]);
        assert_eq!(numbers("2\t4\t5.5\t7"), vec![2.0, 4.0, 5.5, 7.0]);
        assert_eq!(numbers("2 4 5.5 7"), vec![2.0, 4.0, 5.5, 7.0]);
        assert_eq!(numbers("\u{2212}3, 4"), vec![-3.0, 4.0]);
    }

    #[test]
    fn words_are_skipped_and_counted() {
        let r = read("Score\n10\n12\nabsent\n15", 1);
        assert_eq!(r.rows.len(), 3);
        assert_eq!(r.skipped, 2);
        assert_eq!(summary("Score\n10\n12", 1), "2 values (1 skipped: not numbers)");
        assert_eq!(summary("7", 1), "1 value");
    }

    #[test]
    fn absurd_magnitudes_are_skipped_like_words() {
        let r = read("1, 1e308, -1e13, 2, 1e12", 1);
        assert_eq!(r.rows, vec![vec![1.0], vec![2.0], vec![1e12]]);
        assert_eq!(r.skipped, 2);
        assert!(pairs("1e300, 1\n2, 3").len() == 1);
    }

    #[test]
    fn a_huge_paste_is_capped() {
        let big = (0..MAX_ROWS + 50).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let r = read(&big, 1);
        assert_eq!(r.rows.len(), MAX_ROWS);
        assert!(r.truncated);
        assert!(summary(&big, 1).contains("only the first 5000"));
        assert!(!read("1\n2", 1).truncated);
    }

    #[test]
    fn two_columns_skip_a_header_row_and_ignore_extra_cells() {
        let csv = "Hours,Score,Name\n1,52,Ana\n2,60,Bo\n3,\"71\",Cy";
        assert_eq!(pairs(csv), vec![(1.0, 52.0), (2.0, 60.0), (3.0, 71.0)]);
        assert_eq!(pairs("1\t2\n3\t4"), vec![(1.0, 2.0), (3.0, 4.0)]);
        assert_eq!(pairs("5 6\n7 8"), vec![(5.0, 6.0), (7.0, 8.0)]);
        // a row missing its y is dropped, not paired with the next row
        assert_eq!(pairs("1,2\n3\n5,6"), vec![(1.0, 2.0), (5.0, 6.0)]);
    }

    #[test]
    fn canonical_text_round_trips_and_never_wipes_input() {
        assert_eq!(canonical("2\n4\n5.50", 1).as_deref(), Some("2, 4, 5.5"));
        assert_eq!(canonical("Hours,Score\n1,52\n2,60", 2).as_deref(), Some("1, 52\n2, 60"));
        assert_eq!(canonical("no numbers here", 1), None);
        let t = canonical("1\t2\n3\t4", 2).unwrap();
        assert_eq!(canonical(&t, 2).unwrap(), t);
    }

    #[test]
    fn same_seed_same_data() {
        let a = generate(Dist::Normal, 30, 7, 50.0, 10.0, 1);
        assert_eq!(a, generate(Dist::Normal, 30, 7, 50.0, 10.0, 1));
        assert_ne!(a, generate(Dist::Normal, 30, 8, 50.0, 10.0, 1));
        assert_eq!(a.len(), 30);
        assert!(a.iter().all(|v| (v * 10.0 - (v * 10.0).round()).abs() < 1e-9), "one decimal");
        let text = column_text(&a);
        assert_eq!(numbers(&text), a, "generated text reads back exactly");
    }

    #[test]
    fn distributions_hit_their_mean_and_skew() {
        for d in Dist::ALL {
            let v = generate(d, 4000, 3, 100.0, 15.0, 3);
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
            assert!((mean - 100.0).abs() < 1.5, "{d:?} mean {mean}");
            if d != Dist::Bimodal {
                assert!((sd - 15.0).abs() < 1.5, "{d:?} sd {sd}");
            }
        }
        let skew = |d| {
            let v = generate(d, 4000, 3, 100.0, 15.0, 3);
            let mut s = v.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.iter().sum::<f64>() / v.len() as f64 - s[s.len() / 2]
        };
        assert!(skew(Dist::SkewedRight) > 1.0, "right skew pulls the mean above the median");
        assert!(skew(Dist::SkewedLeft) < -1.0);
    }

    #[test]
    fn generated_pairs_follow_the_line() {
        let pts = generate_pairs(400, 11, 5.0, 2.0, 3.0, 10.0, 0.5, 2);
        assert_eq!(pts.len(), 400);
        let n = pts.len() as f64;
        let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
        let slope = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / pts.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
        assert!((slope - 3.0).abs() < 0.1, "slope {slope}");
        assert_eq!(pairs(&pairs_text(&pts)), pts);
    }
}
