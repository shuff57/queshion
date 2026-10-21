//! Computing figures (the Computing tab): logic circuits so far, with flowcharts, trees and
//! graphs, binary tables and memory diagrams to follow. Original designs on the shared pieces
//! (`svg::Doc`, `Fills`-style print rules, `show_answers`); the circuit model lives in `logic`.
use super::{Example, FigureDef, Params};

pub(crate) mod logic_circuit;

pub fn defs() -> Vec<FigureDef> {
    vec![logic_circuit::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    logic_circuit::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    logic_circuit::examples(id)
}
