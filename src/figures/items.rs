//! Numbered item slots (`f1_ang`, `v2_mag`, ...) edited as cards: delete any item (the later
//! ones move up a slot) and swap two. Values stay in the same params the generators read, so
//! saved figures are untouched until edited.

use super::layout::item_of;
use super::{ParamDef, Params};

/// The params of slot `idx` of the `letters` family, with the part of the key after the slot
/// number (`f2_lab_mode` -> `lab_mode`).
fn slot<'a>(defs: &'a [ParamDef], letters: &str, idx: usize) -> Vec<(&'a ParamDef, &'a str)> {
    defs.iter()
        .filter(|d| item_of(d.key) == Some((letters, idx)))
        .filter_map(|d| d.key.split_once('_').map(|(_, rest)| (d, rest)))
        .collect()
}

fn key(letters: &str, idx: usize, rest: &str) -> String {
    format!("{letters}{idx}_{rest}")
}

/// Removes item `idx` (1-based) of `count_key` items: the items after it move up one slot and
/// the freed last slot goes back to its defaults. Keeps at least one item. Returns whether it did.
pub fn remove_item(p: &mut Params, defs: &[ParamDef], letters: &str, count_key: &str, idx: usize) -> bool {
    let n = p.i(count_key).max(0) as usize;
    if idx == 0 || idx > n || n <= 1 {
        return false;
    }
    for j in idx..n {
        for (_, rest) in slot(defs, letters, j + 1) {
            if let Some(v) = p.0.get(&key(letters, j + 1, rest)).cloned() {
                p.0.insert(key(letters, j, rest), v);
            }
        }
    }
    for (d, _) in slot(defs, letters, n) {
        p.0.insert(d.key.to_string(), d.default.clone());
    }
    p.set_i(count_key, n as i64 - 1);
    true
}

/// Swaps items `a` and `b` (1-based) of the `letters` family.
pub fn swap_items(p: &mut Params, defs: &[ParamDef], letters: &str, a: usize, b: usize) {
    if a == b {
        return;
    }
    for (_, rest) in slot(defs, letters, a) {
        let (ka, kb) = (key(letters, a, rest), key(letters, b, rest));
        let va = p.0.get(&ka).cloned();
        let vb = p.0.get(&kb).cloned();
        match vb {
            Some(v) => p.0.insert(ka, v),
            None => p.0.remove(&ka),
        };
        match va {
            Some(v) => p.0.insert(kb, v),
            None => p.0.remove(&kb),
        };
    }
}

/// The reference's quick direction buttons and their angles (0° = right, counter-clockwise).
pub const QUICK_DIRS: [(&str, f64); 4] = [("Up", 90.0), ("Down", 270.0), ("Left", 180.0), ("Right", 0.0)];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::registry;

    fn fig(id: &str) -> (Vec<ParamDef>, Params, fn(&Params) -> String) {
        let d = registry().into_iter().find(|d| d.id == id).unwrap();
        let defs = (d.params)();
        let p = Params::defaults(&defs);
        (defs, p, d.render)
    }

    #[test]
    fn removing_a_middle_force_moves_the_rest_up() {
        let (defs, mut p, render) = fig("free-body-diagram");
        p.set_i("n", 3);
        p.set_s("f3_lab", "F_f");
        p.set_f("f3_ang", 180.0);
        p.set_b("f3_comps", true);
        // removing force 2 draws what forces 1 and 3 alone draw
        let mut want = p.clone();
        want.set_i("n", 2);
        for (k, v) in [("ang", 180.0)] {
            want.set_f(&format!("f2_{k}"), v);
        }
        want.set_s("f2_lab", "F_f");
        want.set_b("f2_comps", true);
        assert!(remove_item(&mut p, &defs, "f", "n", 2));
        assert_eq!(p.i("n"), 2);
        assert_eq!(p.s("f2_lab"), "F_f");
        assert_eq!(p.f("f2_ang"), 180.0);
        assert!(p.b("f2_comps"));
        assert_eq!(render(&p), render(&want));
        // the freed slot is back to its defaults, so adding a force there starts clean
        assert!(!p.b("f3_comps"));
        assert_eq!(p.s("f3_lab"), "F_f"); // slot 3's own default label
        // the last item stays
        assert!(remove_item(&mut p, &defs, "f", "n", 1));
        assert!(!remove_item(&mut p, &defs, "f", "n", 1));
        assert_eq!(p.i("n"), 1);
        assert!(!remove_item(&mut p, &defs, "f", "n", 5));
    }

    #[test]
    fn vectors_swap_and_delete() {
        let (defs, mut p, _) = fig("vector-diagram");
        p.set_i("n", 3);
        swap_items(&mut p, &defs, "v", 1, 2);
        assert_eq!((p.s("v1_lab"), p.f("v1_mag"), p.f("v1_ang")), ("B", 3.0, 90.0));
        assert_eq!((p.s("v2_lab"), p.f("v2_mag")), ("A", 4.0));
        assert!(remove_item(&mut p, &defs, "v", "n", 1));
        assert_eq!((p.i("n"), p.s("v1_lab"), p.s("v2_lab")), (2, "A", "C"));
        // other families are untouched
        assert!(p.0.contains_key("res_lab"));
    }
}
