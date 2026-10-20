//! Shared drawing for the statistics figures: a plot frame that maps data to pixels, "nice" tick
//! choices, axes with gridlines and titles, and rotated text. Ink and font match the rest of the
//! Math figures so a statistics figure sits beside a coordinate grid without looking foreign.
use super::svg::{esc, Doc};

pub const INK: &str = "#111827";
pub const GRID: &str = "#dfe3ea";

/// A plot area inside a canvas: margins in px and the data range shown in it.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub w: f64,
    pub h: f64,
    pub l: f64,
    pub r: f64,
    pub t: f64,
    pub b: f64,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

impl Frame {
    pub fn px(&self, x: f64) -> f64 { self.l + (x - self.x0) / (self.x1 - self.x0) * (self.w - self.l - self.r) }
    pub fn py(&self, y: f64) -> f64 { self.h - self.b - (y - self.y0) / (self.y1 - self.y0) * (self.h - self.t - self.b) }
    pub fn left(&self) -> f64 { self.l }
    pub fn right(&self) -> f64 { self.w - self.r }
    pub fn top(&self) -> f64 { self.t }
    pub fn bottom(&self) -> f64 { self.h - self.b }
}

/// A tick position and its label.
pub type Tick = (f64, String);

/// A step of 1, 2 or 5 times a power of ten that puts about `target` intervals across `range`.
pub fn nice_step(range: f64, target: f64) -> f64 {
    let raw = (range / target.max(1.0)).abs();
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let mag = 10f64.powf(raw.log10().floor());
    for f in [1.0, 2.0, 5.0, 10.0] {
        if raw <= f * mag + 1e-12 {
            return f * mag;
        }
    }
    10.0 * mag
}

/// A number as short axis text (no trailing zeros, a true minus sign).
pub fn fmt(v: f64) -> String {
    let s = super::data::fmt_num((v * 1e9).round() / 1e9);
    s.replace('-', "\u{2212}")
}

/// Ticks from `lo` to `hi` (inclusive, multiples of `step`).
pub fn ticks(lo: f64, hi: f64, step: f64) -> Vec<Tick> {
    if !(step.is_finite() && step > 0.0) || !lo.is_finite() || !hi.is_finite() {
        return Vec::new();
    }
    let k0 = (lo / step - 1e-9).ceil() as i64;
    let k1 = (hi / step + 1e-9).floor() as i64;
    (k0..=k1.min(k0 + 400)).map(|k| (k as f64 * step, fmt(k as f64 * step))).collect()
}

/// Text rotated a quarter turn counter-clockwise about (x, y), centred (for y-axis titles).
pub fn vtext(d: &mut Doc, x: f64, y: f64, size: f64, s: &str) {
    d.raw(&format!(
        r##"<text transform="translate({x:.2} {y:.2}) rotate(-90)" font-size="{size}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
        esc(s)
    ));
}

pub fn text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
        esc(s)
    ));
}

/// What `axes` draws besides the two axis lines.
pub struct AxesOpts<'a> {
    pub xticks: &'a [Tick],
    pub yticks: &'a [Tick],
    pub grid: bool,
    pub x_title: &'a str,
    pub y_title: &'a str,
    /// Text size of tick labels; titles are a little larger.
    pub fs: f64,
}

/// Gridlines (behind), the left and bottom axes, tick marks and labels, and the axis titles.
/// Call before drawing the data so the data sits on the grid.
pub fn axes(d: &mut Doc, f: &Frame, o: &AxesOpts) {
    if o.grid {
        for (v, _) in o.yticks {
            let y = f.py(*v);
            d.line(f.left(), y, f.right(), y, GRID, 1.0);
        }
    }
    d.line(f.left(), f.bottom(), f.right(), f.bottom(), INK, 2.0);
    d.line(f.left(), f.top(), f.left(), f.bottom(), INK, 2.0);
    for (v, label) in o.xticks {
        let x = f.px(*v);
        d.line(x, f.bottom(), x, f.bottom() + 6.0, INK, 1.6);
        if !label.is_empty() {
            text(d, x, f.bottom() + 8.0 + o.fs * 1.05, o.fs, "middle", label);
        }
    }
    for (v, label) in o.yticks {
        let y = f.py(*v);
        d.line(f.left() - 6.0, y, f.left(), y, INK, 1.6);
        if !label.is_empty() {
            text(d, f.left() - 10.0, y + o.fs * 0.35, o.fs, "end", label);
        }
    }
    if !o.x_title.is_empty() {
        text(d, (f.left() + f.right()) / 2.0, f.h - 10.0, o.fs * 1.1, "middle", o.x_title);
    }
    if !o.y_title.is_empty() {
        vtext(d, 18.0 + o.fs * 0.2, (f.top() + f.bottom()) / 2.0, o.fs * 1.1, o.y_title);
    }
}

/// A centred line of text for a figure with nothing to draw yet.
pub fn message(msg: &str) -> String {
    let mut d = Doc::new(420.0, 120.0);
    text(&mut d, 210.0, 64.0, 15.0, "middle", msg);
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_steps_are_1_2_5() {
        assert_eq!(nice_step(100.0, 5.0), 20.0);
        assert_eq!(nice_step(7.0, 7.0), 1.0);
        assert_eq!(nice_step(0.9, 9.0), 0.1);
        assert_eq!(nice_step(0.0, 5.0), 1.0);
        assert_eq!(nice_step(f64::NAN, 5.0), 1.0);
    }

    #[test]
    fn ticks_cover_the_range_and_survive_bad_input() {
        let t = ticks(0.0, 10.0, 2.5);
        assert_eq!(t.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(), ["0", "2.5", "5", "7.5", "10"]);
        assert!(ticks(0.0, 1e300, 1e-3).len() <= 402, "tick count is capped");
        assert!(ticks(0.0, f64::INFINITY, 1.0).is_empty());
        assert!(ticks(0.0, 1.0, 0.0).is_empty());
        assert_eq!(ticks(-1.0, 1.0, 1.0)[0].1, "\u{2212}1");
    }

    #[test]
    fn frame_maps_corners() {
        let f = Frame { w: 200.0, h: 100.0, l: 20.0, r: 10.0, t: 10.0, b: 20.0, x0: 0.0, x1: 10.0, y0: 0.0, y1: 5.0 };
        assert_eq!((f.px(0.0), f.px(10.0)), (20.0, 190.0));
        assert_eq!((f.py(0.0), f.py(5.0)), (80.0, 10.0));
    }
}
