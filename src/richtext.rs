//! Stem markup: how a question stem (a plain `String`) is structured on the exam page, in the
//! exports and in previews. One parser serves every surface, so a table or bullet list looks the
//! same in the preview, the PDF, the DOCX and the answer key. See `docs/stem-format.md`.
//!
//! * Lines are separated by `\n`; a blank line starts a new paragraph.
//! * `• item`, `- item`, `* item` lines are a bullet list; two or more `1. item` lines a numbered one.
//! * `> text` lines are a quote.
//! * Two or more consecutive lines with the same number of ` | `-separated cells are a table; a
//!   `---|---` line right after the first row (or a first row without blanks above rows with `___`
//!   blanks) marks the first row as a header.
//! * Inline: `**bold**`, `*italic*`, `~~strikethrough~~`, `_{sub}`, `^{sup}`, `$math$` (left to
//!   `equation`), `[text](https://...)` links (http, https and mailto only; see [`link_at`]), and
//!   `___` blanks stay plain text. Everything else is literal.
//! * `# `, `## `, `### ` lines are headings (a line with a `___` blank stays text), a line that is
//!   only `---` is a divider, and `::: side` / `::: next` / `:::` fence lines put the blocks
//!   between them side by side (one panel per `::: next`; without the closing `:::` the fences
//!   are text).
//! A stem with none of this is one paragraph of one line, exactly as before.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub sub: bool,
    pub sup: bool,
    /// `~~struck through~~`.
    pub strike: bool,
    /// Raw `$...$` content (without the dollars); the renderer typesets it.
    pub math: bool,
    /// The URL of a `[text](url)` link this text belongs to.
    pub link: Option<String>,
}

pub type Line = Vec<Span>;

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Lines of one paragraph (each `\n` is a hard line break).
    Para(Vec<Line>),
    Bullets(Vec<Line>),
    Numbered(Vec<Line>),
    Quote(Vec<Line>),
    Table { header: bool, rows: Vec<Vec<Line>> },
    /// `# ` (1) to `### ` (3) heading line.
    Heading(u8, Line),
    /// A `---` line.
    Divider,
    /// `::: side` ... `::: next` ... `:::`: each panel's blocks, laid out side by side.
    Side(Vec<Vec<Block>>),
}

/// True when the stem needs anything beyond one plain line.
pub fn has_markup(stem: &str) -> bool {
    stem.contains('\n') || stem.contains("**") || stem.contains("~~") || stem.contains("_{") || stem.contains("^{") || stem.contains('$') || heading(stem).is_some() || divider(stem) || has_link(stem)
}

/// Does the stem hold a `[text](url)` link anywhere (a paragraph, list item, table cell, ...)?
pub fn has_link(s: &str) -> bool {
    if !s.contains("](") {
        return false;
    }
    let mut found = false;
    walk(&parse(s), &mut |b| {
        let mut lines = |ls: &[Line]| found |= ls.iter().flatten().any(|sp| sp.link.is_some());
        match b {
            Block::Para(ls) | Block::Bullets(ls) | Block::Numbered(ls) | Block::Quote(ls) => lines(ls),
            Block::Heading(_, l) => lines(std::slice::from_ref(l)),
            Block::Table { rows, .. } => rows.iter().for_each(|r| lines(r)),
            Block::Divider | Block::Side(_) => {}
        }
    });
    found
}

/// A URL a link may point at: `http://`, `https://` or `mailto:` and something after it, with
/// no whitespace or parentheses (an unbalanced `)` would end the link early).
pub fn link_url_ok(u: &str) -> bool {
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).or_else(|| u.strip_prefix("mailto:"));
    rest.is_some_and(|r| !r.is_empty()) && !u.chars().any(|c| c.is_whitespace() || c == '(' || c == ')' || c.is_control())
}

/// A `[text](url)` link opening at char `i` of a line: the indices of its `]` and of its closing
/// `)`. The text is non-empty and holds no `[`, `]`, `$`, `{` or `}` (so math, scripts and other
/// links never straddle it); the URL must pass [`link_url_ok`]. Anything else stays literal.
pub fn link_at(ch: &[char], i: usize) -> Option<(usize, usize)> {
    if ch.get(i) != Some(&'[') {
        return None;
    }
    let close = (i + 1..ch.len()).find(|&j| ch[j] == ']')?;
    if close == i + 1 || ch[i + 1..close].iter().any(|c| matches!(c, '[' | '$' | '{' | '}')) || ch.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = (close + 2..ch.len()).find(|&j| ch[j] == ')')?;
    let url: String = ch[close + 2..end].iter().collect();
    link_url_ok(&url).then_some((close, end))
}

/// The URL a typed address stands for: kept when it already has an allowed scheme, `mailto:`
/// added to an e-mail address, `https://` to anything else that looks like a host; None when
/// it cannot be a link (empty, spaces, parentheses).
pub fn normalize_url(u: &str) -> Option<String> {
    let u = u.trim();
    let full = if link_url_ok(u) {
        u.to_string()
    } else if u.contains("://") || u.starts_with("mailto:") {
        return None;
    } else if u.contains('@') && !u.contains('/') {
        format!("mailto:{u}")
    } else if u.contains('.') {
        format!("https://{u}")
    } else {
        return None;
    };
    link_url_ok(&full).then_some(full)
}

/// A heading line: `#`, `##` or `###` at the very start, then a space; the level and the text.
/// A line holding a `___` blank is not a heading (it reads as a "# of ..." prompt).
pub fn heading(l: &str) -> Option<(u8, &str)> {
    let n = l.bytes().take_while(|&b| b == b'#').count();
    if !(1..=3).contains(&n) {
        return None;
    }
    let rest = l[n..].strip_prefix(' ')?;
    (!rest.contains("___")).then_some((n as u8, rest))
}

/// Chars before a heading's text (`## ` is 3).
pub fn heading_prefix_len(l: &str) -> usize {
    heading(l).map_or(0, |(n, _)| n as usize + 1)
}

/// A divider line: only `---` (surrounding spaces allowed).
pub fn divider(l: &str) -> bool {
    l.trim() == "---"
}

/// Side-by-side fence lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fence {
    Open,
    Next,
    Close,
}

pub const FENCE_OPEN: &str = "::: side";
pub const FENCE_NEXT: &str = "::: next";
pub const FENCE_CLOSE: &str = ":::";

pub fn fence(l: &str) -> Option<Fence> {
    match l.trim() {
        FENCE_OPEN => Some(Fence::Open),
        FENCE_NEXT => Some(Fence::Next),
        FENCE_CLOSE => Some(Fence::Close),
        _ => None,
    }
}

/// End (exclusive, after the closing fence) of a side-by-side group opening at line `i`, when
/// the opener has a closer and no other opener comes first.
fn side_end(lines: &[&str], i: usize) -> Option<usize> {
    if fence(lines[i]) != Some(Fence::Open) {
        return None;
    }
    for (j, l) in lines.iter().enumerate().skip(i + 1) {
        match fence(l) {
            Some(Fence::Close) => return Some(j + 1),
            Some(Fence::Open) => return None,
            _ => {}
        }
    }
    None
}

/// Line ranges of the panels of a side-by-side segment (between its fence lines).
pub fn side_panels(lines: &[&str], seg: &Seg) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut from = seg.start + 1;
    for j in seg.start + 1..seg.end {
        if matches!(fence(lines[j]), Some(Fence::Next) | Some(Fence::Close)) {
            out.push(from..j);
            from = j + 1;
        }
    }
    out
}

/// One block of a stem and the source lines it came from (`start..end`, indices into the stem's
/// `\n`-separated lines). Blank lines between blocks belong to no segment. [`parse`] is built on
/// this, so the editor (which edits the source lines in place) and every renderer agree.
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub kind: SegKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegKind {
    Para,
    Bullets,
    Numbered,
    Quote,
    /// Header: a `---|---` line after the first row, or implied by blanks (see [`parse`]).
    Table { header: bool },
    /// One heading line of this level.
    Heading(u8),
    /// One `---` line.
    Divider,
    /// A side-by-side group, fence lines included (panels: [`side_panels`]).
    Side,
}

/// The block structure of `lines` (see [`Seg`]).
pub fn segment(lines: &[&str]) -> Vec<Seg> {
    let mut segs: Vec<Seg> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        // Side by side, heading and divider lines.
        if let Some(end) = side_end(lines, i) {
            segs.push(Seg { kind: SegKind::Side, start: i, end });
            i = end;
            continue;
        }
        if let Some((n, _)) = heading(l) {
            segs.push(Seg { kind: SegKind::Heading(n), start: i, end: i + 1 });
            i += 1;
            continue;
        }
        if divider(l) {
            segs.push(Seg { kind: SegKind::Divider, start: i, end: i + 1 });
            i += 1;
            continue;
        }
        // Table: >= 2 consecutive pipe rows with the same cell count.
        if let Some(cols) = cells(l).map(|c| c.len()) {
            let mut j = i;
            let mut raw: Vec<Vec<String>> = Vec::new();
            while j < lines.len() {
                match cells(lines[j]) {
                    Some(c) if c.len() == cols || is_separator(&c) => {
                        raw.push(c);
                        j += 1;
                    }
                    _ => break,
                }
            }
            if raw.iter().filter(|r| !is_separator(r)).count() >= 2 {
                let sep_at = raw.iter().position(|r| is_separator(r));
                let mut header = sep_at == Some(1);
                let rows_raw: Vec<Vec<String>> = raw.into_iter().filter(|r| !is_separator(r)).collect();
                if !header && !rows_raw[0].iter().any(|c| c.contains("___")) && rows_raw[1..].iter().any(|r| r.iter().any(|c| c.contains("___"))) {
                    header = true;
                }
                segs.push(Seg { kind: SegKind::Table { header }, start: i, end: j });
                i = j;
                continue;
            }
        }
        // Bullet / numbered / quote runs.
        let run = |i: &mut usize, f: fn(&str) -> Option<&str>| {
            while *i < lines.len() && f(lines[*i]).is_some() {
                *i += 1;
            }
        };
        let s = i;
        if bullet(l).is_some() {
            run(&mut i, bullet);
            segs.push(Seg { kind: SegKind::Bullets, start: s, end: i });
            continue;
        }
        if numbered(l).is_some() && i + 1 < lines.len() && numbered(lines[i + 1]).is_some() {
            run(&mut i, numbered);
            segs.push(Seg { kind: SegKind::Numbered, start: s, end: i });
            continue;
        }
        if quote(l).is_some() {
            run(&mut i, quote);
            segs.push(Seg { kind: SegKind::Quote, start: s, end: i });
            continue;
        }
        // Paragraph: lines until a blank line or something that starts another block.
        while i < lines.len() && !lines[i].trim().is_empty() {
            if i > s && (bullet(lines[i]).is_some() || quote(lines[i]).is_some() || starts_table(lines, i) || heading(lines[i]).is_some() || divider(lines[i]) || side_end(lines, i).is_some()) {
                break;
            }
            i += 1;
        }
        segs.push(Seg { kind: SegKind::Para, start: s, end: i });
    }
    segs
}

pub fn parse(stem: &str) -> Vec<Block> {
    let lines: Vec<&str> = stem.split('\n').collect();
    let mut blocks: Vec<Block> = segment(&lines)
        .into_iter()
        .map(|sg| {
            let ls = &lines[sg.start..sg.end];
            match sg.kind {
                SegKind::Para => Block::Para(ls.iter().map(|l| inline(l.trim_end())).collect()),
                SegKind::Bullets => Block::Bullets(ls.iter().map(|l| inline(bullet(l).unwrap_or(l))).collect()),
                SegKind::Numbered => Block::Numbered(ls.iter().map(|l| inline(numbered(l).unwrap_or(l))).collect()),
                SegKind::Quote => Block::Quote(ls.iter().map(|l| inline(quote(l).unwrap_or(l))).collect()),
                SegKind::Table { header } => Block::Table {
                    header,
                    rows: ls
                        .iter()
                        .filter_map(|l| cells(l))
                        .filter(|c| !is_separator(c))
                        .map(|r| r.iter().map(|c| inline(c)).collect())
                        .collect(),
                },
                SegKind::Heading(n) => Block::Heading(n, inline(heading(ls[0]).map_or(ls[0], |h| h.1).trim_end())),
                SegKind::Divider => Block::Divider,
                SegKind::Side => Block::Side(side_panels(&lines, &sg).into_iter().map(|r| parse(&lines[r].join("\n"))).collect()),
            }
        })
        .collect();
    if blocks.is_empty() {
        blocks.push(Block::Para(vec![Vec::new()]));
    }
    blocks
}

/// Chars of `line` before a list/quote item's text (leading spaces + marker), for the kind of
/// segment it is in.
pub fn item_prefix_len(kind: SegKind, line: &str) -> usize {
    let rest = match kind {
        SegKind::Bullets => bullet(line),
        SegKind::Numbered => numbered(line),
        SegKind::Quote => quote(line),
        _ => None,
    };
    rest.map_or(0, |r| line.chars().count() - r.chars().count())
}

/// Char ranges of a pipe row's cell texts (trimmed, as [`cells`] reads them); an empty cell is an
/// empty range where typing lands inside its padding. None when the line is not a row.
pub fn cell_ranges(line: &str) -> Option<Vec<std::ops::Range<usize>>> {
    let ch: Vec<char> = line.chars().collect();
    cells(line)?;
    let mut s = ch.iter().position(|c| !c.is_whitespace()).unwrap_or(ch.len());
    let mut e = ch.iter().rposition(|c| !c.is_whitespace()).map_or(s, |i| i + 1);
    if s < e && ch[s] == '|' {
        s += 1;
    }
    if e > s && ch[e - 1] == '|' {
        e -= 1;
    }
    let mut out = Vec::new();
    let mut a = s;
    for k in s..=e {
        if k == e || ch[k] == '|' {
            let (mut x, mut y) = (a, k);
            while x < y && ch[x].is_whitespace() {
                x += 1;
            }
            while y > x && ch[y - 1].is_whitespace() {
                y -= 1;
            }
            if x == y && k > a {
                // all padding: put the slot after the first space so typed text is padded
                x = (a + 1).min(k);
                y = x;
            }
            out.push(x..y);
            a = k + 1;
        }
    }
    Some(out)
}

/// Is this line a table header separator (`---|---`)?
pub fn is_separator_line(line: &str) -> bool {
    cells(line).is_some_and(|c| is_separator(&c))
}

fn starts_table(lines: &[&str], i: usize) -> bool {
    match (cells(lines[i]), lines.get(i + 1).and_then(|l| cells(l))) {
        (Some(a), Some(b)) => a.len() == b.len(),
        _ => false,
    }
}

/// Cells of a pipe row (`a | b | c`), or None when the line is not one (< 2 cells).
pub(crate) fn cells(line: &str) -> Option<Vec<String>> {
    if !line.contains('|') {
        return None;
    }
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let c: Vec<String> = t.split('|').map(|x| x.trim().to_string()).collect();
    (c.len() >= 2).then_some(c)
}

fn is_separator(c: &[String]) -> bool {
    !c.is_empty() && c.iter().all(|x| !x.is_empty() && x.chars().all(|ch| matches!(ch, '-' | ':' | ' ')) && x.contains('-'))
}

pub(crate) fn bullet(l: &str) -> Option<&str> {
    let t = l.trim_start();
    for p in ["• ", "- ", "* ", "· ", "– "] {
        if let Some(r) = t.strip_prefix(p) {
            return Some(r);
        }
    }
    None
}

pub(crate) fn numbered(l: &str) -> Option<&str> {
    let t = l.trim_start();
    let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if d == 0 || d > 2 {
        return None;
    }
    let rest = &t[d..];
    rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))
}

pub(crate) fn quote(l: &str) -> Option<&str> {
    let t = l.trim_start();
    t.strip_prefix("> ").or_else(|| t.strip_prefix('>'))
}

/// What a stretch of one line's source is (see [`inline_runs`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    /// Markup that is not shown: `**`, `*`, `~~`, `_{`, `^{`, `}`.
    Marker,
    /// Shown text with the style of the run.
    Text,
    /// A whole `$...$` (dollars included).
    Math,
}

/// A stretch of a line's source: chars `start..end`, its kind and the style in force.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub start: usize,
    pub end: usize,
    pub kind: RunKind,
    pub bold: bool,
    pub italic: bool,
    pub sub: bool,
    pub sup: bool,
    pub strike: bool,
    /// Part of a `[text](url)` link: its `[` and `](url)` markers and the text between them.
    pub link: bool,
}

/// One line's source cut into runs that cover it exactly, in order: markers, styled text and math.
/// [`inline`] is built on this, and the editor styles its text from the same runs. A `_{}` gives an
/// empty sub run between its markers.
pub fn inline_runs(s: &str) -> Vec<Run> {
    let ch: Vec<char> = s.chars().collect();
    let mut out: Vec<Run> = Vec::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    let mut text_start = 0usize;
    // Inside a link: the indices of its `]` and closing `)`.
    let mut link: Option<(usize, usize)> = None;
    let run = |start, end, kind, bold, italic, sub, sup, strike, link: Option<(usize, usize)>| Run { start, end, kind, bold, italic, sub, sup, strike, link: link.is_some() };
    let flush = |out: &mut Vec<Run>, from: usize, to: usize, bold: bool, italic: bool, strike: bool, link: Option<(usize, usize)>| {
        if to > from {
            out.push(run(from, to, RunKind::Text, bold, italic, false, false, strike, link));
        }
    };
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        // [text](url): the `[` and the `](url)` are markers, the text keeps its own styles.
        if let Some((close, end)) = link
            && i == close
        {
            flush(&mut out, text_start, i, bold, italic, strike, link);
            out.push(run(close, end + 1, RunKind::Marker, bold, italic, false, false, strike, link));
            link = None;
            i = end + 1;
            text_start = i;
            continue;
        }
        if link.is_none()
            && c == '['
            && let Some(l) = link_at(&ch, i)
        {
            flush(&mut out, text_start, i, bold, italic, strike, None);
            link = Some(l);
            out.push(run(i, i + 1, RunKind::Marker, bold, italic, false, false, strike, link));
            i += 1;
            text_start = i;
            continue;
        }
        // $math$
        if c == '$'
            && let Some(end) = (i + 1..ch.len()).find(|&j| ch[j] == '$')
            && end > i + 1
        {
            flush(&mut out, text_start, i, bold, italic, strike, link);
            out.push(run(i, end + 1, RunKind::Math, false, false, false, false, false, link));
            i = end + 1;
            text_start = i;
            continue;
        }
        // _{sub} / ^{sup}
        if (c == '_' || c == '^')
            && ch.get(i + 1) == Some(&'{')
            && let Some(end) = (i + 2..ch.len()).find(|&j| ch[j] == '}')
        {
            flush(&mut out, text_start, i, bold, italic, strike, link);
            out.push(run(i, i + 2, RunKind::Marker, bold, italic, false, false, strike, link));
            out.push(run(i + 2, end, RunKind::Text, bold, italic, c == '_', c == '^', strike, link));
            out.push(run(end, end + 1, RunKind::Marker, bold, italic, false, false, strike, link));
            i = end + 1;
            text_start = i;
            continue;
        }
        // **bold**
        if c == '*' && ch.get(i + 1) == Some(&'*') {
            let closes = bold || (i + 2..ch.len().saturating_sub(1)).any(|j| ch[j] == '*' && ch[j + 1] == '*');
            if closes {
                flush(&mut out, text_start, i, bold, italic, strike, link);
                out.push(run(i, i + 2, RunKind::Marker, bold, italic, false, false, strike, link));
                bold = !bold;
                i += 2;
                text_start = i;
                continue;
            }
        }
        // ~~strike~~: the opener hugs the next char and needs a closer that hugs the previous one
        // (so a stray `~~` or `a ~~ b` stays literal)
        if c == '~' && ch.get(i + 1) == Some(&'~') {
            let closes = strike && i > 0 && !ch[i - 1].is_whitespace();
            let opens = !strike
                && ch.get(i + 2).is_some_and(|n| !n.is_whitespace() && *n != '~')
                && (i + 3..ch.len().saturating_sub(1)).any(|j| ch[j] == '~' && ch[j + 1] == '~' && !ch[j - 1].is_whitespace());
            if opens || closes {
                flush(&mut out, text_start, i, bold, italic, strike, link);
                out.push(run(i, i + 2, RunKind::Marker, bold, italic, false, false, strike, link));
                strike = !strike;
                i += 2;
                text_start = i;
                continue;
            }
        }
        // *italic*: opener hugs the next word, closer the previous one (so `a * b` stays literal)
        if c == '*' {
            let opens = !italic && ch.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != '*') && (i == 0 || !ch[i - 1].is_alphanumeric());
            let closes = italic && i > 0 && !ch[i - 1].is_whitespace();
            let has_close = opens && (i + 2..ch.len()).any(|j| ch[j] == '*' && !ch[j - 1].is_whitespace() && ch.get(j + 1) != Some(&'*'));
            if has_close || closes {
                flush(&mut out, text_start, i, bold, italic, strike, link);
                out.push(run(i, i + 1, RunKind::Marker, bold, italic, false, false, strike, link));
                italic = !italic;
                i += 1;
                text_start = i;
                continue;
            }
        }
        i += 1;
    }
    flush(&mut out, text_start, ch.len(), bold, italic, strike, link);
    out
}

/// Inline spans of one line: `$math$`, `**bold**`, `*italic*`, `~~strike~~`, `_{sub}`, `^{sup}`,
/// `[text](url)`.
pub fn inline(s: &str) -> Line {
    let ch: Vec<char> = s.chars().collect();
    let mut url: Option<String> = None;
    inline_runs(s)
        .into_iter()
        .filter_map(|r| match r.kind {
            RunKind::Marker => {
                if r.link && ch[r.start] == '[' {
                    url = link_at(&ch, r.start).map(|(close, end)| ch[close + 2..end].iter().collect());
                } else if r.link && ch[r.start] == ']' {
                    url = None;
                }
                None
            }
            RunKind::Math => Some(Span { text: ch[r.start + 1..r.end - 1].iter().collect(), math: true, ..Default::default() }),
            RunKind::Text => Some(Span {
                text: ch[r.start..r.end].iter().collect(),
                bold: r.bold,
                italic: r.italic,
                sub: r.sub,
                sup: r.sup,
                strike: r.strike,
                math: false,
                link: if r.link { url.clone() } else { None },
            }),
        })
        .collect()
}

/// Text of a line with the markup removed (math stays `$..$`; sub/sup become Unicode digits and
/// signs where every character has a glyph, else keep their braces).
pub fn line_text(line: &Line) -> String {
    line.iter()
        .map(|s| {
            if s.math {
                format!("${}$", s.text)
            } else if s.sub {
                unicode_script(&s.text, false).unwrap_or_else(|| format!("_{{{}}}", s.text))
            } else if s.sup {
                unicode_script(&s.text, true).unwrap_or_else(|| format!("^{{{}}}", s.text))
            } else {
                s.text.clone()
            }
        })
        .collect()
}

/// One-line preview (bank cards, search, lists): markup stripped, line breaks as spaces, table
/// rows joined with " · ", cells with " | ".
pub fn plain(stem: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    plain_blocks(&parse(stem), &mut parts);
    parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn plain_blocks(blocks: &[Block], parts: &mut Vec<String>) {
    for b in blocks.iter().cloned() {
        match b {
            Block::Heading(_, l) => parts.push(line_text(&l)),
            Block::Divider => {}
            Block::Side(panels) => {
                for p in &panels {
                    plain_blocks(p, parts);
                }
            }
            Block::Para(ls) | Block::Quote(ls) => parts.push(ls.iter().map(line_text).collect::<Vec<_>>().join(" ")),
            Block::Bullets(ls) => parts.push(ls.iter().map(|l| format!("• {}", line_text(l))).collect::<Vec<_>>().join(" ")),
            Block::Numbered(ls) => parts.push(ls.iter().enumerate().map(|(i, l)| format!("{}. {}", i + 1, line_text(l))).collect::<Vec<_>>().join(" ")),
            Block::Table { rows, .. } => parts.push(rows.iter().map(|r| r.iter().map(line_text).collect::<Vec<_>>().join(" | ")).collect::<Vec<_>>().join(" · ")),
        }
    }
}

/// Every block, panels of side-by-side groups included (a group itself is visited first).
pub fn walk(blocks: &[Block], f: &mut dyn FnMut(&Block)) {
    for b in blocks {
        f(b);
        if let Block::Side(panels) = b {
            for p in panels {
                walk(p, f);
            }
        }
    }
}

/// Does the stem use the heading / divider / side-by-side extensions anywhere (stems written
/// before them read exactly as they did when this is false)?
pub fn uses_extensions(stem: &str) -> bool {
    let lines: Vec<&str> = stem.split('\n').collect();
    segment(&lines).iter().any(|s| matches!(s.kind, SegKind::Heading(_) | SegKind::Divider | SegKind::Side))
}

/// Sub/superscript text as Unicode where every character has a glyph, else None.
pub fn unicode_script(text: &str, sup: bool) -> Option<String> {
    let map = |c: char| -> Option<char> {
        let (s, u) = match c {
            '0' => ('⁰', '₀'), '1' => ('¹', '₁'), '2' => ('²', '₂'), '3' => ('³', '₃'), '4' => ('⁴', '₄'),
            '5' => ('⁵', '₅'), '6' => ('⁶', '₆'), '7' => ('⁷', '₇'), '8' => ('⁸', '₈'), '9' => ('⁹', '₉'),
            '+' => ('⁺', '₊'), '-' | '−' => ('⁻', '₋'), '=' => ('⁼', '₌'), '(' => ('⁽', '₍'), ')' => ('⁾', '₎'),
            'n' => ('ⁿ', 'ₙ'), 'i' => ('ⁱ', 'ᵢ'),
            _ => return None,
        };
        Some(if sup { s } else { u })
    };
    text.chars().map(map).collect()
}


/// `inline_runs` as it was before links (P3), kept to prove links changed nothing else.
#[cfg(test)]
mod legacy_inline {
    use super::{Run, RunKind};
    pub fn inline_runs(s: &str) -> Vec<Run> {
        let ch: Vec<char> = s.chars().collect();
        let mut out: Vec<Run> = Vec::new();
        let (mut bold, mut italic, mut strike) = (false, false, false);
        let mut text_start = 0usize;
        let run = |start, end, kind, bold, italic, sub, sup, strike| Run { start, end, kind, bold, italic, sub, sup, strike, link: false };
        let flush = |out: &mut Vec<Run>, from: usize, to: usize, bold: bool, italic: bool, strike: bool| {
            if to > from {
                out.push(run(from, to, RunKind::Text, bold, italic, false, false, strike));
            }
        };
        let mut i = 0;
        while i < ch.len() {
            let c = ch[i];
            // $math$
            if c == '$'
                && let Some(end) = (i + 1..ch.len()).find(|&j| ch[j] == '$')
                && end > i + 1
            {
                flush(&mut out, text_start, i, bold, italic, strike);
                out.push(run(i, end + 1, RunKind::Math, false, false, false, false, false));
                i = end + 1;
                text_start = i;
                continue;
            }
            // _{sub} / ^{sup}
            if (c == '_' || c == '^')
                && ch.get(i + 1) == Some(&'{')
                && let Some(end) = (i + 2..ch.len()).find(|&j| ch[j] == '}')
            {
                flush(&mut out, text_start, i, bold, italic, strike);
                out.push(run(i, i + 2, RunKind::Marker, bold, italic, false, false, strike));
                out.push(run(i + 2, end, RunKind::Text, bold, italic, c == '_', c == '^', strike));
                out.push(run(end, end + 1, RunKind::Marker, bold, italic, false, false, strike));
                i = end + 1;
                text_start = i;
                continue;
            }
            // **bold**
            if c == '*' && ch.get(i + 1) == Some(&'*') {
                let closes = bold || (i + 2..ch.len().saturating_sub(1)).any(|j| ch[j] == '*' && ch[j + 1] == '*');
                if closes {
                    flush(&mut out, text_start, i, bold, italic, strike);
                    out.push(run(i, i + 2, RunKind::Marker, bold, italic, false, false, strike));
                    bold = !bold;
                    i += 2;
                    text_start = i;
                    continue;
                }
            }
            // ~~strike~~: the opener hugs the next char and needs a closer that hugs the previous one
            // (so a stray `~~` or `a ~~ b` stays literal)
            if c == '~' && ch.get(i + 1) == Some(&'~') {
                let closes = strike && i > 0 && !ch[i - 1].is_whitespace();
                let opens = !strike
                    && ch.get(i + 2).is_some_and(|n| !n.is_whitespace() && *n != '~')
                    && (i + 3..ch.len().saturating_sub(1)).any(|j| ch[j] == '~' && ch[j + 1] == '~' && !ch[j - 1].is_whitespace());
                if opens || closes {
                    flush(&mut out, text_start, i, bold, italic, strike);
                    out.push(run(i, i + 2, RunKind::Marker, bold, italic, false, false, strike));
                    strike = !strike;
                    i += 2;
                    text_start = i;
                    continue;
                }
            }
            // *italic*: opener hugs the next word, closer the previous one (so `a * b` stays literal)
            if c == '*' {
                let opens = !italic && ch.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != '*') && (i == 0 || !ch[i - 1].is_alphanumeric());
                let closes = italic && i > 0 && !ch[i - 1].is_whitespace();
                let has_close = opens && (i + 2..ch.len()).any(|j| ch[j] == '*' && !ch[j - 1].is_whitespace() && ch.get(j + 1) != Some(&'*'));
                if has_close || closes {
                    flush(&mut out, text_start, i, bold, italic, strike);
                    out.push(run(i, i + 1, RunKind::Marker, bold, italic, false, false, strike));
                    italic = !italic;
                    i += 1;
                    text_start = i;
                    continue;
                }
            }
            i += 1;
        }
        flush(&mut out, text_start, ch.len(), bold, italic, strike);
        out
    }
}

/// The parser as it was before `segment`/`inline_runs` (kept to prove the refactor changed nothing).
#[cfg(test)]
mod reference {
    use super::{bullet, cells, is_separator, numbered, quote, starts_table, Block, Line, Span};
    pub fn parse(stem: &str) -> Vec<Block> {
        let lines: Vec<&str> = stem.split('\n').collect();
        let mut blocks: Vec<Block> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() {
                i += 1;
                continue;
            }
            // Table: >= 2 consecutive pipe rows with the same cell count.
            if let Some(cols) = cells(l).map(|c| c.len()) {
                let mut j = i;
                let mut raw: Vec<Vec<String>> = Vec::new();
                while j < lines.len() {
                    match cells(lines[j]) {
                        Some(c) if c.len() == cols || is_separator(&c) => {
                            raw.push(c);
                            j += 1;
                        }
                        _ => break,
                    }
                }
                if raw.iter().filter(|r| !is_separator(r)).count() >= 2 {
                    let sep_at = raw.iter().position(|r| is_separator(r));
                    let mut header = sep_at == Some(1);
                    let rows_raw: Vec<Vec<String>> = raw.into_iter().filter(|r| !is_separator(r)).collect();
                    if !header && !rows_raw[0].iter().any(|c| c.contains("___")) && rows_raw[1..].iter().any(|r| r.iter().any(|c| c.contains("___"))) {
                        header = true;
                    }
                    let rows = rows_raw.iter().map(|r| r.iter().map(|c| inline(c)).collect()).collect();
                    blocks.push(Block::Table { header, rows });
                    i = j;
                    continue;
                }
            }
            // Bullet / numbered / quote runs.
            if bullet(l).is_some() {
                let mut items = Vec::new();
                while i < lines.len() {
                    match bullet(lines[i]) {
                        Some(t) => items.push(inline(t)),
                        None => break,
                    }
                    i += 1;
                }
                blocks.push(Block::Bullets(items));
                continue;
            }
            if numbered(l).is_some() && i + 1 < lines.len() && numbered(lines[i + 1]).is_some() {
                let mut items = Vec::new();
                while i < lines.len() {
                    match numbered(lines[i]) {
                        Some(t) => items.push(inline(t)),
                        None => break,
                    }
                    i += 1;
                }
                blocks.push(Block::Numbered(items));
                continue;
            }
            if quote(l).is_some() {
                let mut items = Vec::new();
                while i < lines.len() {
                    match quote(lines[i]) {
                        Some(t) => items.push(inline(t)),
                        None => break,
                    }
                    i += 1;
                }
                blocks.push(Block::Quote(items));
                continue;
            }
            // Paragraph: lines until a blank line or something that starts another block.
            let mut para = Vec::new();
            while i < lines.len() && !lines[i].trim().is_empty() {
                if !para.is_empty() && (bullet(lines[i]).is_some() || quote(lines[i]).is_some() || starts_table(&lines, i)) {
                    break;
                }
                para.push(inline(lines[i].trim_end()));
                i += 1;
            }
            blocks.push(Block::Para(para));
        }
        if blocks.is_empty() {
            blocks.push(Block::Para(vec![Vec::new()]));
        }
        blocks
    }

    pub fn inline(s: &str) -> Line {
        let ch: Vec<char> = s.chars().collect();
        let mut out: Line = Vec::new();
        let mut cur = String::new();
        let (mut bold, mut italic) = (false, false);
        let push = |out: &mut Line, cur: &mut String, bold: bool, italic: bool| {
            if !cur.is_empty() {
                out.push(Span { text: std::mem::take(cur), bold, italic, ..Default::default() });
            }
        };
        let mut i = 0;
        while i < ch.len() {
            let c = ch[i];
            // $math$
            if c == '$'
                && let Some(end) = (i + 1..ch.len()).find(|&j| ch[j] == '$')
                && end > i + 1
            {
                push(&mut out, &mut cur, bold, italic);
                out.push(Span { text: ch[i + 1..end].iter().collect(), math: true, ..Default::default() });
                i = end + 1;
                continue;
            }
            // _{sub} / ^{sup}
            if (c == '_' || c == '^')
                && ch.get(i + 1) == Some(&'{')
                && let Some(end) = (i + 2..ch.len()).find(|&j| ch[j] == '}')
            {
                push(&mut out, &mut cur, bold, italic);
                let text: String = ch[i + 2..end].iter().collect();
                out.push(Span { text, bold, italic, sub: c == '_', sup: c == '^', ..Default::default() });
                i = end + 1;
                continue;
            }
            // **bold**
            if c == '*' && ch.get(i + 1) == Some(&'*') {
                let closes = bold || (i + 2..ch.len().saturating_sub(1)).any(|j| ch[j] == '*' && ch[j + 1] == '*');
                if closes {
                    push(&mut out, &mut cur, bold, italic);
                    bold = !bold;
                    i += 2;
                    continue;
                }
            }
            // *italic*: opener hugs the next word, closer the previous one (so `a * b` stays literal)
            if c == '*' {
                let opens = !italic && ch.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != '*') && (i == 0 || !ch[i - 1].is_alphanumeric());
                let closes = italic && i > 0 && !ch[i - 1].is_whitespace();
                let has_close = opens && (i + 2..ch.len()).any(|j| ch[j] == '*' && !ch[j - 1].is_whitespace() && ch.get(j + 1) != Some(&'*'));
                if has_close || closes {
                    push(&mut out, &mut cur, bold, italic);
                    italic = !italic;
                    i += 1;
                    continue;
                }
            }
            cur.push(c);
            i += 1;
        }
        push(&mut out, &mut cur, bold, italic);
        out
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn a_plain_stem_is_one_paragraph_one_line() {
        assert_eq!(parse("Which gas?"), vec![Block::Para(vec![inline("Which gas?")])]);
        assert!(!has_markup("Which gas?"));
    }

    #[test]
    fn newlines_are_hard_breaks_blank_lines_split_paragraphs() {
        let b = parse("one\ntwo\n\nthree");
        assert_eq!(b.len(), 2);
        assert!(matches!(&b[0], Block::Para(l) if l.len() == 2));
        assert!(matches!(&b[1], Block::Para(l) if l.len() == 1));
    }

    #[test]
    fn bullets_numbered_and_quote_blocks() {
        let b = parse("Do all:\n• Draw it.\n• Count them.\n1. first\n2. second\n> quoted");
        assert!(matches!(&b[0], Block::Para(_)));
        assert!(matches!(&b[1], Block::Bullets(l) if l.len() == 2));
        assert!(matches!(&b[2], Block::Numbered(l) if l.len() == 2));
        assert!(matches!(&b[3], Block::Quote(l) if l.len() == 1));
        // A lone "1. x" line is not a list.
        assert!(matches!(&parse("1. only")[0], Block::Para(_)));
    }

    #[test]
    fn pipe_rows_become_a_table_with_a_header_when_marked_or_implied() {
        let b = parse("Name the compounds.\nFormula | Ionic or Covalent? | Name\nKI | ___ | ___\nSO₃ | ___ | ___");
        assert!(matches!(&b[0], Block::Para(_)));
        match &b[1] {
            Block::Table { header, rows } => {
                assert!(*header, "blank-free first row above blank rows is a header");
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[1].len(), 3);
            }
            other => panic!("{other:?}"),
        }
        let sep = parse("a | b\n---|---\n1 | 2");
        assert!(matches!(&sep[0], Block::Table { header: true, rows } if rows.len() == 2));
        // One pipe line alone, or ragged rows, are not tables.
        assert!(matches!(&parse("x | y")[0], Block::Para(_)));
        assert!(matches!(&parse("a | b\nc | d | e")[0], Block::Para(_)));
    }

    #[test]
    fn inline_bold_italic_script_and_math() {
        let l = inline("a **bold** and *it* x_{2} y^{3} $x^2$ 2 * 3");
        assert!(l.iter().any(|s| s.bold && s.text == "bold"));
        assert!(l.iter().any(|s| s.italic && s.text == "it"));
        assert!(l.iter().any(|s| s.sub && s.text == "2"));
        assert!(l.iter().any(|s| s.sup && s.text == "3"));
        assert!(l.iter().any(|s| s.math && s.text == "x^2"));
        assert!(line_text(&l).ends_with("2 * 3"), "a spaced * stays literal: {}", line_text(&l));
        // Blanks and unmatched markers are literal.
        assert_eq!(line_text(&inline("fill ___ and ** open")), "fill ___ and ** open");
    }

    #[test]
    fn plain_preview_flattens_everything() {
        assert_eq!(plain("Do all:\n• Draw.\n• Count."), "Do all: • Draw. • Count.");
        assert_eq!(plain("Name:\nA | B\n1 | 2"), "Name: A | B · 1 | 2");
        assert_eq!(plain("**Bold** text"), "Bold text");
        assert_eq!(t(&plain("x")), "x");
    }

    #[test]
    fn plain_turns_scripts_into_unicode() {
        assert_eq!(plain("H_{2}O and Ca^{2+}"), "H₂O and Ca²⁺");
        assert_eq!(plain("x_{ab}"), "x_{ab}");
    }

    #[test]
    fn unicode_script_maps_digits_and_signs() {
        assert_eq!(unicode_script("2", false).as_deref(), Some("₂"));
        assert_eq!(unicode_script("2+", true).as_deref(), Some("²⁺"));
        assert_eq!(unicode_script("ab", true), None);
    }

    /// Tiny deterministic generator for the property tests (no extra crates).
    pub(crate) struct Rng(pub u64);
    impl Rng {
        pub(crate) fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        pub(crate) fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
            v[(self.next() % v.len() as u64) as usize]
        }
    }

    /// Random markup built from the tokens the parser cares about.
    pub(crate) fn random_markup(r: &mut Rng, max: usize) -> String {
        const TOK: &[&str] = &[
            "a", "word", "b", " ", " ", "*", "**", "_{", "^{", "}", "$", "x^2", "___", "|", " | ", "\n", "\n", "• ", "- ", "* ",
            "1. ", "2. ", "> ", ">", "---|---", "H", "_", "^", "{", "2", "+", "é", "  ", "\n\n", "| ", " |", "1/2", "$x$", "**b**", "*i*",
        ];
        let n = (r.next() % max as u64) as usize;
        (0..n).map(|_| r.pick(TOK)).collect()
    }

    #[test]
    fn property_refactored_parser_matches_the_original() {
        let mut r = Rng(0x9e37_79b9_7f4a_7c15);
        for _ in 0..20_000 {
            let s = random_markup(&mut r, 40);
            assert_eq!(parse(&s), reference::parse(&s), "stem {s:?}");
            for line in s.split('\n') {
                assert_eq!(inline(line), reference::inline(line), "line {line:?}");
            }
        }
    }

    #[test]
    fn property_runs_cover_each_line_exactly() {
        let mut r = Rng(42);
        for _ in 0..20_000 {
            let s = random_markup(&mut r, 30);
            for line in s.split('\n') {
                let runs = inline_runs(line);
                let mut at = 0;
                for run in &runs {
                    assert_eq!(run.start, at, "{line:?} {runs:?}");
                    assert!(run.end >= run.start);
                    at = run.end;
                }
                assert_eq!(at, line.chars().count(), "{line:?}");
            }
        }
    }

    #[test]
    fn property_cell_ranges_read_what_cells_reads() {
        let mut r = Rng(7);
        for _ in 0..20_000 {
            let s = random_markup(&mut r, 20);
            for line in s.split('\n') {
                let (Some(c), Some(rg)) = (cells(line), cell_ranges(line)) else {
                    assert_eq!(cells(line).is_some(), cell_ranges(line).is_some());
                    continue;
                };
                let ch: Vec<char> = line.chars().collect();
                let got: Vec<String> = rg.iter().map(|x| ch[x.clone()].iter().collect()).collect();
                assert_eq!(got, c, "{line:?}");
            }
        }
    }

    #[test]
    fn strikethrough_markup() {
        let l = inline("a ~~old~~ new **~~b~~** ~~x_{2}~~");
        assert!(l.iter().any(|s| s.strike && s.text == "old" && !s.bold));
        assert!(l.iter().any(|s| s.strike && s.bold && s.text == "b"));
        assert!(l.iter().any(|s| s.strike && s.sub && s.text == "2"));
        assert!(l.iter().any(|s| !s.strike && s.text.contains("new")));
        assert_eq!(line_text(&l), "a old new b x₂");
        // stray or spaced tildes stay literal
        for lit in ["about ~~5 g", "a ~~ b ~~ c", "~~", "x~~", "~~ a~~"] {
            let l = inline(lit);
            assert!(l.iter().all(|s| !s.strike), "{lit:?}");
            assert_eq!(line_text(&l), lit);
        }
        assert!(has_markup("~~a~~") && !has_markup("a ~ b"));
        assert_eq!(plain("~~gone~~ kept"), "gone kept");
    }

    #[test]
    fn segments_and_prefixes() {
        let lines: Vec<&str> = "Intro\n• a\n  - b\n\n1. x\n2. y\n> q\nA | B\n---|---\n1 | 2".split('\n').collect();
        let segs = segment(&lines);
        let kinds: Vec<SegKind> = segs.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, vec![SegKind::Para, SegKind::Bullets, SegKind::Numbered, SegKind::Quote, SegKind::Table { header: true }]);
        assert_eq!((segs[4].start, segs[4].end), (7, 10));
        assert_eq!(item_prefix_len(SegKind::Bullets, "  - b"), 4);
        assert_eq!(item_prefix_len(SegKind::Numbered, "12. x"), 4);
        assert_eq!(item_prefix_len(SegKind::Quote, ">q"), 1);
        assert!(is_separator_line("---|:--:"));
        assert_eq!(cell_ranges("|  | b |"), Some(vec![2..2, 5..6]));
    }
    /// `segment` as it was before headings, dividers and side-by-side (round 6).
    fn legacy_segment(lines: &[&str]) -> Vec<Seg> {
        let mut segs: Vec<Seg> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() {
                i += 1;
                continue;
            }
            // Table: >= 2 consecutive pipe rows with the same cell count.
            if let Some(cols) = cells(l).map(|c| c.len()) {
                let mut j = i;
                let mut raw: Vec<Vec<String>> = Vec::new();
                while j < lines.len() {
                    match cells(lines[j]) {
                        Some(c) if c.len() == cols || is_separator(&c) => {
                            raw.push(c);
                            j += 1;
                        }
                        _ => break,
                    }
                }
                if raw.iter().filter(|r| !is_separator(r)).count() >= 2 {
                    let sep_at = raw.iter().position(|r| is_separator(r));
                    let mut header = sep_at == Some(1);
                    let rows_raw: Vec<Vec<String>> = raw.into_iter().filter(|r| !is_separator(r)).collect();
                    if !header && !rows_raw[0].iter().any(|c| c.contains("___")) && rows_raw[1..].iter().any(|r| r.iter().any(|c| c.contains("___"))) {
                        header = true;
                    }
                    segs.push(Seg { kind: SegKind::Table { header }, start: i, end: j });
                    i = j;
                    continue;
                }
            }
            // Bullet / numbered / quote runs.
            let run = |i: &mut usize, f: fn(&str) -> Option<&str>| {
                while *i < lines.len() && f(lines[*i]).is_some() {
                    *i += 1;
                }
            };
            let s = i;
            if bullet(l).is_some() {
                run(&mut i, bullet);
                segs.push(Seg { kind: SegKind::Bullets, start: s, end: i });
                continue;
            }
            if numbered(l).is_some() && i + 1 < lines.len() && numbered(lines[i + 1]).is_some() {
                run(&mut i, numbered);
                segs.push(Seg { kind: SegKind::Numbered, start: s, end: i });
                continue;
            }
            if quote(l).is_some() {
                run(&mut i, quote);
                segs.push(Seg { kind: SegKind::Quote, start: s, end: i });
                continue;
            }
            // Paragraph: lines until a blank line or something that starts another block.
            while i < lines.len() && !lines[i].trim().is_empty() {
                if i > s && (bullet(lines[i]).is_some() || quote(lines[i]).is_some() || starts_table(lines, i)) {
                    break;
                }
                i += 1;
            }
            segs.push(Seg { kind: SegKind::Para, start: s, end: i });
        }
        segs
    }

    /// Random markup that also uses the round-6 extensions (and near misses of them).
    pub(crate) fn random_markup_ext(r: &mut Rng, max: usize) -> String {
        const TOK: &[&str] = &[
            "a", "word", " ", "*", "**", "~~", "_{", "}", "$x$", "___", " | ", "| ", "\n", "\n", "\n\n", "• ", "1. ", "2. ", "> ", "---|---",
            "# ", "## ", "### ", "#### ", "#", " # ", "---", " --- ", "----", "::: side", "::: next", ":::", ":: side", "::: sides", "H", "é",
        ];
        let n = (r.next() % max as u64) as usize;
        (0..n).map(|_| r.pick(TOK)).collect()
    }

    #[test]
    fn property_stems_without_extensions_segment_as_before() {
        let mut r = Rng(0x5eed_1234_abcd_0001);
        let mut checked = 0;
        for k in 0..60_000 {
            let s = if k % 2 == 0 { random_markup(&mut r, 40) } else { random_markup_ext(&mut r, 40) };
            if uses_extensions(&s) {
                continue;
            }
            checked += 1;
            let lines: Vec<&str> = s.split('\n').collect();
            assert_eq!(segment(&lines), legacy_segment(&lines), "stem {s:?}");
        }
        assert!(checked > 30_000, "{checked}");
    }

    #[test]
    fn property_extension_segments_cover_lines_in_order() {
        let mut r = Rng(99);
        for _ in 0..20_000 {
            let s = random_markup_ext(&mut r, 40);
            let lines: Vec<&str> = s.split('\n').collect();
            let segs = segment(&lines);
            let mut at = 0;
            for sg in &segs {
                assert!(sg.start >= at && sg.end > sg.start && sg.end <= lines.len(), "{s:?} {segs:?}");
                if sg.kind == SegKind::Side {
                    let panels = side_panels(&lines, sg);
                    assert!(!panels.is_empty());
                    assert_eq!(fence(lines[sg.start]), Some(Fence::Open));
                    assert_eq!(fence(lines[sg.end - 1]), Some(Fence::Close));
                }
                at = sg.end;
            }
            // parse never panics and plain is one line
            assert!(!plain(&s).contains('\n'));
        }
    }

    /// `inline` and `parse` exactly as before links, on the frozen run cutter.
    fn legacy_line(s: &str) -> Line {
        let ch: Vec<char> = s.chars().collect();
        super::legacy_inline::inline_runs(s)
            .into_iter()
            .filter_map(|r| match r.kind {
                RunKind::Marker => None,
                RunKind::Math => Some(Span { text: ch[r.start + 1..r.end - 1].iter().collect(), math: true, ..Default::default() }),
                RunKind::Text => Some(Span { text: ch[r.start..r.end].iter().collect(), bold: r.bold, italic: r.italic, sub: r.sub, sup: r.sup, strike: r.strike, ..Default::default() }),
            })
            .collect()
    }
    fn legacy_parse(stem: &str) -> Vec<Block> {
        let lines: Vec<&str> = stem.split('\n').collect();
        let mut blocks: Vec<Block> = segment(&lines)
            .into_iter()
            .map(|sg| {
                let ls = &lines[sg.start..sg.end];
                match sg.kind {
                    SegKind::Para => Block::Para(ls.iter().map(|l| legacy_line(l.trim_end())).collect()),
                    SegKind::Bullets => Block::Bullets(ls.iter().map(|l| legacy_line(bullet(l).unwrap_or(l))).collect()),
                    SegKind::Numbered => Block::Numbered(ls.iter().map(|l| legacy_line(numbered(l).unwrap_or(l))).collect()),
                    SegKind::Quote => Block::Quote(ls.iter().map(|l| legacy_line(quote(l).unwrap_or(l))).collect()),
                    SegKind::Table { header } => Block::Table { header, rows: ls.iter().filter_map(|l| cells(l)).filter(|c| !is_separator(c)).map(|r| r.iter().map(|c| legacy_line(c)).collect()).collect() },
                    SegKind::Heading(n) => Block::Heading(n, legacy_line(heading(ls[0]).map_or(ls[0], |h| h.1).trim_end())),
                    SegKind::Divider => Block::Divider,
                    SegKind::Side => Block::Side(side_panels(&lines, &sg).into_iter().map(|r| legacy_parse(&lines[r].join("\n"))).collect()),
                }
            })
            .collect();
        if blocks.is_empty() {
            blocks.push(Block::Para(vec![Vec::new()]));
        }
        blocks
    }

    /// Random markup with links and near misses of them (bad schemes, spaces, missing parts).
    fn random_markup_links(r: &mut Rng, max: usize) -> String {
        const TOK: &[&str] = &[
            "a", "word", " ", "*", "**", "~~", "_{", "}", "$x$", "$", "___", " | ", "\n", "\n\n", "• ", "1. ", "> ", "[", "]", "(", ")", "](", "[a](https://x.org)",
            "[t](http://a.b/c?d=1)", "[m](mailto:me@x.org)", "[x](ftp://x)", "[x](https://)", "[x]( https://a)", "[](https://a)", "[a b](https://a b)", "https://x.org", "mailto:",
            "[**b**](https://b.c)", "[x](javascript:alert(1))", "[a[b](https://c)", "]](", "[x]", "(https://a)", "é", "H", "2",
        ];
        let n = (r.next() % max as u64) as usize;
        (0..n).map(|_| r.pick(TOK)).collect()
    }

    /// Lines without a valid link cut into exactly the runs the pre-link parser made, and every
    /// stem without one parses as before.
    #[test]
    fn property_lines_without_links_read_as_before() {
        let mut r = Rng(0x11_4e_ab_cd);
        let mut plain_lines = 0;
        let mut link_lines = 0;
        for k in 0..60_000 {
            let s = if k % 3 == 0 { random_markup(&mut r, 40) } else { random_markup_links(&mut r, 30) };
            for line in s.split('\n') {
                let runs = inline_runs(line);
                if runs.iter().any(|x| x.link) {
                    link_lines += 1;
                    // runs still cover the line exactly
                    let mut at = 0;
                    for x in &runs {
                        assert_eq!(x.start, at, "{line:?}");
                        at = x.end;
                    }
                    assert_eq!(at, line.chars().count());
                    continue;
                }
                plain_lines += 1;
                assert_eq!(runs, super::legacy_inline::inline_runs(line), "line {line:?}");
                assert!(inline(line).iter().all(|sp| sp.link.is_none()));
            }
            if !has_link(&s) {
                assert_eq!(parse(&s), legacy_parse(&s), "stem {s:?}");
            }
        }
        assert!(plain_lines > 30_000 && link_lines > 5_000, "{plain_lines} {link_lines}");
    }

    #[test]
    fn links_parse_with_their_url_and_styles() {
        let l = inline("See [the **notes**](https://ex.org/a?b=1) or [mail me](mailto:t@s.org).");
        assert_eq!(line_text(&l), "See the notes or mail me.");
        assert!(l.iter().any(|s| s.text == "the " && s.link.as_deref() == Some("https://ex.org/a?b=1") && !s.bold));
        assert!(l.iter().any(|s| s.text == "notes" && s.bold && s.link.as_deref() == Some("https://ex.org/a?b=1")));
        assert!(l.iter().any(|s| s.text == "mail me" && s.link.as_deref() == Some("mailto:t@s.org")));
        assert!(l.iter().any(|s| s.text == " or " && s.link.is_none()));
        assert_eq!(plain("Read [this](https://a.org) now"), "Read this now");
        assert!(has_markup("[x](https://a.org)") && has_link("[x](http://a.org)"));
        // Literal: other schemes, spaces, empty text or URL, unclosed pieces.
        for lit in ["[x](ftp://a.org)", "[x](javascript:alert(1))", "[x](https://a b)", "[](https://a.org)", "[x](https://)", "[x] (https://a.org)", "[x](https://a.org", "a [b] c", "[a $x$](https://a.org)"] {
            assert!(inline(lit).iter().all(|s| s.link.is_none()), "{lit:?}");
            assert!(!has_link(lit), "{lit:?}");
        }
        assert_eq!(line_text(&inline("[x](ftp://a.org)")), "[x](ftp://a.org)");
        // A link inside a table cell and a list item.
        let b = parse("• go to [site](https://s.org)\nA | B\n[c](https://c.org) | d");
        assert!(matches!(&b[0], Block::Bullets(ls) if ls[0].iter().any(|s| s.link.is_some())));
        assert!(matches!(&b[1], Block::Table { rows, .. } if rows[1][0].iter().any(|s| s.link.as_deref() == Some("https://c.org"))));
        assert_eq!(normalize_url("example.org/x").as_deref(), Some("https://example.org/x"));
        assert_eq!(normalize_url(" https://a.org ").as_deref(), Some("https://a.org"));
        assert_eq!(normalize_url("me@school.edu").as_deref(), Some("mailto:me@school.edu"));
        assert_eq!(normalize_url("ftp://a.org"), None);
        assert_eq!(normalize_url("hello"), None);
        assert_eq!(normalize_url("a b.org"), None);
    }

    #[test]
    fn headings_dividers_and_side_by_side() {
        let b = parse("# Title\nText under\n---\n## Part **B**\n### c");
        assert_eq!(b.len(), 5);
        assert!(matches!(&b[0], Block::Heading(1, l) if line_text(l) == "Title"));
        assert!(matches!(&b[1], Block::Para(ls) if ls.len() == 1));
        assert_eq!(b[2], Block::Divider);
        assert!(matches!(&b[3], Block::Heading(2, l) if l.iter().any(|s| s.bold && s.text == "B")));
        assert!(matches!(&b[4], Block::Heading(3, _)));
        // not headings
        for lit in ["#Title", "#### four", " # indented", "# of protons: ___", "a # b"] {
            assert!(parse(lit).iter().all(|b| !matches!(b, Block::Heading(..))), "{lit:?}");
        }
        assert!(!divider("----") && !divider("--- x") && divider(" --- "));
        let b = parse("Look:\n::: side\nleft **one**\n• l1\n• l2\n::: next\nright\n:::\nafter");
        assert_eq!(b.len(), 3);
        let Block::Side(panels) = &b[1] else { panic!("{b:?}") };
        assert_eq!(panels.len(), 2);
        assert!(matches!(&panels[0][1], Block::Bullets(items) if items.len() == 2));
        assert!(matches!(&panels[1][0], Block::Para(_)));
        assert_eq!(plain("::: side\na\n::: next\nb\n:::\n# H\n---"), "a b H");
        // unclosed / nested fences stay text
        assert!(parse("::: side\na\n::: next\nb").iter().all(|b| !matches!(b, Block::Side(_))));
        let nested = parse("::: side\n::: side\nx\n:::\n");
        assert!(matches!(&nested[0], Block::Para(_)) && matches!(&nested[1], Block::Side(p) if p.len() == 1));
        // empty panels still parse
        assert!(matches!(&parse("::: side\n::: next\n:::")[0], Block::Side(p) if p.len() == 2));
        assert!(has_markup("# Title") && has_markup("---") && !has_markup("# of ___"));
        assert!(uses_extensions("a\n# b") && !uses_extensions("a # b\n#x"));
    }
}
