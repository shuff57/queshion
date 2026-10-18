//! Text-only equation helpers: splitting `$...$` out of a stem, parsing linear content, and the
//! Unicode plain-text rendering used where math cannot be drawn. No UI or rendering
//! dependencies; [`crate::equation`] re-exports everything here.

use std::ops::Range;

use equashion_math::{Math, from_linear, to_linear, unknown_commands};

// ---------- segments ----------

/// A piece of question text: plain text, or the linear content between a pair of `$`.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Text(String),
    Math(String),
}

/// A segment with the char range it covers in the source (delimiters included).
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub seg: Segment,
    pub span: Range<usize>,
}

/// Split text on `$...$`. `\$` is a literal dollar. A `$` pair only counts as math when the
/// content is non-empty and neither starts nor ends with whitespace (so "costs $5 and $6" stays text).
pub fn pieces(text: &str) -> Vec<Piece> {
    let ch: Vec<char> = text.chars().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut buf = String::new();
    let mut buf_start = 0usize;
    let mut i = 0;
    let flush = |buf: &mut String, out: &mut Vec<Piece>, start: usize, end: usize| {
        if !buf.is_empty() {
            out.push(Piece { seg: Segment::Text(std::mem::take(buf)), span: start..end });
        }
    };
    while i < ch.len() {
        let c = ch[i];
        if c == '\\' && ch.get(i + 1) == Some(&'$') {
            if buf.is_empty() {
                buf_start = i;
            }
            buf.push('$');
            i += 2;
            continue;
        }
        if c == '$' {
            let dbl = ch.get(i + 1) == Some(&'$');
            let start = if dbl { i + 2 } else { i + 1 };
            let mut j = start;
            let mut close = None;
            while j < ch.len() {
                if ch[j] == '\\' {
                    j += 2;
                    continue;
                }
                if ch[j] == '$' {
                    close = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(j) = close {
                let inner: String = ch[start..j].iter().collect();
                let ok = !inner.trim().is_empty() && !inner.starts_with(char::is_whitespace) && !inner.ends_with(char::is_whitespace);
                if ok {
                    flush(&mut buf, &mut out, buf_start, i);
                    let end = j + if dbl && ch.get(j + 1) == Some(&'$') { 2 } else { 1 };
                    out.push(Piece { seg: Segment::Math(inner), span: i..end });
                    i = end;
                    continue;
                }
            }
            if buf.is_empty() {
                buf_start = i;
            }
            buf.push('$');
            i += 1;
            continue;
        }
        if buf.is_empty() {
            buf_start = i;
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut out, buf_start, ch.len());
    out
}

pub fn rich_segments(text: &str) -> Vec<Segment> {
    pieces(text).into_iter().map(|p| p.seg).collect()
}

/// True when `text` contains at least one `$...$` equation.
pub fn has_math(text: &str) -> bool {
    text.contains('$') && pieces(text).iter().any(|p| matches!(p.seg, Segment::Math(_)))
}

/// Wrap linear content as an inline equation chip string.
pub fn wrap_math(linear: &str) -> String {
    format!("${}$", linear.trim())
}

/// Replace the char range `span` of `text` (or insert at char `caret` when `span` is None).
pub fn splice(text: &str, span: Option<Range<usize>>, caret: usize, insert: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    let r = match span {
        Some(r) => r.start.min(chars.len())..r.end.min(chars.len()),
        None => {
            let c = caret.min(chars.len());
            c..c
        }
    };
    chars.splice(r, insert.chars());
    chars.into_iter().collect()
}

/// The linear content of the equation covering `span` (delimiters stripped).
pub fn content_of(text: &str, span: &Range<usize>) -> String {
    pieces(text)
        .into_iter()
        .find(|p| p.span == *span)
        .and_then(|p| if let Segment::Math(m) = p.seg { Some(m) } else { None })
        .unwrap_or_default()
}

// ---------- parsing ----------

/// Parse linear (or legacy LaTeX-ish) content. None when it is empty or has commands the
/// parser does not know (those would draw as literal text, so they count as a failure here).
pub fn parse(content: &str) -> Option<Math> {
    let m = from_linear(content);
    if m.is_empty() || !unknown_commands(&m).is_empty() {
        return None;
    }
    Some(m)
}

// ---------- plain text ----------

const SUP: &str = "0123456789+-=()ni";
const SUPC: &str = "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿⁱ";
const SUB: &str = "0123456789+-=()";
const SUBC: &str = "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎";

fn script_run(rest: &[char], from: &str, to: &str) -> (String, usize) {
    let f: Vec<char> = from.chars().collect();
    let t: Vec<char> = to.chars().collect();
    let mut out = String::new();
    let mut n = 0;
    for (k, &c) in rest.iter().enumerate() {
        let c2 = if c == '−' { '-' } else { c };
        // a sign only belongs to the script when it leads it (x^-1, not x^2-9)
        if k > 0 && matches!(c2, '+' | '-' | '=' | '(' | ')') {
            break;
        }
        match f.iter().position(|x| *x == c2) {
            Some(i) => {
                out.push(t[i]);
                n += 1;
            }
            None => break,
        }
    }
    (out, n)
}

/// Unicode approximation of one equation: equaSHion's own linear print, with simple
/// exponents and subscripts turned into superscript/subscript characters.
pub fn to_plain_text(content: &str) -> String {
    let Some(m) = parse(content) else { return content.to_string() };
    let lin = to_linear(&m);
    let ch: Vec<char> = lin.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        let (map_from, map_to) = match ch[i] {
            '^' => (SUP, SUPC),
            '_' => (SUB, SUBC),
            _ => {
                out.push(ch[i]);
                i += 1;
                continue;
            }
        };
        let (s, n) = script_run(&ch[i + 1..], map_from, map_to);
        if n > 0 {
            out.push_str(&s);
            i += 1 + n;
        } else {
            out.push(ch[i]);
            i += 1;
        }
    }
    out
}

/// Plain-text rendering of a whole stem (equations converted with [`to_plain_text`]), for
/// exports and cards that cannot draw math. Never shows raw `$` delimiters for valid math.
pub fn stem_to_plain(text: &str) -> String {
    rich_segments(text)
        .into_iter()
        .map(|s| match s {
            Segment::Text(t) => t,
            Segment::Math(m) => to_plain_text(&m),
        })
        .collect()
}

/// One-line preview of a stem for cards, lists and search: markup flattened
/// (`richtext::plain`) and equations as their Unicode approximation.
pub fn stem_preview(text: &str) -> String {
    stem_to_plain(&scripts_to_unicode(&crate::richtext::plain(text)))
}

/// `_{2}` / `^{2+}` (left in by `richtext::plain`) as Unicode sub/superscripts where every
/// character has one, else the bare text ("x_{ab}" -> "x_ab").
fn scripts_to_unicode(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut in_math = false;
    while i < ch.len() {
        let c = ch[i];
        if c == '$' {
            in_math = !in_math;
        }
        if !in_math
            && (c == '_' || c == '^')
            && ch.get(i + 1) == Some(&'{')
            && let Some(end) = (i + 2..ch.len()).find(|&j| ch[j] == '}')
        {
            let inner: String = ch[i + 2..end].iter().collect();
            match crate::richtext::unicode_script(&inner, c == '^') {
                Some(u) => out.push_str(&u),
                None => {
                    out.push(c);
                    out.push_str(&inner);
                }
            }
            i = end + 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}
