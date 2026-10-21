//! Computing figures (the Computing tab): logic circuits, graphs and trees so far, with
//! flowcharts, binary tables and memory diagrams to follow. Original designs on the shared
//! pieces (`svg::Doc`, `show_answers`, print rules); the models live in `logic` and `graph`.
use super::{Example, FigureDef, Params};

mod graph_diagram;
pub(crate) mod graph_draw;
pub(crate) mod logic_circuit;
mod tree_diagram;

pub fn defs() -> Vec<FigureDef> {
    vec![logic_circuit::def(), graph_diagram::def(), tree_diagram::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    logic_circuit::relevant(fig, key, p) && graph_diagram::relevant(fig, key, p) && tree_diagram::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    logic_circuit::examples(id).or_else(|| graph_diagram::examples(id)).or_else(|| tree_diagram::examples(id))
}
