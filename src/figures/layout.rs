//! Groups a figure's flat parameter list into small-caps sections for the side panel.
use super::{Kind, ParamDef};

/// One section: header + indexes into the param list (original order preserved).
pub struct Section {
    pub title: String,
    pub items: Vec<usize>,
    /// One line of guidance under the header (from the reference generator), may be empty.
    pub help: String,
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

/// The side panel's sections for a figure: the reference layout when `sections::reference` has one
/// (named sections in the reference's order, per-item sections where it puts them), else the
/// keyword grouping below. Every param lands in exactly one section either way.
pub fn sections(fig: &str, defs: &[ParamDef], n_now: Option<i64>) -> Vec<Section> {
    // Params a Measures card owns are drawn there, not in a section.
    let mut owned: Vec<&str> = super::measures_for(fig).map(|c| c.keys()).unwrap_or_default();
    owned.extend(super::rows_for(fig).map(|c| c.keys()).unwrap_or_default());
    owned.extend(super::card_keys(fig));
    let mut out = sections_all(fig, defs, n_now);
    for s in &mut out {
        s.items.retain(|&i| !owned.contains(&defs[i].key) && !super::cards::owns(fig, defs[i].key));
    }
    out.retain(|s| !s.items.is_empty());
    out
}

fn sections_all(fig: &str, defs: &[ParamDef], n_now: Option<i64>) -> Vec<Section> {
    let Some(reference) = super::sections::reference(fig) else { return heuristic_sections(fig, defs, n_now) };
    // Keys the table names explicitly belong to that section even when they look like per-item keys
    // (`a2_count`: the "after" box of particle kind a).
    let listed: Vec<&str> = reference.iter().filter(|r| !r.item).flat_map(|r| r.keys.iter().copied()).collect();
    let in_item = |i: usize| item_of(defs[i].key).is_some() && !listed.contains(&defs[i].key);
    let mut placed = vec![false; defs.len()];
    let mut out: Vec<Section> = Vec::new();
    for rs in reference {
        if rs.item {
            // One section per item number, params in declaration order.
            let mut nums: Vec<usize> = defs.iter().filter(|d| !listed.contains(&d.key)).filter_map(|d| item_of(d.key).map(|(_, n)| n)).collect();
            nums.sort_unstable();
            nums.dedup();
            for n in nums {
                if n_now.is_some_and(|c| n as i64 > c) {
                    continue;
                }
                let idx: Vec<usize> = (0..defs.len()).filter(|&i| item_of(defs[i].key).is_some_and(|(_, k)| k == n) && !placed[i] && !listed.contains(&defs[i].key)).collect();
                if idx.is_empty() {
                    continue;
                }
                let letters = item_of(defs[idx[0]].key).map(|(l, _)| l).unwrap_or("");
                let noun = if letters.is_empty() { rs.title } else { rs.title };
                for &i in &idx {
                    placed[i] = true;
                }
                out.push(Section { title: format!("{noun} {n}"), items: idx, help: String::new() });
            }
        } else {
            let idx: Vec<usize> = (0..defs.len()).filter(|&i| !placed[i] && !in_item(i) && rs.keys.contains(&defs[i].key)).collect();
            if idx.is_empty() {
                continue;
            }
            for &i in &idx {
                placed[i] = true;
            }
            out.push(Section { title: rs.title.to_string(), items: idx, help: rs.help.to_string() });
        }
    }
    // Anything the table doesn't name (new params, other item families) goes to the end.
    let rest: Vec<usize> = (0..defs.len()).filter(|&i| !placed[i]).collect();
    for i in rest {
        let pd = &defs[i];
        if let Some((letters, n)) = item_of(pd.key) {
            if n_now.is_some_and(|c| n as i64 > c) {
                continue;
            }
            let title = format!("{} {}", item_noun(fig, letters), n);
            match out.iter_mut().find(|s| s.title == title) {
                Some(s) => s.items.push(i),
                None => out.push(Section { title, items: vec![i], help: String::new() }),
            }
        } else {
            match out.iter_mut().find(|s| s.title == "More options") {
                Some(s) => s.items.push(i),
                None => out.push(Section { title: "More options".into(), items: vec![i], help: String::new() }),
            }
        }
    }
    out
}

/// Builds sections in order of first appearance. If the figure has an integer count `n` the
/// per-item params of items beyond `n` are skipped (`n_now`).
fn heuristic_sections(fig: &str, defs: &[ParamDef], n_now: Option<i64>) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut put = |title: String, i: usize| {
        if let Some(s) = out.iter_mut().find(|s| s.title == title) {
            s.items.push(i);
        } else {
            out.push(Section { title, items: vec![i], help: String::new() });
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

    /// Dev tool: `QUESHION_DUMP=/path/sections.json cargo test --release dump_sections -- --ignored`
    /// writes every figure's panel section titles (used to compare with the reference pages).
    #[test]
    #[ignore]
    fn dump_sections() {
        let Ok(path) = std::env::var("QUESHION_DUMP") else { return };
        let mut out = serde_json::Map::new();
        let mut keys = serde_json::Map::new();
        for d in super::super::registry() {
            keys.insert(d.id.to_string(), serde_json::json!((d.params)().iter().map(|x| x.key).collect::<Vec<_>>()));
            let defs = (d.params)();
            let n = defs.iter().any(|x| x.key == "n").then_some(4);
            let mut titles: Vec<String> = sections(d.id, &defs, n).iter().map(|s| s.title.clone()).collect();
            if let Some(c) = super::super::measures_for(d.id) {
                titles.insert(0, c.title.to_string());
            }
            if let Some(c) = super::super::rows_for(d.id) {
                titles.insert(0, c.title.to_string());
            }
            if let Some(c) = super::super::chips_for(d.id) {
                titles.insert(0, c.title.to_string());
            }
            out.insert(d.id.to_string(), serde_json::json!(titles));
        }
        std::fs::write(&path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
        std::fs::write(path.replace(".json", "-keys.json"), serde_json::to_string_pretty(&keys).unwrap()).unwrap();
    }

    #[test]
    fn item_keys() {
        assert_eq!(item_of("f1_ang"), Some(("f", 1)));
        assert_eq!(item_of("res_style"), None);
        assert_eq!(item_of("mark_h"), None);
    }

    /// No figure gets a stray "Item 2" section from a key that only looks like a per-item key
    /// (population growth's `y2_max`), and population growth opens on its Population card.
    #[test]
    fn no_stray_item_sections() {
        for d in super::super::registry() {
            let defs = (d.params)();
            for s in sections(d.id, &defs, None) {
                assert!(!s.title.starts_with("Item "), "{}: stray section {}", d.id, s.title);
            }
        }
        let pg = super::super::registry().into_iter().find(|d| d.id == "population-growth").unwrap();
        let defs = (pg.params)();
        let secs = sections(pg.id, &defs, None);
        assert_eq!(secs[0].title, "Population");
        let yaxis = secs.iter().find(|s| s.title == "y-axis").unwrap();
        assert!(yaxis.items.iter().any(|&i| defs[i].key == "y2_max"));
        assert!(secs.iter().all(|s| s.title != "More options"), "every population param has a reference section");
    }

    /// Section order follows the reference pages on the figures review 3 flagged.
    #[test]
    fn reference_section_order() {
        let titles = |id: &str, n: Option<i64>| {
            let d = super::super::registry().into_iter().find(|d| d.id == id).unwrap();
            let defs = (d.params)();
            sections(id, &defs, n).into_iter().map(|s| s.title).collect::<Vec<_>>()
        };
        let mg = titles("motion-graphs", Some(4));
        assert_eq!(mg[0], "Motion");
        assert!(mg[1].starts_with("Segment 1"), "{mg:?}");
        let g = mg.iter().position(|t| t == "Graphs").unwrap();
        assert!(mg[1..g].iter().all(|t| t.starts_with("Segment ")), "segments sit under Motion, then Graphs: {mg:?}");
        assert_eq!(&mg[g + 1..g + 3], ["Marks", "Titles"]);
        assert_eq!(titles("gel-electrophoresis", None)[..2], ["Lanes", "Gel"]);
        assert_eq!(titles("predator-prey", None)[..2], ["Predator and prey", "Graph"]);
        assert_eq!(titles("punnett-square", None), ["Phenotypes", "Square", "Shading", "Ratios", "Title and question"]);
    }

    #[test]
    fn every_param_lands_in_exactly_one_section() {
        for d in super::super::registry() {
            let defs = (d.params)();
            let secs = sections(d.id, &defs, None);
            let mut seen: Vec<usize> = secs.iter().flat_map(|s| s.items.clone()).collect();
            let mut owned = super::super::measures_for(d.id).map(|c| c.keys()).unwrap_or_default();
            owned.extend(super::super::rows_for(d.id).map(|c| c.keys()).unwrap_or_default());
            owned.extend(super::super::card_keys(d.id));
            seen.extend((0..defs.len()).filter(|&i| owned.contains(&defs[i].key) || super::super::cards::owns(d.id, defs[i].key)));
            seen.sort();
            assert_eq!(seen, (0..defs.len()).collect::<Vec<_>>(), "{}", d.id);
            assert!(secs.len() <= 12, "{} has {} sections", d.id, secs.len());
        }
    }
}
