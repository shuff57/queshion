//! The state a figure opens in on the Figures tab (the reference generators' first screen):
//! parameter overrides applied on top of each figure's `ParamDef` defaults when a figure is
//! picked, on "Reset all", and by the headless hooks.
//!
//! Kept separate from the `ParamDef` defaults on purpose: saved figures and presets store every
//! param, examples and classic setups are written against the plain defaults, and many unit
//! tests render the plain defaults, so none of those change when the opening state does.
use super::Value;

/// Overrides for figure `id`'s opening state (empty = open on the plain defaults).
pub fn overrides(id: &str) -> Vec<(&'static str, Value)> {
    let st = |v: &str| Value::S(v.to_string());
    match id {
        // Math polygons open unfilled (black outline on white), angles unshaded.
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" => vec![("fill", st("#ffffff"))],
        "angles" => vec![("shade", Value::B(false))],
        // Readings open with the magnifier on.
        "gas-syringe" | "mass-reading" | "temperature-reading" | "volume-reading" => vec![("mag_show", st("Instrument and magnifier"))],
        "length-reading" => vec![("mag_show", st("Ruler and magnifiers"))],
        "ruler-reading" => vec![("magnify", st("Ruler and magnifiers"))],
        // Chemistry starting examples.
        "line-spectrum" => vec![("strips", st("Hydrogen; Helium; Sodium; Unknown = Hydrogen + Sodium"))],
        // the reference opens on percent abundance (78.99 %, "Abundance (%)")
        "mass-spectrum" => vec![("element", st("Magnesium")), ("values", Value::B(true)), ("relative", Value::B(false))],
        "photoelectron-spectrum" => vec![("element", st("11 Na - Sodium"))],
        "orbital-diagram" => vec![("element", st("8 O - Oxygen"))],
        "particle-diagram" => vec![
            ("a_count", Value::I(4)), ("a_size", st("L")), ("a_shade", st("Light gray")), ("a_charge", st("\u{2212}")),
            ("b_count", Value::I(4)), ("b_size", st("S")), ("b_shade", st("White")), ("b_charge", st("+")),
        ],
        "bohr-model" => vec![("nucleus", st("Balls"))],
        "heating-cooling-curve" => vec![("curve_color", st("#c0392b")), ("plateau_labels", st("The change (Melting)"))],
        // Blank starting grids for students.
        "coordinate-grid" => vec![("equations", st("")), ("xmax", Value::F(15.0)), ("ymax", Value::F(15.0))],
        "number-line" => vec![("inequality", st(""))],
        // Mapping diagram: two empty, unfilled ovals; box plot: a bare number line.
        "mapping-diagram" => vec![("fill", st("White")), ("inputs", st("")), ("outputs", st("")), ("pairs", st(""))],
        "box-plot" => vec![("data", st(""))],
        // Solids open black and white.
        "prism" | "cylinder" | "pyramid" | "cone" | "sphere" => vec![("fill", st("White"))],
        // the reference draws the resultant dashed (added with share links v2)
        "vector-diagram" => vec![("res_style", st("Dashed"))],
        _ => Vec::new(),
    }
}

/// Opening-state keys added after share links v1: a v1 link (no `v=`) was made against an
/// opening state without them, so they fall back to the plain default when it is opened.
pub fn since_v1(id: &str) -> &'static [&'static str] {
    match id {
        "vector-diagram" => &["res_style"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    /// Every override names a declared param with a value of the right type and in range, and
    /// the opening state renders.
    #[test]
    fn overrides_are_valid_and_render() {
        for d in registry() {
            let defs = (d.params)();
            for (k, v) in super::overrides(d.id) {
                let pd = defs.iter().find(|p| p.key == k).unwrap_or_else(|| panic!("{}: unknown key {k}", d.id));
                assert_eq!(std::mem::discriminant(&pd.default), std::mem::discriminant(&v), "{}: {k} has the wrong type", d.id);
                let mut p = Params::defaults(&defs);
                p.0.insert(k.to_string(), v.clone());
                p.normalize(&defs, false);
                assert_eq!(p.0.get(k), Some(&v), "{}: {k} out of range", d.id);
            }
            let p = start_params(&d);
            to_png(&(d.render)(&p), 0.5).unwrap_or_else(|e| panic!("{} start state failed: {e}", d.id));
        }
    }

    fn start(id: &str) -> Params {
        let d = registry().into_iter().find(|d| d.id == id).unwrap();
        start_params(&d)
    }

    #[test]
    fn representative_opening_states() {
        assert_eq!(start("volume-reading").s("mag_show"), "Instrument and magnifier");
        assert_eq!(start("length-reading").s("mag_show"), "Ruler and magnifiers");
        assert_eq!(start("mass-spectrum").s("element"), "Magnesium");
        assert!(!start("mass-spectrum").b("relative"), "mass spectrum opens on percent abundance");
        assert_eq!(start("orbital-diagram").s("element"), "8 O - Oxygen");
        assert_eq!(start("triangle").s("fill"), "#ffffff");
        let g = start("coordinate-grid");
        assert_eq!(g.s("equations"), "");
        assert_eq!(g.f("xmax"), 15.0);
        // figures without overrides open on their plain defaults
        let d = registry().into_iter().find(|d| d.id == "waves").unwrap();
        assert_eq!(format!("{:?}", start_params(&d).0), format!("{:?}", Params::defaults(&(d.params)()).0));
    }

    /// The blank openings draw the bare figure (no "Enter ..." prompt), solids open white.
    #[test]
    fn blank_and_white_openings() {
        for id in ["box-plot", "mapping-diagram"] {
            let d = registry().into_iter().find(|d| d.id == id).unwrap();
            let svg = (d.render)(&start_params(&d));
            assert!(!svg.contains("Enter "), "{id}: {svg}");
        }
        let bp = registry().into_iter().find(|d| d.id == "box-plot").unwrap();
        let svg = (bp.render)(&start_params(&bp));
        assert!(svg.contains(">10<") && !svg.contains("<rect x=\"1"), "box plot: a bare 0-10 line");
        let md = registry().into_iter().find(|d| d.id == "mapping-diagram").unwrap();
        let svg = (md.render)(&start_params(&md));
        assert_eq!(svg.matches("<ellipse").count(), 2);
        assert!(!svg.contains("<polygon"), "no arrows on the blank diagram");
        for id in ["prism", "cylinder", "pyramid", "cone", "sphere"] {
            assert_eq!(start(id).s("fill"), "White", "{id}");
        }
    }

    /// The overrides only touch the opening state: plain defaults (used by saved specs, examples
    /// and the other tests) are unchanged.
    #[test]
    fn plain_defaults_untouched() {
        let d = registry().into_iter().find(|d| d.id == "mass-spectrum").unwrap();
        assert_eq!(Params::defaults(&(d.params)()).s("element"), "Chlorine");
    }
}

