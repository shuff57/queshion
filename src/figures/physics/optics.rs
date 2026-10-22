//! Motion map and ray diagram (the reference physics site's two figures with a one-line setup).
use super::common::*;
use crate::figures::{FigureDef, ParamDef, Params, Subject};
use std::fmt::Write;

/// The reference's ink colour for these two figures.
const NAVY: &str = "#111827";

// ---------------------------------------------------------------- motion map

fn map_params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("up", "Speeding up for (s)", 0.0, 12.0, 0.5, 4.0),
        ParamDef::float("accel", "Acceleration (m/s\u{b2})", 0.1, 6.0, 0.1, 1.0),
        ParamDef::float("cruise", "Constant velocity for (s)", 0.0, 12.0, 0.5, 4.0),
        ParamDef::float("down", "Slowing down for (s)", 0.0, 12.0, 0.5, 4.0),
        ParamDef::float("rest", "At rest for (s)", 0.0, 12.0, 0.5, 3.0),
        ParamDef::choice("dir", "Direction", &["Forward", "Back"], "Forward"),
        ParamDef::float("dt", "Time between dots (s)", 0.25, 4.0, 0.25, 1.0),
        ParamDef::choice("arrows", "Arrows", &["Velocity", "None"], "Velocity"),
        ParamDef::bool("numbers", "Numbers on the number line", true),
        ParamDef::text("xlabel", "Number line label", "x (m)"),
        ParamDef::text("title", "Chart title", ""),
    ]
}

/// Position and velocity at time `t` of the four-phase motion.
fn map_state(p: &Params, t: f64) -> (f64, f64) {
    let (a, tu, tc, td) = (p.f("accel"), p.f("up"), p.f("cruise"), p.f("down"));
    let v1 = a * tu;
    let x1 = 0.5 * a * tu * tu;
    let x2 = x1 + v1 * tc;
    let dec = if td > 0.0 { v1 / td } else { 0.0 };
    let x3 = x2 + v1 * td - 0.5 * dec * td * td;
    if t <= tu { (0.5 * a * t * t, a * t) }
    else if t <= tu + tc { (x1 + v1 * (t - tu), v1) }
    else if t <= tu + tc + td { let d = t - tu - tc; (x2 + v1 * d - 0.5 * dec * d * d, v1 - dec * d) }
    else { (x3, 0.0) }
}

fn map_render(p: &Params) -> String {
    let total = p.f("up") + p.f("cruise") + p.f("down") + p.f("rest");
    let dt = p.f("dt").max(0.05);
    let n = ((total / dt) + 1e-6).floor() as usize + 1;
    let sign = if p.s("dir") == "Back" { -1.0 } else { 1.0 };
    let dots: Vec<(f64, f64)> = (0..n.min(400)).map(|i| map_state(p, i as f64 * dt)).collect();
    let xmax = dots.iter().map(|d| d.0).fold(1.0, f64::max);
    // ticks every 1, numbered every 5, the line one past the last numbered tick
    let last = (xmax / 5.0).ceil() * 5.0;
    let len = last + 1.0;
    let r = 3.93;
    let (axis_len, pad) = (782.0, 16.0);
    let k = axis_len / len;
    let xo = |m: f64| r + m * k;
    let xlab = p.s("xlabel").trim().to_string();
    let lab_w = if p.b("numbers") && !xlab.is_empty() { 8.0 + 4.69 * xlab.chars().count() as f64 } else { 0.0 };
    let w = 2.0 * pad + axis_len + r + 4.0 + lab_w + if lab_w > 0.0 { 6.0 } else { 0.0 };
    let title = p.s("title").trim().to_string();
    let top = if title.is_empty() { 0.0 } else { 32.0 };
    let h = 2.0 * pad + 60.04 + top;
    let mut g = G::new(w, h, false);
    if !title.is_empty() { g.title_text(w / 2.0, pad + 14.0, 19.0, &title); }
    g.raw(&format!(r#"<g transform="translate({pad} {})" font-family="Arial, Helvetica, sans-serif">"#, pad + top));
    let ay = 34.64;
    g.line(r, ay, r + axis_len, ay, NAVY, 2.0);
    let ticks = last as i64;
    for m in 0..=ticks {
        let major = m % 5 == 0;
        let (y0, y1, sw) = if major { (ay - 6.0, ay + 6.0, 1.5) } else { (ay - 3.5, ay + 3.5, 1.0) };
        g.line(xo(m as f64), y0, xo(m as f64), y1, NAVY, sw);
    }
    g.poly(&[(r + axis_len + 2.0, ay), (r + axis_len - 10.0, ay - 5.0), (r + axis_len - 10.0, ay + 5.0)], NAVY, NAVY, 0.0);
    if p.b("numbers") {
        for m in (0..=ticks).step_by(5) {
            g.text(xo(m as f64), 55.84, 14.0, "middle", &format!("{m}"), true);
        }
        if !xlab.is_empty() { g.text(r + axis_len + 8.0, 39.54, 14.0, "start", &xlab, true); }
    }
    let at = |m: f64| xo(sign * m + if sign < 0.0 { last } else { 0.0 });
    for &(x, v) in &dots {
        g.circle(at(x), 16.71, r, NAVY, NAVY, 0.0);
        if p.s("arrows") == "Velocity" && v > 1e-9 {
            let l = (v * 2.0 * r).max(2.0 * r);
            let (x0, tip) = (at(x), at(x) + sign * l);
            let head = 9.0_f64.min(l);
            let base = tip - sign * head;
            let shaft = (base - x0).abs();
            let (dash, gap) = if shaft < 1.0 { (shaft.max(1.0), 0.0) } else {
                let n = (shaft / 12.0).round().max(1.0);
                let d = shaft / (n + 0.625 * (n - 1.0));
                (d, 0.625 * d)
            };
            g.raw(&format!(r#"<line x1="{x0:.2}" y1="3.78" x2="{base:.2}" y2="3.78" stroke="{NAVY}" stroke-width="3" stroke-dasharray="{dash:.2} {gap:.2}" stroke-linecap="butt"/>"#));
            g.poly(&[(tip, 3.78), (base, 7.56), (base, 0.0)], NAVY, NAVY, 0.0);
        }
    }
    g.raw("</g>");
    g.finish()
}

// ---------------------------------------------------------------- ray diagram

const KINDS: [&str; 4] = ["Converging lens", "Diverging lens", "Concave mirror", "Convex mirror"];

fn ray_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Type", &KINDS, "Converging lens"),
        ParamDef::float("f", "Focal length (cm)", 2.0, 20.0, 0.5, 10.0),
        ParamDef::float("dist", "Distance (cm)", 2.0, 31.0, 0.5, 30.0),
        ParamDef::float("height", "Height (cm)", 0.5, 7.0, 0.5, 4.0),
        ParamDef::int("rays", "Principal rays", 1, 3, 3),
        ParamDef::bool("image", "Draw the image", true),
        ParamDef::bool("show_labels", "Label the points", true),
        ParamDef::text("lab_f", "Focal point label", "F"),
        ParamDef::text("lab_f2", "Other focal point label", "F\u{2032}"),
        ParamDef::text("lab_2f", "Twice the focal length label", "2F"),
        ParamDef::text("lab_2f2", "Other twice label", "2F\u{2032}"),
        ParamDef::text("lab_o", "Object label", "O"),
        ParamDef::text("lab_i", "Image label", "I"),
        ParamDef::text("title", "Chart title", ""),
    ]
}

/// Italic letters, upright digits and primes (the reference's "2F\u{2032}").
fn rich(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        let t = crate::figures::svg::esc(&c.to_string());
        if c.is_alphabetic() { let _ = write!(o, r#"<tspan font-style="italic">{t}</tspan>"#); } else { let _ = write!(o, "<tspan>{t}</tspan>"); }
    }
    o
}

/// One principal ray `a -> b -> c` (arrowheads mid-segment, clipped to the diagram) with the dashed
/// back-extension of its last leg to a virtual image when there is one.
fn ray(g: &mut G, pts: [(f64, f64); 3], virt: Option<(f64, f64)>, right: f64) {
    let mut s = String::from(r#"<g clip-path="url(#rclip)">"#);
    for w2 in pts.windows(2) {
        let ((x1, y1), (x2, y2)) = (w2[0], w2[1]);
        let _ = write!(s, r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{NAVY}" stroke-width="1.7"/>"#);
    }
    if let Some((ix, iy)) = virt {
        let (hx, hy) = pts[1];
        let _ = write!(s, r#"<line x1="{hx:.2}" y1="{hy:.2}" x2="{ix:.2}" y2="{iy:.2}" stroke="{NAVY}" stroke-width="1.2" stroke-dasharray="6 4"/>"#);
    }
    for w2 in pts.windows(2) {
        let ((x1, y1), (x2, y2)) = (w2[0], w2[1]);
        if (x2 - x1).hypot(y2 - y1) < 30.0 { continue; }
        let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        if mx < 0.0 || mx > right { continue; }
        let ang = (y2 - y1).atan2(x2 - x1).to_degrees();
        let _ = write!(s, r#"<polygon points="5,0 -4,-4.2 -4,4.2" fill="{NAVY}" transform="translate({mx:.2} {my:.2}) rotate({ang:.2})"/>"#);
    }
    s.push_str("</g>");
    g.raw(&s);
}

fn ray_render(p: &Params) -> String {
    let kind = p.s("kind").to_string();
    let mirror = kind.contains("mirror");
    let conv = kind.starts_with("Converging") || kind.starts_with("Concave");
    let k = 14.4; // px per cm
    let (xl, ya) = (466.0, 133.2);
    let half = 115.2;
    let fcm = p.f("f").max(0.5);
    let f = if conv { fcm } else { -fcm };
    let fp = f * k;
    let dcm = p.f("dist").max(0.5).min(31.0);
    let ho = p.f("height").clamp(0.2, 8.0);
    let xo = xl - dcm * k;
    let yt = ya - ho * k;
    let right = 788.0;
    let bottom = ya + half + 18.0;
    let title = p.s("title").trim().to_string();
    let top = if title.is_empty() { 0.0 } else { 32.0 };
    let (w, h) = (right + 32.0, bottom + 32.0 + top);
    let mut g = G::new(w, h, false);
    if !title.is_empty() { g.title_text(w / 2.0, 30.0, 19.0, &title); }
    g.raw(&format!(r#"<g transform="translate(16 {})" stroke-linecap="round"><clipPath id="rclip"><rect x="-2" y="-2" width="{}" height="{}"/></clipPath>"#, 16.0 + top, right + 4.0, bottom + 4.0));
    g.line(0.0, ya, right, ya, NAVY, 1.5);
    // the lens or mirror
    if !mirror {
        let bulge = if conv { 27.65 } else { -27.65 };
        let d = if conv {
            format!("M{xl},{:.1} Q{:.2},{ya} {xl},{:.1} Q{:.2},{ya} {xl},{:.1} Z", ya - half, xl + bulge, ya + half, xl - bulge, ya - half)
        } else {
            let e = 13.0;
            format!("M{:.1},{:.1} Q{:.1},{ya} {:.1},{:.1} L{:.1},{:.1} Q{:.1},{ya} {:.1},{:.1} Z", xl - e, ya - half, xl - e + 10.0, xl - e, ya + half, xl + e, ya + half, xl + e - 10.0, xl + e, ya - half)
        };
        g.raw(&format!(r##"<path d="{d}" fill="#eef1f5" stroke="{NAVY}" stroke-width="2" stroke-linejoin="round"/>"##));
    } else {
        let sag = 20.0;
        let (ex, vx) = if conv { (xl - sag, xl) } else { (xl + sag, xl) };
        g.raw(&format!(r##"<path d="M{ex:.1},{:.1} Q{:.1},{ya} {ex:.1},{:.1}" fill="none" stroke="{NAVY}" stroke-width="2.4"/>"##, ya - half, vx + (vx - ex), ya + half));
    }
    // the image first (the virtual rays aim at it)
    let di = if (dcm - fcm).abs() < 1e-6 && conv { f64::INFINITY } else { f * dcm / (dcm - f) };
    let img = if di.is_finite() {
        let ix = if mirror { xl - di * k } else { xl + di * k };
        let hi = -ho * di / dcm; // cm, positive = upright
        Some((ix, ya - hi * k, di < 0.0))
    } else { None };
    let nrays = p.i("rays").clamp(1, 3);
    let focal_near = xl - fp;
    let sgn = fp.signum();
    for r in 0..nrays {
        let t = (xl - xo) / (focal_near - xo);
        let (hit_y, dir) = match (mirror, r) {
            (false, 0) => (yt, (sgn * fp, sgn * (ya - yt))),
            (false, 1) => (ya, (xl - xo, ya - yt)),
            (false, _) => (yt + (ya - yt) * t, (1.0, 0.0)),
            (true, 0) => (yt, (sgn * -fp, sgn * (ya - yt))),
            (true, 1) => (ya, (-(xl - xo), ya - yt)),
            (true, _) => (yt + (ya - yt) * t, (-1.0, 0.0)),
        };
        let xe = if mirror { 0.0 } else { right };
        let e = if dir.0.abs() < 1e-9 { (xl, hit_y) } else { (xe, hit_y + dir.1 / dir.0 * (xe - xl)) };
        let virt = match img { Some((ix, iy, true)) if p.b("image") => Some((ix, iy)), _ => None };
        ray(&mut g, [(xo, yt), (xl, hit_y), e], virt, right);
    }
    let dots: Vec<f64> = if mirror { vec![xl - 2.0 * fp, xl - fp] } else { vec![xl - 2.0 * fp, xl - fp, xl + fp, xl + 2.0 * fp] };
    for x in &dots { g.circle(*x, ya, 3.2, NAVY, NAVY, 0.0); }
    {
        let hl = 13.0_f64.min(0.4 * ho * k);
        g.line(xo, ya, xo, yt + hl * 0.85, NAVY, 3.0);
        g.poly(&[(xo, yt), (xo - hl * 0.45, yt + hl), (xo + hl * 0.45, yt + hl)], NAVY, NAVY, 0.0);
    }
    let mut img_x = None;
    if let (true, Some((ix, iy, virtual_))) = (p.b("image"), img) {
        if (ix - xo).abs() > 1.0 && ix > -20.0 && ix < right + 20.0 {
            let hl = (0.4 * (ya - iy).abs()).min(13.0);
            let dir = if iy < ya { 1.0 } else { -1.0 };
            let dash = if virtual_ { r#" stroke-dasharray="5 3""# } else { "" };
            let mut s = String::new();
            let _ = write!(s, r#"<line x1="{ix:.2}" y1="{ya:.2}" x2="{ix:.2}" y2="{:.2}" stroke="{NAVY}" stroke-width="3"{dash}/>"#, iy + dir * hl * 0.85);
            let _ = write!(s, r#"<polygon points="{ix:.2},{iy:.2} {:.2},{:.2} {:.2},{:.2}" fill="{NAVY}"/>"#, ix - hl * 0.45, iy + dir * hl, ix + hl * 0.45, iy + dir * hl);
            g.raw(&s);
            img_x = Some((ix, iy));
        }
    }
    // virtual rays behind the mirror or lens are drawn above; now the labels
    if p.b("show_labels") {
        let mut lab = |x: f64, y: f64, t: &str| {
            let t = t.trim();
            if t.is_empty() { return; }
            let s = format!(r##"<text x="{x:.2}" y="{y:.2}" text-anchor="middle" font-family="'Times New Roman', Times, serif" font-size="20" fill="{NAVY}" stroke="#fff" stroke-width="4" paint-order="stroke" stroke-linejoin="round">{}</text>"##, rich(t));
            g.raw(&s);
        };
        if mirror {
            lab(xl - fp, ya + 26.0, p.s("lab_f"));
            lab(xl - 2.0 * fp, ya + 26.0, p.s("lab_2f"));
        } else {
            lab(xl - 2.0 * fp, ya + 26.0, p.s("lab_2f"));
            lab(xl - fp, ya + 26.0, p.s("lab_f"));
            lab(xl + fp, ya - 12.0, p.s("lab_f2"));
            lab(xl + 2.0 * fp, ya - 12.0, p.s("lab_2f2"));
        }
        lab(xo - 20.0, (yt + ya) / 2.0 - 2.0 + 0.0, p.s("lab_o"));
        if let Some((ix, iy)) = img_x { lab(ix + 20.0, if iy < ya { ya - 8.0 } else { ya - 8.0 }, p.s("lab_i")); }
    }
    g.raw("</g>");
    g.finish()
}

/// Adds the L/M/S label-size param (the reference toolbar toggle).
fn ls(mut v: Vec<ParamDef>) -> Vec<ParamDef> {
    v.push(crate::figures::svg::label_size_param());
    v
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "motion-map", subject: Subject::Physics, title: "Motion Map",
            blurb: "Dots along a number line, one per tick of the clock, each moving dot with its velocity arrow.",
            params: || ls(map_params()), render: |p| crate::figures::svg::with_label_size(map_render(p), p) },
        FigureDef { id: "ray-diagram", subject: Subject::Physics, title: "Ray Diagram",
            blurb: "Principal rays and the image for a lens or a mirror, with the focal points marked.",
            params: || ls(ray_params()), render: |p| crate::figures::svg::with_label_size(ray_render(p), p) },
    ]
}
