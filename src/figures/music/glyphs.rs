//! Music symbols drawn as plain shapes: noteheads, stems, flags, rests, accidentals, ledger lines
//! and clefs. `sp` is the gap between two staff lines; (x, y) is the centre of the symbol.
use crate::figures::svg::Doc;

pub const INK: &str = "#111827";

/// The y of `step` (diatonic steps above C0) on a staff whose bottom line is `bottom_step` and sits at `yb`.
pub fn step_y(step: i32, bottom_step: i32, yb: f64, sp: f64) -> f64 {
    yb - (step - bottom_step) as f64 * sp / 2.0
}

pub fn staff_lines(d: &mut Doc, x0: f64, x1: f64, yb: f64, sp: f64) {
    for k in 0..5 {
        d.line(x0, yb - k as f64 * sp, x1, yb - k as f64 * sp, INK, 1.2);
    }
}

/// Short lines through the notehead for notes above or below the staff: one at every even offset
/// (counted in half spaces from the bottom line) from the staff out to the note.
pub fn ledger_lines(d: &mut Doc, x: f64, step: i32, bottom_step: i32, yb: f64, sp: f64) {
    for k in ledger_offsets(step - bottom_step) {
        let y = yb - k as f64 * sp / 2.0;
        d.line(x - sp * 0.95, y, x + sp * 0.95, y, INK, 1.2);
    }
}

pub fn ledger_offsets(off: i32) -> Vec<i32> {
    if off <= -2 {
        (off..=-2).filter(|k| k % 2 == 0).collect()
    } else if off >= 10 {
        (10..=off).filter(|k| k % 2 == 0).collect()
    } else {
        Vec::new()
    }
}

/// A notehead: hollow for whole and half notes, filled otherwise.
pub fn notehead(d: &mut Doc, x: f64, y: f64, kind: u8, sp: f64) {
    let (rx, ry) = (if kind == 0 { sp * 0.75 } else { sp * 0.62 }, sp * 0.46);
    let fill = if kind <= 1 { "#fff" } else { INK };
    d.raw(&format!(r##"<ellipse cx="{x:.2}" cy="{y:.2}" rx="{rx:.2}" ry="{ry:.2}" transform="rotate(-20 {x:.2} {y:.2})" fill="{fill}" stroke="{INK}" stroke-width="{}"/>"##, if kind <= 1 { 1.8 } else { 1.0 }));
    if kind == 0 {
        // the slanted hole of a whole note
        d.raw(&format!(r##"<ellipse cx="{x:.2}" cy="{y:.2}" rx="{:.2}" ry="{:.2}" transform="rotate(-50 {x:.2} {y:.2})" fill="{INK}"/>"##, sp * 0.2, sp * 0.42));
    }
}

/// The stem and flags of a note (nothing for a whole note); `up` puts the stem on the right.
pub fn stem(d: &mut Doc, x: f64, y: f64, kind: u8, up: bool, sp: f64) {
    if kind == 0 {
        return;
    }
    let len = sp * 3.4;
    let sx = if up { x + sp * 0.6 } else { x - sp * 0.6 };
    let ey = if up { y - len } else { y + len };
    d.line(sx, y, sx, ey, INK, 1.4);
    let flags = if kind >= 4 { 2 } else if kind == 3 { 1 } else { 0 };
    for f in 0..flags {
        let off = f as f64 * sp * 0.75;
        let (y0, dir) = if up { (ey + off, 1.0) } else { (ey - off, -1.0) };
        d.path(&format!("M{sx:.2},{y0:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}", sx, y0 + dir * sp * 1.1, sx + sp * 1.3, y0 + dir * sp * 1.0, sx + sp * 0.95, y0 + dir * sp * 2.5), "none", INK, 1.8);
    }
}

/// The dot after a dotted note, moved into the space above when the note sits on a line.
pub fn dot(d: &mut Doc, x: f64, y: f64, on_line: bool, sp: f64) {
    d.circle(x + sp * 1.0, if on_line { y - sp * 0.5 } else { y }, 1.9, INK, INK, 0.5);
}

/// Accidental signs: 1 sharp, -1 flat, 0 natural. (x, y) is where the sign's centre goes.
pub fn accidental(d: &mut Doc, x: f64, y: f64, acc: i8, sp: f64) {
    match acc {
        1 => {
            for dx in [-0.22, 0.22] {
                d.line(x + dx * sp, y - sp * 1.0, x + dx * sp, y + sp * 1.0, INK, 1.2);
            }
            for dy in [-0.28, 0.28] {
                d.line(x - sp * 0.5, y + (dy + 0.14) * sp, x + sp * 0.5, y + (dy - 0.14) * sp, INK, 2.4);
            }
        }
        -1 => {
            d.line(x - sp * 0.25, y - sp * 1.5, x - sp * 0.25, y + sp * 0.5, INK, 1.4);
            d.path(&format!("M{:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}", x - sp * 0.25, y - sp * 0.1, x + sp * 0.2, y - sp * 0.6, x + sp * 0.65, y - sp * 0.1, x + sp * 0.5, y + sp * 0.25, x + sp * 0.3, y + sp * 0.5, x, y + sp * 0.55, x - sp * 0.25, y + sp * 0.5), "none", INK, 1.4);
        }
        _ => {
            d.line(x - sp * 0.3, y - sp * 1.2, x - sp * 0.3, y + sp * 0.5, INK, 1.2);
            d.line(x + sp * 0.3, y - sp * 0.5, x + sp * 0.3, y + sp * 1.2, INK, 1.2);
            d.line(x - sp * 0.3, y - sp * 0.15, x + sp * 0.3, y - sp * 0.5, INK, 2.2);
            d.line(x - sp * 0.3, y + sp * 0.5, x + sp * 0.3, y + sp * 0.15, INK, 2.2);
        }
    }
}

/// A rest of the given kind on a staff whose bottom line is at `yb`.
pub fn rest(d: &mut Doc, x: f64, kind: u8, yb: f64, sp: f64) {
    let mid = yb - 2.0 * sp;
    match kind {
        0 => {
            d.rect(x - sp * 0.6, yb - 3.0 * sp, sp * 1.2, sp * 0.5, INK, INK, 0.5);
        }
        1 => {
            d.rect(x - sp * 0.6, mid - sp * 0.5, sp * 1.2, sp * 0.5, INK, INK, 0.5);
        }
        2 => {
            d.polyline(&[(x - sp * 0.25, mid - sp * 1.5), (x + sp * 0.4, mid - sp * 0.6), (x - sp * 0.35, mid + sp * 0.2), (x + sp * 0.35, mid + sp * 1.0)], INK, 2.4);
            d.path(&format!("M{:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}", x + sp * 0.35, mid + sp * 1.0, x - sp * 0.4, mid + sp * 0.9, x - sp * 0.5, mid + sp * 1.5, x, mid + sp * 1.7), "none", INK, 1.8);
        }
        n => {
            let flags = if n >= 4 { 2 } else { 1 };
            d.line(x + sp * 0.35, mid - sp * 1.1, x - sp * 0.25, mid + sp * 1.2, INK, 1.6);
            for f in 0..flags {
                let y = mid - sp * 0.9 + f as f64 * sp * 0.8;
                d.circle(x - sp * 0.15 + f as f64 * -0.0, y + 0.0, 2.2, INK, INK, 0.5);
                d.path(&format!("M{:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}", x - sp * 0.15, y, x + sp * 0.2, y + sp * 0.25, x + sp * 0.4, y - sp * 0.1, x + sp * 0.55, y - sp * 0.25), "none", INK, 1.5);
            }
        }
    }
}

/// A clef whose staff has its bottom line at `yb` (x is the left edge); the width used is about 2.8 spaces.
pub fn clef(d: &mut Doc, kind: &str, x: f64, yb: f64, sp: f64) {
    let u = sp;
    let t = |px: f64, py: f64, oy: f64| format!("{:.2},{:.2}", x + px * u, oy + py * u);
    match kind {
        "Bass" => {
            let oy = yb - 3.0 * sp; // the F line
            d.circle(x + 0.55 * u, oy, 0.42 * u, INK, INK, 0.5);
            d.path(&format!("M{} C{} {} {} C{} {} {}", t(0.55, -0.4, oy), t(1.9, -0.9, oy), t(2.7, 0.2, oy), t(2.4, 1.3, oy), t(2.0, 2.4, oy), t(0.9, 3.3, oy), t(-0.2, 3.9, oy)), "none", INK, 2.6);
            d.circle(x + 3.2 * u, oy - 0.5 * u, 0.28 * u, INK, INK, 0.5);
            d.circle(x + 3.2 * u, oy + 0.55 * u, 0.28 * u, INK, INK, 0.5);
        }
        "Alto" => {
            let oy = yb - 2.0 * sp; // the middle line
            d.rect(x, yb - 4.0 * sp, 0.35 * u, 4.0 * sp, INK, INK, 0.5);
            d.line(x + 0.75 * u, yb - 4.0 * sp, x + 0.75 * u, yb, INK, 1.3);
            d.path(&format!("M{} C{} {} {} C{} {} {}", t(2.5, -2.0, oy), t(1.7, -2.3, oy), t(1.2, -1.3, oy), t(1.4, -0.3, oy), t(1.7, -0.9, oy), t(2.6, -0.8, oy), t(2.6, -0.2, oy)), "none", INK, 2.2);
            d.path(&format!("M{} C{} {} {} C{} {} {}", t(2.5, 2.0, oy), t(1.7, 2.3, oy), t(1.2, 1.3, oy), t(1.4, 0.3, oy), t(1.7, 0.9, oy), t(2.6, 0.8, oy), t(2.6, 0.2, oy)), "none", INK, 2.2);
        }
        _ => {
            let oy = yb - sp; // the G line
            d.path(&format!("M{} C{} {} {} C{} {} {} C{} {} {} C{} {} {} C{} {} {} C{} {} {}", t(1.0, 2.9, oy), t(1.9, 3.0, oy), t(1.8, 2.0, oy), t(1.2, 1.9, oy), t(0.0, 1.8, oy), t(-0.2, 0.0, oy), t(0.9, -0.1, oy), t(1.7, -0.2, oy), t(2.1, 0.8, oy), t(1.4, 1.3, oy), t(0.9, 1.6, oy), t(0.6, 1.0, oy), t(0.9, 0.5, oy), t(1.1, -1.4, oy), t(0.5, -2.6, oy), t(1.5, -4.2, oy), t(1.7, -4.6, oy), t(1.6, -3.2, oy), t(1.2, -1.0, oy)), "none", INK, 2.0);
            d.line(x + 1.2 * u, oy - 1.0 * u, x + 1.0 * u, oy + 2.9 * u, INK, 2.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_lines_start_one_space_off_the_staff() {
        assert!(ledger_offsets(0).is_empty() && ledger_offsets(8).is_empty() && ledger_offsets(9).is_empty());
        assert!(ledger_offsets(-1).is_empty());
        assert_eq!(ledger_offsets(-2), vec![-2]);
        assert_eq!(ledger_offsets(-3), vec![-2]);
        assert_eq!(ledger_offsets(-4), vec![-4, -2]);
        assert_eq!(ledger_offsets(10), vec![10]);
        assert_eq!(ledger_offsets(11), vec![10]);
        assert_eq!(ledger_offsets(12), vec![10, 12]);
    }

    #[test]
    fn steps_climb_the_page() {
        assert_eq!(step_y(30, 30, 100.0, 10.0), 100.0);
        assert_eq!(step_y(32, 30, 100.0, 10.0), 90.0);
        assert!(step_y(29, 30, 100.0, 10.0) > 100.0);
    }
}
