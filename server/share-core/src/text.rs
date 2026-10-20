//! Character rules and text normalisation.
//!
//! # Character rules
//!
//! Text is checked code point by code point **before** normalisation. A few code points are
//! *mapped* (CR, VT and the Unicode line/paragraph separators become line breaks; a BOM is
//! dropped); everything else that breaks a rule is **refused**, never silently cleaned, so what is
//! stored is what the author typed. Two classes:
//!
//! * **Block** text: stems, part stems, explanations, answer texts, match items, distractors,
//!   accepted blanks, samples, bank descriptions, text figure params. Multi-line.
//! * **Line** text: bank names, topics, authors, licence names and URLs, attribution names, and
//!   publish metadata (titles, tags, standards). One line.
//!
//! | Code points | Block | Line | Why |
//! |---|---|---|---|
//! | LF U+000A, TAB U+0009 | allowed | refused | stem markup is line based |
//! | CR U+000D (and CRLF), VT U+000B, LINE SEPARATOR U+2028, PARAGRAPH SEPARATOR U+2029 | mapped to LF | mapped to a space (then collapsed) | Windows, Word and PDF text |
//! | BOM / ZWNBSP U+FEFF | dropped | dropped | editors insert it |
//! | every other Cc (C0, DEL, C1 incl. NEL U+0085) | refused | refused | fake line breaks, terminal escapes |
//! | ZWNJ U+200C, ZWJ U+200D, LRM U+200E, RLM U+200F | allowed | allowed | Persian, Indic half-forms, emoji ZWJ sequences, mixed-direction text |
//! | ZWSP U+200B, ALM U+061C, soft hyphen U+00AD, U+2060-2064 (word joiner, invisible math operators) | allowed | refused | Thai/Khmer word breaks, Arabic, hyphenation, MathML |
//! | script format characters: Arabic number signs U+0600-0605, U+06DD, U+08E2; Syriac U+070F; Mongolian vowel separator U+180E; Kaithi U+110BD, U+110CD | allowed | refused | needed to write those scripts |
//! | tag characters U+E0020-E007F | allowed **only** as the flags of England, Scotland and Wales (U+1F3F4, the tags "gbeng" / "gbsct" / "gbwls", U+E007F: Unicode's recommended tag sequences) | same | any other run spells hidden ASCII |
//! | bidi embeddings and overrides U+202A-202E, isolates U+2066-2069 | refused | refused | reorder what a reader sees ("Trojan Source") |
//! | deprecated format controls U+206A-206F, interlinear annotations U+FFF9-FFFB, U+E0001, every other Cf | refused | refused | invisible or deprecated |
//! | surrogates (Cs), private use (Co), unassigned and noncharacters (Cn) | refused | refused | no meaning to share; Cn as of `UNICODE_VERSION` |
//! | Hangul fillers U+115F, U+1160, U+3164, U+FFA0; Braille blank U+2800 | refused | refused | render as nothing; used to fake blank text |
//! | more than [`MAX_MARK_RUN`] consecutive combining marks (Mn, Mc, Me), counted in canonical decomposition, with ZWJ/ZWNJ not ending a run | refused | refused | "Zalgo" stacking; also bounds NFC work |
//! | variation selectors U+FE00-FE0F, U+E0100-E01EF | allowed (they are Mn) | allowed | emoji presentation (U+FE0F), CJK variants |
//! | everything else assigned (letters, marks, numbers, punctuation, symbols, spaces) | allowed | allowed | |
//!
//! Then text is NFC-normalised (Line text also has whitespace runs collapsed and is trimmed), and
//! measured after normalisation; normalisation stops as soon as the limit is passed.
//!
//! Stems and choices must also be **visible**: contain at least one letter, number, punctuation
//! or symbol (L, N, P, S), not only spaces, marks and invisible characters ([`has_visible`]).
//!
//! The categories are those of `unicode-general-category` (Unicode [`UNICODE_VERSION`]); NFC is
//! `unicode-normalization`. Both are pinned exactly in `Cargo.toml`; the app links this crate and
//! the Worker runs it as wasm, so both sides always use the same tables.
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;

pub const UNICODE_VERSION: (u64, u64, u64) = unicode_general_category::UNICODE_VERSION;

/// The longest run of consecutive combining marks accepted. Tibetan stacks and Vedic accents
/// reach about ten; "Zalgo" text uses dozens.
pub const MAX_MARK_RUN: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Line,
    Block,
}

/// A length limit, in UTF-8 bytes or in characters, applied after normalisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Max {
    Bytes(usize),
    Chars(usize),
}

impl Max {
    pub fn limit(self) -> u64 {
        match self {
            Max::Bytes(n) | Max::Chars(n) => n as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextError {
    /// A refused code point at character index `offset` (and UTF-16 index `offset16`) of the
    /// input as sent.
    Forbidden { offset: usize, offset16: usize, cp: u32 },
    /// A run of more than `MAX_MARK_RUN` combining marks ending at `offset` / `offset16`.
    MarkRun { offset: usize, offset16: usize },
    TooLong,
}

fn is_mark(c: char) -> bool {
    matches!(get_general_category(c), GeneralCategory::NonspacingMark | GeneralCategory::SpacingMark | GeneralCategory::EnclosingMark)
}

fn is_tag(u: u32) -> bool {
    (0xE0020..=0xE007F).contains(&u)
}

/// True when `c` may never appear in text of class `class` on its own (the table above; tag
/// characters, which depend on their neighbours, are judged by [`check`]).
pub fn forbidden(c: char, class: Class) -> bool {
    let u = c as u32;
    match class {
        Class::Block if c == '\n' || c == '\t' => return false,
        Class::Block if matches!(u, 0xAD | 0x61C | 0x200B..=0x200F | 0x2060..=0x2064) => return false,
        Class::Block if matches!(u, 0x600..=0x605 | 0x6DD | 0x70F | 0x8E2 | 0x180E | 0x110BD | 0x110CD) => return false,
        Class::Line if matches!(u, 0x200C..=0x200F) => return false,
        _ => {}
    }
    if matches!(u, 0x115F | 0x1160 | 0x3164 | 0xFFA0 | 0x2800) {
        return true;
    }
    matches!(
        get_general_category(c),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::Surrogate
            | GeneralCategory::PrivateUse
            | GeneralCategory::Unassigned
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    )
}

/// The emoji tag sequences Unicode recommends (`RGI_Emoji_Tag_Sequence`): the flags of England,
/// Scotland and Wales. Any other run of tag characters could spell hidden ASCII.
const FLAG_TAGS: [&str; 3] = ["gbeng", "gbsct", "gbwls"];

/// Length of a recommended emoji tag sequence's tag part starting at `chars[i]` (which must
/// follow U+1F3F4): one of [`FLAG_TAGS`] in tag letters, then CANCEL TAG.
fn tag_sequence(chars: &[char], i: usize) -> Option<usize> {
    FLAG_TAGS.iter().find_map(|t| {
        let n = t.len();
        let ok = chars.len() > i + n
            && t.chars().zip(&chars[i..i + n]).all(|(a, b)| *b as u32 == 0xE0000 + a as u32)
            && chars[i + n] as u32 == 0xE007F;
        ok.then_some(n + 1)
    })
}

/// Advances a run of combining marks over `c`'s canonical decomposition (what NFC may leave as
/// marks): a base character starts a new run, each mark extends it. Returns the new run length.
fn mark_run(run: usize, c: char) -> usize {
    let mut r = run;
    unicode_normalization::char::decompose_canonical(c, |d| {
        if is_mark(d) {
            r += 1;
        } else {
            r = 0;
        }
    });
    r
}

/// Checks every code point of `s` (the table, the tag-sequence rule and the mark-run cap).
pub fn check(s: &str, class: Class) -> Result<(), TextError> {
    let chars: Vec<char> = s.chars().collect();
    let mut run = 0usize;
    let mut i = 0usize;
    let mut off16 = 0usize;
    while i < chars.len() {
        let c = chars[i];
        let u = c as u32;
        if u == 0x1F3F4
            && let Some(n) = tag_sequence(&chars, i + 1)
        {
            off16 += 2 + n * 2;
            i += 1 + n;
            run = 0;
            continue;
        }
        if is_tag(u) || forbidden(c, class) {
            return Err(TextError::Forbidden { offset: i, offset16: off16, cp: u });
        }
        // Marks are counted in their canonical decomposition (U+0F73 is two marks), so the cap
        // holds after NFC and the offset is the input's; ZWJ/ZWNJ neither count nor end a run, so
        // they cannot be used to split one.
        if !matches!(u, 0x200C | 0x200D) {
            run = mark_run(run, c);
            if run > MAX_MARK_RUN {
                return Err(TextError::MarkRun { offset: i, offset16: off16 });
            }
        }
        off16 += c.len_utf16();
        i += 1;
    }
    Ok(())
}

/// The mapped code points: line breaks unified, BOM dropped. Offsets in errors refer to the
/// input as sent, so the mapping keeps one char per char except CRLF (which counts as its CR).
fn map(s: &str, class: Class) -> std::borrow::Cow<'_, str> {
    if !s.contains(['\r', '\u{B}', '\u{2028}', '\u{2029}', '\u{FEFF}']) {
        return std::borrow::Cow::Borrowed(s);
    }
    let nl = if class == Class::Block { '\n' } else { ' ' };
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push(nl);
            }
            '\u{B}' | '\u{2028}' | '\u{2029}' => out.push(nl),
            '\u{FEFF}' => {}
            c => out.push(c),
        }
    }
    std::borrow::Cow::Owned(out)
}

/// NFC into a `String`, stopping (with `None`) as soon as the output passes `max`.
fn nfc_bounded(s: &str, max: Max) -> Option<String> {
    let mut out = String::with_capacity(s.len().min(64 * 1024));
    let mut chars = 0usize;
    for c in s.nfc() {
        out.push(c);
        chars += 1;
        let over = match max {
            Max::Bytes(n) => out.len() > n,
            // Line text is collapsed afterwards, which only shortens it; allow slack for that.
            Max::Chars(n) => chars > n.saturating_mul(4).saturating_add(64),
        };
        if over {
            return None;
        }
    }
    Some(out)
}

/// Checks and canonicalises one text value: maps line breaks and the BOM, refuses forbidden
/// characters and long mark runs, then applies NFC (Line: whitespace collapsed and trimmed) and
/// enforces `max`.
pub fn clean(s: &str, class: Class, max: Max) -> Result<String, TextError> {
    // Offsets are reported against the input, so check it before mapping, ignoring what `map`
    // will turn into something allowed.
    let mapped = map(s, class);
    if let Err(e) = check(&mapped, class) {
        return Err(remap_offset(s, &mapped, e, class));
    }
    let n = nfc_bounded(&mapped, max).ok_or(TextError::TooLong)?;
    let out = match class {
        Class::Block => n,
        Class::Line => collapse(&n),
    };
    let ok = match max {
        Max::Bytes(m) => out.len() <= m,
        Max::Chars(m) => out.chars().count() <= m,
    };
    if !ok {
        return Err(TextError::TooLong);
    }
    // NFC composes only assigned characters; the check is cheap and keeps the guarantee local.
    check(&out, class)?;
    Ok(out)
}

/// Translates an error offset in the mapped text back to the input as sent: `map` keeps one
/// char per char except CRLF (two become one) and the BOM (dropped).
fn remap_offset(orig: &str, mapped: &str, e: TextError, _class: Class) -> TextError {
    if orig.len() == mapped.len() && orig == mapped {
        return e;
    }
    let target = match e {
        TextError::Forbidden { offset, .. } | TextError::MarkRun { offset, .. } => offset,
        TextError::TooLong => return e,
    };
    // For each mapped char, its (char index, UTF-16 offset) in the input.
    let mut pos: Vec<(usize, usize)> = Vec::new();
    let chars: Vec<char> = orig.chars().collect();
    let (mut i, mut o16) = (0usize, 0usize);
    while i < chars.len() {
        let c = chars[i];
        if c != '\u{FEFF}' {
            pos.push((i, o16));
        }
        if c == '\r' && chars.get(i + 1) == Some(&'\n') {
            o16 += 1;
            i += 1;
        }
        o16 += chars[i].len_utf16();
        i += 1;
    }
    let (offset, offset16) = pos.get(target).copied().unwrap_or((chars.len(), o16));
    match e {
        TextError::Forbidden { cp, .. } => TextError::Forbidden { offset, offset16, cp },
        TextError::MarkRun { .. } => TextError::MarkRun { offset, offset16 },
        TextError::TooLong => e,
    }
}

/// Like `clean` (Line) but with NFKC, for publish metadata (titles, tags, standards), matching
/// `cleanText` in `server/auth/src/validate.ts`.
pub fn clean_meta(s: &str, max_chars: usize) -> Result<String, TextError> {
    let mapped = map(s, Class::Line);
    if let Err(e) = check(&mapped, Class::Line) {
        return Err(remap_offset(s, &mapped, e, Class::Line));
    }
    let mut n = String::new();
    for (i, c) in mapped.nfkc().enumerate() {
        if i > max_chars.saturating_mul(4).saturating_add(64) {
            return Err(TextError::TooLong);
        }
        n.push(c);
    }
    let out = collapse(&n);
    if out.chars().count() > max_chars {
        return Err(TextError::TooLong);
    }
    check(&out, Class::Line)?;
    Ok(out)
}

/// Runs of Unicode whitespace become one ASCII space; leading and trailing whitespace go.
pub fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for w in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

/// At least one letter, number, punctuation mark or symbol.
pub fn has_visible(s: &str) -> bool {
    s.chars().any(is_visible)
}

pub fn is_visible(c: char) -> bool {
    !c.is_whitespace()
        && !is_tag(c as u32)
        && matches!(
            get_general_category(c),
            GeneralCategory::UppercaseLetter
                | GeneralCategory::LowercaseLetter
                | GeneralCategory::TitlecaseLetter
                | GeneralCategory::ModifierLetter
                | GeneralCategory::OtherLetter
                | GeneralCategory::DecimalNumber
                | GeneralCategory::LetterNumber
                | GeneralCategory::OtherNumber
                | GeneralCategory::ConnectorPunctuation
                | GeneralCategory::DashPunctuation
                | GeneralCategory::OpenPunctuation
                | GeneralCategory::ClosePunctuation
                | GeneralCategory::InitialPunctuation
                | GeneralCategory::FinalPunctuation
                | GeneralCategory::OtherPunctuation
                | GeneralCategory::MathSymbol
                | GeneralCategory::CurrencySymbol
                | GeneralCategory::ModifierSymbol
                | GeneralCategory::OtherSymbol
        )
        && !matches!(c as u32, 0x115F | 0x1160 | 0x3164 | 0xFFA0 | 0x2800)
}

/// The comparison form used by `match_hash`: NFC; within each line, runs of spaces and tabs
/// collapsed to one space and the ends trimmed; line breaks kept (a table is not a sentence);
/// leading and trailing blank lines removed.
pub fn hash_form(s: &str) -> String {
    let n: String = map(s, Class::Block).nfc().collect();
    let lines: Vec<String> = n.split('\n').map(collapse).collect();
    lines.join("\n").trim_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    const B: Max = Max::Bytes(1 << 20);

    #[test]
    fn refused_everywhere() {
        for bad in [
            "a\u{0}b", "a\u{7}b", "a\u{1b}b", "a\u{7f}b", "a\u{85}b", "a\u{202A}b", "a\u{202B}b", "a\u{202C}b", "a\u{202D}b",
            "a\u{202E}b", "a\u{2066}b", "a\u{2067}b", "a\u{2068}b", "a\u{2069}b", "a\u{206A}b", "a\u{206F}b", "a\u{FFF9}b",
            "a\u{FFFB}b", "a\u{E0001}b", "a\u{1D173}b", "a\u{E000}b", "a\u{F8FF}b", "a\u{F0000}b", "a\u{FDD0}b", "a\u{FFFE}b",
            "a\u{10FFFF}b", "a\u{0378}b", "a\u{115F}b", "a\u{1160}b", "a\u{3164}b", "a\u{FFA0}b", "a\u{2800}b",
            // Tag characters outside a complete flag sequence.
            "a\u{E0041}b", "Pick\u{E0049}\u{E0047}\u{E004E}", "\u{1F3F4}\u{E0067}\u{E0062}", "\u{1F3F4}\u{E0067}\u{E007F}",
            "x\u{E007F}",
        ] {
            assert!(clean(bad, Class::Line, B).is_err(), "{}", bad.escape_unicode());
            assert!(clean(bad, Class::Block, B).is_err(), "{}", bad.escape_unicode());
        }
    }

    #[test]
    fn mapped_not_refused() {
        assert_eq!(clean("a\r\nb\rc\u{B}d\u{2028}e\u{2029}f", Class::Block, B).as_deref(), Ok("a\nb\nc\nd\ne\nf"));
        assert_eq!(clean("\u{FEFF}What?", Class::Block, B).as_deref(), Ok("What?"));
        assert_eq!(clean("a\u{2028}b\u{B}c", Class::Line, B).as_deref(), Ok("a b c"));
        assert_eq!(clean("\u{FEFF}Title", Class::Line, B).as_deref(), Ok("Title"));
    }

    #[test]
    fn allowed_in_block_text() {
        for good in [
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
            "\u{2764}\u{FE0F}",
            "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}",
            "\u{915}\u{94D}\u{200D}\u{937}",
            "\u{5D4}\u{5E0}\u{5D5}\u{5E1}\u{5D7}\u{5D4} \u{200E}H\u{2082}O\u{200E}",
            "x \u{61C} y", "\u{E2A}\u{E27}\u{E31}\u{E2A}\u{200B}\u{E14}\u{E35}",
            "photo\u{AD}synthesis", "2\u{2062}x", "a\u{2060}b",
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}",
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} flag?",
            "\u{1F44B}\u{1F3FD}", "\u{1D465}", "x\u{302} + y\u{303}", "\u{8FBB}\u{E0100}", "a\tb\nc",
            "\u{1820}\u{180E}\u{1820}", "\u{1820}\u{180B}", "x\u{110BD}", "\u{600}123", "\u{70F}\u{710}", "\u{6DD}1", "\u{8E2}1",
        ] {
            assert!(clean(good, Class::Block, B).is_ok(), "{}", good.escape_unicode());
            assert!(has_visible(good), "{}", good.escape_unicode());
        }
        assert!(clean("\u{645}\u{6CC}\u{200C}\u{62E}", Class::Line, B).is_ok());
        assert!(clean("\u{1F468}\u{200D}\u{1F469}", Class::Line, B).is_ok());
        assert!(clean("\u{5D0} \u{200F}x", Class::Line, B).is_ok());
        assert!(clean("\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}", Class::Line, B).is_ok(), "a flag in a topic");
        for bad in ["a\u{200B}b", "a\u{AD}b", "a\u{2062}b", "\u{600}1", "a\nb", "a\tb"] {
            assert!(clean(bad, Class::Line, B).is_err(), "{}", bad.escape_unicode());
        }
    }

    #[test]
    fn offsets_count_the_input_as_sent() {
        assert_eq!(clean("ab\u{202E}", Class::Block, B), Err(TextError::Forbidden { offset: 2, offset16: 2, cp: 0x202E }));
        // An astral character before the refused one: char offset 1, UTF-16 offset 2.
        assert_eq!(clean("\u{1D465}\u{202E}", Class::Block, B), Err(TextError::Forbidden { offset: 1, offset16: 2, cp: 0x202E }));
        // A dropped BOM and a CRLF before it do not shift the reported position.
        assert_eq!(clean("\u{FEFF}a\r\nb\u{202E}", Class::Block, B), Err(TextError::Forbidden { offset: 5, offset16: 5, cp: 0x202E }));
    }

    #[test]
    fn mark_runs_and_visibility() {
        assert!(clean(&format!("a{}", "\u{301}".repeat(MAX_MARK_RUN)), Class::Block, B).is_ok());
        // A Tibetan stack of nine marks is real text.
        assert!(clean("\u{F40}\u{F7A}\u{F74}\u{F71}\u{F72}\u{F80}\u{F82}\u{F83}\u{F37}\u{F39}", Class::Block, B).is_ok());
        assert_eq!(clean(&format!("a{}", "\u{301}".repeat(MAX_MARK_RUN + 1)), Class::Block, B), Err(TextError::MarkRun { offset: MAX_MARK_RUN + 1, offset16: MAX_MARK_RUN + 1 }));
        for blank in ["", "   ", "\u{200B}\u{200D}", "\u{301}", "\n\t", "\u{AD}", "\u{3164}"] {
            assert!(!has_visible(blank), "{}", blank.escape_unicode());
        }
    }

    #[test]
    fn bounded_normalisation() {
        let s = "\u{1D160}".repeat(10_000);
        assert_eq!(clean(&s, Class::Block, Max::Bytes(8192)), Err(TextError::TooLong));
        assert!(nfc_bounded(&s, Max::Bytes(8192)).is_none());
        assert_eq!(nfc_bounded("abc", Max::Bytes(3)).as_deref(), Some("abc"));
        assert_eq!(clean("e\u{301}", Class::Block, Max::Bytes(2)).as_deref(), Ok("\u{e9}"));
        assert_eq!(clean("  a \u{3000}  b ", Class::Line, B).as_deref(), Ok("a b"));
        assert_eq!(clean_meta("x\u{b2} \u{FB01}", 64).as_deref(), Ok("x2 fi"));
    }

    #[test]
    fn hash_form_keeps_lines() {
        assert_eq!(hash_form("  a  |\tb \n c | d\n\n"), "a | b\nc | d");
        assert_ne!(hash_form("a | b\nc | d"), hash_form("a | b c | d"));
        assert_ne!(hash_form("one\n\ntwo"), hash_form("one two"));
        assert_eq!(hash_form("Cafe\u{301}"), "Caf\u{e9}");
    }
}
