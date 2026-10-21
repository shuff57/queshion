//! Shared pieces of the node-and-wire canvas editors (the logic circuit now; flowcharts, trees
//! and graphs to follow): a pan / zoom view over "world" pixels, a dotted grid, segment hit
//! testing and the toolbar button. A canvas owns no figure state; it reads and writes the
//! figure's text spec each frame, as the composite-shape canvas does.
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, RichText, Stroke, pos2};

use crate::theme;

pub type World = (f64, f64);

pub const MIN_ZOOM: f64 = 0.25;
pub const MAX_ZOOM: f64 = 4.0;

/// Screen points per world pixel (`s`) and the world point at the middle of the canvas (`c`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub s: f64,
    pub c: World,
}

impl View {
    pub fn to_screen(&self, rect: Rect, w: World) -> Pos2 {
        let c = rect.center();
        pos2(c.x + ((w.0 - self.c.0) * self.s) as f32, c.y + ((w.1 - self.c.1) * self.s) as f32)
    }

    pub fn to_world(&self, rect: Rect, p: Pos2) -> World {
        let c = rect.center();
        (self.c.0 + (p.x - c.x) as f64 / self.s, self.c.1 + (p.y - c.y) as f64 / self.s)
    }

    pub fn zoom_about(&mut self, rect: Rect, p: Pos2, f: f64) {
        let w = self.to_world(rect, p);
        self.s = (self.s * f).clamp(MIN_ZOOM, MAX_ZOOM);
        let c = rect.center();
        self.c = (w.0 - (p.x - c.x) as f64 / self.s, w.1 - (p.y - c.y) as f64 / self.s);
    }

    pub fn pan_by(&mut self, d: egui::Vec2) {
        self.c.0 -= d.x as f64 / self.s;
        self.c.1 -= d.y as f64 / self.s;
    }

    /// The view that shows `bounds` (x0, y0, x1, y1) with a margin, as large as 1:1.
    pub fn fit(rect: Rect, bounds: Option<(f64, f64, f64, f64)>) -> View {
        let Some((x0, y0, x1, y1)) = bounds else { return View { s: 1.0, c: (0.0, 0.0) } };
        let (mw, mh) = ((rect.width() as f64 - 60.0).max(60.0), (rect.height() as f64 - 60.0).max(60.0));
        let s = (mw / (x1 - x0).max(1.0)).min(mh / (y1 - y0).max(1.0)).clamp(MIN_ZOOM, 1.0);
        View { s, c: ((x0 + x1) / 2.0, (y0 + y1) / 2.0) }
    }

    /// Whether the world box is inside the canvas with room to spare.
    pub fn shows(&self, rect: Rect, bounds: (f64, f64, f64, f64)) -> bool {
        let r = Rect::from_two_pos(self.to_screen(rect, (bounds.0, bounds.1)), self.to_screen(rect, (bounds.2, bounds.3)));
        rect.shrink(8.0).contains_rect(r)
    }
}

/// A dotted grid every `step` world pixels (coarser when zoomed out so it stays light).
pub fn draw_grid(painter: &Painter, rect: Rect, view: &View, step: f64) {
    let mut g = step;
    while g * view.s < 14.0 {
        g *= 2.0;
    }
    let (a, b) = (view.to_world(rect, rect.left_top()), view.to_world(rect, rect.right_bottom()));
    let (i0, i1) = ((a.0 / g).floor() as i64, (b.0 / g).ceil() as i64);
    let (j0, j1) = ((a.1 / g).floor() as i64, (b.1 / g).ceil() as i64);
    if (i1 - i0) * (j1 - j0) > 40_000 {
        return;
    }
    let dot = Color32::from_rgb(0xc8, 0xcd, 0xd6);
    for i in i0..=i1 {
        for j in j0..=j1 {
            painter.circle_filled(view.to_screen(rect, (i as f64 * g, j as f64 * g)), 1.1, dot);
        }
    }
}

/// Distance from `p` to the segment `a`-`b`.
pub fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq();
    let t = if l2 <= f32::EPSILON { 0.0 } else { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) };
    p.distance(a + ab * t)
}

/// Points along a quadratic Bezier curve (for the curved gate outlines).
pub fn quad(a: Pos2, c: Pos2, b: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            pos2(u * u * a.x + 2.0 * u * t * c.x + t * t * b.x, u * u * a.y + 2.0 * u * t * c.y + t * t * b.y)
        })
        .collect()
}

/// A toolbar button in the canvas editors' style.
pub fn seg_btn(label: &str, on: bool) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_string()).size(12.0).color(if on { theme::accent_col() } else { theme::text_col() }))
        .fill(if on { theme::accent_faded() } else { Color32::TRANSPARENT })
        .stroke(Stroke::new(1.0, if on { theme::accent_col() } else { theme::border() }))
        .corner_radius(CornerRadius::same(8))
}


#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    #[test]
    fn world_and_screen_round_trip_and_zoom_keeps_the_point_still() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 300.0));
        let mut v = View { s: 1.0, c: (50.0, 60.0) };
        let p = pos2(123.0, 77.0);
        let w = v.to_world(rect, p);
        assert!((v.to_screen(rect, w) - p).length() < 1e-3);
        v.zoom_about(rect, p, 2.0);
        assert!((v.to_screen(rect, w) - p).length() < 1e-3, "the point under the pointer stays put");
        v.zoom_about(rect, p, 1e9);
        assert_eq!(v.s, MAX_ZOOM);
        v.zoom_about(rect, p, 1e-9);
        assert_eq!(v.s, MIN_ZOOM);
    }

    #[test]
    fn fit_shows_the_box_and_never_zooms_in_past_one_to_one() {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 400.0));
        let big = (0.0, 0.0, 1800.0, 800.0);
        let v = View::fit(rect, Some(big));
        assert!(v.shows(rect, big));
        assert_eq!(View::fit(rect, Some((0.0, 0.0, 10.0, 10.0))).s, 1.0);
        assert_eq!(View::fit(rect, None).s, 1.0);
    }

    #[test]
    fn segment_distance() {
        let (a, b) = (pos2(0.0, 0.0), pos2(10.0, 0.0));
        assert_eq!(seg_dist(pos2(5.0, 3.0), a, b), 3.0);
        assert_eq!(seg_dist(pos2(-4.0, 3.0), a, b), 5.0);
        assert_eq!(seg_dist(pos2(1.0, 1.0), a, a), 2f32.sqrt());
    }
}
