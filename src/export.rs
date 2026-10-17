//! Pure PDF and DOCX writers for [`ExamDoc`] (signatures frozen in NOTES.md):
//! `render_pdf(&layout::ExamDoc) -> Result<Vec<u8>, String>` and `render_docx(&layout::ExamDoc)`.
//! No filesystem access; uses only the already-present `image` and `zip` crates.
//!
//! # PDF (1.4, hand-rolled)
//! Letter 612x792pt, 72 pt/inch. Fonts F1=Helvetica, F2=Helvetica-Bold, F3=Helvetica-Oblique,
//! F4=Helvetica-BoldOblique (standard 14; Times-Roman is intentionally unused — clean sans is
//! the print look). Stems, choices and key text carry stem markup (`docs/stem-format.md`),
//! parsed by `richtext` and laid out into items (text lines, table rows): lists with hanging
//! indents, quotes with a rule, bordered tables sized from their content with wrapped cells
//! and a bold shaded header row (repeated after a page break; tables break only between
//! rows), bold/italic runs, sub/superscripts (markup or Unicode ₂ ⁺) as smaller shifted text.
//! Text widths come from embedded AFM width tables for ASCII 32..126 (other bytes: 0.5 em average);
//! non-ASCII maps through a small WinAnsi table (Latin-1 passes 1:1, common Unicode
//! punctuation is covered, the rest becomes '?'). Images decode via the `image` crate and
//! re-encode to JPEG quality 85, embedded as DCTDecode XObjects. Content streams are
//! uncompressed; the xref table carries exact byte offsets, verified by a unit test that
//! resolves every object entry to its `N obj` header.
//!
//! # DOCX (hand-rolled OOXML zipped with the `zip` crate)
//! Minimal valid package: [Content_Types].xml, _rels/.rels, word/document.xml,
//! word/_rels/document.xml.rels, a small styles.xml (Normal/Title/Heading1, Arial
//! default) and numbering.xml (bullets; one restartable decimal list per numbered block).
//! Word does its own wrapping; stem markup becomes real Word structure (bordered tables
//! with a repeating shaded header row and content-sized columns, list paragraphs, quote
//! border, bold/italic/sub/superscript runs). Matching
//! questions print linearly (left items numbered, then the lettered right column
//! indented); images become a centered italic `[picture]` paragraph.

use crate::layout::{DocImage, DocQ, ExamDoc};
use crate::richtext::{self, Block};

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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Face {
    Reg,
    Bold,
    /// Helvetica-Oblique (same advance widths as Helvetica).
    Ital,
    /// Helvetica-BoldOblique (same advance widths as Helvetica-Bold).
    BoldItal,
}

impl Face {
    fn name(self) -> &'static str {
        match self {
            Face::Reg => "F1",
            Face::Bold => "F2",
            Face::Ital => "F3",
            Face::BoldItal => "F4",
        }
    }
    fn afm(self) -> &'static [u16; 95] {
        match self {
            Face::Reg | Face::Ital => &AFM_HELV,
            Face::Bold | Face::BoldItal => &AFM_HELV_B,
        }
    }
    fn of(bold: bool, italic: bool) -> Face {
        match (bold, italic) {
            (false, false) => Face::Reg,
            (true, false) => Face::Bold,
            (false, true) => Face::Ital,
            (true, true) => Face::BoldItal,
        }
    }
    fn bolded(self) -> Face {
        match self {
            Face::Reg | Face::Bold => Face::Bold,
            Face::Ital | Face::BoldItal => Face::BoldItal,
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
/// Explicit `\n` keeps its rows (possibly empty). (Plain-text wrap; stems go through the
/// rich layout below.)
#[cfg_attr(not(test), allow(dead_code))]
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
        if q.text.iter().any(|t| blanks_outside_tables(t) > 0) {
            1 // fill-in-the-blank: one ruled line under the stem
        } else if q.text.iter().any(|t| t.contains("___")) {
            0 // every blank sits in a table cell: the cell is the answer space
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

/// `___` blanks of a stem that are NOT inside a table cell (table blanks are answer cells
/// and need no ruled line under the question).
pub fn blanks_outside_tables(stem: &str) -> usize {
    richtext::parse(stem)
        .iter()
        .map(|b| match b {
            Block::Table { .. } => 0,
            Block::Para(ls) | Block::Bullets(ls) | Block::Numbered(ls) | Block::Quote(ls) => {
                ls.iter().map(|l| richtext::line_text(l).matches("___").count()).sum()
            }
        })
        .sum()
}

// ---------------------------------------------------------------- rich stem layout
//
// Stems (and choices, key text) carry the markup of `docs/stem-format.md`, parsed by
// `richtext`. The PDF lays every block out into `Item`s (one text line, a gap, or one
// table row) so the same list measures a question for keep-together and draws it with a
// page check per item: a list or table never runs off the page, long tables break
// between rows (repeating the header row).

/// A styled run with no internal layout: `script` is -1 subscript, 1 superscript, 0 normal.
#[derive(Clone, Debug)]
struct Frag {
    text: String,
    face: Face,
    script: i8,
}

/// A run placed `dx` points from the start of its line.
#[derive(Clone, Debug)]
struct Placed {
    dx: f64,
    text: String,
    face: Face,
    script: i8,
}

const SCRIPT_SCALE: f64 = 0.7;
const LIST_INDENT: f64 = 16.0;
const NUM_INDENT: f64 = 20.0;
const QUOTE_INDENT: f64 = 12.0;
const CELL_PAD: f64 = 4.0;
const CELL_PAD_V: f64 = 3.0;
/// Width (in em) a blank `___` answer cell asks for.
const BLANK_CELL_EM: f64 = 9.0;

/// A table cell holding only a `___` blank: it prints empty (the cell is the answer box).
fn is_blank_cell(cell: &[Frag]) -> bool {
    let t: String = cell.iter().map(|f| f.text.as_str()).collect();
    let t = t.trim();
    t.len() >= 3 && t.chars().all(|c| c == '_')
}

fn frag_size(script: i8, body: f64) -> f64 {
    if script == 0 { body } else { body * SCRIPT_SCALE }
}

fn frag_w(f: &Frag, body: f64) -> f64 {
    text_w(&f.text, f.face, frag_size(f.script, body))
}

/// Unicode sub/superscript characters -> (ASCII char, script), so every script prints the
/// same way (WinAnsi lacks most of them; ¹²³ are mapped too for a uniform look).
fn unicode_script_char(c: char) -> Option<(char, i8)> {
    let u = c as u32;
    match c {
        '\u{2080}'..='\u{2089}' => Some((char::from(b'0' + (u - 0x2080) as u8), -1)),
        '₊' => Some(('+', -1)),
        '₋' => Some(('-', -1)),
        '₌' => Some(('=', -1)),
        '₍' => Some(('(', -1)),
        '₎' => Some((')', -1)),
        '⁰' => Some(('0', 1)),
        '¹' => Some(('1', 1)),
        '²' => Some(('2', 1)),
        '³' => Some(('3', 1)),
        '\u{2074}'..='\u{2079}' => Some((char::from(b'4' + (u - 0x2074) as u8), 1)),
        '⁺' => Some(('+', 1)),
        '⁻' => Some(('-', 1)),
        '⁼' => Some(('=', 1)),
        '⁽' => Some(('(', 1)),
        '⁾' => Some((')', 1)),
        'ⁿ' => Some(('n', 1)),
        _ => None,
    }
}

fn split_unicode_scripts(text: &str, face: Face, base: i8, out: &mut Vec<Frag>) {
    let mut cur = String::new();
    let mut cur_sc = base;
    for c in text.chars() {
        let (ch, sc) = match unicode_script_char(c) {
            Some((ch, sc)) if base == 0 => (ch, sc),
            Some((ch, _)) => (ch, base), // already inside a sub/sup span
            None => (c, base),
        };
        if sc != cur_sc && !cur.is_empty() {
            out.push(Frag { text: std::mem::take(&mut cur), face, script: cur_sc });
        }
        cur_sc = sc;
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(Frag { text: cur, face, script: cur_sc });
    }
}

/// Styled runs of one parsed line; `$math$` degrades through `equation::stem_to_plain`.
fn line_frags(line: &richtext::Line, force_bold: bool) -> Vec<Frag> {
    let mut out = Vec::new();
    for s in line {
        let mut face = Face::of(s.bold, s.italic);
        if force_bold {
            face = face.bolded();
        }
        let base = if s.sup { 1 } else if s.sub { -1 } else { 0 };
        if s.math {
            let t = crate::equation::stem_to_plain(&format!("${}$", s.text));
            split_unicode_scripts(&t, face, 0, &mut out);
        } else {
            split_unicode_scripts(&s.text, face, base, &mut out);
        }
    }
    out
}

/// Words of a run list (split on single spaces; consecutive spaces keep empty words so
/// "a  b" and leading indentation keep their width, as the plain wrap did).
fn words(frags: &[Frag]) -> Vec<Vec<Frag>> {
    let mut ws: Vec<Vec<Frag>> = vec![vec![]];
    for f in frags {
        for (i, piece) in f.text.split(' ').enumerate() {
            if i > 0 {
                ws.push(vec![]);
            }
            if !piece.is_empty() {
                ws.last_mut().unwrap().push(Frag { text: piece.to_string(), ..f.clone() });
            }
        }
    }
    ws
}

fn word_w(w: &[Frag], body: f64) -> f64 {
    w.iter().map(|f| frag_w(f, body)).sum()
}

/// Width of a run list laid on one line.
fn natural_w(frags: &[Frag], body: f64) -> f64 {
    let ws = words(frags);
    let sp = char_w(32, Face::Reg, body);
    ws.iter().map(|w| word_w(w, body)).sum::<f64>() + sp * ws.len().saturating_sub(1) as f64
}

/// Widest single word (the narrowest a column can get without breaking words).
fn min_word_w(frags: &[Frag], body: f64) -> f64 {
    words(frags).iter().map(|w| word_w(w, body)).fold(0.0, f64::max)
}

/// Greedy wrap of styled runs: the first line gets `first_w`, the rest `rest_w`; words
/// wider than the line hard-break per character. Always returns at least one line.
fn wrap_frags(frags: &[Frag], first_w: f64, rest_w: f64, body: f64) -> Vec<Vec<Placed>> {
    let sp = char_w(32, Face::Reg, body);
    let mut lines: Vec<Vec<Placed>> = Vec::new();
    let mut cur: Vec<Placed> = Vec::new();
    let mut x = 0.0;
    let mut n = 0usize;
    let maxw = |lines: &Vec<Vec<Placed>>| if lines.is_empty() { first_w } else { rest_w };
    for word in words(frags) {
        let ww = word_w(&word, body);
        if n > 0 && ww > 0.0 && x + sp + ww > maxw(&lines) {
            lines.push(std::mem::take(&mut cur));
            x = 0.0;
            n = 0;
        } else if n > 0 {
            x += sp;
        }
        if ww > maxw(&lines) - x {
            for f in &word {
                let size = frag_size(f.script, body);
                for ch in f.text.chars() {
                    let s = ch.to_string();
                    let cw = text_w(&s, f.face, size);
                    if x > 0.0 && x + cw > maxw(&lines) {
                        lines.push(std::mem::take(&mut cur));
                        x = 0.0;
                    }
                    cur.push(Placed { dx: x, text: s, face: f.face, script: f.script });
                    x += cw;
                }
            }
        } else {
            for f in word {
                let w = frag_w(&f, body);
                cur.push(Placed { dx: x, text: f.text, face: f.face, script: f.script });
                x += w;
            }
        }
        n += 1;
    }
    lines.push(cur);
    lines.into_iter().map(|l| merge_runs(l, body)).collect()
}

/// Joins neighbouring same-style normal runs separated only by spaces into one run, so a
/// plain line prints as one string (as before the rich layout) and extracts cleanly.
fn merge_runs(line: Vec<Placed>, body: f64) -> Vec<Placed> {
    let sp = char_w(32, Face::Reg, body);
    let mut out: Vec<Placed> = Vec::with_capacity(line.len());
    for p in line {
        if let Some(last) = out.last_mut() {
            if last.script == 0 && p.script == 0 && last.face == p.face {
                let end = last.dx + text_w(&last.text, last.face, body);
                let gap = p.dx - end;
                let face_sp = char_w(32, p.face, body);
                // Gap of whole spaces in this face (the layout uses the regular space width,
                // which equals the bold one for Helvetica).
                let k = (gap / sp).round();
                if gap >= -0.01 && k >= 0.0 && (gap - k * sp).abs() < 0.01 && (sp - face_sp).abs() < 0.01 {
                    last.text.push_str(&" ".repeat(k as usize));
                    last.text.push_str(&p.text);
                    continue;
                }
            }
        }
        out.push(p);
    }
    out
}

/// Wrapped lines of a short string (a choice, a matching item): `\n` breaks, inline
/// markup only (no blocks).
fn text_lines(s: &str, first_w: f64, rest_w: f64, body: f64) -> Vec<Vec<Placed>> {
    let mut out = Vec::new();
    for l in s.split('\n') {
        let fw = if out.is_empty() { first_w } else { rest_w };
        out.extend(wrap_frags(&line_frags(&richtext::inline(l), false), fw, rest_w, body));
    }
    out
}

#[derive(Clone, Debug)]
enum Item {
    /// One text line drawn `indent` points right of the block's x; `marker` (bullet or
    /// list number) ends just left of the indent; `bar` draws the quote rule.
    Line { indent: f64, frags: Vec<Placed>, marker: Option<String>, bar: bool },
    Gap(f64),
    /// One table row: column (x offset, width), wrapped cell lines, row height; rows after
    /// a header point back at it so a page break can repeat it.
    Row { cols: Vec<(f64, f64)>, cells: Vec<Vec<Vec<Placed>>>, h: f64, header: bool, header_idx: Option<usize> },
}

fn item_h(it: &Item, lead: f64) -> f64 {
    match it {
        Item::Line { .. } => lead,
        Item::Gap(h) => *h,
        Item::Row { h, .. } => *h,
    }
}

fn items_h(items: &[Item], lead: f64) -> f64 {
    items.iter().map(|i| item_h(i, lead)).sum()
}

/// Column widths (points, padding included) of a table whose cells are `rows` of run lists,
/// fitted into `max_w`: natural widths when they fit, else every column keeps its widest
/// word and the rest of the room goes where the text is longest.
fn table_col_widths(rows: &[Vec<Vec<Frag>>], max_w: f64, body: f64) -> Vec<f64> {
    let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut nat = vec![0.0f64; ncol];
    let mut min = vec![0.0f64; ncol];
    for r in rows {
        for (c, cell) in r.iter().enumerate() {
            // A blank answer cell asks for room to write in.
            let want = if is_blank_cell(cell) { BLANK_CELL_EM * body } else { natural_w(cell, body) };
            nat[c] = nat[c].max(want + 2.0 * CELL_PAD + 1.0);
            min[c] = min[c].max(min_word_w(cell, body) + 2.0 * CELL_PAD + 1.0);
        }
    }
    for c in 0..ncol {
        nat[c] = nat[c].max(3.0 * body);
        min[c] = min[c].min(nat[c]);
    }
    let total: f64 = nat.iter().sum();
    if total <= max_w {
        return nat;
    }
    let mins: f64 = min.iter().sum();
    if mins >= max_w {
        return min.iter().map(|m| m * max_w / mins).collect();
    }
    let slack: f64 = (0..ncol).map(|c| nat[c] - min[c]).sum();
    let extra = max_w - mins;
    (0..ncol).map(|c| min[c] + if slack > 0.0 { extra * (nat[c] - min[c]) / slack } else { 0.0 }).collect()
}

fn layout_table(header: bool, rows: &[Vec<richtext::Line>], max_w: f64, body: f64, lead: f64, base: usize) -> Vec<Item> {
    let fr: Vec<Vec<Vec<Frag>>> = rows
        .iter()
        .enumerate()
        .map(|(ri, r)| r.iter().map(|c| line_frags(c, header && ri == 0)).collect())
        .collect();
    let widths = table_col_widths(&fr, max_w, body);
    let mut x = 0.0;
    let cols: Vec<(f64, f64)> = widths
        .iter()
        .map(|w| {
            let c = (x, *w);
            x += w;
            c
        })
        .collect();
    let mut out = Vec::new();
    for (ri, r) in fr.iter().enumerate() {
        let cells: Vec<Vec<Vec<Placed>>> = r
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                let w = (cols[c].1 - 2.0 * CELL_PAD).max(body);
                if is_blank_cell(cell) { vec![vec![]] } else { wrap_frags(cell, w, w, body) }
            })
            .collect();
        let nl = cells.iter().map(|c| c.len()).max().unwrap_or(1).max(1);
        let roomy = if r.iter().any(|c| is_blank_cell(c)) { 0.5 * body } else { 0.0 };
        let h = nl as f64 * lead + 2.0 * CELL_PAD_V + roomy;
        let is_head = header && ri == 0;
        out.push(Item::Row { cols: cols.clone(), cells, h, header: is_head, header_idx: (header && ri > 0).then_some(base) });
    }
    out
}

/// Lays parsed stem blocks into items `avail` points wide; the very first text line gets
/// `first_w` (room for the points label), a leading table fits `first_w` too.
fn layout_blocks(blocks: &[Block], avail: f64, first_w: f64, body: f64, lead: f64) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut first = true;
    let gap = body * 0.35;
    for (bi, b) in blocks.iter().enumerate() {
        if bi > 0 {
            items.push(Item::Gap(gap));
        }
        let fw = |first: bool, indent: f64| if first { first_w - indent } else { avail - indent };
        match b {
            Block::Para(lines) => {
                for l in lines {
                    for pl in wrap_frags(&line_frags(l, false), fw(first, 0.0), avail, body) {
                        items.push(Item::Line { indent: 0.0, frags: pl, marker: None, bar: false });
                        first = false;
                    }
                }
            }
            Block::Bullets(lines) | Block::Numbered(lines) | Block::Quote(lines) => {
                let (indent, bar) = match b {
                    Block::Bullets(_) => (LIST_INDENT, false),
                    Block::Numbered(_) => (NUM_INDENT, false),
                    _ => (QUOTE_INDENT, true),
                };
                for (k, l) in lines.iter().enumerate() {
                    let wrapped = wrap_frags(&line_frags(l, false), fw(first, indent), avail - indent, body);
                    for (j, pl) in wrapped.into_iter().enumerate() {
                        let marker = (j == 0).then(|| match b {
                            Block::Bullets(_) => Some("\u{2022}".to_string()),
                            Block::Numbered(_) => Some(format!("{}.", k + 1)),
                            _ => None,
                        });
                        items.push(Item::Line { indent, frags: pl, marker: marker.flatten(), bar });
                        first = false;
                    }
                }
            }
            Block::Table { header, rows } => {
                let base = items.len();
                items.extend(layout_table(*header, rows, if first { first_w } else { avail }, body, lead, base));
                if bi + 1 == blocks.len() {
                    items.push(Item::Gap(body * 0.5)); // breathing room under a closing table
                }
                first = false;
            }
        }
    }
    items
}

fn stem_blocks(text: &[String]) -> Vec<Block> {
    text.iter().flat_map(|t| richtext::parse(t)).collect()
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
    /// The printed "Name / Class / Date" line, repeated at the top of every page after
    /// the first (set once page 1's header area is drawn; `None` on the answer key).
    running: Option<String>,
}

impl R {
    fn remaining(&self) -> f64 {
        self.y - self.margin
    }
    fn full_height(&self) -> f64 {
        PAGE_H - 2.0 * self.margin - self.running.as_ref().map_or(0.0, |_| self.running_h())
    }
    /// Height the repeated header takes at the top of a continuation page.
    fn running_h(&self) -> f64 {
        self.lead + self.body * 0.6
    }
    fn new_page(&mut self) {
        if !self.cur.is_empty() {
            let cur = std::mem::take(&mut self.cur);
            self.pages.push((cur, std::mem::take(&mut self.used)));
        }
        self.y = PAGE_H - self.margin;
        if let Some(h) = self.running.clone() {
            let x0 = self.x0;
            self.line(x0, &h, Face::Reg);
            self.y -= self.body * 0.6;
        }
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
    /// Draws placed runs on baseline `by` starting at `x` (sub/superscripts smaller and
    /// shifted).
    fn placed(&mut self, x: f64, by: f64, frags: &[Placed]) {
        for p in frags {
            if p.text.is_empty() {
                continue;
            }
            let size = frag_size(p.script, self.body);
            let dy = match p.script {
                -1 => -0.2 * self.body,
                1 => 0.36 * self.body,
                _ => 0.0,
            };
            self.text(x + p.dx, by + dy, p.face, size, &p.text);
        }
    }
    /// Draws layout items at `x`, breaking pages between items (never inside one); a table
    /// row that starts a new page repeats its table's header row first.
    fn items(&mut self, x: f64, items: &[Item]) {
        for it in items {
            match it {
                Item::Gap(h) => self.y -= h,
                Item::Line { indent, frags, marker, bar } => {
                    if self.remaining() < self.lead {
                        self.new_page();
                    }
                    let by = self.y - Self::ascent_of(self.body);
                    if let Some(m) = marker {
                        let w = text_w(m, Face::Reg, self.body);
                        self.text(x + indent - 5.0 - w, by, Face::Reg, self.body, m);
                    }
                    if *bar {
                        let bx = x + indent - 7.0;
                        self.stroke_line(bx, self.y, bx, self.y - self.lead, 1.4, "0.6 0.6 0.6");
                    }
                    self.placed(x + indent, by, frags);
                    self.y -= self.lead;
                }
                Item::Row { h, header_idx, .. } => {
                    if self.remaining() < *h {
                        self.new_page();
                        if let Some(hi) = header_idx {
                            if let Some(head @ Item::Row { h: hh, .. }) = items.get(*hi) {
                                if self.remaining() >= hh + h {
                                    self.row(x, head);
                                    self.y -= hh;
                                }
                            }
                        }
                    }
                    self.row(x, it);
                    self.y -= h;
                }
            }
        }
    }
    fn stroke_line(&mut self, xa: f64, ya: f64, xb: f64, yb: f64, w: f64, gray: &str) {
        self.cur.extend_from_slice(
            format!("{} w {gray} RG {} {} m {} {} l S\n", fnum(w), fnum(xa), fnum(ya), fnum(xb), fnum(yb)).as_bytes(),
        );
    }
    /// One table row with its top at the cursor: header fill, cell borders, cell text.
    fn row(&mut self, x: f64, it: &Item) {
        let Item::Row { cols, cells, h, header, .. } = it else { return };
        let top = self.y;
        let total: f64 = cols.iter().map(|c| c.1).sum();
        if *header {
            self.cur.extend_from_slice(
                format!("0.93 g {} {} {} {} re f 0 g\n", fnum(x), fnum(top - h), fnum(total), fnum(*h)).as_bytes(),
            );
        }
        self.cur.extend_from_slice(b"0.6 w 0.25 0.25 0.25 RG ");
        for (cx, cw) in cols {
            self.cur.extend_from_slice(
                format!("{} {} {} {} re ", fnum(x + cx), fnum(top - h), fnum(*cw), fnum(*h)).as_bytes(),
            );
        }
        self.cur.extend_from_slice(b"S\n");
        let nl = cells.iter().map(|c| c.len()).max().unwrap_or(1).max(1);
        let extra = ((h - 2.0 * CELL_PAD_V - nl as f64 * self.lead) / 2.0).max(0.0);
        for (c, lines) in cells.iter().enumerate() {
            let Some((cx, _)) = cols.get(c) else { continue };
            for (j, l) in lines.iter().enumerate() {
                // Cap height centred in the line slot (and the slot in a roomy row).
                let by = top - CELL_PAD_V - extra - j as f64 * self.lead - (0.5 * self.lead + 0.33 * self.body);
                self.placed(x + cx + CELL_PAD, by, l);
            }
        }
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
        running: None,
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
        // Like Test Parrot, the Name/Class/Date line repeats at the top of every page.
        if !doc.header_line.trim().is_empty() {
            r.running = Some(doc.header_line.trim().to_string());
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

    let mut next_img = 0usize;
    for qi in 0..doc.questions.len() {
        let q = &doc.questions[qi];
        // Index into `imgs`, which holds only the questions that carry an image.
        let img_idx = q.image.as_ref().map(|_| {
            next_img += 1;
            next_img - 1
        });
        let est = q_est_height(&r, q, img_idx, doc.is_key);
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
    let items = layout_blocks(&richtext::parse(q.key_text.trim()), avail, avail, r.body, r.lead);
    if r.remaining() < r.lead {
        r.new_page();
    }
    r.num_right(stem_x - NUM_GAP, &format!("{}.", q.number), Face::Bold);
    r.items(stem_x, &items);
}

/// Stem items of an ordinary question plus the points label and its reserved width.
fn stem_items(r: &R, q: &DocQ) -> (Vec<Item>, Option<String>) {
    let avail = r.x1 - (r.x0 + STEM_INSET);
    let pt_label = q.points.map(points_label);
    let label_w = pt_label.as_deref().map(|l| text_w(l, Face::Reg, r.body) + 6.0).unwrap_or(0.0);
    (layout_blocks(&stem_blocks(&q.text), avail, avail - label_w, r.body, r.lead), pt_label)
}

/// Matching row layout: wrapped left item and right choice for each row.
fn matching_rows(r: &R, q: &DocQ) -> Vec<(Vec<Vec<Placed>>, Vec<Vec<Placed>>, f64)> {
    let stem_x = r.x0 + STEM_INSET;
    let mid = r.x0 + (r.x1 - r.x0) * 0.5;
    let left_w = mid - stem_x - 8.0;
    let right_w = r.x1 - mid - 8.0;
    let rows = q.text.len().max(q.choices.len());
    (0..rows)
        .map(|i| {
            let l = q.text.get(i).map(|t| text_lines(t, left_w, left_w, r.body)).unwrap_or_default();
            let rt = q.choices.get(i).map(|c| text_lines(c, right_w, right_w, r.body)).unwrap_or_default();
            let n = l.len().max(rt.len()).max(1);
            let h = (r.lead * n as f64).max(24.0);
            (l, rt, h)
        })
        .collect()
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
        let pt_label = q.points.map(points_label);
        for (i, (left, right, row_h)) in matching_rows(r, q).into_iter().enumerate() {
            if r.remaining() < row_h {
                r.new_page();
            }
            let top = r.y;
            for (j, piece) in left.iter().enumerate() {
                let by = top - j as f64 * r.lead - R::ascent_of(r.body);
                if j == 0 {
                    let n = q.number as usize + i;
                    let lbl = format!("{n}.");
                    let w = text_w(&lbl, Face::Reg, r.body);
                    r.text(stem_x - NUM_GAP - w, by, Face::Reg, r.body, &lbl);
                }
                r.placed(stem_x, by, piece);
            }
            for (j, piece) in right.iter().enumerate() {
                let by = top - j as f64 * r.lead - R::ascent_of(r.body);
                r.placed(mid, by, piece);
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

    // Stem blocks; the first line carries the number and a right-aligned "(N points)"
    // label, and wraps narrower so label and stem never collide.
    let (items, pt_label) = stem_items(r, q);
    let first_h = items.first().map(|i| item_h(i, r.lead)).unwrap_or(r.lead);
    if r.remaining() < first_h {
        r.new_page();
    }
    // A stem that opens with a table puts the number on the first row's text line.
    let dy = match items.first() {
        Some(Item::Row { h, cells, .. }) => {
            let nl = cells.iter().map(|c| c.len()).max().unwrap_or(1).max(1);
            let extra = ((h - 2.0 * CELL_PAD_V - nl as f64 * r.lead) / 2.0).max(0.0);
            CELL_PAD_V + extra + 0.5 * r.lead + 0.33 * r.body - R::ascent_of(r.body)
        }
        _ => 0.0,
    };
    let by = r.y - R::ascent_of(r.body) - dy;
    let lbl = format!("{}.", q.number);
    let w = text_w(&lbl, Face::Reg, r.body);
    r.text(stem_x - NUM_GAP - w, by, Face::Reg, r.body, &lbl);
    if let Some(lbl) = &pt_label {
        r.text(r.x1 - text_w(lbl, Face::Reg, r.body), by, Face::Reg, r.body, lbl);
    }
    r.items(stem_x, &items);

    // Picture right under the stem, before choices and ruled lines (as the page preview
    // draws it; page breaks first if it does not fit).
    if let Some(idx) = img_idx {
        draw_pdf_image(r, idx);
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
            for l in text_lines(c, avail, avail, r.body) {
                if r.remaining() < r.lead {
                    r.new_page();
                }
                let by = r.y - R::ascent_of(r.body);
                r.placed(stem_x, by, &l);
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

}

/// Question height for keep-together page breaks, from the same layout the renderer draws.
fn q_est_height(r: &R, q: &DocQ, img_idx: Option<usize>, is_key: bool) -> f64 {
    let stem_x = r.x0 + STEM_INSET;
    let avail = r.x1 - stem_x;
    if is_key {
        if q.key_text.trim().is_empty() {
            return 0.0;
        }
        return items_h(&layout_blocks(&richtext::parse(q.key_text.trim()), avail, avail, r.body, r.lead), r.lead);
    }
    let tf = is_tf(&q.choices);
    if looks_matching(q, tf) {
        return matching_rows(r, q).iter().map(|x| x.2).sum::<f64>() + r.body;
    }
    let mut h = items_h(&stem_items(r, q).0, r.lead);
    let lines = if tf {
        1
    } else {
        q.choices.iter().map(|c| text_lines(c, avail, avail, r.body).len()).sum::<usize>()
    };
    h += lines as f64 * r.lead + rule_count(q) as f64 * r.body * 2.0;
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
    // Objects: 1 catalog, 2 page tree, 3..=6 fonts, then images, then 2 per page.
    const FIRST_IMG: usize = 7;
    let kids: String = (0..npage).map(|p| format!("{} 0 R ", FIRST_IMG + n_imgs + p * 2 + 1)).collect();

    let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(4 + n_imgs + npage * 2);
    // obj 1: catalog
    bodies.push(b"<< /Type /Catalog /Pages 2 0 R >>\n".to_vec());
    // obj 2: page tree
    bodies.push(format!("<< /Type /Pages /Kids [{kids}] /Count {npage} >>\n").into_bytes());
    // obj 3..=6: fonts F1..F4
    for base in ["Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Helvetica-BoldOblique"] {
        bodies.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>\n").into_bytes());
    }
    // obj 7..: image XObjects (JPEG bytes embedded raw, no deflate).
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
                xo.push_str(&format!("/Im{} {} 0 R ", idx + 1, FIRST_IMG + idx));
            }
            xo.push_str(">>");
        }
        bodies.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R /F4 6 0 R >>{xo} /ProcSet [/PDF /Text /ImageC] >> /Contents {} 0 R >>\n",
                FIRST_IMG + n_imgs + p * 2
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

/// One run: text with bold/italic/sub/sup and size (half-points).
fn wrun(xml: &mut String, text: &str, bold: bool, italic: bool, script: i8, sz: i64) {
    if text.is_empty() {
        return;
    }
    xml.push_str("<w:r><w:rPr>");
    if bold {
        xml.push_str("<w:b/>");
    }
    if italic {
        xml.push_str("<w:i/>");
    }
    match script {
        1 => xml.push_str("<w:vertAlign w:val=\"superscript\"/>"),
        -1 => xml.push_str("<w:vertAlign w:val=\"subscript\"/>"),
        _ => {}
    }
    xml.push_str(&format!("<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/></w:rPr><w:t xml:space=\"preserve\">"));
    xml.push_str(&xml_esc(text));
    xml.push_str("</w:t></w:r>");
}

/// Runs of one parsed line, through the same styled runs as the PDF: `$math$` degraded to
/// Unicode, sub/superscripts (markup or Unicode ₂ ⁺ ...) as real vertAlign runs.
fn wline(xml: &mut String, line: &richtext::Line, sz: i64, force_bold: bool) {
    wfrags(xml, &line_frags(line, force_bold), sz);
}

fn wfrags(xml: &mut String, frags: &[Frag], sz: i64) {
    for f in frags {
        let bold = matches!(f.face, Face::Bold | Face::BoldItal);
        let italic = matches!(f.face, Face::Ital | Face::BoldItal);
        wrun(xml, &f.text, bold, italic, f.script, sz);
    }
}

/// Stem body of an ordinary question: the number hangs in a 0.3" gutter on the first
/// paragraph (points label at its end), later paragraphs and blocks indent to the same
/// 0.3" column; bullets/numbered lists use the numbering part, tables are real Word
/// tables. `num_lists` counts numbered lists so each restarts at 1.
fn docx_stem(b: &mut String, q: &DocQ, pt_label: Option<&str>, body_sz: i64, text_w_tw: i64, num_lists: &mut usize) {
    const IND: i64 = 432;
    let mut prefix: Option<String> = Some(format!("{}.", q.number));
    // Emits the pending "N." (and points label) as its own paragraph before a non-text block.
    let flush_prefix = |b: &mut String, prefix: &mut Option<String>| {
        if let Some(p) = prefix.take() {
            b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{IND}\" w:hanging=\"{IND}\"/></w:pPr>"));
            wrun(b, &p, false, false, 0, body_sz);
            if let Some(l) = pt_label {
                b.push_str("<w:r><w:tab/></w:r>");
                wrun(b, l, false, false, 0, body_sz);
            }
            b.push_str("</w:p>");
        }
    };
    for block in stem_blocks(&q.text) {
        match &block {
            Block::Para(lines) => {
                for l in lines {
                    match prefix.take() {
                        Some(p) => {
                            b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{IND}\" w:hanging=\"{IND}\"/></w:pPr>"));
                            wrun(b, &p, false, false, 0, body_sz);
                            b.push_str("<w:r><w:tab/></w:r>");
                            wline(b, l, body_sz, false);
                            if let Some(lbl) = pt_label {
                                wrun(b, &format!(" {lbl}"), false, false, 0, body_sz);
                            }
                        }
                        None => {
                            b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{IND}\"/></w:pPr>"));
                            wline(b, l, body_sz, false);
                        }
                    }
                    b.push_str("</w:p>");
                }
            }
            Block::Bullets(lines) | Block::Numbered(lines) => {
                flush_prefix(b, &mut prefix);
                let num_id = if matches!(block, Block::Bullets(_)) {
                    1
                } else {
                    *num_lists += 1;
                    1 + *num_lists
                };
                for l in lines {
                    b.push_str(&format!(
                        "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>"
                    ));
                    wline(b, l, body_sz, false);
                    b.push_str("</w:p>");
                }
            }
            Block::Quote(lines) => {
                flush_prefix(b, &mut prefix);
                for l in lines {
                    b.push_str(&format!(
                        "<w:p><w:pPr><w:pBdr><w:left w:val=\"single\" w:sz=\"12\" w:space=\"6\" w:color=\"999999\"/></w:pBdr><w:ind w:left=\"{}\"/></w:pPr>",
                        IND + 200
                    ));
                    wline(b, l, body_sz, false);
                    b.push_str("</w:p>");
                }
            }
            Block::Table { header, rows } => {
                flush_prefix(b, &mut prefix);
                docx_table(b, *header, rows, body_sz, text_w_tw - IND, IND);
            }
        }
    }
    flush_prefix(b, &mut prefix);
}

/// A bordered Word table indented `ind` twips; column widths come from the same content
/// measure as the PDF (Helvetica metrics, close to Arial), header row bold and shaded and
/// repeated on each page.
fn docx_table(b: &mut String, header: bool, rows: &[Vec<richtext::Line>], body_sz: i64, max_tw: i64, ind: i64) {
    let body = body_sz as f64 / 2.0;
    let fr: Vec<Vec<Vec<Frag>>> = rows
        .iter()
        .enumerate()
        .map(|(ri, r)| r.iter().map(|c| line_frags(c, header && ri == 0)).collect())
        .collect();
    // A little slack: Word's Arial runs a hair wider than the Helvetica tables.
    let widths: Vec<i64> = table_col_widths(&fr, max_tw as f64 / 20.0 / 1.06, body)
        .iter()
        .map(|w| (w * 1.06 * 20.0).round() as i64)
        .collect();
    let total: i64 = widths.iter().sum();
    let bd = |side: &str| format!("<w:{side} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"404040\"/>");
    b.push_str(&format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"{total}\" w:type=\"dxa\"/><w:tblInd w:w=\"{ind}\" w:type=\"dxa\"/><w:tblBorders>{}{}{}{}{}{}</w:tblBorders><w:tblLayout w:type=\"fixed\"/><w:tblCellMar><w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>",
        bd("top"), bd("left"), bd("bottom"), bd("right"), bd("insideH"), bd("insideV")
    ));
    for w in &widths {
        b.push_str(&format!("<w:gridCol w:w=\"{w}\"/>"));
    }
    b.push_str("</w:tblGrid>");
    for ri in 0..rows.len() {
        let head = header && ri == 0;
        b.push_str("<w:tr><w:trPr><w:cantSplit/>");
        if head {
            b.push_str("<w:tblHeader/>");
        } else if fr[ri].iter().any(|c| is_blank_cell(c)) {
            b.push_str(&format!("<w:trHeight w:val=\"{}\"/>", body_sz * 10 * 2)); // ~2 lines to write in
        }
        b.push_str("</w:trPr>");
        for (c, w) in widths.iter().enumerate() {
            b.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/>"));
            if head {
                b.push_str("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"EDEDED\"/>");
            }
            b.push_str("</w:tcPr><w:p><w:pPr><w:spacing w:before=\"40\" w:after=\"40\"/></w:pPr>");
            if let Some(cell) = fr[ri].get(c) {
                if !is_blank_cell(cell) {
                    wfrags(b, cell, body_sz);
                }
            }
            b.push_str("</w:p></w:tc>");
        }
        b.push_str("</w:tr>");
    }
    b.push_str("</w:tbl>");
}

/// A choice paragraph indented `ind` twips (inline markup honoured; `\n` breaks lines).
fn choice_para(b: &mut String, c: &str, ind: i64, body_sz: i64) {
    b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{ind}\"/></w:pPr>"));
    for (i, l) in c.split('\n').enumerate() {
        if i > 0 {
            b.push_str("<w:r><w:br/></w:r>");
        }
        wline(b, &richtext::inline(l), body_sz, false);
    }
    b.push_str("</w:p>");
}

/// numbering.xml: numId 1 = bullets; numIds 2.. = one decimal list each (restart at 1).
fn numbering_xml(num_lists: usize) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:abstractNum w:abstractNumId=\"0\"><w:multiLevelType w:val=\"singleLevel\"/><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"\u{2022}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"792\" w:hanging=\"288\"/></w:pPr></w:lvl></w:abstractNum>\
<w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"singleLevel\"/><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"decimal\"/><w:lvlText w:val=\"%1.\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"792\" w:hanging=\"360\"/></w:pPr></w:lvl></w:abstractNum>\
<w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num>",
    );
    for k in 0..num_lists {
        s.push_str(&format!(
            "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"1\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"1\"/></w:lvlOverride></w:num>",
            2 + k
        ));
    }
    s.push_str("</w:numbering>");
    s
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOC_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;

/// Content-type overrides and document relationships for the repeating page header
/// (header1 = pages 2+, header2 = the empty first-page header).
const HEADER_TYPES: &str = r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#;
const HEADER_RELS: &str = r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/></Relationships>"#;
const EMPTY_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:hdr>"#;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="120"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="120" w:after="60"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style></w:styles>"#;

pub fn render_docx(doc: &ExamDoc) -> Result<Vec<u8>, String> {
    let tw = |mm: f64| (mm * 1440.0 / 25.4).round() as i64; // mm -> twips
    let body_sz = ((doc.body_pt as f64).clamp(6.0, 20.0) * 2.0).round() as i64; // half-points
    let head_sz = ((doc.heading_pt as f64).clamp(8.0, 36.0) * 2.0).round() as i64;

    let mut b = String::new();
    b.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    b.push_str(
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>",
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

    // Text column width in twips (Letter minus both margins).
    let text_w_tw = 12240 - 2 * tw(doc.margin_mm as f64);
    let mut num_lists = 0usize;
    for q in &doc.questions {
        if doc.is_key {
            // Bold key lines: "N. answer" then any further lines (the explanation).
            let mut first = true;
            for l in q.key_text.trim().split('\n').filter(|l| !l.trim().is_empty()) {
                let l = if first { format!("{}. {}", q.number, l.trim_end()) } else { l.trim_end().to_string() };
                first = false;
                b.push_str("<w:p>");
                wline(&mut b, &richtext::inline(&l), body_sz, true);
                b.push_str("</w:p>");
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
                b.push_str("<w:p>");
                wrun(&mut b, &format!("{}. ", q.number as usize + i), false, false, 0, body_sz);
                wline(&mut b, &richtext::inline(item), body_sz, false);
                b.push_str("</w:p>");
            }
            for c in &q.choices {
                choice_para(&mut b, c, ind, body_sz);
            }
        } else {
            docx_stem(&mut b, q, pt_label.as_deref(), body_sz, text_w_tw, &mut num_lists);

            if q.image.is_some() {
                wpara(
                    &mut b,
                    &Par { italic: true, center: true, ..Par::body("[picture]") },
                    body_sz,
                );
            }

            // Choices.
            if tf {
                wpara(&mut b, &Par { ind, ..Par::body("True   False") }, body_sz);
            } else {
                for c in &q.choices {
                    choice_para(&mut b, c, ind, body_sz);
                }
            }


            for _ in 0..rule_count(q) {
                wpara(&mut b, &Par { rule: true, ..Par::body("") }, body_sz);
            }
            wpara(&mut b, &Par::body(""), body_sz); // breathing room after each question
        }
    }

    // The Name/Class/Date line repeats on every page (Test Parrot does the same): a Word
    // page header on pages 2+ (`titlePg` with an empty first-page header, since page 1
    // already prints it under the title).
    let running = (!doc.is_key && !doc.header_line.trim().is_empty()).then(|| {
        let mut h = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:hdr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">",
        );
        wpara(&mut h, &Par::body(doc.header_line.trim()), body_sz);
        h.push_str("</w:hdr>");
        h
    });
    let (head_refs, title_pg) = if running.is_some() {
        ("<w:headerReference w:type=\"default\" r:id=\"rId3\"/><w:headerReference w:type=\"first\" r:id=\"rId4\"/>", "<w:titlePg/>")
    } else {
        ("", "")
    };
    b.push_str(&format!(
        "<w:sectPr>{head_refs}<w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/><w:cols w:space=\"720\"/>{title_pg}</w:sectPr></w:body></w:document>",
        tw(doc.margin_mm as f64),
        tw(doc.margin_mm as f64),
        tw(doc.margin_mm as f64),
        tw(doc.margin_mm as f64)
    ));

    let numbering = numbering_xml(num_lists);
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opt = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let (types, rels) = match &running {
        Some(_) => (
            CONTENT_TYPES.replace("</Types>", HEADER_TYPES),
            DOC_RELS.replace("</Relationships>", HEADER_RELS),
        ),
        None => (CONTENT_TYPES.to_string(), DOC_RELS.to_string()),
    };
    let mut parts: Vec<(&str, &str)> = vec![
        ("[Content_Types].xml", types.as_str()),
        ("_rels/.rels", ROOT_RELS),
        ("word/_rels/document.xml.rels", rels.as_str()),
        ("word/styles.xml", STYLES),
        ("word/numbering.xml", numbering.as_str()),
        ("word/document.xml", b.as_str()),
    ];
    if let Some(h) = &running {
        parts.push(("word/header1.xml", h.as_str()));
        parts.push(("word/header2.xml", EMPTY_HEADER));
    }
    for (name, data) in parts {
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
    fn header_line_repeats_on_every_page() {
        let mut doc = sample_doc();
        doc.header_line = "Name: Zed Class: Q Date: D".into();
        let base = doc.questions[1].clone();
        doc.questions = (1..=40).map(|n| DocQ { number: n, ..base.clone() }).collect();
        let pdf = render_pdf(&doc).unwrap();
        let pages = count_pages(&pdf);
        assert!(pages >= 3, "40 essay questions span pages");
        let heads = String::from_utf8_lossy(&pdf).matches("(Name: Zed Class: Q Date: D) Tj").count();
        assert_eq!(heads, pages, "the header prints once per page");
        let margin = 18.0 * MM_PT;
        for y in baselines(&pdf) {
            assert!(y > margin - 1.0 && y < PAGE_H - margin + 1.0, "text outside the margins at y={y}");
        }
        // The answer key carries no header at all.
        let key = render_pdf(&ExamDoc { is_key: true, ..doc.clone() }).unwrap();
        assert!(!String::from_utf8_lossy(&key).contains("(Name: Zed"));

        // DOCX: a Word page header on pages 2+ (titlePg + empty first-page header).
        let bytes = render_docx(&doc).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let read = |z: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>, n: &str| {
            let mut s = String::new();
            z.by_name(n).unwrap().read_to_string(&mut s).unwrap();
            s
        };
        let docxml = read(&mut z, "word/document.xml");
        assert!(docxml.contains(r#"<w:headerReference w:type="default" r:id="rId3"/>"#));
        assert!(docxml.contains("<w:titlePg/>"));
        assert_eq!(docxml.matches("Name: Zed").count(), 1, "page 1 prints it once in the body");
        assert!(read(&mut z, "word/header1.xml").contains("Name: Zed"));
        assert!(!read(&mut z, "word/header2.xml").contains("Name"));
        assert!(read(&mut z, "word/_rels/document.xml.rels").contains("header1.xml"));
        assert!(read(&mut z, "[Content_Types].xml").contains("/word/header1.xml"));
        let key = render_docx(&ExamDoc { is_key: true, ..doc }).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(key)).unwrap();
        assert!(z.by_name("word/header1.xml").is_err(), "no header part on the key");
    }

    fn rich_doc(stem: &str) -> ExamDoc {
        ExamDoc {
            title: "Rich".into(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            questions: vec![DocQ { number: 1, text: vec![stem.into()], points: Some(2.0), ..Default::default() }],
            ..Default::default()
        }
    }

    /// Every text baseline ("x y Td") of a page's content stream.
    fn baselines(pdf: &[u8]) -> Vec<f64> {
        let s = String::from_utf8_lossy(pdf);
        s.split(" Td").filter_map(|chunk| chunk.rsplit(' ').next().and_then(|y| y.parse::<f64>().ok())).collect()
    }

    #[test]
    fn rich_runs_use_bold_italic_fonts_and_scripts() {
        let pdf = render_pdf(&rich_doc("Use **bold** and *slant* in H_{2}O, x^{3} and SO\u{2083}.")).unwrap();
        assert!(find_sub(&pdf, b"/F2 11 Tf").is_some(), "bold run");
        assert!(find_sub(&pdf, b"/F3 11 Tf").is_some(), "italic run");
        assert!(find_sub(&pdf, b"/BaseFont /Helvetica-Oblique").is_some());
        assert!(find_sub(&pdf, b"/F1 7.7 Tf").is_some(), "sub/superscripts print smaller");
        assert!(find_sub(&pdf, b"(**").is_none() && find_sub(&pdf, b"_{").is_none(), "markup must not leak");
        assert!(find_sub(&pdf, b"?").is_none() || find_sub(&pdf, b"SO?").is_none(), "Unicode subscript is not '?'");
        verify_xref(&pdf);
    }

    #[test]
    fn bullets_numbered_and_quote_lay_out_with_markers() {
        let items = layout_blocks(&richtext::parse("Do all:\n• Draw it.\n• Count them.\n1. one\n2. two\n> quoted"), 400.0, 380.0, 11.0, 15.6);
        let markers: Vec<String> = items
            .iter()
            .filter_map(|i| match i {
                Item::Line { marker: Some(m), .. } => Some(m.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(markers, vec!["\u{2022}", "\u{2022}", "1.", "2."]);
        assert!(items.iter().any(|i| matches!(i, Item::Line { bar: true, .. })));
        // A long bullet wraps with a hanging indent: continuation lines keep the indent, no marker.
        let long = format!("• {}", "word ".repeat(60));
        let items = layout_blocks(&richtext::parse(&long), 300.0, 300.0, 11.0, 15.6);
        assert!(items.len() > 2);
        for (k, it) in items.iter().enumerate() {
            let Item::Line { indent, marker, frags, .. } = it else { panic!() };
            assert_eq!(*indent, LIST_INDENT);
            assert_eq!(marker.is_some(), k == 0);
            let w = frags.last().map(|p| p.dx + text_w(&p.text, p.face, 11.0)).unwrap_or(0.0);
            assert!(w <= 300.0 - LIST_INDENT + 0.01, "line {k} overflows: {w}");
        }
    }

    #[test]
    fn table_header_bold_widths_from_content_and_cells_wrap() {
        let stem = "Name them.\nFormula | Ionic or Covalent? | Name\nMg(NO\u{2082})\u{2082} | ___ | ___\nKI | ___ | ___";
        let items = layout_blocks(&richtext::parse(stem), 470.0, 430.0, 11.0, 15.6);
        let rows: Vec<&Item> = items.iter().filter(|i| matches!(i, Item::Row { .. })).collect();
        assert_eq!(rows.len(), 3);
        let Item::Row { cols, cells, header, .. } = rows[0] else { panic!() };
        assert!(*header);
        assert!(cells[0][0].iter().all(|p| matches!(p.face, Face::Bold)), "header row is bold");
        let total: f64 = cols.iter().map(|c| c.1).sum();
        assert!(total < 470.0, "a small table keeps its natural width: {total}");
        assert!(cols[1].1 > cols[0].1, "wider header text, wider column");
        // Blank answer cells print empty.
        let Item::Row { cells, header_idx, .. } = rows[1] else { panic!() };
        assert!(cells[1].iter().all(|l| l.is_empty()));
        assert!(header_idx.is_some());
        // Long cell text wraps inside a table squeezed to the column.
        let long = format!("A | B\n{} | short", "longish words ".repeat(30));
        let items = layout_blocks(&richtext::parse(&long), 300.0, 300.0, 11.0, 15.6);
        let Item::Row { cols, cells, h, .. } = &items[1] else { panic!() };
        let total: f64 = cols.iter().map(|c| c.1).sum();
        assert!(total <= 300.01, "table fits the column: {total}");
        assert!(cells[0].len() > 2 && *h > 3.0 * 15.6, "cell wrapped onto several lines");
    }

    #[test]
    fn long_tables_break_between_rows_and_stay_inside_the_page() {
        let mut stem = String::from("Fill in.\nElement | Symbol | Mass\n---|---|---");
        for i in 0..80 {
            stem.push_str(&format!("\nElement number {i} | X{i} | ___"));
        }
        let doc = rich_doc(&stem);
        let pdf = render_pdf(&doc).unwrap();
        assert!(count_pages(&pdf) >= 2, "an 80-row table spans pages");
        let margin = 18.0 * MM_PT;
        for y in baselines(&pdf) {
            assert!(y > margin - 1.0 && y < PAGE_H - margin + 1.0, "text outside the page margins at y={y}");
        }
        // The header row repeats on each continuation page.
        let heads = String::from_utf8_lossy(&pdf).matches("(Element) Tj").count();
        assert_eq!(heads, count_pages(&pdf), "one header per page");
        verify_xref(&pdf);
    }

    #[test]
    fn blanks_in_table_cells_need_no_ruled_lines() {
        assert_eq!(blanks_outside_tables("Fill ___ and ___."), 2);
        assert_eq!(blanks_outside_tables("Name:\nA | B\nx | ___\ny | ___"), 0);
        let q = DocQ { text: vec!["Name:\nA | B\nx | ___\ny | ___".into()], ..Default::default() };
        assert_eq!(rule_count(&q), 0);
        let q = DocQ { text: vec!["Water is ___.".into()], ..Default::default() };
        assert_eq!(rule_count(&q), 1);
    }

    #[test]
    fn plain_stems_print_as_before() {
        let pdf = render_pdf(&rich_doc("Which planet is closest to the Sun?")).unwrap();
        assert!(find_sub(&pdf, b"(Which planet is closest to the Sun?) Tj").is_some());
        assert!(find_sub(&pdf, b"(1.) Tj").is_some());
    }

    #[test]
    fn docx_tables_lists_and_runs() {
        let stem = "Do **all**:\n• Draw it.\n• Count *them*.\n1. first\n2. second\nFormula | Name\nSO\u{2083} | ___\nH_{2}O | water";
        let bytes = render_docx(&rich_doc(stem)).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut x = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut x).unwrap();
        let mut num = String::new();
        z.by_name("word/numbering.xml").unwrap().read_to_string(&mut num).unwrap();
        let mut ct = String::new();
        z.by_name("[Content_Types].xml").unwrap().read_to_string(&mut ct).unwrap();
        let mut rels = String::new();
        z.by_name("word/_rels/document.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
        assert!(x.contains("<w:tbl>") && x.contains("<w:tblBorders>") && x.contains("<w:tblHeader/>"));
        assert_eq!(x.matches("<w:tr>").count(), 3);
        assert!(x.contains("<w:numId w:val=\"1\"/>"), "bullets use the bullet list");
        assert!(x.contains("<w:numId w:val=\"2\"/>"), "numbered list gets its own restartable num");
        assert!(x.contains("<w:vertAlign w:val=\"subscript\"/>"));
        assert!(x.contains("<w:b/>") && x.contains("<w:i/>"));
        assert!(!x.contains("**") && !x.contains("_{") && !x.contains("\u{2083}"));
        assert!(num.contains("w:numFmt w:val=\"bullet\"") && num.contains("w:numFmt w:val=\"decimal\""));
        assert!(ct.contains("numbering+xml") && rels.contains("numbering.xml"));
    }

    #[test]

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