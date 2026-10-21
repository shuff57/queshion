//! Economics figures (the Economics tab): supply and demand, elasticity, production possibilities,
//! cost curves, monopoly, the circular flow, aggregate demand and supply, and the Lorenz curve.
//! Original designs on the shared pieces (`chart`, `svg::Doc`, `Fills`, `show_answers`, print
//! rules). A graph's curves and given numbers are the question; equilibria, areas, readouts and
//! classifications are the answers, so a student copy (Show answers off) leaves them out and is
//! the same size as its key.
use super::chart::{self, AxesOpts, Frame};
use super::svg::Doc;
use super::{Example, FigureDef, Params};

mod ad_as;
mod circular_flow;
mod cost_curves;
mod elasticity;
mod lorenz;
mod monopoly;
mod ppf;
mod supply_demand;

pub fn defs() -> Vec<FigureDef> {
    vec![supply_demand::def(), elasticity::def(), ppf::def(), cost_curves::def(), monopoly::def(), circular_flow::def(), ad_as::def(), lorenz::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    supply_demand::relevant(fig, key, p) && ad_as::relevant(fig, key, p) && ppf::relevant(fig, key, p) && circular_flow::relevant(fig, key, p) && lorenz::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    supply_demand::examples(id)
        .or_else(|| elasticity::examples(id))
        .or_else(|| ppf::examples(id))
        .or_else(|| cost_curves::examples(id))
        .or_else(|| monopoly::examples(id))
        .or_else(|| circular_flow::examples(id))
        .or_else(|| ad_as::examples(id))
        .or_else(|| lorenz::examples(id))
}

pub const GW: f64 = 560.0;
pub const GH: f64 = 440.0;

/// A graph canvas: a document with `extra` px below the plot for readouts, and the frame that maps
/// quantities and prices to pixels. The frame is the same whatever the answers show.
pub fn plane(xmax: f64, ymax: f64, extra: f64) -> (Doc, Frame) {
    let f = Frame { w: GW, h: GH, l: 70.0, r: 36.0, t: 24.0, b: 58.0, x0: 0.0, x1: xmax, y0: 0.0, y1: ymax };
    (Doc::new(GW, GH + extra), f)
}

/// Axes with ticks at a nice step (or none when `numbers` is off, as on a textbook sketch).
pub fn draw_axes(d: &mut Doc, f: &Frame, numbers: bool, fs: f64, x_title: &str, y_title: &str) {
    let (xt, yt) = if numbers {
        (chart::ticks(0.0, f.x1, chart::nice_step(f.x1, 6.0)), chart::ticks(0.0, f.y1, chart::nice_step(f.y1, 6.0)))
    } else {
        (vec![], vec![])
    };
    chart::axes(d, f, &AxesOpts { xticks: &xt, yticks: &yt, grid: false, x_title, y_title, fs });
}

/// The part of the line P = k + m Q (Q from 0 to the frame's right edge) that lies inside the
/// frame, as two (Q, P) points; None when none of it does.
pub fn clip_line(f: &Frame, m: f64, k: f64) -> Option<((f64, f64), (f64, f64))> {
    let (mut q0, mut q1) = (f.x0, f.x1);
    if m.abs() > 1e-12 {
        let (a, b) = ((f.y0 - k) / m, (f.y1 - k) / m);
        q0 = q0.max(a.min(b));
        q1 = q1.min(a.max(b));
    } else if k < f.y0 || k > f.y1 {
        return None;
    }
    (q0 < q1 - 1e-9).then(|| ((q0, k + m * q0), (q1, k + m * q1)))
}

/// A line P = k + m Q drawn in the frame; returns where it was cut off at the top or right, for a label.
pub fn draw_line(d: &mut Doc, f: &Frame, m: f64, k: f64, colour: &str, dash: &str, sw: f64) -> Option<(f64, f64)> {
    let ((q0, p0), (q1, p1)) = clip_line(f, m, k)?;
    let (x0, y0, x1, y1) = (f.px(q0), f.py(p0), f.px(q1), f.py(p1));
    let da = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
    d.raw(&format!(r##"<line x1="{x0:.2}" y1="{y0:.2}" x2="{x1:.2}" y2="{y1:.2}" stroke="{colour}" stroke-width="{sw}" stroke-linecap="round"{da}/>"##));
    Some((q1, p1))
}

/// A curve's name just beside a downward line, near its lower end but clear of the axes.
pub fn tag(d: &mut Doc, f: &Frame, m: f64, k: f64, frac: f64, name: &str, fs: f64) {
    if let Some(((q0, _), (q1, _))) = clip_line(f, m, k) {
        let q = q0 + (q1 - q0) * frac;
        chart::text(d, f.px(q) + 10.0, f.py(k + m * q) - 8.0, fs * 1.1, "start", name);
    }
}

/// A dashed guide from a point to both axes.
pub fn guides(d: &mut Doc, f: &Frame, q: f64, p: f64) {
    d.raw(&format!(
        r##"<path d="M{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2}" fill="none" stroke="#6b7280" stroke-width="1.3" stroke-dasharray="5 4"/>"##,
        f.left(), f.py(p), f.px(q), f.py(p), f.px(q), f.bottom()
    ));
}

pub fn dot(d: &mut Doc, f: &Frame, q: f64, p: f64) {
    d.circle(f.px(q), f.py(p), 4.6, "#111827", "#fff", 1.4);
}

/// A number as short text: at most two decimals.
pub fn n2(v: f64) -> String {
    chart::fmt((v * 100.0).round() / 100.0)
}

/// An axis maximum that covers `v` with a nice round number.
pub fn nice_max(v: f64) -> f64 {
    let v = if v.is_finite() && v > 0.0 { v } else { 1.0 };
    let step = chart::nice_step(v, 5.0);
    ((v / step - 1e-9).ceil().max(1.0)) * step
}

#[cfg(test)]
pub mod testing {
    use super::super::{Params, Value};

    pub fn size_of(s: &str) -> String {
        s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string()
    }

    pub fn apply(p: &mut Params, vals: &[(&str, Value)]) {
        for (k, v) in vals {
            match v {
                Value::S(s) => p.set_s(k, s),
                Value::B(b) => p.set_b(k, *b),
                Value::I(i) => p.set_i(k, *i),
                Value::F(x) => p.set_f(k, *x),
            }
        }
    }

    /// No pixel in a rendering of the figure is more than slightly coloured.
    pub fn assert_grey(svg: &str) {
        let png = crate::figures::to_png(svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24), "a Print figure must be grey only");
    }

    pub fn sane(svg: &str) -> bool {
        svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_cut_to_the_frame() {
        let (_, f) = plane(10.0, 10.0, 0.0);
        let ((q0, p0), (q1, p1)) = clip_line(&f, 1.0, 2.0).unwrap();
        assert_eq!((q0, p0, q1, p1), (0.0, 2.0, 8.0, 10.0));
        let ((q0, p0), (q1, p1)) = clip_line(&f, -2.0, 4.0).unwrap();
        assert_eq!((q0, p0, q1, p1), (0.0, 4.0, 2.0, 0.0));
        assert!(clip_line(&f, 0.0, 12.0).is_none() && clip_line(&f, 1.0, 20.0).is_none());
        assert!(clip_line(&f, 0.0, 5.0).is_some());
    }

    #[test]
    fn axis_maxima_are_round() {
        assert_eq!(nice_max(83.0), 100.0);
        assert_eq!(nice_max(100.0), 100.0);
        assert_eq!(nice_max(0.0), 1.0);
        assert_eq!(nice_max(f64::NAN), 1.0);
        assert_eq!(n2(1.23456), "1.23");
    }
}
