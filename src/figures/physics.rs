//! Physics figure generators (original re-creations of the common physics teaching figures).
use super::*;

mod handles;
mod circuit_solve;
mod common;
mod graphs;
mod instr;
mod mech;
mod optics;
mod pulley;
pub(crate) use pulley::{pulley_relevant, pulley_setup_changed};
mod examples;
mod summary;

/// The collapsed-row summary of a section on the physics page skin, where the page has one.
pub fn skin_summary(id: &str, title: &str, p: &Params) -> Option<String> {
    match id {
        "waves" => graphs::wave_summary(title, p),
        "inclined-plane" => mech::incline_summary(title, p),
        _ if summary::summary(id, title, p).is_some() => summary::summary(id, title, p),
        // the reference words the Figure card's summary as "black and white" (or "color")
        _ if title == "Figure" && matches!(id, "free-body-diagram" | "vector-diagram" | "pulley" | "projectile-motion" | "motion-graphs" | "circuit-diagram") => {
            Some(if p.b("color") { "color" } else { "black and white" }.into())
        }
        _ => None,
    }
}

/// The grey worked line under a motion-graph segment: "A to B (0-4 s): speeding up from 0 to 4 m/s, ...".
pub fn segment_caption(p: &Params, i: usize) -> String {
    graphs::segment_caption(p, i)
}

/// Whether segment `i` (1-based) of the motion graphs asks which way it speeds up: only a
/// speed-up from rest has a direction of its own.
pub fn segment_has_way(p: &Params, i: usize) -> bool {
    graphs::segment_has_way(p, i)
}

/// How many objects the inclined plane draws.
pub fn incline_objects(p: &Params) -> usize {
    mech::incline_count(p)
}

/// "Start from an example" entries for this subject's figures.
pub fn examples(id: &str) -> Option<Vec<super::Example>> {
    examples::examples(id)
}

/// "Add a force / vector / segment / part / object" chips for the figures that keep a numbered list of items.
pub fn chips(id: &str) -> Option<super::ChipsCard> {
    use super::{Chip, ChipsCard, Value};
    let force = |label, ang: f64, lab: &'static str| Chip {
        label,
        set: vec![("f{n}_ang", Value::F(ang)), ("f{n}_len", Value::F(1.0)), ("f{n}_lab", Value::S(lab.into())), ("f{n}_lab_mode", Value::S("Text".into()))],
    };
    match id {
        "free-body-diagram" => Some(ChipsCard {
            title: "Add a force",
            count_key: "n",
            max: 8,
            chips: vec![
                force("Gravity", 270.0, "F_g"),
                force("Normal", 90.0, "F_N"),
                force("Friction", 180.0, "F_f"),
                force("Tension", 90.0, "F_T"),
                force("Applied", 0.0, "F_app"),
                force("Spring", 0.0, "F_s"),
                force("Air resistance", 180.0, "F_air"),
                force("Force", 45.0, "F"),
            ],
        }),
        "vector-diagram" => Some(ChipsCard {
            title: "Add a vector",
            count_key: "n",
            max: 4,
            chips: vec![Chip {
                label: "Vector",
                set: vec![("v{n}_mag", Value::F(3.0)), ("v{n}_ang", Value::F(90.0)), ("v{n}_lab", Value::S("{L}".into())), ("v{n}_lab_mode", Value::S("Text".into()))],
            }],
        }),
        "motion-graphs" => {
            // the reference's "Add a segment" buttons: kind, speed, 3 s, forward
            let seg = |label: &'static str, v: f64| Chip {
                label,
                set: vec![("s{n}_kind", Value::S(label.into())), ("s{n}_v", Value::F(v)), ("s{n}_t", Value::F(3.0)), ("s{n}_dir", Value::S("Forward".into()))],
            };
            Some(ChipsCard {
                title: "Add a segment",
                count_key: "n",
                max: 6,
                chips: vec![
                    seg("At rest", 0.0),
                    seg("Constant velocity forward", 4.0),
                    seg("Constant velocity back", 4.0),
                    seg("Speeding up", 4.0),
                    seg("Slowing down", 0.0),
                ],
            })
        }
        "circuit-diagram" => {
            // a new part keeps the slot's own value (2, 4, 6, 8 ohms) and an automatic name
            let part = |label: &'static str| Chip {
                label,
                set: vec![("r{n}_kind", Value::S(label.into())), ("r{n}_name_mode", Value::S("Text".into())), ("r{n}_name", Value::S(String::new())), ("r{n}_val_mode", Value::S("Text".into()))],
            };
            Some(ChipsCard { title: "Add a resistor or bulb", count_key: "n", max: 4, chips: vec![part("Resistor"), part("Bulb")] })
        }
        "inclined-plane" => {
            // objects 2 and 3 sit above object 1 (the legacy single-object keys)
            let obj = |label: &'static str| Chip {
                label,
                set: vec![("obj{n}_kind", Value::S(label.into())), ("obj{n}_lab_mode", Value::S("Text".into())), ("obj{n}_size", Value::I(100))],
            };
            Some(ChipsCard { title: "Add an object above", count_key: "n", max: 3, chips: vec![obj("Block"), obj("Ball"), obj("Cart")] })
        }
        _ => None,
    }
}

/// Direct-manipulation handles for this subject's figures (None = not one of ours).
pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<interact::Handle>> {
    handles::handles(id, p, size)
}

/// The spring scale's "Random" button: a force between 5% and 95% of the capacity, to 2 decimal
/// places (the reference's readings, e.g. 3.48, 8.27 N on the 10 N scale).
pub fn random_reading(id: &str, p: &mut Params, seed: u64) -> bool {
    if id != "spring-scale" {
        return false;
    }
    let cap = p.s("cap").trim_end_matches(" N").parse::<f64>().unwrap_or(10.0);
    let v = cap * (0.05 + 0.9 * super::unit_random(seed));
    p.set_f("reading", (v * 100.0).round() / 100.0);
    true
}

pub fn defs() -> Vec<FigureDef> {
    let mut v = mech::defs();
    v.extend(graphs::defs());
    v.extend(instr::defs());
    v.extend(optics::defs());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = "/tmp/claude-1000/-home-shuff57/4fa1e9b4-a716-475e-b753-2c60208b3c78/scratchpad/physics";

    fn check(d: &FigureDef, p: &Params, tag: &str) {
        let svg = (d.render)(p);
        let png = to_png(&svg, 1.0).unwrap_or_else(|e| panic!("{} [{tag}] failed: {e}", d.id));
        assert!(png.len() > 500, "{} [{tag}] empty", d.id);
        if std::fs::create_dir_all(OUT).is_ok() {
            let _ = std::fs::write(format!("{OUT}/{}_{tag}.png", d.id), png);
        }
    }

    /// Round-5 cosmetics: bold arrowheads, tenths on the spring scale's dial, and the vector
    /// diagram opening with a dashed resultant (its plain default, used by saved figures and
    /// examples, stays drawn).
    #[test]
    fn cosmetics_match_the_reference() {
        assert!(common::head_len(3.5) >= 19.0 && common::head_len(1.0) >= 12.0);
        let get = |id: &str| defs().into_iter().find(|d| d.id == id).unwrap();
        let sp = get("spring-scale");
        let svg = (sp.render)(&Params::defaults(&(sp.params)()));
        // 0..10 N in tenths (101 marks) where fifths gave 51
        assert!(svg.matches("<line").count() >= 101, "tenths drawn: {}", svg.matches("<line").count());
        let vd = get("vector-diagram");
        assert_eq!(Params::defaults(&(vd.params)()).s("res_style"), "Drawn");
        assert_eq!(start_params(&vd).s("res_style"), "Dashed");
        let dashed = (vd.render)(&start_params(&vd));
        assert!(dashed.contains("stroke-dasharray"), "the resultant opens dashed");
    }

    #[test]
    fn eleven_physics_generators() {
        let ids: Vec<_> = defs().iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), 11);
        assert!(defs().iter().all(|d| d.subject == Subject::Physics));
    }

    /// Defaults, every choice option, and min/max of every numeric param render validly.
    #[test]
    fn physics_sweeps_params() {
        for d in defs() {
            let defs_ = (d.params)();
            let base = Params::defaults(&defs_);
            check(&d, &base, "default");
            for pd in &defs_ {
                match &pd.kind {
                    Kind::Choice(opts) => for o in opts {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::S((*o).into()));
                        check(&d, &p, "sweep");
                    },
                    Kind::Float { min, max, .. } => for v in [*min, *max] {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::F(v));
                        check(&d, &p, "sweep");
                    },
                    Kind::Int { min, max } => for v in [*min, *max] {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::I(v));
                        check(&d, &p, "sweep");
                    },
                    Kind::Bool => {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::B(true));
                        check(&d, &p, "sweep");
                    }
                    Kind::Text | Kind::Long | Kind::List | Kind::Data { .. } | Kind::Color(_) => {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::S(String::new()));
                        check(&d, &p, "sweep");
                    }
                }
            }
        }
    }

    /// Every chip key resolves to a real param for every slot it can fill, and the count param's
    /// range matches the card's maximum.
    #[test]
    fn physics_chips_fill_real_params() {
        for d in defs() {
            let Some(card) = chips(d.id) else { continue };
            let pdefs = (d.params)();
            let count = pdefs.iter().find(|p| p.key == card.count_key).unwrap_or_else(|| panic!("{}: no count key", d.id));
            match count.kind { Kind::Int { max, .. } => assert_eq!(max, card.max, "{}", d.id), _ => panic!("{}: count not int", d.id) }
            for slot in 2..=card.max {
                for chip in &card.chips {
                    for (k, v) in &chip.set {
                        let key = k.replace("{n}", &slot.to_string());
                        let pd = pdefs.iter().find(|p| p.key == key).unwrap_or_else(|| panic!("{}: chip {} sets unknown {key}", d.id, chip.label));
                        if let (Kind::Choice(opts), Value::S(s)) = (&pd.kind, v) {
                            assert!(opts.contains(&s.as_str()), "{}: {key} has no option {s}", d.id);
                        }
                    }
                }
            }
        }
        for id in ["free-body-diagram", "vector-diagram", "motion-graphs", "circuit-diagram", "inclined-plane"] {
            assert!(chips(id).is_some(), "{id}");
        }
        assert_eq!(chips("free-body-diagram").unwrap().max, 8);
        assert_eq!(chips("motion-graphs").unwrap().max, 6);
    }

    /// Old two-block specs still give two blocks; `n` gives 1-3 objects foot first, in order.
    #[test]
    fn incline_rows() {
        let all = defs();
        let d = all.iter().find(|d| d.id == "inclined-plane").unwrap();
        let p = with(d, &[("two", Value::B(true))]);
        assert_eq!(mech::incline_count(&p), 2);
        let objs = mech::incline_objects(&p, 400.0);
        assert!(objs.iter().all(|o| o.kind == "Block"));
        for (n, joined) in [(1, "By strings"), (2, "By strings"), (3, "By strings"), (3, "Touching")] {
            let p = with(d, &[("n", Value::I(n)), ("joined", s(joined)), ("obj2_kind", s("Cart")), ("obj3_kind", s("Ball"))]);
            let o = mech::incline_objects(&p, 550.0);
            assert_eq!(o.len(), n as usize);
            for w in o.windows(2) {
                let gap = (w[1].s - mech::obj_half_len(w[1].kind, w[1].sz)) - (w[0].s + mech::obj_half_len(w[0].kind, w[0].sz));
                if joined == "Touching" { assert!(gap.abs() < 1e-6); } else { assert!(gap > 7.0); }
            }
        }
        // single object: unchanged centre at pos * slope length
        let p = with(d, &[]);
        let o = mech::incline_objects(&p, 400.0);
        assert!((o[0].s - 0.55 * 400.0).abs() < 1e-9);
    }

    fn with(d: &FigureDef, kv: &[(&str, Value)]) -> Params {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in kv { p.0.insert((*k).into(), v.clone()); }
        p
    }
    fn s(x: &str) -> Value { Value::S(x.into()) }

    /// Showcase renders used for visual checks (written to the scratch dir).
    #[test]
    fn physics_showcase() {
        let all = defs();
        let get = |id: &str| all.iter().find(|d| d.id == id).unwrap().clone();
        let d = get("free-body-diagram");
        check(&d, &with(&d, &[("body", s("Block")), ("n", Value::I(4)), ("notation", s("Arrow")), ("motion", s("Both")), ("mdir", Value::F(30.0))]), "show1");
        let d = get("vector-diagram");
        check(&d, &with(&d, &[("n", Value::I(3)), ("v3_ang", Value::F(200.0)), ("mark_angle", Value::B(true)), ("components", Value::B(true))]), "show1");
        let d = get("inclined-plane");
        check(&d, &with(&d, &[("angle", Value::F(40.0)), ("gravity", Value::B(true)), ("normal", Value::B(true)), ("friction", s("Up the ramp")), ("applied", s("Up the ramp")), ("velocity", s("Down the ramp")), ("mark_len", Value::B(true)), ("mark_h", Value::B(true)), ("surface", s("Rough"))]), "show1");
        let d = get("pulley");
        check(&d, &with(&d, &[("tension", Value::B(true)), ("gravity", Value::B(true)), ("accel", s("Right falls")), ("third", Value::B(true))]), "show1");
        for su in ["Table", "Ramp", "Block & tackle"] {
            check(&d, &with(&d, &[("setup", s(su)), ("tension", Value::B(true)), ("accel", s("Right falls"))]), &format!("show_{}", su.replace(' ', "").replace('&', "")));
        }
        let d = get("projectile-motion");
        check(&d, &with(&d, &[("from", s("A cliff")), ("comps", Value::B(true)), ("steps", Value::I(6)), ("mark_h", Value::B(true)), ("mark_r", Value::B(true)), ("show_g", Value::B(true))]), "show1");
        let d = get("motion-graphs");
        check(&d, &with(&d, &[("show", s("All three, stacked")), ("letters", Value::B(true))]), "show1");
        let d = get("waves");
        check(&d, &with(&d, &[("kind", s("Both")), ("mark_ct", Value::B(true))]), "show1");
        let d = get("spring-scale");
        check(&d, &with(&d, &[("units", s("Both")), ("reading", Value::F(6.2)), ("answer", Value::B(true))]), "show1");
        let d = get("circuit-diagram");
        check(&d, &with(&d, &[("switch", s("Open")), ("ammeter", Value::B(true)), ("voltmeter", s("Across component 2")), ("polarity", Value::B(true))]), "show1");
        check(&d, &with(&d, &[("layout", s("In parallel")), ("n", Value::I(3)), ("r2_kind", s("Bulb")), ("ammeter", Value::B(true)), ("switch", s("Closed")), ("voltmeter", s("Across the battery"))]), "show2");
    }
}
#[cfg(test)]
mod dbg_svg {
    #[test]
    fn dump_svg() {
        let Ok(spec) = std::env::var("PHYS_SVG") else { return };
        let (id, js) = spec.split_once('|').unwrap();
        let def = super::defs().into_iter().find(|d| d.id == id).unwrap();
        let p = crate::figures::params_from_json(&def, &serde_json::from_str(js).unwrap());
        println!("{}", (def.render)(&p));
    }
}
#[cfg(test)]
mod dbg_examples {
    use super::*;
    /// Every example applies only known keys and renders; with PHYS_EX_DIR set the renders
    /// are written there as <id>_ex<k>.png for eyeballing.
    #[test]
    fn physics_examples_render() {
        let dir = std::env::var("PHYS_EX_DIR").ok();
        for d in defs() {
            let defs_ = (d.params)();
            let ex = examples(d.id).unwrap_or_default();
            for (k, e) in ex.iter().enumerate() {
                let mut p = Params::defaults(&defs_);
                for (key, v) in &e.params {
                    assert!(defs_.iter().any(|pd| pd.key == *key), "{}: example '{}' uses unknown key {key}", d.id, e.name);
                    p.0.insert((*key).into(), v.clone());
                }
                p.normalize(&defs_, false);
                let png = to_png(&(d.render)(&p), 1.0).unwrap();
                if let Some(dir) = &dir {
                    let _ = std::fs::create_dir_all(dir);
                    std::fs::write(format!("{dir}/{}_ex{k}.png", d.id), png).unwrap();
                }
            }
        }
    }
}
