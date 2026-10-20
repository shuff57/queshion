//! Direct-manipulation handles (see `figures::interact`). Geometry lives next to each renderer
//! (`measure`, `curves`, `atoms`, `lewis`, `spectra`); this file only dispatches by figure id.
use crate::figures::interact::Handle;
use crate::figures::Params;

use super::{atoms, curves, lewis, measure, spectra};

pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<Handle>> {
    measure::handles(id, p, size)
        .or_else(|| curves::handles(id, p, size))
        .or_else(|| atoms::handles(id, p, size))
        .or_else(|| lewis::handles(id, p, size))
        .or_else(|| spectra::handles(id, p, size))
}

/// Set float `key` to `v` clamped into the effective range `[lo, hi]` the renderer uses. If that
/// equals what the renderer already shows (current value clamped the same way) the stored
/// parameter is left untouched, so an off-scale stored value is not rewritten by a no-op drag.
pub(super) fn set_eff(p: &mut Params, key: &str, v: f64, lo: f64, hi: f64) {
    let cur = p.f(key).clamp(lo, hi);
    let nv = v.clamp(lo, hi);
    if (nv - cur).abs() > 1e-9 {
        p.set_f(key, nv);
    }
}

#[cfg(test)]
mod tests {
    use crate::figures::{registry, Params};

    fn fig(id: &str) -> (crate::figures::FigureDef, Params) {
        let d = registry().into_iter().find(|d| d.id == id).unwrap();
        let p = Params::defaults(&(d.params)());
        (d, p)
    }

    #[test]
    fn dragging_moves_the_reading() {
        let (d, mut p) = fig("volume-reading");
        let h = d.handles(&p, (360.0, 600.0)).remove(0);
        (h.drag.unwrap())(&mut p, (76.0, 272.0)); // halfway up the 100 mL cylinder
        assert!((p.f("reading") - 50.0).abs() < 0.2, "{}", p.f("reading"));
    }

    #[test]
    fn heating_plateau_switches_to_custom() {
        let (d, mut p) = fig("heating-cooling-curve");
        let hs = d.handles(&p, (750.0, 480.0));
        let melt = hs.iter().find(|h| h.tip.starts_with("Melting")).unwrap();
        (melt.drag.clone().unwrap())(&mut p, (300.0, 200.0));
        assert_eq!(p.s("substance"), "Custom");
        assert!(p.f("mp") > 20.0 && p.f("bp") == 100.0);
    }

    #[test]
    fn lewis_click_cycles_bond_via_tweak() {
        let (d, mut p) = fig("lewis-structures");
        p.set_s("formula", "CO2");
        let hs = d.handles(&p, (300.0, 200.0));
        let bond = hs.iter().find(|h| h.tip.contains("bond")).unwrap();
        (bond.click.clone().unwrap())(&mut p);
        assert!(p.s("tweak").starts_with("b0="), "{}", p.s("tweak"));
        assert!((d.render)(&p).contains("<svg"));
    }
}
