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
    /// `` `inline code` `` (printed in a monospace face; the markup inside is literal).
    pub code: bool,
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
    stem.contains('\n') || stem.contains("**") || stem.contains("~~") || stem.contains("_{") || stem.contains("^{") || stem.contains('$') || stem.contains('`') || heading(stem).is_some() || divider(stem) || has_link(stem)
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
    /// `::: center` (round 7): the blocks up to the next `:::` are centred.
    Center,
}

pub const FENCE_OPEN: &str = "::: side";
pub const FENCE_NEXT: &str = "::: next";
pub const FENCE_CLOSE: &str = ":::";
pub const FENCE_CENTER: &str = "::: center";

pub fn fence(l: &str) -> Option<Fence> {
    match l.trim() {
        FENCE_OPEN => Some(Fence::Open),
        FENCE_NEXT => Some(Fence::Next),
        FENCE_CLOSE => Some(Fence::Close),
        FENCE_CENTER => Some(Fence::Center),
        _ => None,
    }
}

/// End (exclusive, after its closing `:::`) of a centred group opening at line `i`: the next
/// fence line must be that closer (a centred group holds no side-by-side group, no `::: next`
/// and no other centred group; without its closer the `::: center` line is text).
pub fn center_end(lines: &[&str], i: usize) -> Option<usize> {
    if fence(lines[i]) != Some(Fence::Center) {
        return None;
    }
    for (j, l) in lines.iter().enumerate().skip(i + 1) {
        match fence(l) {
            Some(Fence::Close) => return Some(j + 1),
            Some(_) => return None,
            None => {}
        }
    }
    None
}

/// End (exclusive, after the closing fence) of a side-by-side group opening at line `i`, when
/// the opener has a closer and no other opener comes first. Centred groups inside a panel are
/// skipped whole (their `:::` closes them, not the panel).
fn side_end(lines: &[&str], i: usize) -> Option<usize> {
    if fence(lines[i]) != Some(Fence::Open) {
        return None;
    }
    let mut j = i + 1;
    while j < lines.len() {
        match fence(lines[j]) {
            Some(Fence::Close) => return Some(j + 1),
            Some(Fence::Open) => return None,
            Some(Fence::Center) if let Some(e) = center_end(lines, j) => {
                j = e;
                continue;
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// Line ranges of the panels of a side-by-side segment (between its fence lines).
pub fn side_panels(lines: &[&str], seg: &Seg) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut from = seg.start + 1;
    let mut j = seg.start + 1;
    while j < seg.end {
        match fence(lines[j]) {
            Some(Fence::Next) | Some(Fence::Close) => {
                out.push(from..j);
                from = j + 1;
            }
            Some(Fence::Center) if let Some(e) = center_end(lines, j).filter(|&e| e < seg.end) => {
                j = e;
                continue;
            }
            _ => {}
        }
        j += 1;
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
    /// A centred group, fence lines included (its blocks are lines `start + 1..end - 1`).
    Center,
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
        // Centred groups, side by side, heading and divider lines.
        if let Some(end) = center_end(lines, i) {
            segs.push(Seg { kind: SegKind::Center, start: i, end });
            i = end;
            continue;
        }
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
            if i > s && (bullet(lines[i]).is_some() || quote(lines[i]).is_some() || starts_table(lines, i) || heading(lines[i]).is_some() || divider(lines[i]) || side_end(lines, i).is_some() || center_end(lines, i).is_some()) {
                break;
            }
            i += 1;
        }
        segs.push(Seg { kind: SegKind::Para, start: s, end: i });
    }
    segs
}

/// The blocks of a stem, as every renderer reads them. Round-7 layout (centred groups, list
/// nesting) is dropped here: a centred group's blocks stand in its place, nested items are
/// plain items of their list. [`parse_tree`] keeps that layout.
pub fn parse(stem: &str) -> Vec<Block> {
    lower(&parse_tree(stem))
}

/// A block of a stem with its round-7 layout (see [`parse_tree`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// Any block but `Block::Side`. `center`: it sits in a `::: center` group. `depths`: for a
    /// bullet or numbered list, the nesting level of each item (0 = outermost, see
    /// [`item_depths`]); empty for other blocks.
    Block { block: Block, center: bool, depths: Vec<u8> },
    /// A side-by-side group: each panel's nodes (an empty panel holds one empty paragraph).
    Side(Vec<Vec<Node>>),
}

/// [`parse`] with layout: the same blocks in the same order, side-by-side groups as
/// [`Node::Side`], centring and list nesting kept. `lower(&parse_tree(s)) == parse(s)`.
pub fn parse_tree(stem: &str) -> Vec<Node> {
    let lines: Vec<&str> = stem.split('\n').collect();
    let mut out = tree_of(&lines, false);
    if out.is_empty() {
        out.push(Node::Block { block: Block::Para(vec![Vec::new()]), center: false, depths: Vec::new() });
    }
    out
}

fn tree_of(lines: &[&str], center: bool) -> Vec<Node> {
    let mut out = Vec::new();
    for sg in segment(lines) {
        let ls = &lines[sg.start..sg.end];
        let leaf = |block: Block, depths: Vec<u8>| Node::Block { block, center, depths };
        out.push(match sg.kind {
            SegKind::Para => leaf(Block::Para(ls.iter().map(|l| inline(l.trim_end())).collect()), Vec::new()),
            SegKind::Bullets => leaf(Block::Bullets(ls.iter().map(|l| inline(bullet(l).unwrap_or(l))).collect()), item_depths(ls)),
            SegKind::Numbered => leaf(Block::Numbered(ls.iter().map(|l| inline(numbered(l).unwrap_or(l))).collect()), item_depths(ls)),
            SegKind::Quote => leaf(Block::Quote(ls.iter().map(|l| inline(quote(l).unwrap_or(l))).collect()), Vec::new()),
            SegKind::Table { header } => leaf(
                Block::Table {
                    header,
                    rows: ls
                        .iter()
                        .filter_map(|l| cells(l))
                        .filter(|c| !is_separator(c))
                        .map(|r| r.iter().map(|c| inline(c)).collect())
                        .collect(),
                },
                Vec::new(),
            ),
            SegKind::Heading(n) => leaf(Block::Heading(n, inline(heading(ls[0]).map_or(ls[0], |h| h.1).trim_end())), Vec::new()),
            SegKind::Divider => leaf(Block::Divider, Vec::new()),
            SegKind::Side => Node::Side(
                side_panels(lines, &sg)
                    .into_iter()
                    .map(|r| {
                        let mut p = tree_of(&lines[r], center);
                        if p.is_empty() {
                            p.push(Node::Block { block: Block::Para(vec![Vec::new()]), center, depths: Vec::new() });
                        }
                        p
                    })
                    .collect(),
            ),
            SegKind::Center => {
                out.extend(tree_of(&lines[sg.start + 1..sg.end - 1], true));
                continue;
            }
        });
    }
    out
}

/// The plain blocks of `nodes` (layout dropped; an empty stem is one empty paragraph).
pub fn lower(nodes: &[Node]) -> Vec<Block> {
    let mut out: Vec<Block> = nodes
        .iter()
        .map(|n| match n {
            Node::Block { block, .. } => block.clone(),
            Node::Side(panels) => Block::Side(panels.iter().map(|p| lower(p)).collect()),
        })
        .collect();
    if out.is_empty() {
        out.push(Block::Para(vec![Vec::new()]));
    }
    out
}

/// A task item: a bullet item whose text starts with its box, `☐ ` (open) or `☑ ` / `☒ `
/// (ticked). Whether it is ticked, or None for an ordinary item. The box is part of the
/// text; a renderer that knows task items leaves out the bullet in front of it.
pub fn task_item(line: &Line) -> Option<bool> {
    let first = line.first().filter(|sp| !sp.math && !sp.code)?;
    let mut c = first.text.chars();
    let done = match c.next()? {
        '\u{2610}' => false,
        '\u{2611}' | '\u{2612}' => true,
        _ => return None,
    };
    (matches!(c.next(), None | Some(' ')) && (first.text.chars().count() > 1 || line.len() == 1)).then_some(done)
}

/// Deepest list nesting level (0-based) the markup keeps.
pub const MAX_DEPTH: u8 = 4;

/// Nesting levels of a list's item lines: two leading spaces (a tab counts as two) per level,
/// measured from the first item, which is level 0; an item is at most one level deeper than
/// the item above it, and at most [`MAX_DEPTH`].
pub fn item_depths(lines: &[&str]) -> Vec<u8> {
    let raw = |l: &str| -> usize { l.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 2 } else { 1 }).sum::<usize>() / 2 };
    let base = lines.first().map_or(0, |l| raw(l));
    let mut out: Vec<u8> = Vec::with_capacity(lines.len());
    for l in lines {
        let want = raw(l).saturating_sub(base).min(MAX_DEPTH as usize) as u8;
        out.push(match out.last() {
            None => 0,
            Some(&p) => want.min(p + 1),
        });
    }
    out
}

/// Per-level ordinals of numbered items with these depths (1-based; a level restarts under
/// each new parent item): `[0, 1, 1, 0, 1]` gives `[1, 1, 2, 2, 1]`.
pub fn item_ordinals(depths: &[u8]) -> Vec<usize> {
    let mut count = [0usize; MAX_DEPTH as usize + 1];
    depths
        .iter()
        .map(|&d| {
            let d = d.min(MAX_DEPTH) as usize;
            count[d] += 1;
            for c in &mut count[d + 1..] {
                *c = 0;
            }
            count[d]
        })
        .collect()
}

/// The label of numbered item `n` (1-based) at nesting `depth`: `1.`, then `a.`, then `i.`,
/// repeating (as nested ordered lists usually print).
pub fn number_label(depth: u8, n: usize) -> String {
    match depth % 3 {
        0 => format!("{n}."),
        1 => format!("{}.", alpha(n)),
        _ => format!("{}.", roman(n)),
    }
}

/// The bullet glyph at nesting `depth`: •, ◦, ▪, repeating.
pub fn bullet_glyph(depth: u8) -> char {
    ['\u{2022}', '\u{25E6}', '\u{25AA}'][depth as usize % 3]
}

fn alpha(mut n: usize) -> String {
    let mut s = Vec::new();
    while n > 0 {
        n -= 1;
        s.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    s.iter().rev().collect()
}

fn roman(n: usize) -> String {
    let mut n = n.min(3999);
    let mut s = String::new();
    for (v, r) in [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")] {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
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

/// A `` `code` `` span opening at char `i` of a line: the index of its closing backtick. The
/// code is non-empty, has no space next to either backtick, holds no backtick and no `___`
/// blank, and (inside a link, whose `]` is at `limit`) closes before the link text ends.
/// Anything else stays literal, so a lone backtick or `` a ` b ` `` reads as before.
pub fn code_at(ch: &[char], i: usize, limit: Option<usize>) -> Option<usize> {
    if ch.get(i) != Some(&'`') {
        return None;
    }
    let end = (i + 1..ch.len()).find(|&j| ch[j] == '`')?;
    if end == i + 1 || ch[i + 1].is_whitespace() || ch[end - 1].is_whitespace() || limit.is_some_and(|l| end >= l) {
        return None;
    }
    let body = &ch[i + 1..end];
    (!body.windows(3).any(|w| w == ['_', '_', '_'])).then_some(end)
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
    /// The text of a `` `code` `` span (its backticks are markers).
    pub code: bool,
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
    let run = |start, end, kind, bold, italic, sub, sup, strike, link: Option<(usize, usize)>| Run { start, end, kind, bold, italic, sub, sup, strike, link: link.is_some(), code: false };
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
        // `code`: the text up to the closing backtick is literal (see [`code_at`])
        if c == '`'
            && let Some(end) = code_at(&ch, i, link.map(|l| l.0))
        {
            flush(&mut out, text_start, i, bold, italic, strike, link);
            out.push(run(i, i + 1, RunKind::Marker, bold, italic, false, false, strike, link));
            out.push(Run { code: true, ..run(i + 1, end, RunKind::Text, bold, italic, false, false, strike, link) });
            out.push(run(end, end + 1, RunKind::Marker, bold, italic, false, false, strike, link));
            i = end + 1;
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
                code: r.code,
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
    segment(&lines).iter().any(|s| matches!(s.kind, SegKind::Heading(_) | SegKind::Divider | SegKind::Side | SegKind::Center))
}

/// Does the stem use the round-7 extensions anywhere: a `` `code` `` span, a centred group or a
/// nested list item? Stems for which this is false read exactly as they did before round 7.
pub fn uses_r7(stem: &str) -> bool {
    fn nodes(ns: &[Node]) -> bool {
        ns.iter().any(|n| match n {
            Node::Block { center: true, .. } => true,
            Node::Block { depths, .. } if depths.iter().any(|&d| d > 0) => true,
            Node::Block { block, .. } => {
                let code = |ls: &[Line]| ls.iter().flatten().any(|sp| sp.code);
                match block {
                    Block::Para(ls) | Block::Bullets(ls) | Block::Numbered(ls) | Block::Quote(ls) => code(ls),
                    Block::Heading(_, l) => code(std::slice::from_ref(l)),
                    Block::Table { rows, .. } => rows.iter().any(|r| code(r)),
                    Block::Divider | Block::Side(_) => false,
                }
            }
            Node::Side(ps) => ps.iter().any(|p| nodes(p)),
        })
    }
    let lines: Vec<&str> = stem.split('\n').collect();
    // Code as the whole line reads it (the editor's runs) or as a cell / item reads it; a
    // `::: center` line with its closer is always a centred group, in a panel or not.
    stem.split('\n').any(|l| inline_runs(l).iter().any(|r| r.code))
        || (0..lines.len()).any(|i| center_end(&lines, i).is_some())
        || nodes(&parse_tree(stem))
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


/// The round-6 block parser (before centred groups) on the P3 run cutter (before inline code),
/// kept to prove round 7 changed nothing for stems that do not use it.
#[cfg(test)]
mod legacy_r6 {
    use super::{bullet, cells, divider, fence, heading, is_separator, numbered, quote, starts_table, Block, Fence, Line, RunKind, Seg, SegKind, Span};
    fn p3_inline(s: &str) -> Line {
        let ch: Vec<char> = s.chars().collect();
        let mut url: Option<String> = None;
        super::legacy_p3::inline_runs(s)
            .into_iter()
            .filter_map(|r| match r.kind {
                RunKind::Marker => {
                    if r.link && ch[r.start] == '[' {
                        url = super::link_at(&ch, r.start).map(|(close, end)| ch[close + 2..end].iter().collect());
                    } else if r.link && ch[r.start] == ']' {
                        url = None;
                    }
                    None
                }
                RunKind::Math => Some(Span { text: ch[r.start + 1..r.end - 1].iter().collect(), math: true, ..Default::default() }),
                RunKind::Text => Some(Span { text: ch[r.start..r.end].iter().collect(), bold: r.bold, italic: r.italic, sub: r.sub, sup: r.sup, strike: r.strike, math: false, link: if r.link { url.clone() } else { None }, code: false }),
            })
            .collect()
    }
    pub use self::r6_parse as parse;
    pub use self::r6_segment as segment;
    /// End (exclusive, after the closing fence) of a side-by-side group opening at line `i`, when
    /// the opener has a closer and no other opener comes first.
    fn r6_side_end(lines: &[&str], i: usize) -> Option<usize> {
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
    fn r6_side_panels(lines: &[&str], seg: &Seg) -> Vec<std::ops::Range<usize>> {
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

    /// The block structure of `lines` (see [`Seg`]).
    pub fn r6_segment(lines: &[&str]) -> Vec<Seg> {
        let mut segs: Vec<Seg> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() {
                i += 1;
                continue;
            }
            // Side by side, heading and divider lines.
            if let Some(end) = r6_side_end(lines, i) {
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
                if i > s && (bullet(lines[i]).is_some() || quote(lines[i]).is_some() || starts_table(lines, i) || heading(lines[i]).is_some() || divider(lines[i]) || r6_side_end(lines, i).is_some()) {
                    break;
                }
                i += 1;
            }
            segs.push(Seg { kind: SegKind::Para, start: s, end: i });
        }
        segs
    }

    pub fn r6_parse(stem: &str) -> Vec<Block> {
        let lines: Vec<&str> = stem.split('\n').collect();
        let mut blocks: Vec<Block> = r6_segment(&lines)
            .into_iter()
            .map(|sg| {
                let ls = &lines[sg.start..sg.end];
                match sg.kind {
                    SegKind::Para => Block::Para(ls.iter().map(|l| p3_inline(l.trim_end())).collect()),
                    SegKind::Bullets => Block::Bullets(ls.iter().map(|l| p3_inline(bullet(l).unwrap_or(l))).collect()),
                    SegKind::Numbered => Block::Numbered(ls.iter().map(|l| p3_inline(numbered(l).unwrap_or(l))).collect()),
                    SegKind::Quote => Block::Quote(ls.iter().map(|l| p3_inline(quote(l).unwrap_or(l))).collect()),
                    SegKind::Table { header } => Block::Table {
                        header,
                        rows: ls
                            .iter()
                            .filter_map(|l| cells(l))
                            .filter(|c| !is_separator(c))
                            .map(|r| r.iter().map(|c| p3_inline(c)).collect())
                            .collect(),
                    },
                    SegKind::Heading(n) => Block::Heading(n, p3_inline(heading(ls[0]).map_or(ls[0], |h| h.1).trim_end())),
                    SegKind::Divider => Block::Divider,
                    SegKind::Side => Block::Side(r6_side_panels(&lines, &sg).into_iter().map(|r| r6_parse(&lines[r].join("\n"))).collect()),
                    SegKind::Center => unreachable!("the round-6 segmenter makes no centred groups"),
                }
            })
            .collect();
        if blocks.is_empty() {
            blocks.push(Block::Para(vec![Vec::new()]));
        }
        blocks
    }


}

/// `inline_runs` as it was in P3 (links, before inline code in round 7), kept to prove code
/// spans changed nothing else.
#[cfg(test)]
mod legacy_p3 {
    use super::{link_at, Run, RunKind};
    pub fn inline_runs(s: &str) -> Vec<Run> {
        let ch: Vec<char> = s.chars().collect();
        let mut out: Vec<Run> = Vec::new();
        let (mut bold, mut italic, mut strike) = (false, false, false);
        let mut text_start = 0usize;
        // Inside a link: the indices of its `]` and closing `)`.
        let mut link: Option<(usize, usize)> = None;
        let run = |start, end, kind, bold, italic, sub, sup, strike, link: Option<(usize, usize)>| Run { start, end, kind, bold, italic, sub, sup, strike, link: link.is_some(), code: false };
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
        let run = |start, end, kind, bold, italic, sub, sup, strike| Run { start, end, kind, bold, italic, sub, sup, strike, link: false, code: false };
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
                    SegKind::Center => unreachable!("no centred groups before round 7"),
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

    /// Random markup with the round-7 syntax and near misses of it (lone or spaced backticks,
    /// `::: centre`, unclosed centre fences, indented list items).
    pub(crate) fn random_markup_r7(r: &mut Rng, max: usize) -> String {
        const TOK: &[&str] = &[
            "a", "word", " ", "*", "**", "~~", "_{", "}", "$x$", "$", "___", " | ", "| ", "\n", "\n", "\n\n", "• ", "- ", "1. ", "2. ", "> ", "---",
            "# ", "::: side", "::: next", ":::", "`", "``", "`x`", "`a b`", "` a`", "`a `", "`___`", "`**b**`", "[t](https://a.b)", "[`c`](https://a.b)",
            "::: center", "::: centre", ":: center", "  ", "  • ", "    • ", "  1. ", "    2. ", "\t• ", " - ", "é", "H",
        ];
        let n = (r.next() % max as u64) as usize;
        (0..n).map(|_| r.pick(TOK)).collect()
    }

    /// Round 7: a stem that uses no inline code, centred group or nested list item reads exactly
    /// as before (segments, blocks and every line's runs); the rest keeps the invariants.
    #[test]
    fn property_stems_without_r7_read_as_before() {
        let mut r = Rng(0x7a7a_5eed_0007);
        let (mut same, mut ext) = (0, 0);
        for k in 0..80_000 {
            let s = match k % 4 {
                0 => random_markup(&mut r, 40),
                1 => random_markup_ext(&mut r, 40),
                _ => random_markup_r7(&mut r, 40),
            };
            let lines: Vec<&str> = s.split('\n').collect();
            assert_eq!(lower(&parse_tree(&s)), parse(&s), "{s:?}");
            for line in &lines {
                let runs = inline_runs(line);
                let mut at = 0;
                for x in &runs {
                    assert_eq!(x.start, at, "{line:?}");
                    at = x.end;
                }
                assert_eq!(at, line.chars().count(), "{line:?}");
            }
            if uses_r7(&s) {
                ext += 1;
                continue;
            }
            same += 1;
            assert_eq!(segment(&lines), legacy_r6::segment(&lines), "stem {s:?}");
            assert_eq!(parse(&s), legacy_r6::parse(&s), "stem {s:?}");
            for line in &lines {
                assert_eq!(inline_runs(line), super::legacy_p3::inline_runs(line), "line {line:?}");
            }
        }
        assert!(same > 40_000 && ext > 10_000, "{same} {ext}");
    }

    /// Real stems (a JSON list of strings in QUESHION_STEMS_JSON): the ones that do not use
    /// round 7 read exactly as before. `cargo test -- --ignored fixture_stems`.
    #[test]
    #[ignore]
    fn fixture_stems_read_as_before() {
        let Ok(path) = std::env::var("QUESHION_STEMS_JSON") else { return };
        let stems: Vec<String> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut using = 0;
        for s in &stems {
            if uses_r7(s) {
                using += 1;
                eprintln!("uses round 7: {s:?}");
                continue;
            }
            let lines: Vec<&str> = s.split('\n').collect();
            assert_eq!(segment(&lines), legacy_r6::segment(&lines), "{s:?}");
            assert_eq!(parse(s), legacy_r6::parse(s), "{s:?}");
        }
        eprintln!("{} stems, {using} use round 7", stems.len());
    }

    #[test]
    fn inline_code_spans() {
        let l = inline("Call `print(x)` then **`y` z** and `a*b*c`.");
        assert!(l.iter().any(|s| s.code && s.text == "print(x)" && !s.bold));
        assert!(l.iter().any(|s| s.code && s.bold && s.text == "y"));
        assert!(l.iter().any(|s| s.code && s.text == "a*b*c" && !s.italic), "markup inside code is literal: {l:?}");
        assert_eq!(line_text(&l), "Call print(x) then y z and a*b*c.");
        assert!(has_markup("a `b` c"));
        // literal: lone, empty, spaced, holding a blank
        for lit in ["it`s", "``", "a ` b ` c", "` x`", "`x `", "fill `___` in", "`"] {
            let l = inline(lit);
            assert!(l.iter().all(|s| !s.code), "{lit:?}");
            assert_eq!(line_text(&l), lit);
            assert!(!uses_r7(lit), "{lit:?}");
        }
        // inside a link it must close before the link text ends
        let l = inline("[run `ls`](https://a.org)");
        assert!(l.iter().any(|s| s.code && s.text == "ls" && s.link.as_deref() == Some("https://a.org")));
        assert!(uses_r7("x `y`") && !uses_r7("x y"));
        assert_eq!(plain("Type `cd ..` now"), "Type cd .. now");
    }

    #[test]
    fn centred_groups() {
        let s = "Intro\n::: center\n# Title\nA line\n• one\n:::\nafter";
        let b = parse(s);
        assert_eq!(b.len(), 5, "{b:?}");
        assert!(matches!(&b[1], Block::Heading(1, _)) && matches!(&b[3], Block::Bullets(_)));
        let t = parse_tree(s);
        let centred: Vec<bool> = t.iter().map(|n| matches!(n, Node::Block { center: true, .. })).collect();
        assert_eq!(centred, vec![false, true, true, true, false]);
        assert!(uses_r7(s) && uses_extensions(s));
        // inside a side-by-side panel; the panel's own `:::` still closes the group
        let s2 = "::: side\n::: center\nleft\n:::\n::: next\nright\n:::";
        let t2 = parse_tree(s2);
        let Node::Side(ps) = &t2[0] else { panic!("{t2:?}") };
        assert_eq!(ps.len(), 2);
        assert!(matches!(&ps[0][0], Node::Block { center: true, .. }) && matches!(&ps[1][0], Node::Block { center: false, .. }));
        assert_eq!(plain(s2), "left right");
        // unclosed, nested, holding a side group or `::: next`: the fence lines stay text
        for lit in ["::: center\nx", "::: center\n::: center\nx\n:::\n:::", "::: center\n::: side\na\n:::\n:::", "::: center\na\n::: next\nb\n:::"] {
            assert!(parse(lit).iter().any(|b| matches!(b, Block::Para(ls) if ls.iter().any(|l| line_text(l).contains("::: center")))), "{lit:?}");
        }
        // an empty group is nothing
        assert_eq!(parse("::: center\n:::"), vec![Block::Para(vec![Vec::new()])]);
        assert!(uses_r7("::: center\n:::") && !uses_r7("::: centre\nx\n:::"));
    }

    #[test]
    fn nested_list_depths_and_labels() {
        let s = "• a\n  • b\n    • c\n  • d\n• e";
        let t = parse_tree(s);
        assert!(matches!(&t[0], Node::Block { block: Block::Bullets(items), depths, .. } if items.len() == 5 && depths == &vec![0, 1, 2, 1, 0]));
        assert_eq!(parse(s), legacy_r6::parse(s), "flat for renderers that know no nesting");
        assert!(uses_r7(s) && !uses_r7("• a\n• b") && !uses_r7("  • a\n  • b"));
        // no deeper than one level past the item above; relative to the first item
        assert_eq!(item_depths(&["  - a", "        - b", "- c"]), vec![0, 1, 0]);
        assert_eq!(item_depths(&["1. a", "\t2. b"]), vec![0, 1]);
        assert_eq!(item_ordinals(&[0, 1, 1, 0, 1, 2, 0]), vec![1, 1, 2, 2, 1, 1, 3]);
        assert_eq!((number_label(0, 3), number_label(1, 2), number_label(2, 4), number_label(1, 27)), ("3.".into(), "b.".into(), "iv.".into(), "aa.".into()));
        assert_eq!((bullet_glyph(0), bullet_glyph(1), bullet_glyph(2), bullet_glyph(3)), ('•', '◦', '▪', '•'));
        assert_eq!(task_item(&inline("☐ milk")), Some(false));
        assert_eq!(task_item(&inline("☑ **eggs**")), Some(true));
        assert_eq!(task_item(&inline("☐")), Some(false));
        assert_eq!(task_item(&inline("☐milk")), None);
        assert_eq!(task_item(&inline("milk ☐")), None);
        let n = parse_tree("1. a\n  1. b\n  2. c\n2. d");
        assert!(matches!(&n[0], Node::Block { block: Block::Numbered(_), depths, .. } if depths == &vec![0, 1, 1, 0]));
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
