//! Physics figure generators (original re-creations of the common physics teaching figures).
use super::*;

mod handles;
mod common;
mod graphs;
mod instr;
mod mech;

/// Direct-manipulation handles for this subject's figures (None = not one of ours).
pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<interact::Handle>> {
    handles::handles(id, p, size)
}

pub fn defs() -> Vec<FigureDef> {
    let mut v = mech::defs();
    v.extend(graphs::defs());
    v.extend(instr::defs());
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

    #[test]
    fn nine_physics_generators() {
        let ids: Vec<_> = defs().iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), 9);
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
                    Kind::Text | Kind::Long | Kind::List | Kind::Color(_) => {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::S(String::new()));
                        check(&d, &p, "sweep");
                    }
                }
            }
        }
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
