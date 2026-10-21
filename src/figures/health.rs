//! Health, PE and technology figures (the Health, PE and technology tab): a food plate, heart-rate
//! zones, court and field layouts, a body map, and engineering drawings (block diagram, free-body
//! diagram, gear train, orthographic views). Original drawings on the shared pieces (`svg::Doc`,
//! `Fills`, `show_answers`, print rules). What the teacher supplies is the question; the names,
//! the heart-rate ranges, the forces or the ratios are the answers, so a student copy (Show
//! answers off) is the same size as its key.
use super::{Example, FigureDef, Params};

mod blockdiagram;
mod bodymap;
mod court;
mod foodplate;
mod freebody;
mod gears;
mod heartzones;
mod ortho;

pub fn defs() -> Vec<FigureDef> {
    vec![foodplate::def(), heartzones::def(), court::def(), bodymap::def(), blockdiagram::def(), freebody::def(), gears::def(), ortho::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    court::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    foodplate::examples(id)
        .or_else(|| heartzones::examples(id))
        .or_else(|| court::examples(id))
        .or_else(|| bodymap::examples(id))
        .or_else(|| blockdiagram::examples(id))
        .or_else(|| freebody::examples(id))
        .or_else(|| gears::examples(id))
        .or_else(|| ortho::examples(id))
}
