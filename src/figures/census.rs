//! Render census (QA, ignored by default): writes the SVG of every figure in many states to
//! `$QUESHION_CENSUS_DIR`, one file per case, so two builds can be compared file by file
//! (`diff -r before after`). Cases per figure: plain defaults (what saved figures, presets and
//! examples build on), the opening state, every example, every classic setup, every option of
//! every choice and both values of every switch (over the plain defaults), and a slider nudge.
//!
//! `QUESHION_CENSUS_DIR=/tmp/x cargo test --release census_dump -- --ignored`
use super::*;

fn put(dir: &std::path::Path, name: String, svg: &str) {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect();
    std::fs::write(dir.join(format!("{safe}.svg")), svg).unwrap();
}

#[test]
#[ignore]
fn census_dump() {
    let Ok(dir) = std::env::var("QUESHION_CENSUS_DIR") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut n = 0;
    for d in registry() {
        let defs = (d.params)();
        let base = Params::defaults(&defs);
        let mut case = |tag: String, p: &Params| {
            put(&dir, format!("{}__{tag}", d.id), &(d.render)(p));
            n += 1;
        };
        case("defaults".into(), &base);
        case("start".into(), &start_params(&d));
        let over = |list: &[(&'static str, Value)]| {
            let mut p = base.clone();
            for (k, v) in list {
                p.0.insert(k.to_string(), v.clone());
            }
            p.normalize(&defs, false);
            p
        };
        for (i, e) in examples_for(d.id).iter().enumerate() {
            case(format!("ex{i}"), &over(&e.params));
        }
        for (i, e) in classic_setups(d.id).iter().enumerate() {
            case(format!("setup{i}"), &over(&e.params));
        }
        for pd in &defs {
            match &pd.kind {
                Kind::Choice(opts) => {
                    for (i, o) in opts.iter().enumerate() {
                        let mut p = base.clone();
                        p.set_s(pd.key, o);
                        case(format!("{}={i}", pd.key), &p);
                    }
                }
                Kind::Bool => {
                    let mut p = base.clone();
                    p.set_b(pd.key, !base.b(pd.key));
                    case(format!("{}=flip", pd.key), &p);
                }
                Kind::Float { min, max, .. } => {
                    let mut p = base.clone();
                    p.set_f(pd.key, min + (max - min) * 0.3);
                    case(format!("{}=0.3", pd.key), &p);
                }
                Kind::Int { min, max } => {
                    let mut p = base.clone();
                    p.set_i(pd.key, min + (max - min) / 3);
                    case(format!("{}=third", pd.key), &p);
                }
                _ => {}
            }
        }
    }
    // composite presets (stored as spec text)
    for name in math::COMPOSITE_STARTS {
        let d = registry().into_iter().find(|d| d.id == "composite-shape").unwrap();
        let mut p = Params::defaults(&(d.params)());
        p.set_s("spec", math::preset_spec(name).unwrap_or(""));
        put(&dir, format!("composite-shape__preset_{name}"), &(d.render)(&p));
        n += 1;
    }
    eprintln!("census: {n} renders in {}", dir.display());
}
