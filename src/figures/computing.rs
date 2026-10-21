//! Computing figures (the Computing tab): logic circuits, graphs and trees so far, with
//! flowcharts, binary tables and memory diagrams to follow. Original designs on the shared
//! pieces (`svg::Doc`, `show_answers`, print rules); the models live in `logic` and `graph`.
use super::{Example, FigureDef, Params};

mod boolean_table;
mod data_structure;
mod er_diagram;
mod flowchart;
mod graph_diagram;
pub(crate) mod graph_draw;
pub(crate) mod logic_circuit;
mod number_base;
mod recursion_tree;
mod sorting;
mod state_machine;
mod trace_table;
mod tree_diagram;

pub fn defs() -> Vec<FigureDef> {
    vec![logic_circuit::def(), graph_diagram::def(), tree_diagram::def(), number_base::def(), data_structure::def(), flowchart::def(), boolean_table::def(), state_machine::def(), trace_table::def(), sorting::def(), recursion_tree::def(), er_diagram::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    logic_circuit::relevant(fig, key, p) && graph_diagram::relevant(fig, key, p) && tree_diagram::relevant(fig, key, p) && number_base::relevant(fig, key, p) && data_structure::relevant(fig, key, p) && flowchart::relevant(fig, key, p) && boolean_table::relevant(fig, key, p) && state_machine::relevant(fig, key, p) && trace_table::relevant(fig, key, p) && sorting::relevant(fig, key, p) && recursion_tree::relevant(fig, key, p) && er_diagram::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    logic_circuit::examples(id).or_else(|| graph_diagram::examples(id)).or_else(|| tree_diagram::examples(id)).or_else(|| number_base::examples(id)).or_else(|| data_structure::examples(id)).or_else(|| flowchart::examples(id)).or_else(|| boolean_table::examples(id)).or_else(|| state_machine::examples(id)).or_else(|| trace_table::examples(id)).or_else(|| sorting::examples(id)).or_else(|| recursion_tree::examples(id)).or_else(|| er_diagram::examples(id))
}
