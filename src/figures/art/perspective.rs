//! Perspective grids: a horizon, the vanishing points and guide lines, in one-point or two-point
//! perspective. The guides are the question; the box drawn on them is the answer, so the student
//! copy (Show answers off) is the same grid with nothing drawn on it.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const KINDS: [&str; 2] = ["One-point", "Two-point"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "perspective-grid",
        subject: Subject::Art,
        title: "Perspective grid",
        blurb: "A one-point or two-point perspective grid with the horizon line and vanishing points, and a box drawn on it, or the empty grid for students to draw on.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "perspective-grid" => vec![
            Example { name: "One-point", params: vec![] },
            Example { name: "Two-point", params: vec![("kind", s(KINDS[1]))] },
            Example { name: "High horizon (looking down)", params: vec![("horizon", Value::F(30.0))] },
            Example { name: "A box above the horizon", params: vec![("down", Value::F(-30.0)), ("across", Value::F(25.0))] },
            Example { name: "Two-point, a tall box", params: vec![("kind", s(KINDS[1])), ("size", Value::F(40.0)), ("down", Value::F(-5.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Kind of perspective", &KINDS, KINDS[0]),
        ParamDef::float("horizon", "Horizon height (% from the top)", 15.0, 85.0, 1.0, 45.0),
        ParamDef::int("rays", "Guide lines from each vanishing point", 4, 24, 12),
        ParamDef::float("size", "Box size (% of the height)", 8.0, 50.0, 1.0, 26.0),
        ParamDef::float("across", "Box position across (% from the middle)", -40.0, 40.0, 1.0, -20.0),
        ParamDef::float("down", "Box position down (% from the horizon, below +, above -)", -40.0, 40.0, 1.0, 14.0),
        ParamDef::float("depth", "Box depth (how far it recedes, %)", 10.0, 70.0, 1.0, 40.0),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the grid and leaves the box off, for students to draw"),
    ]
}

/// Where the line through (a, b) meets the line through (c, d).
pub fn meet(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> Option<(f64, f64)> {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((c.0 - a.0) * s.1 - (c.1 - a.1) * s.0) / den;
    Some((a.0 + t * r.0, a.1 + t * r.1))
}

fn toward(p: (f64, f64), vp: (f64, f64), t: f64) -> (f64, f64) {
    (p.0 + (vp.0 - p.0) * t, p.1 + (vp.1 - p.1) * t)
}

fn render(p: &Params) -> String {
    let two = p.s("kind") == KINDS[1];
    let hz = p.f("horizon");
    let hz = if hz.is_finite() { hz.clamp(15.0, 85.0) } else { 45.0 };
    let rays = p.i("rays").clamp(4, 24) as usize;
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (860.0, 620.0);
    let mut d = Doc::new(w, top + h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (x0, y0, fw, fh) = (30.0, top + 20.0, w - 60.0, h - 40.0);
    let hy = y0 + fh * hz / 100.0;
    let guide = if print { "#b0b0b0" } else { "#9db4d6" };
    d.rect(x0, y0, fw, fh, "#fff", INK, 2.0);
    let vps: Vec<(f64, f64)> = if two { vec![(x0 + fw * 0.06, hy), (x0 + fw * 0.94, hy)] } else { vec![(x0 + fw / 2.0, hy)] };
    for vp in &vps {
        for k in 0..rays {
            // spread the lines round the whole point, clipped to the frame
            let a = (k as f64 + 0.5) / rays as f64 * std::f64::consts::TAU;
            let far = (fw + fh) * 2.0;
            let (ex, ey) = (vp.0 + far * a.cos(), vp.1 + far * a.sin());
            // clip to the frame by scaling back
            let mut t = 1.0f64;
            if ex > x0 + fw { t = t.min((x0 + fw - vp.0) / (ex - vp.0)); }
            if ex < x0 { t = t.min((x0 - vp.0) / (ex - vp.0)); }
            if ey > y0 + fh { t = t.min((y0 + fh - vp.1) / (ey - vp.1)); }
            if ey < y0 { t = t.min((y0 - vp.1) / (ey - vp.1)); }
            d.line(vp.0, vp.1, vp.0 + (ex - vp.0) * t, vp.1 + (ey - vp.1) * t, guide, 1.0);
        }
    }
    d.line(x0, hy, x0 + fw, hy, INK, 2.2);
    chart::text(&mut d, x0 + 8.0, hy - 8.0, fs * 0.9, "start", "Horizon line");
    for vp in &vps {
        d.circle(vp.0, vp.1, 6.0, INK, INK, 1.0);
        chart::text(&mut d, vp.0, hy + 24.0, fs * 0.9, "middle", "Vanishing point");
    }
    if answers {
        let (fill_t, fill_l, fill_r) = if print { ("#f4f4f4", "#d6d6d6", "#b4b4b4") } else { ("#f6d8a8", "#e3b974", "#c99a4e") };
        let num = |k: &str, lo: f64, hi: f64, dflt: f64| { let v = p.f(k); if v.is_finite() { v.clamp(lo, hi) } else { dflt } };
        let size = fh * num("size", 8.0, 50.0, 26.0) / 100.0;
        let across = fw * num("across", -40.0, 40.0, -20.0) / 100.0;
        let down = fh * num("down", -40.0, 40.0, 14.0) / 100.0;
        let depth = num("depth", 10.0, 70.0, 40.0) / 100.0;
        if !two {
            let vp = vps[0];
            let (fx, fy) = (vp.0 + across - size / 2.0, hy + down - size / 2.0);
            let front = [(fx, fy), (fx + size, fy), (fx + size, fy + size), (fx, fy + size)];
            let back: Vec<(f64, f64)> = front.iter().map(|q| toward(*q, vp, depth)).collect();
            // only the faces that point towards the viewer: the side facing the vanishing point's side
            // of the box, and the top or bottom facing the horizon
            if fy > hy {
                d.poly(&[front[0], front[1], back[1], back[0]], fill_t, INK, 2.0);
            } else if fy + size < hy {
                d.poly(&[front[3], front[2], back[2], back[3]], fill_t, INK, 2.0);
            }
            if fx + size < vp.0 {
                d.poly(&[front[1], front[2], back[2], back[1]], fill_r, INK, 2.0);
            } else if fx > vp.0 {
                d.poly(&[front[0], front[3], back[3], back[0]], fill_r, INK, 2.0);
            }
            d.poly(&front, fill_l, INK, 2.2);
        } else {
            let (l, r) = (vps[0], vps[1]);
            let cx = x0 + fw * 0.5 + across * 0.6;
            let (yt, yb) = (hy + down - size / 2.0, hy + down + size / 2.0);
            let (t_f, t_b) = ((cx, yt), (cx, yb));
            let k = depth;
            let (lt, lb, rt, rb) = (toward(t_f, l, k), toward(t_b, l, k), toward(t_f, r, k), toward(t_b, r, k));
            if yt > hy {
                if let Some(bt) = meet(lt, r, rt, l) {
                    d.poly(&[t_f, lt, bt, rt], fill_t, INK, 2.0);
                }
            } else if yb < hy {
                if let Some(bb) = meet(lb, r, rb, l) {
                    d.poly(&[t_b, lb, bb, rb], fill_t, INK, 2.0);
                }
            }
            d.poly(&[t_f, lt, lb, t_b], fill_l, INK, 2.2);
            d.poly(&[t_f, rt, rb, t_b], fill_r, INK, 2.2);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn lines_meet_where_expected() {
        let m = meet((0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)).unwrap();
        assert!((m.0 - 1.0).abs() < 1e-9 && (m.1 - 1.0).abs() < 1e-9);
        assert!(meet((0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)).is_none());
    }

    #[test]
    fn key_draws_the_box_student_leaves_the_grid() {
        for kind in KINDS {
            let mut p = defaults();
            p.set_s("kind", kind);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert_eq!(student.matches("<polygon").count(), 0, "{kind}");
            assert!(key.matches("<polygon").count() >= 3, "{kind}");
            assert!(student.contains(">Horizon line<") && student.contains(">Vanishing point<"));
            assert_eq!(size_of(&student), size_of(&key));
        }
        let mut p = defaults();
        p.set_s("kind", KINDS[1]);
        assert_eq!(render(&p).matches(">Vanishing point<").count(), 2);
    }

    #[test]
    fn only_visible_faces_are_drawn() {
        let mut p = defaults();
        // one-point box straddling the vanishing point and the horizon shows only its front
        p.set_f("across", 0.0);
        p.set_f("down", 0.0);
        assert_eq!(render(&p).matches("<polygon").count(), 1);
        // below the horizon and to the left: top, side and front
        p.set_f("across", -30.0);
        p.set_f("down", 25.0);
        assert_eq!(render(&p).matches("<polygon").count(), 3);
    }

    #[test]
    fn print_style_is_grey() {
        for kind in KINDS {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("kind", kind);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("perspective-grid").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300] {
            for kind in KINDS {
                let mut p = defaults();
                p.set_f("horizon", v);
                p.set_s("kind", kind);
                assert!(sane(&render(&p)), "{v}");
            }
        }
        for k in ["size", "across", "down", "depth"] {
            for v in [f64::NAN, f64::INFINITY, -1e300, 1e300] {
                for kind in KINDS {
                    let mut p = defaults();
                    p.set_f(k, v);
                    p.set_s("kind", kind);
                    assert!(sane(&render(&p)), "{k} {v}");
                }
            }
        }
        for v in [-5, 0, 999, i64::MAX] {
            let mut p = defaults();
            p.set_i("rays", v);
            assert!(sane(&render(&p)), "{v}");
        }
    }
}
