//! Groups a figure's flat parameter list into small-caps sections for the side panel.
use super::{Kind, ParamDef};

/// One section: header + indexes into the param list (original order preserved).
pub struct Section {
    pub title: String,
    pub items: Vec<usize>,
}

/// `f1_ang` -> Some(("f", 1)). Per-item params are `<letters><digits>_<rest>`.
pub fn item_of(key: &str) -> Option<(&str, usize)> {
    let (head, _) = key.split_once('_')?;
    let digits = head.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let letters = &head[..head.len() - digits.len()];
    if letters.is_empty() || digits.is_empty() {
        return None;
    }
    Some((letters, digits.parse().ok()?))
}

fn item_noun(fig: &str, letters: &str) -> &'static str {
    match (fig, letters) {
        ("free-body-diagram", _) => "Force",
        ("vector-diagram", _) => "Vector",
        ("motion-graphs", _) => "Segment",
        ("circuit-diagram", _) => "Component",
        _ => "Item",
    }
}

fn is_text_like(pd: &ParamDef) -> bool {
    matches!(pd.kind, Kind::Text | Kind::Long | Kind::List)
}

fn plain_section(pd: &ParamDef) -> &'static str {
    let k = pd.key;
    let toks: Vec<&str> = k.split('_').collect();
    let has = |w: &[&str]| w.iter().any(|x| toks.iter().any(|t| t == x || (x.len() >= 4 && t.ends_with(x))));
    match &pd.kind {
        Kind::Bool => {
            if has(&["answer", "blank", "hidden", "hide"]) { "Question mode" } else { "Show and hide" }
        }
        Kind::Color(_) => "Appearance",
        Kind::Text | Kind::Long | Kind::List => {
            if has(&["title", "xt", "yt", "label", "lab", "name", "caption", "question", "unit", "census", "organism", "prey", "pred", "inequality"]) {
                "Labels and text"
            } else {
                "Data"
            }
        }
        Kind::Choice(_) | Kind::Float { .. } | Kind::Int { .. } => {
            if has(&["color", "colour", "style", "look", "shade", "shape", "size", "grid", "minor", "scale", "layout", "view", "stands", "lean", "rotate", "mirror", "notation", "units", "unit", "fill", "width", "dec"]) {
                "Appearance"
            } else if has(&["min", "max", "step", "xmin", "ymin", "xmax", "ymax", "xstep", "ystep", "tspan", "tshow", "axis", "range", "xaxis"]) {
                "Axes and range"
            } else {
                "Values"
            }
        }
    }
}

/// Builds sections in order of first appearance. If the figure has an integer count `n` the
/// per-item params of items beyond `n` are skipped (`n_now`).
pub fn sections(fig: &str, defs: &[ParamDef], n_now: Option<i64>) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut put = |title: String, i: usize| {
        if let Some(s) = out.iter_mut().find(|s| s.title == title) {
            s.items.push(i);
        } else {
            out.push(Section { title, items: vec![i] });
        }
    };
    for (i, pd) in defs.iter().enumerate() {
        if let Some((letters, n)) = item_of(pd.key) {
            if n_now.is_some_and(|c| n as i64 > c) {
                continue;
            }
            put(format!("{} {}", item_noun(fig, letters), n), i);
        } else if pd.key == "n" || pd.key == "count" {
            put("Quantity".into(), i);
        } else {
            let _ = is_text_like(pd);
            put(plain_section(pd).to_string(), i);
        }
    }
    // Bring the lead section (first param's) first; keep appearance/labels last.
    out.sort_by_key(|s| match s.title.as_str() {
        "Quantity" => 0,
        "Values" => 1,
        "Axes and range" => 2,
        "Data" => 3,
        t if t.contains(' ') && t.chars().last().is_some_and(|c| c.is_ascii_digit()) => 4,
        "Labels and text" => 6,
        "Appearance" => 7,
        "Show and hide" => 8,
        _ => 9,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_keys() {
        assert_eq!(item_of("f1_ang"), Some(("f", 1)));
        assert_eq!(item_of("res_style"), None);
        assert_eq!(item_of("mark_h"), None);
    }

    #[test]
    fn every_param_lands_in_exactly_one_section() {
        for d in super::super::registry() {
            let defs = (d.params)();
            let secs = sections(d.id, &defs, None);
            let mut seen: Vec<usize> = secs.iter().flat_map(|s| s.items.clone()).collect();
            seen.sort();
            assert_eq!(seen, (0..defs.len()).collect::<Vec<_>>(), "{}", d.id);
            assert!(secs.len() <= 12, "{} has {} sections", d.id, secs.len());
        }
    }
}
