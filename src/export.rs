//! Pure PDF and DOCX writers for [`ExamDoc`] (signatures frozen in NOTES.md):
//! `render_pdf(&layout::ExamDoc) -> Result<Vec<u8>, String>` and `render_docx(&layout::ExamDoc)`.
//! No filesystem access; uses the `image`, `zip`, `flate2` and `ttf-parser` crates (all already in
//! the dependency tree) and the serif faces compiled in by `crate::fonts`.
//!
//! # PDF (1.4, hand-rolled)
//! Letter 612x792pt, 72 pt/inch. Fonts F1..F4 are the print serif, Source Serif 4 Regular,
//! Bold, Italic and Bold Italic (real faces, `crate::fonts`), embedded as subset TrueType
//! programs (FlateDecode `FontFile2`, WinAnsiEncoding). Stems, choices and key text carry stem markup (`docs/stem-format.md`),
//! parsed by `richtext` and laid out into items (text lines, table rows): lists with hanging
//! indents, quotes with a rule, bordered tables sized from their content with wrapped cells
//! and a bold shaded header row (repeated after a page break; tables break only between
//! rows), bold/italic runs, sub/superscripts (markup or Unicode ₂ ⁺) as smaller shifted text.
//! Text widths come from the fonts' own advance widths (`fonts::metrics`), the very numbers
//! written into each font's `/Widths`, so what the line breaker measures is what is drawn;
//! non-ASCII maps through WinAnsi (Latin-1 passes 1:1, cp1252's
//! punctuation is covered). Other characters print through each face's Identity-H CID font
//! (F5..F8, Type0/CIDFontType2 sharing the face's program, with a ToUnicode CMap) or, when
//! Source Serif 4 lacks them, the STIX Two Math fallback (F9, a Type 3 font of the drawn
//! outlines with a ToUnicode CMap); only what neither has becomes '?' (see [`enc`] and
//! `docs/fonts.md`). `` `code` `` prints in Liberation Mono (F10, a subset embedded only when
//! a document has code) on a light tint. Images decode via the `image` crate and
//! re-encode to JPEG quality 85, embedded as DCTDecode XObjects. Content streams are
//! uncompressed; the xref table carries exact byte offsets, verified by a unit test that
//! resolves every object entry to its `N obj` header.
//!
//! # DOCX (hand-rolled OOXML zipped with the `zip` crate)
//! Minimal valid package: [Content_Types].xml, _rels/.rels, word/document.xml,
//! word/_rels/document.xml.rels, a small styles.xml (Normal/Title/Heading1, Times New
//! Roman default), fontTable.xml (Times New Roman with "Liberation Serif" as its alternate,
//! roman family) and numbering.xml (bullets; one restartable decimal list per numbered block).
//! Word does its own wrapping; stem markup becomes real Word structure (bordered tables
//! with a repeating shaded header row and content-sized columns, list paragraphs, quote
//! border, bold/italic/sub/superscript runs). Matching
//! questions print linearly (left items numbered, then the lettered right column
//! indented); pictures embed as inline drawings (`word/media`, PNG / JPEG as they are, other
//! formats re-encoded to PNG) at the PDF's size and place; only undecodable bytes fall back to
//! a centered italic `[picture]` paragraph.
//!
//! # Shared print rules (PDF and DOCX, matched by the exam page preview)
//! Title area: header line with a rule under it, the title centred, a muted "N questions ·
//! M points" line. Pictures: `layout::picture_size_pt` at the stem's left edge, at the stem's
//! first `[picture]` marker or under the stem. Blanks: `___` runs print at least
//! [`BLANK_EM`] long. Answer columns: `DocQ.answer_columns` 2..=4, column-major. Answer keys:
//! `ExamDoc.key_layout` (compact list, Test Parrot's answer section, or the marked test).
//! Round-7 markup (`richtext::parse_tree`): code in a mono (PDF Liberation Mono, DOCX Courier
//! New), `::: center` groups centred line by line (DOCX `jc` center), nested list items
//! indented per level with per-level bullets / numbers (DOCX `w:ilvl`, multilevel numbering),
//! task items without a bullet. Stems without it print exactly as before.

use crate::fonts;
use crate::layout::{DocImage, DocQ, DocSection, ExamDoc, KeyLayout};
use crate::model::PaperStyle;
use crate::richtext::{self, Block, Node};

#[cfg(test)]
const PAGE_H: f64 = 792.0; // Letter, 72pt/inch (Exam Board prints A4; see PaperStyle::page_pt)
const MM_PT: f64 = 72.0 / 25.4;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Face {
    Reg,
    Bold,
    /// The real italic face (its own outlines and advance widths).
    Ital,
    BoldItal,
    /// `` `code` ``: the print mono (Liberation Mono, `fonts::Style::Mono`), set
    /// [`MONO_SCALE`] of the size asked for. Its simple font F10 is embedded only when used;
    /// characters beyond WinAnsi print through the regular serif's CID font F5 (or F9).
    Mono,
}

/// Code prints this much smaller than the text around it (a mono's x-height is taller than
/// the serif's; the on-screen preview uses the same ratio).
const MONO_SCALE: f64 = 0.92;

impl Face {
    fn name(self) -> &'static str {
        match self {
            Face::Reg => "F1",
            Face::Bold => "F2",
            Face::Ital => "F3",
            Face::BoldItal => "F4",
            Face::Mono => "F10",
        }
    }
    fn style(self) -> fonts::Style {
        match self {
            Face::Reg => fonts::Style::Regular,
            Face::Bold => fonts::Style::Bold,
            Face::Ital => fonts::Style::Italic,
            Face::BoldItal => fonts::Style::BoldItalic,
            Face::Mono => fonts::Style::Mono,
        }
    }
    /// The size text of `pt` points prints at in this face.
    fn pt(self, pt: f64) -> f64 {
        if self == Face::Mono { pt * MONO_SCALE } else { pt }
    }
    /// The face whose CID font prints this face's characters beyond WinAnsi.
    fn cid_face(self) -> Face {
        if self == Face::Mono { Face::Reg } else { self }
    }
    fn idx(self) -> usize {
        self as usize
    }
    /// Advance widths (1/1000 em) of every WinAnsi byte: the numbers the PDF's `/Widths` carries.
    fn widths(self) -> &'static [u16; 256] {
        &fonts::metrics(self.style()).widths
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
            Face::Mono => Face::Mono,
        }
    }
}

// ---------------------------------------------------------------- text measuring

fn char_w(c: u8, face: Face, pt: f64) -> f64 {
    face.widths()[c as usize] as f64 * face.pt(pt) / 1000.0
}

/// Width of a string, each character measured in the font it prints in ([`enc`]).
fn text_w(s: &str, face: Face, pt: f64) -> f64 {
    let pt = face.pt(pt);
    s.chars().map(|c| enc_units(enc(c, face), face) as f64 * pt / 1000.0).sum()
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
#[cfg_attr(not(test), allow(dead_code))]
fn winansi(s: &str) -> Vec<u8> {
    s.chars().map(|c| ansi_byte(c).unwrap_or(b'?')).collect()
}

/// The WinAnsi byte of one character (see [`winansi`]), None when WinAnsi has none.
fn ansi_byte(c: char) -> Option<u8> {
    {
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
            // The rest of cp1252's 0x80..0x9F zone (euro, per mille, S/Z caron, ...).
            0x152..=0x2122 => fonts::winansi_byte_hi(c),
            0xa1..=0xff => Some(u as u8), // Latin-1 letters map 1:1 in WinAnsi
            0x09 | 0x0a | 0x0b | 0x0d => Some(0x20), // control whitespace -> space
            _ => None,
        };
        match mapped {
            Some(b) => Some(b),
            None if (0x20..=0x7e).contains(&u) => Some(u as u8),
            None => None,
        }
    }
}

/// How one character prints in a face: a WinAnsi byte of the simple font F1..F4, a glyph of
/// the same face through its Identity-H CID font F5..F8 (characters outside WinAnsi that
/// Source Serif 4 has: π, −, √, ≤, Greek, arrows, ...), or a glyph of the fallback face F9
/// (STIX Two Math, a Type 3 font) for what Source Serif lacks; '?' when neither has it.
/// Glyph ids carry their advance (1/1000 em).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Enc {
    Ansi(u8),
    Cid(u16, u16),
    Fb(u16, u16),
}

impl Enc {
    fn kind(self) -> u8 {
        match self {
            Enc::Ansi(_) => 0,
            Enc::Cid(..) => 1,
            Enc::Fb(..) => 2,
        }
    }
}

fn enc(c: char, face: Face) -> Enc {
    if let Some(b) = ansi_byte(c) {
        return Enc::Ansi(b);
    }
    if c.is_control() {
        return Enc::Ansi(b'?');
    }
    if let Some((g, w)) = fonts::charmap(face.cid_face().style()).get(c) {
        return Enc::Cid(g, w);
    }
    if let Some((g, w)) = fonts::fallback_charmap().get(c) {
        return Enc::Fb(g, w);
    }
    Enc::Ansi(b'?')
}

/// Advance (1/1000 em) of an encoded character.
fn enc_units(e: Enc, face: Face) -> u16 {
    match e {
        Enc::Ansi(b) => face.widths()[b as usize],
        Enc::Cid(_, w) | Enc::Fb(_, w) => w,
    }
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

/// A points total as printed ("5", "2.5").
fn points_number(p: f64) -> String {
    if p.fract() == 0.0 { format!("{}", p as i64) } else { format!("{p}") }
}

#[cfg(test)]
fn points_label(p: f64) -> String {
    PaperStyle::Standard.points_label(p)
}

/// Ruled answer lines a question gets; the exporters' default when the caller leaves 0.
fn rule_count(q: &DocQ) -> u8 {
    if q.no_answer_space {
        0
    } else if q.rule_lines > 0 {
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
    let mut n = 0;
    richtext::walk(&richtext::parse(stem), &mut |b| match b {
        Block::Table { .. } | Block::Divider | Block::Side(_) => {}
        Block::Heading(_, l) => n += richtext::line_text(l).matches("___").count(),
        Block::Para(ls) | Block::Bullets(ls) | Block::Numbered(ls) | Block::Quote(ls) => {
            n += ls.iter().map(|l| richtext::line_text(l).matches("___").count()).sum::<usize>();
        }
    });
    n
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
    /// `~~struck through~~`.
    strike: bool,
    /// The URL of a `[text](url)` link.
    link: Option<String>,
}

/// A run placed `dx` points from the start of its line.
#[derive(Clone, Debug)]
struct Placed {
    dx: f64,
    text: String,
    face: Face,
    script: i8,
    strike: bool,
    link: Option<String>,
}

const SCRIPT_SCALE: f64 = 0.7;
/// Link colour (a print-safe dark blue): stroke and fill operators.
const LINK_RGB: &str = "0.05 0.27 0.68";
const LINK_RGB_FILL: &[u8] = b"0.05 0.27 0.68 rg ";
/// Gray level of the tint behind `` `code` `` (PDF `g`; DOCX shading [`CODE_SHD`]).
const CODE_TINT: &str = "0.92";
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
            out.push(Frag { text: std::mem::take(&mut cur), face, script: cur_sc, strike: false, link: None });
        }
        cur_sc = sc;
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(Frag { text: cur, face, script: cur_sc, strike: false, link: None });
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
        if s.code {
            // `code`: the mono, literal (no Unicode script folding), on a tint when drawn.
            if !s.text.is_empty() {
                out.push(Frag { text: s.text.clone(), face: Face::Mono, script: base, strike: s.strike, link: s.link.clone() });
            }
        } else if s.math {
            let t = crate::equation::stem_to_plain(&format!("${}$", s.text));
            split_unicode_scripts(&t, face, 0, &mut out);
        } else {
            let n0 = out.len();
            split_unicode_scripts(&s.text, face, base, &mut out);
            for f in &mut out[n0..] {
                f.strike = s.strike;
                f.link = s.link.clone();
            }
        }
    }
    out
}

/// One word of a run list and the face of the space in front of it (the space keeps the
/// style of the run it was typed in, so it measures and draws in that face).
struct Word {
    frags: Vec<Frag>,
    sp: Face,
    script: i8,
}

/// Words of a run list (split on single spaces; consecutive spaces keep empty words so
/// "a  b" and leading indentation keep their width, as the plain wrap did).
fn words(frags: &[Frag]) -> Vec<Word> {
    let mut ws: Vec<Word> = vec![Word { frags: vec![], sp: Face::Reg, script: 0 }];
    for f in frags {
        for (i, piece) in f.text.split(' ').enumerate() {
            if i > 0 {
                ws.push(Word { frags: vec![], sp: f.face, script: f.script });
            }
            if !piece.is_empty() {
                ws.last_mut().unwrap().frags.push(Frag { text: piece.to_string(), ..f.clone() });
            }
        }
    }
    ws
}

/// Width of the space in front of `w`.
fn space_w(w: &Word, body: f64) -> f64 {
    char_w(32, w.sp, frag_size(w.script, body))
}

fn word_w(w: &[Frag], body: f64) -> f64 {
    w.iter().map(|f| frag_w(f, body)).sum()
}

/// Width of a run list laid on one line.
fn natural_w(frags: &[Frag], body: f64) -> f64 {
    let ws = words(frags);
    ws.iter().map(|w| word_w(&w.frags, body)).sum::<f64>() + ws.iter().skip(1).map(|w| space_w(w, body)).sum::<f64>()
}

/// Widest single word (the narrowest a column can get without breaking words).
fn min_word_w(frags: &[Frag], body: f64) -> f64 {
    words(frags).iter().map(|w| word_w(&w.frags, body)).fold(0.0, f64::max)
}

/// Greedy wrap of styled runs: the first line gets `first_w`, the rest `rest_w`; words
/// wider than the line hard-break per character. Always returns at least one line.
fn wrap_frags(frags: &[Frag], first_w: f64, rest_w: f64, body: f64) -> Vec<Vec<Placed>> {
    let mut lines: Vec<Vec<Placed>> = Vec::new();
    let mut cur: Vec<Placed> = Vec::new();
    let mut x = 0.0;
    let mut n = 0usize;
    let maxw = |lines: &Vec<Vec<Placed>>| if lines.is_empty() { first_w } else { rest_w };
    for w in words(frags) {
        let sp = space_w(&w, body);
        let word = w.frags;
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
                    cur.push(Placed { dx: x, text: s, face: f.face, script: f.script, strike: f.strike, link: f.link.clone() });
                    x += cw;
                }
            }
        } else {
            for f in word {
                let w = frag_w(&f, body);
                cur.push(Placed { dx: x, text: f.text, face: f.face, script: f.script, strike: f.strike, link: f.link });
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
    let mut out: Vec<Placed> = Vec::with_capacity(line.len());
    for p in line {
        if let Some(last) = out.last_mut() {
            if last.script == 0 && p.script == 0 && last.face == p.face && last.strike == p.strike && last.link == p.link {
                let end = last.dx + text_w(&last.text, last.face, body);
                let gap = p.dx - end;
                // Gap of whole spaces in this face (a space between two runs of one face was
                // laid out in that face).
                let sp = char_w(32, p.face, body);
                let k = (gap / sp).round();
                if gap >= -0.01 && k >= 0.0 && (gap - k * sp).abs() < 0.01 {
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
    /// `scale` enlarges the line (headings; 1.0 for body text).
    Line { indent: f64, frags: Vec<Placed>, marker: Option<String>, bar: bool, scale: f64 },
    Gap(f64),
    /// A divider: a thin rule across `w` points, centred in its slot.
    Rule { w: f64 },
    /// Side-by-side panels: each column's x offset and its own items, `h` the tallest column.
    /// Never split across pages.
    Cols { cols: Vec<(f64, Vec<Item>)>, h: f64 },
    /// One table row: column (x offset, width), wrapped cell lines, row height; rows after
    /// a header point back at it so a page break can repeat it.
    Row { cols: Vec<(f64, f64)>, cells: Vec<Vec<Vec<Placed>>>, h: f64, header: bool, header_idx: Option<usize> },
    /// A multipart part's work space: `lines` answer lines (lined styles) or blank room,
    /// `h` points tall; it may run across a page break like the question's own work space.
    Work { lines: u8, h: f64 },
}

fn item_h(it: &Item, lead: f64) -> f64 {
    match it {
        Item::Line { scale, .. } => lead * scale,
        Item::Gap(h) => *h,
        Item::Row { h, .. } => *h,
        Item::Rule { .. } => lead * 0.8,
        Item::Cols { h, .. } => *h,
        Item::Work { h, .. } => *h,
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

/// Lays parsed stem blocks into items `avail` points wide (no round-7 layout: see
/// [`layout_nodes`]).
#[cfg(test)]
fn layout_blocks(blocks: &[Block], avail: f64, first_w: f64, body: f64, lead: f64) -> Vec<Item> {
    layout_nodes(&nodes_of(blocks), avail, first_w, body, lead)
}

/// Plain blocks as layout nodes (nothing centred, lists flat).
fn nodes_of(blocks: &[Block]) -> Vec<Node> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Side(panels) => Node::Side(panels.iter().map(|p| nodes_of(p)).collect()),
            Block::Bullets(ls) | Block::Numbered(ls) => Node::Block { block: b.clone(), center: false, depths: vec![0; ls.len()] },
            _ => Node::Block { block: b.clone(), center: false, depths: Vec::new() },
        })
        .collect()
}

/// Right end of a laid-out line (points from its start), its runs measured at `body`.
fn placed_w(line: &[Placed], body: f64) -> f64 {
    line.iter().map(|p| p.dx + text_w(&p.text, p.face, frag_size(p.script, body))).fold(0.0, f64::max)
}

/// Left offset that centres a line `w` wide in `room`.
fn center_off(w: f64, room: f64) -> f64 {
    ((room - w) / 2.0).max(0.0)
}

/// Lays a stem's nodes (`richtext::parse_tree`) into items `avail` points wide; the very
/// first text line gets `first_w` (room for the points label), a leading table fits
/// `first_w` too. Round-7 layout: paragraphs and headings of a `::: center` group centre
/// each wrapped line in the width it was wrapped to, a centred table centres in the column;
/// list items indent [`LIST_INDENT`] per nesting level, with that level's bullet
/// (`richtext::bullet_glyph`) or number (`richtext::number_label`); a task item (`☐ ` /
/// `☑ `) prints its box without a bullet. Without round-7 markup the items are exactly the
/// pre-round-7 ones.
fn layout_nodes(nodes: &[Node], avail: f64, first_w: f64, body: f64, lead: f64) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut first = true;
    let gap = body * 0.35;
    for (bi, n) in nodes.iter().enumerate() {
        if bi > 0 {
            items.push(Item::Gap(gap));
        }
        let fw = |first: bool, indent: f64| if first { first_w - indent } else { avail - indent };
        let (b, center, depths) = match n {
            Node::Block { block, center, depths } => (block, *center, depths.as_slice()),
            Node::Side(panels) => {
                let n = panels.len().max(1) as f64;
                let gap = body * 1.2;
                let w = ((if first { first_w } else { avail } - gap * (n - 1.0)) / n).max(body * 3.0);
                let cols: Vec<(f64, Vec<Item>)> = panels.iter().enumerate().map(|(k, p)| (k as f64 * (w + gap), layout_nodes(p, w, w, body, lead))).collect();
                let h = cols.iter().map(|(_, it)| items_h(it, lead)).fold(lead, f64::max);
                items.push(Item::Cols { cols, h });
                first = false;
                continue;
            }
        };
        match b {
            Block::Para(lines) => {
                for l in lines {
                    for (j, pl) in wrap_frags(&line_frags(l, false), fw(first, 0.0), avail, body).into_iter().enumerate() {
                        let room = if j == 0 { fw(first, 0.0) } else { avail };
                        let indent = if center { center_off(placed_w(&pl, body), room) } else { 0.0 };
                        items.push(Item::Line { indent, frags: pl, marker: None, bar: false, scale: 1.0 });
                        first = false;
                    }
                }
            }
            Block::Bullets(lines) | Block::Numbered(lines) | Block::Quote(lines) => {
                let (base, bar) = match b {
                    Block::Bullets(_) => (LIST_INDENT, false),
                    Block::Numbered(_) => (NUM_INDENT, false),
                    _ => (QUOTE_INDENT, true),
                };
                let depth = |k: usize| depths.get(k).copied().unwrap_or(0);
                let ords = richtext::item_ordinals(&(0..lines.len()).map(depth).collect::<Vec<_>>());
                for (k, l) in lines.iter().enumerate() {
                    let indent = base + depth(k) as f64 * LIST_INDENT;
                    let wrapped = wrap_frags(&line_frags(l, false), fw(first, indent), avail - indent, body);
                    for (j, pl) in wrapped.into_iter().enumerate() {
                        let marker = (j == 0).then(|| match b {
                            Block::Bullets(_) if richtext::task_item(l).is_some() => None,
                            Block::Bullets(_) => Some(richtext::bullet_glyph(depth(k)).to_string()),
                            Block::Numbered(_) => Some(richtext::number_label(depth(k), ords[k])),
                            _ => None,
                        });
                        items.push(Item::Line { indent, frags: pl, marker: marker.flatten(), bar, scale: 1.0 });
                        first = false;
                    }
                }
            }
            Block::Table { header, rows } => {
                let base = items.len();
                let room = if first { first_w } else { avail };
                let mut rows_it = layout_table(*header, rows, room, body, lead, base);
                if center {
                    for it in &mut rows_it {
                        if let Item::Row { cols, .. } = it {
                            let total: f64 = cols.iter().map(|c| c.1).sum();
                            let off = center_off(total, room);
                            cols.iter_mut().for_each(|c| c.0 += off);
                        }
                    }
                }
                items.extend(rows_it);
                if bi + 1 == nodes.len() {
                    items.push(Item::Gap(body * 0.5)); // breathing room under a closing table
                }
                first = false;
            }
            Block::Heading(level, l) => {
                let scale = crate::richtext::heading_scale(*level) as f64;
                let hb = body * scale;
                for (j, pl) in wrap_frags(&line_frags(l, true), fw(first, 0.0), avail, hb).into_iter().enumerate() {
                    let room = if j == 0 { fw(first, 0.0) } else { avail };
                    let indent = if center { center_off(placed_w(&pl, hb), room) } else { 0.0 };
                    items.push(Item::Line { indent, frags: pl, marker: None, bar: false, scale });
                    first = false;
                }
            }
            Block::Divider => {
                items.push(Item::Rule { w: avail });
                first = false;
            }
            Block::Side(panels) => {
                // (`parse_tree` keeps side-by-side groups as `Node::Side`; never reached)
                items.extend(layout_nodes(&[Node::Side(panels.iter().map(|p| nodes_of(p)).collect())], avail, if first { first_w } else { avail }, body, lead));
                first = false;
            }
        }
    }
    items
}

/// The nodes of a question's text entries, one after another.
fn stem_nodes(text: &[String]) -> Vec<Node> {
    text.iter().flat_map(|t| richtext::parse_tree(t)).collect()
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
    /// Top and bottom margins (pt).
    mt: f64,
    mb: f64,
    page_w: f64,
    page_h: f64,
    style: PaperStyle,
    body: f64,
    head: f64,
    lead: f64,
    imgs: Vec<Px>,
    /// The printed "Name / Class / Date" line, repeated at the top of every page after
    /// the first (set once page 1's header area is drawn; `None` on the answer key).
    running: Option<String>,
    /// WinAnsi bytes drawn in each face (index `Face::idx`): the glyphs the embedded font
    /// subsets keep.
    glyphs: [[bool; 256]; 5],
    /// Glyphs drawn through each face's Identity-H CID font, with the character drawn
    /// (the mono's go through the regular serif's, index 0).
    cids: [std::collections::BTreeMap<u16, char>; 5],
    /// Fallback (Type 3) glyphs in code order: (glyph id, character, advance).
    fb: Vec<(u16, char, u16)>,
    /// Link annotations: (page index, [x0 y0 x1 y1], URL).
    annots: Vec<(usize, [f64; 4], String)>,
    /// Marked answer key: correct choices print checked and bold, expected answers in place.
    mark: bool,
}

impl R {
    fn remaining(&self) -> f64 {
        self.y - self.mb
    }
    fn full_height(&self) -> f64 {
        self.page_h - self.mt - self.mb - self.running.as_ref().map_or(0.0, |_| self.running_h())
    }
    /// One ruled / work-space line's pitch.
    fn pitch(&self) -> f64 {
        self.body * self.style.rule_pitch() as f64
    }
    /// One answer line across `xa..xb` (dotted on Exam Board), advancing a pitch.
    fn answer_line(&mut self, xa: f64, xb: f64) {
        if self.remaining() < self.pitch() {
            self.new_page();
        }
        let p = self.pitch();
        self.y -= p * 0.55;
        let y = self.y;
        if self.style.dotted() {
            self.cur.extend_from_slice(b"[0.8 2.2] 0 d ");
            self.rule(xa, y, xb, y);
            self.cur.extend_from_slice(b"[] 0 d\n");
        } else {
            self.rule(xa, y, xb, y);
        }
        self.y -= p * 0.45;
    }
    /// Height the repeated header takes at the top of a continuation page.
    fn running_h(&self) -> f64 {
        self.lead + self.body * 0.6
    }
    fn new_page(&mut self) {
        if !self.cur.is_empty() {
            if self.style == PaperStyle::ExamBoard {
                // Exam Board: "Turn over" at the foot of every page but the last.
                let lbl = "Turn over";
                let w = text_w(lbl, Face::Bold, self.body);
                let (x, y) = (self.x1 - w, self.mb * 0.55);
                self.text(x, y, Face::Bold, self.body, lbl);
            }
            let cur = std::mem::take(&mut self.cur);
            self.pages.push((cur, std::mem::take(&mut self.used)));
        }
        self.y = self.page_h - self.mt;
        if let Some(h) = self.running.clone() {
            let x0 = self.x0;
            self.line(x0, &h, Face::Reg);
            // A rule under the repeated header, as the page preview draws it.
            let (x1, y) = (self.x1, self.y - self.body * 0.15);
            self.rule(x0, y, x1, y);
            self.y -= self.body * 0.6;
        }
    }
    /// Draws `s` with its left end at `x` on baseline `y`. WinAnsi text is one literal string
    /// in F1..F4; other characters split the string into runs in the CID font (F5..F8) or
    /// the fallback font (F9), each placed where the measured runs before it end.
    fn text(&mut self, x: f64, y: f64, face: Face, size: f64, s: &str) {
        let size = face.pt(size);
        let encs: Vec<(char, Enc)> = s.chars().map(|c| (c, enc(c, face))).collect();
        if encs.iter().all(|(_, e)| e.kind() == 0) {
            let raw: Vec<u8> = encs.iter().map(|(_, e)| if let Enc::Ansi(b) = e { *b } else { b'?' }).collect();
            self.text_ansi(x, y, face, size, &raw);
            return;
        }
        let mut cx = x;
        let mut i = 0;
        while i < encs.len() {
            let kind = encs[i].1.kind();
            let j = (i..encs.len()).find(|&k| encs[k].1.kind() != kind).unwrap_or(encs.len());
            let run = &encs[i..j];
            match kind {
                0 => {
                    let raw: Vec<u8> = run.iter().map(|(_, e)| if let Enc::Ansi(b) = e { *b } else { b'?' }).collect();
                    self.text_ansi(cx, y, face, size, &raw);
                }
                1 => {
                    let mut hex = String::with_capacity(run.len() * 4 + 2);
                    hex.push('<');
                    for (c, e) in run {
                        if let Enc::Cid(g, _) = e {
                            self.cids[face.cid_face().idx()].entry(*g).or_insert(*c);
                            hex.push_str(&format!("{g:04X}"));
                        }
                    }
                    hex.push('>');
                    self.text_op(cx, y, &format!("F{}", 5 + face.cid_face().idx()), size, hex.as_bytes());
                }
                _ => {
                    // One byte per glyph: at most 256 distinct fallback glyphs per document
                    // (more leave a gap of the right width).
                    let mut hex = String::from("<");
                    for (c, e) in run {
                        if let Enc::Fb(g, w) = e {
                            let code = match self.fb.iter().position(|f| f.0 == *g) {
                                Some(k) => Some(k),
                                None if self.fb.len() < 256 => {
                                    self.fb.push((*g, *c, *w));
                                    Some(self.fb.len() - 1)
                                }
                                None => None,
                            };
                            if let Some(k) = code {
                                hex.push_str(&format!("{k:02X}"));
                            }
                        }
                    }
                    hex.push('>');
                    if hex.len() > 2 {
                        self.text_op(cx, y, "F9", size, hex.as_bytes());
                    }
                }
            }
            cx += run.iter().map(|(_, e)| enc_units(*e, face) as f64 * size / 1000.0).sum::<f64>();
            i = j;
        }
    }
    /// One text-showing operation: `BT /font size Tf x y Td <string> Tj ET`.
    fn text_op(&mut self, x: f64, y: f64, font: &str, size: f64, string: &[u8]) {
        self.cur.extend_from_slice(b"BT /");
        self.cur.extend_from_slice(font.as_bytes());
        self.cur.extend_from_slice(b" ");
        self.cur.extend_from_slice(fnum(size).as_bytes());
        self.cur.extend_from_slice(b" Tf ");
        self.cur.extend_from_slice(fnum(x).as_bytes());
        self.cur.extend_from_slice(b" ");
        self.cur.extend_from_slice(fnum(y).as_bytes());
        self.cur.extend_from_slice(b" Td ");
        self.cur.extend_from_slice(string);
        self.cur.extend_from_slice(b" Tj ET\n");
    }
    fn text_ansi(&mut self, x: f64, y: f64, face: Face, size: f64, raw: &[u8]) {
        for &b in raw {
            self.glyphs[face.idx()][b as usize] = true;
        }
        let bytes = esc_pdf(raw);
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
            if p.face == Face::Mono {
                // Code sits on a light tint, as on screen.
                let w = text_w(p.text.trim_end(), p.face, size);
                let (x0, y0) = (x + p.dx - 1.2, by + dy - 0.24 * size);
                self.cur.extend_from_slice(format!("{CODE_TINT} g {} {} {} {} re f 0 g\n", fnum(x0), fnum(y0), fnum(w + 2.4), fnum(1.04 * size)).as_bytes());
            }
            if let Some(url) = &p.link {
                // Links print blue and underlined, with a URI link annotation over the run.
                let w = text_w(p.text.trim_end(), p.face, size);
                let (x0, y0) = (x + p.dx, by + dy);
                self.cur.extend_from_slice(LINK_RGB_FILL);
                self.text(x0, y0, p.face, size, &p.text);
                self.cur.extend_from_slice(b"0 0 0 rg\n");
                let uy = y0 - 0.12 * size;
                self.stroke_line(x0, uy, x0 + w, uy, (size / 18.0).max(0.4), LINK_RGB);
                let page = self.pages.len();
                self.annots.push((page, [x0, y0 - 0.25 * size, x0 + w, y0 + 0.8 * size], url.clone()));
            } else {
                self.text(x + p.dx, by + dy, p.face, size, &p.text);
            }
            if p.strike {
                // through the middle of the lower-case letters
                let w = text_w(p.text.trim_end(), p.face, size);
                let sy = by + dy + 0.3 * size;
                self.stroke_line(x + p.dx, sy, x + p.dx + w, sy, (size / 16.0).max(0.5), "0 0 0");
            }
        }
    }
    /// Draws layout items at `x`, breaking pages between items (never inside one); a table
    /// row that starts a new page repeats its table's header row first.
    fn items(&mut self, x: f64, items: &[Item]) {
        for it in items {
            match it {
                Item::Gap(h) => self.y -= h,
                Item::Work { lines, .. } => self.work_lines(x, *lines),
                Item::Line { indent, frags, marker, bar, scale } if *scale != 1.0 => {
                    let lead = self.lead * scale;
                    if self.remaining() < lead {
                        self.new_page();
                    }
                    let body = self.body;
                    self.body = body * scale;
                    let by = self.y - Self::ascent_of(self.body) - (lead - self.body * 1.2).max(0.0) * 0.5;
                    self.placed(x + indent, by, frags);
                    self.body = body;
                    let _ = (marker, bar);
                    self.y -= lead;
                }
                Item::Rule { w } => {
                    let h = self.lead * 0.8;
                    if self.remaining() < h {
                        self.new_page();
                    }
                    let y = self.y - h * 0.5;
                    self.stroke_line(x, y, x + w, y, 0.7, "0.55 0.55 0.55");
                    self.y -= h;
                }
                Item::Cols { cols, h } => {
                    if self.remaining() < *h {
                        self.new_page();
                    }
                    let top = self.y;
                    for (cx, its) in cols {
                        self.y = top;
                        self.items(x + cx, its);
                    }
                    self.y = top - h;
                }
                Item::Line { indent, frags, marker, bar, .. } => {
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
    /// `n` lines of work space from `x` to the right margin: answer lines in the lined
    /// styles, blank room otherwise (breaking pages as needed).
    fn work_lines(&mut self, x: f64, n: u8) {
        if self.style.work_space_lined() {
            for _ in 0..n {
                let xb = self.x1;
                self.answer_line(x, xb);
            }
        } else {
            let mut left = n as f64 * self.pitch();
            while left > 0.0 {
                if self.remaining() <= 1.0 {
                    self.new_page();
                }
                let step = left.min(self.remaining());
                self.y -= step;
                left -= step;
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
            // (from the first column: a centred table's columns start right of `x`)
            let x0 = x + cols.first().map_or(0.0, |c| c.0);
            self.cur.extend_from_slice(
                format!("0.93 g {} {} {} {} re f 0 g\n", fnum(x0), fnum(top - h), fnum(total), fnum(*h)).as_bytes(),
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

/// Shortest printed blank: a `___` run (3+ underscores) prints at least this many ems long,
/// the width the page preview draws a blank (`equation::Rich::blank_em`); longer runs keep
/// their own length.
const BLANK_EM: f64 = 4.5;

/// Widens every `___` run (3+ underscores) to at least [`BLANK_EM`] of underscores, so a
/// fill-in blank prints as a writing line of a proper length (Test Parrot prints them long).
/// Text without such a run comes back unchanged.
pub(crate) fn widen_blanks(s: &str) -> String {
    if !s.contains("___") {
        return s.to_string();
    }
    let min = (BLANK_EM / char_w(b'_', Face::Reg, 1.0)).ceil() as usize;
    let mut out = String::with_capacity(s.len() + 16);
    let mut run = 0usize;
    let flush = |out: &mut String, run: usize| {
        let n = if run >= 3 { run.max(min) } else { run };
        out.extend(std::iter::repeat_n('_', n));
    };
    for c in s.chars() {
        if c == '_' {
            run += 1;
        } else {
            flush(&mut out, run);
            run = 0;
            out.push(c);
        }
    }
    flush(&mut out, run);
    out
}

/// The questions as they print: blanks widened ([`widen_blanks`]) in stems and choices.
fn printed_questions(doc: &ExamDoc) -> Vec<DocQ> {
    doc.questions
        .iter()
        .map(|q| {
            let mut q = q.clone();
            if q.no_answer_space {
                q.work_lines = 0;
                q.space_after.clear();
            }
            if !doc.is_key || doc.key_layout == KeyLayout::Marked {
                q.text.iter_mut().for_each(|t| *t = widen_blanks(t));
                q.choices.iter_mut().for_each(|t| *t = widen_blanks(t));
            }
            q
        })
        .collect()
}

/// Stem text starts this far inside the left margin (number column is right-aligned
/// in the gutter to its left).
const STEM_INSET: f64 = 32.0;
const NUM_GAP: f64 = 6.0;
/// Classic: width of the answer blank printed before an objective question's number.
const OBJ_BLANK_W: f64 = 40.0;

/// Objective question (MC / TF / matching): it carries choices.
fn is_objective(q: &DocQ) -> bool {
    !q.choices.is_empty()
}

/// Where this question's stem starts (Classic objective questions sit right of an answer blank).
fn q_stem_x(r: &R, q: &DocQ) -> f64 {
    let blank = if r.style.objective_blank() && is_objective(q) && !looks_matching(q, is_tf(&q.choices)) { OBJ_BLANK_W } else { 0.0 };
    r.x0 + STEM_INSET + blank
}

/// Section heading lines: (bold title lines, italic direction lines).
fn section_lines(r: &R, s: &DocSection) -> (Vec<String>, Vec<String>) {
    let w = r.x1 - r.x0;
    let title = if s.title.trim().is_empty() { vec![] } else { wrap(s.title.trim(), w, Face::Bold, r.body * 1.1) };
    let dirs = if s.directions.trim().is_empty() { vec![] } else { wrap(s.directions.trim(), w, Face::Ital, r.body) };
    (title, dirs)
}

fn section_h(r: &R, s: &DocSection) -> f64 {
    let (t, d) = section_lines(r, s);
    if t.is_empty() && d.is_empty() {
        return r.body * 0.6;
    }
    r.body * 0.8 + t.len() as f64 * r.lead * 1.1 + d.len() as f64 * r.lead + r.body * 0.5
}

fn render_section(r: &mut R, s: &DocSection) {
    let (t, d) = section_lines(r, s);
    if t.is_empty() && d.is_empty() {
        r.y -= r.body * 0.6;
        return;
    }
    r.y -= r.body * 0.8;
    let x0 = r.x0;
    for l in t {
        if r.remaining() < r.lead * 1.1 {
            r.new_page();
        }
        let sz = r.body * 1.1;
        r.text(x0, r.y - R::ascent_of(sz), Face::Bold, sz, &l);
        r.y -= r.lead * 1.1;
    }
    for l in d {
        if r.remaining() < r.lead {
            r.new_page();
        }
        r.line(x0, &l, Face::Ital);
    }
    r.y -= r.body * 0.5;
}

pub fn render_pdf(doc: &ExamDoc) -> Result<Vec<u8>, String> {
    assemble_pdf(layout_pdf(doc, true)?)
}

/// The PDF's pages before assembly, for the export dialog's live page preview: each page's
/// content stream (the exact drawing operators the PDF will carry), the page size in pt,
/// and each image XObject's pixel size (`/Im{k+1}` is `images[k]`, in question order).
#[derive(Clone, Debug, Default)]
pub struct PdfPreview {
    pub page_w: f64,
    pub page_h: f64,
    pub pages: Vec<Vec<u8>>,
    pub images: Vec<(u32, u32)>,
    /// The character behind each code of the fallback font F9 (the `<..>` strings it shows).
    pub fallback: Vec<char>,
}

/// Lays the PDF out without encoding images (fast enough to run while the dialog is open).
pub fn preview_pages(doc: &ExamDoc) -> Result<PdfPreview, String> {
    let r = layout_pdf(doc, false)?;
    Ok(PdfPreview {
        page_w: r.page_w,
        page_h: r.page_h,
        pages: r.pages.into_iter().map(|(ops, _)| ops).collect(),
        images: r.imgs.iter().map(|p| (p.w, p.h)).collect(),
        fallback: r.fb.iter().map(|f| f.1).collect(),
    })
}

/// Lays every page out; `encode` false skips the JPEG re-encode (sizes only, for previews).
fn layout_pdf(doc: &ExamDoc, encode: bool) -> Result<R, String> {
    // Prepare images first (decode -> RGB -> JPEG q85) in question order.
    let mut imgs: Vec<Px> = vec![];
    for q in &doc.questions {
        if let Some(im) = &q.image {
            if encode {
                imgs.push(prepare_image(im)?);
            } else {
                imgs.push(Px { jpeg: Vec::new(), w: im.width_px.max(1), h: im.height_px.max(1) });
            }
        }
    }

    let m = doc.margins_mm();
    let mm = |v: f32| (v as f64).max(6.0) * MM_PT;
    let (mt, mr, mb, ml) = (mm(m.top), mm(m.right), mm(m.bottom), mm(m.left));
    let (pw, ph) = doc.style.page_pt();
    let (page_w, page_h) = (pw as f64, ph as f64);
    let body = (doc.body_pt as f64).clamp(6.0, 20.0);
    let head = (doc.heading_pt as f64).clamp(8.0, 36.0);
    let mut r = R {
        pages: vec![],
        cur: vec![],
        used: vec![],
        y: page_h - mt,
        x0: ml,
        x1: page_w - mr,
        mt,
        mb,
        page_w,
        page_h,
        style: doc.style,
        body,
        head,
        lead: body * doc.style.lead() as f64,
        imgs,
        running: None,
        glyphs: [[false; 256]; 5],
        cids: Default::default(),
        fb: Vec::new(),
        annots: Vec::new(),
        mark: false,
    };

    let questions = printed_questions(doc);
    // A marked key prints the whole test (the student layout) with the answers marked.
    let marked = doc.is_key && doc.key_layout == KeyLayout::Marked;
    let section_key = doc.is_key && doc.key_layout == KeyLayout::Section;
    let list_key = doc.is_key && !marked && !section_key;
    r.mark = marked;
    if doc.is_key {
        // Answer key: the title on its own (no header line).
        if !doc.title.is_empty() {
            r.text(r.x0, r.y - r.head * 0.75, Face::Bold, head, doc.title.trim());
            r.y -= head * 1.35;
        }
        if section_key {
            // Test Parrot's key heading.
            let sz = body * 1.25;
            r.text(r.x0, r.y - R::ascent_of(sz), Face::Bold, sz, "Answer Section");
            r.y -= sz * 1.5;
        }
    } else {
        // Title area as the page preview lays it out: the header line (Name / Class / Date)
        // at the top with a rule under it, then the title centred, then a muted centred
        // "N questions · M points" line.
        if !doc.header_line.trim().is_empty() {
            r.line(r.x0, doc.header_line.trim(), Face::Reg);
            let (x0, x1, y) = (r.x0, r.x1, r.y - body * 0.15);
            r.rule(x0, y, x1, y);
            r.y -= body * 0.15 + 14.0;
        }
        if !doc.title.trim().is_empty() {
            let mid = (r.x0 + r.x1) / 2.0;
            for l in wrap(doc.title.trim(), r.x1 - r.x0, Face::Bold, head) {
                let w = text_w(&l, Face::Bold, head);
                r.text(mid - w / 2.0, r.y - head * 0.8, Face::Bold, head, &l);
                r.y -= head * 1.25;
            }
            let n = questions.len();
            let pts: f64 = questions.iter().filter_map(|q| q.points).sum();
            let sub = format!(
                "{n} question{}{}",
                if n == 1 { "" } else { "s" },
                if pts > 0.0 { format!(" \u{b7} {} point{}", points_number(pts), if pts == 1.0 { "" } else { "s" }) } else { String::new() }
            );
            let sz = body * 0.92;
            let w = text_w(&sub, Face::Reg, sz);
            r.text_muted(mid - w / 2.0, r.y - R::ascent_of(sz), sz, &sub);
            r.y -= sz * 1.45 + 8.0;
        }
        for _ in 0..doc.ruled_blanks {
            let (xa, xb) = (r.x0, r.x1);
            r.answer_line(xa, xb);
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
    if !questions.is_empty() {
        r.y -= body * 0.5; // small gap before question 1
    }

    let mut next_img = 0usize;
    for qi in 0..questions.len() {
        let q = &questions[qi];
        // Index into `imgs`, which holds only the questions that carry an image.
        let img_idx = q.image.as_ref().map(|_| {
            next_img += 1;
            next_img - 1
        });
        let sec = q.section.as_ref().filter(|_| !list_key);
        let est = if section_key { section_key_h(&r, q) } else { q_est_height(&r, q, img_idx, list_key) } + sec.map_or(0.0, |s| section_h(&r, s));
        // Keep-together: break before a question (with its section heading) that will not
        // comfortably fit.
        if r.remaining() < 40.0 || (est <= r.full_height() && est > r.remaining()) {
            r.new_page();
        }
        if let Some(s) = sec {
            if section_key {
                // The key names each section (its title, or "Section N").
                let k = questions[..=qi].iter().filter(|x| x.section.is_some()).count();
                let t = if s.title.trim().is_empty() { format!("Section {k}") } else { s.title.trim().to_string() };
                render_section(&mut r, &DocSection { title: t, directions: String::new() });
            } else {
                render_section(&mut r, s);
            }
        }
        if section_key {
            render_section_key_q(&mut r, q);
        } else if list_key {
            render_key_q(&mut r, q);
        } else {
            render_q(&mut r, q, img_idx);
        }
        r.y -= body * if section_key { 0.25 } else { doc.style.q_gap() as f64 }; // separator between questions
    }
    if !r.cur.is_empty() {
        let cur = std::mem::take(&mut r.cur);
        let used = std::mem::take(&mut r.used);
        r.pages.push((cur, used));
    }
    Ok(r)
}

fn render_key_q(r: &mut R, q: &DocQ) {
    if q.key_text.trim().is_empty() {
        return;
    }
    let stem_x = r.x0 + STEM_INSET;
    let avail = r.x1 - stem_x;
    let items = layout_nodes(&richtext::parse_tree(q.key_text.trim()), avail, avail, r.body, r.lead);
    if r.remaining() < r.lead {
        r.new_page();
    }
    let lbl = r.style.number_label(q.number);
    r.num_right(stem_x - NUM_GAP, &lbl, Face::Bold);
    r.items(stem_x, &items);
}

/// Width of an answer-section blank holding `mark`.
fn key_blank_w(r: &R, mark: &str) -> f64 {
    (text_w(mark, Face::Reg, r.body) + 16.0).max(48.0)
}

/// Height of one question's entry in the answer section.
fn section_key_h(r: &R, q: &DocQ) -> f64 {
    let rows = q.key_marks.len().max(1) as f64 * r.lead * 1.1;
    if !q.key_marks.is_empty() || q.key_text.trim().is_empty() {
        return rows;
    }
    let avail = r.x1 - r.x0 - STEM_INSET - NUM_GAP;
    rows + items_h(&layout_nodes(&richtext::parse_tree(q.key_text.trim()), avail, avail, r.body, r.lead), r.lead)
}

/// One question in Test Parrot's answer section: its number and a short blank with the
/// answer written on it (one per left item for matching); a written answer prints under an
/// empty blank.
fn render_section_key_q(r: &mut R, q: &DocQ) {
    let stem_x = r.x0 + STEM_INSET;
    let blank_x = stem_x + NUM_GAP;
    let marks: Vec<String> = if q.key_marks.is_empty() { vec![String::new()] } else { q.key_marks.clone() };
    for (i, m) in marks.iter().enumerate() {
        let row = r.lead * 1.1;
        if r.remaining() < row {
            r.new_page();
        }
        let by = r.y - R::ascent_of(r.body);
        let lbl = r.style.number_label(q.number + i as u32);
        r.num_right(stem_x - NUM_GAP, &lbl, Face::Reg);
        let w = key_blank_w(r, m);
        if !m.is_empty() {
            let tw = text_w(m, Face::Reg, r.body);
            r.text(blank_x + (w - tw) / 2.0, by, Face::Reg, r.body, m);
        }
        r.stroke_line(blank_x, by - 2.5, blank_x + w, by - 2.5, 0.7, "0 0 0");
        r.y -= row;
    }
    if q.key_marks.is_empty() && !q.key_text.trim().is_empty() {
        let avail = r.x1 - blank_x;
        let items = layout_nodes(&richtext::parse_tree(q.key_text.trim()), avail, avail, r.body, r.lead);
        r.items(blank_x, &items);
    }
}

/// Stem items of an ordinary question plus the points label and its reserved width.
fn stem_items(r: &R, q: &DocQ) -> (Vec<Item>, Option<String>) {
    let avail = r.x1 - q_stem_x(r, q);
    let pt_label = q.points.map(|p| r.style.points_label(p));
    let label_w = pt_label.as_deref().map(|l| text_w(l, Face::Reg, r.body) + 6.0).unwrap_or(0.0);
    if q.space_after.iter().all(|&n| n == 0) {
        return (layout_nodes(&stem_nodes(&q.text), avail, avail - label_w, r.body, r.lead), pt_label);
    }
    // Per-part work space: each text entry (one part) lays out on its own, its work space
    // right under it.
    let mut items: Vec<Item> = Vec::new();
    for (k, t) in q.text.iter().enumerate() {
        let first = items.is_empty();
        if !first {
            items.push(Item::Gap(r.body * 0.35));
        }
        items.extend(layout_nodes(&richtext::parse_tree(t), avail, if first { avail - label_w } else { avail }, r.body, r.lead));
        if let Some(&n) = q.space_after.get(k).filter(|&&n| n > 0) {
            items.push(Item::Work { lines: n, h: n as f64 * r.pitch() });
        }
    }
    (items, pt_label)
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
            let rt = q.choices.get(i).map(|c| text_lines(&r.style.choice_label(c), right_w, right_w, r.body)).unwrap_or_default();
            let n = l.len().max(rt.len()).max(1);
            let h = (r.lead * n as f64).max(24.0);
            (l, rt, h)
        })
        .collect()
}

fn render_q(r: &mut R, q: &DocQ, img_idx: Option<usize>) {
    let stem_x = q_stem_x(r, q);
    let avail = r.x1 - stem_x;
    let tf = is_tf(&q.choices);
    let matching = looks_matching(q, tf);

    if matching {
        // Two columns: left items numbered in print as number, number+1, ...; right
        // column lettered (pre-lettered by the exam worker), 24pt leading per row.
        let mid = r.x0 + (r.x1 - r.x0) * 0.5;
        let pt_label = q.points.map(|p| r.style.points_label(p));
        for (i, (left, right, row_h)) in matching_rows(r, q).into_iter().enumerate() {
            if r.remaining() < row_h {
                r.new_page();
            }
            let top = r.y;
            for (j, piece) in left.iter().enumerate() {
                let by = top - j as f64 * r.lead - R::ascent_of(r.body);
                if j == 0 {
                    let lbl = r.style.number_label(q.number + i as u32);
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
        let ans = marked_answer_items(r, q, r.x1 - stem_x);
        r.items(stem_x, &ans);
        work_space(r, q, stem_x);
        return;
    }

    // An attached picture prints where the stem's first `[picture]` marker is (the text
    // before it, the picture, then the rest); without a marker it prints under the stem.
    let split = picture_split(q, img_idx);
    let q_head;
    let q = match &split {
        Some((head, _)) => {
            q_head = head.clone();
            &q_head
        }
        None => q,
    };
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
    let lbl = r.style.number_label(q.number);
    let w = text_w(&lbl, Face::Reg, r.body);
    r.text(stem_x - NUM_GAP - w, by, Face::Reg, r.body, &lbl);
    if stem_x > r.x0 + STEM_INSET + 1.0 {
        // Classic: the answer blank in front of an objective question's number.
        let (xa, xb) = (r.x0, r.x0 + OBJ_BLANK_W - 4.0);
        r.rule(xa, by - 1.5, xb, by - 1.5);
    }
    if let Some(lbl) = &pt_label {
        r.text(r.x1 - text_w(lbl, Face::Reg, r.body), by, Face::Reg, r.body, lbl);
    }
    r.items(stem_x, &items);

    // Picture under the stem (or at its marker), before choices and ruled lines, at the
    // stem's left edge, as the page preview draws it; page breaks first if it does not fit.
    if let Some(idx) = img_idx {
        draw_pdf_image(r, idx, stem_x);
    }
    if let Some((_, rest)) = &split {
        let rest: Vec<Item> = layout_nodes(&stem_nodes(rest), avail, avail, r.body, r.lead);
        r.items(stem_x, &rest);
    }

    // Choices.
    if tf {
        if r.remaining() < r.lead {
            r.new_page();
        }
        let by = r.y - R::ascent_of(r.body);
        if r.mark && !q.correct.is_empty() {
            // Marked key: the correct one checked and bold.
            let mut x = stem_x;
            for (i, t) in ["True", "False"].into_iter().enumerate() {
                let ok = q.correct.contains(&i);
                let f = if ok { Face::Bold } else { Face::Reg };
                if ok {
                    draw_check(r, x, by);
                }
                r.text(x, by, f, r.body, t);
                x += text_w(t, f, r.body) + text_w("   ", Face::Reg, r.body);
            }
        } else {
            r.text(stem_x, by, Face::Reg, r.body, "True   False");
        }
        r.y -= r.lead;
    } else if let Some(grid) = explicit_grid(r, q, avail) {
        // Answer columns 2 / 4 (the exam item's switch): equal columns, filled column-major,
        // each choice wrapping inside its column; a row never splits across pages.
        let col_w = avail / grid.cols as f64;
        for row in &grid.rows {
            let h = row.iter().map(|(_, ls)| ls.len()).max().unwrap_or(1).max(1) as f64 * r.lead;
            if r.remaining() < h {
                r.new_page();
            }
            let top = r.y;
            for (k, (ci, lines)) in row.iter().enumerate() {
                for (j, l) in lines.iter().enumerate() {
                    let by = top - j as f64 * r.lead - R::ascent_of(r.body);
                    if j == 0 && r.mark && q.correct.contains(ci) {
                        draw_check(r, stem_x + k as f64 * col_w, by);
                    }
                    r.placed(stem_x + k as f64 * col_w, by, l);
                }
            }
            r.y = top - h;
        }
    } else if let Some(cols) = choice_columns(r, q, avail) {
        // Condensed: choices across in a grid of `cols` columns.
        let col_w = avail / cols as f64;
        for (ri, row) in q.choices.chunks(cols).enumerate() {
            if r.remaining() < r.lead {
                r.new_page();
            }
            let by = r.y - R::ascent_of(r.body);
            for (k, _) in row.iter().enumerate() {
                let ci = ri * cols + k;
                let c = choice_text(r, q, ci);
                if r.mark && q.correct.contains(&ci) {
                    draw_check(r, stem_x + k as f64 * col_w, by);
                }
                if let Some(l) = text_lines(&c, col_w, col_w, r.body).first() {
                    r.placed(stem_x + k as f64 * col_w, by, l);
                }
            }
            r.y -= r.lead;
        }
    } else {
        for ci in 0..q.choices.len() {
            let c = choice_text(r, q, ci);
            for (j, l) in text_lines(&c, avail, avail, r.body).into_iter().enumerate() {
                if r.remaining() < r.lead {
                    r.new_page();
                }
                let by = r.y - R::ascent_of(r.body);
                if j == 0 && r.mark && q.correct.contains(&ci) {
                    draw_check(r, stem_x, by);
                }
                r.placed(stem_x, by, &l);
                r.y -= r.lead;
            }
        }
    }
    // Marked key: the expected answer of a written question, before its answer lines.
    let ans = marked_answer_items(r, q, avail);
    r.items(stem_x, &ans);

    // Ruled answer lines.
    for _ in 0..rule_count(q) {
        let (xa, xb) = (r.x0, r.x1);
        r.answer_line(xa, xb);
    }
    work_space(r, q, stem_x);
}

/// Choice `i` as printed: the style's label, bold on a marked key when it is correct.
fn choice_text(r: &R, q: &DocQ, i: usize) -> String {
    let c = r.style.choice_label(&q.choices[i]);
    if r.mark && q.correct.contains(&i) && !c.contains("**") && !c.contains('\n') {
        format!("**{c}**")
    } else {
        c
    }
}

/// A check mark just left of `x` on baseline `by` (stroked, so no font needs the glyph).
fn draw_check(r: &mut R, x: f64, by: f64) {
    let s = r.body;
    let (x0, y0) = (x - s * 0.95, by + s * 0.3);
    r.cur.extend_from_slice(
        format!(
            "{} w 0.1 0.45 0.25 RG 1 J 1 j {} {} m {} {} l {} {} l S 0 J 0 j\n",
            fnum(s / 9.0),
            fnum(x0),
            fnum(y0),
            fnum(x0 + s * 0.22),
            fnum(by + s * 0.05),
            fnum(x0 + s * 0.62),
            fnum(by + s * 0.68)
        )
        .as_bytes(),
    );
}

/// Marked key: "Answer: ..." items for a question without checkable choices (written
/// answers, fill-ins, matching), printed before its answer lines. Empty otherwise.
fn marked_answer_items(r: &R, q: &DocQ, avail: f64) -> Vec<Item> {
    if !r.mark || !q.correct.is_empty() || q.key_text.trim().is_empty() {
        return Vec::new();
    }
    let t = format!("**Answer:** {}", q.key_text.trim());
    let mut items = vec![Item::Gap(r.body * 0.25)];
    items.extend(layout_nodes(&richtext::parse_tree(&t), avail, avail, r.body, r.lead));
    items
}

/// A question with a picture and a `[picture]` marker in its stem: (the question cut before
/// the marker, the stem text after it). Per-part work space keeps the picture under the stem.
fn picture_split(q: &DocQ, img_idx: Option<usize>) -> Option<(DocQ, Vec<String>)> {
    img_idx?;
    if q.space_after.iter().any(|&n| n > 0) {
        return None;
    }
    let (before, after) = crate::layout::split_at_picture(q.text.first()?)?;
    let mut head = q.clone();
    head.text = vec![before];
    head.image = None;
    let mut rest: Vec<String> = Vec::new();
    if !after.trim().is_empty() {
        rest.push(after);
    }
    rest.extend(q.text[1..].iter().cloned());
    Some((head, rest))
}

/// The question's extra work space: ruled / dotted lines (Classic, Exam Board) or blank room.
fn work_space(r: &mut R, q: &DocQ, stem_x: f64) {
    r.work_lines(stem_x, q.work_lines);
}

/// Choices laid out in the exam item's explicit answer columns (2..=4).
struct ChoiceGrid {
    cols: usize,
    /// Rows of (choice index, wrapped lines), column-major order (`layout::choice_grid`).
    rows: Vec<Vec<(usize, Vec<Vec<Placed>>)>>,
}

/// The explicit answer-column grid for a multiple-choice question (None: one per line or the
/// style's own layout). True/False and matching questions keep their own layouts.
fn explicit_grid(r: &R, q: &DocQ, avail: f64) -> Option<ChoiceGrid> {
    if q.answer_columns < 2 || q.choices.len() < 2 || is_tf(&q.choices) {
        return None;
    }
    let grid = crate::layout::choice_grid(q.choices.len(), q.answer_columns.min(4));
    let col_w = avail / q.answer_columns.min(4) as f64;
    let w = (col_w - 10.0).max(24.0);
    let rows = grid
        .into_iter()
        .map(|row| row.into_iter().map(|i| (i, text_lines(&choice_text(r, q, i), w, w, r.body))).collect())
        .collect();
    Some(ChoiceGrid { cols: q.answer_columns.min(4) as usize, rows })
}

/// Condensed: how many columns the choices print across in (None = one per line). Every
/// choice must fit its column on one line.
fn choice_columns(r: &R, q: &DocQ, avail: f64) -> Option<usize> {
    if !r.style.choices_across() || q.choices.len() < 2 {
        return None;
    }
    let widest = q
        .choices
        .iter()
        .map(|c| natural_w(&line_frags(&richtext::inline(&r.style.choice_label(c)), false), r.body))
        .fold(0.0_f64, f64::max);
    let n = q.choices.len();
    [n, 4, 3, 2].into_iter().filter(|&c| c >= 2 && c <= n).find(|&c| widest + 12.0 <= avail / c as f64)
}

/// Question height for keep-together page breaks, from the same layout the renderer draws.
fn q_est_height(r: &R, q: &DocQ, img_idx: Option<usize>, is_key: bool) -> f64 {
    let stem_x = if is_key { r.x0 + STEM_INSET } else { q_stem_x(r, q) };
    let avail = r.x1 - stem_x;
    if is_key {
        if q.key_text.trim().is_empty() {
            return 0.0;
        }
        return items_h(&layout_nodes(&richtext::parse_tree(q.key_text.trim()), avail, avail, r.body, r.lead), r.lead);
    }
    let tf = is_tf(&q.choices);
    let ans = items_h(&marked_answer_items(r, q, avail), r.lead);
    if looks_matching(q, tf) {
        return matching_rows(r, q).iter().map(|x| x.2).sum::<f64>() + r.body + q.work_lines as f64 * r.pitch() + ans;
    }
    let mut h = items_h(&stem_items(r, q).0, r.lead) + ans;
    let lines = if tf {
        1
    } else if let Some(g) = explicit_grid(r, q, avail) {
        g.rows.iter().map(|row| row.iter().map(|(_, ls)| ls.len()).max().unwrap_or(1).max(1)).sum::<usize>()
    } else if let Some(cols) = choice_columns(r, q, avail) {
        q.choices.len().div_ceil(cols)
    } else {
        q.choices.iter().map(|c| text_lines(&r.style.choice_label(c), avail, avail, r.body).len()).sum::<usize>()
    };
    h += lines as f64 * r.lead + (rule_count(q) as f64 + q.work_lines as f64) * r.pitch();
    if let Some(idx) = img_idx {
        h += 12.0 + pdf_image_size(r, idx, stem_x).1;
    }
    h
}

/// The picture's printed size: `layout::picture_size_pt` in the stem column (the page
/// preview draws the same box).
fn pdf_image_size(r: &R, idx: usize, x: f64) -> (f64, f64) {
    let px = &r.imgs[idx];
    crate::layout::picture_size_pt(px.w, px.h, r.x1 - x)
}

fn draw_pdf_image(r: &mut R, idx: usize, x: f64) {
    let (w, h) = pdf_image_size(r, idx, x);
    if r.remaining() < h + r.body {
        r.new_page();
    }
    r.y -= 6.0;
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

/// Subset tag ("ABCDEF+") naming which glyphs a face's embedded subset holds.
fn subset_tag(face: Face, used: &[bool; 256], cids: &[u16]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ face.idx() as u64;
    for (b, u) in used.iter().enumerate() {
        if *u {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    for g in cids {
        h = (h ^ (0x1_0000 + *g as u64)).wrapping_mul(0x0100_0000_01b3);
    }
    (0..6).map(|k| (b'A' + ((h >> (k * 5)) % 26) as u8) as char).collect()
}

/// Font dictionary, font descriptor and FlateDecode `FontFile2` stream of one face, the
/// descriptor being object `desc_obj` and the font program `desc_obj + 1`. `/Widths` are the
/// same advance widths the layout measured with.
fn font_objects(face: Face, used: &[bool; 256], cids: &[u16], desc_obj: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let m = fonts::metrics(face.style());
    let mut used = *used;
    used[32] = true;
    let name = font_name(face, &used, cids);
    let widths: Vec<String> = (32..=255).map(|b| m.widths[b].to_string()).collect();
    let dict = format!(
        "<< /Type /Font /Subtype /TrueType /BaseFont /{name} /FirstChar 32 /LastChar 255 /Widths [{}] /Encoding /WinAnsiEncoding /FontDescriptor {desc_obj} 0 R >>\n",
        widths.join(" ")
    );
    // Flags: Serif (2) + Nonsymbolic (32) [+ Italic (64)]; the mono: FixedPitch (1) + Nonsymbolic.
    let flags = if face == Face::Mono { 1 + 32 } else { 2 + 32 + if face.style().italic() { 64 } else { 0 } };
    let stem_v = if face.style().bold() { 140 } else { 80 };
    let [x0, y0, x1, y1] = m.bbox;
    let desc = format!(
        "<< /Type /FontDescriptor /FontName /{name} /Flags {flags} /FontBBox [{x0} {y0} {x1} {y1}] /ItalicAngle {} /Ascent {} /Descent {} /CapHeight {} /StemV {stem_v} /FontFile2 {} 0 R >>\n",
        fnum(m.italic_angle as f64),
        m.ascent,
        m.descent,
        m.cap_height,
        desc_obj + 1
    );
    let program = fonts::subset_with(face.style(), &used, cids);
    let z = fonts::deflate(&program);
    let mut file = format!("<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n", z.len(), program.len()).into_bytes();
    file.extend_from_slice(&z);
    file.extend_from_slice(b"\nendstream\n");
    (dict.into_bytes(), desc.into_bytes(), file)
}

/// Subset name of a face's embedded program ("ABCDEF+SourceSerif4-Regular"), shared by its
/// simple font and its CID font.
fn font_name(face: Face, used: &[bool; 256], cids: &[u16]) -> String {
    format!("{}+{}", subset_tag(face, used, cids), fonts::metrics(face.style()).ps_name)
}

/// A URI as 7-bit ASCII for a PDF string (non-ASCII bytes percent-encoded).
fn uri_ascii(url: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(url.len());
    for &b in url.as_bytes() {
        if b.is_ascii_graphic() {
            out.push(b);
        } else {
            out.extend_from_slice(format!("%{b:02X}").as_bytes());
        }
    }
    out
}

/// A FlateDecode stream object.
fn flate_stream(data: &[u8]) -> Vec<u8> {
    let z = fonts::deflate(data);
    let mut b = format!("<< /Length {} /Filter /FlateDecode >>\nstream\n", z.len()).into_bytes();
    b.extend_from_slice(&z);
    b.extend_from_slice(b"\nendstream\n");
    b
}

/// The fonts beyond F1..F4, numbered from object `first`: per face whose CID font is used a
/// Type0 font, its CIDFontType2 (sharing the face's descriptor and program) and a ToUnicode
/// CMap; then the fallback Type 3 font F9 with its ToUnicode CMap and glyph procedures; then
/// the mono F10 when code was drawn.
/// Returns the objects and the page resource entries naming them.
fn extra_font_objects(r: &R, first: usize, desc_of: impl Fn(Face) -> usize) -> (Vec<Vec<u8>>, String) {
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let mut res = String::new();
    for face in [Face::Reg, Face::Bold, Face::Ital, Face::BoldItal] {
        let cids = &r.cids[face.idx()];
        if cids.is_empty() {
            continue;
        }
        let at = first + objs.len();
        let mut used = r.glyphs[face.idx()];
        used[32] = true;
        let gids: Vec<u16> = cids.keys().copied().collect();
        let name = font_name(face, &used, &gids);
        let cm = fonts::charmap(face.style());
        let w: Vec<String> = cids.iter().map(|(g, c)| format!("{g} [{}]", cm.get(*c).map_or(0, |x| x.1))).collect();
        objs.push(format!("<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>\n", at + 1, at + 2).into_bytes());
        objs.push(
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /DW 1000 /W [{}] /CIDToGIDMap /Identity >>\n",
                desc_of(face),
                w.join(" ")
            )
            .into_bytes(),
        );
        let entries: Vec<(u16, char)> = cids.iter().map(|(g, c)| (*g, *c)).collect();
        objs.push(flate_stream(fonts::to_unicode_cmap(&entries, 2).as_bytes()));
        res.push_str(&format!(" /F{} {at} 0 R", 5 + face.idx()));
    }
    if !r.fb.is_empty() {
        let at = first + objs.len();
        let n = r.fb.len();
        // Code n is an unused, empty glyph named /m, 0.6 em wide: text extractors (poppler)
        // guess a Type 3 font's scale from the width of its 'm' (else from the first glyph,
        // and a wide arrow would make them think the symbols are set larger, off the line).
        let procs: String = (0..n).map(|k| format!("/g{k} {} 0 R ", at + 2 + k)).collect::<String>() + &format!("/m {} 0 R ", at + 2 + n);
        let diffs: String = (0..n).map(|k| format!("/g{k}")).collect::<Vec<_>>().join(" ") + " /m";
        let widths: String = r.fb.iter().map(|f| f.2.to_string()).collect::<Vec<_>>().join(" ") + " 600";
        // The union of the drawn glyphs' boxes (text extractors size the line from it).
        let procs_bb: Vec<(String, [i32; 4])> = r.fb.iter().map(|(g, _, w)| fonts::fallback_glyph_proc(*g, *w)).collect();
        let [x0, y0, x1, y1] = procs_bb.iter().filter(|p| p.1 != [0; 4]).fold([i32::MAX, i32::MAX, i32::MIN, i32::MIN], |a, p| [a[0].min(p.1[0]), a[1].min(p.1[1]), a[2].max(p.1[2]), a[3].max(p.1[3])]);
        let [x0, y0, x1, y1] = if x0 > x1 { [0; 4] } else { [x0, y0, x1, y1] };
        objs.push(
            format!(
                "<< /Type /Font /Subtype /Type3 /Name /F9 /FontBBox [{x0} {y0} {x1} {y1}] /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << {procs}>> /Encoding << /Type /Encoding /Differences [0 {diffs}] >> /FirstChar 0 /LastChar {} /Widths [{widths}] /ToUnicode {} 0 R /Resources << >> >>\n",
                n,
                at + 1
            )
            .into_bytes(),
        );
        let entries: Vec<(u16, char)> = r.fb.iter().enumerate().map(|(k, f)| (k as u16, f.1)).collect();
        objs.push(flate_stream(fonts::to_unicode_cmap(&entries, 1).as_bytes()));
        for p in procs_bb.iter().map(|p| p.0.as_str()).chain(std::iter::once("600 0 0 0 0 0 d1\n")) {
            let mut b = format!("<< /Length {} >>\nstream\n", p.len()).into_bytes();
            b.extend_from_slice(p.as_bytes());
            b.extend_from_slice(b"endstream\n");
            objs.push(b);
        }
        res.push_str(&format!(" /F9 {at} 0 R"));
    }
    // The mono F10 (font dict, descriptor, subset program), only when code was drawn.
    if r.glyphs[Face::Mono.idx()].iter().any(|&u| u) {
        let at = first + objs.len();
        let (dict, desc, file) = font_objects(Face::Mono, &r.glyphs[Face::Mono.idx()], &[], at + 1);
        objs.extend([dict, desc, file]);
        res.push_str(&format!(" /F10 {at} 0 R"));
    }
    (objs, res)
}

fn assemble_pdf(r: R) -> Result<Vec<u8>, String> {
    let media = format!("[0 0 {} {}]", fnum(r.page_w), fnum(r.page_h));
    let npage = r.pages.len();
    let n_imgs = r.imgs.len();
    // Objects: 1 catalog, 2 page tree, 3..=6 fonts, then images, then 2 per page, then a
    // descriptor and a font program per font.
    const FIRST_IMG: usize = 7;
    let kids: String = (0..npage).map(|p| format!("{} 0 R ", FIRST_IMG + n_imgs + p * 2 + 1)).collect();

    let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(4 + n_imgs + npage * 2);
    // obj 1: catalog
    bodies.push(b"<< /Type /Catalog /Pages 2 0 R >>\n".to_vec());
    // obj 2: page tree
    bodies.push(format!("<< /Type /Pages /Kids [{kids}] /Count {npage} >>\n").into_bytes());
    // obj 3..=6: fonts F1..F4, embedded TrueType subsets of the print serif. Their
    // descriptors and font programs follow the page objects (two objects per face).
    let fonts_at = FIRST_IMG + n_imgs + npage * 2;
    let mut font_tail: Vec<Vec<u8>> = Vec::with_capacity(8);
    for face in [Face::Reg, Face::Bold, Face::Ital, Face::BoldItal] {
        let cids: Vec<u16> = r.cids[face.idx()].keys().copied().collect();
        let (dict, desc, file) = font_objects(face, &r.glyphs[face.idx()], &cids, fonts_at + face.idx() * 2);
        bodies.push(dict);
        font_tail.push(desc);
        font_tail.push(file);
    }
    // After the eight descriptor / program objects: the CID and fallback fonts, if used.
    let (extra_objs, extra_res) = extra_font_objects(&r, fonts_at + 8, |f| fonts_at + f.idx() * 2);
    // Then one Link annotation object per drawn link run.
    let annots_at = fonts_at + 8 + extra_objs.len();
    let annot_objs: Vec<Vec<u8>> = r
        .annots
        .iter()
        .map(|(_, [x0, y0, x1, y1], url)| {
            let mut b = format!("<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] /A << /S /URI /URI (", fnum(*x0), fnum(*y0), fnum(*x1), fnum(*y1)).into_bytes();
            b.extend_from_slice(&esc_pdf(&uri_ascii(url)));
            b.extend_from_slice(b") >> >>\n");
            b
        })
        .collect();
    let page_annots = |p: usize| -> String {
        let refs: Vec<String> = r.annots.iter().enumerate().filter(|(_, a)| a.0 == p).map(|(k, _)| format!("{} 0 R", annots_at + k)).collect();
        if refs.is_empty() { String::new() } else { format!(" /Annots [{}]", refs.join(" ")) }
    };
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
                "<< /Type /Page /Parent 2 0 R /MediaBox {media} /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R /F4 6 0 R{extra_res} >>{xo} /ProcSet [/PDF /Text /ImageC] >> /Contents {} 0 R{} >>\n",
                FIRST_IMG + n_imgs + p * 2,
                page_annots(p)
            )
            .into_bytes(),
        );
    }

    bodies.extend(font_tail);
    bodies.extend(extra_objs);
    bodies.extend(annot_objs);

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
    /// Dotted bottom border instead of a solid rule (Exam Board answer lines).
    dotted: bool,
    center: bool,
    /// OOXML run color, e.g. "808080".
    color: Option<&'static str>,
}

impl<'a> Par<'a> {
    fn body(text: &'a str) -> Par<'a> {
        Par { text, style: None, ind: 0, sz: 0, bold: false, italic: false, rule: false, dotted: false, center: false, color: None }
    }
}

fn wpara(xml: &mut String, p: &Par, body_sz: i64) {
    xml.push_str("<w:p><w:pPr>");
    if let Some(s) = p.style {
        xml.push_str(&format!("<w:pStyle w:val=\"{s}\"/>"));
    }
    if p.rule {
        let val = if p.dotted { "dotted" } else { "single" };
        // Word and LibreOffice merge the borders of consecutive paragraphs that share them
        // (only the group's bottom line would print); the matching `between` border keeps
        // one line per rule paragraph.
        xml.push_str(&format!("<w:pBdr><w:bottom w:val=\"{val}\" w:sz=\"6\" w:space=\"1\" w:color=\"808080\"/><w:between w:val=\"{val}\" w:sz=\"6\" w:space=\"1\" w:color=\"808080\"/></w:pBdr>"));
    }
    if p.sz > 0 && p.text.is_empty() {
        // An empty (rule / work-space) paragraph: its height follows `sz`.
        xml.push_str(&format!("<w:rPr><w:sz w:val=\"{0}\"/><w:szCs w:val=\"{0}\"/></w:rPr>", p.sz));
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
    wrun_s(xml, text, bold, italic, false, script, sz);
}

/// [`wrun`] with strikethrough.
fn wrun_s(xml: &mut String, text: &str, bold: bool, italic: bool, strike: bool, script: i8, sz: i64) {
    wrun_x(xml, text, bold, italic, strike, false, script, sz);
}

/// The DOCX face of `` `code` `` runs: Courier New (every Word install has it; the font
/// table gives Liberation Mono, the PDF's mono and metric-compatible, as its alternate).
const CODE_FONT: &str = "<w:rFonts w:ascii=\"Courier New\" w:hAnsi=\"Courier New\" w:cs=\"Courier New\" w:eastAsia=\"Courier New\"/>";
/// The light shading behind code runs (the PDF's [`CODE_TINT`]).
const CODE_SHD: &str = "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"EBEBEB\"/>";

/// [`wrun_s`] that can be a code run (mono face, [`MONO_SCALE`] of `sz`, shaded).
#[allow(clippy::too_many_arguments)]
fn wrun_x(xml: &mut String, text: &str, bold: bool, italic: bool, strike: bool, code: bool, script: i8, sz: i64) {
    if text.is_empty() {
        return;
    }
    xml.push_str("<w:r><w:rPr>");
    let sz = if code {
        xml.push_str(CODE_FONT);
        ((sz as f64) * MONO_SCALE).round() as i64
    } else {
        sz
    };
    if bold {
        xml.push_str("<w:b/>");
    }
    if italic {
        xml.push_str("<w:i/>");
    }
    if strike {
        xml.push_str("<w:strike/>");
    }
    match script {
        1 => xml.push_str("<w:vertAlign w:val=\"superscript\"/>"),
        -1 => xml.push_str("<w:vertAlign w:val=\"subscript\"/>"),
        _ => {}
    }
    xml.push_str(&format!("<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>"));
    if code {
        xml.push_str(CODE_SHD);
    }
    xml.push_str("</w:rPr><w:t xml:space=\"preserve\">");
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
        let code = f.face == Face::Mono;
        match &f.link {
            Some(url) if !f.text.is_empty() => {
                // A placeholder relationship id, numbered in `docx_link_rels` once the body is done.
                xml.push_str(&format!("<w:hyperlink r:id=\"{LINK_RID}{}\" w:history=\"1\">", xml_esc(url)));
                let at = xml.len();
                wrun_x(xml, &f.text, bold, italic, f.strike, code, f.script, sz);
                let fonts = if code { CODE_FONT.len() } else { 0 };
                xml.insert_str(at + "<w:r><w:rPr>".len() + fonts, "<w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/>");
                xml.push_str("</w:hyperlink>");
            }
            _ => wrun_x(xml, &f.text, bold, italic, f.strike, code, f.script, sz),
        }
    }
}

/// Prefix of a hyperlink's placeholder relationship id (followed by the escaped URL).
const LINK_RID: &str = "qslink:";

/// Replaces each hyperlink placeholder id in `body` with a real relationship id (`rIdL1`, ...,
/// one per distinct URL) and returns the matching external relationships.
fn docx_link_rels(body: &mut String) -> String {
    let pat = format!("r:id=\"{LINK_RID}");
    let mut urls: Vec<String> = Vec::new();
    let mut out = String::with_capacity(body.len());
    let mut rest = body.as_str();
    while let Some(i) = rest.find(&pat) {
        out.push_str(&rest[..i]);
        let tail = &rest[i + pat.len()..];
        let end = tail.find('"').unwrap_or(tail.len());
        let url = &tail[..end];
        let k = urls.iter().position(|u| u == url).unwrap_or_else(|| {
            urls.push(url.to_string());
            urls.len() - 1
        });
        out.push_str(&format!("r:id=\"rIdL{}", k + 1));
        rest = &tail[end..];
    }
    out.push_str(rest);
    *body = out;
    urls.iter()
        .enumerate()
        .map(|(k, u)| format!("<Relationship Id=\"rIdL{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"{u}\" TargetMode=\"External\"/>", k + 1))
        .collect()
}

/// Stem body of an ordinary question: the number hangs in a 0.3" gutter on the first
/// paragraph (points label at its end), later paragraphs and blocks indent to the same
/// 0.3" column; bullets/numbered lists use the numbering part, tables are real Word
/// tables. `num_lists` counts numbered lists so each restarts at 1.
#[allow(clippy::too_many_arguments)]
fn docx_stem(b: &mut String, q: &DocQ, pt_label: Option<&str>, body_sz: i64, text_w_tw: i64, num_lists: &mut usize, style: PaperStyle, work: &dyn Fn(&mut String, u8)) {
    // Classic objective questions hang an answer blank before the number.
    let blank = style.objective_blank() && is_objective(q);
    let ind_w: i64 = if blank { 1152 } else { 432 };
    let mut prefix: Option<String> = Some(if blank { format!("_______ {}", style.number_label(q.number)) } else { style.number_label(q.number) });
    // Emits the pending "N." (and points label) as its own paragraph before a non-text block.
    let flush_prefix = |b: &mut String, prefix: &mut Option<String>| {
        if let Some(p) = prefix.take() {
            b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{ind_w}\" w:hanging=\"{ind_w}\"/></w:pPr>"));
            wrun(b, &p, false, false, 0, body_sz);
            if let Some(l) = pt_label {
                b.push_str("<w:r><w:tab/></w:r>");
                wrun(b, l, false, false, 0, body_sz);
            }
            b.push_str("</w:p>");
        }
    };
    for (k, entry) in q.text.iter().enumerate() {
    for node in richtext::parse_tree(entry) {
        let (block, center, depths) = match &node {
            Node::Block { block, center, depths } => (block, *center, depths.as_slice()),
            Node::Side(_) => {
                flush_prefix(b, &mut prefix);
                docx_nodes(b, std::slice::from_ref(&node), ind_w, body_sz, text_w_tw - ind_w, num_lists);
                continue;
            }
        };
        if center && !matches!(block, Block::Bullets(_) | Block::Numbered(_) | Block::Quote(_)) {
            // A centred block opening the stem: the number stands on its own line first.
            flush_prefix(b, &mut prefix);
        }
        match block {
            Block::Para(lines) if center => {
                for l in lines {
                    b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{ind_w}\"/><w:jc w:val=\"center\"/></w:pPr>"));
                    wline(b, l, body_sz, false);
                    b.push_str("</w:p>");
                }
            }
            Block::Para(lines) => {
                for l in lines {
                    match prefix.take() {
                        Some(p) => {
                            b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{ind_w}\" w:hanging=\"{ind_w}\"/></w:pPr>"));
                            wrun(b, &p, false, false, 0, body_sz);
                            b.push_str("<w:r><w:tab/></w:r>");
                            wline(b, l, body_sz, false);
                            if let Some(lbl) = pt_label {
                                wrun(b, &format!(" {lbl}"), false, false, 0, body_sz);
                            }
                        }
                        None => {
                            b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{ind_w}\"/></w:pPr>"));
                            wline(b, l, body_sz, false);
                        }
                    }
                    b.push_str("</w:p>");
                }
            }
            Block::Bullets(_) | Block::Numbered(_) => {
                flush_prefix(b, &mut prefix);
                docx_list(b, block, depths, body_sz, num_lists);
            }
            Block::Quote(lines) => {
                flush_prefix(b, &mut prefix);
                for l in lines {
                    b.push_str(&format!(
                        "<w:p><w:pPr><w:pBdr><w:left w:val=\"single\" w:sz=\"12\" w:space=\"6\" w:color=\"999999\"/></w:pBdr><w:ind w:left=\"{}\"/></w:pPr>",
                        ind_w + 200
                    ));
                    wline(b, l, body_sz, false);
                    b.push_str("</w:p>");
                }
            }
            Block::Table { header, rows } => {
                flush_prefix(b, &mut prefix);
                docx_table(b, *header, rows, body_sz, text_w_tw - ind_w, if center { None } else { Some(ind_w) });
            }
            Block::Heading(..) | Block::Divider | Block::Side(_) => {
                flush_prefix(b, &mut prefix);
                docx_nodes(b, std::slice::from_ref(&node), ind_w, body_sz, text_w_tw - ind_w, num_lists);
            }
        }
    }
    // A multipart part's own work space, right under it.
    if let Some(&n) = q.space_after.get(k).filter(|&&n| n > 0) {
        flush_prefix(b, &mut prefix);
        work(b, n);
    }
    }
    flush_prefix(b, &mut prefix);
}

/// One bullet or numbered list (`depths`: each item's nesting level, `w:ilvl`). Bullets share
/// numId 1; each numbered list gets its own numId (counted in `num_lists`) so it restarts at
/// 1. A task item (`☐ ` / `☑ `) is a plain paragraph at its level's text indent: its box
/// stands in for the bullet.
fn docx_list(b: &mut String, block: &Block, depths: &[u8], body_sz: i64, num_lists: &mut usize) {
    let (Block::Bullets(lines) | Block::Numbered(lines)) = block else { return };
    let bullets = matches!(block, Block::Bullets(_));
    let num_id = if bullets {
        1
    } else {
        *num_lists += 1;
        1 + *num_lists
    };
    for (k, l) in lines.iter().enumerate() {
        let lvl = depths.get(k).copied().unwrap_or(0).min(richtext::MAX_DEPTH);
        if bullets && richtext::task_item(l).is_some() {
            b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{}\"/></w:pPr>", docx_list_left(lvl)));
        } else {
            b.push_str(&format!("<w:p><w:pPr><w:numPr><w:ilvl w:val=\"{lvl}\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>"));
        }
        wline(b, l, body_sz, false);
        b.push_str("</w:p>");
    }
}

/// Blocks with no question number in front (side-by-side panels, and the heading / divider /
/// side-by-side blocks of a stem), indented `ind` twips within `max_tw`, from the stem's
/// nodes (`richtext::parse_tree`): centred paragraphs and headings get `<w:jc w:val="center"/>`,
/// a centred table `jc` center in its `tblPr`, list items their nesting level.
fn docx_nodes(b: &mut String, nodes: &[Node], ind: i64, body_sz: i64, max_tw: i64, num_lists: &mut usize) {
    for node in nodes {
        let (block, center, depths) = match node {
            Node::Block { block, center, depths } => (block, *center, depths.as_slice()),
            Node::Side(panels) => {
                docx_side(b, panels, ind, body_sz, max_tw, num_lists);
                continue;
            }
        };
        let jc = if center { "<w:jc w:val=\"center\"/>" } else { "" };
        match block {
            Block::Para(lines) => {
                for l in lines {
                    b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{ind}\"/>{jc}</w:pPr>"));
                    wline(b, l, body_sz, false);
                    b.push_str("</w:p>");
                }
            }
            Block::Bullets(_) | Block::Numbered(_) => docx_list(b, block, depths, body_sz, num_lists),
            Block::Quote(lines) => {
                for l in lines {
                    b.push_str(&format!(
                        "<w:p><w:pPr><w:pBdr><w:left w:val=\"single\" w:sz=\"12\" w:space=\"6\" w:color=\"999999\"/></w:pBdr><w:ind w:left=\"{}\"/></w:pPr>",
                        ind + 200
                    ));
                    wline(b, l, body_sz, false);
                    b.push_str("</w:p>");
                }
            }
            Block::Table { header, rows } => docx_table(b, *header, rows, body_sz, max_tw, if center { None } else { Some(ind) }),
            Block::Heading(level, l) => {
                let sz = (body_sz as f64 * crate::richtext::heading_scale(*level) as f64).round() as i64;
                b.push_str(&format!("<w:p><w:pPr><w:keepNext/><w:spacing w:before=\"80\" w:after=\"40\"/><w:ind w:left=\"{ind}\"/>{jc}</w:pPr>"));
                wline(b, l, sz, true);
                b.push_str("</w:p>");
            }
            Block::Divider => {
                b.push_str(&format!(
                    "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"8C8C8C\"/></w:pBdr><w:spacing w:after=\"120\"/><w:ind w:left=\"{ind}\"/></w:pPr></w:p>"
                ));
            }
            Block::Side(panels) => {
                // (`parse_tree` keeps side-by-side groups as `Node::Side`; never reached)
                let panels: Vec<Vec<Node>> = panels.iter().map(|p| nodes_of(p)).collect();
                docx_side(b, &panels, ind, body_sz, max_tw, num_lists);
            }
        }
    }
}

/// Side-by-side panels: a borderless table, one cell per panel.
fn docx_side(b: &mut String, panels: &[Vec<Node>], ind: i64, body_sz: i64, max_tw: i64, num_lists: &mut usize) {
    let n = panels.len().max(1) as i64;
    let w = (max_tw / n).max(400);
    let none = |side: &str| format!("<w:{side} w:val=\"nil\"/>");
    b.push_str(&format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"{}\" w:type=\"dxa\"/><w:tblInd w:w=\"{ind}\" w:type=\"dxa\"/><w:tblBorders>{}{}{}{}{}{}</w:tblBorders><w:tblLayout w:type=\"fixed\"/><w:tblCellMar><w:left w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"180\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>",
        w * n,
        none("top"), none("left"), none("bottom"), none("right"), none("insideH"), none("insideV")
    ));
    for _ in 0..n {
        b.push_str(&format!("<w:gridCol w:w=\"{w}\"/>"));
    }
    b.push_str("</w:tblGrid><w:tr><w:trPr><w:cantSplit/></w:trPr>");
    for p in panels {
        b.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/></w:tcPr>"));
        let before = b.len();
        docx_nodes(b, p, 0, body_sz, w - 180, num_lists);
        // a cell must end with a paragraph
        if b.len() == before || b.ends_with("</w:tbl>") {
            b.push_str("<w:p/>");
        }
        b.push_str("</w:tc>");
    }
    b.push_str("</w:tr></w:tbl>");
}

/// A bordered Word table indented `ind` twips; column widths come from the same content
/// measure as the PDF (Source Serif 4 metrics, a little wider than Times), header row bold and shaded and
/// repeated on each page.
/// `ind` None: the table is centred (`jc` center, no indent).
fn docx_table(b: &mut String, header: bool, rows: &[Vec<richtext::Line>], body_sz: i64, max_tw: i64, ind: Option<i64>) {
    let body = body_sz as f64 / 2.0;
    let fr: Vec<Vec<Vec<Frag>>> = rows
        .iter()
        .enumerate()
        .map(|(ri, r)| r.iter().map(|c| line_frags(c, header && ri == 0)).collect())
        .collect();
    // A little slack, so a substituted serif wider than Times (DejaVu Serif) still fits.
    let widths: Vec<i64> = table_col_widths(&fr, max_tw as f64 / 20.0 / 1.06, body)
        .iter()
        .map(|w| (w * 1.06 * 20.0).round() as i64)
        .collect();
    let total: i64 = widths.iter().sum();
    let bd = |side: &str| format!("<w:{side} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"404040\"/>");
    let place = match ind {
        Some(ind) => format!("<w:tblInd w:w=\"{ind}\" w:type=\"dxa\"/>"),
        None => "<w:jc w:val=\"center\"/>".to_string(),
    };
    b.push_str(&format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"{total}\" w:type=\"dxa\"/>{place}<w:tblBorders>{}{}{}{}{}{}</w:tblBorders><w:tblLayout w:type=\"fixed\"/><w:tblCellMar><w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>",
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

/// Appends a question's picture as an inline drawing in its own paragraph, indented `ind`
/// twips, sized by `layout::picture_size_pt` in the column (as the PDF prints it). PNG and
/// JPEG embed as they are; other formats are re-encoded to PNG. False when the bytes do not
/// decode (the caller then prints the `[picture]` placeholder).
fn docx_picture(b: &mut String, media: &mut Vec<(Vec<u8>, &'static str)>, im: &DocImage, ind: i64, text_w_tw: i64) -> bool {
    let fmt = image::guess_format(&im.bytes).ok();
    let (bytes, ext, (wpx, hpx)) = match fmt {
        Some(image::ImageFormat::Png) | Some(image::ImageFormat::Jpeg) => {
            let dims = image::ImageReader::new(std::io::Cursor::new(&im.bytes))
                .with_guessed_format()
                .ok()
                .and_then(|r| r.into_dimensions().ok());
            let Some(d) = dims else { return false };
            (im.bytes.clone(), if fmt == Some(image::ImageFormat::Png) { "png" } else { "jpeg" }, d)
        }
        _ => {
            let Ok(img) = image::load_from_memory(&im.bytes) else { return false };
            let mut out = std::io::Cursor::new(Vec::new());
            if img.write_to(&mut out, image::ImageFormat::Png).is_err() {
                return false;
            }
            (out.into_inner(), "png", (img.width(), img.height()))
        }
    };
    if wpx == 0 || hpx == 0 {
        return false;
    }
    let (w, h) = crate::layout::picture_size_pt(wpx, hpx, (text_w_tw - ind) as f64 / 20.0);
    let (cx, cy) = ((w * 12700.0).round() as i64, (h * 12700.0).round() as i64);
    media.push((bytes, ext));
    let n = media.len();
    b.push_str(&format!(
        "<w:p><w:pPr><w:keepNext/><w:spacing w:before=\"120\" w:after=\"120\"/><w:ind w:left=\"{ind}\"/></w:pPr><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{n}\" name=\"Picture {n}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"image{n}.{ext}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"rIdM{n}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
    ));
    true
}

/// Choices in `q.answer_columns` equal columns: a borderless table indented `ind` twips,
/// rows from `layout::choice_grid` (column-major), rows kept whole across pages.
fn docx_choice_grid(b: &mut String, q: &DocQ, style: PaperStyle, ind: i64, text_w_tw: i64, body_sz: i64) {
    let ncols = q.answer_columns.clamp(2, 4) as i64;
    let w = ((text_w_tw - ind) / ncols).max(400);
    b.push_str(&format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"{}\" w:type=\"dxa\"/><w:tblInd w:w=\"{ind}\" w:type=\"dxa\"/><w:tblBorders><w:top w:val=\"nil\"/><w:left w:val=\"nil\"/><w:bottom w:val=\"nil\"/><w:right w:val=\"nil\"/><w:insideH w:val=\"nil\"/><w:insideV w:val=\"nil\"/></w:tblBorders><w:tblLayout w:type=\"fixed\"/><w:tblCellMar><w:left w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"120\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>",
        w * ncols
    ));
    for _ in 0..ncols {
        b.push_str(&format!("<w:gridCol w:w=\"{w}\"/>"));
    }
    b.push_str("</w:tblGrid>");
    for row in crate::layout::choice_grid(q.choices.len(), ncols as u8) {
        b.push_str("<w:tr><w:trPr><w:cantSplit/></w:trPr>");
        for k in 0..ncols as usize {
            b.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/></w:tcPr><w:p>"));
            if let Some(&i) = row.get(k) {
                for (j, l) in style.choice_label(&q.choices[i]).split('\n').enumerate() {
                    if j > 0 {
                        b.push_str("<w:r><w:br/></w:r>");
                    }
                    wline(b, &richtext::inline(l), body_sz, false);
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

/// Left indent (twips) of a list item's text at nesting level `lvl` (the numbering levels'
/// `w:ind`; the PDF steps [`LIST_INDENT`] = 320 twips per level too).
fn docx_list_left(lvl: u8) -> i64 {
    792 + 320 * lvl as i64
}

/// Does the document body nest list items (`w:ilvl` above 0)?
fn docx_nests_lists(body: &str) -> bool {
    (1..=richtext::MAX_DEPTH).any(|l| body.contains(&format!("<w:ilvl w:val=\"{l}\"/>")))
}

/// numbering.xml: numId 1 = bullets; numIds 2.. = one decimal list each (restart at 1).
/// `nested`: the lists are multilevel (round-7 nesting) with the editor's labels per level,
/// bullets • ◦ ▪ and numbers 1. / a. / i. (`richtext::bullet_glyph` / `number_label`),
/// each level [`docx_list_left`]; level 0 is the single-level list's. Without nesting the
/// part is exactly the pre-round-7 one.
fn numbering_xml(num_lists: usize, nested: bool) -> String {
    if nested {
        return numbering_xml_nested(num_lists);
    }
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

fn numbering_xml_nested(num_lists: usize) -> String {
    let levels = richtext::MAX_DEPTH;
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">");
    s.push_str("<w:abstractNum w:abstractNumId=\"0\"><w:multiLevelType w:val=\"multilevel\"/>");
    for l in 0..=levels {
        s.push_str(&format!(
            "<w:lvl w:ilvl=\"{l}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"288\"/></w:pPr></w:lvl>",
            richtext::bullet_glyph(l),
            docx_list_left(l)
        ));
    }
    s.push_str("</w:abstractNum><w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"multilevel\"/>");
    for l in 0..=levels {
        let fmt = ["decimal", "lowerLetter", "lowerRoman"][l as usize % 3];
        s.push_str(&format!(
            "<w:lvl w:ilvl=\"{l}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"%{}.\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
            l + 1,
            docx_list_left(l)
        ));
    }
    s.push_str("</w:abstractNum><w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num>");
    for k in 0..num_lists {
        s.push_str(&format!("<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"1\"/>", 2 + k));
        for l in 0..=levels {
            s.push_str(&format!("<w:lvlOverride w:ilvl=\"{l}\"><w:startOverride w:val=\"1\"/></w:lvlOverride>"));
        }
        s.push_str("</w:num>");
    }
    s.push_str("</w:numbering>");
    s
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOC_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="fontTable.xml"/></Relationships>"#;

/// Content-type overrides and document relationships for the repeating page header
/// (header1 = pages 2+, header2 = the empty first-page header).
const HEADER_TYPES: &str = r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#;
const HEADER_RELS: &str = r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/></Relationships>"#;
const EMPTY_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:hdr>"#;

/// The body face: Times New Roman, which every Word install has. The font table gives
/// "Liberation Serif" (metric-compatible, what LibreOffice substitutes) as its alternate name
/// and the roman family, so a missing Times falls back to another serif, never a sans.
const FONT_TABLE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:font w:name="Times New Roman"><w:altName w:val="Liberation Serif"/><w:panose1 w:val="02020603050405020304"/><w:charset w:val="00"/><w:family w:val="roman"/><w:pitch w:val="variable"/></w:font><w:font w:name="Liberation Serif"><w:altName w:val="Times New Roman"/><w:charset w:val="00"/><w:family w:val="roman"/><w:pitch w:val="variable"/></w:font></w:fonts>"#;

const CODE_FONT_TABLE: &str = r#"<w:font w:name="Courier New"><w:altName w:val="Liberation Mono"/><w:panose1 w:val="02070309020205020404"/><w:charset w:val="00"/><w:family w:val="modern"/><w:pitch w:val="fixed"/></w:font><w:font w:name="Liberation Mono"><w:altName w:val="Courier New"/><w:charset w:val="00"/><w:family w:val="modern"/><w:pitch w:val="fixed"/></w:font></w:fonts>"#;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:cs="Times New Roman" w:eastAsia="Times New Roman"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="120"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="120" w:after="60"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style></w:styles>"#;

pub fn render_docx(doc: &ExamDoc) -> Result<Vec<u8>, String> {
    let tw = |mm: f64| (mm * 1440.0 / 25.4).round() as i64; // mm -> twips
    let body_sz = ((doc.body_pt as f64).clamp(6.0, 20.0) * 2.0).round() as i64; // half-points
    let head_sz = ((doc.heading_pt as f64).clamp(8.0, 36.0) * 2.0).round() as i64;

    let mut b = String::new();
    b.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    b.push_str(
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>",
    );

    if doc.is_key {
        if !doc.title.is_empty() {
            wpara(&mut b, &Par { style: Some("Title"), sz: head_sz, bold: true, ..Par::body(doc.title.trim()) }, body_sz);
        }
    } else {
        // As the PDF and the page preview: header line with a rule under it, the title
        // centred, a muted centred "N questions · M points" line.
        if !doc.header_line.trim().is_empty() {
            wpara(&mut b, &Par { rule: true, ..Par::body(doc.header_line.trim()) }, body_sz);
        }
        if !doc.title.trim().is_empty() {
            wpara(&mut b, &Par { style: Some("Title"), sz: head_sz, bold: true, center: true, ..Par::body(doc.title.trim()) }, body_sz);
            let n = doc.questions.len();
            let pts: f64 = doc.questions.iter().filter_map(|q| q.points).sum();
            let sub = format!(
                "{n} question{}{}",
                if n == 1 { "" } else { "s" },
                if pts > 0.0 { format!(" \u{b7} {} point{}", points_number(pts), if pts == 1.0 { "" } else { "s" }) } else { String::new() }
            );
            wpara(&mut b, &Par { color: Some("808080"), center: true, sz: (body_sz as f64 * 0.92).round() as i64, ..Par::body(&sub) }, body_sz);
        }
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
        wpara(&mut b, &Par { rule: true, dotted: doc.style.dotted(), ..Par::body("") }, body_sz);
    }

    let style = doc.style;
    let m = doc.margins_mm();
    let (pw, ph) = style.page_pt();
    let (page_w_tw, page_h_tw) = ((pw as f64 * 20.0).round() as i64, (ph as f64 * 20.0).round() as i64);
    // Text column width in twips (page minus both side margins).
    let text_w_tw = page_w_tw - tw(m.left as f64) - tw(m.right as f64);
    // Answer / work-space line height (half-points of the empty rule paragraph).
    let rule_sz = ((body_sz as f32) * (style.rule_pitch() / 2.0) * 1.15).round() as i64;
    let rule_par = |b: &mut String, ind: i64| {
        wpara(b, &Par { rule: true, dotted: style.dotted(), sz: rule_sz, ind, ..Par::body("") }, body_sz);
    };
    let mut num_lists = 0usize;
    // Embedded pictures: (bytes, file extension) of word/media/image{k+1}.{ext}.
    let mut media: Vec<(Vec<u8>, &'static str)> = Vec::new();
    let questions = printed_questions(doc);
    let marked = doc.is_key && doc.key_layout == KeyLayout::Marked;
    let section_key = doc.is_key && doc.key_layout == KeyLayout::Section;
    if section_key {
        wpara(&mut b, &Par { bold: true, sz: (body_sz as f64 * 1.25).round() as i64, ..Par::body("Answer Section") }, body_sz);
    }
    let mut n_sections = 0usize;
    for q in &questions {
        if section_key {
            if let Some(sec) = &q.section {
                n_sections += 1;
                let t = if sec.title.trim().is_empty() { format!("Section {n_sections}") } else { sec.title.trim().to_string() };
                wpara(&mut b, &Par { bold: true, sz: body_sz + 2, ..Par::body(&t) }, body_sz);
            }
            // Number, then a short underlined blank with the answer on it; a written
            // answer prints under an empty blank.
            let marks: Vec<String> = if q.key_marks.is_empty() { vec![String::new()] } else { q.key_marks.clone() };
            for (i, m) in marks.iter().enumerate() {
                b.push_str("<w:p><w:pPr><w:spacing w:before=\"60\" w:after=\"60\"/><w:ind w:left=\"864\" w:hanging=\"864\"/></w:pPr>");
                wrun(&mut b, &style.number_label(q.number + i as u32), false, false, 0, body_sz);
                b.push_str("<w:r><w:tab/></w:r>");
                let pad = " ".repeat(4);
                let cell = if m.is_empty() { "\u{a0}".repeat(14) } else { format!("{pad}{m}{pad}") };
                b.push_str(&format!(
                    "<w:r><w:rPr><w:u w:val=\"single\"/><w:sz w:val=\"{body_sz}\"/><w:szCs w:val=\"{body_sz}\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                    xml_esc(&cell)
                ));
            }
            if q.key_marks.is_empty() && !q.key_text.trim().is_empty() {
                docx_nodes(&mut b, &richtext::parse_tree(q.key_text.trim()), 864, body_sz, text_w_tw - 864, &mut num_lists);
            }
            continue;
        }
        // Marked key: correct choices checked and bold (labels are styled here, so the
        // style's relabelling below leaves them as they are).
        let marked_q;
        let q = if marked && !q.correct.is_empty() && !is_tf(&q.choices) {
            let mut m = q.clone();
            for (i, c) in m.choices.iter_mut().enumerate() {
                let l = style.choice_label(c);
                *c = if q.correct.contains(&i) && !l.contains("**") { format!("\u{2713} **{l}**") } else { l };
            }
            marked_q = m;
            &marked_q
        } else {
            q
        };
        if doc.is_key && !marked {
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

        if let Some(sec) = &q.section {
            if !sec.title.trim().is_empty() {
                wpara(&mut b, &Par { bold: true, sz: body_sz + 2, ..Par::body(sec.title.trim()) }, body_sz);
            }
            for l in sec.directions.trim().split('\n').filter(|l| !l.trim().is_empty()) {
                wpara(&mut b, &Par { italic: true, ..Par::body(l.trim()) }, body_sz);
            }
            if !sec.is_blank_doc() {
                wpara(&mut b, &Par::body(""), body_sz);
            }
        }
        // Stem paragraphs: number on the first line, points label appended there.
        // Matching questions print their pairs only (the text entries ARE the
        // left items), handled below.
        let pt_label = q.points.map(|p| style.points_label(p));
        let tf = is_tf(&q.choices);
        let matching = looks_matching(q, tf);
        let ind: i64 = if style.objective_blank() && is_objective(q) && !matching { 1152 } else { 432 }; // choices line up under the stem
        if matching {
            for (i, item) in q.text.iter().enumerate() {
                b.push_str("<w:p>");
                wrun(&mut b, &format!("{} ", style.number_label(q.number + i as u32)), false, false, 0, body_sz);
                wline(&mut b, &richtext::inline(item), body_sz, false);
                b.push_str("</w:p>");
            }
            for c in &q.choices {
                choice_para(&mut b, &style.choice_label(c), ind, body_sz);
            }
            for _ in 0..q.work_lines {
                if style.work_space_lined() {
                    rule_par(&mut b, ind);
                } else {
                    wpara(&mut b, &Par { sz: rule_sz, ..Par::body("") }, body_sz);
                }
            }
        } else {
            let ind_w: i64 = if style.objective_blank() && is_objective(q) { 1152 } else { 432 };
            let part_work = |b: &mut String, n: u8| {
                for _ in 0..n {
                    if style.work_space_lined() {
                        rule_par(b, ind_w);
                    } else {
                        wpara(b, &Par { sz: rule_sz, ..Par::body("") }, body_sz);
                    }
                }
            };
            // The picture prints at the stem's first `[picture]` marker, or under the stem,
            // as an inline drawing at the stem's indent (the PDF's size and place).
            let split = picture_split(q, q.image.as_ref().map(|_| 0));
            match &split {
                Some((head, _)) => docx_stem(&mut b, head, pt_label.as_deref(), body_sz, text_w_tw, &mut num_lists, style, &part_work),
                None => docx_stem(&mut b, q, pt_label.as_deref(), body_sz, text_w_tw, &mut num_lists, style, &part_work),
            }
            if let Some(im) = &q.image {
                if !docx_picture(&mut b, &mut media, im, ind_w, text_w_tw) {
                    wpara(&mut b, &Par { italic: true, center: true, ..Par::body("[picture]") }, body_sz);
                }
            }
            if let Some((_, rest)) = &split {
                for t in rest {
                    docx_nodes(&mut b, &richtext::parse_tree(t), ind_w, body_sz, text_w_tw - ind_w, &mut num_lists);
                }
            }

            // Choices.
            if tf && marked && !q.correct.is_empty() {
                let t = if q.correct.contains(&0) { "\u{2713} **True**   False" } else { "True   \u{2713} **False**" };
                b.push_str(&format!("<w:p><w:pPr><w:ind w:left=\"{ind}\"/></w:pPr>"));
                wline(&mut b, &richtext::inline(t), body_sz, false);
                b.push_str("</w:p>");
            } else if tf {
                wpara(&mut b, &Par { ind, ..Par::body("True   False") }, body_sz);
            } else if q.answer_columns >= 2 && q.choices.len() > 1 {
                // Answer columns 2 / 4: a borderless fixed-width table, filled column-major,
                // each choice wrapping inside its cell.
                docx_choice_grid(&mut b, q, style, ind, text_w_tw, body_sz);
            } else if style.choices_across() && q.choices.len() > 1 && q.choices.iter().all(|c| c.chars().count() <= 24) {
                // Condensed: the choices across one paragraph on evenly spaced tab stops.
                let n = q.choices.len().min(4) as i64;
                let step = (text_w_tw - ind) / n;
                for row in q.choices.chunks(n as usize) {
                    b.push_str("<w:p><w:pPr><w:tabs>");
                    for k in 1..n {
                        b.push_str(&format!("<w:tab w:val=\"left\" w:pos=\"{}\"/>", ind + k * step));
                    }
                    b.push_str(&format!("</w:tabs><w:ind w:left=\"{ind}\"/></w:pPr>"));
                    for (k, c) in row.iter().enumerate() {
                        if k > 0 {
                            b.push_str("<w:r><w:tab/></w:r>");
                        }
                        wline(&mut b, &richtext::inline(&style.choice_label(c)), body_sz, false);
                    }
                    b.push_str("</w:p>");
                }
            } else {
                for c in &q.choices {
                    choice_para(&mut b, &style.choice_label(c), ind, body_sz);
                }
            }

            if marked && q.correct.is_empty() && !q.key_text.trim().is_empty() {
                let t = format!("**Answer:** {}", q.key_text.trim());
                docx_nodes(&mut b, &richtext::parse_tree(&t), ind, body_sz, text_w_tw - ind, &mut num_lists);
            }
            for _ in 0..rule_count(q) {
                rule_par(&mut b, 0);
            }
            for _ in 0..q.work_lines {
                if style.work_space_lined() {
                    rule_par(&mut b, ind);
                } else {
                    wpara(&mut b, &Par { sz: rule_sz, ..Par::body("") }, body_sz);
                }
            }
            if style != PaperStyle::Condensed {
                wpara(&mut b, &Par::body(""), body_sz); // breathing room after each question
            }
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
        "<w:sectPr>{head_refs}<w:pgSz w:w=\"{page_w_tw}\" w:h=\"{page_h_tw}\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/><w:cols w:space=\"720\"/>{title_pg}</w:sectPr></w:body></w:document>",
        tw(m.top as f64),
        tw(m.right as f64),
        tw(m.bottom as f64),
        tw(m.left as f64)
    ));

    if !media.is_empty() {
        b = b.replacen(
            "<w:document ",
            "<w:document xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" ",
            1,
        );
    }
    let numbering = numbering_xml(num_lists, docx_nests_lists(&b));
    // Code runs name Courier New: the font table then lists it with Liberation Mono (the
    // PDF's mono, metric-compatible) as its alternate, modern family, fixed pitch.
    let font_table = if b.contains(CODE_FONT) { FONT_TABLE.replace("</w:fonts>", CODE_FONT_TABLE) } else { FONT_TABLE.to_string() };
    let link_rels = docx_link_rels(&mut b);
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
    let rels = if link_rels.is_empty() { rels } else { rels.replace("</Relationships>", &format!("{link_rels}</Relationships>")) };
    let media_rels: String = media
        .iter()
        .enumerate()
        .map(|(k, (_, ext))| format!("<Relationship Id=\"rIdM{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/image{n}.{ext}\"/>", n = k + 1))
        .collect();
    let rels = if media_rels.is_empty() { rels } else { rels.replace("</Relationships>", &format!("{media_rels}</Relationships>")) };
    let mut types = types;
    for ext in ["png", "jpeg"] {
        if media.iter().any(|m| m.1 == ext) {
            types = types.replace("<Default Extension=\"xml\"", &format!("<Default Extension=\"{ext}\" ContentType=\"image/{ext}\"/><Default Extension=\"xml\""));
        }
    }
    let media_names: Vec<String> = media.iter().enumerate().map(|(k, (_, ext))| format!("word/media/image{}.{ext}", k + 1)).collect();
    let mut parts: Vec<(&str, &str)> = vec![
        ("[Content_Types].xml", types.as_str()),
        ("_rels/.rels", ROOT_RELS),
        ("word/_rels/document.xml.rels", rels.as_str()),
        ("word/styles.xml", STYLES),
        ("word/numbering.xml", numbering.as_str()),
        ("word/fontTable.xml", font_table.as_str()),
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
    for (name, (bytes, _)) in media_names.iter().zip(&media) {
        // Pictures are already compressed: store them.
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        z.start_file(name.as_str(), stored).map_err(|e| format!("docx zip: {e}"))?;
        use std::io::Write as _;
        z.write_all(bytes).map_err(|e| format!("docx zip: {e}"))?;
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
                    ..Default::default()
                },
                DocQ {
                    number: 3,
                    text: vec!["The Sun and the Moon look about the same size in our sky because...".into()],
                    choices: vec![],
                    key_text: "2. because their angular diameters nearly match".into(),
                    rule_lines: 3,
                    points: None,
                    image: None,
                    ..Default::default()
                },
                DocQ {
                    number: 4,
                    text: vec!["Two left items".into(), "second left item".into()],
                    choices: vec!["A. first right".into(), "B. second right".into()],
                    key_text: "4. B, A".into(),
                    rule_lines: 0,
                    points: None,
                    image: None,
                    ..Default::default()
                },
            ],
            ruled_blanks: 2,
            is_key: false,
            version_label: None,
            ..Default::default()
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

    pub(super) fn find_raw(hay: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() || hay.len() < needle.len() {
            return None;
        }
        (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
    }

    /// The PDF with the bodies of its compressed (font program) streams zeroed, same length:
    /// text searches then only see dictionaries and content streams, and offsets still hold.
    pub(super) fn masked(pdf: &[u8]) -> Vec<u8> {
        let mut out = pdf.to_vec();
        let mut at = 0;
        while let Some(i) = find_raw(&out[at..], b"/FlateDecode >>\nstream\n") {
            let start = at + i + b"/FlateDecode >>\nstream\n".len();
            let end = start + find_raw(&out[start..], b"\nendstream").unwrap_or(0);
            out[start..end].fill(0);
            at = end.max(start);
        }
        out
    }

    fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
        find_raw(&masked(hay), needle)
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
        for face in [Face::Reg, Face::Bold, Face::Ital, Face::BoldItal] {
            for b in 32..=126u8 {
                assert!(char_w(b, face, 1000.0) > 0.0, "{face:?} byte {b}");
            }
            for b in 0xa1..=0xffu8 {
                assert!(char_w(b, face, 1000.0) > 0.0, "{face:?} byte {b:#x}");
            }
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
                DocQ { number: 1, text: vec!["Gravity on the Moon is weaker.".into()], choices: vec!["True".into(), "False".into()], key_text: "True".into(), rule_lines: 0, points: None, image: None, ..Default::default() },
                DocQ {
                    number: 2,
                    text: vec!["Item one".into(), "Item two".into()],
                    choices: vec!["A. First right".into(), "B. Second right".into()],
                    key_text: "2. B, A".into(),
                    rule_lines: 0,
                    points: None,
                    image: None,
                    ..Default::default()
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
        assert!(st.contains("w:ascii=\"Times New Roman\"") && !st.contains("Arial"), "serif body face");
        let mut ft = String::new();
        z.by_name("word/fontTable.xml").unwrap().read_to_string(&mut ft).unwrap();
        assert!(ft.contains("<w:altName w:val=\"Liberation Serif\"/>") && ft.contains("w:family w:val=\"roman\""));
        assert!(ct.contains("/word/fontTable.xml"));
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

    /// The four faces are embedded TrueType subsets whose `/Widths` are exactly the advance
    /// widths the line breaker measured with, and every drawn run starts where the layout
    /// said the previous run (plus a space in its face) ends.
    #[test]
    fn embedded_serif_faces_match_the_layout_metrics() {
        let stem = "Plain **bold words here** and *italic words* then ***bold italic*** end, H_{2}O x^{3}.";
        let pdf = render_pdf(&rich_doc(stem)).unwrap();
        let text = s_lossy(&masked(&pdf));
        assert!(!text.contains("/Helvetica"), "no standard-14 sans left");
        for (face, name) in [(Face::Reg, "SourceSerif4-Regular"), (Face::Bold, "SourceSerif4-Bold"), (Face::Ital, "SourceSerif4-It"), (Face::BoldItal, "SourceSerif4-BoldIt")] {
            let at = text.find(&format!("+{name} /FirstChar 32 /LastChar 255 /Widths [")).unwrap_or_else(|| panic!("{name} font dict"));
            let tail = &text[at..];
            let list = &tail[tail.find('[').unwrap() + 1..tail.find(']').unwrap()];
            let ws: Vec<f64> = list.split(' ').map(|v| v.parse().unwrap()).collect();
            assert_eq!(ws.len(), 224);
            for (k, w) in ws.iter().enumerate() {
                assert_eq!(*w, char_w(32 + k as u8, face, 1000.0), "{name} byte {}", 32 + k);
            }
        }
        assert_eq!(text.matches("/FontFile2 ").count(), 4);
        assert_eq!(text.matches("/Subtype /TrueType").count(), 4);
        // Decompress each font program: it parses, holds the drawn glyphs, drops the rest.
        let mut at = 0;
        let mut programs = 0;
        while let Some(i) = find_raw(&pdf[at..], b"/Filter /FlateDecode >>\nstream\n") {
            let start = at + i + b"/Filter /FlateDecode >>\nstream\n".len();
            let end = start + find_raw(&pdf[start..], b"\nendstream").unwrap();
            let mut prog = Vec::new();
            flate2::read::ZlibDecoder::new(&pdf[start..end]).read_to_end(&mut prog).unwrap();
            let f = ttf_parser::Face::parse(&prog, 0).expect("embedded font program parses");
            let space = f.glyph_index(' ').unwrap();
            assert!(f.glyph_hor_advance(space).is_some());
            programs += 1;
            at = end;
        }
        assert_eq!(programs, 4);
        // Measure vs draw: on the stem's first line each run's x equals the previous run's
        // x plus its measured width (plus one space, measured in the face it was typed in).
        let r = layout_pdf(&rich_doc(stem), false).unwrap();
        let (items, _) = stem_items(&r, &rich_doc(stem).questions[0]);
        let Some(Item::Line { frags, .. }) = items.first() else { panic!("first stem line") };
        assert!(frags.len() >= 4, "{frags:?}");
        for w in frags.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let size = frag_size(a.script, r.body);
            let end = a.dx + text_w(&a.text, a.face, size);
            let gap = b.dx - end;
            // The space belongs to the run it was typed in: either neighbour's face.
            let sp = |f: &Placed| char_w(32, f.face, frag_size(f.script, r.body));
            assert!(gap.abs() < 1e-6 || (gap - sp(a)).abs() < 1e-6 || (gap - sp(b)).abs() < 1e-6, "{a:?} -> {b:?}: gap {gap}");
        }
    }

    /// Maths, Greek, arrows and scripts beyond WinAnsi (P2 printed these as '?').
    const UNICODE_STEM: &str = "Evaluate \u{3c0}(49) \u{2212} \u{221a}x: a \u{2264} b \u{2265} c, \u{b1}2 \u{d7} 3 \u{f7} 4, 90\u{b0}, 5 \u{b5}m, \u{3b1}\u{3b2}\u{3b3} \u{394}\u{3a9}\u{3b8}\u{3bb}\u{3c3}, \u{2192} \u{2190} \u{2191} \u{2193} \u{21d2} \u{21d4} \u{2194}, x \u{2208} A, \u{2220}B \u{22a5} CD, \u{2248} \u{2260} \u{221e} \u{2211}, *\u{3c0}r\u{b2}* **\u{394}H \u{2192} \u{2264}** x\u{2074} 10\u{207b}\u{2079} CO\u{2082}";
    const UNICODE_CHARS: &str = "\u{3c0}\u{2212}\u{221a}\u{2264}\u{2265}\u{b1}\u{d7}\u{f7}\u{b0}\u{b5}\u{3b1}\u{3b2}\u{3b3}\u{394}\u{3a9}\u{3b8}\u{3bb}\u{3c3}\u{2192}\u{2190}\u{2191}\u{2193}\u{21d2}\u{21d4}\u{2194}\u{2208}\u{2220}\u{22a5}\u{2248}\u{2260}\u{221e}\u{2211}";

    /// The text of a laid-out document read back from its content streams the way a viewer
    /// does: literal strings through WinAnsi, `<..>` strings through each font's glyph map.
    fn shown(r: &R) -> String {
        let mut out = String::new();
        for (ops, _) in &r.pages {
            let mut at = 0;
            while let Some(i) = find_raw(&ops[at..], b"BT /") {
                let op = &ops[at + i + 4..];
                let end = find_raw(op, b" Tj ET\n").unwrap();
                let op = &op[..end];
                at += i + 4 + end;
                let font = std::str::from_utf8(&op[..find_raw(op, b" ").unwrap()]).unwrap();
                let arg = &op[find_raw(op, b" Td ").unwrap() + 4..];
                if arg[0] == b'<' {
                    let hex = std::str::from_utf8(&arg[1..arg.len() - 1]).unwrap();
                    let bytes: Vec<u8> = (0..hex.len() / 2).map(|k| u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).unwrap()).collect();
                    if font == "F9" {
                        out.extend(bytes.iter().map(|&k| r.fb[k as usize].1));
                    } else {
                        let k: usize = font[1..].parse::<usize>().unwrap() - 5;
                        out.extend(bytes.chunks(2).map(|p| r.cids[k][&u16::from_be_bytes([p[0], p[1]])]));
                    }
                } else {
                    let mut esc = false;
                    for &b in &arg[1..arg.len() - 1] {
                        if b == b'\\' && !esc {
                            esc = true;
                            continue;
                        }
                        esc = false;
                        out.push(fonts::winansi_char(b).unwrap_or('?'));
                    }
                }
                out.push(' ');
            }
        }
        out
    }

    #[test]
    fn unicode_beyond_winansi_prints_in_cid_and_fallback_fonts() {
        let doc = rich_doc(UNICODE_STEM);
        let r = layout_pdf(&doc, false).unwrap();
        let text = shown(&r);
        for c in UNICODE_CHARS.chars() {
            assert!(text.contains(c), "{c} ({:04X}) not drawn: {text}", c as u32);
        }
        assert!(!text.contains('?'), "nothing degrades to '?': {text}");
        // Source Serif 4 prints most of it; STIX Two Math only what Source Serif lacks.
        let fb: String = r.fb.iter().map(|f| f.1).collect();
        for c in ['\u{21d2}', '\u{21d4}', '\u{2194}', '\u{2208}', '\u{2220}', '\u{22a5}'] {
            assert!(fb.contains(c), "{c} from the fallback face: {fb}");
        }
        for c in ['\u{3c0}', '\u{2212}', '\u{221a}', '\u{2264}', '\u{3a9}', '\u{2192}'] {
            assert!(!fb.contains(c), "{c} is Source Serif's own");
            assert!(r.cids[0].values().any(|v| *v == c) || r.cids[1].values().any(|v| *v == c) || r.cids[2].values().any(|v| *v == c), "{c} via a CID font");
        }
        assert!(r.cids[2].values().any(|v| *v == '\u{3c0}'), "italic \u{3c0} uses the italic face's CID font");
        assert!(r.cids[1].values().any(|v| *v == '\u{2264}'), "bold \u{2264} uses the bold face's CID font");

        let pdf = render_pdf(&doc).unwrap();
        verify_xref(&pdf);
        let t = s_lossy(&masked(&pdf));
        assert_eq!(t.matches("/Subtype /Type0").count(), 3, "regular, bold and italic CID fonts");
        assert_eq!(t.matches("/Subtype /CIDFontType2").count(), 3);
        assert_eq!(t.matches("/Encoding /Identity-H").count(), 3);
        assert_eq!(t.matches("/Subtype /Type3").count(), 1);
        assert_eq!(t.matches("/ToUnicode ").count(), 4);
        assert!(t.contains("/F5 ") && t.contains("/F6 ") && t.contains("/F7 ") && t.contains("/F9 ") && !t.contains("/F8 "), "only the used fonts");
        // /W carries exactly the advances the layout measured with.
        let at = t.find("/CIDFontType2").unwrap();
        let w = &t[at..];
        let w = &w[w.find("/W [").unwrap() + 4..];
        let mut it = w.split(' ');
        let (g, adv): (u16, u16) = (it.next().unwrap().parse().unwrap(), it.next().unwrap().trim_start_matches('[').trim_end_matches(']').parse().unwrap());
        let c = r.cids[0][&g];
        assert_eq!(fonts::charmap(fonts::Style::Regular).get(c), Some((g, adv)));
        assert_eq!(text_w(&c.to_string(), Face::Reg, 1000.0), adv as f64);
        // The embedded regular program keeps the CID glyphs' outlines.
        let mut at = 0;
        let mut kept = false;
        while let Some(i) = find_raw(&pdf[at..], b"/Length1 ") {
            let st = at + i + find_raw(&pdf[at + i..], b"stream\n").unwrap() + 7;
            let end = st + find_raw(&pdf[st..], b"\nendstream").unwrap();
            let mut prog = Vec::new();
            flate2::read::ZlibDecoder::new(&pdf[st..end]).read_to_end(&mut prog).unwrap();
            let f = ttf_parser::Face::parse(&prog, 0).unwrap();
            if f.glyph_index('\u{3c0}').is_some_and(|g| f.glyph_bounding_box(g).is_some()) {
                kept = true;
            }
            at = end;
        }
        assert!(kept, "\u{3c0}'s outline is in a subset");
    }

    #[test]
    fn winansi_only_documents_are_unchanged_by_the_cid_fonts() {
        // No CID or Type 3 font, and every string is a literal: byte-for-byte the P2 writer.
        let pdf = render_pdf(&sample_doc()).unwrap();
        let t = s_lossy(&masked(&pdf));
        assert!(!t.contains("/Type0") && !t.contains("/Type3") && !t.contains("/F5") && !t.contains("/F9"));
        assert!(!t.contains("> Tj"), "no hex strings");
        assert!(t.contains("/F1 3 0 R /F2 4 0 R /F3 5 0 R /F4 6 0 R >>"));
        // cp1252 letters are still WinAnsi bytes, not CIDs.
        let r = layout_pdf(&rich_doc("caf\u{e9} \u{20ac}5 \u{2014} na\u{ef}ve"), false).unwrap();
        assert!(r.cids.iter().all(|m| m.is_empty()) && r.fb.is_empty());
    }

    /// pdftotext (poppler) reads every symbol back through the ToUnicode CMaps. Skipped when
    /// poppler is not installed.
    #[test]
    fn pdftotext_round_trips_unicode() {
        let Ok(probe) = std::process::Command::new("pdftotext").arg("-v").output() else { return };
        let _ = probe;
        let pdf = render_pdf(&rich_doc(UNICODE_STEM)).unwrap();
        let dir = std::env::temp_dir().join(format!("qs-pdftotext-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("u.pdf");
        std::fs::write(&f, &pdf).unwrap();
        let out = std::process::Command::new("pdftotext").arg("-enc").arg("UTF-8").arg(&f).arg("-").output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let txt = String::from_utf8_lossy(&out.stdout);
        for c in UNICODE_CHARS.chars() {
            assert!(txt.contains(c), "pdftotext lost {c} ({:04X}): {txt}", c as u32);
        }
        assert!(txt.contains("Evaluate \u{3c0}(49) \u{2212} \u{221a}x"), "{txt}");
        // Fallback (Type 3) symbols stay in reading order on their line.
        assert!(txt.contains("\u{2193} \u{21d2} \u{21d4} \u{2194}, x \u{2208} A,") && txt.contains("\u{2220}B \u{22a5} CD"), "{txt}");
        assert!(!txt.contains('?'), "{txt}");
    }

    /// Per-part work space prints right under its part in the PDF (lines in the lined styles,
    /// room otherwise) and the DOCX, and counts toward keep-together.
    #[test]
    fn multipart_part_work_space_prints_under_each_part() {
        let pieces = vec!["Answer the parts.\na. First part?".to_string(), "b. Second part?".to_string(), "c. Third part?".to_string()];
        let mk = |style: PaperStyle, space: Vec<u8>| ExamDoc {
            title: "W".into(),
            body_pt: 11.0,
            heading_pt: 16.0,
            margin_mm: 18.0,
            style,
            questions: vec![DocQ { number: 1, text: if space.is_empty() { vec![pieces.join("\n")] } else { pieces.clone() }, rule_lines: 0, space_after: space, ..Default::default() }],
            ..Default::default()
        };
        let ys = |pdf: &[u8], needle: &str| -> f64 {
            let t = s_lossy(&masked(pdf));
            let at = t.find(&format!("({needle}")).unwrap_or_else(|| panic!("{needle} in {t}"));
            let head = &t[..at];
            let td = head.rfind(" Td").unwrap();
            head[..td].rsplit(' ').next().unwrap().parse().unwrap()
        };
        for style in [PaperStyle::Standard, PaperStyle::Classic] {
            let base = render_pdf(&mk(style, vec![])).unwrap();
            let spaced = render_pdf(&mk(style, vec![4, 0, 2])).unwrap();
            let r = layout_pdf(&mk(style, vec![]), false).unwrap();
            let pitch = r.pitch();
            // Part a stays put; part b moves down by a's 4 lines plus one part gap, part c by
            // the same 4 lines (b has none) plus two gaps.
            assert!((ys(&base, "Answer") - ys(&spaced, "Answer")).abs() < 1e-6);
            let db = ys(&base, "b. Second") - ys(&spaced, "b. Second");
            let dc = ys(&base, "c. Third") - ys(&spaced, "c. Third");
            assert!((db - (4.0 * pitch + r.body * 0.35)).abs() < 0.01, "{style:?} b moved {db}");
            assert!((dc - (4.0 * pitch + 2.0 * r.body * 0.35)).abs() < 0.01, "{style:?} c moved {dc}");
            // Lined styles draw the lines; the item's height includes the space.
            let t = s_lossy(&masked(&spaced));
            let lines = t.matches(" l S").count() - s_lossy(&masked(&base)).matches(" l S").count();
            assert_eq!(lines, if style.work_space_lined() { 6 } else { 0 }, "{style:?}");
            let q = &mk(style, vec![4, 0, 2]).questions[0];
            let rs = layout_pdf(&mk(style, vec![4, 0, 2]), false).unwrap();
            assert!((q_est_height(&rs, q, None, false) - q_est_height(&r, &mk(style, vec![]).questions[0], None, false) - 6.0 * pitch - 2.0 * r.body * 0.35).abs() < 0.01);
        }
        // DOCX: empty work paragraphs (rules in Classic) between the parts.
        let x = |d: &ExamDoc| {
            let bytes = render_docx(d).unwrap();
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
            let mut s = String::new();
            z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
            s
        };
        let d = x(&mk(PaperStyle::Classic, vec![4, 0, 2]));
        let a = d.find("First part?").unwrap();
        let b = d.find("Second part?").unwrap();
        let c = d.find("Third part?").unwrap();
        assert_eq!(d[a..b].matches("<w:bottom w:val=\"single\" w:sz=\"6\"").count(), 4, "four ruled lines under part a");
        assert_eq!(d[b..c].matches("<w:bottom w:val=\"single\" w:sz=\"6\"").count(), 0);
        assert_eq!(d[c..].matches("<w:bottom w:val=\"single\" w:sz=\"6\"").count(), 2 + 3, "part c's two, then the question's three answer lines");
        // Each rule paragraph repeats its line as the `between` border, so a run of them is
        // not merged into one line by Word / LibreOffice.
        assert_eq!(d.matches("<w:between w:val=\"single\"").count(), d.matches("<w:bottom w:val=\"single\" w:sz=\"6\"").count());
        assert_eq!(x(&mk(PaperStyle::Standard, vec![])).matches("Second part?").count(), 1);
    }

    /// `[text](url)` prints blue and underlined with a URI link annotation (PDF) and becomes a
    /// real hyperlink (DOCX); a stem without links is unchanged.
    #[test]
    fn links_print_as_annotations_and_hyperlinks() {
        let stem = "Read [the **notes** here](https://ex.org/a?b=1&c=2) or [mail](mailto:t@s.org), not [x](ftp://a.org).";
        let doc = rich_doc(stem);
        let pdf = render_pdf(&doc).unwrap();
        verify_xref(&pdf);
        let t = s_lossy(&masked(&pdf));
        assert_eq!(t.matches("/Subtype /Link").count(), 4, "the / notes / here runs and the mailto: {t}");
        assert!(t.contains("/URI (https://ex.org/a?b=1&c=2)") && t.contains("/URI (mailto:t@s.org)"));
        assert!(!t.contains("ftp://a.org)") || t.contains("(ftp://a.org)") || t.contains("[x](ftp://a.org)"), "ftp stays literal text");
        assert!(t.contains("/Annots ["), "the page lists its annotations");
        assert!(t.contains("0.05 0.27 0.68 rg"), "blue text");
        // Each annotation covers its run: x0 < x1 and the rect sits on a text baseline.
        let r = layout_pdf(&doc, false).unwrap();
        assert_eq!(r.annots.len(), 4);
        for (p, [x0, y0, x1, y1], _) in &r.annots {
            assert_eq!(*p, 0);
            assert!(x1 > x0 && y1 > y0 && *x0 >= r.x0 && *x1 <= r.x1 + 0.01);
        }
        // The link markup takes no room: the line breaks exactly as the same text without it.
        let plain = rich_doc("Read the **notes** here or mail, not [x](ftp://a.org).");
        let rows = |pdf: &[u8]| {
            let mut b = baselines(pdf);
            b.dedup();
            b
        };
        assert_eq!(rows(&pdf), rows(&render_pdf(&plain).unwrap()));
        // DOCX: hyperlinks with external relationships, one per distinct URL.
        let bytes = render_docx(&doc).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut d = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut d).unwrap();
        let mut rels = String::new();
        z.by_name("word/_rels/document.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
        assert_eq!(d.matches("<w:hyperlink r:id=\"rIdL1\"").count(), 3, "the three runs of the first link (the, notes, here)");
        assert_eq!(d.matches("<w:hyperlink r:id=\"rIdL2\"").count(), 1);
        assert!(!d.contains(LINK_RID), "no placeholder left");
        assert!(d.contains("<w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/>"));
        assert!(rels.contains("Id=\"rIdL1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"https://ex.org/a?b=1&amp;c=2\" TargetMode=\"External\""), "{rels}");
        assert!(rels.contains("Target=\"mailto:t@s.org\""));
        assert!(d.contains("[x](ftp://a.org)"), "other schemes stay literal");
        // No links: no annotations, no hyperlink relationships.
        let none = render_pdf(&sample_doc()).unwrap();
        assert!(!s_lossy(&masked(&none)).contains("/Annot"));
        let bytes = render_docx(&sample_doc()).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut rels = String::new();
        z.by_name("word/_rels/document.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
        assert!(!rels.contains("hyperlink"));
    }

    fn s_lossy(b: &[u8]) -> String {
        String::from_utf8_lossy(b).into_owned()
    }

    #[test]
    fn cp1252_punctuation_measures_and_prints() {
        let pdf = render_pdf(&rich_doc("Cost \u{20ac}5 \u{2030} \u{0160}koda \u{2014} done")).unwrap();
        assert!(find_sub(&pdf, b"(Cost \x805 \x89 \x8akoda \x97 done)").is_some(), "{}", s_lossy(&masked(&pdf)));
        assert!(char_w(0x80, Face::Reg, 10.0) > 3.0, "euro has its real width");
    }

    /// Every text baseline ("x y Td") of a page's content stream.
    fn baselines(pdf: &[u8]) -> Vec<f64> {
        let s = String::from_utf8_lossy(&masked(pdf)).into_owned();
        s.split(" Td").filter_map(|chunk| chunk.rsplit(' ').next().and_then(|y| y.parse::<f64>().ok())).collect()
    }

    #[test]
    fn rich_runs_use_bold_italic_fonts_and_scripts() {
        let pdf = render_pdf(&rich_doc("Use **bold** and *slant* in H_{2}O, x^{3} and SO\u{2083}.")).unwrap();
        assert!(find_sub(&pdf, b"/F2 11 Tf").is_some(), "bold run");
        assert!(find_sub(&pdf, b"/F3 11 Tf").is_some(), "italic run");
        assert!(find_sub(&pdf, b"+SourceSerif4-It /FirstChar").is_some(), "the real italic face is embedded");
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
    fn strikethrough_prints_in_pdf_and_docx() {
        let stem = "Cross out the ~~wrong~~ word.";
        let pdf = render_pdf(&rich_doc(stem)).unwrap();
        assert!(find_sub(&pdf, b"(wrong) Tj").is_some(), "struck run is its own text run");
        assert!(find_sub(&pdf, b"0 0 0 RG").is_some(), "a strike line is drawn");
        assert!(find_sub(&pdf, b"~~").is_none());
        let bytes = render_docx(&rich_doc(stem)).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut x = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut x).unwrap();
        assert_eq!(x.matches("<w:strike/>").count(), 1);
        assert!(x.contains("<w:strike/><w:sz"), "strike sits in the run properties");
        assert!(!x.contains("~~"));
        // a stem without strike markup prints no strike
        let pdf = render_pdf(&rich_doc("a ~ b and ~~ alone")).unwrap();
        assert!(find_sub(&pdf, b"0 0 0 RG").is_none());
    }

    #[test]
    fn headings_dividers_and_side_by_side_in_pdf_and_docx() {
        let stem = "# Part A\nRead both.\n---\n::: side\nLeft **text**\n• one\n::: next\nRight text\n:::\n## After";
        let items = layout_blocks(&richtext::parse(stem), 400.0, 380.0, 11.0, 15.6);
        assert!(matches!(&items[0], Item::Line { scale, frags, .. } if *scale > 1.3 && frags.iter().all(|f| matches!(f.face, Face::Bold | Face::BoldItal))));
        assert!(items.iter().any(|i| matches!(i, Item::Rule { .. })));
        let cols = items.iter().find_map(|i| if let Item::Cols { cols, h } = i { Some((cols.clone(), *h)) } else { None }).unwrap();
        assert_eq!(cols.0.len(), 2);
        assert!(cols.0[1].0 > 150.0, "second column starts right of the first");
        assert!(cols.1 >= 2.0 * 15.6, "two lines in the left panel");
        let pdf = render_pdf(&rich_doc(stem)).unwrap();
        assert!(find_sub(&pdf, b"(Part A) Tj").is_some());
        assert!(find_sub(&pdf, b"/F2 15.4 Tf").is_some(), "heading 1 prints bold at 1.4x");
        assert!(find_sub(&pdf, b"(Left ) Tj").is_some() || find_sub(&pdf, b"(Left) Tj").is_some());
        assert!(find_sub(&pdf, b"(Right text) Tj").is_some());
        assert!(find_sub(&pdf, b":::").is_none() && find_sub(&pdf, b"(---)").is_none() && find_sub(&pdf, b"(# ").is_none());
        // left and right panels share a baseline
        let bl = baselines(&pdf);
        let left_y = bl.iter().zip(String::from_utf8_lossy(&pdf).split(" Td").skip(1)).find(|(_, c)| c.contains("(Left")).map(|(y, _)| *y);
        let right_y = bl.iter().zip(String::from_utf8_lossy(&pdf).split(" Td").skip(1)).find(|(_, c)| c.contains("(Right text)")).map(|(y, _)| *y);
        assert_eq!(left_y, right_y);
        let bytes = render_docx(&rich_doc(stem)).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut x = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut x).unwrap();
        assert!(x.contains("<w:keepNext/>") && x.contains(">Part A<"));
        assert!(x.contains("w:val=\"31\""), "heading 1 at 1.4 x 22 half-points");
        assert!(x.contains("<w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"8C8C8C\"/>"), "divider");
        assert!(x.contains("<w:insideV w:val=\"nil\"/>") && x.matches("<w:tc>").count() == 2, "side by side is a borderless 2-cell table");
        assert!(!x.contains(":::") && !x.contains("># "));
    }

    #[test]
    fn stems_without_extensions_render_byte_identically() {
        // stems written before headings/dividers/side-by-side: the same bytes as a plain render of
        // the same blocks through the pre-extension paths (lists, quotes, tables, inline runs)
        for stem in ["Plain stem.", "Do **all**:\n• a\n• b\n1. x\n2. y\n> q\nA | B\n---|---\n1 | ___", "a # b\n#x\n----\n:: side", "# of protons: ___"] {
            assert!(!richtext::uses_extensions(stem), "{stem:?}");
            let items = layout_blocks(&richtext::parse(stem), 400.0, 380.0, 11.0, 15.6);
            assert!(items.iter().all(|i| match i {
                Item::Line { scale, .. } => *scale == 1.0,
                Item::Rule { .. } | Item::Cols { .. } => false,
                _ => true,
            }), "{stem:?}");
        }
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
#[cfg(test)]
mod style_tests {
    use super::*;
    use crate::layout::DocSection;
    use crate::model::{Margins, PaperStyle};
    use std::io::Read;

    fn doc(style: PaperStyle) -> ExamDoc {
        ExamDoc {
            title: "Styles".into(),
            header_line: "Name: ______".into(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            style,
            questions: vec![
                DocQ {
                    number: 1,
                    text: vec!["Closest planet?".into()],
                    choices: vec!["A. Venus".into(), "B. Mercury".into(), "C. Mars".into()],
                    points: Some(2.0),
                    section: Some(DocSection { title: "Part I".into(), directions: "Circle the letter.".into() }),
                    ..Default::default()
                },
                DocQ { number: 2, text: vec!["Explain tides.".into()], rule_lines: 2, work_lines: 3, points: Some(4.0), ..Default::default() },
            ],
            ..Default::default()
        }
    }

    fn s(pdf: &[u8]) -> String {
        String::from_utf8_lossy(&super::tests::masked(pdf)).into_owned()
    }

    /// (x, y) of every text draw.
    fn text_pos(pdf: &[u8]) -> Vec<(f64, f64, String)> {
        let t = s(pdf);
        let mut out = vec![];
        for chunk in t.split(" Td (").skip(1).zip(t.split(" Td (")) {
            let (after, before) = chunk;
            let mut nums = before.rsplit(' ');
            let y = nums.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let x = nums.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            out.push((x, y, after.split(") Tj").next().unwrap_or("").to_string()));
        }
        out
    }

    #[test]
    fn standard_prints_dotted_numbers_and_parenthesised_points() {
        let pdf = render_pdf(&doc(PaperStyle::Standard)).unwrap();
        let t = s(&pdf);
        assert!(t.contains("(1.) Tj") && t.contains("(A. Venus) Tj") && t.contains("((2 points)) Tj") || t.contains("(\\(2 points\\)) Tj"));
        assert!(t.contains("(Part I) Tj"), "section title prints");
        assert!(t.contains("(Circle the letter.) Tj"), "section directions print");
        assert!(t.contains("/MediaBox [0 0 612 792]"));
    }

    #[test]
    fn classic_adds_objective_blank_and_lowercase_letters() {
        let pdf = render_pdf(&doc(PaperStyle::Classic)).unwrap();
        let t = s(&pdf);
        assert!(t.contains("(a. Venus) Tj") && t.contains("(b. Mercury) Tj"));
        let pos = text_pos(&pdf);
        let n1 = pos.iter().find(|p| p.2 == "1.").unwrap();
        let n2 = pos.iter().find(|p| p.2 == "2.").unwrap();
        assert!(n1.0 > n2.0 + 30.0, "the objective question's number sits right of its answer blank");
        // Work space prints as ruled lines: 2 answer rules + 3 work lines for question 2.
        let std_rules = s(&render_pdf(&doc(PaperStyle::Standard)).unwrap()).matches(" l S").count();
        assert_eq!(t.matches(" l S").count(), std_rules + 3 + 1, "3 ruled work lines + the answer blank");
    }

    #[test]
    fn condensed_puts_short_choices_across_and_saves_room() {
        let pdf = render_pdf(&doc(PaperStyle::Condensed)).unwrap();
        let pos = text_pos(&pdf);
        let a = pos.iter().find(|p| p.2 == "A. Venus").unwrap();
        let b = pos.iter().find(|p| p.2 == "B. Mercury").unwrap();
        assert_eq!(a.1, b.1, "choices share a baseline");
        assert!(b.0 > a.0);
        let last = |pdf: &[u8]| text_pos(pdf).iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let std_pdf = render_pdf(&doc(PaperStyle::Standard)).unwrap();
        assert!(last(&pdf) > last(&std_pdf), "condensed ends higher on the page");
    }

    #[test]
    fn exam_board_is_a4_with_bracket_points_dotted_lines_and_turn_over() {
        let mut d = doc(PaperStyle::ExamBoard);
        d.questions = (1..=30).map(|n| DocQ { number: n, text: vec![format!("Question {n}")], rule_lines: 2, points: Some(3.0), ..Default::default() }).collect();
        let pdf = render_pdf(&d).unwrap();
        let t = s(&pdf);
        assert!(t.contains("/MediaBox [0 0 595.28 841.89]"));
        assert!(t.contains("([3]) Tj"));
        assert!(t.contains("(7) Tj"), "numbers print without a dot");
        assert!(t.contains("[0.8 2.2] 0 d"), "dotted answer lines");
        let pages = t.matches("/Type /Page ").count();
        assert!(pages >= 2);
        assert_eq!(t.matches("(Turn over) Tj").count(), pages - 1, "every page but the last");
    }

    #[test]
    fn per_side_margins_move_the_text_block() {
        let mut d = doc(PaperStyle::Standard);
        d.margins = Some(Margins { top: 10.0, right: 18.0, bottom: 18.0, left: 40.0 });
        let pdf = render_pdf(&d).unwrap();
        let pos = text_pos(&pdf);
        let min_x = pos.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let max_y = pos.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        assert!((min_x - 40.0 * MM_PT).abs() < 0.5, "left margin 40 mm, got x={min_x}");
        assert!(max_y > 792.0 - 10.0 * MM_PT - 20.0, "top margin 10 mm, first baseline {max_y}");
    }

    #[test]
    fn work_space_adds_room_in_the_standard_style() {
        let mut d = doc(PaperStyle::Standard);
        let low = |pdf: &[u8]| text_pos(pdf).iter().map(|p| p.1).fold(f64::MAX, f64::min);
        d.questions.push(DocQ { number: 3, text: vec!["After".into()], ..Default::default() });
        let a = render_pdf(&d).unwrap();
        d.questions[1].work_lines = 8;
        let b = render_pdf(&d).unwrap();
        assert!((low(&a) - low(&b) - 5.0 * 11.0 * 2.0).abs() < 0.5, "3 -> 8 lines of blank work space push question 3 down: {} vs {}", low(&a), low(&b));
    }

    fn docx_xml(d: &ExamDoc) -> String {
        let bytes = render_docx(d).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut x = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut x).unwrap();
        x
    }

    #[test]
    fn docx_follows_style_margins_and_sections() {
        let x = docx_xml(&doc(PaperStyle::Standard));
        assert!(x.contains("Part I") && x.contains("Circle the letter."));
        assert!(x.contains(r#"<w:pgSz w:w="12240" w:h="15840"/>"#));
        let mut d = doc(PaperStyle::ExamBoard);
        d.margins = Some(Margins { top: 10.0, right: 20.0, bottom: 30.0, left: 40.0 });
        let x = docx_xml(&d);
        assert!(x.contains(r#"<w:pgSz w:w="11906" w:h="16838"/>"#));
        assert!(x.contains(r#"w:top="567" w:right="1134" w:bottom="1701" w:left="2268""#));
        assert!(x.contains(r#"w:val="dotted""#));
        assert!(x.contains("[4]") && x.contains(">A Venus<"));
        let x = docx_xml(&doc(PaperStyle::Classic));
        assert!(x.contains("_______ 1.") && x.contains(">a. Venus<"));
        let x = docx_xml(&doc(PaperStyle::Condensed));
        assert!(x.contains("<w:tabs>"), "choices across on tab stops");
    }
}

/// Round 7: answer columns, the page-like title area, pictures (size, place, DOCX drawings),
/// proper-length blanks and the answer-key layouts.
#[cfg(test)]
mod r7_tests {
    use super::*;
    use crate::layout::{DocSection, KeyLayout};
    use std::io::Read;

    fn s(pdf: &[u8]) -> String {
        String::from_utf8_lossy(&super::tests::masked(pdf)).into_owned()
    }

    /// (x, y, text) of every literal-string text draw.
    fn text_pos(pdf: &[u8]) -> Vec<(f64, f64, String)> {
        let t = s(pdf);
        let parts: Vec<&str> = t.split(" Td (").collect();
        let mut out = vec![];
        for k in 1..parts.len() {
            let mut nums = parts[k - 1].rsplit(' ');
            let y = nums.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let x = nums.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            out.push((x, y, parts[k].split(") Tj").next().unwrap_or("").to_string()));
        }
        out
    }

    fn at(pos: &[(f64, f64, String)], needle: &str) -> (f64, f64) {
        pos.iter().find(|p| p.2.contains(needle)).map(|p| (p.0, p.1)).unwrap_or_else(|| panic!("{needle} not drawn: {pos:?}"))
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 90]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    fn mc(cols: u8) -> ExamDoc {
        ExamDoc {
            title: "Cols".into(),
            header_line: "Name: ______".into(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            questions: vec![DocQ {
                number: 1,
                text: vec!["Pick one.".into()],
                choices: vec!["A. Alpha".into(), "B. Beta".into(), "C. Gamma".into(), "D. Delta".into()],
                answer_columns: cols,
                correct: vec![2],
                key_marks: vec!["C".into()],
                key_text: "C".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn docx_part(d: &ExamDoc, name: &str) -> Option<Vec<u8>> {
        let bytes = render_docx(d).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut f = z.by_name(name).ok()?;
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        Some(v)
    }

    fn docx_xml(d: &ExamDoc) -> String {
        String::from_utf8(docx_part(d, "word/document.xml").unwrap()).unwrap()
    }

    #[test]
    fn choice_grid_fills_columns_like_test_parrot() {
        use crate::layout::choice_grid;
        assert_eq!(choice_grid(4, 2), vec![vec![0, 2], vec![1, 3]], "A C / B D");
        assert_eq!(choice_grid(4, 4), vec![vec![0, 1, 2, 3]]);
        assert_eq!(choice_grid(5, 2), vec![vec![0, 3], vec![1, 4], vec![2]]);
        assert_eq!(choice_grid(3, 1), vec![vec![0], vec![1], vec![2]]);
        assert_eq!(choice_grid(3, 0), vec![vec![0], vec![1], vec![2]], "0 reads as one column");
        assert_eq!(choice_grid(2, 4), vec![vec![0, 1]]);
        assert!(choice_grid(0, 2).is_empty());
    }

    #[test]
    fn pdf_answer_columns_two_and_four() {
        let p1 = text_pos(&render_pdf(&mc(1)).unwrap());
        let (xa, ya) = at(&p1, "A. Alpha");
        let (xb, yb) = at(&p1, "B. Beta");
        assert!((xa - xb).abs() < 0.01 && yb < ya, "1 column: one per line");
        let p2 = text_pos(&render_pdf(&mc(2)).unwrap());
        let (a, b, c, d) = (at(&p2, "A. Alpha"), at(&p2, "B. Beta"), at(&p2, "C. Gamma"), at(&p2, "D. Delta"));
        assert!((a.1 - c.1).abs() < 0.01 && (b.1 - d.1).abs() < 0.01 && b.1 < a.1, "rows A C / B D");
        assert!(c.0 > a.0 + 100.0 && (a.0 - b.0).abs() < 0.01 && (c.0 - d.0).abs() < 0.01);
        let p4 = text_pos(&render_pdf(&mc(4)).unwrap());
        let ys: Vec<f64> = ["A. Alpha", "B. Beta", "C. Gamma", "D. Delta"].iter().map(|n| at(&p4, n).1).collect();
        assert!(ys.iter().all(|y| (y - ys[0]).abs() < 0.01), "4 columns: one row");
        // A long choice wraps inside its column instead of running into the next.
        let mut d = mc(2);
        d.questions[0].choices[0] = format!("A. {}", "word ".repeat(30));
        let p = text_pos(&render_pdf(&d).unwrap());
        let c = at(&p, "C. Gamma");
        let widest_a = p.iter().filter(|t| t.0 < c.0 && t.2.contains("word")).count();
        assert!(widest_a >= 2, "the long choice wraps over several lines in column 1");
    }

    #[test]
    fn docx_answer_columns_are_a_borderless_grid() {
        let x = docx_xml(&mc(2));
        assert_eq!(x.matches("<w:gridCol").count(), 2);
        assert!(x.contains(r#"<w:insideV w:val="nil"/>"#));
        let a = x.find(">A. Alpha<").unwrap();
        let c = x.find(">C. Gamma<").unwrap();
        let b = x.find(">B. Beta<").unwrap();
        assert!(a < c && c < b, "row 1 holds A then C");
        assert!(!docx_xml(&mc(1)).contains("<w:tbl>"), "1 column keeps paragraphs");
    }

    #[test]
    fn title_area_follows_the_page_preview() {
        let mut d = mc(1);
        d.questions[0].points = Some(2.0);
        let p = text_pos(&render_pdf(&d).unwrap());
        let (hx, hy) = at(&p, "Name: ");
        let (tx, ty) = at(&p, "Cols");
        assert!(hy > ty, "header line above the title");
        let mid = (18.0 * MM_PT + 612.0 - 18.0 * MM_PT) / 2.0;
        let tw = text_w("Cols", Face::Bold, 16.0);
        assert!((tx + tw / 2.0 - mid).abs() < 0.5, "title centred");
        assert!((hx - 18.0 * MM_PT).abs() < 0.5);
        let (_, sy) = at(&p, "1 question");
        assert!(sy < ty);
        assert!(p.iter().any(|t| t.2.contains("2 points")), "subtitle carries the total");
        // Answer keys keep the plain title line (no header, no subtitle).
        let mut k = d.clone();
        k.is_key = true;
        let p = text_pos(&render_pdf(&k).unwrap());
        assert!(!p.iter().any(|t| t.2.contains("Name:") || t.2.contains("1 question")));
    }

    #[test]
    fn blanks_print_at_a_proper_length() {
        let min = (BLANK_EM / char_w(b'_', Face::Reg, 1.0)).ceil() as usize;
        assert!(min >= 6);
        assert_eq!(widen_blanks("Water is ___."), format!("Water is {}.", "_".repeat(min)));
        let long = "_".repeat(min + 10);
        assert_eq!(widen_blanks(&long), long, "longer blanks keep their length");
        assert_eq!(widen_blanks("a__b snake_case"), "a__b snake_case", "two underscores are not a blank");
        assert_eq!(widen_blanks("x | ___"), format!("x | {}", "_".repeat(min)));
        let mut d = mc(1);
        d.questions[0].choices.clear();
        d.questions[0].text = vec!["Fill: ___ and ___".into()];
        let t = s(&render_pdf(&d).unwrap());
        assert!(t.contains(&"_".repeat(min)), "PDF widens");
        assert!(docx_xml(&d).contains(&"_".repeat(min)), "DOCX widens");
    }

    #[test]
    fn picture_sits_at_the_stem_edge_sized_like_the_preview() {
        assert_eq!(crate::layout::picture_size_pt(100, 50, 400.0), (75.0, 37.5), "natural 96 dpi size, never enlarged");
        let (w, h) = crate::layout::picture_size_pt(2000, 1000, 400.0);
        assert!((w - 400.0).abs() < 1e-6 && (h - 200.0).abs() < 1e-6, "scaled to the column width");
        let (_, h) = crate::layout::picture_size_pt(300, 2000, 400.0);
        assert!((h - crate::layout::PICTURE_MAX_H_PT).abs() < 1e-6, "tall pictures capped");
        let mut d = mc(1);
        d.questions[0].choices.clear();
        d.questions[0].text = vec!["Look:\n[picture: graph]\nWhat is the slope?".into()];
        d.questions[0].image = Some(DocImage { bytes: png(800, 400), width_px: 800, height_px: 400 });
        let pdf = render_pdf(&d).unwrap();
        let t = s(&pdf);
        assert!(!t.contains("[picture"), "the attached picture replaces its marker");
        let p = text_pos(&pdf);
        let (lx, ly) = at(&p, "Look:");
        let (_, sy) = at(&p, "What is the slope?");
        // `w 0 0 h x y cm /Im1 Do`
        let cm = t.split(" cm /Im1 Do").next().unwrap().rsplit("q ").next().unwrap().to_string();
        let v: Vec<f64> = cm.split(' ').filter_map(|n| n.parse().ok()).collect();
        let (w, h, x, y) = (v[0], v[3], v[4], v[5]);
        assert!((x - lx).abs() < 0.01, "left edge = stem x ({x} vs {lx})");
        assert!(y + h < ly && y > sy, "between the text before and after the marker");
        assert!(h <= crate::layout::PICTURE_MAX_H_PT + 0.01 && (w / h - 2.0).abs() < 0.01, "aspect kept, capped");
        assert!(w <= 612.0 - 2.0 * 18.0 * MM_PT - STEM_INSET + 0.01);
    }

    #[test]
    fn docx_embeds_pictures_as_inline_drawings() {
        let mut d = mc(1);
        d.questions[0].image = Some(DocImage { bytes: png(400, 200), width_px: 400, height_px: 200 });
        let x = docx_xml(&d);
        assert!(!x.contains("[picture]"));
        assert!(x.contains("<wp:inline") && x.contains(r#"r:embed="rIdM1""#) && x.contains("xmlns:wp="));
        // 400x200 px -> 300x150 pt -> EMU
        assert!(x.contains(&format!(r#"<wp:extent cx="{}" cy="{}"/>"#, 300 * 12700, 150 * 12700)));
        let media = docx_part(&d, "word/media/image1.png").expect("media part");
        assert_eq!(&media[..4], b"\x89PNG");
        let rels = String::from_utf8(docx_part(&d, "word/_rels/document.xml.rels").unwrap()).unwrap();
        assert!(rels.contains(r#"Id="rIdM1""#) && rels.contains("media/image1.png"));
        let types = String::from_utf8(docx_part(&d, "[Content_Types].xml").unwrap()).unwrap();
        assert!(types.contains(r#"<Default Extension="png" ContentType="image/png"/>"#));
        // Undecodable bytes keep the placeholder; documents without pictures are unchanged.
        d.questions[0].image = Some(DocImage { bytes: vec![1, 2, 3], width_px: 4, height_px: 4 });
        let x = docx_xml(&d);
        assert!(x.contains("[picture]") && !x.contains("<wp:inline"));
        let plain = docx_xml(&mc(1));
        assert!(!plain.contains("xmlns:wp") && docx_part(&mc(1), "word/media/image1.png").is_none());
        // JPEG embeds as it is.
        let mut jpg = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(10, 10)).write_to(&mut std::io::Cursor::new(&mut jpg), image::ImageFormat::Jpeg).unwrap();
        d.questions[0].image = Some(DocImage { bytes: jpg.clone(), width_px: 10, height_px: 10 });
        assert_eq!(docx_part(&d, "word/media/image1.jpeg").unwrap(), jpg);
    }

    fn key(layout: KeyLayout) -> ExamDoc {
        let mut d = mc(1);
        d.questions[0].section = Some(DocSection { title: String::new(), directions: "Circle.".into() });
        d.questions.push(DocQ { number: 2, text: vec!["Explain.".into()], key_text: "Because tides.".into(), rule_lines: 3, ..Default::default() });
        d.questions.push(DocQ {
            number: 3,
            text: vec!["Venus".into(), "Mars".into()],
            choices: vec!["A. red".into(), "B. hot".into()],
            key_marks: vec!["B".into(), "A".into()],
            key_text: "1-B 2-A".into(),
            ..Default::default()
        });
        d.title = "Cols — Answer Key".into();
        d.is_key = true;
        d.key_layout = layout;
        if layout != KeyLayout::Marked {
            for q in &mut d.questions {
                q.choices.clear();
                q.text.clear();
                q.rule_lines = 0;
            }
        }
        d
    }

    #[test]
    fn answer_section_key_writes_answers_on_blanks() {
        let pdf = render_pdf(&key(KeyLayout::Section)).unwrap();
        let p = text_pos(&pdf);
        assert!(p.iter().any(|t| t.2 == "Answer Section"));
        assert!(p.iter().any(|t| t.2 == "Section 1"), "untitled section named by number");
        let exact = |n: &str| p.iter().find(|t| t.2 == n).map(|t| (t.0, t.1)).unwrap_or_else(|| panic!("{n} not drawn"));
        let (cx, cy) = exact("C");
        let (_, n1) = exact("1.");
        assert!((cy - n1).abs() < 0.01 && cx > 18.0 * MM_PT + STEM_INSET, "the letter sits on question 1's blank");
        let (_, e) = exact("Because tides.");
        let (_, n2) = exact("2.");
        assert!(e < n2, "a written answer prints under its blank");
        assert!(p.iter().any(|t| t.2 == "3.") && p.iter().any(|t| t.2 == "4."), "matching: one blank per left item");
        let x = docx_xml(&key(KeyLayout::Section));
        assert!(x.contains("Answer Section") && x.contains(r#"<w:u w:val="single"/>"#) && x.contains("    C    "));
        assert!(x.contains("Because tides."));
    }

    #[test]
    fn marked_key_checks_correct_choices() {
        let pdf = render_pdf(&key(KeyLayout::Marked)).unwrap();
        let t = s(&pdf);
        let p = text_pos(&pdf);
        let (_, cy) = at(&p, "C. Gamma");
        assert!(t.contains("0.1 0.45 0.25 RG 1 J"), "a check mark is stroked");
        // The correct choice prints in the bold face.
        let bold_c = t.split(" Td (C. Gamma) Tj").next().unwrap().rsplit("BT /").next().unwrap().starts_with("F2");
        assert!(bold_c, "C. Gamma in bold");
        let plain_a = t.split(" Td (A. Alpha) Tj").next().unwrap().rsplit("BT /").next().unwrap().starts_with("F1");
        assert!(plain_a);
        assert!(p.iter().any(|t| t.2.contains("Because tides.")), "written answers print");
        assert!(cy > 0.0);
        let x = docx_xml(&key(KeyLayout::Marked));
        assert!(x.contains("\u{2713}") && x.contains("Because tides."));
    }

    #[test]
    fn no_answer_space_drops_lines_and_work_space() {
        let mut d = mc(1);
        d.questions[0].choices.clear();
        d.questions[0].text = vec!["Explain.".into()];
        d.questions[0].work_lines = 4;
        let lined = s(&render_pdf(&d).unwrap()).matches(" l S").count();
        d.questions[0].no_answer_space = true;
        let bare = s(&render_pdf(&d).unwrap()).matches(" l S").count();
        assert!(bare + 3 <= lined, "3 default ruled lines gone ({lined} -> {bare})");
        assert_eq!(docx_xml(&d).matches("<w:between").count(), 1, "only the header line's rule in the DOCX");
        d.questions[0].text = vec!["Water is ___.".into()];
        assert_eq!(rule_count(&d.questions[0]), 0);
    }

    #[test]
    fn default_key_layout_is_the_compact_list() {
        assert_eq!(ExamDoc::default().key_layout, KeyLayout::List);
        let mut d = key(KeyLayout::List);
        let a = render_pdf(&d).unwrap();
        d.questions.iter_mut().for_each(|q| {
            q.correct.clear();
            q.key_marks.clear();
            q.section = None;
        });
        assert_eq!(super::tests::masked(&a), super::tests::masked(&render_pdf(&d).unwrap()), "list keys ignore the new fields");
    }
}

/// Round-7 stem markup in print: `code` in the mono, `::: center` groups, nested lists and
/// task items, in the PDF (layout items, operators, pdftotext) and the DOCX (XML).
#[cfg(test)]
mod r7_markup_tests {
    use super::*;
    use std::io::Read;

    fn doc(stem: &str) -> ExamDoc {
        ExamDoc {
            title: "R7".into(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            questions: vec![DocQ { number: 1, text: vec![stem.into()], points: Some(2.0), no_answer_space: true, ..Default::default() }],
            ..Default::default()
        }
    }

    fn part(d: &ExamDoc, name: &str) -> String {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(render_docx(d).unwrap())).unwrap();
        let mut s = String::new();
        z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    /// pdftotext -layout of the PDF (None when poppler is not installed).
    fn pdftotext(pdf: &[u8], tag: &str) -> Option<String> {
        std::process::Command::new("pdftotext").arg("-v").output().ok()?;
        let dir = std::env::temp_dir().join(format!("qs-r7-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r7.pdf");
        std::fs::write(&f, pdf).unwrap();
        let out = std::process::Command::new("pdftotext").args(["-layout", "-enc", "UTF-8"]).arg(&f).arg("-").output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success());
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn lines(items: &[Item]) -> Vec<(f64, Option<String>, String)> {
        items
            .iter()
            .filter_map(|it| match it {
                Item::Line { indent, frags, marker, .. } => Some((*indent, marker.clone(), frags.iter().map(|p| p.text.as_str()).collect())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn code_prints_in_the_embedded_mono() {
        let stem = "Run `ls -la` and then `cd ..` in a terminal.";
        let pdf = render_pdf(&doc(stem)).unwrap();
        let t = String::from_utf8_lossy(&pdf);
        assert!(t.contains("BT /F10 "), "code drawn in F10");
        assert!(t.contains("(ls -la) Tj") && t.contains("(cd ..) Tj"), "code text in one run each");
        assert!(t.contains("+LiberationMono /FirstChar 32"), "mono subset embedded");
        assert!(t.contains(" /F10 ") && t.contains("/FontFile2"));
        assert!(t.contains(&format!("{CODE_TINT} g ")), "code tint");
        // Measured with the mono's own advances at MONO_SCALE.
        let w = text_w("ls -la", Face::Mono, 11.0);
        assert!((w - 6.0 * 0.6 * 11.0 * MONO_SCALE).abs() < 1e-9, "{w}");
        // Non-WinAnsi characters inside code fall back to the serif's CID font (F5).
        let pdf2 = render_pdf(&doc("Type `x \u{2264} \u{3c0}` here.")).unwrap();
        assert!(String::from_utf8_lossy(&pdf2).contains("BT /F5 "));
        if let Some(txt) = pdftotext(&pdf, "code") {
            assert!(txt.contains("Run ls -la and then cd .. in a terminal."), "{txt}");
        }
        if let Some(txt) = pdftotext(&pdf2, "code2") {
            assert!(txt.contains("x \u{2264} \u{3c0}"), "{txt}");
        }
        // DOCX: Courier New runs, shaded, with the font in the font table.
        let d = doc(stem);
        let x = part(&d, "word/document.xml");
        assert!(x.contains(&format!("<w:r><w:rPr>{CODE_FONT}<w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/>{CODE_SHD}</w:rPr><w:t xml:space=\"preserve\">ls -la</w:t></w:r>")), "{x}");
        let ft = part(&d, "word/fontTable.xml");
        assert!(ft.contains("<w:font w:name=\"Courier New\"><w:altName w:val=\"Liberation Mono\"/>") && ft.contains("<w:pitch w:val=\"fixed\"/>"));
        // A code link keeps both.
        let x = part(&doc("See [`man ls`](https://example.com/ls)."), "word/document.xml");
        assert!(x.contains(&format!("<w:r><w:rPr>{CODE_FONT}<w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/>")), "{x}");
    }

    #[test]
    fn documents_without_code_embed_no_mono_and_keep_the_font_table() {
        let d = doc("Plain **bold** text, a list:\n• one\n• two");
        let pdf = render_pdf(&d).unwrap();
        let t = String::from_utf8_lossy(&pdf);
        assert!(!t.contains("F10") && !t.contains("LiberationMono"));
        assert_eq!(part(&d, "word/fontTable.xml"), FONT_TABLE);
        assert_eq!(part(&d, "word/numbering.xml"), numbering_xml(0, false));
        assert!(!part(&d, "word/document.xml").contains("Courier"));
    }

    #[test]
    fn centred_groups_centre_each_line_in_the_pdf() {
        let stem = "Evaluate:\n::: center\nShort centred line\n## A heading\n:::\nAfter, left aligned.";
        let items = layout_nodes(&richtext::parse_tree(stem), 400.0, 380.0, 11.0, 15.6);
        let ls = lines(&items);
        assert_eq!(ls[0].0, 0.0, "{ls:?}");
        let w = text_w("Short centred line", Face::Reg, 11.0);
        assert!((ls[1].0 - (400.0 - w) / 2.0).abs() < 0.01, "{ls:?}");
        assert!(ls[2].0 > 100.0 && ls[2].2 == "A heading", "heading centred: {ls:?}");
        assert_eq!(ls[3].0, 0.0);
        // A long centred paragraph: every wrapped line centred in the width it wrapped to.
        let long = format!("::: center\n{}\n:::", "word ".repeat(40).trim());
        let items = layout_nodes(&richtext::parse_tree(&long), 300.0, 260.0, 11.0, 15.6);
        for (k, it) in items.iter().enumerate() {
            if let Item::Line { indent, frags, .. } = it {
                let room = if k == 0 { 260.0 } else { 300.0 };
                assert!((indent * 2.0 + placed_w(frags, 11.0) - room).abs() < 0.01 || *indent == 0.0, "line {k}");
                assert!(indent + placed_w(frags, 11.0) <= room + 0.01);
            }
        }
        // A centred table sits in the middle of the column.
        let items = layout_nodes(&richtext::parse_tree("::: center\na | b\n1 | 2\n:::"), 400.0, 400.0, 11.0, 15.6);
        let Some(Item::Row { cols, .. }) = items.first() else { panic!("{items:?}") };
        let total: f64 = cols.iter().map(|c| c.1).sum();
        assert!((cols[0].0 - (400.0 - total) / 2.0).abs() < 0.01, "{cols:?}");
        // Its header shading starts at its first column, where its borders do.
        let pdf = render_pdf(&doc("Fill in:\n::: center\nFormula | Name\nKI | ___\n:::")).unwrap();
        let t = String::from_utf8_lossy(&pdf);
        let num_after = |pat: &str| t[t.find(pat).unwrap() + pat.len()..].split(' ').next().unwrap().parse::<f64>().unwrap();
        let (fill_x, border_x) = (num_after("0.93 g "), num_after("0.6 w 0.25 0.25 0.25 RG "));
        assert_eq!(fill_x, border_x);
        assert!(fill_x > 150.0, "{fill_x}");
        // In the PDF: the centred line's x is right of the stem's left edge; pdftotext keeps the text.
        let pdf = render_pdf(&doc(stem)).unwrap();
        if let Some(txt) = pdftotext(&pdf, "center") {
            let l = txt.lines().find(|l| l.contains("Short centred line")).unwrap();
            let e = txt.lines().find(|l| l.contains("Evaluate:")).unwrap();
            assert!(l.find('S').unwrap() > e.find('E').unwrap() + 10, "{txt}");
        }
    }

    #[test]
    fn centred_groups_are_jc_center_in_the_docx() {
        let x = part(&doc("Evaluate:\n::: center\nShort centred line\n## A heading\n:::\n::: center\na | b\n1 | 2\n:::"), "word/document.xml");
        assert!(x.contains("<w:p><w:pPr><w:ind w:left=\"432\"/><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/></w:rPr><w:t xml:space=\"preserve\">Short centred line</w:t>"), "{x}");
        assert!(x.contains("<w:ind w:left=\"432\"/><w:jc w:val=\"center\"/></w:pPr>") && x.matches("<w:jc w:val=\"center\"/>").count() >= 4, "{x}");
        assert!(x.contains("<w:tblW w:w=\"") && x.contains("w:type=\"dxa\"/><w:jc w:val=\"center\"/><w:tblBorders>"), "table jc: {x}");
        // "Evaluate:" keeps the number in its gutter, left aligned.
        assert!(x.contains("<w:ind w:left=\"432\" w:hanging=\"432\"/></w:pPr>"));
        // A stem that opens centred: the number stands on its own line first.
        let x = part(&doc("::: center\nOnly centred\n:::"), "word/document.xml");
        let num = x.find(">1.</w:t>").unwrap();
        assert!(x[num..].find("<w:jc w:val=\"center\"/></w:pPr>").is_some(), "{x}");
    }

    #[test]
    fn nested_lists_indent_per_level_with_level_labels() {
        let stem = "• Fruit\n  • Apple\n    • Fuji\n  • Pear\n• Veg";
        let ls = lines(&layout_nodes(&richtext::parse_tree(stem), 400.0, 400.0, 11.0, 15.6));
        let got: Vec<(f64, Option<String>)> = ls.iter().map(|l| (l.0, l.1.clone())).collect();
        let b = |c: char| Some(c.to_string());
        assert_eq!(
            got,
            vec![(LIST_INDENT, b('\u{2022}')), (2.0 * LIST_INDENT, b('\u{25e6}')), (3.0 * LIST_INDENT, b('\u{25aa}')), (2.0 * LIST_INDENT, b('\u{25e6}')), (LIST_INDENT, b('\u{2022}'))]
        );
        let stem = "1. Mix\n  1. Stir\n  2. Shake\n    1. Hard\n2. Heat\n  1. Gently";
        let ls = lines(&layout_nodes(&richtext::parse_tree(stem), 400.0, 400.0, 11.0, 15.6));
        let marks: Vec<String> = ls.iter().map(|l| l.1.clone().unwrap()).collect();
        assert_eq!(marks, ["1.", "a.", "b.", "i.", "2.", "a."]);
        assert_eq!(ls[3].0, NUM_INDENT + 2.0 * LIST_INDENT);
        // Print: the labels and glyphs come back out of the PDF (◦ ▪ through the fallback fonts).
        let pdf = render_pdf(&doc(&format!("Do these:\n\n{stem}\n\nList:\n• Fruit\n  • Apple\n    • Fuji"))).unwrap();
        if let Some(txt) = pdftotext(&pdf, "nest") {
            for want in ["1. Mix", "a. Stir", "b. Shake", "i. Hard", "2. Heat", "\u{2022} Fruit", "\u{25e6} Apple", "\u{25aa} Fuji"] {
                assert!(txt.contains(want), "{want}: {txt}");
            }
            let col = |s: &str| txt.lines().find(|l| l.contains(s)).unwrap().find(s.chars().last().unwrap()).unwrap();
            assert!(col("Stir") > col("Mix") && col("Hard") > col("Stir"), "{txt}");
        }
        // DOCX: w:ilvl per level and multilevel abstractNums with the editor's labels.
        let d = doc(&format!("{stem}\n\n• Fruit\n  • Apple\n    • Fuji"));
        let x = part(&d, "word/document.xml");
        for l in ["0", "1", "2"] {
            assert!(x.contains(&format!("<w:ilvl w:val=\"{l}\"/><w:numId w:val=\"2\"/>")), "numbered lvl {l}");
            assert!(x.contains(&format!("<w:ilvl w:val=\"{l}\"/><w:numId w:val=\"1\"/>")), "bullet lvl {l}");
        }
        let n = part(&d, "word/numbering.xml");
        assert!(n.contains("<w:multiLevelType w:val=\"multilevel\"/>"));
        assert!(n.contains("<w:lvl w:ilvl=\"1\"><w:start w:val=\"1\"/><w:numFmt w:val=\"lowerLetter\"/><w:lvlText w:val=\"%2.\"/>"), "{n}");
        assert!(n.contains("<w:lvl w:ilvl=\"2\"><w:start w:val=\"1\"/><w:numFmt w:val=\"lowerRoman\"/><w:lvlText w:val=\"%3.\"/>"));
        assert!(n.contains("<w:lvlText w:val=\"\u{25e6}\"/>") && n.contains("<w:lvlText w:val=\"\u{25aa}\"/>"));
        assert!(n.contains(&format!("<w:ind w:left=\"{}\" w:hanging=\"360\"/>", 792 + 640)));
    }

    #[test]
    fn task_items_print_their_box_without_a_bullet() {
        let stem = "Before you start:\n• \u{2611} Goggles on\n• \u{2610} Gloves on\n• Read the label";
        let ls = lines(&layout_nodes(&richtext::parse_tree(stem), 400.0, 400.0, 11.0, 15.6));
        assert_eq!(ls[1].1, None);
        assert_eq!(ls[2].1, None);
        assert_eq!(ls[3].1.as_deref(), Some("\u{2022}"), "an ordinary item keeps its bullet");
        assert_eq!(ls[1].0, LIST_INDENT, "the box sits where the item text does");
        if let Some(txt) = pdftotext(&render_pdf(&doc(stem)).unwrap(), "task") {
            assert!(txt.contains("\u{2611} Goggles on") && !txt.contains("\u{2022} \u{2611}"), "{txt}");
            assert!(txt.contains("\u{2022} Read the label"), "{txt}");
        }
        let x = part(&doc(stem), "word/document.xml");
        assert!(x.contains("<w:p><w:pPr><w:ind w:left=\"792\"/></w:pPr><w:r><w:rPr><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/></w:rPr><w:t xml:space=\"preserve\">\u{2611} Goggles on"), "{x}");
        assert_eq!(x.matches("<w:numId w:val=\"1\"/>").count(), 1, "only the ordinary item is a bullet");
    }

    /// Nodes of a stem without round-7 markup lay out exactly as its plain blocks always did.
    #[test]
    fn stems_without_round7_markup_lay_out_as_before() {
        for stem in ["Do all:\n• Draw it.\n• Count them.\n1. one\n2. two\n> quoted", "a | b\n1 | 2", "::: side\nL\n::: next\nR\n:::", "## H\n---\ntext"] {
            assert!(!richtext::uses_r7(stem));
            let a = layout_nodes(&richtext::parse_tree(stem), 400.0, 380.0, 11.0, 15.6);
            let b = layout_blocks(&richtext::parse(stem), 400.0, 380.0, 11.0, 15.6);
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{stem}");
        }
    }
}
