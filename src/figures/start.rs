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
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "regular-polygon" => vec![("fill", st("#ffffff"))],
        // the reference names the kite counter-clockwise (A right, B top, C left): our mirror
        // image, so it opens flipped (share links v3)
        "kite" => vec![("fill", st("#ffffff")), ("flip", Value::B(true))],
        // the reference draws the bare angle: no point letters (share links v3)
        "angles" => vec![("shade", Value::B(false)), ("letters", Value::B(false))],
        // the reference opens on the bare lines and transversal, no angle marked (share links v3)
        "parallel-lines" => vec![("marked", st("")), ("unknown", st(""))],
        // Readings open with the magnifier on.
        "gas-syringe" | "mass-reading" | "temperature-reading" | "volume-reading" => vec![("mag_show", st("Instrument and magnifier"))],
        // the reference: one magnifier above the object's right end (share links v3)
        "length-reading" => vec![("mag_show", st("Ruler and one magnifier above"))],
        "ruler-reading" => vec![("magnify", st("Ruler and magnifiers"))],
        // the reference's micropipette: a short body with a long shaft and tip (share links v4)
        "micropipette-reading" => vec![("shape", st("Short body, long shaft and tip"))],
        // Chemistry starting examples.
        "line-spectrum" => vec![("strips", st("Hydrogen; Helium; Sodium; Unknown = Hydrogen + Sodium"))],
        // the reference opens on percent abundance (78.99 %, "Abundance (%)")
        "mass-spectrum" => vec![("element", st("Magnesium")), ("values", Value::B(true)), ("relative", Value::B(false))],
        "photoelectron-spectrum" => vec![("element", st("11 Na - Sodium"))],
        // the reference leaves the configuration line off ("Nothing left blank"; share links v3)
        "orbital-diagram" => vec![("element", st("8 O - Oxygen")), ("config", Value::B(false))],
        "particle-diagram" => vec![
            ("a_count", Value::I(4)), ("a_size", st("L")), ("a_shade", st("Light gray")), ("a_charge", st("\u{2212}")),
            ("b_count", Value::I(4)), ("b_size", st("S")), ("b_shade", st("White")), ("b_charge", st("+")),
        ],
        "bohr-model" => vec![("nucleus", st("Balls"))],
        // the reference's meter: ON/OFF and CAL, the probe standing in a beaker (share links v3)
        "ph-reading" => vec![("meter", st("Probe in a beaker"))],
        // the reference's grey marbles and no arrow between the cylinders (share links v3)
        "volume-by-displacement" => vec![("marble_color", st("Gray")), ("arrow", Value::B(false))],
        "heating-cooling-curve" => vec![("curve_color", st("#c0392b")), ("plateau_labels", st("The change (Melting)")), ("scale", st("To scale"))],
        // Blank starting grids for students.
        "coordinate-grid" => vec![("equations", st("")), ("xmax", Value::F(15.0)), ("ymax", Value::F(15.0))],
        "number-line" => vec![("inequality", st(""))],
        // Mapping diagram: two empty, unfilled ovals; box plot: a bare number line.
        "mapping-diagram" => vec![("fill", st("White")), ("inputs", st("")), ("outputs", st("")), ("pairs", st(""))],
        "box-plot" => vec![("data", st(""))],
        // Solids open black and white.
        "prism" | "cylinder" | "pyramid" | "cone" | "sphere" => vec![("fill", st("White"))],
        // the reference draws the resultant dashed (added with share links v2)
        "vector-diagram" => vec![("res_style", st("Dashed")), ("bold", Value::B(true))],
        // the reference's ball sits centred on the ground line (share links v5)
        "projectile-motion" => vec![("ball_at", st("Is centred on the ground line"))],
        _ => Vec::new(),
    }
}

/// Opening-state keys by the share-link version that introduced them. A link written with an
/// older version (`share::version`) was made against an opening state without them, so they fall
/// back to the plain default when it is opened (see `newer_than` / `older_values`).
pub fn added_in(id: &str) -> &'static [(u32, &'static [&'static str])] {
    match id {
        "vector-diagram" => &[(2, &["res_style"]), (6, &["bold"])],
        "orbital-diagram" => &[(3, &["config"])],
        "angles" => &[(3, &["letters"])],
        "parallel-lines" => &[(3, &["marked", "unknown"])],
        "volume-by-displacement" => &[(3, &["marble_color", "arrow"])],
        "ph-reading" => &[(3, &["meter"])],
        "kite" => &[(3, &["flip"])],
        "length-reading" => &[(3, &["mag_show"])],
        "micropipette-reading" => &[(4, &["shape"])],
        "projectile-motion" => &[(5, &["ball_at"])],
        _ => &[],
    }
}

/// A key whose opening value was already overridden before the version that changed it: what
/// links of earlier versions opened with (instead of the plain default).
fn was_overridden(id: &str, key: &str) -> Option<Value> {
    match (id, key) {
        // v1/v2 opened with the two magnifiers under the ruler
        ("length-reading", "mag_show") => Some(Value::S("Ruler and magnifiers".into())),
        _ => None,
    }
}

/// The opening-state keys a link written with format version `v` did not know about: they read
/// as their older opening value, so the link opens exactly as it did when it was made.
pub fn newer_than(id: &str, v: u32) -> Vec<&'static str> {
    added_in(id).iter().filter(|(since, _)| *since > v).flat_map(|(_, keys)| keys.iter().copied()).collect()
}

/// For a link of version `v`: each newer opening key with the value it opened with then (`None`
/// = the plain `ParamDef` default).
pub fn older_values(id: &str, v: u32) -> Vec<(&'static str, Option<Value>)> {
    newer_than(id, v).into_iter().map(|k| (k, was_overridden(id, k))).collect()
}

/// Opening-state keys added after share links v1 (kept for callers of the round-5 API).
pub fn since_v1(id: &str) -> Vec<&'static str> {
    newer_than(id, 1)
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
        assert_eq!(start("length-reading").s("mag_show"), "Ruler and one magnifier above");
        assert_eq!(super::older_values("length-reading", 2), vec![("mag_show", Some(Value::S("Ruler and magnifiers".into())))]);
        assert_eq!(super::older_values("kite", 2), vec![("flip", None)]);
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

    /// Round-6/P4 opening states (the reference's first screen), each listed under the link
    /// version that introduced it, and every overridden key is versioned or was there at v1.
    #[test]
    fn p4_opening_states_and_versions() {
        assert!(!start("orbital-diagram").b("config"), "no configuration line");
        assert!(!start("angles").b("letters"), "no point letters");
        let d = registry().into_iter().find(|d| d.id == "ph-reading").unwrap();
        let open = (d.render)(&start_params(&d));
        assert!(open.contains(">ON/OFF<") && open.contains(">CAL<") && !open.contains(">MODE<"), "bench meter: {open}");
        assert!(open.contains("meter reading 4.62<"));
        let plain = (d.render)(&Params::defaults(&(d.params)()));
        assert!(plain.contains(">MODE<"), "saved meters keep the hanging probe");
        let mut one = start_params(&d);
        one.set_s("places", "0.0");
        assert!((d.render)(&one).contains("meter reading 4.6<"));
        assert!(start("kite").b("flip"), "kite opens mirrored like the reference");
        assert_eq!(start("kite").s("fill"), "#ffffff");
        let vd = start("volume-by-displacement");
        assert_eq!((vd.s("marble_color"), vd.b("arrow")), ("Gray", false));
        let d = registry().into_iter().find(|d| d.id == "volume-by-displacement").unwrap();
        let open = (d.render)(&start_params(&d));
        let plain = (d.render)(&Params::defaults(&(d.params)()));
        assert!(open.contains("#8f8f8f") && !open.contains("#8fb4d6"), "grey marbles");
        assert!(plain.contains("#8fb4d6"), "saved figures keep blue marbles");
        assert!(open.matches("<polygon").count() < plain.matches("<polygon").count(), "no arrow head");
        let pl = start("parallel-lines");
        assert_eq!((pl.s("marked"), pl.s("unknown")), ("", ""));
        let d = registry().into_iter().find(|d| d.id == "parallel-lines").unwrap();
        let svg = (d.render)(&start_params(&d));
        assert!(!svg.contains("65\u{b0}") && !svg.contains(">x<"), "no angle marks: {svg}");
        assert_eq!(super::newer_than("parallel-lines", 2), vec!["marked", "unknown"]);
        assert!(super::newer_than("parallel-lines", 3).is_empty());
        assert_eq!(super::newer_than("vector-diagram", 1), vec!["res_style", "bold"]);
        assert_eq!(super::newer_than("vector-diagram", 2), vec!["bold"]);
        assert!(super::newer_than("vector-diagram", 6).is_empty());
        assert_eq!(super::since_v1("orbital-diagram"), vec!["config"]);
        for (_, keys) in super::added_in("angles") {
            for k in *keys {
                assert!(super::overrides("angles").iter().any(|(o, _)| o == k), "{k}");
            }
        }
        for d in registry() {
            for (v, _) in super::added_in(d.id) {
                assert!((2..=crate::figures::share::VERSION).contains(v), "{}", d.id);
            }
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

