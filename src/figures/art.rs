//! Art, psychology and sociology figures (the Art and psychology tab): a colour wheel, perspective
//! grids, Maslow's hierarchy, simplified brain regions and an experimental design flow. Original
//! drawings on the shared pieces (`svg::Doc`, `show_answers`, print rules). What the teacher
//! supplies is the question; the colour names, the cube, the level names, the region names or the
//! roles are the answers, so a student copy (Show answers off) is the same size as its key.
use super::{Example, FigureDef, Params};

mod brain;
mod experiment;
mod maslow;
mod perspective;
mod wheel;

pub fn defs() -> Vec<FigureDef> {
    vec![wheel::def(), perspective::def(), maslow::def(), brain::def(), experiment::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    wheel::examples(id)
        .or_else(|| perspective::examples(id))
        .or_else(|| maslow::examples(id))
        .or_else(|| brain::examples(id))
        .or_else(|| experiment::examples(id))
}
