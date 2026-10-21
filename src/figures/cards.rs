//! Figure-specific cards that take the place of generic controls in the side panel, as on the
//! reference generators: gel lane cards, per-force / per-vector cards with quick directions, the
//! free-body Motion card, the population-growth and predator-prey readouts with "Fit now", line
//! spectrum strip cards and particle-diagram kind cards. A card sits where one "anchor" param of
//! its section would be and owns the params it edits (they leave the generic sections). Every
//! card edits the params the generators already read, so saved figures render unchanged.

use super::biology::{gel, popdyn};
use super::chemistry::{atoms, spectra};
use super::layout::item_of;
use super::Params;

/// A custom card drawn in place of (or after) an anchor param.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    GelLanes,
    Forces,
    Motion,
    Vectors,
    GrowthFit,
    /// A predator-prey axis "fit" checkbox (true = x-axis) that keeps the range when unticked.
    PpFit(bool),
    PpReadout,
    /// The titration's equivalence-point readout box.
    TitReadout,
    Strips,
    Particles,
}

/// The card drawn instead of param `key`'s own control.
pub fn replaces(fig: &str, key: &str) -> Option<Card> {
    Some(match (fig, key) {
        ("gel-electrophoresis", "lanes") => Card::GelLanes,
        ("free-body-diagram", "n") => Card::Forces,
        ("free-body-diagram", "motion") => Card::Motion,
        ("vector-diagram", "n") => Card::Vectors,
        ("population-growth", "fit") => Card::GrowthFit,
        ("predator-prey", "x_fit") => Card::PpFit(true),
        ("predator-prey", "y_fit") => Card::PpFit(false),
        ("line-spectrum", "strips") => Card::Strips,
        ("particle-diagram", "shuffle") => Card::Particles,
        _ => return None,
    })
}

/// The card drawn right after param `key`'s control.
pub fn after(fig: &str, key: &str) -> Option<Card> {
    match (fig, key) {
        ("predator-prey", "unit") => Some(Card::PpReadout),
        ("titration-curve", "pk" | "kp_eqph") => Some(Card::TitReadout),
        _ => None,
    }
}

/// The old per-element ticks of the line spectrum (now edited as strip cards).
const SPECTRUM_OLD: [&str; 8] = ["custom", "hydrogen", "helium", "neon", "sodium", "mercury", "lithium", "calcium"];
const FBD_MOTION: [&str; 6] = ["mdir", "adir", "mv_lab_mode", "mv_lab", "ma_lab_mode", "ma_lab"];

/// The card that edits param `key` of figure `fig` (so it leaves the generic sections).
pub fn owner(fig: &str, key: &str) -> Option<Card> {
    let card = match fig {
        "gel-electrophoresis" if key == "setup" => Card::GelLanes,
        "free-body-diagram" if item_of(key).is_some_and(|(l, _)| l == "f") => Card::Forces,
        "free-body-diagram" if FBD_MOTION.contains(&key) => Card::Motion,
        "vector-diagram" if item_of(key).is_some_and(|(l, _)| l == "v") => Card::Vectors,
        "line-spectrum" if SPECTRUM_OLD.contains(&key) => Card::Strips,
        "particle-diagram" if atoms::is_kind_key(key) => Card::Particles,
        _ => return None,
    };
    Some(card)
}

/// Whether param `key` is edited by one of figure `fig`'s cards (or the pedigree's Family card).
pub fn owns(fig: &str, key: &str) -> bool {
    owner(fig, key).is_some() || (fig == "pedigree" && super::card_keys(fig).contains(&key))
}

/// Figures whose "Add ..." chips live inside a card instead of their own card above the sections.
pub fn embeds_chips(fig: &str) -> bool {
    matches!(fig, "free-body-diagram" | "vector-diagram" | "particle-diagram")
}

/// The section header summary when the section holds a card's anchor `key`.
pub fn summary(fig: &str, key: &str, p: &Params) -> Option<String> {
    let quoted = |letters: &str| {
        (1..=p.i("n").max(0))
            .filter(|i| p.s(&format!("{letters}{i}_lab_mode")) != "None")
            .map(|i| format!("“{}”", p.s(&format!("{letters}{i}_lab"))))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Some(match replaces(fig, key)? {
        Card::GelLanes => gel::lanes_summary(p),
        Card::Forces => quoted("f"),
        Card::Vectors => quoted("v"),
        Card::Motion => match p.s("motion") {
            "Velocity" => "velocity".into(),
            "Acceleration" => "acceleration".into(),
            "Both" => "velocity and acceleration".into(),
            _ => "none".into(),
        },
        Card::Strips => spectra::strip_cards(p).iter().map(|c| c.name().to_string()).collect::<Vec<_>>().join(", "),
        Card::Particles => (0..4).filter(|&i| atoms::kind_used(p, i)).map(|i| atoms::kind_summary(p, i)).collect::<Vec<_>>().join(", "),
        _ => return None,
    })
}

/// The population-growth / predator-prey readout lines for figure `fig` (empty for others).
pub fn readout(fig: &str, p: &Params) -> Vec<String> {
    match fig {
        "population-growth" => popdyn::growth_readout(p),
        "predator-prey" => popdyn::pp_readout(p),
        "titration-curve" => super::chemistry::tit_readout(p),
        _ => Vec::new(),
    }
}

/// Whether param `key` applies to the current params of these figures (others: always).
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("free-body-diagram", "body_size" | "body_lab") => p.s("body") != "Dot",
        ("population-growth", "k") => p.s("growth") != "Exponential (J-curve)",
        ("population-growth", "tau") => p.s("growth") == "Overshoot and oscillation",
        ("titration-curve", _) => super::chemistry::tit_relevant(key, p),
        ("predator-prey", "prey_avg" | "pred_avg" | "cycle") => !p.s("source").starts_with("Model rates"),
        ("predator-prey", "alpha" | "beta" | "gamma" | "delta") => p.s("source").starts_with("Model rates"),
        ("particle-diagram", "state2" | "lab_before" | "lab_after") => p.s("boxes") == "Before and after",
        ("lewis-structures", "lone" | "expand" | "resonance" | "resonance_pick") => super::chemistry::lewis_relevant(key, p),
        ("circuit-diagram", "show_answers") => p.s("solve") != "Off",
        ("ph-reading", "places" | "meter") => p.s("instrument") == "Digital meter",
        _ => super::stats::relevant(fig, key, p) && super::computing::relevant(fig, key, p) && super::electric::relevant(fig, key, p) && super::accounting::relevant(fig, key, p) && super::economics::relevant(fig, key, p) && super::earth::relevant(fig, key, p) && super::geography::relevant(fig, key, p) && super::music::relevant(fig, key, p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{layout, registry, Params};

    /// Every card has its anchor param and owns only params its figure declares; the anchors stay
    /// in the generic sections so the card is drawn where the reference puts it.
    #[test]
    fn cards_have_anchors_and_own_real_params() {
        let mut anchors = 0;
        for d in registry() {
            let defs = (d.params)();
            let secs = layout::sections(d.id, &defs, None);
            for pd in &defs {
                if replaces(d.id, pd.key).is_some() || after(d.id, pd.key).is_some() {
                    anchors += 1;
                    assert!(!owns(d.id, pd.key), "{}: anchor {} is owned", d.id, pd.key);
                    assert!(secs.iter().any(|s| s.items.iter().any(|&i| defs[i].key == pd.key)), "{}: anchor {} in no section", d.id, pd.key);
                }
                if owns(d.id, pd.key) {
                    assert!(secs.iter().all(|s| s.items.iter().all(|&i| defs[i].key != pd.key)), "{}: owned {} still in a section", d.id, pd.key);
                }
            }
        }
        assert_eq!(anchors, 12);
    }

    #[test]
    fn section_summaries_follow_the_reference() {
        let get = |id: &str| {
            let d = registry().into_iter().find(|d| d.id == id).unwrap();
            crate::figures::start_params(&d)
        };
        assert_eq!(summary("free-body-diagram", "n", &get("free-body-diagram")).as_deref(), Some("“F_g”, “F_N”"));
        assert_eq!(summary("vector-diagram", "n", &get("vector-diagram")).as_deref(), Some("“A”, “B”"));
        assert_eq!(summary("free-body-diagram", "motion", &get("free-body-diagram")).as_deref(), Some("none"));
        assert_eq!(summary("gel-electrophoresis", "lanes", &get("gel-electrophoresis")).as_deref(), Some("Ladder, Crime scene, Suspect 1, Suspect 2, Suspect 3"));
        assert_eq!(summary("line-spectrum", "strips", &get("line-spectrum")).as_deref(), Some("Hydrogen, Helium, Sodium, Unknown"));
        assert_eq!(summary("particle-diagram", "shuffle", &get("particle-diagram")).as_deref(), Some("4 large light gray − ions, 4 small white + ions"));
        assert_eq!(summary("waves", "n", &Params::default()), None);
    }

    #[test]
    fn setup_specific_controls_hide() {
        let mut p = Params::default();
        p.set_s("body", "Dot");
        assert!(!relevant("free-body-diagram", "body_size", &p));
        p.set_s("body", "Block");
        assert!(relevant("free-body-diagram", "body_size", &p));
        p.set_s("source", "Model rates (α β γ δ)");
        assert!(relevant("predator-prey", "alpha", &p) && !relevant("predator-prey", "cycle", &p));
        p.set_s("source", "Populations");
        assert!(!relevant("predator-prey", "delta", &p) && relevant("predator-prey", "prey_avg", &p));
        p.set_s("growth", "Exponential (J-curve)");
        assert!(!relevant("population-growth", "k", &p) && !relevant("population-growth", "tau", &p));
        p.set_s("boxes", "One box");
        assert!(!relevant("particle-diagram", "state2", &p));
        p.set_s("boxes", "Before and after");
        assert!(relevant("particle-diagram", "state2", &p) && relevant("particle-diagram", "lab_after", &p));
    }
}
