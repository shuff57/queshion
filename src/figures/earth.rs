//! Earth science figures (the Earth science tab): the rock and water cycles, the layers of the
//! Earth, topographic contours, the Hertzsprung-Russell diagram, plate boundaries, strata and
//! moon phases and seasons. Original geometric drawings on the shared pieces (`chart`,
//! `svg::Doc`, `Fills`, `show_answers`, print rules). What is given is the question; names,
//! classifications, contours and orders are the answers, so a student copy (Show answers off)
//! leaves them out and is the same size as its key.
use super::{Example, FigureDef, Params};

mod cycles;
mod hr;
mod layers;
mod plates;
mod sky;
mod strata;
mod topo;

pub fn defs() -> Vec<FigureDef> {
    vec![cycles::def(), layers::def(), plates::def(), strata::def(), sky::def(), topo::def(), hr::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    cycles::relevant(fig, key, p) && sky::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    cycles::examples(id)
        .or_else(|| layers::examples(id))
        .or_else(|| plates::examples(id))
        .or_else(|| strata::examples(id))
        .or_else(|| sky::examples(id))
        .or_else(|| topo::examples(id))
        .or_else(|| hr::examples(id))
}

/// A point on a circle: the angle counts anticlockwise from east, in degrees.
pub fn polar(cx: f64, cy: f64, r: f64, deg: f64) -> (f64, f64) {
    let t = deg.to_radians();
    (cx + r * t.cos(), cy - r * t.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polar_points_run_anticlockwise_from_east() {
        let (x, y) = polar(10.0, 10.0, 5.0, 0.0);
        assert!((x - 15.0).abs() < 1e-9 && (y - 10.0).abs() < 1e-9);
        let (x, y) = polar(10.0, 10.0, 5.0, 90.0);
        assert!((x - 10.0).abs() < 1e-9 && (y - 5.0).abs() < 1e-9, "north is up the page");
    }
}
