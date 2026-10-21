//! Geography figures (the Geography tab): climate graphs, population pyramids, grid references
//! and bearings, and map scale and distance workings. Original geometric drawings on the shared
//! pieces (`chart`, `svg::Doc`, `Fills`, `show_answers`, print rules). What is given (the table
//! of figures, the positions, the scale) is the question; the plotted graph, the references,
//! bearings and distances are the answers, so a student copy (Show answers off) leaves them
//! out and is the same size as its key.
use super::{Example, FigureDef, Params};

mod climate;
mod grid;
mod pyramid;
mod scale;

pub fn defs() -> Vec<FigureDef> {
    vec![climate::def(), pyramid::def(), grid::def(), scale::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    climate::examples(id).or_else(|| pyramid::examples(id)).or_else(|| grid::examples(id)).or_else(|| scale::examples(id))
}

/// Rows of `cols` numbers (separated by commas, spaces or semicolons, one row per line), each in
/// `lo..=hi`. `what` names the quantity in the error message.
pub fn rows(spec: &str, cols: usize, min_rows: usize, max_rows: usize, lo: f64, hi: f64, what: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut out = Vec::new();
    for line in spec.lines() {
        let toks: Vec<&str> = line.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|t| !t.is_empty()).collect();
        if toks.is_empty() {
            continue;
        }
        if toks.len() != cols {
            return Err(format!("Each row needs {cols} number{}", if cols == 1 { "" } else { "s" }));
        }
        let mut r = Vec::with_capacity(cols);
        for t in toks {
            match t.parse::<f64>() {
                Ok(x) if x.is_finite() && (lo..=hi).contains(&x) => r.push(x),
                _ => return Err(format!("\u{201c}{}\u{201d} is not {what}", t.chars().take(20).collect::<String>())),
            }
        }
        out.push(r);
        if out.len() > max_rows {
            return Err(format!("A figure is limited to {max_rows} rows"));
        }
    }
    if out.len() < min_rows {
        return Err(format!("Give {min_rows} row{} of numbers", if min_rows == 1 { "" } else { "s" }));
    }
    Ok(out)
}

/// A number to at most `dp` decimals with no trailing zeros.
pub fn trim(v: f64, dp: usize) -> String {
    if !v.is_finite() {
        return "?".into();
    }
    let s = format!("{:.*}", dp, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s.replace('-', "\u{2212}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_parse_and_refuse() {
        assert_eq!(rows("1, 2\n3 4\n\n", 2, 1, 5, 0.0, 9.0, "a number").unwrap(), vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        assert!(rows("1 2 3", 2, 1, 5, 0.0, 9.0, "x").unwrap_err().contains("2 numbers"));
        assert!(rows("a b", 2, 1, 5, 0.0, 9.0, "a number").unwrap_err().contains("not a number"));
        assert!(rows("1 99", 2, 1, 5, 0.0, 9.0, "a number").is_err());
        assert!(rows("NaN 1", 2, 1, 5, 0.0, 9.0, "a number").is_err());
        assert!(rows("", 2, 1, 5, 0.0, 9.0, "x").is_err());
        assert!(rows("1 1\n1 1\n1 1", 2, 1, 2, 0.0, 9.0, "x").unwrap_err().contains("limited to 2"));
    }

    #[test]
    fn trim_drops_zeros_and_uses_a_true_minus() {
        assert_eq!(trim(2.50, 3), "2.5");
        assert_eq!(trim(3.0, 2), "3");
        assert_eq!(trim(-1.25, 2), "\u{2212}1.25");
        assert_eq!(trim(-0.0001, 2), "0");
        assert_eq!(trim(f64::NAN, 2), "?");
    }
}
