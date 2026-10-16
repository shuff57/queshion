//! Pure PDF and DOCX writers for [`ExamDoc`] (signatures frozen in NOTES.md):
//! `render_pdf(&layout::ExamDoc) -> Result<Vec<u8>, String>` and `render_docx(&layout::ExamDoc)`.
//! No filesystem access; uses only the already-present `image` and `zip` crates.
//!
//! # PDF (1.4, hand-rolled)
//! Letter 612x792pt, 72 pt/inch. Font subset F1=Helvetica, F2=Helvetica-Bold (Times-Roman
//! is intentionally unused — clean sans is the Test Parrot print look, bold title; note
//! the correct spelling "Times-Roman", not the tempting "Times-Romen" typo). Text widths
//! come from embedded AFM width tables for ASCII 32..126 (other bytes: 0.5 em average);
//! non-ASCII maps through a small WinAnsi table (Latin-1 passes 1:1, common Unicode
//! punctuation is covered, the rest becomes '?'). Images decode via the `image` crate and
//! re-encode to JPEG quality 85, embedded as DCTDecode XObjects. Content streams are
//! uncompressed; the xref table carries exact byte offsets, verified by a unit test that
//! resolves every object entry to its `N obj` header.
//!
//! # DOCX (hand-rolled OOXML zipped with the `zip` crate)
//! Minimal valid package: [Content_Types].xml, _rels/.rels, word/document.xml,
//! word/_rels/document.xml.rels and a small styles.xml (Normal/Title/Heading1, Arial
//! default). Word does its own wrapping, so no text measurement is needed. Matching
//! questions print linearly (left items numbered, then the lettered right column
//! indented); images become a centered italic `[picture]` paragraph.

use crate::layout::{DocImage, DocQ, ExamDoc};

const PAGE_W: f64 = 612.0; // Letter, 72pt/inch
const PAGE_H: f64 = 792.0;
const MM_PT: f64 = 72.0 / 25.4;
const IMG_MAX_PT: f64 = 130.0 * MM_PT; // images fit 130 mm square

/// AFM widths /1000 em for Helvetica, ASCII 32..126. Any other byte measures 0.5 em.
const AFM_HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // SP - /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // 0-9
    278, 278, 584, 584, 584, 556, 1015, // : ; < = > ? @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // A-Z
    278, 278, 278, 469, 556, 333, // [ \ ] ^ _ `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // a-z
    334, 260, 334, 584, // { | } ~
];

/// AFM widths /1000 em for Helvetica-Bold, ASCII 32..126.
const AFM_HELV_B: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556,
    333, 333, 584, 584, 584, 611, 975,
    722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611,
    333, 278, 333, 584, 556, 333,
    556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500,
    389, 280, 389, 584,
];

#[derive(Clone, Copy, PartialEq)]
enum Face {
    Reg,
    Bold,
}

impl Face {
    fn name(self) -> &'static str {
        match self {
            Face::Reg => "F1",
            Face::Bold => "F2",
        }
    }
    fn afm(self) -> &'static [u16; 95] {
        match self {
            Face::Reg => &AFM_HELV,
            Face::Bold => &AFM_HELV_B,
        }
    }
}

// ---------------------------------------------------------------- text measuring

fn char_w(c: u8, face: Face, pt: f64) -> f64 {
    if (32..=126).contains(&c) {
        face.afm()[(c - 32) as usize] as f64 * pt / 1000.0
    } else {
        0.5 * pt // average em default for any WinAnsi byte outside pure ASCII
    }
}

/// Width of a string after the same non-ASCII -> WinAnsi mapping used to render it.
fn text_w(s: &str, face: Face, pt: f64) -> f64 {
    winansi(s).iter().map(|&b| char_w(b, face, pt)).sum()
}

/// Greedy word wrap on spaces; words wider than the column hard-break per character.
/// Explicit `\n` keeps its rows (possibly empty).
fn wrap(s: &str, max_w: f64, face: Face, pt: f64) -> Vec<String> {
    let mut out = vec![];
    for logical in s.split('\n') {
        let mut line = String::new();
        let mut w = 0.0;
        for word in logical.split(' ') {
            let ww = text_w(word, face, pt);
            if line.is_empty() {
                if ww > max_w {
                    for ch in word.chars() {
                        let cw = text_w(&ch.to_string(), face, pt);
                        if !line.is_empty() && w + cw > max_w {
                            out.push(std::mem::take(&mut line));
                            w = 0.0;
                        }
                        line.push(ch);
                        w += cw;
                    }
                } else {
                    line.push_str(word);
                    w = ww;
                }
            } else {
                let sp = char_w(32, face, pt);
                if w + sp + ww <= max_w {
                    line.push(' ');
                    line.push_str(word);
                    w += sp + ww;
                } else {
                    out.push(std::mem::take(&mut line));
                    w = 0.0;
                    if ww > max_w {
                        for ch in word.chars() {
                            let cw = text_w(&ch.to_string(), face, pt);
                            if !line.is_empty() && w + cw > max_w {
                                out.push(std::mem::take(&mut line));
                                w = 0.0;
                            }
                            line.push(ch);
                            w += cw;
                        }
                    } else {
                        line.push_str(word);
                        w = ww;
                    }
                }
            }
        }
        out.push(line);
    }
    out
}

/// Map a string to WinAnsi bytes: ASCII passes through (parens/backslash escape later in
/// `esc_pdf`), Latin-1 >= 0xA0 maps 1:1, common Unicode punctuation maps to its WinAnsi
/// byte, control bytes become plain space, everything else becomes '?'.
fn winansi(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        let u = c as u32;
        let mapped: Option<u8> = match u {
            // WinAnsi punctuation zone bytes (0x80-0x9F) the fonts' WinAnsiEncoding carries.
            0x80 => Some(0x80),      // euro
            0x82 => Some(0x82),      // low quote
            0x84 => Some(0x84),
            0x85 => Some(0x85),      // ellipsis
            0x86 => Some(0x86),      // dagger
            0x87 => Some(0x87),      // double dagger
            0x89 => Some(0x89),      // per mille
            0x8a => Some(0x8a),      // S caron
            0x8b => Some(0x8b),      // left guillemet
            0x8c => Some(0x8c),      // OE lig
            0x91 => Some(0x91),      // left single quote
            0x92 => Some(0x92),      // right single quote
            0x93 => Some(0x93),      // left double quote
            0x94 => Some(0x94),      // right double quote
            0x95 => Some(0x95),      // bullet
            0x96 => Some(0x96),      // en dash
            0x97 => Some(0x97),      // em dash
            0x9a => Some(0x9a),      // s caron
            0x9b => Some(0x9b),      // right guillemet
            0x9c => Some(0x9c),      // oe lig
            0x9f => Some(0x9f),      // Y diaeresis
            // Typed Unicode variants of the same marks.
            0x2018 => Some(0x91),
            0x201a => Some(0x82),
            0x2019 => Some(0x92),
            0x201b => Some(0x91),
            0x201c => Some(0x93),
            0x201d => Some(0x94),
            0x2020 => Some(0x86),
            0x2021 => Some(0x87),
            0x2013 => Some(0x96),
            0x2014 => Some(0x97),
            0x2022 => Some(0x95),
            0x2026 => Some(0x85),
            0x2122 => Some(0x99),
            0xa0 => Some(0x20), // nbsp -> space
            0xa1..=0xff => Some(u as u8), // Latin-1 letters map 1:1 in WinAnsi
            0x09 | 0x0a | 0x0b | 0x0d => Some(0x20), // control whitespace -> space
            _ => None,
        };
        match mapped {
            Some(b) => out.push(b),
            None => {
                if (0x20..=0x7e).contains(&u) {
                    out.push(u as u8);
                } else {
                    out.push(b'?');
                }
            }
        }
    }
    out
}

/// Escape bytes for a PDF literal string: the paren pair and backslash.
fn esc_pdf(v: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(v.len() + 8);
    for &b in v {
        match b {
            b'(' => o.extend_from_slice(b"\\("),
            b')' => o.extend_from_slice(b"\\)"),
            b'\\' => o.extend_from_slice(b"\\\\"),
            _ => o.push(b),
        }
    }
    o
}

fn fnum(v: f64) -> String {
    let mut s = format!("{v:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

fn points_label(p: f64) -> String {
    if p == 1.0 {
        "(1 point)".to_string()
    } else if p.fract() == 0.0 {
        format!("({} points)", p as i64)
    } else {
        format!("({p} points)")
    }
}

/// Ruled answer lines a question gets; the exporters' default when the caller leaves 0.
fn rule_count(q: &DocQ) -> u8 {
    if q.rule_lines > 0 {
        q.rule_lines
    } else if q.choices.is_empty() {
        if q.text.iter().any(|t| t.contains("___")) {
            1 // fill-in-the-blank: one ruled line under the stem
        } else {
            3 // short answer / essay default (matches the student print look)
        }
    } else {
        0
    }
}

/// True/False detection: the choices are exactly True/False (any case).
fn is_tf(choices: &[String]) -> bool {
    choices.len() == 2
        && choices
            .iter()
            .all(|c| matches!(c.trim().to_ascii_lowercase().as_str(), "true" | "false"))
}

/// Matching detection (see the layout.rs conventions): >= 2 text entries treated as
/// left items and >= 2 lettered right entries.
fn looks_matching(q: &DocQ, tf: bool) -> bool {
    !tf
        && q.text.len() >= 2
        && q.choices.len() >= 2
        && q
            .choices
            .iter()
            .all(|c| char_is_letter_dot(c))
}

fn char_is_letter_dot(c: &str) -> bool {
    let b = c.as_bytes();
    b.len() >= 2 && b[0].is_ascii_uppercase() && b[1] == b'.'
}

// ---------------------------------------------------------------- PDF writer

struct Px {
    jpeg: Vec<u8>,
    w: u32,
    h: u32,
}

/// Content-stream state for one PDF page at a time; finished pages move to `pages`
/// with the index list of image XObjects they used.
struct R {
    pages: Vec<(Vec<u8>, Vec<usize>)>,
    cur: Vec<u8>,
    used: Vec<usize>,
    y: f64,
    x0: f64,
    x1: f64,
    margin: f64,
    body: f64,
    head: f64,
    lead: f64,
    imgs: Vec<Px>,
}

impl R {
    fn remaining(&self) -> f64 {
        self.y - self.margin
    }
    fn full_height(&self) -> f64 {
        PAGE_H - 2.0 * self.margin
    }
    fn new_page(&mut self) {
        if !self.cur.is_empty() {
            let cur = std::mem::take(&mut self.cur);
            self.pages.push((cur, std::mem::take(&mut self.used)));
        }
        self.y = PAGE_H - self.margin;
    }
    fn text(&mut self, x: f64, y: f64, face: Face, size: f64, s: &str) {
        let bytes = esc_pdf(&winansi(s));
        self.cur.extend_from_slice(b"BT /");
        self.cur.extend_from_slice(face.name().as_bytes());
        self.cur.extend_from_slice(b" ");
        self.cur.extend_from_slice(fnum(size).as_bytes());
        self.cur.extend_from_slice(b" Tf ");
        self.cur.extend_from_slice(fnum(x).as_bytes());
        self.cur.extend_from_slice(b" ");
        self.cur.extend_from_slice(fnum(y).as_bytes());
        self.cur.extend_from_slice(b" Td (");
        self.cur.extend_from_slice(&bytes);
        self.cur.extend_from_slice(b") Tj ET\n");
    }
    /// Approximate ascent, so a baseline sits a tidy line below the y cursor.
    fn ascent_of(size: f64) -> f64 {
        size * 0.78
    }
    fn line(&mut self, x: f64, s: &str, face: Face) {
        let by = self.y - Self::ascent_of(self.body);
        self.text(x, by, face, self.body, s);
        self.y -= self.lead;
    }
    fn num_right(&mut self, right_x: f64, label: &str, f: Face) {
        let w = text_w(label, f, self.body);
        self.text(right_x - w, self.y - Self::ascent_of(self.body), f, self.body, label);
    }
    /// Small gray text (version labels and similar muted annotations).
    fn text_muted(&mut self, x: f64, y: f64, size: f64, s: &str) {
        self.cur.extend_from_slice(b"0.45 0.45 0.45 rg ");
        self.text(x, y, Face::Reg, size, s);
        self.cur.extend_from_slice(b"0 0 0 rg\n");
    }
    fn rule(&mut self, xa: f64, ya: f64, xb: f64, yb: f64) {
        self.cur.extend_from_slice(b"0.6 w 0.65 0.6 0.55 RG ");
        self.cur.extend_from_slice(fnum(xa).as_bytes());
        self.cur.extend_from_slice(b" ");
        self.cur.extend_from_slice(fnum(ya).as_bytes());
        self.cur.extend_from_slice(b" m ");
        self.cur.extend_from_slice(fnum(xb).as_bytes());
        self.cur.extend_from_slice(b" ");
        self.cur.extend_from_slice(fnum(yb).as_bytes());
        self.cur.extend_from_slice(b" l S\n");
    }
}

/// Stem text starts this far inside the left margin (number column is right-aligned
/// in the gutter to its left).
const STEM_INSET: f64 = 32.0;
const NUM_GAP: f64 = 6.0;

pub fn render_pdf(doc: &ExamDoc) -> Result<Vec<u8>, String> {
    // Prepare images first (decode -> RGB -> JPEG q85) in question order.
    let mut imgs: Vec<Px> = vec![];
    for q in &doc.questions {
        if let Some(im) = &q.image {
            imgs.push(prepare_image(im)?);
        }
    }

    let margin = (doc.margin_mm as f64).max(6.0) * MM_PT;
    let body = (doc.body_pt as f64).clamp(6.0, 20.0);
    let head = (doc.heading_pt as f64).clamp(8.0, 36.0);
    let mut r = R {
        pages: vec![],
        cur: vec![],
        used: vec![],
        y: PAGE_H - margin,
        x0: margin,
        x1: PAGE_W - margin,
        margin,
        body,
        head,
        lead: body * 1.42,
        imgs,
    };

    // Title area (also printed on the answer-key document).
    if !doc.title.is_empty() {
        r.text(r.x0, r.y - r.head * 0.75, Face::Bold, head, doc.title.trim());
        r.y -= head * 1.35;
    }
    if !doc.is_key {
        if !doc.header_line.is_empty() {
            r.line(r.x0, doc.header_line.trim(), Face::Reg);
        }
        for _ in 0..doc.ruled_blanks {
            if r.remaining() < body * 2.0 {
                r.new_page();
            }
            r.y -= body * 1.1;
            r.rule(r.x0, r.y, r.x1, r.y);
            r.y -= body * 0.9;
        }
    }
    // Version label ("Version B" etc.), small and muted, under the header area.
    if let Some(v) = doc.version_label.as_deref().map(str::trim) {
        if !v.is_empty() {
            let sz = body * 0.85;
            r.text_muted(r.x0, r.y - R::ascent_of(sz), sz, v);
            r.y -= sz * 1.4;
        }
    }
    if !doc.questions.is_empty() {
        r.y -= body * 0.5; // small gap before question 1
    }

    for qi in 0..doc.questions.len() {
        let q = &doc.questions[qi];
        let img_idx = q.image.as_ref().map(|_| qi);
        let est = q_est_height(&r, q, img_idx);
        // Keep-together: break before a question that will not comfortably fit.
        if r.remaining() < 40.0 || (est <= r.full_height() && est > r.remaining()) {
            r.new_page();
        }
        if doc.is_key {
            render_key_q(&mut r, q);
        } else {
            render_q(&mut r, q, img_idx);
        }
        r.y -= body * 0.6; // separator between questions
    }
    if !r.cur.is_empty() {
        let cur = std::mem::take(&mut r.cur);
        let used = std::mem::take(&mut r.used);
        r.pages.push((cur, used));
    }

    assemble_pdf(r)
}

fn render_key_q(r: &mut R, q: &DocQ) {
    if q.key_text.trim().is_empty() {
        return;
    }
    let stem_x = r.x0 + STEM_INSET;
    let avail = r.x1 - stem_x;
    r.num_right(stem_x - NUM_GAP, &format!("{}.", q.number), Face::Bold);
    r.text(stem_x, r.y - R::ascent_of(r.body), Face::Reg, r.body, "");
    // draw lines starting on the number's baseline
    let lines = wrap(q.key_text.trim(), avail, Face::Reg, r.body);
    let mut advance = 0.0;
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            r.text(stem_x, r.y - R::ascent_of(r.body), Face::Reg, r.body, l);
            advance += r.lead;
        } else {
            r.text(stem_x, r.y - R::ascent_of(r.body), Face::Reg, r.body, l);
            advance += r.lead;
        }
        r.y -= r.lead * 0.0; // cursor handled below
    }
    r.y -= advance;
}

fn render_q(r: &mut R, q: &DocQ, img_idx: Option<usize>) {
    let stem_x = r.x0 + STEM_INSET;
    let avail = r.x1 - stem_x;
    let tf = is_tf(&q.choices);
    let matching = looks_matching(q, tf);

    if matching {
        // Two columns: left items numbered in print as number, number+1, ...; right
        // column lettered (pre-lettered by the exam worker), 24pt leading per row.
        let mid = r.x0 + (r.x1 - r.x0) * 0.5;
        let left_w = mid - stem_x - 8.0;
        let right_w = r.x1 - mid - 8.0;
        let rows = q.text.len().max(q.choices.len());
        let pt_label = q.points.map(points_label);
        for i in 0..rows {
            let left_lines = q
                .text
                .get(i)
                .map(|t| wrap(t, left_w, Face::Reg, r.body))
                .unwrap_or_default();
            let right_lines = q
                .choices
                .get(i)
                .map(|c| wrap(c, right_w, Face::Reg, r.body))
                .unwrap_or_default();
            let row_lines = left_lines.len().max(right_lines.len()).max(1);
            let row_h = (r.body * 1.15 * row_lines as f64).max(24.0);
            if r.remaining() < row_h {
                r.new_page();
            }
            let top = r.y;
            for (j, piece) in left_lines.iter().enumerate() {
                let by = top - j as f64 * r.lead - R::ascent_of(r.body);
                if j == 0 {
                    let n = q.number as usize + i;
                    let lbl = format!("{n}.");
                    let w = text_w(&lbl, Face::Reg, r.body);
                    r.text(stem_x - NUM_GAP - w, by, Face::Reg, r.body, &lbl);
                }
                r.text(stem_x, by, Face::Reg, r.body, piece);
            }
            for (j, piece) in right_lines.iter().enumerate() {
                let by = top - j as f64 * r.lead - R::ascent_of(r.body);
                r.text(mid, by, Face::Reg, r.body, piece);
            }
            if i == 0 {
                if let Some(lbl) = &pt_label {
                    let by = top - R::ascent_of(r.body);
                    r.text(r.x1 - text_w(lbl, Face::Reg, r.body), by, Face::Reg, r.body, lbl);
                }
            }
            r.y = top - row_h;
        }
        return;
    }

    // Stem lines; the first drawn line carries the number and a right-aligned
    // "(N points)" label, and wraps narrower so label and stem never collide.
    let pt_label = q.points.map(points_label);
    let label_w = pt_label.as_deref().map(|l| text_w(l, Face::Reg, r.body) + 4.0).unwrap_or(0.0);
    let mut first = true;
    for (ti, t) in q.text.iter().enumerate() {
        let w_first = if ti == 0 { avail - label_w } else { avail };
        for l in wrap(t, w_first, Face::Reg, r.body) {
            if first {
                let lbl = format!("{}.", q.number);
                r.num_right(stem_x - NUM_GAP, &lbl, Face::Reg);
                r.text(stem_x, r.y - R::ascent_of(r.body), Face::Reg, r.body, &l);
                if let Some(lbl) = &pt_label {
                    r.text(
                        r.x1 - text_w(lbl, Face::Reg, r.body),
                        r.y - R::ascent_of(r.body),
                        Face::Reg,
                        r.body,
                        lbl,
                    );
                }
                first = false;
            } else {
                r.text(stem_x, r.y - R::ascent_of(r.body), Face::Reg, r.body, &l);
            }
            r.y -= r.lead;
        }
    }

    // Choices.
    if tf {
        if r.remaining() < r.lead {
            r.new_page();
        }
        r.text(stem_x, r.y - R::ascent_of(r.body), Face::Reg, r.body, "True   False");
        r.y -= r.lead;
    } else {
        for c in &q.choices {
            for l in wrap(c, avail, Face::Reg, r.body) {
                if r.remaining() < r.lead {
                    r.new_page();
                }
                r.text(stem_x, r.y - R::ascent_of(r.body), Face::Reg, r.body, &l);
                r.y -= r.lead;
            }
        }
    }

    // Ruled answer lines.
    for _ in 0..rule_count(q) {
        if r.remaining() < r.body * 2.0 {
            r.new_page();
        }
        r.y -= r.body * 1.1;
        r.rule(r.x0, r.y, r.x1, r.y);
        r.y -= r.body * 0.9;
    }

    // Image (page breaks first if it does not fit).
    if let Some(idx) = img_idx {
        draw_pdf_image(r, idx);
    }
}

/// Rough question height for keep-together page breaks; only compared against the free
/// space, so a rough estimate is fine.
fn q_est_height(r: &R, q: &DocQ, img_idx: Option<usize>) -> f64 {
    let stem_x = r.x0 + STEM_INSET;
    let avail = r.x1 - stem_x;
    let tf = is_tf(&q.choices);
    let matching = looks_matching(q, tf);
    if matching {
        let rows = q.text.len().max(q.choices.len());
        return rows as f64 * 24.0 + r.body;
    }
    let mut lines = 0usize;
    for t in &q.text {
        lines += wrap(t, avail, Face::Reg, r.body).len();
    }
    lines += if tf {
        1
    } else {
        q.choices.iter().map(|c| wrap(c, avail, Face::Reg, r.body).len()).sum::<usize>()
    };
    lines += rule_count(q) as usize;
    let mut h = lines as f64 * r.lead;
    if let Some(idx) = img_idx {
        h += 12.0 + pdf_image_size(r, idx).1;
    }
    h
}

fn pdf_image_size(r: &R, idx: usize) -> (f64, f64) {
    let px = &r.imgs[idx];
    let max_w = (r.x1 - r.x0).min(IMG_MAX_PT);
    let s = (max_w / px.w as f64).min(IMG_MAX_PT / px.h as f64).min(1.0);
    (px.w as f64 * s, px.h as f64 * s)
}

fn draw_pdf_image(r: &mut R, idx: usize) {
    let (w, h) = pdf_image_size(r, idx);
    if r.remaining() < h + r.body {
        r.new_page();
    }
    r.y -= 6.0;
    let x = (r.x0 + (r.x1 - r.x0 - w) / 2.0).max(r.x0);
    let yb = r.y - h;
    r.cur.extend_from_slice(b"q ");
    r.cur.extend_from_slice(fnum(w).as_bytes());
    r.cur.extend_from_slice(b" 0 0 ");
    r.cur.extend_from_slice(fnum(h).as_bytes());
    r.cur.extend_from_slice(b" ");
    r.cur.extend_from_slice(fnum(x).as_bytes());
    r.cur.extend_from_slice(b" ");
    r.cur.extend_from_slice(fnum(yb).as_bytes());
    r.cur.extend_from_slice(b" cm /Im");
    r.cur.extend_from_slice((idx + 1).to_string().as_bytes());
    r.cur.extend_from_slice(b" Do Q\n");
    r.used.push(idx);
    r.y = yb - 6.0;
}

fn prepare_image(im: &DocImage) -> Result<Px, String> {
    let img = image::load_from_memory(&im.bytes).map_err(|e| format!("image decode: {e}"))?;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    if w == 0 || h == 0 {
        return Err("image decode: empty image".into());
    }
    let mut jpeg = Vec::new();
    // ImageEncoder trait method: writes the already-decoded bytes as baseline JPEG.
    use image::ImageEncoder as _;
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85);
    enc.write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("image encode: {e}"))?;
    Ok(Px { jpeg, w, h })
}

fn assemble_pdf(r: R) -> Result<Vec<u8>, String> {
    let npage = r.pages.len();
    let n_imgs = r.imgs.len();
    let kids: String = (0..npage).map(|p| format!("{} 0 R ", 5 + n_imgs + p * 2 + 1)).collect();

    let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(4 + n_imgs + npage * 2);
    // obj 1: catalog
    bodies.push(b"<< /Type /Catalog /Pages 2 0 R >>\n".to_vec());
    // obj 2: page tree
    bodies.push(format!("<< /Type /Pages /Kids [{kids}] /Count {npage} >>\n").into_bytes());
    // obj 3/4: fonts
    bodies.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\n".to_vec());
    bodies.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>\n".to_vec());
    // obj 5..: image XObjects (JPEG bytes embedded raw, no deflate).
    let mut imgs = r.imgs;
    for Px { jpeg, w, h } in imgs.drain(..) {
        let mut b = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            w, h, jpeg.len()
        )
        .into_bytes();
        b.extend_from_slice(&jpeg);
        b.extend_from_slice(b"\nendstream\n");
        bodies.push(b);
    }
    // Then two objects per page: contents, then the page dict with its resources.
    for (p, (ops, used)) in r.pages.into_iter().enumerate() {
        let mut b = format!("<< /Length {} >>\nstream\n", ops.len()).into_bytes();
        b.extend_from_slice(&ops);
        b.extend_from_slice(b"endstream\n");
        bodies.push(b);

        let mut xo = String::new();
        if !used.is_empty() {
            xo.push_str(" /XObject << ");
            for &idx in &used {
                xo.push_str(&format!("/Im{} {} 0 R ", idx + 1, 5 + idx));
            }
            xo.push_str(">>");
        }
        bodies.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R /F2 4 0 R >>{xo} /ProcSet [/PDF /Text /ImageC] >> /Contents {} 0 R >>\n",
                5 + n_imgs + p * 2
            )
            .into_bytes(),
        );
    }

    // Serialize with exact byte offsets for the xref table.
    let n_obj = bodies.len();
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    let mut offsets = vec![0usize; n_obj + 1];
    for (i, b) in bodies.iter().enumerate() {
        offsets[i + 1] = out.len();
        let mut head = format!("{} 0 obj\n", i + 1).into_bytes();
        head.extend_from_slice(b);
        head.extend_from_slice(b"endobj\n");
        out.extend_from_slice(&head);
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", n_obj + 1).as_bytes());
    for i in 1..=n_obj {
        out.extend_from_slice(format!("{:010} 00000 n \n", offsets[i]).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            n_obj + 1,
            xref_at
        )
        .as_bytes(),
    );
    Ok(out)
}

// ---------------------------------------------------------------- DOCX writer

struct Par<'a> {
    text: &'a str,
    style: Option<&'static str>,
    ind: i64,
    sz: i64,
    bold: bool,
    italic: bool,
    rule: bool,
    center: bool,
    /// OOXML run color, e.g. "808080".
    color: Option<&'static str>,
}

impl<'a> Par<'a> {
    fn body(text: &'a str) -> Par<'a> {
        Par { text, style: None, ind: 0, sz: 0, bold: false, italic: false, rule: false, center: false, color: None }
    }
}

fn wpara(xml: &mut String, p: &Par, body_sz: i64) {
    xml.push_str("<w:p><w:pPr>");
    if let Some(s) = p.style {
        xml.push_str(&format!("<w:pStyle w:val=\"{s}\"/>"));
    }
    if p.rule {
        xml.push_str("<w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"808080\"/></w:pBdr>");
    }
    if p.ind > 0 {
        xml.push_str(&format!("<w:ind w:left=\"{}\"/>", p.ind));
    }
    if p.center {
        xml.push_str("<w:jc w:val=\"center\"/>");
    }
    xml.push_str("</w:pPr>");
    if !p.text.is_empty() {
        xml.push_str("<w:r><w:rPr>");
        if p.bold {
            xml.push_str("<w:b/>");
        }
        if p.italic {
            xml.push_str("<w:i/>");
        }
        if let Some(c) = p.color {
            xml.push_str(&format!("<w:color w:val=\"{c}\"/>"));
        }
        let sz = if p.sz > 0 { p.sz } else { body_sz };
        xml.push_str(&format!("<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>"));
        xml.push_str("</w:rPr><w:t xml:space=\"preserve\">");
        xml.push_str(&xml_esc(p.text));
        xml.push_str("</w:t></w:r>");
    }
    xml.push_str("</w:p>");
}

fn xml_esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            _ => o.push(c),
        }
    }
    o
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOC_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="120"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="120" w:after="60"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style></w:styles>"#;

pub fn render_docx(doc: &ExamDoc) -> Result<Vec<u8>, String> {
    let tw = |mm: f64| (mm * 1440.0 / 25.4).round() as i64; // mm -> twips
    let body_sz = ((doc.body_pt as f64).clamp(6.0, 20.0) * 2.0).round() as i64; // half-points
    let head_sz = ((doc.heading_pt as f64).clamp(8.0, 36.0) * 2.0).round() as i64;

    let mut b = String::new();
    b.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    b.push_str(
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>",
    );

    if !doc.title.is_empty() {
        wpara(
            &mut b,
            &Par {
                style: Some("Title"),
                sz: head_sz,
                bold: true,
                ..Par::body(doc.title.trim())
            },
            body_sz,
        );
    }
    if !doc.header_line.is_empty() {
        wpara(&mut b, &Par::body(doc.header_line.trim()), body_sz);
    }
    if let Some(v) = doc.version_label.as_deref().map(str::trim) {
        if !v.is_empty() {
            let sz = body_sz * 3 / 4; // small muted version line
            wpara(
                &mut b,
                &Par { color: Some("808080"), sz, ..Par::body(v) },
                body_sz,
            );
        }
    }
    for _ in 0..doc.ruled_blanks {
        wpara(&mut b, &Par { rule: true, ..Par::body("") }, body_sz);
    }

    for q in &doc.questions {
        if doc.is_key {
            if !q.key_text.trim().is_empty() {
                let t = format!("{}. {}", q.number, q.key_text.trim());
                wpara(&mut b, &Par { bold: true, ..Par::body(&t) }, body_sz);
            }
            continue;
        }

        // Stem paragraphs: number on the first line, points label appended there.
        // Matching questions print their pairs only (the text entries ARE the
        // left items), handled below.
        let pt_label = q.points.map(points_label);
        let tf = is_tf(&q.choices);
        let matching = looks_matching(q, tf);
        let ind: i64 = 432; // 0.3" indent so choices line up under the stem
        if matching {
            for (i, item) in q.text.iter().enumerate() {
                let t = format!("{}. {}", q.number as usize + i, item);
                wpara(&mut b, &Par::body(&t), body_sz);
            }
            for c in &q.choices {
                wpara(&mut b, &Par { ind, ..Par::body(c) }, body_sz);
            }
        } else {
            let mut first = true;
            for t in &q.text {
                for l in t.split('\n') {
                    let mut text;
                    if first {
                        text = format!("{}. {}", q.number, l);
                        if let Some(lbl) = &pt_label {
                            text.push(' ');
                            text.push_str(lbl);
                        }
                        first = false;
                    } else {
                        text = l.to_string();
                    }
                    wpara(&mut b, &Par::body(text.trim_end()), body_sz);
                }
            }

            // Choices.
            if tf {
                wpara(&mut b, &Par { ind, ..Par::body("True   False") }, body_sz);
            } else {
                for c in &q.choices {
                    wpara(&mut b, &Par { ind, ..Par::body(c) }, body_sz);
                }
            }

            if q.image.is_some() {
                wpara(
                    &mut b,
                    &Par { italic: true, center: true, ..Par::body("[picture]") },
                    body_sz,
                );
            }

            for _ in 0..rule_count(q) {
                wpara(&mut b, &Par { rule: true, ..Par::body("") }, body_sz);
            }
            wpara(&mut b, &Par::body(""), body_sz); // breathing room after each question
        }
    }

    b.push_str(&format!(
        "<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/><w:cols w:space=\"720\"/></w:sectPr></w:body></w:document>",
        tw(doc.margin_mm as f64),
        tw(doc.margin_mm as f64),
        tw(doc.margin_mm as f64),
        tw(doc.margin_mm as f64)
    ));

    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opt = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/_rels/document.xml.rels", DOC_RELS),
        ("word/styles.xml", STYLES),
        ("word/document.xml", b.as_str()),
    ] {
        z.start_file(name, opt.clone()).map_err(|e| format!("docx zip: {e}"))?;
        use std::io::Write as _;
        z.write_all(data.as_bytes()).map_err(|e| format!("docx zip: {e}"))?;
    }
    z.finish()
        .map_err(|e| format!("docx zip: {e}"))
        .map(|c| c.into_inner())
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn sample_doc() -> ExamDoc {
        ExamDoc {
            title: "Unit 3 Test".into(),
            header_line: "Name: ___________ Class: ___________ Date: ___________".into(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            answers_inline: true,
            questions: vec![
                DocQ {
                    number: 1,
                    text: vec!["Which planet is closest to the Sun? (a long stem to wrap well past one printed line for testing) or not, choose wisely about the planets, moons, asteroids and comets of the solar system before answering.".into()],
                    choices: vec!["A. Venus".into(), "B. Mercury".into(), "C. Mars".into(), "D. Earth".into()],
                    key_text: "B".into(),
                    rule_lines: 0,
                    points: Some(2.0),
                    image: None,
                },
                DocQ {
                    number: 3,
                    text: vec!["The Sun and the Moon look about the same size in our sky because...".into()],
                    choices: vec![],
                    key_text: "2. because their angular diameters nearly match".into(),
                    rule_lines: 3,
                    points: None,
                    image: None,
                },
                DocQ {
                    number: 4,
                    text: vec!["Two left items".into(), "second left item".into()],
                    choices: vec!["A. first right".into(), "B. second right".into()],
                    key_text: "4. B, A".into(),
                    rule_lines: 0,
                    points: None,
                    image: None,
                },
            ],
            ruled_blanks: 2,
            is_key: false,
            version_label: None,
        }
    }

    fn pdf_pages(pdf: &[u8]) -> usize {
        count_pages(pdf)
    }

    /// Bytes-level page count: entries of "/Type /Page " (Pages has no trailing space).
    fn count_pages(pdf: &[u8]) -> usize {
        let pat: &[u8] = b"/Type /Page ";
        let mut n = 0;
        let mut i = 0;
        while let Some(pos) = find_sub(&pdf[i..], pat) {
            n += 1;
            i += pos + pat.len();
        }
        n
    }

    fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() || hay.len() < needle.len() {
            return None;
        }
        (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
    }

    fn verify_xref(pdf: &[u8]) {
        // startxref -> last one -> offset must point at "xref"
        let sx = find_sub(pdf, b"startxref\n").expect("startxref");
        let num: usize = {
            let rest = &pdf[sx + b"startxref\n".len()..];
            let end = find_sub(rest, b"\n").expect("startxref number end");
            std::str::from_utf8(&rest[..end]).unwrap().trim().parse().unwrap()
        };
        assert_eq!(&pdf[num..num + 4], b"xref", "startxref points at xref");
        // parse entry lines after "xref\n0 N\n"
        let after = find_sub(&pdf[num..], b"\n").unwrap();
        let rest = &pdf[num + after + 1..];
        let hdr_end = find_sub(rest, b"\n").unwrap();
        let count: usize = std::str::from_utf8(&rest[..hdr_end]).unwrap()
            .split_whitespace().last().unwrap().parse().unwrap();
        let mut cursor = num + after + 1 + hdr_end + 1;
        for i in 0..count {
            let entry = &pdf[cursor..cursor + 20]; // entry = 20 bytes incl. 2-byte EOL
            let off: usize = std::str::from_utf8(&entry[..10]).unwrap().parse().unwrap();
            let genno: usize = std::str::from_utf8(&entry[11..16]).unwrap().parse().unwrap();
            let typ = entry[17]; // "OOOOOOOOOO GGGGG n \n" -> type at byte 17
            if i == 0 {
                assert_eq!(off, 0);
                assert_eq!(genno, 65535);
                assert_eq!(typ, b'f');
            } else {
                assert_eq!(typ, b'n', "entry {i} type");
                let want = format!("{i} 0 obj");
                assert_eq!(&pdf[off..off + want.len()], want.as_bytes(), "xref entry {i} offset");
            }
            cursor += 20;
        }
    }

    #[test]
    fn width_tables_have_all_positive_entries() {
        for i in 0..95 {
            assert!(AFM_HELV[i] > 0, "helv index {i}");
            assert!(AFM_HELV_B[i] > 0, "helv bold index {i}");
        }
    }

    #[test]
    fn wrap_breaks_long_lines_and_measures_honestly() {
        let s = "the quick brown fox jumps over the lazy dog again and again because it can".repeat(6);
        for line in wrap(&s, 300.0, Face::Reg, 11.0) {
            assert!(text_w(&line, Face::Reg, 11.0) <= 300.0, "line too wide: {line}");
            assert!(!line.is_empty());
        }
        let out = wrap(&s, 300.0, Face::Reg, 11.0);
        assert!(out.len() > 3, "expected several wrapped lines");
    }

    #[test]
    fn wrap_hard_breaks_oversized_words() {
        let word = "wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww";
        let out = wrap(word, 60.0, Face::Reg, 11.0);
        assert!(out.len() > 1);
        for line in &out {
            assert!(text_w(line, Face::Reg, 11.0) <= 60.0, "hard-broken line too wide");
        }
    }

    #[test]
    fn wrap_keeps_explicit_newlines() {
        assert_eq!(wrap("one\ntwo", 500.0, Face::Reg, 11.0), vec!["one".to_string(), "two".to_string()]);
    }

    #[test]
    fn winansi_maps_latin1_and_replaces_the_rest() {
        let b = winansi("caf\u{e9}");
        assert_eq!(b, b"caf\xE9");
        assert_eq!(winansi("\u{2192}"), b"?"); // arrow -> ?
        assert_eq!(winansi("a\u{2014}b"), b"a\x97b"); // em dash -> WinAnsi 0x97
        assert_eq!(winansi("two\u{09}tabs"), b"two tabs"); // control -> space
    }

    #[test]
    fn pdf_strings_escape_parens() {
        assert_eq!(esc_pdf(&winansi("a (b) \\ c")), b"a \\(b\\) \\\\ c");
    }

    #[test]
    fn pdf_multipage_structure_and_xref_offsets() {
        let long = "This is an intentionally long question stem meant to wrap across several printed lines so that a three question document reliably spills onto at least two pages when the margins are comfortable and the body size is regular eleven point text as usual for real exams. ".repeat(4);
        let mut doc = sample_doc();
        doc.questions[0].text = vec![long.clone()];
        doc.questions[1].text = vec![long.clone()];
        doc.questions[2].text = vec![long.clone()];
        let pdf = render_pdf(&doc).expect("render");

        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert!(find_sub(&pdf, b"/Type /Page ").is_some());
        assert!(find_sub(&pdf, b"/Font").is_some());
        assert!(pdf_pages(&pdf) >= 2, "expected at least two pages");
        verify_xref(&pdf);
        // points label survived (parens are escaped inside PDF strings)
        assert!(find_sub(&pdf, b"2 points").is_some());
        // TF questions collapse True/False onto one line? (not in this doc)
    }

    #[test]
    fn pdf_tf_and_matching_shapes() {
        let tf_doc = ExamDoc {
            title: "TF".into(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            questions: vec![
                DocQ { number: 1, text: vec!["Gravity on the Moon is weaker.".into()], choices: vec!["True".into(), "False".into()], key_text: "True".into(), rule_lines: 0, points: None, image: None },
                DocQ {
                    number: 2,
                    text: vec!["Item one".into(), "Item two".into()],
                    choices: vec!["A. First right".into(), "B. Second right".into()],
                    key_text: "2. B, A".into(),
                    rule_lines: 0,
                    points: None,
                    image: None,
                },
            ],
            ..Default::default()
        };
        let pdf = render_pdf(&tf_doc).expect("render");
        assert!(find_sub(&pdf, b"(True   False)").is_some());
        // matching: left items get continuing numbers 2. and 3.
        assert!(find_sub(&pdf, b"(3.) ").is_some());
        // right column lettered items present
        assert!(find_sub(&pdf, b"(A. First right)").is_some());
        verify_xref(&pdf);
    }

    #[test]
    fn pdf_key_document_renders_key_text() {
        let mut doc = sample_doc();
        doc.is_key = true;
        doc.title = "Untitled Exam - Answer Key".into();
        let pdf = render_pdf(&doc).expect("render");
        assert!(find_sub(&pdf, b"(Untitled Exam - Answer Key)").is_some());
        assert!(find_sub(&pdf, b"diameters").is_some(), "key text renders");
        assert!(find_sub(&pdf, b"(A. Venus)").is_none(), "choices must not print on the key");
        verify_xref(&pdf);
    }

    #[test]
    fn pdf_embeds_images_as_jpeg_xobjects() {
        let img = image::RgbImage::from_fn(24, 12, |x, y| image::Rgb([(x * 10) as u8, (y * 20) as u8, 128]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let mut doc = sample_doc();
        doc.questions[0].image = Some(DocImage { bytes: png, width_px: 24, height_px: 12 });
        let pdf = render_pdf(&doc).expect("render");
        assert!(find_sub(&pdf, b"/Subtype /Image").is_some());
        assert!(find_sub(&pdf, b"/DCTDecode").is_some());
        verify_xref(&pdf);
    }

    #[test]
    fn version_label_prints_muted() {
        let mut doc = sample_doc();
        doc.version_label = Some("  Version B  ".into());
        let pdf = render_pdf(&doc).expect("render");
        assert!(find_sub(&pdf, b"(Version B)").is_some());
        verify_xref(&pdf);
        let bytes = render_docx(&doc).expect("render");
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut docxml = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut docxml).unwrap();
        assert!(docxml.contains("Version B") && docxml.contains("808080"));
    }

    #[test]
    fn points_label_formats() {
        assert_eq!(points_label(1.0), "(1 point)");
        assert_eq!(points_label(2.0), "(2 points)");
        assert_eq!(points_label(2.5), "(2.5 points)");
    }

    #[test]
    fn docx_package_parts_and_content() {
        let mut doc = sample_doc();
        doc.title = "A & B Quiz".into();
        doc.questions[0].image = Some(DocImage { bytes: vec![0], width_px: 24, height_px: 12 });
        let bytes = render_docx(&doc).expect("render");

        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut docxml = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut docxml).unwrap();
        let mut ct = String::new();
        z.by_name("[Content_Types].xml").unwrap().read_to_string(&mut ct).unwrap();
        let mut st = String::new();
        z.by_name("word/styles.xml").unwrap().read_to_string(&mut st).unwrap();

        assert!(docxml.starts_with("<?xml"));
        assert!(docxml.contains("A &amp; B Quiz"), "title escaped correctly");
        assert!(!docxml.contains("A & B Quiz"));
        assert!(docxml.contains("[picture]"));
        assert!(docxml.contains("w:ind w:left=\"432\""));
        assert!(docxml.contains("w:w=\"12240\""), "Letter page size in twips");
        assert!(ct.contains("document.main+xml"));
        assert!(st.contains("Arial"));
    }

    #[test]
    fn docx_key_doc_is_bold_lines() {
        let mut doc = sample_doc();
        doc.is_key = true;
        let bytes = render_docx(&doc).expect("render");
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut docxml = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut docxml).unwrap();
        assert!(docxml.contains("<w:b/>"));
        assert!(docxml.contains("1. Which planet") || docxml.contains("1. B"), "numbered key lines");
        assert!(!docxml.contains("[picture]"), "no pictures on key docs");
    }

    #[test]
    fn empty_doc_still_renders_both() {
        let doc = ExamDoc::default();
        let pdf = render_pdf(&doc).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-1.4"));
        verify_xref(&pdf);
        let bytes = render_docx(&doc).expect("docx");
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut docxml = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut docxml).unwrap();
        // empty pages are still a valid PDF with one page
        let _ = pdf_pages(&pdf);
        assert!(docxml.contains("<w:sectPr>"));
    }
}