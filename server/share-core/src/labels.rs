//! Choice labels and sibling references: what `strip` may never decide on its own.
//!
//! A hand-typed label ("A. Venus") keeps its original slot after a shuffle and gives the order
//! away; removing one that is really text ("A. thaliana", "I. only", "1: 2") corrupts the
//! question. So share-core **never strips labels silently**. It detects them broadly, shows the
//! author a before/after preview ([`LabelFinding`]), and the author decides per question or part:
//! [`LabelChoice::Strip`] or [`LabelChoice::Keep`]. A `hide_key` publish with an undecided finding
//! is refused (`labels_need_decision`).
//!
//! # Detection
//! A choice carries a **label** when it starts with (after optional whitespace):
//!
//! | Part | Accepted |
//! |---|---|
//! | optional word | `Option`, `Choice`, `Answer` (any case), then whitespace |
//! | optional opening bracket | `(` `[` and their full-width forms |
//! | token | one letter of the Latin (incl. full-width), Greek, Cyrillic or Hebrew alphabets, or a katakana/hiragana gojūon letter; an enclosed alphanumeric (Ⓐ ⓐ ① ⑴ ⒈ ⒜ ❶ ➀ ➊ 🄐 🄰 🅐 🅰); one or two decimal digits of any script; a roman numeral i-xxvi / I-XXVI or Ⅰ-Ⅻ / ⅰ-ⅻ |
//! | closer | the matching `)` `]` when a bracket was opened; otherwise one of `)` `]` `.` `:` `-` `–` `—` `、` `/` `・` (full-width forms included), spaces allowed before it ("A ) x"); not needed after an enclosed token or after the word form |
//! | after | whitespace, or the end, or nothing at all after `)` `]` `、` `・` a full-width closer or an enclosed token, or a capital letter after `.` ("A.Venus") |
//!
//! or ends with whitespace and a bracketed token ("Venus (A)"). Each token has values per
//! *style* (Latin letter, Greek, Cyrillic, Hebrew, kana, roman, number, enclosed) and the
//! position of each style's first letter is 1.
//!
//! A question has a **finding** when two or more choices carry a label, or exactly one does and
//! its value is its position (1-based) in some style. The labels are **order-revealing** when, in
//! one style, every labelled choice's value is its position plus one constant (A, B, C... and
//! also B, C, D...), or the single labelled choice is at its position.
//!
//! # Decisions
//! * **Strip** is allowed only when every choice carries a label of the same kind (all prefix or
//!   all suffix), the labels are exactly positions 1, 2, 3... in one style, and every result is
//!   visible and distinct after `hash_form`. Further layers ("A. (i) x") are removed while the same
//!   holds. If not allowed, asking for Strip is refused (`labels_strip_refused`).
//! * **Keep** leaves the text alone. If the labels are order-revealing, **nothing in the question
//!   is shuffled** (the order is preserved and listed in `needs_order_ack`); otherwise (labels that
//!   are text, like "E. coli", "S. aureus") the question is shuffled as usual.
//!
//! # Sibling references
//! A choice that names other choices cannot follow a shuffle: "Both A and B", "A and C only",
//! "I and II", "Neither A nor B", "II only". Any such choice ⇒ the question is not shuffled and is
//! listed in `needs_order_ack` (reason `sibling_refs`). "All/None/Both/Neither of the above" (or
//! "of these") is fine where it is **locked** (it keeps its slot) and blocks the shuffle when it
//! is not.
use crate::text::{collapse, has_visible};
use serde::{Deserialize, Serialize};
use unicode_general_category::{GeneralCategory, get_general_category};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelChoice {
    Strip,
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Style {
    Latin,
    Greek,
    Cyrillic,
    Hebrew,
    Kana,
    Roman,
    Number,
    Enclosed,
}

/// One choice's label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// Byte range of the label in the text (a prefix starts at 0; a suffix ends at the end).
    pub start: usize,
    pub end: usize,
    pub suffix: bool,
    pub values: Vec<(Style, u32)>,
}

/// One choice in a finding: the text as it is and as Strip would leave it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Preview {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Analysis {
    pub labelled: usize,
    pub order_revealing: bool,
    pub strip_allowed: bool,
    /// Byte offset to cut from the start, and byte length to drop from the end, per choice, if
    /// stripped.
    #[serde(skip)]
    pub cut: Vec<(usize, usize)>,
}

impl Analysis {
    /// The before/after preview for the author (built only when asked for: it copies the text).
    pub fn previews(&self, texts: &[&str]) -> Vec<Preview> {
        texts.iter().zip(&self.cut).map(|(t, &c)| Preview { before: t.to_string(), after: apply(t, c) }).collect()
    }
}

const LATIN_UP: u32 = 'A' as u32;

fn kana_value(c: char) -> Option<u32> {
    const K: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノ";
    const H: &str = "あいうえおかきくけこさしすせそたちつてとなにぬねの";
    K.chars().position(|k| k == c).or_else(|| H.chars().position(|k| k == c)).map(|p| p as u32 + 1)
}

fn hebrew_value(c: char) -> Option<u32> {
    const A: &str = "אבגדהוזחטיכלמנסעפצקרשת";
    A.chars().position(|k| k == c).map(|p| p as u32 + 1)
}

fn greek_value(c: char) -> Option<u32> {
    const L: &str = "αβγδεζηθικλμνξοπρστυφχψω";
    const U: &str = "ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ";
    L.chars().position(|k| k == c).or_else(|| U.chars().position(|k| k == c)).map(|p| p as u32 + 1)
}

fn cyrillic_value(c: char) -> Option<u32> {
    // The order Russian test makers letter choices in (no ё, й, ъ, ы, ь).
    const L: &str = "абвгдежзиклмнопрстуфхцчшщэюя";
    const U: &str = "АБВГДЕЖЗИКЛМНОПРСТУФХЦЧШЩЭЮЯ";
    L.chars().position(|k| k == c).or_else(|| U.chars().position(|k| k == c)).map(|p| p as u32 + 1)
}

/// Enclosed alphanumerics: (value, style) for ⓐ Ⓐ ① ⑴ ⒈ ⒜ ❶ ➀ ➊ 🄐 🄰 🅐 🅰 and roman Ⅰ ⅰ.
fn enclosed(c: char) -> Option<(Style, u32)> {
    let u = c as u32;
    let r = |base: u32, n: u32| (base..base + n).contains(&u).then(|| u - base + 1);
    r(0x24B6, 26)
        .or_else(|| r(0x24D0, 26))
        .or_else(|| r(0x249C, 26))
        .or_else(|| r(0x1F110, 26))
        .or_else(|| r(0x1F130, 26))
        .or_else(|| r(0x1F150, 26))
        .or_else(|| r(0x1F170, 26))
        .map(|v| (Style::Latin, v))
        .or_else(|| r(0x2460, 20).or_else(|| r(0x2474, 20)).or_else(|| r(0x2488, 20)).or_else(|| r(0x2776, 10)).or_else(|| r(0x2780, 10)).or_else(|| r(0x278A, 10)).map(|v| (Style::Number, v)))
        .or_else(|| r(0x2160, 12).or_else(|| r(0x2170, 12)).map(|v| (Style::Roman, v)))
}

fn fold(c: char) -> char {
    let u = c as u32;
    if (0xFF01..=0xFF5E).contains(&u) { char::from_u32(u - 0xFEE0).unwrap_or(c) } else { c }
}

fn wide(c: char) -> bool {
    (0xFF01..=0xFF5E).contains(&(c as u32))
}

/// The value of a decimal digit of any script (`None` if `c` is not Nd).
fn digit_value(c: char) -> Option<u32> {
    if get_general_category(c) != GeneralCategory::DecimalNumber {
        return None;
    }
    // Decimal digit blocks are runs of ten consecutive code points starting at zero; adjacent
    // blocks (the mathematical digits) are multiples of ten, so the value is the offset from the
    // start of the run modulo ten.
    let mut u = c as u32;
    let mut k = 0;
    while k < 60 && u > 0 && char::from_u32(u - 1).is_some_and(|p| get_general_category(p) == GeneralCategory::DecimalNumber) {
        u -= 1;
        k += 1;
    }
    Some(k % 10)
}

fn roman(t: &str) -> Option<u32> {
    const R: [&str; 26] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii", "xiv", "xv", "xvi", "xvii", "xviii", "xix", "xx",
        "xxi", "xxii", "xxiii", "xxiv", "xxv", "xxvi",
    ];
    let lower = t.chars().all(|c| c.is_ascii_lowercase());
    let upper = t.chars().all(|c| c.is_ascii_uppercase());
    if !(lower || upper) || t.is_empty() {
        return None;
    }
    let l = t.to_ascii_lowercase();
    R.iter().position(|r| *r == l).map(|p| p as u32 + 1)
}

fn letter_values(c: char) -> Vec<(Style, u32)> {
    let f = fold(c);
    let mut v = Vec::new();
    if f.is_ascii_alphabetic() {
        v.push((Style::Latin, (f.to_ascii_uppercase() as u32) - LATIN_UP + 1));
    }
    if let Some(x) = greek_value(c) {
        v.push((Style::Greek, x));
    }
    if let Some(x) = cyrillic_value(c) {
        v.push((Style::Cyrillic, x));
    }
    if let Some(x) = hebrew_value(c) {
        v.push((Style::Hebrew, x));
    }
    if let Some(x) = kana_value(c) {
        v.push((Style::Kana, x));
    }
    v
}

const CLOSERS: [char; 11] = [')', ']', '.', ':', '-', '\u{2013}', '\u{2014}', '\u{3001}', '/', '\u{30FB}', '\u{FF65}'];

/// Reads a prefix label (see the module docs).
fn prefix(s: &str) -> Option<Hit> {
    let b: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0;
    let ws = |i: &mut usize| {
        while *i < b.len() && b[*i].1.is_whitespace() {
            *i += 1;
        }
    };
    ws(&mut i);
    // The word form.
    let mut word = false;
    for w in ["option", "choice", "answer"] {
        let rest = &s[b.get(i).map_or(s.len(), |x| x.0)..];
        if rest.get(..w.len()).is_some_and(|p| p.eq_ignore_ascii_case(w)) && rest[w.len()..].starts_with(char::is_whitespace) {
            i += w.chars().count();
            ws(&mut i);
            word = true;
            break;
        }
    }
    let opened = b.get(i).is_some_and(|x| matches!(fold(x.1), '(' | '['));
    if opened {
        i += 1;
    }
    let start_tok = i;
    let (values, enc) = match b.get(i).map(|x| x.1) {
        Some(c) if enclosed(c).is_some() => {
            i += 1;
            (vec![enclosed(c)?], true)
        }
        Some(c) if digit_value(c).is_some() => {
            let d1 = digit_value(c)?;
            i += 1;
            let mut v = d1;
            if let Some(d2) = b.get(i).and_then(|x| digit_value(x.1)) {
                v = d1 * 10 + d2;
                i += 1;
            }
            if d1 == 0 || b.get(i).is_some_and(|x| digit_value(x.1).is_some()) {
                return None;
            }
            (vec![(Style::Number, v)], false)
        }
        Some(_) => {
            // A run of letters: one letter (any listed alphabet) or an ASCII roman numeral.
            let mut j = i;
            while j < b.len() && j - i < 6 && (fold(b[j].1).is_alphabetic()) {
                j += 1;
            }
            let tok: String = b[i..j].iter().map(|x| fold(x.1)).collect();
            let mut v = Vec::new();
            if j - i == 1 {
                v.extend(letter_values(b[i].1));
            }
            if let Some(r) = roman(&tok) {
                v.push((Style::Roman, r));
            }
            if v.is_empty() {
                return None;
            }
            i = j;
            (v, false)
        }
        None => return None,
    };
    let _ = start_tok;
    // The closer.
    let mut k = i;
    while k < b.len() && k - i < 2 && b[k].1 == ' ' {
        k += 1;
    }
    let mut closer = None;
    if let Some(&(_, c)) = b.get(k) {
        let f = fold(c);
        let ok = if opened { matches!(f, ')' | ']') } else { CLOSERS.contains(&f) || CLOSERS.contains(&c) };
        if ok {
            closer = Some(c);
            i = k + 1;
        }
    }
    if opened && closer.is_none() {
        return None;
    }
    if closer.is_none() && !enc && !word {
        return None;
    }
    // What may follow.
    let next = b.get(i).map(|x| x.1);
    let tight = closer.is_some_and(|c| matches!(fold(c), ')' | ']') || matches!(c, '\u{3001}' | '\u{30FB}' | '\u{FF65}') || wide(c)) || enc && closer.is_none();
    let ok = match next {
        None => true,
        Some(c) if c.is_whitespace() => true,
        Some(c) if tight => !c.is_whitespace(),
        Some(c) if closer.is_some_and(|x| fold(x) == '.') => c.is_uppercase(),
        _ => false,
    };
    if !ok {
        return None;
    }
    ws(&mut i);
    Some(Hit { start: 0, end: b.get(i).map_or(s.len(), |x| x.0), suffix: false, values })
}

/// Reads a suffix label: whitespace then `(token)` or `[token]` at the end.
fn suffix(s: &str) -> Option<Hit> {
    let t = s.trim_end();
    let close = t.chars().last()?;
    let open = match fold(close) {
        ')' => '(',
        ']' => '[',
        _ => return None,
    };
    let body_end = t.len() - close.len_utf8();
    let open_at = t[..body_end].rfind(|c: char| fold(c) == open)?;
    let tok = &t[open_at + t[open_at..].chars().next()?.len_utf8()..body_end];
    let before = &t[..open_at];
    if !before.ends_with(char::is_whitespace) || !has_visible(before) {
        return None;
    }
    let mut v = Vec::new();
    let chars: Vec<char> = tok.chars().collect();
    if chars.len() == 1 {
        v.extend(letter_values(chars[0]));
        if let Some(e) = enclosed(chars[0]) {
            v.push(e);
        }
        if let Some(d) = digit_value(chars[0]).filter(|d| *d > 0) {
            v.push((Style::Number, d));
        }
    }
    if chars.len() == 2
        && let (Some(a), Some(b)) = (digit_value(chars[0]), digit_value(chars[1]))
        && a > 0
    {
        v.push((Style::Number, a * 10 + b));
    }
    let folded: String = chars.iter().map(|c| fold(*c)).collect();
    if let Some(r) = roman(&folded) {
        v.push((Style::Roman, r));
    }
    if v.is_empty() {
        return None;
    }
    Some(Hit { start: before.trim_end().len(), end: s.len(), suffix: true, values: v })
}

/// The label of one choice, prefix first.
pub fn detect(s: &str) -> Option<Hit> {
    prefix(s).or_else(|| suffix(s))
}

fn styles() -> [Style; 8] {
    [Style::Latin, Style::Greek, Style::Cyrillic, Style::Hebrew, Style::Kana, Style::Roman, Style::Number, Style::Enclosed]
}

fn value(h: &Hit, st: Style) -> Option<u32> {
    h.values.iter().find(|(s, _)| *s == st).map(|(_, v)| *v)
}

/// Whether the labelled choices are order-revealing (see the module docs).
fn revealing(hits: &[Option<Hit>]) -> bool {
    let labelled: Vec<(usize, &Hit)> = hits.iter().enumerate().filter_map(|(i, h)| h.as_ref().map(|h| (i, h))).collect();
    if labelled.is_empty() {
        return false;
    }
    styles().iter().any(|&st| {
        let vals: Option<Vec<i64>> = labelled.iter().map(|(i, h)| value(h, st).map(|v| v as i64 - *i as i64)).collect();
        // One label at its own position; or labels whose values rise with their positions
        // (A, B, C; B, C, D; and with gaps, A, B, D or A, C, E): any of these gives the order away.
        match vals {
            Some(d) if labelled.len() == 1 => d[0] == 1,
            Some(_) => labelled.windows(2).all(|w| matches!((value(w[0].1, st), value(w[1].1, st)), (Some(a), Some(b)) if b > a)),
            None => false,
        }
    })
}

/// Exactly positions 1, 2, 3... in one style, every choice labelled the same way.
fn exact(hits: &[Option<Hit>]) -> bool {
    if hits.len() < 2 || hits.iter().any(|h| h.is_none()) {
        return false;
    }
    let suffix = hits[0].as_ref().is_some_and(|h| h.suffix);
    if hits.iter().any(|h| h.as_ref().is_some_and(|h| h.suffix != suffix)) {
        return false;
    }
    styles().iter().any(|&st| hits.iter().enumerate().all(|(i, h)| h.as_ref().and_then(|h| value(h, st)) == Some(i as u32 + 1)))
}

/// Analyses one question's (or part's) choices in the author's order. `None` when there is no
/// finding.
pub fn analyze(texts: &[&str]) -> Option<Analysis> {
    let hits: Vec<Option<Hit>> = texts.iter().map(|t| detect(t)).collect();
    let labelled = hits.iter().filter(|h| h.is_some()).count();
    let finding = labelled >= 2 || labelled == 1 && revealing(&hits);
    if !finding {
        return None;
    }
    let order_revealing = revealing(&hits);
    // Strip: remove layers while every choice keeps an exact position label.
    let n = texts.len();
    let mut cut: Vec<(usize, usize)> = vec![(0, 0); n];
    let mut layer = hits.clone();
    let mut layers = 0;
    // At most 8 layers ("A. (i) x" is two); more is not a label, and the leftover rule below then
    // refuses Strip. This also bounds the work per question.
    while exact(&layer) && layers < 8 {
        let next: Vec<(usize, usize)> = (0..n)
            .map(|i| {
                let h = layer[i].as_ref().map_or((0, 0), |h| (h.start, h.end));
                let suffix = layer[i].as_ref().is_some_and(|h| h.suffix);
                let (a, z) = cut[i];
                if suffix { (a, z + (texts[i].len() - z - a - h.0)) } else { (a + h.1, z) }
            })
            .collect();
        // The texts are already NFC (validated), so the comparison form only needs whitespace
        // collapsed per line: no normalisation work per layer.
        let results: Vec<String> = (0..n).map(|i| lines_collapsed(&texts[i][next[i].0..texts[i].len() - next[i].1])).collect();
        let distinct = {
            let mut r = results.clone();
            r.sort();
            r.dedup();
            r.len() == n
        };
        if !distinct || results.iter().any(|r| r.is_empty() || !has_visible(r)) {
            break;
        }
        cut = next;
        layers += 1;
        layer = (0..n).map(|i| prefix(&texts[i][cut[i].0..texts[i].len() - cut[i].1])).collect();
    }
    // Labels still left after the last removable layer (a 65th layer, or one whose removal would
    // empty or merge choices) would give the order away after a shuffle: then Strip is refused.
    let rest: Vec<Option<Hit>> = (0..n).map(|i| detect(&texts[i][cut[i].0..texts[i].len() - cut[i].1])).collect();
    let leftover = rest.iter().any(|h| h.is_some()) && revealing(&rest);
    let strip_allowed = layers > 0 && !leftover;
    if !strip_allowed {
        cut = vec![(0, 0); n];
    }
    Some(Analysis { labelled, order_revealing, strip_allowed, cut })
}

/// `hash_form` for text that is already NFC.
fn lines_collapsed(s: &str) -> String {
    let lines: Vec<String> = s.split('\n').map(collapse).collect();
    lines.join("\n").trim_matches('\n').to_string()
}

/// The text Strip leaves.
pub fn apply(text: &str, cut: (usize, usize)) -> String {
    text[cut.0..text.len() - cut.1].to_string()
}

// ---- sibling references ---------------------------------------------------------------------

/// The kinds of reference a choice makes to its siblings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiblingRef {
    /// "All/None of the above": names *every* other choice, so it is safe where it is locked.
    Above,
    /// "Both A and B", "I and II only", "The first two", bare "Both": names particular choices,
    /// so the question is never shuffled.
    Labels,
}

/// Phrases that name every other choice (any case; English, Spanish, French, Portuguese,
/// Italian, German). A hint, not a grammar: the author can always freeze a question with
/// `keep_order`.
const ALL_OTHERS: [&str; 40] = [
    "of the above", "of these", "of them", "of the others", "the above", "above", "of the preceding", "of the previous",
    "of the options", "of the choices", "of the answers", "all the options", "all the choices", "all are", "none are",
    "all answers", "all the answers", "all choices", "all options", "none of the", "all of the", "all correct", "none correct",
    "las anteriores", "los anteriores", "todas las", "ninguna de", "ninguno de", "todas as", "nenhuma das", "nenhum dos",
    "toutes ces", "toutes les", "aucune de", "aucune des", "tutte le", "nessuna delle", "alle obigen", "keine der", "alle genannten",
];

/// Phrases that name particular choices by position.
const POSITIONS: [&str; 16] = [
    "first two", "first three", "last two", "last three", "previous two", "previous three", "preceding two", "former",
    "latter", "the first", "the second", "the third", "the last", "both of", "neither of", "two above",
];

/// Words that join or qualify choice names ("A and B", "A + B", "all except C", "B only").
const JOINERS: [&str; 22] = [
    "and", "or", "nor", "and/or", "&", "+", "/", "y", "o", "et", "ou", "und", "oder", "e", "but", "except", "not", "only",
    "both", "neither", "either", "plus",
];

/// "a", "(b)", "C.", "iv", "3)": what a choice's own label looks like, standing alone.
fn label_token(w: &str) -> bool {
    let core = w.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '.' | ',' | ':' | ';' | '"' | '\'' | '?' | '!'));
    if core.is_empty() || core.chars().count() > 5 {
        return false;
    }
    let mut cs = core.chars();
    let single = cs.next().filter(|_| cs.next().is_none());
    match single {
        Some(c) if c.is_alphabetic() && !letter_values(c).is_empty() => true,
        Some(c) if c.is_ascii_digit() => true,
        _ => roman(&core.to_ascii_lowercase()).is_some() || core.parse::<u32>().is_ok_and(|n| (1..=26).contains(&n)),
    }
}

/// Whether `text` refers to sibling choices (see the module docs; case-insensitive).
pub fn sibling_ref(text: &str) -> Option<SiblingRef> {
    let orig = collapse(text);
    let low = orig.to_lowercase();
    // Tokens in their own case ("A" is a label, "a" may be an article); "a+b", "a&b", "a/b" split.
    let mut toks: Vec<String> = Vec::new();
    for w in orig.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|w| !w.is_empty()) {
        if w.eq_ignore_ascii_case("and/or") {
            toks.push(w.to_string());
            continue;
        }
        let mut cur = String::new();
        for c in w.chars() {
            if matches!(c, '+' | '&' | '/') {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(c.to_string());
            } else {
                cur.push(c);
            }
        }
        if !cur.is_empty() {
            toks.push(cur);
        }
    }
    let bare = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '&' && c != '+' && c != '/').to_lowercase();
    let joiner = |w: &str| JOINERS.contains(&bare(w).as_str()) || w.eq_ignore_ascii_case("and/or");
    let first = toks.first().map(|w| bare(w)).unwrap_or_default();
    // A bare label ("B", "(c)"), a bare "Both" / "Neither", or "Both answers".
    if toks.len() == 1 && label_token(&toks[0]) {
        return Some(SiblingRef::Labels);
    }
    if toks.len() <= 3 && matches!(first.as_str(), "both" | "neither" | "ambas" | "ambos" | "ninguna" | "ninguno" | "aucune" | "beide" | "keine") {
        return Some(SiblingRef::Labels);
    }
    if POSITIONS.iter().any(|p| low.contains(p)) {
        return Some(SiblingRef::Labels);
    }
    // A label-like token next to a joiner: "A and", "and B", "(a) and (b)", "A + B", "except C".
    for (k, t) in toks.iter().enumerate() {
        if !label_token(t) {
            continue;
        }
        let prev = k.checked_sub(1).and_then(|j| toks.get(j));
        let next = toks.get(k + 1);
        if !(prev.is_some_and(|w| joiner(w)) || next.is_some_and(|w| joiner(w))) {
            continue;
        }
        // A lowercase "a" may be an article ("a large and bright star"): it counts only with a
        // second label across the joiner.
        if t == "a" {
            let label_at = |j: Option<usize>| j.and_then(|j| toks.get(j)).is_some_and(|w| label_token(w) && w != "a" || w == "b");
            let after = next.is_some_and(|w| joiner(w)) && label_at(Some(k + 2));
            let before = prev.is_some_and(|w| joiner(w)) && label_at(k.checked_sub(2));
            if !(after || before) {
                continue;
            }
        }
        return Some(SiblingRef::Labels);
    }
    if ALL_OTHERS.iter().any(|p| low.contains(p)) {
        return Some(SiblingRef::Above);
    }
    None
}

// ---- keys carried by formatting ---------------------------------------------------------------

const PREFIX_MARKS: [char; 12] = ['✓', '✔', '☑', '✅', '√', '★', '☆', '→', '⇒', '►', '▶', '✗'];
const SUFFIX_WORDS: [&str; 9] = ["(correct)", "(answer)", "(right)", "(true)", "[correct]", "[answer]", "(correct answer)", "(key)", "<- correct"];
const WRAPS: [(&str, &str); 13] = [
    ("**", "**"), ("__", "__"), ("~~", "~~"), ("==", "=="), ("*", "*"), ("_", "_"), ("<b>", "</b>"), ("<strong>", "</strong>"),
    ("<u>", "</u>"), ("<i>", "</i>"), ("<em>", "</em>"), ("<mark>", "</mark>"), ("[b]", "[/b]"),
];

/// The decorations of one choice text ("**x**", "x*", "✓ x", "x (correct)", "<b>x</b>"...), as a
/// signature: two choices with different signatures look different apart from their text.
pub fn decoration(text: &str) -> Vec<&'static str> {
    let mut sig = Vec::new();
    let mut t = text.trim();
    loop {
        let before = t;
        if let Some(c) = t.chars().next().filter(|c| PREFIX_MARKS.contains(c)) {
            sig.push("prefix_mark");
            t = t[c.len_utf8()..].trim_start();
        }
        if let Some(c) = t.chars().last().filter(|c| PREFIX_MARKS.contains(c)) {
            sig.push("suffix_mark");
            t = t[..t.len() - c.len_utf8()].trim_end();
        }
        let low = t.to_lowercase();
        if let Some(w) = SUFFIX_WORDS.iter().find(|w| low.ends_with(*w) && t.len() > w.len()) {
            sig.push("suffix_word");
            t = t[..t.len() - w.len()].trim_end();
        }
        for (a, z) in WRAPS {
            let lo = t.to_ascii_lowercase();
            if lo.len() > a.len() + z.len() && lo.starts_with(a) && lo.ends_with(z) {
                sig.push(a);
                t = t[a.len()..t.len() - z.len()].trim();
                break;
            }
        }
        if t.ends_with('*') && !t.starts_with('*') && t.len() > 1 {
            sig.push("trailing_star");
            t = t.trim_end_matches('*').trim_end();
        }
        if std::ptr::eq(before, t) || before == t {
            break;
        }
    }
    sig.sort_unstable();
    sig
}

/// The correct choices whose formatting alone sets them apart: no wrong choice looks the same
/// (empty when every or no choice is correct).
pub fn key_in_formatting(choices: &[(&str, bool)]) -> Vec<usize> {
    let n_correct = choices.iter().filter(|c| c.1).count();
    if n_correct == 0 || n_correct == choices.len() {
        return Vec::new();
    }
    let sigs: Vec<Vec<&str>> = choices.iter().map(|c| decoration(c.0)).collect();
    if sigs.iter().all(|s| *s == sigs[0]) {
        return Vec::new();
    }
    // A correct choice whose look no wrong choice shares is singled out by its formatting
    // ("**Venus**" among plain choices, or a plain "Venus" among bold ones).
    let wrong: Vec<&Vec<&str>> = choices.iter().zip(&sigs).filter(|(c, _)| !c.1).map(|(_, s)| s).collect();
    choices.iter().enumerate().filter(|(i, c)| c.1 && !wrong.iter().any(|w| **w == sigs[*i])).map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(t: &[&str]) -> Option<Analysis> {
        analyze(t)
    }

    #[test]
    fn detection_is_broad() {
        for t in [
            vec!["A)Venus", "B)Mars", "C)Earth", "D)Jupiter"],
            vec!["A.Venus", "B.Mars", "C.Earth"],
            vec!["\u{24B6} Venus", "\u{24B7} Mars", "\u{24B8} Earth"],
            vec!["\u{2460} Venus", "\u{2461} Mars", "\u{2462} Earth"],
            vec!["Option A: Venus", "Option B: Mars", "Option C: Earth"],
            vec!["\u{3B1}) Venus", "\u{3B2}) Mars", "\u{3B3}) Earth"],
            vec!["\u{430}) Venus", "\u{431}) Mars", "\u{432}) Earth"],
            vec!["\u{661}) Venus", "\u{662}) Mars", "\u{663}) Earth"],
            vec!["A ) Venus", "B ) Mars", "C ) Earth"],
            vec!["Venus (A)", "Mars (B)", "Earth (C)"],
            vec!["B) Venus", "C) Mars", "D) Earth"],
            vec!["A\u{3001}Venus", "B\u{3001}Mars", "C\u{3001}Earth"],
            vec!["A/ Venus", "B/ Mars", "C/ Earth"],
            vec!["i) Venus", "ii) Mars", "iii) Earth"],
            vec!["A)\nVenus", "B)\nMars"],
            vec!["A)\tVenus", "B)\tMars"],
            vec!["\u{FF21}\u{FF0E}\u{6771}\u{4EAC}", "\u{FF22}\u{FF0E}\u{5927}\u{962A}"],
            vec!["\u{5D0}. \u{5E9}\u{5DC}\u{5D5}\u{5DD}", "\u{5D1}. \u{5DC}\u{5D4}\u{5EA}\u{5E8}\u{5D0}\u{5D5}\u{5EA}"],
            vec!["\u{30A2}. x", "\u{30A4}. y"],
            vec!["\u{2160}. x", "\u{2161}. y"],
        ] {
            let a = found(&t).unwrap_or_else(|| panic!("not found: {t:?}"));
            assert!(a.order_revealing, "{t:?}");
        }
    }

    #[test]
    fn strip_previews() {
        let after = |t: &[&str]| -> Vec<String> { found(t).unwrap().previews(t).into_iter().map(|p| p.after).collect() };
        assert!(found(&["A)Venus", "B)Mars", "C)Earth"]).unwrap().strip_allowed);
        assert_eq!(after(&["A)Venus", "B)Mars", "C)Earth"]), ["Venus", "Mars", "Earth"]);
        assert_eq!(after(&["Venus (A)", "Mars (B)"]), ["Venus", "Mars"]);
        assert_eq!(after(&["Option A: Venus", "Option B: Mars"])[1], "Mars");
        assert_eq!(after(&["A. (i) x", "B. (ii) y"])[1], "y", "two layers");
        // Strip refused: it would collide, empty a choice, or the labels are not 1, 2, 3...
        for t in [vec!["I. only", "II. only", "III. only"], vec!["A.", "B."], vec!["B) Venus", "C) Mars"], vec!["A. x", "B. y", "None of these"], vec!["A. x", "C. y"], vec!["A. x", "B. y", "D. z"]] {
            let a = found(&t).unwrap();
            assert!(!a.strip_allowed, "{t:?}");
            assert!(a.previews(&t).iter().all(|c| c.before == c.after), "{t:?}");
        }
        // A second layer that would merge the choices stops the removal; the first layer's
        // result ("A. x", "B. x") still numbers them, so Strip is refused outright.
        assert!(!found(&["A. A. x", "B. B. x"]).unwrap().strip_allowed);
        assert!(found(&["A. A. x", "B. B. y"]).unwrap().strip_allowed, "two clean layers");
    }

    #[test]
    fn text_is_not_mistaken_for_a_revealing_label() {
        // Detected (two labelled choices), but not order-revealing: Keep shuffles as usual.
        let a = found(&["E. coli", "S. aureus", "B. subtilis"]).unwrap();
        assert!(!a.order_revealing);
        assert!(!a.strip_allowed);
        // Not detected at all.
        for t in [vec!["A man", "B movie"], vec!["X-ray", "Y-axis"], vec!["1.5 kg", "2.5 kg"], vec!["Venus", "Mars"], vec!["U2", "R2"], vec!["e.g. x", "i.e. y"]] {
            assert!(found(&t).is_none(), "{t:?}");
        }
        // A lone label at its position is a finding; elsewhere it is not.
        assert!(found(&["A. Venus", "Mars"]).is_some());
        assert!(found(&["C. Venus", "Mars"]).is_none());
    }

    #[test]
    fn digits_of_any_script() {
        assert_eq!(digit_value('7'), Some(7));
        assert_eq!(digit_value('\u{661}'), Some(1));
        assert_eq!(digit_value('\u{967}'), Some(1));
        assert_eq!(digit_value('\u{1D7CF}'), Some(1), "mathematical bold one");
        assert_eq!(digit_value('\u{1D7E3}'), Some(1), "next block of mathematical digits");
        assert_eq!(digit_value('a'), None);
    }

    #[test]
    fn sibling_references() {
        for t in ["Both A and B", "A and C only", "I and II", "Neither A nor B", "II only", "only C", "A, B and C", "I, II and III"] {
            assert_eq!(sibling_ref(t), Some(SiblingRef::Labels), "{t}");
        }
        for t in ["All of the above", "None of these", "None of the others", "Todas las anteriores", "Toutes ces r\u{e9}ponses"] {
            assert_eq!(sibling_ref(t), Some(SiblingRef::Above), "{t}");
        }
        for t in ["Venus", "A large planet", "I think so", "Vitamin C", "Mars and Venus"] {
            assert_eq!(sibling_ref(t), None, "{t}");
        }
    }
}
