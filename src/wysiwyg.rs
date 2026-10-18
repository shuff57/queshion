//! WYSIWYG editing of stem markup (`docs/stem-format.md`).
//!
//! The markup `String` stays the only state. Each frame it is cut into blocks by the parser's own
//! [`richtext::segment`]: *flow* runs (paragraph lines and the blank lines between them), list and
//! quote items, and table cells. Every editable unit is the exact char range of its raw text in the
//! stem; it is drawn as an egui `TextEdit` over a copy of that substring with a layouter that styles
//! the inline markup (markers collapsed to zero width, bold/italic/sub/sup applied, `$math$` and
//! `___` reserved as atoms and painted over), and an edit is spliced back at that range. Text that
//! is not edited is never re-serialised, so editor -> markup is lossless by construction.
//!
//! Structure edits (Enter in a list, toolbar list/table/format buttons, make-blank) are pure
//! functions on the string ([`enter_op`], [`toggle_list`], [`inline_op`], [`make_blank`], ...);
//! the focused unit follows a global caret (char offset in the stem) across re-segmentation.
//! Undo/redo is one stem-level stack. See wys/DESIGN.md for why this shape fits egui 0.36.

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange, CharIndex, LayoutJob, TextFormat};
use egui::{
    Align, Color32, CornerRadius, Event, FontFamily, FontId, Frame, Galley, Id as UiId, Key, Modifiers, Pos2,
    Rect, RichText, Shape, Stroke, StrokeKind, TextBuffer, TextEdit, Ui, Vec2,
};

use crate::richtext::{self, RunKind, SegKind};
use crate::theme;

// =====================================================================================
// String helpers (char indices throughout)
// =====================================================================================

fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map_or(s.len(), |(b, _)| b)
}

fn clen(s: &str) -> usize {
    s.chars().count()
}

/// Chars `r` of `s`.
pub fn slice(s: &str, r: Range<usize>) -> String {
    s.chars().skip(r.start).take(r.end.saturating_sub(r.start)).collect()
}

/// `s` with chars `r` replaced by `ins`.
pub fn splice(s: &str, r: Range<usize>, ins: &str) -> String {
    let (a, b) = (byte_at(s, r.start), byte_at(s, r.end.max(r.start)));
    format!("{}{}{}", &s[..a], ins, &s[b..])
}

/// Start chars of the non-overlapping `___` matches (the same blanks `str::matches` counts).
pub fn blank_starts(s: &str) -> Vec<usize> {
    let ch: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= ch.len() {
        if ch[i] == '_' && ch[i + 1] == '_' && ch[i + 2] == '_' {
            out.push(i);
            i += 3;
        } else {
            i += 1;
        }
    }
    out
}

// =====================================================================================
// Units: where each editable piece of the stem lives
// =====================================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullets,
    Numbered,
    Quote,
}

impl ListKind {
    fn seg(self) -> SegKind {
        match self {
            ListKind::Bullets => SegKind::Bullets,
            ListKind::Numbered => SegKind::Numbered,
            ListKind::Quote => SegKind::Quote,
        }
    }
}

/// A virtual empty paragraph before/after a stem that starts/ends with a list or table: typing in
/// it inserts `text\n` at the start / `\ntext` at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Virt {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UKind {
    Flow,
    Item { list: ListKind, line: usize },
    Cell { row: usize, col: usize, line: usize },
    /// The text of a `# ` heading line.
    Head { level: u8, line: usize },
}

/// One editable piece: chars `start..end` of the stem.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub start: usize,
    pub end: usize,
    pub kind: UKind,
    pub virt: Option<Virt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockKind {
    Flow,
    List(ListKind),
    Table { header: bool, ncols: usize, sep: Option<usize> },
    Heading(u8),
    /// A `---` line (no units).
    Divider,
    /// A side-by-side group (no units of its own): its panels' line ranges (between the fence
    /// lines). The panels' blocks follow it in [`Doc::blocks`], tagged with [`EBlock::panel`].
    Side { panels: Vec<Range<usize>> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct EBlock {
    pub kind: BlockKind,
    /// Source lines of the block.
    pub lines: Range<usize>,
    /// Items in order; table cells row-major.
    pub units: Vec<Unit>,
    /// (index of the side-by-side block, panel) for the blocks inside a panel.
    pub panel: Option<(usize, usize)>,
    /// The lines (fences included) of the `::: center` group this block is in.
    pub center: Option<Range<usize>>,
}

/// The stem cut into editable blocks.
#[derive(Clone, Debug)]
pub struct Doc {
    pub blocks: Vec<EBlock>,
    pub line_start: Vec<usize>,
    pub line_len: Vec<usize>,
    pub len: usize,
    /// Fence lines of the side-by-side and centred groups (structure, not text).
    pub fences: Vec<usize>,
}

impl Doc {
    /// Every unit in document order with its (block, index-in-block).
    pub fn flat(&self) -> Vec<(usize, usize)> {
        self.blocks.iter().enumerate().flat_map(|(b, bl)| (0..bl.units.len()).map(move |u| (b, u))).collect()
    }

    pub fn unit(&self, at: (usize, usize)) -> &Unit {
        &self.blocks[at.0].units[at.1]
    }

    /// The line holding char `g`.
    pub fn line_of(&self, g: usize) -> usize {
        match self.line_start.binary_search(&g) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }

    pub fn line_end(&self, l: usize) -> usize {
        self.line_start[l] + self.line_len[l]
    }

    /// Flat index of the unit holding global caret `g` (`virt` picks a virtual paragraph).
    pub fn find(&self, g: usize, virt: Option<Virt>) -> Option<usize> {
        let flat = self.flat();
        if let Some(v) = virt
            && let Some(i) = flat.iter().position(|&f| self.unit(f).virt == Some(v))
        {
            return Some(i);
        }
        if let Some(i) = flat.iter().position(|&f| {
            let u = self.unit(f);
            u.virt.is_none() && u.start <= g && g <= u.end
        }) {
            return Some(i);
        }
        // In a prefix or separator: the nearest real unit.
        flat.iter()
            .enumerate()
            .filter(|(_, f)| self.unit(**f).virt.is_none())
            .min_by_key(|(_, f)| {
                let u = self.unit(**f);
                if g < u.start { u.start - g } else { g.saturating_sub(u.end) }
            })
            .map(|(i, _)| i)
    }

    /// Global char of local caret `c` in unit `u` (a virtual paragraph's text lands after its `\n`).
    pub fn global(&self, u: &Unit, c: usize) -> usize {
        match u.virt {
            Some(Virt::Before) => c,
            Some(Virt::After) => self.len + 1 + c,
            None => u.start + c,
        }
    }
}

/// Cut `stem` into blocks. `structured == false` gives one flow unit over everything (answer
/// fields: inline markup only).
pub fn doc(stem: &str, structured: bool) -> Doc {
    let lines: Vec<&str> = stem.split('\n').collect();
    let mut line_start = Vec::with_capacity(lines.len());
    let mut line_len = Vec::with_capacity(lines.len());
    let mut at = 0;
    for l in &lines {
        line_start.push(at);
        let n = clen(l);
        line_len.push(n);
        at += n + 1;
    }
    let len = clen(stem);
    if !structured {
        let blocks = vec![flow_block(&line_start, &line_len, 0, lines.len(), None, None)];
        return Doc { blocks, line_start, line_len, len, fences: Vec::new() };
    }
    let mut blocks = Vec::new();
    let mut fences = Vec::new();
    build_blocks(&lines, 0, lines.len(), &line_start, &line_len, None, None, &mut blocks, &mut fences);
    let virt = |v: Virt, at: usize| EBlock {
        kind: BlockKind::Flow,
        lines: 0..0,
        units: vec![Unit { start: at, end: at, kind: UKind::Flow, virt: Some(v) }],
        panel: None,
        center: None,
    };
    if blocks.first().is_some_and(|b| b.kind != BlockKind::Flow || b.center.is_some()) {
        blocks.insert(0, virt(Virt::Before, 0));
        for b in &mut blocks {
            if let Some((si, _)) = &mut b.panel {
                *si += 1;
            }
        }
    }
    if blocks.iter().rev().find(|b| b.panel.is_none()).is_some_and(|b| b.kind != BlockKind::Flow || b.center.is_some()) {
        blocks.push(virt(Virt::After, len));
    }
    Doc { blocks, line_start, line_len, len, fences }
}

fn flow_block(line_start: &[usize], line_len: &[usize], a: usize, b: usize, panel: Option<(usize, usize)>, center: Option<Range<usize>>) -> EBlock {
    EBlock {
        kind: BlockKind::Flow,
        lines: a..b,
        units: vec![Unit { start: line_start[a], end: line_start[b - 1] + line_len[b - 1], kind: UKind::Flow, virt: None }],
        panel,
        center,
    }
}

/// The blocks of lines `from..to` (a whole stem, or one side-by-side panel).
#[allow(clippy::too_many_arguments)]
fn build_blocks(lines: &[&str], from: usize, to: usize, line_start: &[usize], line_len: &[usize], panel: Option<(usize, usize)>, center: Option<Range<usize>>, blocks: &mut Vec<EBlock>, fences: &mut Vec<usize>) {
    let segs: Vec<richtext::Seg> = richtext::segment(&lines[from..to])
        .into_iter()
        .map(|mut s| {
            s.start += from;
            s.end += from;
            s
        })
        .collect();
    let mut flow_from: Option<usize> = None;
    let mut k = 0;
    let mut i = from;
    while i < to {
        if k < segs.len() && segs[k].start == i {
            let sg = segs[k].clone();
            k += 1;
            if sg.kind == SegKind::Para {
                flow_from.get_or_insert(i);
                i = sg.end;
                continue;
            }
            if let Some(f) = flow_from.take() {
                blocks.push(flow_block(line_start, line_len, f, i, panel, center.clone()));
            }
            let mut units = Vec::new();
            let kind = match sg.kind {
                SegKind::Center => {
                    // its blocks, centred (the fence lines are structure, never text)
                    fences.push(sg.start);
                    fences.push(sg.end - 1);
                    if sg.start + 1 < sg.end - 1 {
                        build_blocks(lines, sg.start + 1, sg.end - 1, line_start, line_len, panel, Some(sg.start..sg.end), blocks, fences);
                    }
                    i = sg.end;
                    continue;
                }
                SegKind::Side => {
                    let rel = richtext::Seg { kind: SegKind::Side, start: sg.start, end: sg.end };
                    let panels = richtext::side_panels(lines, &rel);
                    fences.push(sg.start);
                    for r in &panels {
                        fences.push(r.end);
                    }
                    let si = blocks.len();
                    blocks.push(EBlock { kind: BlockKind::Side { panels: panels.clone() }, lines: sg.start..sg.end, units: Vec::new(), panel, center: None });
                    for (pi, r) in panels.iter().enumerate() {
                        if r.start < r.end {
                            build_blocks(lines, r.start, r.end, line_start, line_len, Some((si, pi)), None, blocks, fences);
                        }
                    }
                    i = sg.end;
                    continue;
                }
                SegKind::Divider => BlockKind::Divider,
                SegKind::Heading(level) => {
                    let l = sg.start;
                    let p = richtext::heading_prefix_len(lines[l]);
                    units.push(Unit { start: line_start[l] + p, end: line_start[l] + line_len[l], kind: UKind::Head { level, line: l }, virt: None });
                    BlockKind::Heading(level)
                }
                SegKind::Table { header } => {
                    let mut row = 0;
                    let mut ncols = 0;
                    let mut sep = None;
                    for l in sg.start..sg.end {
                        if richtext::is_separator_line(lines[l]) {
                            sep.get_or_insert(l);
                            continue;
                        }
                        let rg = richtext::cell_ranges(lines[l]).unwrap_or_default();
                        ncols = rg.len();
                        for (col, r) in rg.into_iter().enumerate() {
                            units.push(Unit { start: line_start[l] + r.start, end: line_start[l] + r.end, kind: UKind::Cell { row, col, line: l }, virt: None });
                        }
                        row += 1;
                    }
                    BlockKind::Table { header, ncols, sep }
                }
                other => {
                    let list = match other {
                        SegKind::Bullets => ListKind::Bullets,
                        SegKind::Numbered => ListKind::Numbered,
                        _ => ListKind::Quote,
                    };
                    for l in sg.start..sg.end {
                        let p = richtext::item_prefix_len(other, lines[l]);
                        units.push(Unit { start: line_start[l] + p, end: line_start[l] + line_len[l], kind: UKind::Item { list, line: l }, virt: None });
                    }
                    BlockKind::List(list)
                }
            };
            blocks.push(EBlock { kind, lines: sg.start..sg.end, units, panel, center: center.clone() });
            i = sg.end;
        } else {
            // a blank line: part of the surrounding flow
            flow_from.get_or_insert(i);
            i += 1;
        }
    }
    if let Some(f) = flow_from.take() {
        blocks.push(flow_block(line_start, line_len, f, to, panel, center));
    }
}

/// `stem` after unit `u`'s text became `text`.
pub fn apply_unit(stem: &str, u: &Unit, text: &str) -> String {
    match u.virt {
        Some(_) if text.is_empty() => stem.to_string(),
        Some(Virt::Before) => format!("{text}\n{stem}"),
        Some(Virt::After) => format!("{stem}\n{text}"),
        None => splice(stem, u.start..u.end, text),
    }
}

/// `stem` after flow unit `u` became `text`, keeping a numbered list that starts right after the
/// unit a list (a paragraph line directly above `1. ..` lines would otherwise absorb them).
pub fn apply_flow(stem: &str, d: &Doc, u: &Unit, text: &str) -> String {
    let s = apply_unit(stem, u, text);
    if u.virt.is_some() {
        return s;
    }
    let next = d.line_of(u.end) + 1;
    let numbered_next = d.blocks.iter().any(|b| b.lines.start == next && b.kind == BlockKind::List(ListKind::Numbered));
    if !numbered_next {
        return s;
    }
    let nd = doc(&s, true);
    let shift = clen(text) as i64 - (u.end - u.start) as i64;
    let at = (u.end as i64 + shift) as usize; // end of the edited unit in the new text
    let still = nd.blocks.iter().any(|b| b.kind == BlockKind::List(ListKind::Numbered) && nd.line_start.get(b.lines.start) == Some(&(at + 1)));
    if still { s } else { splice(&s, at..at, "\n") }
}

/// Why the visual editor must not open this text (it then shows the markup with a notice).
pub fn check(stem: &str) -> Result<(), String> {
    if stem.chars().count() > 20_000 {
        return Err("it is very long".into());
    }
    if stem.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err("it holds control characters (such as Windows line endings)".into());
    }
    // The units must read back exactly what the parser reads.
    let d = doc(stem, true);
    let lines: Vec<&str> = stem.split('\n').collect();
    let mut rebuilt = rebuild(stem, &lines, 0, lines.len(), &d)?;
    if rebuilt.is_empty() {
        rebuilt.push(richtext::Block::Para(vec![Vec::new()]));
    }
    if rebuilt != richtext::parse(stem) {
        return Err("the editor could not read it back exactly".into());
    }
    Ok(())
}

/// The parser's blocks of lines `from..to`, read back from the editor's units.
fn rebuild(stem: &str, lines: &[&str], from: usize, to: usize, d: &Doc) -> Result<Vec<richtext::Block>, String> {
    let mut out: Vec<richtext::Block> = Vec::new();
    for mut sg in richtext::segment(&lines[from..to]) {
        sg.start += from;
        sg.end += from;
        if sg.kind == SegKind::Para {
            out.push(richtext::Block::Para(lines[sg.start..sg.end].iter().map(|l| richtext::inline(l.trim_end())).collect()));
            continue;
        }
        if sg.kind == SegKind::Center {
            // the group's blocks stand in its place (as `richtext::parse` reads it)
            out.extend(rebuild(stem, lines, sg.start + 1, sg.end - 1, d)?);
            continue;
        }
        let Some(b) = d.blocks.iter().find(|b| b.lines.start == sg.start && b.kind != BlockKind::Flow) else {
            return Err("the editor could not find one of its blocks".into());
        };
        let texts: Vec<String> = b.units.iter().map(|u| slice(stem, u.start..u.end)).collect();
        match &b.kind {
            BlockKind::Flow => {}
            BlockKind::List(k) => {
                let items = texts.iter().map(|t| richtext::inline(t)).collect();
                out.push(match k {
                    ListKind::Bullets => richtext::Block::Bullets(items),
                    ListKind::Numbered => richtext::Block::Numbered(items),
                    ListKind::Quote => richtext::Block::Quote(items),
                });
            }
            BlockKind::Table { header, ncols, .. } => {
                let rows = texts.chunks(*ncols.max(&1)).map(|r| r.iter().map(|t| richtext::inline(t)).collect()).collect();
                out.push(richtext::Block::Table { header: *header, rows });
            }
            BlockKind::Heading(level) => {
                let t = texts.first().ok_or("a heading without text")?;
                out.push(richtext::Block::Heading(*level, richtext::inline(t.trim_end())));
            }
            BlockKind::Divider => out.push(richtext::Block::Divider),
            BlockKind::Side { panels } => {
                let mut ps = Vec::new();
                for r in panels {
                    let mut p = rebuild(stem, lines, r.start, r.end, d)?;
                    if p.is_empty() {
                        p.push(richtext::Block::Para(vec![Vec::new()]));
                    }
                    ps.push(p);
                }
                out.push(richtext::Block::Side(ps));
            }
        }
    }
    Ok(out)
}

// =====================================================================================
// Inline analysis: what each char of a unit is on screen
// =====================================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Text { b: bool, i: bool, sub: bool, sup: bool, s: bool },
    /// Collapsed markup marker.
    Hidden,
    /// `$..$` that does not typeset: shown literally in the error colour.
    Bad,
    Newline,
    /// First two chars of atom k (they carry its width).
    Head(usize),
    /// The rest of atom k (collapsed).
    Tail(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AtomKind {
    /// Linear content of a `$..$`.
    Math(String),
    /// A `___`; its ordinal among the unit's blanks.
    Blank(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Atom {
    pub start: usize,
    pub end: usize,
    pub kind: AtomKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ana {
    pub role: Vec<Role>,
    pub atoms: Vec<Atom>,
    /// Chars of a `[text](url)` link's text (shown blue and underlined).
    pub link: Vec<bool>,
    /// Chars of `` `code` `` (shown in a monospace face on a tint).
    pub code: Vec<bool>,
}

/// Roles of `text`'s chars; `math_ok` says whether a `$..$` content typesets.
pub fn analyze(text: &str, math_ok: &dyn Fn(&str) -> bool) -> Ana {
    let ch: Vec<char> = text.chars().collect();
    let mut role = vec![Role::Text { b: false, i: false, sub: false, sup: false, s: false }; ch.len()];
    let mut link = vec![false; ch.len()];
    let mut code = vec![false; ch.len()];
    let mut atoms: Vec<Atom> = Vec::new();
    let mut base = 0;
    for line in text.split('\n') {
        for r in richtext::inline_runs(line) {
            let (s, e) = (base + r.start, base + r.end);
            match r.kind {
                RunKind::Marker => role[s..e].fill(Role::Hidden),
                RunKind::Text => {
                    role[s..e].fill(Role::Text { b: r.bold, i: r.italic, sub: r.sub, sup: r.sup, s: r.strike });
                    link[s..e].fill(r.link);
                    code[s..e].fill(r.code);
                }
                RunKind::Math => {
                    let content: String = ch[s + 1..e - 1].iter().collect();
                    if math_ok(&content) {
                        atoms.push(Atom { start: s, end: e, kind: AtomKind::Math(content) });
                    } else {
                        role[s..e].fill(Role::Bad);
                    }
                }
            }
        }
        base += clen(line);
        if base < ch.len() {
            role[base] = Role::Newline;
            base += 1;
        }
    }
    for (n, m) in blank_starts(text).into_iter().enumerate() {
        if role[m..m + 3].iter().all(|r| matches!(r, Role::Text { sub: false, sup: false, .. })) && !atoms.iter().any(|a| a.start < m + 3 && m < a.end) {
            atoms.push(Atom { start: m, end: m + 3, kind: AtomKind::Blank(n) });
        }
    }
    atoms.sort_by_key(|a| a.start);
    for (k, a) in atoms.iter().enumerate() {
        role[a.start] = Role::Head(k);
        role[a.start + 1] = Role::Head(k);
        for r in &mut role[a.start + 2..a.end] {
            *r = Role::Tail(k);
        }
    }
    Ana { role, atoms, link, code }
}

impl Ana {
    fn len(&self) -> usize {
        self.role.len()
    }

    /// Is caret `p` strictly inside an atom?
    pub fn inside_atom(&self, p: usize) -> Option<&Atom> {
        self.atoms.iter().find(|a| a.start < p && p < a.end)
    }

    /// Visible things before caret `p` (an atom counts once, collapsed markers not at all).
    pub fn spot(&self, p: usize) -> usize {
        let mut n = 0;
        for (i, r) in self.role.iter().enumerate().take(p) {
            match r {
                Role::Hidden | Role::Tail(_) => {}
                Role::Head(k) => {
                    if self.atoms[*k].start == i {
                        n += 1;
                    }
                }
                _ => n += 1,
            }
        }
        n
    }

    /// The caret position the editor keeps for a visual spot: never inside an atom; among the
    /// positions that look the same, the leftmost (typing continues the style on the left), or
    /// the rightmost at the very start.
    pub fn canonical(&self, p: usize, prev: Option<usize>, keyboard: bool) -> usize {
        let n = self.len();
        let mut p = p.min(n);
        let dir = prev.map_or(0i32, |o| (p as i64 - o as i64).signum() as i32);
        if let Some(a) = self.inside_atom(p) {
            p = if dir < 0 || (dir == 0 && p - a.start <= a.end - p) { a.start } else { a.end };
        }
        let valid = |q: usize| self.inside_atom(q).is_none();
        if keyboard
            && let Some(o) = prev
            && o != p
            && self.spot(p) == self.spot(o.min(n))
        {
            let sp = self.spot(o.min(n));
            let found = if dir > 0 { (p + 1..=n).find(|&q| valid(q) && self.spot(q) > sp) } else { (0..p).rev().find(|&q| valid(q) && self.spot(q) < sp) };
            if let Some(q) = found {
                p = q;
            }
        }
        let sp = self.spot(p);
        let same: Vec<usize> = (0..=n).filter(|&q| valid(q) && self.spot(q) == sp).collect();
        if same.is_empty() {
            return p;
        }
        if sp == 0 { *same.last().unwrap() } else { same[0] }
    }

    /// Style of the visible char before caret `p` (or after, at the start).
    pub fn style_at(&self, p: usize) -> Style {
        let pick = |r: &Role| match r {
            Role::Text { b, i, sub, sup, s } => Some(Style { b: *b, i: *i, sub: *sub, sup: *sup, s: *s }),
            _ => None,
        };
        self.role[..p.min(self.len())].iter().rev().find_map(pick).or_else(|| self.role[p.min(self.len())..].iter().find_map(pick)).unwrap_or_default()
    }
}

// =====================================================================================
// Styled-char model of one line (formatting toggles, blanks, splitting)
// =====================================================================================

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Ch { c: char, b: bool, i: bool, sub: bool, sup: bool, s: bool },
    /// A whole `$..$` kept raw.
    Atom(String),
}

impl Item {
    fn plain(c: char) -> Item {
        Item::Ch { c, b: false, i: false, sub: false, sup: false, s: false }
    }
    fn raw_len(&self) -> usize {
        match self {
            Item::Ch { .. } => 1,
            Item::Atom(r) => clen(r),
        }
    }
    fn is_ws(&self) -> bool {
        matches!(self, Item::Ch { c, .. } if c.is_whitespace())
    }
}

/// The items of one line and the raw char where each starts.
pub fn decode_line(line: &str) -> (Vec<Item>, Vec<usize>) {
    let ch: Vec<char> = line.chars().collect();
    let mut items = Vec::new();
    let mut starts = Vec::new();
    let mut in_code = false;
    for r in richtext::inline_runs(line) {
        // `code` is kept whole and raw too (its backticks are its only markers)
        let tick = !r.link && r.kind == RunKind::Marker && ch[r.start] == '`';
        if tick || (in_code && !r.link) {
            if tick && !in_code {
                starts.push(r.start);
                items.push(Item::Atom(String::new()));
            }
            if let Some(Item::Atom(raw)) = items.last_mut() {
                raw.extend(&ch[r.start..r.end]);
            }
            if tick {
                in_code = !in_code;
            }
            continue;
        }
        if r.link {
            // A `[text](url)` link is kept whole, raw (like `$math$`): formatting toggles and
            // blanks never cut into it.
            if r.kind == RunKind::Marker && ch[r.start] == '[' {
                starts.push(r.start);
                items.push(Item::Atom(String::new()));
            }
            if let Some(Item::Atom(raw)) = items.last_mut() {
                raw.extend(&ch[r.start..r.end]);
            }
            continue;
        }
        match r.kind {
            RunKind::Marker => {}
            RunKind::Math => {
                starts.push(r.start);
                items.push(Item::Atom(ch[r.start..r.end].iter().collect()));
            }
            RunKind::Text => {
                for (k, c) in ch[r.start..r.end].iter().enumerate() {
                    starts.push(r.start + k);
                    items.push(Item::Ch { c: *c, b: r.bold, i: r.italic, sub: r.sub, sup: r.sup, s: r.strike });
                }
            }
        }
    }
    (items, starts)
}

/// Markup for `items` and the raw char where each item starts. Markers hug the text (spaces stay
/// outside), sub/superscripts become `_{..}` / `^{..}`. Bold opens outermost, then italic, then
/// strikethrough; they close in the reverse order.
pub fn encode_line(items: &[Item]) -> (String, Vec<usize>) {
    let mut out: Vec<char> = Vec::new();
    let mut starts = vec![0; items.len()];
    let (mut ob, mut oi, mut os) = (false, false, false);
    // Style of the next non-space plain char after index k (atoms are neutral).
    let next_style = |k: usize| -> Option<(bool, bool, bool)> {
        items[k + 1..].iter().find_map(|it| match it {
            Item::Ch { c, b, i, s, .. } if !c.is_whitespace() => Some((*b, *i, *s)),
            Item::Ch { sub: true, b, i, s, .. } | Item::Ch { sup: true, b, i, s, .. } => Some((*b, *i, *s)),
            _ => None,
        })
    };
    let mut k = 0;
    while k < items.len() {
        match &items[k] {
            Item::Atom(raw) => {
                starts[k] = out.len();
                out.extend(raw.chars());
                k += 1;
            }
            Item::Ch { c, b, i, sub, sup, s } => {
                let script = *sub || *sup;
                if !c.is_whitespace() || script {
                    if os && !*s {
                        out.extend(['~', '~']);
                        os = false;
                    }
                    if oi && !*i {
                        out.push('*');
                        oi = false;
                    }
                    if ob && !*b {
                        out.extend(['*', '*']);
                        ob = false;
                    }
                    if *b && !ob {
                        out.extend(['*', '*']);
                        ob = true;
                    }
                    if *i && !oi {
                        out.push('*');
                        oi = true;
                    }
                    if *s && !os {
                        out.extend(['~', '~']);
                        os = true;
                    }
                }
                let last;
                if script {
                    out.extend([if *sub { '_' } else { '^' }, '{']);
                    let mut j = k;
                    while let Some(Item::Ch { c: cj, b: bj, i: ij, sub: sj, sup: pj, s: xj }) = items.get(j) {
                        if (*sj, *pj, *bj, *ij, *xj) != (*sub, *sup, *b, *i, *s) {
                            break;
                        }
                        starts[j] = out.len();
                        out.push(*cj);
                        j += 1;
                    }
                    out.push('}');
                    last = j - 1;
                    k = j;
                } else {
                    starts[k] = out.len();
                    out.push(*c);
                    last = k;
                    k += 1;
                }
                let closing = !c.is_whitespace() || script;
                if closing {
                    let nx = next_style(last);
                    if os && !nx.is_some_and(|(_, _, ns)| ns) {
                        out.extend(['~', '~']);
                        os = false;
                    }
                    if oi && !nx.is_some_and(|(_, ni, _)| ni) {
                        out.push('*');
                        oi = false;
                    }
                    if ob && !nx.is_some_and(|(nb, _, _)| nb) {
                        out.extend(['*', '*']);
                        ob = false;
                    }
                }
            }
        }
    }
    if os {
        out.extend(['~', '~']);
    }
    if oi {
        out.push('*');
    }
    if ob {
        out.extend(['*', '*']);
    }
    (out.into_iter().collect(), starts)
}

/// Same text and styles (a space's bold/italic/strikethrough does not matter).
fn same_items(a: &[Item], b: &[Item]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| match (x, y) {
            (Item::Atom(p), Item::Atom(q)) => p == q,
            (Item::Ch { c: c1, b: b1, i: i1, sub: s1, sup: p1, s: x1 }, Item::Ch { c: c2, b: b2, i: i2, sub: s2, sup: p2, s: x2 }) => {
                c1 == c2 && s1 == s2 && p1 == p2 && (c1.is_whitespace() || (b1 == b2 && i1 == i2 && x1 == x2))
            }
            _ => false,
        })
}

/// `encode_line`, but only when the parser reads the result back as `items`.
pub fn encode_checked(items: &[Item]) -> Option<(String, Vec<usize>)> {
    let (s, st) = encode_line(items);
    same_items(&decode_line(&s).0, items).then_some((s, st))
}

/// Item index of raw char `p` in a decoded line (items starting before `p`).
fn item_at(starts: &[usize], p: usize) -> usize {
    starts.iter().take_while(|&&s| s < p).count()
}

/// Raw position of item boundary `k` in an encoded line.
fn raw_of(items: &[Item], enc_starts: &[usize], enc: &str, k: usize, end: bool) -> usize {
    if end && k > 0 {
        enc_starts[k - 1] + items[k - 1].raw_len()
    } else if k < items.len() {
        enc_starts[k]
    } else {
        clen(enc)
    }
}

/// Inline formatting a toolbar button or shortcut applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IOp {
    Bold,
    Italic,
    Sub,
    Sup,
    Strike,
    Clear,
}

/// The inline style of a char (or the style typing will use).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub b: bool,
    pub i: bool,
    pub sub: bool,
    pub sup: bool,
    pub s: bool,
}

impl Style {
    /// The style after toggling `op` (sub and sup exclude each other; Clear drops everything).
    pub fn toggled(self, op: IOp) -> Style {
        let mut t = self;
        match op {
            IOp::Bold => t.b = !t.b,
            IOp::Italic => t.i = !t.i,
            IOp::Strike => t.s = !t.s,
            IOp::Sub => (t.sub, t.sup) = (!t.sub, false),
            IOp::Sup => (t.sup, t.sub) = (!t.sup, false),
            IOp::Clear => t = Style::default(),
        }
        t
    }
}

/// Apply `op` to the selection `a..b` (local chars) of a unit's text, which may span lines. With
/// no selection the word at the caret is used, or a selected placeholder is inserted. Returns the
/// new text and selection, or None when the result would not read back as intended.
pub fn inline_op(text: &str, a: usize, b: usize, op: IOp) -> Option<(String, usize, usize)> {
    let (a, b) = (a.min(b), a.max(b));
    // Inside a link's text (or cutting through one) the link would be lost: not applied.
    if link_spans(text).iter().any(|&(s, _, e)| (s < a && a <= e) || (s < b && b <= e)) {
        return None;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let mut starts = Vec::new();
    let mut at = 0;
    for l in &lines {
        starts.push(at);
        at += clen(l) + 1;
    }
    let li_of = |p: usize| starts.iter().rposition(|&s| s <= p).unwrap_or(0);
    if a == b {
        // Word under the caret, else a placeholder.
        let li = li_of(a);
        let (items, st) = decode_line(lines[li]);
        let k = item_at(&st, a - starts[li]);
        let wordy = |it: &Item| matches!(it, Item::Ch { c, .. } if c.is_alphanumeric() || *c == '\'' || *c == '-');
        let (mut s, mut e) = (k, k);
        while s > 0 && wordy(&items[s - 1]) {
            s -= 1;
        }
        while e < items.len() && wordy(&items[e]) {
            e += 1;
        }
        if s < e {
            let ra = starts[li] + st[s];
            let rb = starts[li] + st[e - 1] + 1;
            return inline_op(text, ra, rb, op);
        }
        if op == IOp::Clear {
            return None;
        }
        let ph = match op {
            IOp::Bold => "bold text",
            IOp::Italic => "italic text",
            IOp::Strike => "struck text",
            _ => "2",
        };
        let mut items = items;
        let ins: Vec<Item> = ph.chars().map(Item::plain).collect();
        let n = ins.len();
        items.splice(k..k, ins);
        let (s2, e2) = (k, k + n);
        return line_apply(&lines, &starts, li, items, s2, e2, op);
    }
    // Decide set vs clear over the whole selection.
    let mut all = true;
    let mut any = false;
    for li in li_of(a)..=li_of(b.saturating_sub(1).max(a)) {
        let (items, st) = decode_line(lines[li]);
        let k0 = item_at(&st, a.max(starts[li]) - starts[li]);
        let k1 = item_at(&st, b.min(starts[li] + clen(lines[li])) - starts[li]);
        for it in &items[k0..k1.max(k0)] {
            if let Item::Ch { c, b, i, sub, sup, s } = it
                && !c.is_whitespace()
            {
                any = true;
                let has = match op {
                    IOp::Bold => *b,
                    IOp::Italic => *i,
                    IOp::Strike => *s,
                    IOp::Sub => *sub,
                    IOp::Sup => *sup,
                    IOp::Clear => false,
                };
                all &= has;
            }
        }
    }
    if !any {
        return None;
    }
    let set = !all;
    let (la, lb) = (li_of(a), li_of(b.saturating_sub(1).max(a)));
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let mut sel = (0usize, 0usize);
    for li in la..=lb {
        let (items, st) = decode_line(lines[li]);
        let mut k0 = item_at(&st, a.max(starts[li]) - starts[li]);
        let mut k1 = item_at(&st, b.min(starts[li] + clen(lines[li])) - starts[li]);
        // keep the selection's edge spaces outside the markers
        while k0 < k1 && items[k0].is_ws() {
            k0 += 1;
        }
        while k1 > k0 && items[k1 - 1].is_ws() {
            k1 -= 1;
        }
        let mut items = items;
        for it in &mut items[k0..k1] {
            if let Item::Ch { b, i, sub, sup, s, .. } = it {
                match op {
                    IOp::Bold => *b = set,
                    IOp::Italic => *i = set,
                    IOp::Strike => *s = set,
                    IOp::Sub => {
                        *sub = set;
                        if set {
                            *sup = false;
                        }
                    }
                    IOp::Sup => {
                        *sup = set;
                        if set {
                            *sub = false;
                        }
                    }
                    IOp::Clear => (*b, *i, *sub, *sup, *s) = (false, false, false, false, false),
                }
            }
        }
        let (enc, es) = encode_checked(&items)?;
        if li == la {
            sel.0 = raw_of(&items, &es, &enc, k0, false);
        }
        if li == lb {
            sel.1 = raw_of(&items, &es, &enc, k1, true);
        }
        new_lines[li] = enc;
    }
    let mut off = 0;
    let mut out_sel = (0, 0);
    for (li, l) in new_lines.iter().enumerate() {
        if li == la {
            out_sel.0 = off + sel.0;
        }
        if li == lb {
            out_sel.1 = off + sel.1;
        }
        off += clen(l) + 1;
    }
    Some((new_lines.join("\n"), out_sel.0, out_sel.1.max(out_sel.0)))
}

/// Apply `op` (set) to items `s..e` of line `li` and rebuild the text; the selection covers them.
fn line_apply(lines: &[&str], starts: &[usize], li: usize, mut items: Vec<Item>, s: usize, e: usize, op: IOp) -> Option<(String, usize, usize)> {
    for it in &mut items[s..e] {
        if let Item::Ch { b, i, sub, sup, s, .. } = it {
            match op {
                IOp::Bold => *b = true,
                IOp::Italic => *i = true,
                IOp::Strike => *s = true,
                IOp::Sub => (*sub, *sup) = (true, false),
                IOp::Sup => (*sup, *sub) = (true, false),
                IOp::Clear => {}
            }
        }
    }
    let (enc, es) = encode_checked(&items)?;
    let (ra, rb) = (raw_of(&items, &es, &enc, s, false), raw_of(&items, &es, &enc, e, true));
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    out[li] = enc;
    let base = starts[li];
    Some((out.join("\n"), base + ra, base + rb))
}

/// Typing mode (a format toggled with a collapsed caret): `new` is `old` with text typed at
/// local caret `at`; give the typed text the style `want`. Returns the new text and the caret
/// after the typed text, or None when nothing was inserted there (or it cannot be styled).
pub fn style_typed(old: &str, new: &str, at: usize, want: Style) -> Option<(String, usize)> {
    let o: Vec<char> = old.chars().collect();
    let n: Vec<char> = new.chars().collect();
    if n.len() <= o.len() || at > o.len() {
        return None;
    }
    let k = n.len() - o.len();
    if n[..at] != o[..at] || n[at + k..] != o[at..] || n[at..at + k].contains(&'\n') {
        return None;
    }
    let (mut text, mut a, mut b) = (new.to_string(), at, at + k);
    // styles of the typed visible chars
    let styles = |t: &str, a: usize, b: usize| -> Vec<Style> {
        let ana = analyze(t, &|_| false);
        let ch: Vec<char> = t.chars().collect();
        (a..b.min(ch.len()))
            .filter_map(|q| match ana.role[q] {
                Role::Text { b, i, sub, sup, s } if !ch[q].is_whitespace() => Some(Style { b, i, sub, sup, s }),
                _ => None,
            })
            .collect()
    };
    if styles(&text, a, b).is_empty() {
        return None;
    }
    let get = |st: &Style, op: IOp| match op {
        IOp::Bold => st.b,
        IOp::Italic => st.i,
        IOp::Strike => st.s,
        IOp::Sub => st.sub,
        IOp::Sup => st.sup,
        IOp::Clear => false,
    };
    for op in [IOp::Sub, IOp::Sup, IOp::Bold, IOp::Italic, IOp::Strike] {
        // a toggle sets a mixed run first, so a wanted "off" can take two passes
        for _ in 0..2 {
            let cur = styles(&text, a, b);
            if cur.iter().all(|st| get(st, op) == get(&want, op)) {
                break;
            }
            let (t, x, y) = inline_op(&text, a, b, op)?;
            (text, a, b) = (t, x, y);
        }
    }
    let ok = styles(&text, a, b).iter().all(|st| *st == want);
    ok.then_some((text, b))
}

/// Replace the selection `a..b` (one line) with a `___` blank. Returns the new text, the caret
/// after the blank, the blank's start and the selected word (the blank's answer).
pub fn make_blank(text: &str, a: usize, b: usize) -> Option<(String, usize, usize, String)> {
    let (a, b) = (a.min(b), a.max(b));
    let lines: Vec<&str> = text.split('\n').collect();
    let mut off = 0;
    for (li, l) in lines.iter().enumerate() {
        let n = clen(l);
        if a >= off && b <= off + n {
            let (items, st) = decode_line(l);
            let mut k0 = item_at(&st, a - off);
            let mut k1 = item_at(&st, b - off);
            while k0 < k1 && items[k0].is_ws() {
                k0 += 1;
            }
            while k1 > k0 && items[k1 - 1].is_ws() {
                k1 -= 1;
            }
            if k0 == k1 {
                return None;
            }
            let word: String = items[k0..k1]
                .iter()
                .map(|it| match it {
                    Item::Ch { c, .. } => c.to_string(),
                    Item::Atom(r) => r.clone(),
                })
                .collect();
            let mut items = items;
            items.splice(k0..k1, "___".chars().map(Item::plain));
            let (enc, es) = encode_checked(&items)?;
            let bs = raw_of(&items, &es, &enc, k0, false);
            let caret = raw_of(&items, &es, &enc, k0 + 3, true);
            let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            out[li] = enc;
            return Some((out.join("\n"), off + caret, off + bs, word.trim().to_string()));
        }
        off += n + 1;
    }
    None
}

/// The `[text](url)` links of a unit's text (any line): local chars of each `[`, `]` and `)`.
pub fn link_spans(text: &str) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut base = 0;
    for line in text.split('\n') {
        let ch: Vec<char> = line.chars().collect();
        for r in richtext::inline_runs(line) {
            if r.link && r.kind == RunKind::Marker && ch[r.start] == '['
                && let Some((close, end)) = richtext::link_at(&ch, r.start)
            {
                out.push((base + r.start, base + close, base + end));
            }
        }
        base += ch.len() + 1;
    }
    out
}

/// Link the selection `a..b` (local chars of a unit's text) to `url`, or change or remove the
/// link the selection or caret is in (an empty `url` removes it, keeping its text). With a bare
/// caret outside a link the address is inserted as its own text. The address goes through
/// [`richtext::normalize_url`] (`example.org` becomes `https://example.org`). Returns the new
/// text and the link text's range, or None when it does not apply (a selection across lines or
/// through math, or an address that cannot be a link).
pub fn link_op(text: &str, a: usize, b: usize, url: &str) -> Option<(String, usize, usize)> {
    let (a, b) = (a.min(b), a.max(b));
    let ch: Vec<char> = text.chars().collect();
    if let Some(&(s, close, end)) = link_spans(text).iter().find(|&&(s, _, e)| s < b.max(a + 1) && a <= e) {
        let label: String = ch[s + 1..close].iter().collect();
        if url.trim().is_empty() {
            return Some((splice(text, s..end + 1, &label), s, s + clen(&label)));
        }
        let u = richtext::normalize_url(url)?;
        return Some((splice(text, close + 2..end, &u), s + 1, close));
    }
    let u = richtext::normalize_url(url)?;
    let (new, x, y) = if a == b {
        let label = u.strip_prefix("mailto:").unwrap_or(&u).to_string();
        let n = clen(&label);
        (splice(text, a..a, &format!("[{label}]({u})")), a + 1, a + 1 + n)
    } else {
        let (mut x, mut y) = (a, b.min(ch.len()));
        while x < y && ch[x].is_whitespace() {
            x += 1;
        }
        while y > x && ch[y - 1].is_whitespace() {
            y -= 1;
        }
        if x == y || ch[x..y].contains(&'\n') {
            return None;
        }
        let t = splice(text, y..y, &format!("]({u})"));
        (splice(&t, x..x, "["), x + 1, y + 1)
    };
    // It must read back as a link to `u` over exactly that text.
    link_spans(&new).iter().any(|&(s, close, end)| s + 1 == x && close == y && slice(&new, close + 2..end) == u).then_some((new, x, y))
}

/// Split one line at the selection `a..b` (removing it): the two halves, each with its own
/// complete markup (a bold run cut in two stays bold on both sides).
pub fn split_line(line: &str, a: usize, b: usize) -> (String, String) {
    let (a, b) = (a.min(b), a.max(b));
    let (items, st) = decode_line(line);
    let (ka, kb) = (item_at(&st, a), item_at(&st, b));
    let left = &items[..ka];
    let right = &items[kb.max(ka)..];
    match (encode_checked(left), encode_checked(right)) {
        (Some((l, _)), Some((r, _))) => (l, r),
        _ => (slice(line, 0..a), slice(line, b..clen(line))),
    }
}

/// Is a unit's text visibly empty (nothing but collapsed markers or spaces)?
pub fn visibly_empty(text: &str) -> bool {
    text.split('\n').all(|l| decode_line(l).0.iter().all(|it| it.is_ws()))
}

// =====================================================================================
// Blanks: keep Kind::Blank answers matched to the `___` markers
// =====================================================================================

/// Answer rows for `new` given the rows that matched `old`'s blanks. The two strings are aligned
/// char by char (common prefix/suffix, then a longest-common-subsequence of the rest), and a blank
/// keeps its row when its three `_` survive the edit in place; blanks inside deleted text lose
/// their rows and new blanks get empty ones. Rewrites that move every character (a table written
/// back with pipes) keep their answers because the cells' text still aligns.
pub fn remap_blanks(old: &str, new: &str, rows: &[String]) -> Vec<String> {
    let o: Vec<char> = old.chars().collect();
    let n: Vec<char> = new.chars().collect();
    let ob = blank_starts(old);
    let nb = blank_starts(new);
    let row = |k: usize| rows.get(k).cloned().unwrap_or_default();
    let map = align(&o, &n);
    let mut out = vec![String::new(); nb.len()];
    for (k, &m) in nb.iter().enumerate() {
        // where did these three chars come from?
        let src: Vec<Option<usize>> = (m..m + 3).map(|j| map[j]).collect();
        if let [Some(a), Some(b), Some(c)] = src[..]
            && b == a + 1
            && c == a + 2
            && let Some(ok) = ob.iter().position(|&s| s == a)
        {
            out[k] = row(ok);
        }
    }
    out
}

/// For each char of `n`, the char of `o` it is kept from (None: inserted).
fn align(o: &[char], n: &[char]) -> Vec<Option<usize>> {
    let mut map = vec![None; n.len()];
    let mut p = 0;
    while p < o.len() && p < n.len() && o[p] == n[p] {
        map[p] = Some(p);
        p += 1;
    }
    let mut s = 0;
    while s < o.len() - p && s < n.len() - p && o[o.len() - 1 - s] == n[n.len() - 1 - s] {
        map[n.len() - 1 - s] = Some(o.len() - 1 - s);
        s += 1;
    }
    let (oa, ob) = (p, o.len() - s);
    let (na, nb) = (p, n.len() - s);
    let (lo, ln) = (ob - oa, nb - na);
    if lo == 0 || ln == 0 || lo * ln > 4_000_000 {
        return map;
    }
    // LCS table over the middle parts
    let w = ln + 1;
    let mut t = vec![0u16; (lo + 1) * w];
    for i in (0..lo).rev() {
        for j in (0..ln).rev() {
            t[i * w + j] = if o[oa + i] == n[na + j] { t[(i + 1) * w + j + 1].saturating_add(1) } else { t[(i + 1) * w + j].max(t[i * w + j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < lo && j < ln {
        if o[oa + i] == n[na + j] {
            map[na + j] = Some(oa + i);
            i += 1;
            j += 1;
        } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    map
}

// =====================================================================================
// Structure edits (pure; each returns the new stem and the caret/selection to focus)
// =====================================================================================

/// A stem edit: the new text and the global selection to restore.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub stem: String,
    pub anchor: usize,
    pub caret: usize,
    /// Prefer the virtual paragraph (the caret is in text that does not exist yet).
    pub virt: Option<Virt>,
    /// A blank was made at this char with this answer.
    pub blank: Option<(usize, String)>,
    /// For each blank of the new text, the old blank whose answer it keeps (when the edit knows
    /// exactly, as table row/column edits do); else the texts are aligned ([`remap_blanks`]).
    pub keep: Option<Vec<Option<usize>>>,
}

impl Change {
    fn at(stem: String, caret: usize) -> Change {
        Change { stem, anchor: caret, caret, virt: None, blank: None, keep: None }
    }
}

fn lines_of(stem: &str) -> Vec<String> {
    stem.split('\n').map(|s| s.to_string()).collect()
}

fn line_starts(lines: &[String]) -> Vec<usize> {
    let mut v = Vec::new();
    let mut at = 0;
    for l in lines {
        v.push(at);
        at += clen(l) + 1;
    }
    v
}

fn numbered_prefix(l: &str) -> Option<(String, u32, char, usize)> {
    // (leading spaces, number, delimiter, prefix chars)
    let ws: String = l.chars().take_while(|c| c.is_whitespace()).collect();
    let rest = &l[ws.len()..];
    let d: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if d.is_empty() || d.len() > 2 {
        return None;
    }
    let after = &rest[d.len()..];
    let delim = after.chars().next()?;
    if !(delim == '.' || delim == ')') || !after[1..].starts_with(' ') {
        return None;
    }
    Some((ws.clone(), d.parse().ok()?, delim, clen(&ws) + d.len() + 2))
}

/// Rewrite the numbers of the numbered lines `range` as 1, 2, 3... (each nesting level
/// counts on its own: `richtext::item_ordinals`).
fn renumber(lines: &mut [String], range: Range<usize>) {
    let idx: Vec<usize> = range.filter(|&l| numbered_prefix(&lines[l]).is_some()).collect();
    let refs: Vec<&str> = idx.iter().map(|&l| lines[l].as_str()).collect();
    let ords = richtext::item_ordinals(&richtext::item_depths(&refs));
    for (k, &l) in idx.iter().enumerate() {
        if let Some((ws, _, d, p)) = numbered_prefix(&lines[l]) {
            let rest: String = lines[l].chars().skip(p).collect();
            lines[l] = format!("{ws}{}{d} {rest}", ords[k]);
        }
    }
}

fn prefix_for(kind: ListKind, n: usize) -> String {
    match kind {
        ListKind::Bullets => "• ".to_string(),
        ListKind::Numbered => format!("{n}. "),
        ListKind::Quote => "> ".to_string(),
    }
}

/// Toolbar list/quote button with the caret/selection `sel` (global) in flat unit `fi`. A numbered
/// list right under a paragraph line gets a blank line above it (the markup reads `1.` lines
/// that continue a paragraph as text).
pub fn toggle_list(stem: &str, d: &Doc, fi: usize, sel: (usize, usize), kind: ListKind) -> Option<Change> {
    let mut c = toggle_list_raw(stem, d, fi, sel, kind)?;
    if kind != ListKind::Numbered {
        return Some(c);
    }
    let mut lines = lines_of(&c.stem);
    let ls = line_starts(&lines);
    let cl = ls.iter().rposition(|&x| x <= c.caret).unwrap_or(0);
    if numbered_prefix(&lines[cl]).is_none() {
        return Some(c);
    }
    let mut st = cl;
    while st > 0 && numbered_prefix(&lines[st - 1]).is_some() {
        st -= 1;
    }
    let mut en = cl + 1;
    while en < lines.len() && numbered_prefix(&lines[en]).is_some() {
        en += 1;
    }
    let nd = doc(&c.stem, true);
    let is_list = nd.blocks.iter().any(|b| b.lines.start == st && b.kind == BlockKind::List(ListKind::Numbered));
    if en - st >= 2 && st > 0 && !lines[st - 1].trim().is_empty() && !is_list {
        let at = ls[st];
        lines.insert(st, String::new());
        c.stem = lines.join("\n");
        if c.anchor >= at {
            c.anchor += 1;
        }
        if c.caret >= at {
            c.caret += 1;
        }
    }
    Some(c)
}

fn toggle_list_raw(stem: &str, d: &Doc, fi: usize, sel: (usize, usize), kind: ListKind) -> Option<Change> {
    let at = d.flat()[fi];
    let u = d.unit(at).clone();
    let mut lines = lines_of(stem);
    let (a, c) = sel;
    match u.kind {
        UKind::Item { list, line } => {
            let bl = &d.blocks[at.0];
            let p = richtext::item_prefix_len(list.seg(), &lines[line]);
            if list == kind {
                // lift this item out of the list
                lines[line] = lines[line].chars().skip(p).collect();
                if list == ListKind::Numbered {
                    renumber(&mut lines, bl.lines.clone());
                }
                let s = lines.join("\n");
                let shift = |g: usize| g.saturating_sub(p).max(d.line_start[line]);
                return Some(Change { stem: s, anchor: shift(a), caret: shift(c), virt: None, blank: None, keep: None });
            }
            // convert the whole list
            let mut caret_shift = 0i64;
            let mut anchor_shift = 0i64;
            let mut acc = 0i64;
            for (k, l) in bl.lines.clone().enumerate() {
                let op = richtext::item_prefix_len(list.seg(), &lines[l]);
                let np = prefix_for(kind, k + 1);
                let delta = clen(&np) as i64 - op as i64;
                lines[l] = format!("{np}{}", lines[l].chars().skip(op).collect::<String>());
                acc += delta;
                if l == line {
                    caret_shift = acc;
                    anchor_shift = acc;
                }
            }
            let s = lines.join("\n");
            let mv = |g: usize, sh: i64| (g as i64 + sh).max(0) as usize;
            Some(Change { stem: s, anchor: mv(a, anchor_shift), caret: mv(c, caret_shift), virt: None, blank: None, keep: None })
        }
        UKind::Flow if u.virt.is_none() => {
            let (lo, hi) = (a.min(c), a.max(c));
            let (l0, l1) = (d.line_of(lo), d.line_of(hi));
            let mut targets: Vec<usize> = (l0..=l1).filter(|&l| !lines[l].trim().is_empty()).collect();
            if targets.is_empty() {
                targets.push(d.line_of(c));
            }
            let mut acc = 0i64;
            let (mut na, mut nc) = (a as i64, c as i64);
            for (k, &l) in targets.iter().enumerate() {
                let np = prefix_for(kind, k + 1);
                let ls = d.line_start[l] as i64 + acc;
                let w = clen(&np) as i64;
                lines[l] = format!("{np}{}", lines[l].trim_start());
                let trimmed = (d.line_len[l] - clen(stem.split('\n').nth(l).unwrap_or("").trim_start())) as i64;
                let delta = w - trimmed;
                if na >= ls {
                    na += delta;
                }
                if nc >= ls {
                    nc += delta;
                }
                acc += delta;
            }
            Some(Change { stem: lines.join("\n"), anchor: na.max(0) as usize, caret: nc.max(0) as usize, virt: None, blank: None, keep: None })
        }
        UKind::Flow => {
            // virtual paragraph: start a list there
            let np = prefix_for(kind, 1);
            let s = match u.virt {
                Some(Virt::Before) => format!("{np}\n{stem}"),
                _ => format!("{stem}\n{np}"),
            };
            let caret = match u.virt {
                Some(Virt::Before) => clen(&np),
                _ => clen(stem) + 1 + clen(&np),
            };
            Some(Change::at(s, caret))
        }
        UKind::Head { line, .. } => {
            // a heading becomes a list item with the same text
            let p = richtext::heading_prefix_len(&lines[line]);
            let np = prefix_for(kind, 1);
            lines[line] = format!("{np}{}", lines[line].chars().skip(p).collect::<String>());
            let delta = clen(&np) as i64 - p as i64;
            let mv = |g: usize| (g as i64 + delta).max(d.line_start[line] as i64) as usize;
            Some(Change { stem: lines.join("\n"), anchor: mv(a), caret: mv(c), virt: None, blank: None, keep: None })
        }
        UKind::Cell { .. } => None,
    }
}

/// Lines of a canonical table (outer pipes, so empty cells survive the parser).
fn table_lines(cells: &[Vec<String>], header: bool) -> Vec<String> {
    let row = |r: &Vec<String>| format!("| {} |", r.join(" | "));
    let mut out = Vec::new();
    for (k, r) in cells.iter().enumerate() {
        out.push(row(r));
        if k == 0 && header {
            out.push(format!("|{}|", vec!["---"; r.len()].join("|")));
        }
    }
    out
}

/// Insert a new table after the caret's line (or block). `blanks`: body cells hold `___`.
pub fn insert_table(stem: &str, d: &Doc, fi: usize, caret: usize, blanks: bool) -> Change {
    let at = d.flat()[fi];
    let u = d.unit(at);
    let body = if blanks { "___" } else { "" };
    let cells = vec![
        vec!["Column 1".to_string(), "Column 2".to_string()],
        vec![body.to_string(), body.to_string()],
        vec![body.to_string(), body.to_string()],
    ];
    let tl = table_lines(&cells, true).join("\n");
    let mut lines = lines_of(stem);
    let after_line = match (u.virt, &d.blocks[at.0].kind) {
        (Some(Virt::Before), _) => None,
        (Some(Virt::After), _) => Some(lines.len() - 1),
        (None, BlockKind::Flow) => Some(d.line_of(caret)),
        (None, _) => Some(d.blocks[at.0].lines.end - 1),
    };
    let s = match after_line {
        None => format!("{tl}\n{stem}"),
        Some(l) if lines[l].trim().is_empty() && !stem.is_empty() => {
            lines[l] = tl;
            lines.join("\n")
        }
        Some(_) if stem.is_empty() => tl,
        Some(l) => {
            lines.insert(l + 1, tl);
            lines.join("\n")
        }
    };
    // select "Column 1"
    let start = s.find("| Column 1 |").map(|b| clen(&s[..b]) + 2).unwrap_or(0);
    Change { stem: s, anchor: start, caret: start + 8, virt: None, blank: None, keep: None }
}

/// Table structure edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TOp {
    RowAbove,
    RowBelow,
    ColLeft,
    ColRight,
    DelRow,
    DelCol,
    Header,
    DelTable,
}

/// The cell texts of table block `bi`, row-major.
pub fn table_cells(stem: &str, d: &Doc, bi: usize) -> Vec<Vec<String>> {
    let bl = &d.blocks[bi];
    let BlockKind::Table { ncols, .. } = bl.kind else { return Vec::new() };
    bl.units.chunks(ncols.max(1)).map(|r| r.iter().map(|u| slice(stem, u.start..u.end)).collect()).collect()
}

/// Would the parser call this table's first row a header without a `---` line?
fn header_implied(cells: &[Vec<String>]) -> bool {
    cells.len() >= 2 && !cells[0].iter().any(|c| c.contains("___")) && cells[1..].iter().any(|r| r.iter().any(|c| c.contains("___")))
}

/// Can the header toggle turn the header off (it cannot when the blanks imply it)?
pub fn header_toggle_ok(stem: &str, d: &Doc, bi: usize) -> bool {
    match d.blocks[bi].kind {
        BlockKind::Table { header: true, sep: None, .. } => false,
        BlockKind::Table { header: true, .. } => !header_implied(&table_cells(stem, d, bi)),
        _ => true,
    }
}

/// Apply a table edit at cell (`r`, `c`) of block `bi`; the caret goes to the cell that makes
/// sense afterwards. None when the edit would leave fewer than 2 rows or columns.
pub fn table_op(stem: &str, d: &Doc, bi: usize, r: usize, c: usize, op: TOp) -> Option<Change> {
    let bl = &d.blocks[bi];
    let BlockKind::Table { header, ncols, .. } = bl.kind else { return None };
    // each cell carries the (row, col) it came from, so blank answers can follow their cells
    type Cell = (String, Option<(usize, usize)>);
    let mut cells: Vec<Vec<Cell>> = table_cells(stem, d, bi)
        .into_iter()
        .enumerate()
        .map(|(ri, row)| row.into_iter().enumerate().map(|(ci, t)| (t, Some((ri, ci)))).collect())
        .collect();
    let blank_row = |n: usize| -> Vec<Cell> { vec![(String::new(), None); n] };
    let mut header = header;
    let (mut tr, mut tc) = (r, c);
    match op {
        TOp::RowAbove => {
            cells.insert(r, blank_row(ncols));
            if r == 0 && header {
                // keep the header on top: the new row goes under it
                cells.swap(0, 1);
                tr = 1;
            }
        }
        TOp::RowBelow => {
            cells.insert(r + 1, blank_row(ncols));
            tr = r + 1;
        }
        TOp::ColLeft => {
            for row in &mut cells {
                row.insert(c, (String::new(), None));
            }
        }
        TOp::ColRight => {
            for row in &mut cells {
                row.insert(c + 1, (String::new(), None));
            }
            tc = c + 1;
        }
        TOp::DelRow => {
            if cells.len() <= 2 {
                return None;
            }
            cells.remove(r);
            tr = r.min(cells.len() - 1);
        }
        TOp::DelCol => {
            if ncols <= 2 {
                return None;
            }
            for row in &mut cells {
                row.remove(c);
            }
            tc = c.min(ncols - 2);
        }
        TOp::Header => header = !header,
        TOp::DelTable => {}
    }
    let texts: Vec<Vec<String>> = cells.iter().map(|r| r.iter().map(|c| c.0.clone()).collect()).collect();
    let mut lines = lines_of(stem);
    let new_lines = if op == TOp::DelTable { Vec::new() } else { table_lines(&texts, header) };
    let at_line = bl.lines.start;
    lines.splice(bl.lines.clone(), new_lines);
    if lines.is_empty() {
        lines.push(String::new());
    }
    let s = lines.join("\n");
    // blank answers: before the table, by cell, after the table
    let old_starts = blank_starts(stem);
    let (t0, t1) = (d.line_start[bl.lines.start], d.line_end(bl.lines.end - 1));
    let before = old_starts.iter().filter(|&&b| b < t0).count();
    let after_old: Vec<usize> = (0..old_starts.len()).filter(|&k| old_starts[k] >= t1).collect();
    let in_cell = |r: usize, c: usize| -> Vec<usize> {
        let u = &bl.units[r * ncols + c];
        (0..old_starts.len()).filter(|&k| old_starts[k] >= u.start && old_starts[k] + 3 <= u.end).collect()
    };
    let mut keep: Vec<Option<usize>> = (0..before).map(Some).collect();
    if op != TOp::DelTable {
        for row in &cells {
            for (t, from) in row {
                let n = blank_starts(t).len();
                match from {
                    Some((r0, c0)) => {
                        let olds = in_cell(*r0, *c0);
                        keep.extend((0..n).map(|j| olds.get(j).copied()));
                    }
                    None => keep.extend(std::iter::repeat_n(None, n)),
                }
            }
        }
    }
    keep.extend(after_old.iter().map(|&k| Some(k)));
    let keep = (keep.len() == blank_starts(&s).len()).then_some(keep);
    if op == TOp::DelTable {
        let ls = line_starts(&lines);
        let g = ls.get(at_line).copied().unwrap_or(clen(&s)).min(clen(&s));
        return Some(Change { keep, ..Change::at(s, g) });
    }
    let nd = doc(&s, true);
    let nbi = nd.blocks.iter().position(|b| b.lines.start == at_line && matches!(b.kind, BlockKind::Table { .. }))?;
    let BlockKind::Table { ncols: nc, .. } = nd.blocks[nbi].kind else { return None };
    let u = nd.blocks[nbi].units.get(tr * nc + tc)?;
    Some(Change { stem: s, anchor: u.start, caret: u.end, virt: None, blank: None, keep })
}

/// A cell edit: splice when the table keeps its shape, else rewrite the table canonically (so an
/// emptied edge cell does not break the row). `|` typed into a cell becomes `∣`.
pub fn cell_edit(stem: &str, d: &Doc, bi: usize, k: usize, text: &str) -> Option<String> {
    let text = text.replace(['\n', '\r'], " ").replace('|', "∣");
    let bl = &d.blocks[bi];
    let u = &bl.units[k];
    let s = splice(stem, u.start..u.end, &text);
    let BlockKind::Table { ncols: nc0, .. } = bl.kind else { return None };
    // same lines, same number of columns and cells: the splice is safe
    let keeps_shape = |dd: &Doc| dd.blocks.iter().any(|b| b.lines == bl.lines && matches!(b.kind, BlockKind::Table { ncols, .. } if ncols == nc0) && b.units.len() == bl.units.len());
    if keeps_shape(&doc(&s, true)) {
        return Some(s);
    }
    let BlockKind::Table { header, ncols, .. } = bl.kind else { return None };
    let mut cells = table_cells(stem, d, bi);
    cells[k / ncols.max(1)][k % ncols.max(1)] = text.trim().to_string();
    let mut lines = lines_of(stem);
    lines.splice(bl.lines.clone(), table_lines(&cells, header));
    let s2 = lines.join("\n");
    let nd2 = doc(&s2, true);
    nd2.blocks.iter().any(|b| b.lines.start == bl.lines.start && matches!(b.kind, BlockKind::Table { ncols: n, .. } if n == ncols)).then_some(s2)
}

/// Enter in flat unit `fi` with the local selection `a..b`. None: nothing to do (a field that
/// takes no line breaks).
pub fn enter_op(stem: &str, d: &Doc, fi: usize, a: usize, b: usize, newlines: bool) -> Option<Change> {
    let at = d.flat()[fi];
    let u = d.unit(at).clone();
    let text = slice(stem, u.start..u.end);
    match u.kind {
        UKind::Item { list, line } => {
            let mut lines = lines_of(stem);
            let p = richtext::item_prefix_len(list.seg(), &lines[line]);
            let task = task_state(&text).is_some();
            let body_empty = if task { visibly_empty(&slice(&text, 2..clen(&text))) } else { visibly_empty(&text) };
            // an empty nested item moves out a level first
            if body_empty && item_depth(stem, d, fi).is_some_and(|k| k > 0) {
                let g = u.start + a.max(b);
                return indent_op(stem, d, fi, (g, g), false);
            }
            if body_empty {
                let text = if task { String::new() } else { text.clone() };
                // leave the list: the line becomes an empty paragraph line
                lines[line] = text.clone();
                if list == ListKind::Numbered {
                    let bl = d.blocks[at.0].lines.clone();
                    renumber(&mut lines, bl.start..line);
                    renumber(&mut lines, line + 1..bl.end);
                }
                let s = lines.join("\n");
                let g = line_starts(&lines)[line];
                return Some(Change::at(s, g));
            }
            let (left, right) = split_line(&text, a, b);
            let prefix: String = lines[line].chars().take(p).collect();
            let mut next_prefix = match list {
                ListKind::Numbered => numbered_prefix(&lines[line]).map_or(prefix.clone(), |(ws, n, dl, _)| format!("{ws}{}{dl} ", n + 1)),
                _ => prefix.clone(),
            };
            // a task item continues as an open task item
            if task && a.min(b) >= 2 {
                next_prefix.push_str(&format!("{TASK_OPEN} "));
            }
            lines[line] = format!("{prefix}{left}");
            lines.insert(line + 1, format!("{next_prefix}{right}"));
            if list == ListKind::Numbered {
                let bl = d.blocks[at.0].lines.clone();
                renumber(&mut lines, bl.start..bl.end + 1);
            }
            let s = lines.join("\n");
            let ls = line_starts(&lines);
            let np = richtext::item_prefix_len(list.seg(), &lines[line + 1]) + if task && a.min(b) >= 2 { 2 } else { 0 };
            Some(Change::at(s, ls[line + 1] + np))
        }
        UKind::Cell { .. } => None,
        UKind::Head { line, .. } => {
            let mut lines = lines_of(stem);
            if visibly_empty(&text) {
                // an empty heading turns back into a paragraph line
                lines[line] = text.clone();
                return Some(Change::at(lines.join("\n"), d.line_start[line]));
            }
            let p = richtext::heading_prefix_len(&lines[line]);
            let (left, right) = split_line(&text, a, b);
            let prefix: String = lines[line].chars().take(p).collect();
            lines[line] = format!("{prefix}{left}");
            lines.insert(line + 1, right);
            let ls = line_starts(&lines);
            Some(Change::at(lines.join("\n"), ls[line + 1]))
        }
        UKind::Flow => {
            if !newlines {
                return None;
            }
            if u.virt.is_some() {
                let s = apply_unit(stem, &u, "\n");
                let caret = match u.virt {
                    Some(Virt::Before) => 0,
                    _ => clen(&s),
                };
                return Some(Change { stem: s, anchor: caret, caret, virt: None, blank: None, keep: None });
            }
            let ga = u.start + a.min(b);
            let gb = u.start + a.max(b);
            let line = d.line_of(ga);
            let ls = d.line_start[line];
            let lt = slice(stem, ls..d.line_end(line));
            // Enter at the end of a centred group's last line starts a plain paragraph below it
            if let Some(g) = &d.blocks[at.0].center
                && ga == gb
                && line + 2 == g.end
                && ga == d.line_end(line)
                && !lt.trim().is_empty()
            {
                let mut lines = lines_of(stem);
                let mut at_line = g.end;
                lines.insert(at_line, String::new());
                if at_line + 1 < lines.len() && !lines[at_line + 1].trim().is_empty() {
                    lines.insert(at_line + 1, String::new());
                } else if at_line + 1 < lines.len() {
                    // the blank line already there separates it
                    at_line = g.end;
                }
                let ls2 = line_starts(&lines);
                return Some(Change::at(lines.join("\n"), ls2[at_line]));
            }
            // "1. text" lines continue as a numbered list
            if let Some((ws, n, dl, p)) = numbered_prefix(&lt)
                && ga >= ls + p
            {
                let body: String = lt.chars().skip(p).collect();
                if visibly_empty(&body) {
                    let s = splice(stem, ls..ls + p, "");
                    return Some(Change::at(s, ls));
                }
                let (left, right) = split_line(&body, ga - ls - p, gb.min(d.line_end(line)) - ls - p);
                let np = format!("{ws}{}{dl} ", n + 1);
                let rep = format!("{}{left}\n{np}{right}", lt.chars().take(p).collect::<String>());
                let s = splice(stem, ls..d.line_end(line), &rep);
                // renumber the numbered lines that follow
                let mut ls2 = lines_of(&s);
                let mut end = line + 1;
                while end < ls2.len() && numbered_prefix(&ls2[end]).is_some() {
                    end += 1;
                }
                let mut start = line;
                while start > 0 && numbered_prefix(&ls2[start - 1]).is_some() {
                    start -= 1;
                }
                let first = numbered_prefix(&ls2[start]).map_or(1, |x| x.1);
                let mut k = first;
                for l in &mut ls2[start..end] {
                    if let Some((w, _, dl, pp)) = numbered_prefix(l) {
                        let rest: String = l.chars().skip(pp).collect();
                        *l = format!("{w}{k}{dl} {rest}");
                        k += 1;
                    }
                }
                let s2 = ls2.join("\n");
                let caret = line_starts(&ls2)[line + 1] + numbered_prefix(&ls2[line + 1]).map_or(0, |x| x.3);
                return Some(Change::at(s2, caret));
            }
            let gb_line = gb.min(d.line_end(line));
            let (left, right) = split_line(&lt, ga - ls, gb_line - ls);
            let rep = format!("{left}\n{right}");
            let s = splice(stem, ls..d.line_end(line).max(gb_line), &rep);
            // a selection that ran onto later lines: drop the rest of it too
            let s = if gb > d.line_end(line) {
                let cut_from = ls + clen(&rep);
                let cut_to = cut_from + (gb - d.line_end(line));
                splice(&s, cut_from..cut_to.min(clen(&s)), "")
            } else {
                s
            };
            Some(Change::at(s, ls + clen(&left) + 1))
        }
    }
}

/// Backspace with the caret before everything visible in unit `fi`. None: nothing to delete
/// (the caller may move focus instead).
pub fn backspace_op(stem: &str, d: &Doc, fi: usize) -> Option<Change> {
    let flat = d.flat();
    let at = flat[fi];
    let u = d.unit(at).clone();
    match u.kind {
        UKind::Item { .. } if item_depth(stem, d, fi).is_some_and(|k| k > 0) => indent_op(stem, d, fi, (u.start, u.start), false),
        UKind::Item { list, line } => {
            let mut lines = lines_of(stem);
            let p = richtext::item_prefix_len(list.seg(), &lines[line]);
            lines[line] = lines[line].chars().skip(p).collect();
            if list == ListKind::Numbered {
                let bl = d.blocks[at.0].lines.clone();
                renumber(&mut lines, bl.start..line);
                renumber(&mut lines, line + 1..bl.end);
            }
            let s = lines.join("\n");
            Some(Change::at(s, d.line_start[line]))
        }
        UKind::Head { line, .. } => {
            let mut lines = lines_of(stem);
            let p = richtext::heading_prefix_len(&lines[line]);
            lines[line] = lines[line].chars().skip(p).collect();
            Some(Change::at(lines.join("\n"), d.line_start[line]))
        }
        UKind::Flow if u.virt.is_none() && u.start > 0 => {
            let prev_line = d.line_of(u.start) - 1;
            let in_table = d.blocks.iter().any(|b| matches!(b.kind, BlockKind::Table { .. }) && b.lines.contains(&prev_line));
            if in_table || d.fences.contains(&prev_line) {
                return None;
            }
            if d.blocks.iter().any(|b| b.kind == BlockKind::Divider && b.lines.start == prev_line) {
                // remove the divider above
                let ls = d.line_start[prev_line];
                return Some(Change::at(splice(stem, ls..u.start, ""), ls));
            }
            Some(Change::at(splice(stem, u.start - 1..u.start, ""), u.start - 1))
        }
        _ => None,
    }
}

/// Delete with the caret after everything visible in unit `fi`: joins the next line.
pub fn delete_op(stem: &str, d: &Doc, fi: usize) -> Option<Change> {
    let at = d.flat()[fi];
    let u = d.unit(at).clone();
    if u.virt.is_some() || matches!(u.kind, UKind::Cell { .. }) || u.end >= d.len {
        return None;
    }
    let line = d.line_of(u.end);
    let next = line + 1;
    if next >= d.line_start.len() {
        return None;
    }
    if d.fences.contains(&next) || d.fences.contains(&line) {
        return None;
    }
    let next_block = d.blocks.iter().find(|b| b.lines.contains(&next) && b.units.iter().all(|x| x.virt.is_none()) && !matches!(b.kind, BlockKind::Side { .. }));
    let cut = match next_block.map(|b| &b.kind) {
        Some(BlockKind::Table { .. }) | Some(BlockKind::Side { .. }) => return None,
        Some(BlockKind::Heading(_)) => 1 + richtext::heading_prefix_len(stem.split('\n').nth(next).unwrap_or("")),
        Some(BlockKind::Divider) => 1 + d.line_len[next],
        Some(BlockKind::List(k)) => {
            let nl = stem.split('\n').nth(next).unwrap_or("");
            1 + richtext::item_prefix_len(k.seg(), nl)
        }
        _ => 1,
    };
    Some(Change::at(splice(stem, u.end..u.end + cut, ""), u.end))
}

// =====================================================================================
// Headings, dividers, side by side, and the slash menu
// =====================================================================================

/// Side-by-side structure edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SOp {
    /// Another (empty) panel at the right.
    AddPanel,
    /// Unwrap the group: the panels' contents one after another.
    Stack,
    /// Remove this panel (with two panels, the other one is unwrapped).
    RemovePanel(usize),
    /// Give an empty (zero-line) panel a line to write on.
    Fill(usize),
}

/// Apply `op` to the side-by-side block `si` of `d`.
pub fn side_op(stem: &str, d: &Doc, si: usize, op: SOp) -> Option<Change> {
    let bl = d.blocks.get(si)?;
    let BlockKind::Side { panels } = &bl.kind else { return None };
    let mut lines = lines_of(stem);
    let (open, close) = (bl.lines.start, bl.lines.end - 1);
    let stack = |lines: &mut Vec<String>, keep: &[Range<usize>]| -> usize {
        let mut out: Vec<String> = Vec::new();
        for r in keep {
            let content: Vec<String> = lines[r.clone()].to_vec();
            if content.iter().all(|l| l.trim().is_empty()) {
                continue;
            }
            if !out.is_empty() {
                out.push(String::new());
            }
            out.extend(content);
        }
        if out.is_empty() {
            out.push(String::new());
        }
        lines.splice(open..close + 1, out);
        open
    };
    match op {
        SOp::AddPanel => {
            lines.insert(close, String::new());
            lines.insert(close, richtext::FENCE_NEXT.to_string());
            let ls = line_starts(&lines);
            Some(Change::at(lines.join("\n"), ls[close + 1]))
        }
        SOp::Stack => {
            let l = stack(&mut lines, panels);
            let ls = line_starts(&lines);
            Some(Change::at(lines.join("\n"), ls[l]))
        }
        SOp::Fill(k) => {
            let at = panels.get(k)?.start;
            lines.insert(at, String::new());
            let ls = line_starts(&lines);
            Some(Change::at(lines.join("\n"), ls[at]))
        }
        SOp::RemovePanel(k) => {
            if k >= panels.len() {
                return None;
            }
            if panels.len() <= 2 {
                let keep: Vec<Range<usize>> = panels.iter().enumerate().filter(|(i, _)| *i != k).map(|(_, r)| r.clone()).collect();
                let l = stack(&mut lines, &keep);
                let ls = line_starts(&lines);
                return Some(Change::at(lines.join("\n"), ls[l]));
            }
            let r = panels[k].clone();
            // the panel's lines and one neighbouring fence (its own `::: next`, or the one before it)
            let cut = if k == 0 { r.start..r.end + 1 } else { r.start - 1..r.end };
            lines.drain(cut.clone());
            let ls = line_starts(&lines);
            let at = if k == 0 { cut.start } else { cut.start.saturating_sub(1).max(open + 1) };
            Some(Change::at(lines.join("\n"), ls[at.min(ls.len() - 1)]))
        }
    }
}

/// Remove source line `line` (a divider) and its line break.
pub fn delete_line(stem: &str, line: usize) -> Change {
    let mut lines = lines_of(stem);
    if line < lines.len() {
        lines.remove(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    let ls = line_starts(&lines);
    let at = ls[line.min(ls.len() - 1)];
    Change::at(lines.join("\n"), at)
}

/// Commands of the slash menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slash {
    Text,
    H1,
    H2,
    H3,
    /// Heading 4-6: a line of its own in bold / bold italic / italic.
    H4,
    H5,
    H6,
    Quote,
    Divider,
    Bullets,
    Numbered,
    /// A bullet item starting with an open box (`• ☐ `).
    Task,
    Image,
    Table,
    Math,
    Side,
}

pub struct SlashItem {
    pub cmd: Slash,
    pub name: &'static str,
    /// 0 text, 1 list, 2 advanced
    pub group: u8,
    /// Extra words the filter matches.
    pub keys: &'static str,
    pub icon: &'static str,
}

pub const SLASH_GROUPS: [&str; 3] = ["Text", "List", "Advanced"];

pub const SLASH_ITEMS: &[SlashItem] = &[
    SlashItem { cmd: Slash::Text, name: "Text", group: 0, keys: "paragraph plain normal", icon: "T" },
    SlashItem { cmd: Slash::H1, name: "Heading 1", group: 0, keys: "title h1 big", icon: "H1" },
    SlashItem { cmd: Slash::H2, name: "Heading 2", group: 0, keys: "subtitle h2", icon: "H2" },
    SlashItem { cmd: Slash::H3, name: "Heading 3", group: 0, keys: "h3 small", icon: "H3" },
    SlashItem { cmd: Slash::H4, name: "Heading 4", group: 0, keys: "h4 bold minor", icon: "H4" },
    SlashItem { cmd: Slash::H5, name: "Heading 5", group: 0, keys: "h5 minor", icon: "H5" },
    SlashItem { cmd: Slash::H6, name: "Heading 6", group: 0, keys: "h6 italic minor", icon: "H6" },
    SlashItem { cmd: Slash::Quote, name: "Quote", group: 0, keys: "blockquote citation", icon: "❝" },
    SlashItem { cmd: Slash::Divider, name: "Divider", group: 0, keys: "rule line separator hr", icon: "—" },
    SlashItem { cmd: Slash::Bullets, name: "Bullet list", group: 1, keys: "unordered points", icon: "•≡" },
    SlashItem { cmd: Slash::Numbered, name: "Numbered list", group: 1, keys: "ordered steps", icon: "1≡" },
    SlashItem { cmd: Slash::Task, name: "Task list", group: 1, keys: "checklist todo checkbox tick", icon: "☑" },
    SlashItem { cmd: Slash::Image, name: "Image placeholder", group: 2, keys: "picture photo figure diagram", icon: "▣" },
    SlashItem { cmd: Slash::Table, name: "Table", group: 2, keys: "grid rows columns", icon: "▦" },
    SlashItem { cmd: Slash::Math, name: "Math", group: 2, keys: "equation formula latex", icon: "Σ" },
    SlashItem { cmd: Slash::Side, name: "Side by side", group: 2, keys: "columns two panels", icon: "◫" },
];

/// Where the slash was typed (decides what the menu offers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashWhere {
    /// A paragraph, heading or list item outside any panel.
    Top,
    /// Inside a side-by-side panel (no nested side by side).
    Panel,
    /// A table cell (inline things only).
    Cell,
    /// A list or quote item (no side by side there).
    Item,
}

/// The menu entries matching `query` (case-insensitive: a word of the name or of its keywords
/// starts with it), in menu order.
pub fn slash_matches(query: &str, at: SlashWhere) -> Vec<&'static SlashItem> {
    let q = query.to_lowercase();
    SLASH_ITEMS
        .iter()
        .filter(|it| match at {
            SlashWhere::Cell => matches!(it.cmd, Slash::Math | Slash::Image),
            SlashWhere::Panel | SlashWhere::Item => it.cmd != Slash::Side,
            SlashWhere::Top => true,
        })
        .filter(|it| {
            let name = it.name.to_lowercase();
            q.is_empty() || name.starts_with(&q) || name.split(' ').chain(it.keys.split(' ')).any(|w| w.starts_with(&q))
        })
        .collect()
}

/// What choosing a slash command does: the edit, and for Math the caret where the equation
/// dialog should insert.
pub struct SlashDone {
    pub change: Change,
    pub eq_at: Option<usize>,
}

/// Run slash command `cmd` typed as `/query` at chars `slash..caret` (global): the `/query` text
/// goes, then the command applies at that spot. None when it cannot apply there.
pub fn slash_apply(stem: &str, slash: usize, caret: usize, cmd: Slash, blanks: bool) -> Option<SlashDone> {
    let s = splice(stem, slash..caret.max(slash), "");
    let g = slash;
    let d = doc(&s, true);
    let fi = d.find(g, None)?;
    let u = d.unit(d.flat()[fi]).clone();
    let in_panel = d.blocks[d.flat()[fi].0].panel.is_some();
    let line = d.line_of(g);
    let mut lines = lines_of(&s);
    let done = |c: Change| Some(SlashDone { change: c, eq_at: None });
    // the line's text without a list / heading prefix: (stripped line, chars removed)
    let strip = |l: &str| -> (String, usize) {
        let p = match u.kind {
            UKind::Item { list, .. } => richtext::item_prefix_len(list.seg(), l),
            UKind::Head { .. } => richtext::heading_prefix_len(l),
            _ => 0,
        };
        (l.chars().skip(p).collect(), p)
    };
    match cmd {
        Slash::Math => Some(SlashDone { change: Change::at(s, g), eq_at: Some(g) }),
        Slash::Image => {
            let ph = "[picture: describe the picture]";
            let t = splice(&s, g..g, ph);
            let a = g + "[picture: ".chars().count();
            done(Change { stem: t, anchor: a, caret: a + "describe the picture".chars().count(), virt: None, blank: None, keep: None })
        }
        _ if matches!(u.kind, UKind::Cell { .. }) => None,
        Slash::Table => done(insert_table(&s, &d, fi, g, blanks)),
        Slash::Bullets | Slash::Numbered | Slash::Quote => {
            let kind = match cmd {
                Slash::Bullets => ListKind::Bullets,
                Slash::Numbered => ListKind::Numbered,
                _ => ListKind::Quote,
            };
            if matches!(u.kind, UKind::Item { list, .. } if list == kind) {
                return done(Change::at(s, g));
            }
            done(toggle_list(&s, &d, fi, (g, g), kind)?)
        }
        Slash::Text => match u.kind {
            UKind::Item { list, .. } => done(toggle_list(&s, &d, fi, (g, g), list)?),
            UKind::Head { .. } => {
                let (t, p) = strip(&lines[line]);
                lines[line] = t;
                done(Change::at(lines.join("\n"), g.saturating_sub(p).max(d.line_start[line])))
            }
            _ => done(Change::at(s, g)),
        },
        Slash::H1 | Slash::H2 | Slash::H3 => {
            let n = match cmd {
                Slash::H1 => 1,
                Slash::H2 => 2,
                _ => 3,
            };
            let (t, p) = strip(&lines[line]);
            let t = if matches!(u.kind, UKind::Flow) { t.trim_start().to_string() } else { t };
            let removed = clen(&lines[line]) - clen(&t);
            let _ = p;
            let pre = format!("{} ", "#".repeat(n));
            lines[line] = format!("{pre}{t}");
            let ls = d.line_start[line];
            let at = (g.saturating_sub(removed)).max(ls) + n + 1;
            done(Change::at(lines.join("\n"), at))
        }
        Slash::H4 | Slash::H5 | Slash::H6 => {
            let level = match cmd {
                Slash::H4 => 4,
                Slash::H5 => 5,
                _ => 6,
            };
            let (t, _) = strip(&lines[line]);
            let (styled, x, y) = minor_heading_line(&t, level)?;
            lines[line] = styled;
            let ls = d.line_start[line];
            done(Change { stem: lines.join("\n"), anchor: ls + x, caret: ls + y, virt: None, blank: None, keep: None })
        }
        Slash::Task => done(make_task(&s, &d, fi, g)?),
        Slash::Divider => {
            if lines[line].trim().is_empty() {
                lines[line] = "---".to_string();
                if line + 1 < lines.len() {
                    // an empty line under the divider to keep writing on
                    if !lines[line + 1].trim().is_empty() {
                        lines.insert(line + 1, String::new());
                    }
                    let ls = line_starts(&lines);
                    done(Change::at(lines.join("\n"), ls[line + 1]))
                } else {
                    let st = lines.join("\n");
                    let n = clen(&st);
                    done(Change { stem: st, anchor: n + 1, caret: n + 1, virt: Some(Virt::After), blank: None, keep: None })
                }
            } else {
                lines.insert(line + 1, "---".to_string());
                done(Change::at(lines.join("\n"), g))
            }
        }
        Slash::Side => {
            if in_panel || !matches!(u.kind, UKind::Flow | UKind::Head { .. }) {
                return None;
            }
            let old = lines[line].clone();
            let ins: Vec<String> = if old.trim().is_empty() {
                vec![richtext::FENCE_OPEN.into(), String::new(), richtext::FENCE_NEXT.into(), String::new(), richtext::FENCE_CLOSE.into()]
            } else {
                vec![richtext::FENCE_OPEN.into(), old, richtext::FENCE_NEXT.into(), String::new(), richtext::FENCE_CLOSE.into()]
            };
            lines.splice(line..line + 1, ins);
            let ls = line_starts(&lines);
            let at = ls[line + 1] + (g - d.line_start[line]).min(clen(&lines[line + 1]));
            done(Change::at(lines.join("\n"), at))
        }
    }
}

// =====================================================================================
// The widget
// =====================================================================================

/// Where an equation should be inserted or which one should be edited.
#[derive(Clone, Debug, PartialEq)]
pub struct EqAsk {
    pub caret: usize,
    pub span: Option<Range<usize>>,
    pub initial: String,
}

pub struct Opts<'a> {
    pub size: f32,
    pub width: f32,
    /// Lists, quotes and tables as blocks (stems); false: inline markup only (answer fields).
    pub structured: bool,
    /// Enter adds a line in a paragraph (false: Enter does nothing).
    pub newlines: bool,
    pub hint: &'a str,
    /// Fill-in-the-blank answer rows, one per `___` (kept in sync; enables Make blank).
    pub blanks: Option<&'a mut Vec<String>>,
    /// The fixed formatting toolbar above the text.
    pub toolbar: bool,
}

pub struct Out {
    pub changed: bool,
    pub eq: Option<EqAsk>,
    pub rect: Rect,
    pub focused: bool,
    /// The caret (global) of the last focused unit, for an external insert.
    pub caret: Option<usize>,
}

impl Default for Out {
    fn default() -> Self {
        Out { changed: false, eq: None, rect: Rect::NOTHING, focused: false, caret: None }
    }
}

#[derive(Clone, Debug)]
struct Want {
    anchor: usize,
    caret: usize,
    virt: Option<Virt>,
}

#[derive(Clone, Debug)]
struct Snap {
    stem: String,
    caret: usize,
    blanks: Option<Vec<String>>,
}

/// Per-editor memory (egui temp data).
#[derive(Clone, Default)]
struct Mem {
    want: Option<Want>,
    /// Global (anchor, caret) of the focused unit, kept after a toolbar click took focus.
    sel: Option<(usize, usize)>,
    sel_unit: Option<usize>,
    /// Focused unit id and local caret last frame (arrow-key caret snapping).
    prev: Option<(UiId, usize)>,
    /// Caret on the first / last row of the focused unit last frame.
    rows: (bool, bool),
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last_push: f64,
    last_unit: Option<UiId>,
    markup: bool,
    float_rect: Option<Rect>,
    /// The selection rect the floating toolbar was last placed for.
    float_sel: Option<Rect>,
    /// (block, row, col) of the last focused table cell.
    cell: Option<(usize, usize, usize)>,
    active: bool,
    /// A focus request that did not land yet (a closing modal can block it for a frame).
    retry: Option<(Want, u8)>,
    /// Typing mode: a format toggled with a collapsed caret at this global position; the next
    /// text typed there takes this style (nothing is selected or changed until then).
    pending: Option<(usize, Style)>,
    /// The slash menu: global char of its `/`, highlighted row, group filter.
    slash: Option<SlashSt>,
    slash_rect: Option<Rect>,
    /// Screen rect of the caret in the focused unit (last drawn).
    caret_rect: Option<Rect>,
    /// The link popover, while open.
    link: Option<LinkSt>,
    /// The top-level block whose rail grip is being dragged.
    drag: Option<usize>,
}

/// The link popover: the selection it was opened on (global chars), its unit, the address
/// being typed, whether the selection already is a link, and whether it has taken focus.
#[derive(Clone, Debug, Default)]
struct LinkSt {
    sel: (usize, usize),
    unit: usize,
    url: String,
    existing: bool,
    focused: bool,
    at: Option<Pos2>,
    /// The last Apply could not link the selection.
    err: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct SlashSt {
    at: usize,
    hi: usize,
    tab: Option<u8>,
}

/// The open slash menu's query and context, when `/` at `st.at` still leads the text up to the
/// caret `gc` in flat unit `fi` (no spaces, at most 24 chars).
fn slash_query(stem: &str, d: &Doc, fi: usize, gc: usize, st: &SlashSt) -> Option<(String, SlashWhere)> {
    let at = d.flat()[fi];
    let u = d.unit(at);
    if u.virt.is_some() || st.at < u.start || st.at >= u.end || gc <= st.at || gc > u.end {
        return None;
    }
    if slice(stem, st.at..st.at + 1) != "/" {
        return None;
    }
    let q = slice(stem, st.at + 1..gc);
    if q.chars().count() > 24 || q.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    let wh = match u.kind {
        UKind::Cell { .. } => SlashWhere::Cell,
        _ if d.blocks[at.0].panel.is_some() => SlashWhere::Panel,
        UKind::Item { .. } => SlashWhere::Item,
        _ => SlashWhere::Top,
    };
    Some((q, wh))
}

fn slash_list(q: &str, wh: SlashWhere, tab: Option<u8>) -> Vec<&'static SlashItem> {
    slash_matches(q, wh).into_iter().filter(|it| tab.is_none_or(|t| it.group == t)).collect()
}

/// Style the layouter needs.
#[derive(Clone)]
struct Sty {
    size: f32,
    color: Color32,
    family: FontFamily,
    bold: FontFamily,
    blank_w: f32,
    /// Rows centred in the unit's width (a `::: center` group).
    center: bool,
}

thread_local! {
    static METRICS: std::cell::RefCell<std::collections::HashMap<(String, u32), (f32, f32)>> = Default::default();
}

/// (ascent, row height) of a font, measured once.
fn metrics(ui: &Ui, font: &FontId) -> (f32, f32) {
    let key = (format!("{:?}", font.family), font.size.to_bits());
    if let Some(m) = METRICS.with(|c| c.borrow().get(&key).copied()) {
        return m;
    }
    let g = ui.fonts_mut(|f| f.layout_no_wrap("Hx".to_string(), font.clone(), Color32::WHITE));
    let m = g.rows.first().map_or((font.size * 0.8, font.size * 1.2), |r| (r.row.glyphs.first().map_or(font.size * 0.8, |gl| gl.pos.y), r.row.size.y));
    METRICS.with(|c| c.borrow_mut().insert(key, m));
    m
}

fn math_ok_ui(ctx: &egui::Context, size: f32) -> impl Fn(&str) -> bool + '_ {
    move |c: &str| crate::equation::math_image(ctx, c, size, f32::INFINITY).is_some()
}

/// Width, reserved height and (for math) baseline of atom `a`.
fn atom_box(ui: &Ui, a: &Atom, st: &Sty, blank_label: &dyn Fn(usize) -> Option<String>) -> (f32, f32, f32) {
    let main = FontId::new(st.size, st.family.clone());
    let (asc, l) = metrics(ui, &main);
    match &a.kind {
        AtomKind::Math(c) => {
            let Some((_, sz, b)) = crate::equation::math_image(ui.ctx(), c, st.size, f32::INFINITY) else { return (st.size, l, 0.0) };
            let h = 2.0 * (b - asc + 0.5 * l).max(sz.y - b + asc - 0.5 * l).max(0.5 * l);
            (sz.x + st.size * 0.24, h + 2.0, b)
        }
        AtomKind::Blank(n) => match blank_label(*n) {
            Some(t) => {
                let g = ui.fonts_mut(|f| f.layout_no_wrap(t, FontId::new(st.size * 0.86, st.family.clone()), Color32::WHITE));
                (g.size().x + st.size * 2.2, l, 0.0)
            }
            None => (st.blank_w, l, 0.0),
        },
    }
}

/// The layout job for a unit's text.
fn build_job(ui: &Ui, text: &str, wrap: f32, st: &Sty, ana: &Ana, blank_label: &dyn Fn(usize) -> Option<String>) -> LayoutJob {
    let main = FontId::new(st.size, st.family.clone());
    let (asc, l) = metrics(ui, &main);
    let small = st.size * 0.68;
    let (asc_s, _) = metrics(ui, &FontId::new(small, st.family.clone()));
    let tiny = FontId::new(0.1, st.family.clone());
    let fmt = |font: FontId, color: Color32, italics: bool, lh: f32| {
        // The family's real italic face when the house fonts have one (egui's slant otherwise).
        let (font, italics) = match italics.then(|| crate::theme::italic_family(ui.ctx(), &font.family)).flatten() {
            Some(family) => (FontId::new(font.size, family), false),
            None => (font, italics),
        };
        TextFormat { font_id: font, color, italics, line_height: Some(lh.max(1.0)), valign: Align::Center, ..Default::default() }
    };
    let boxes: Vec<(f32, f32, f32)> = ana.atoms.iter().map(|a| atom_box(ui, a, st, blank_label)).collect();
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    job.keep_trailing_whitespace = true;
    job.halign = if st.center { Align::Center } else { Align::LEFT };
    let ch: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < ch.len() {
        let r = ana.role[i];
        // extent of chars sharing this format
        let mut j = i + 1;
        match r {
            Role::Head(k) => j = ana.atoms[k].start + 2,
            _ => {
                while j < ch.len() && ana.role[j] == r && ana.link[j] == ana.link[i] && ana.code[j] == ana.code[i] && !matches!(r, Role::Head(_)) {
                    j += 1;
                }
            }
        }
        let s: String = ch[i..j].iter().collect();
        let is_link = ana.link[i];
        let format = match r {
            Role::Text { b, i: it, sub, sup, s: strike } => {
                let fam = if b { st.bold.clone() } else { st.family.clone() };
                let mut f = if sub || sup {
                    let d = if sup { -0.33 * st.size } else { 0.16 * st.size };
                    fmt(FontId::new(small, fam), st.color, it, l + 2.0 * (asc_s - asc - d))
                } else {
                    fmt(FontId::new(st.size, fam), st.color, it, l)
                };
                if strike {
                    f.strikethrough = Stroke::new((st.size / 14.0).max(1.0), st.color);
                }
                if is_link {
                    f.color = crate::equation::link_color(st.color);
                    f.underline = Stroke::new((st.size / 14.0).max(1.0), f.color);
                }
                if ana.code[i] {
                    f.font_id = FontId::new(f.font_id.size * 0.92, FontFamily::Monospace);
                    f.background = crate::equation::code_tint(st.color);
                    f.italics = it;
                }
                f
            }
            Role::Newline => fmt(main.clone(), st.color, false, l),
            Role::Bad => fmt(main.clone(), theme::bad(), false, l),
            Role::Hidden | Role::Tail(_) => fmt(tiny.clone(), Color32::TRANSPARENT, false, l),
            Role::Head(k) => {
                // first char plain, the second carries the atom's width as leading space (epaint
                // applies extra_letter_spacing before every glyph not starting a paragraph, so
                // leading_space is the only exact way to reserve it)
                let (w, h, _) = boxes[k];
                let f = fmt(tiny.clone(), Color32::TRANSPARENT, false, h);
                let c0: String = ch[i..i + 1].iter().collect();
                let c1: String = ch[i + 1..j].iter().collect();
                job.append(&c0, 0.0, f.clone());
                job.append(&c1, w, f);
                i = j;
                continue;
            }
        };
        job.append(&s, 0.0, format);
        i = j;
    }
    if ch.is_empty() {
        job.append("", 0.0, fmt(main.clone(), st.color, false, l));
    }
    job
}

/// Screen rect of atom `a` in a laid-out unit.
fn atom_rect(galley: &Galley, origin: Pos2, a: &Atom) -> Rect {
    let r0 = galley.pos_from_cursor(CCursor::new(CharIndex::from(a.start)));
    let r1 = galley.pos_from_cursor(CCursor::new(CharIndex::from(a.start + 1)));
    if (r1.min.y - r0.min.y).abs() < 0.5 {
        Rect::from_min_max(r0.min, Pos2::new(r1.min.x.max(r0.min.x + 1.0), r0.max.y)).translate(origin.to_vec2())
    } else {
        // the row broke inside the atom: its slot is the start of the next row
        Rect::from_min_max(Pos2::new(0.0, r1.min.y), Pos2::new(r1.min.x.max(1.0), r1.max.y)).translate(origin.to_vec2())
    }
}

struct UnitShow {
    out: egui::text_edit::TextEditOutput,
    ana: Ana,
    origin: Pos2,
    clicked_atom: Option<Atom>,
}

#[allow(clippy::too_many_arguments)]
fn show_unit(ui: &mut Ui, uid: UiId, text: &mut String, width: f32, st: &Sty, hint: &str, blank_base: usize, blanks: Option<&[String]>, cell: bool) -> UnitShow {
    let ctx = ui.ctx().clone();
    let label = |n: usize| -> Option<String> {
        let rows = blanks?;
        let ans = rows.get(blank_base + n).map(|r| r.split(" OR ").next().unwrap_or("").trim().to_string()).unwrap_or_default();
        Some(if ans.is_empty() { format!("Blank {}", blank_base + n + 1) } else { ans })
    };
    let st2 = st.clone();
    let mut lay = |ui: &Ui, buf: &dyn TextBuffer, wrap: f32| -> Arc<Galley> {
        let ana = analyze(buf.as_str(), &math_ok_ui(ui.ctx(), st2.size));
        let job = build_job(ui, buf.as_str(), wrap, &st2, &ana, &label);
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let mut te = TextEdit::multiline(text)
        .id(uid)
        .frame(Frame::new())
        .font(FontId::new(st.size, st.family.clone()))
        .desired_width(width)
        .desired_rows(1)
        .return_key(None)
        // in a table Tab moves between cells, in a list it nests the item (both handled by the
        // editor), so egui must not move focus
        .lock_focus(cell)
        .horizontal_align(if st.center { Align::Center } else { Align::LEFT })
        .layouter(&mut lay);
    if !hint.is_empty() {
        te = te.hint_text(RichText::new(hint.to_string()).size(st.size));
    }
    let out = te.show(ui);
    let ana = analyze(text, &math_ok_ui(&ctx, st.size));
    let origin = out.galley_pos - Vec2::new(out.galley.rect.left(), 0.0);
    // paint atoms into their reserved slots
    let painter = ui.painter_at(out.text_clip_rect.expand(4.0));
    let resp = &out.response.response;
    let hover = resp.hover_pos();
    let mut clicked_atom = None;
    let main = FontId::new(st.size, st.family.clone());
    let (asc, l) = metrics(ui, &main);
    let sel = out.state.cursor.char_range().map(|r| {
        let [x, y] = r.sorted_cursors();
        usize::from(x.index)..usize::from(y.index)
    });
    let focused = ui.memory(|m| m.has_focus(uid));
    let dbg = crate::env_var("VERSHION_WYS_DEBUG").is_ok();
    if dbg && !ana.atoms.is_empty() {
        eprintln!("wys unit {:?}: galley {:?} clip {:?} width {width} rows {:?}", text.chars().take(30).collect::<String>(), out.galley.rect, out.text_clip_rect, out.galley.rows.iter().map(|r| (r.pos, r.row.size)).collect::<Vec<_>>());
    }
    for a in &ana.atoms {
        let r = atom_rect(&out.galley, origin, a);
        if dbg {
            eprintln!("  atom {:?} rect {:?}", a, r);
        }
        let hovered = hover.is_some_and(|p| r.contains(p));
        let selected = focused && sel.as_ref().is_some_and(|s| s.start <= a.start && a.end <= s.end && s.start < s.end);
        if hovered {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && hovered {
            clicked_atom = Some(a.clone());
        }
        let base_y = r.top() + asc + 0.5 * (r.height() - l);
        match &a.kind {
            AtomKind::Math(c) => {
                if hovered || selected {
                    painter.rect_filled(r.expand2(Vec2::new(0.0, -1.0)), 5.0, theme::accent_faded());
                }
                if let Some((tex, sz, b)) = crate::equation::math_image(&ctx, c, st.size, f32::INFINITY) {
                    let x = r.left() + (r.width() - sz.x) / 2.0;
                    let y = (base_y - b).clamp(r.top(), (r.bottom() - sz.y).max(r.top()));
                    painter.image(tex, Rect::from_min_size(Pos2::new(x, y), sz), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), st.color);
                }
            }
            AtomKind::Blank(n) => match label(*n) {
                Some(t) => {
                    let pill = Rect::from_min_max(Pos2::new(r.left() + 1.0, r.top() + 2.0), Pos2::new(r.right() - 1.0, r.bottom() - 2.0));
                    let fill = if selected { theme::accent_col().gamma_multiply(0.35) } else if hovered { theme::accent_faded() } else { theme::accent_faded().gamma_multiply(0.7) };
                    painter.rect(pill, CornerRadius::same(6), fill, Stroke::new(1.0, theme::accent_col().gamma_multiply(0.6)), StrokeKind::Inside);
                    let num = format!("{}", blank_base + n + 1);
                    let nf = FontId::new(st.size * 0.62, theme::bold_family());
                    let cx = pill.left() + st.size * 0.62;
                    painter.circle_filled(Pos2::new(cx, pill.center().y), st.size * 0.42, theme::accent_col());
                    painter.text(Pos2::new(cx, pill.center().y), egui::Align2::CENTER_CENTER, num, nf, theme::card());
                    painter.text(
                        Pos2::new(pill.left() + st.size * 1.25, pill.center().y),
                        egui::Align2::LEFT_CENTER,
                        t,
                        FontId::new(st.size * 0.86, st.family.clone()),
                        theme::text_col(),
                    );
                }
                None => {
                    let y = (base_y + 2.0).min(r.bottom() - 1.0);
                    painter.line_segment([Pos2::new(r.left() + 1.0, y), Pos2::new(r.right() - 2.0, y)], Stroke::new(1.0, st.color));
                    if selected {
                        painter.rect_filled(r, 3.0, theme::accent_faded().gamma_multiply(0.6));
                    }
                }
            },
        }
    }
    UnitShow { out, ana, origin, clicked_atom }
}

/// Remove and report a key press (logical or physical key) with exactly these modifiers.
fn take_key(ui: &Ui, mods: Modifiers, key: Key) -> bool {
    ui.input_mut(|i| {
        let pos = i.events.iter().position(|e| {
            matches!(e, Event::Key { key: k, physical_key, pressed: true, modifiers, .. }
                if (*k == key || *physical_key == Some(key)) && modifiers.matches_exact(mods))
        });
        if let Some(p) = pos {
            i.events.remove(p);
            // drop the matching release too, so nothing else reacts to it
            true
        } else {
            false
        }
    })
}

fn key_down(ui: &Ui, key: Key) -> bool {
    ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && !modifiers.command && !modifiers.alt)))
}

fn set_cursor(ctx: &egui::Context, uid: UiId, anchor: usize, caret: usize) {
    let mut st = TextEdit::load_state(ctx, uid).unwrap_or_default();
    st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(CharIndex::from(anchor)), CCursor::new(CharIndex::from(caret)))));
    TextEdit::store_state(ctx, uid, st);
}

fn local_sel(ctx: &egui::Context, uid: UiId, n: usize) -> (usize, usize) {
    TextEdit::load_state(ctx, uid)
        .and_then(|s| s.cursor.char_range())
        .map_or((n, n), |r| (usize::from(r.secondary.index).min(n), usize::from(r.primary.index).min(n)))
}

/// Identity of each unit (stable while the structure is).
fn unit_ids(id: UiId, d: &Doc) -> Vec<UiId> {
    d.flat().into_iter().map(|(b, u)| id.with(("wys_unit", b, u))).collect()
}

/// Toolbar actions.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Act {
    Inline(IOp),
    /// Open the link popover for the selection (or the link at the caret).
    Link,
    /// The popover's Apply / Remove link.
    LinkApply,
    LinkRemove,
    List(ListKind),
    Table,
    Equation,
    Blank,
    Undo,
    Redo,
    Markup,
    TableOp(TOp),
    /// Inline code on the selection (Ctrl+E).
    Code,
    /// Centre the caret's paragraph or heading, or end its centring (Ctrl+Shift+E).
    Center,
    /// Tab (true) / Shift+Tab in a list item.
    Indent(bool),
    /// Tick / untick the caret's task item (Ctrl+Enter, or a click on its box).
    Task,
    /// Move the caret's top-level block up (true) / down (Alt+Up / Alt+Down).
    Move(bool),
}

/// The context the toolbar reflects: style at the caret and the block kind.
#[derive(Default, Clone, Copy)]
struct Here {
    b: bool,
    i: bool,
    sub: bool,
    sup: bool,
    s: bool,
    list: Option<ListKind>,
    table: bool,
    sel: bool,
    on_blank: bool,
    /// The selection or caret is in a link.
    link: bool,
    /// The selection or caret is in `` `code` ``.
    code: bool,
    /// The caret's block is centred.
    center: bool,
    /// The Centre button applies here (a paragraph or heading, or a centred block).
    center_ok: bool,
}

fn tool_btn(ui: &mut Ui, label: RichText, tip: &str, on: bool, enabled: bool) -> bool {
    let b = egui::Button::new(label.size(13.0).color(if on { theme::accent_col() } else { theme::text_col() }))
        .fill(if on { theme::accent_faded() } else { Color32::TRANSPARENT })
        .stroke(Stroke::new(1.0, if on { theme::accent_col().gamma_multiply(0.5) } else { theme::border() }))
        .corner_radius(CornerRadius::same(7))
        .min_size(Vec2::new(28.0, 24.0));
    ui.add_enabled(enabled, b).on_hover_text(tip).clicked()
}

fn toolbar(ui: &mut Ui, here: Here, blanks: bool, markup: bool, structured: bool) -> Option<Act> {
    let mut act = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
        ui.spacing_mut().button_padding.x = ui.spacing().button_padding.x.min(7.0);
        let mut b = |ui: &mut Ui, l: RichText, tip: &str, on: bool, en: bool, a: Act| {
            if tool_btn(ui, l, tip, on, en) {
                act = Some(a);
            }
        };
        let vis = !markup;
        b(ui, RichText::new("B").family(theme::bold_family()), "Bold (Ctrl+B)", here.b, vis, Act::Inline(IOp::Bold));
        b(ui, RichText::new("I").italics(), "Italic (Ctrl+I)", here.i, vis, Act::Inline(IOp::Italic));
        b(ui, RichText::new("S").strikethrough(), "Strikethrough (Ctrl+Shift+S)", here.s, vis, Act::Inline(IOp::Strike));
        b(ui, RichText::new("x₂"), "Subscript (Ctrl+,)", here.sub, vis, Act::Inline(IOp::Sub));
        b(ui, RichText::new("x²"), "Superscript (Ctrl+.)", here.sup, vis, Act::Inline(IOp::Sup));
        b(ui, RichText::new("code").family(FontFamily::Monospace), "Inline code (Ctrl+E)", here.code, vis && (here.sel || here.code), Act::Code);
        if structured {
            b(ui, RichText::new("Centre"), "Centre the paragraph or heading (Ctrl+Shift+E)", here.center, vis && here.center_ok, Act::Center);
            ui.add_space(4.0);
            b(ui, RichText::new("•  ≡"), "Bullet list (Ctrl+Shift+8 or Ctrl+Alt+8)", here.list == Some(ListKind::Bullets), vis && !here.table, Act::List(ListKind::Bullets));
            b(ui, RichText::new("1. ≡"), "Numbered list (Ctrl+Shift+7 or Ctrl+Alt+7)", here.list == Some(ListKind::Numbered), vis && !here.table, Act::List(ListKind::Numbered));
            b(ui, RichText::new("❝"), "Quote (Ctrl+Shift+B)", here.list == Some(ListKind::Quote), vis && !here.table, Act::List(ListKind::Quote));
            b(ui, RichText::new("Table"), "Insert a table", here.table, vis && !here.table, Act::Table);
        }
        ui.add_space(4.0);
        b(ui, RichText::new("Σ"), if structured { "Equation (Ctrl+M; or type / for the slash menu)" } else { "Equation (Ctrl+M, or type / at the start of a word)" }, false, vis, Act::Equation);
        if blanks {
            let tip = if here.on_blank { "Put the answer back as text (Ctrl+Shift+K)" } else { "Make the selected word a blank; it becomes the answer (Ctrl+Shift+K)" };
            b(ui, RichText::new("___ Blank"), tip, here.on_blank, vis && (here.sel || here.on_blank), Act::Blank);
        }
        b(ui, RichText::new("Link").underline(), "Link the selected text to a web or e-mail address (Ctrl+K)", here.link, vis, Act::Link);
        ui.add_space(4.0);
        if tool_btn(ui, RichText::new("</>"), if markup { "Back to the visual editor" } else { "Edit the markup text" }, markup, true) {
            act = Some(Act::Markup);
        }
    });
    act
}

/// Width of the block rail's gutter at the left of a stem.
const RAIL_W: f32 = 34.0;

/// Height of a table's control row (above the grid).
const TABLE_BAR_H: f32 = 24.0;

/// Where the floating toolbar goes for selection rect `sel`: above it, unless that would cover
/// the fixed toolbar (`fixed`) or leave the window, then below it. `size`: the bar's size last
/// frame (estimated before it was first drawn).
fn float_place(sel: Rect, size: Option<Vec2>, fixed: Option<Rect>, screen: Rect) -> (Pos2, egui::Align2) {
    let sz = size.unwrap_or(Vec2::new(300.0, 36.0));
    let above = Rect::from_min_size(Pos2::new(sel.left(), sel.top() - 6.0 - sz.y), sz);
    let blocked = fixed.is_some_and(|f| f.intersects(above)) || above.top() < screen.top();
    if blocked {
        (Pos2::new(sel.left(), sel.bottom() + 6.0), egui::Align2::LEFT_TOP)
    } else {
        (Pos2::new(sel.left(), sel.top() - 6.0), egui::Align2::LEFT_BOTTOM)
    }
}

/// The floating toolbar over a selection.
#[allow(clippy::too_many_arguments)]
fn float_bar(ui: &Ui, id: UiId, at: Pos2, pivot: egui::Align2, here: Here, blanks: bool, structured: bool) -> (Option<Act>, Rect) {
    let mut act = None;
    let r = egui::Area::new(id.with("wys_float"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .pivot(pivot)
        .show(ui.ctx(), |ui| {
            Frame::new()
                .fill(theme::card())
                .stroke(Stroke::new(1.0, theme::border()))
                .corner_radius(CornerRadius::same(10))
                .shadow(egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(40) })
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
                        let mut b = |ui: &mut Ui, l: RichText, tip: &str, on: bool, a: Act| {
                            if tool_btn(ui, l, tip, on, true) {
                                act = Some(a);
                            }
                        };
                        if here.on_blank && blanks {
                            b(ui, RichText::new("___ Blank"), "Blank: press to put the answer back as text", true, Act::Blank);
                            return;
                        }
                        b(ui, RichText::new("B").family(theme::bold_family()), "Bold", here.b, Act::Inline(IOp::Bold));
                        b(ui, RichText::new("I").italics(), "Italic", here.i, Act::Inline(IOp::Italic));
                        b(ui, RichText::new("S").strikethrough(), "Strikethrough", here.s, Act::Inline(IOp::Strike));
                        b(ui, RichText::new("x₂"), "Subscript", here.sub, Act::Inline(IOp::Sub));
                        b(ui, RichText::new("x²"), "Superscript", here.sup, Act::Inline(IOp::Sup));
                        if structured && here.center_ok {
                            b(ui, RichText::new("Centre"), "Centre the paragraph (Ctrl+Shift+E)", here.center, Act::Center);
                        }
                        ui.add_space(3.0);
                        b(ui, RichText::new("code").family(FontFamily::Monospace), "Inline code (Ctrl+E)", here.code, Act::Code);
                        b(ui, RichText::new("Σ"), "Turn into an equation", false, Act::Equation);
                        b(ui, RichText::new("Link").underline(), "Link to a web or e-mail address (Ctrl+K)", here.link, Act::Link);
                        if blanks {
                            b(ui, RichText::new("___ Blank"), "Make the selected word a blank; it becomes the answer (Ctrl+Shift+K)", false, Act::Blank);
                        }
                        b(ui, RichText::new("Clear"), "Clear formatting (Ctrl+\\)", false, Act::Inline(IOp::Clear));
                    });
                });
        });
    (act, r.response.rect)
}

/// Is the selection `lo..hi` (local) a blank, or the caret right beside one?
fn blank_at(text: &str, la: usize, lb: usize) -> Option<Atom> {
    let ana = analyze(text, &|_| false);
    let (lo, hi) = (la.min(lb), la.max(lb));
    ana.atoms.into_iter().find(|a| matches!(a.kind, AtomKind::Blank(_)) && ((lo <= a.start && a.end <= hi && lo < hi) || (lo == hi && (lo == a.end || lo == a.start))))
}

fn here_blank(text: &str, la: usize, lb: usize) -> bool {
    blank_at(text, la, lb).is_some()
}

/// Put a blank's answer (its first alternative) back in place of the `___`.
fn run_unblank(stem: &str, d: &Doc, u: &Unit, text: &str, la: usize, lb: usize, rows: &Option<Vec<String>>) -> Option<Change> {
    let a = blank_at(text, la, lb)?;
    let g0 = d.global(u, 0);
    let k = blank_starts(stem).iter().filter(|&&s| s < g0 + a.start).count();
    let word = rows.as_ref().and_then(|r| r.get(k)).map(|r| r.split(" OR ").next().unwrap_or("").trim().to_string()).unwrap_or_default();
    let t = splice(text, a.start..a.end, &word);
    let s = apply_unit(stem, u, &t);
    let end = g0 + a.start + clen(&word);
    Some(Change { stem: s, anchor: g0 + a.start, caret: end, virt: u.virt, blank: None, keep: None })
}

/// Draw the editor for `stem` and apply what the user does to it.
pub fn editor(ui: &mut Ui, id: UiId, stem: &mut String, mut o: Opts) -> Out {
    let ctx = ui.ctx().clone();
    let mut m: Mem = ctx.data_mut(|d| d.get_temp::<Mem>(id)).unwrap_or_default();
    let mut out = Out::default();
    let st = Sty { size: o.size, color: theme::text_col(), family: theme::serif_family(), bold: theme::serif_bold_family(), blank_w: o.size * 4.5, center: false };
    let has_blanks = o.blanks.is_some();
    let unsafe_reason = check(stem).err();
    let top = ui.cursor().min;

    // ----- markup (and fallback) view -----
    if m.markup || unsafe_reason.is_some() {
        if o.toolbar
            && unsafe_reason.is_none()
            && toolbar(ui, Here::default(), has_blanks, true, o.structured) == Some(Act::Markup)
        {
            m.markup = false;
        }
        if let Some(why) = &unsafe_reason {
            ui.label(RichText::new(format!("Showing the markup: the visual editor can't show this text safely because {why}.")).size(11.5).color(theme::muted_col()));
            // Windows line endings are the usual cause: offer the one safe fix
            let only_cr = stem.chars().all(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'));
            if only_cr && stem.contains('\r') && ui.small_button("Fix line endings and edit visually").clicked() {
                let before = stem.clone();
                *stem = stem.replace("\r\n", "\n").replace('\r', "\n");
                if let Some(b) = o.blanks.as_deref_mut() {
                    *b = remap_blanks(&before, stem, b);
                }
                push_undo(&mut m, &before, 0, o.blanks.as_deref().map(|v| v.as_slice()), ui.input(|i| i.time), None, false);
                out.changed = true;
            }
        }
        let before = stem.clone();
        let r = TextEdit::multiline(stem)
            .id(id.with("wys_markup"))
            .frame(Frame::new())
            .font(FontId::new(o.size, theme::serif_family()))
            .desired_width(o.width)
            .desired_rows(if o.structured { 3 } else { 1 })
            .hint_text(o.hint.to_string())
            .show(ui);
        if *stem != before {
            if let Some(b) = o.blanks.as_deref_mut() {
                *b = remap_blanks(&before, stem, b);
            }
            push_undo(&mut m, &before, 0, o.blanks.as_deref().map(|v| v.as_slice()), ui.input(|i| i.time), Some(id), true);
            out.changed = true;
        }
        out.focused = r.response.response.has_focus();
        out.caret = r.cursor_range.map(|c| usize::from(c.primary.index));
        // how it prints, under the markup
        if !stem.trim().is_empty() && richtext::has_markup(stem) {
            ui.add_space(6.0);
            ui.label(RichText::new("Preview").size(11.0).color(theme::muted_col()));
            let o2 = crate::equation::Rich::new(o.size, theme::text_col());
            crate::equation::rich_ui(ui, &stem.replace('\r', ""), &o2);
        }
        out.rect = Rect::from_min_max(top, ui.min_rect().max);
        ctx.data_mut(|d| d.insert_temp(id, m));
        return out;
    }

    let mut d = doc(stem, o.structured);
    let mut ids = unit_ids(id, &d);
    // ----- focus wanted by the last edit -----
    if m.want.is_none()
        && let Some((w, n)) = m.retry.take()
        && n < 6
    {
        m.want = Some(w);
        m.retry = Some((m.want.clone().unwrap(), n + 1));
    }
    if let Some(w) = m.want.take()
        && let Some(fi) = d.find(w.caret, w.virt)
    {
        if m.retry.is_none() {
            m.retry = Some((w.clone(), 0));
        }
        let u = d.unit(d.flat()[fi]).clone();
        let off = |g: usize| match u.virt {
            Some(Virt::After) => g.saturating_sub(d.len + 1),
            Some(Virt::Before) => g,
            None => g.saturating_sub(u.start).min(u.end - u.start),
        };
        let (la, lc) = if d.find(w.anchor, w.virt) == Some(fi) { (off(w.anchor), off(w.caret)) } else { (off(w.caret), off(w.caret)) };
        let cur = local_sel(&ctx, ids[fi], usize::MAX);
        let has = ctx.memory(|mm| mm.has_focus(ids[fi]));
        if !has || cur != (la, lc) {
            set_cursor(&ctx, ids[fi], la, lc);
            ctx.memory_mut(|mm| mm.request_focus(ids[fi]));
        }
    }
    let mut focused = ids.iter().position(|u| ctx.memory(|mm| mm.has_focus(*u)));
    let now = ui.input(|i| i.time);

    // ----- what the toolbar reflects -----
    let here_of = |d: &Doc, stem: &str, fi: usize, sel: (usize, usize)| -> Here {
        let u = d.unit(d.flat()[fi]).clone();
        let text = slice(stem, u.start..u.end);
        let ana = analyze(&text, &math_ok_ui(&ctx, st.size));
        let lc = |g: usize| g.saturating_sub(u.start).min(text.chars().count());
        let Style { b, i, sub, sup, s } = ana.style_at(lc(sel.1));
        let (lo, hi) = (lc(sel.0.min(sel.1)), lc(sel.0.max(sel.1)));
        let on_blank = ana.atoms.iter().any(|a| matches!(a.kind, AtomKind::Blank(_)) && ((lo == a.start && hi == a.end) || (lo == hi && (lo == a.end || lo == a.start))));
        let link = link_spans(&text).iter().any(|&(s, _, e)| s < hi.max(lo + 1) && lo <= e);
        let code = code_spans(&text).iter().any(|&(s, e)| s < hi.max(lo + 1) && lo <= e);
        let center = is_centered(d, fi);
        let center_ok = center || (u.virt.is_none() && matches!(u.kind, UKind::Flow | UKind::Head { .. }));
        Here {
            b,
            i,
            sub,
            sup,
            s,
            list: match u.kind {
                UKind::Item { list, .. } => Some(list),
                _ => None,
            },
            table: matches!(u.kind, UKind::Cell { .. }),
            sel: lo < hi,
            on_blank,
            link,
            code,
            center,
            center_ok,
        }
    };

    // ----- keyboard (before any TextEdit sees it) -----
    let mut change: Option<Change> = None;
    let mut act: Option<Act> = None;
    if let Some(fi) = focused {
        let at = d.flat()[fi];
        let u = d.unit(at).clone();
        let uid = ids[fi];
        let text = slice(stem, u.start..u.end);
        let n = clen(&text);
        let (la, lc) = local_sel(&ctx, uid, n);
        let ana = analyze(&text, &math_ok_ui(&ctx, st.size));
        let cmd = Modifiers::COMMAND;
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        // slash menu keys
        if let Some(sl) = m.slash {
            let gc = d.global(&u, lc);
            match slash_query(stem, &d, fi, gc, &sl) {
                Some((q, wh)) if la == lc => {
                    let list = slash_list(&q, wh, sl.tab);
                    let n_items = list.len();
                    if n_items == 0 {
                        m.slash = None;
                    } else if take_key(ui, Modifiers::NONE, Key::ArrowDown) {
                        m.slash = Some(SlashSt { hi: (sl.hi + 1) % n_items, ..sl });
                    } else if take_key(ui, Modifiers::NONE, Key::ArrowUp) {
                        m.slash = Some(SlashSt { hi: (sl.hi + n_items - 1) % n_items, ..sl });
                    } else if take_key(ui, Modifiers::NONE, Key::Escape) {
                        m.slash = None;
                    } else if take_key(ui, Modifiers::NONE, Key::Enter) || take_key(ui, Modifiers::NONE, Key::Tab) {
                        let it = list[sl.hi.min(n_items - 1)];
                        m.slash = None;
                        if let Some(done) = slash_apply(stem, sl.at, gc, it.cmd, has_blanks) {
                            if let Some(g) = done.eq_at {
                                out.eq = Some(EqAsk { caret: g, span: None, initial: String::new() });
                            }
                            change = Some(done.change);
                        }
                    }
                }
                _ => m.slash = None,
            }
        }
        if change.is_some() {
            // a slash command ran: nothing else this frame
        } else if take_key(ui, cmd, Key::Z) {
            act = Some(Act::Undo);
        } else if take_key(ui, cs, Key::Z) || take_key(ui, cmd, Key::Y) {
            act = Some(Act::Redo);
        } else if take_key(ui, cmd, Key::B) {
            act = Some(Act::Inline(IOp::Bold));
        } else if take_key(ui, cmd, Key::I) {
            act = Some(Act::Inline(IOp::Italic));
        } else if take_key(ui, cs, Key::S) || take_key(ui, Modifiers::COMMAND | Modifiers::ALT, Key::X) {
            act = Some(Act::Inline(IOp::Strike));
        } else if o.structured && take_key(ui, cs, Key::E) {
            act = Some(Act::Center);
        } else if take_key(ui, cmd, Key::E) {
            act = Some(Act::Code);
        } else if o.structured && task_state(&text).is_some() && matches!(u.kind, UKind::Item { .. }) && take_key(ui, cmd, Key::Enter) {
            act = Some(Act::Task);
        } else if o.structured && matches!(u.kind, UKind::Item { list, .. } if list != ListKind::Quote) && take_key(ui, Modifiers::SHIFT, Key::Tab) {
            act = Some(Act::Indent(false));
        } else if o.structured && matches!(u.kind, UKind::Item { list, .. } if list != ListKind::Quote) && take_key(ui, Modifiers::NONE, Key::Tab) {
            act = Some(Act::Indent(true));
        } else if o.structured && u.virt.is_none() && take_key(ui, Modifiers::ALT, Key::ArrowUp) {
            act = Some(Act::Move(true));
        } else if o.structured && u.virt.is_none() && take_key(ui, Modifiers::ALT, Key::ArrowDown) {
            act = Some(Act::Move(false));
        } else if take_key(ui, cmd, Key::Comma) {
            act = Some(Act::Inline(IOp::Sub));
        } else if take_key(ui, cmd, Key::Period) {
            act = Some(Act::Inline(IOp::Sup));
        } else if take_key(ui, cmd, Key::Backslash) || take_key(ui, cmd, Key::Space) {
            act = Some(Act::Inline(IOp::Clear));
        } else if take_key(ui, cmd, Key::M) {
            act = Some(Act::Equation);
        } else if o.structured && (take_key(ui, cs, Key::Num8) || take_key(ui, Modifiers::COMMAND | Modifiers::ALT, Key::Num8)) {
            act = Some(Act::List(ListKind::Bullets));
        } else if o.structured && (take_key(ui, cs, Key::Num7) || take_key(ui, Modifiers::COMMAND | Modifiers::ALT, Key::Num7)) {
            act = Some(Act::List(ListKind::Numbered));
        } else if o.structured && (take_key(ui, cs, Key::B) || take_key(ui, cs, Key::Num9)) {
            act = Some(Act::List(ListKind::Quote));
        } else if take_key(ui, cs, Key::K) {
            // (always taken: egui would read Ctrl+K as "delete to the end of the paragraph")
            if has_blanks {
                act = Some(Act::Blank);
            }
        } else if take_key(ui, cmd, Key::K) {
            act = Some(Act::Link);
        }
        // Backspace / Delete remove one visible thing (an atom whole); at the edge of a block
        // they join blocks or leave a list.
        let single = la == lc;
        if act.is_none() && single && key_down(ui, Key::Backspace) && !ui.input(|i| i.modifiers.ctrl || i.modifiers.alt || i.modifiers.mac_cmd) {
            let mut q = lc;
            while q > 0 && ana.role[q - 1] == Role::Hidden {
                q -= 1;
            }
            if q == 0 || ana.spot(lc) == 0 {
                take_key(ui, Modifiers::NONE, Key::Backspace);
                if let Some(c) = backspace_op(stem, &d, fi) {
                    change = Some(c);
                } else if fi > 0 && matches!(u.kind, UKind::Cell { .. } | UKind::Flow) {
                    let pu = d.unit(d.flat()[fi - 1]).clone();
                    let g = d.global(&pu, pu.end - pu.start);
                    m.want = Some(Want { anchor: g, caret: g, virt: pu.virt });
                }
            } else if let Some(a) = ana.atoms.iter().find(|a| a.end == q) {
                set_cursor(&ctx, uid, a.start, a.end);
            } else if q != lc {
                set_cursor(&ctx, uid, q - 1, q);
            }
        }
        if act.is_none() && change.is_none() && single && key_down(ui, Key::Delete) && !ui.input(|i| i.modifiers.ctrl || i.modifiers.alt || i.modifiers.mac_cmd) {
            let mut q = lc;
            while q < n && ana.role[q] == Role::Hidden {
                q += 1;
            }
            if q >= n {
                take_key(ui, Modifiers::NONE, Key::Delete);
                change = delete_op(stem, &d, fi);
            } else if let Some(a) = ana.atoms.iter().find(|a| a.start == q) {
                set_cursor(&ctx, uid, a.start, a.end);
            } else if q != lc {
                set_cursor(&ctx, uid, q, q + 1);
            }
        }
        // Enter
        if act.is_none() && change.is_none() && (take_key(ui, Modifiers::NONE, Key::Enter) || take_key(ui, Modifiers::SHIFT, Key::Enter)) {
            match u.kind {
                UKind::Cell { .. } => {
                    // leave the table: a new paragraph line after it (or the one already there)
                    let last = d.blocks[at.0].lines.end - 1;
                    let end = d.line_end(last);
                    if last + 1 >= d.line_start.len() {
                        m.want = Some(Want { anchor: d.len + 1, caret: d.len + 1, virt: Some(Virt::After) });
                    } else {
                        change = Some(Change::at(splice(stem, end..end, "\n"), end + 1));
                    }
                }
                _ => change = enter_op(stem, &d, fi, la, lc, o.newlines),
            }
        }
        // Tab between cells
        if let UKind::Cell { row, col, .. } = u.kind {
            let back = take_key(ui, Modifiers::SHIFT, Key::Tab);
            let fwd = !back && take_key(ui, Modifiers::NONE, Key::Tab);
            if back || fwd {
                let bl = &d.blocks[at.0];
                let BlockKind::Table { ncols, .. } = bl.kind else { unreachable!() };
                let k = row * ncols + col;
                if fwd && k + 1 >= bl.units.len() {
                    change = table_op(stem, &d, at.0, row, col, TOp::RowBelow).map(|mut c| {
                        // land in the first cell of the new row
                        let nd = doc(&c.stem, true);
                        if let Some(nb) = nd.blocks.iter().find(|b| b.lines.start == bl.lines.start)
                            && let Some(nu) = nb.units.get((row + 1) * ncols)
                        {
                            c.anchor = nu.start;
                            c.caret = nu.end;
                        }
                        c
                    });
                } else {
                    let nk = if fwd { k + 1 } else { k.saturating_sub(1) };
                    let nu = &bl.units[nk];
                    m.want = Some(Want { anchor: nu.start, caret: nu.end, virt: None });
                }
            }
        }
        // Arrows across unit edges
        if act.is_none() && change.is_none() && m.want.is_none() && single && !ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.alt) {
            let flat = d.flat();
            let go = |k: usize, end: bool| -> Want {
                let nu = d.unit(flat[k]);
                let g = if end { d.global(nu, nu.end - nu.start) } else { d.global(nu, 0) };
                Want { anchor: g, caret: g, virt: nu.virt }
            };
            let first = ana.spot(lc) == 0;
            let last = ana.spot(lc) == ana.spot(n);
            if fi > 0 && ((key_down(ui, Key::ArrowLeft) && first) || (key_down(ui, Key::ArrowUp) && m.rows.0)) {
                take_key(ui, Modifiers::NONE, Key::ArrowLeft);
                take_key(ui, Modifiers::NONE, Key::ArrowUp);
                m.want = Some(go(fi - 1, true));
            } else if fi + 1 < flat.len() && ((key_down(ui, Key::ArrowRight) && last) || (key_down(ui, Key::ArrowDown) && m.rows.1)) {
                take_key(ui, Modifiers::NONE, Key::ArrowRight);
                take_key(ui, Modifiers::NONE, Key::ArrowDown);
                m.want = Some(go(fi + 1, false));
            }
        }
        // Typed / pasted text: cells keep to one line without pipes; list items continue the list.
        let prefix_next: Option<String> = match u.kind {
            UKind::Item { list, line } => {
                let l = stem.split('\n').nth(line).unwrap_or("");
                let p: String = l.chars().take(richtext::item_prefix_len(list.seg(), l)).collect();
                Some(if list == ListKind::Numbered { numbered_prefix(l).map_or(p.clone(), |(ws, k, dl, _)| format!("{ws}{}{dl} ", k + 1)) } else { p })
            }
            _ => None,
        };
        let is_cell = matches!(u.kind, UKind::Cell { .. });
        let structured = o.structured;
        let mut slash = false;
        ui.input_mut(|i| {
            for e in &mut i.events {
                match e {
                    Event::Paste(s) => {
                        let mut t = s.replace("\r\n", "\n").replace('\r', "\n").replace('\t', "    ");
                        if is_cell {
                            t = t.replace('\n', " ").replace('|', "∣");
                        } else if let Some(p) = &prefix_next {
                            t = t.replace('\n', &format!("\n{p}"));
                        } else if !structured && !o.newlines {
                            t = t.replace('\n', " ");
                        }
                        *s = t;
                    }
                    Event::Text(s) if is_cell && s.contains('|') => *s = s.replace('|', "∣"),
                    _ => {}
                }
            }
            // '/' at the start of a word opens the slash menu (stems) or the equation editor
            // (one-line fields); mid-word it stays a literal '/'
            if let Some(p) = i.events.iter().position(|e| matches!(e, Event::Text(s) if s == "/")) {
                let chs: Vec<char> = text.chars().collect();
                if single && (lc == 0 || chs.get(lc - 1).is_some_and(|c| c.is_whitespace())) {
                    if !structured {
                        i.events.remove(p);
                    }
                    slash = true;
                }
            }
        });
        if slash && change.is_none() {
            if o.structured {
                m.slash = Some(SlashSt { at: d.global(&u, lc), hi: 0, tab: None });
            } else {
                out.eq = Some(EqAsk { caret: d.global(&u, lc), span: None, initial: String::new() });
            }
        }
    }

    // ----- apply a keyboard edit now so this frame draws it -----
    let mut blanks_rows = o.blanks.as_deref().map(|v| v.to_vec());
    let commit = |m: &mut Mem, stem: &mut String, c: Change, typing_unit: Option<UiId>, blanks_rows: &mut Option<Vec<String>>, out: &mut Out| {
        if c.stem == *stem && c.blank.is_none() {
            m.want = Some(Want { anchor: c.anchor, caret: c.caret, virt: c.virt });
            return;
        }
        let caret0 = m.sel.map_or(0, |s| s.1);
        push_undo(m, stem, caret0, blanks_rows.as_deref(), now, typing_unit, typing_unit.is_some());
        if let Some(rows) = blanks_rows.as_mut() {
            *rows = match &c.keep {
                Some(k) => k.iter().map(|o| o.and_then(|i| rows.get(i).cloned()).unwrap_or_default()).collect(),
                None => remap_blanks(stem, &c.stem, rows),
            };
            if let Some((pos, word)) = &c.blank {
                let k = blank_starts(&c.stem).iter().filter(|&&s| s < *pos).count();
                if let Some(r) = rows.get_mut(k) {
                    *r = word.clone();
                }
            }
        }
        *stem = c.stem;
        m.want = Some(Want { anchor: c.anchor, caret: c.caret, virt: c.virt });
        out.changed = true;
    };
    let run_act = |a: Act, m: &mut Mem, stem: &mut String, d: &Doc, blanks_rows: &mut Option<Vec<String>>, out: &mut Out| -> Option<Change> {
        let sel = m.sel?;
        let fi = m.sel_unit.filter(|&k| k < d.flat().len()).or_else(|| d.find(sel.1, None))?;
        let at = d.flat()[fi];
        let u = d.unit(at).clone();
        let text = slice(stem, u.start..u.end);
        let lc = |g: usize| g.saturating_sub(u.start).min(clen(&text));
        let (la, lb) = (lc(sel.0), lc(sel.1));
        match a {
            Act::Inline(op) if la == lb => {
                // collapsed caret: toggle typing mode, never select or restyle the adjacent word
                if u.virt.is_some() {
                    return None;
                }
                let g = u.start + lb;
                let base = match m.pending {
                    Some((p, st)) if p == g => st,
                    _ => analyze(&text, &|_| false).style_at(lb),
                };
                m.pending = Some((g, base.toggled(op)));
                Some(Change::at(stem.clone(), g))
            }
            Act::Inline(op) => {
                let (t, x, y) = inline_op(&text, la, lb, op)?;
                let s = apply_unit(stem, &u, &t);
                let g0 = if u.virt.is_some() { d.global(&u, 0) } else { u.start };
                Some(Change { stem: s, anchor: g0 + x, caret: g0 + y, virt: u.virt, blank: None, keep: None })
            }
            Act::List(k) => toggle_list(stem, d, fi, sel, k),
            Act::Table => Some(insert_table(stem, d, fi, sel.1, blanks_rows.is_some())),
            Act::Equation => {
                let (lo, hi) = (la.min(lb), la.max(lb));
                let g = |l: usize| d.global(&u, l);
                out.eq = Some(if lo < hi {
                    let words = slice(&text, lo..hi);
                    let plain: String = decode_line(&words.replace('\n', " ")).0.iter().map(|it| match it {
                        Item::Ch { c, .. } => c.to_string(),
                        Item::Atom(r) if r.starts_with('[') => richtext::line_text(&richtext::inline(r)),
                        Item::Atom(r) if r.starts_with('`') => r.trim_matches('`').to_string(),
                        Item::Atom(r) => r.trim_matches('$').to_string(),
                    }).collect();
                    EqAsk { caret: g(lo), span: Some(g(lo)..g(hi)), initial: plain }
                } else {
                    EqAsk { caret: g(lb), span: None, initial: String::new() }
                });
                None
            }
            Act::Blank if here_blank(&text, la, lb) => run_unblank(stem, d, &u, &text, la, lb, blanks_rows),
            Act::Blank => {
                let (t, caret, bs, word) = make_blank(&text, la, lb)?;
                let s = apply_unit(stem, &u, &t);
                let g0 = if u.virt.is_some() { d.global(&u, 0) } else { u.start };
                Some(Change { stem: s, anchor: g0 + caret, caret: g0 + caret, virt: u.virt, blank: Some((g0 + bs, word)), keep: None })
            }
            Act::TableOp(op) => {
                let (bi, r, c) = m.cell?;
                table_op(stem, d, bi, r, c, op)
            }
            Act::Link => {
                let (lo, hi) = (la.min(lb), la.max(lb));
                let spans = link_spans(&text);
                let cur = spans.iter().find(|&&(s, _, e)| s < hi.max(lo + 1) && lo <= e);
                let ch: Vec<char> = text.chars().collect();
                let url = cur.map(|&(_, close, e)| ch[close + 2..e].iter().collect()).unwrap_or_default();
                m.link = Some(LinkSt { sel, unit: fi, url, existing: cur.is_some(), focused: false, at: m.caret_rect.map(|r| r.left_bottom()), err: false });
                None
            }
            Act::Code => {
                let (t, x, y) = code_op(&text, la, lb)?;
                let s = apply_unit(stem, &u, &t);
                let g0 = if u.virt.is_some() { d.global(&u, 0) } else { u.start };
                Some(Change { stem: s, anchor: g0 + x, caret: g0 + y, virt: u.virt, blank: None, keep: None })
            }
            Act::Center => center_op(stem, d, fi, sel),
            // (Tab where an item cannot go deeper still stays in the item)
            Act::Indent(deeper) => indent_op(stem, d, fi, sel, deeper).or_else(|| Some(Change { stem: stem.clone(), anchor: sel.0, caret: sel.1, virt: None, blank: None, keep: None })),
            Act::Task => toggle_task(stem, d, fi, sel),
            Act::Move(up) => {
                let line = d.line_of(sel.1);
                let k = rail_block_of(stem, line)?;
                let start = d.line_start[rail_blocks(stem)[k].1.start];
                let mut c = move_block(stem, k, if up { k.checked_sub(1)? } else { k + 2 })?;
                let base = c.caret;
                c.anchor = base + sel.0.saturating_sub(start);
                c.caret = base + sel.1.saturating_sub(start);
                Some(c)
            }
            Act::LinkApply | Act::LinkRemove => None,
            Act::Undo | Act::Redo | Act::Markup => None,
        }
    };
    let do_undo = |redo: bool, m: &mut Mem, stem: &mut String, blanks_rows: &mut Option<Vec<String>>, out: &mut Out| {
        let (from, to) = if redo { (&mut m.redo, &mut m.undo) } else { (&mut m.undo, &mut m.redo) };
        if let Some(s) = from.pop() {
            to.push(Snap { stem: stem.clone(), caret: m.sel.map_or(0, |x| x.1), blanks: blanks_rows.clone() });
            *stem = s.stem;
            if let (Some(r), Some(b)) = (blanks_rows.as_mut(), s.blanks) {
                *r = b;
            }
            m.want = Some(Want { anchor: s.caret.min(clen(stem)), caret: s.caret.min(clen(stem)), virt: None });
            m.last_unit = None;
            out.changed = true;
        }
    };
    if let Some(fi) = focused {
        // remember the selection for toolbar clicks
        let u = d.unit(d.flat()[fi]).clone();
        let n = u.end - u.start;
        let (la, lc) = local_sel(&ctx, ids[fi], if u.virt.is_some() { 0 } else { n });
        m.sel = Some((d.global(&u, la), d.global(&u, lc)));
        m.sel_unit = Some(fi);
        if m.pending.is_some_and(|(p, _)| u.virt.is_some() || la != lc || d.global(&u, lc) != p) {
            m.pending = None;
        }
        m.cell = match u.kind {
            UKind::Cell { row, col, .. } => Some((d.flat()[fi].0, row, col)),
            _ => None,
        };
    }
    if let Some(a) = act.take() {
        match a {
            Act::Undo => do_undo(false, &mut m, stem, &mut blanks_rows, &mut out),
            Act::Redo => do_undo(true, &mut m, stem, &mut blanks_rows, &mut out),
            a => change = change.or_else(|| run_act(a, &mut m, stem, &d, &mut blanks_rows, &mut out)),
        }
    }
    if let Some(c) = change.take() {
        commit(&mut m, stem, c, None, &mut blanks_rows, &mut out);
    }
    if out.changed || m.want.is_some() {
        d = doc(stem, o.structured);
        ids = unit_ids(id, &d);
        if let Some(w) = m.want.take()
            && let Some(fi) = d.find(w.caret, w.virt)
        {
            let u = d.unit(d.flat()[fi]).clone();
            let off = |g: usize| match u.virt {
                Some(Virt::After) => g.saturating_sub(d.len + 1),
                Some(Virt::Before) => g,
                None => g.saturating_sub(u.start).min(u.end - u.start),
            };
            let la = if d.find(w.anchor, w.virt) == Some(fi) { off(w.anchor) } else { off(w.caret) };
            set_cursor(&ctx, ids[fi], la, off(w.caret));
            ctx.memory_mut(|mm| mm.request_focus(ids[fi]));
            focused = Some(fi);
            m.sel = Some((w.anchor, w.caret));
            m.sel_unit = Some(fi);
        }
        ctx.request_repaint();
    }

    // ----- fixed toolbar -----
    let mut here = match (focused.or(m.sel_unit.filter(|&k| k < ids.len())), m.sel) {
        (Some(fi), Some(sel)) => here_of(&d, stem, fi, sel),
        _ => Here::default(),
    };
    if let (Some((p, st)), Some(sel)) = (m.pending, m.sel)
        && sel == (p, p)
    {
        (here.b, here.i, here.sub, here.sup, here.s) = (st.b, st.i, st.sub, st.sup, st.s);
    }
    let mut bar_act = None;
    let mut fixed_bar: Option<Rect> = None;
    if o.toolbar {
        let y0 = ui.cursor().top();
        bar_act = toolbar(ui, here, has_blanks, false, o.structured);
        fixed_bar = Some(Rect::from_min_max(Pos2::new(top.x, y0), Pos2::new(top.x + o.width, ui.cursor().top())));
        ui.add_space(4.0);
    }

    // ----- the blocks -----
    let bstarts = blank_starts(stem);
    let blank_base = |g: usize| bstarts.iter().filter(|&&s| s < g).count();
    let mut edit: Option<(usize, String)> = None;
    let mut clicked: Option<(usize, Atom)> = None;
    let mut sel_rect: Option<Rect> = None;
    // the block rail ("+" and the drag grip) lives in a gutter at the left of a stem
    let rail = o.structured && o.toolbar;
    let rail_w = if rail { RAIL_W } else { 0.0 };
    let width = o.width - rail_w;
    let empty_stem = stem.is_empty();
    let (asc, _l) = metrics(ui, &FontId::new(st.size, st.family.clone()));
    let active = focused.is_some() || m.active;
    let mut table_bar: Option<(usize, Act)> = None;
    let flat = d.flat();
    let rows_ro = blanks_rows.clone();
    let mut div_act: Option<usize> = None;
    let mut side_act: Option<(usize, SOp)> = None;
    let mut task_click: Option<usize> = None;
    // screen top / bottom of each source line drawn (for the block rail)
    let line_ys: std::cell::RefCell<std::collections::HashMap<usize, (f32, f32)>> = Default::default();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.add_space(rail_w);
        ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
        let mut draw = |ui: &mut Ui, bi: usize, width: f32| {
            let bl = &d.blocks[bi];
            let fidx = |k: usize| flat.iter().position(|&f| f == (bi, k)).unwrap_or(0);
            match &bl.kind {
                BlockKind::Flow => {
                    let u = &bl.units[0];
                    let k = fidx(0);
                    let uf = Some(k) == focused;
                    if u.virt.is_some() && !(active || uf) {
                        // keep the room of the "add text above" line, so the first click (which
                        // shows it) does not move the text under the pointer
                        if u.virt == Some(Virt::Before) {
                            ui.add_space(metrics(ui, &FontId::new(st.size, st.family.clone())).1);
                        }
                        return;
                    }
                    let mut t = slice(stem, u.start..u.end);
                    let hint = match u.virt {
                        _ if empty_stem => o.hint,
                        Some(Virt::Before) => "(type here to add text above)",
                        Some(Virt::After) => "(type here to add text below)",
                        None => "",
                    };
                    let cst = Sty { center: bl.center.is_some(), ..st.clone() };
                    let sh = show_unit(ui, ids[k], &mut t, width, &cst, hint, blank_base(u.start), rows_ro.as_deref(), false);
                    // where each source line starts on screen (the rail lines up with them)
                    for l in bl.lines.clone() {
                        let la = d.line_start[l].saturating_sub(u.start);
                        let lb = d.line_end(l).saturating_sub(u.start);
                        let r0 = sh.out.galley.pos_from_cursor(CCursor::new(CharIndex::from(la))).translate(sh.origin.to_vec2());
                        let r1 = sh.out.galley.pos_from_cursor(CCursor::new(CharIndex::from(lb))).translate(sh.origin.to_vec2());
                        line_ys.borrow_mut().insert(l, (r0.top(), r1.bottom()));
                    }
                    after_unit(&ctx, &mut m, k, ids[k], &sh, &t, stem, u, &mut edit, &mut clicked, &mut sel_rect);
                }
                BlockKind::List(kind) => {
                    let n = bl.units.len();
                    let depths = if *kind == ListKind::Quote { vec![0; n] } else { block_depths(stem, bl) };
                    let ords = richtext::item_ordinals(&depths);
                    let step = st.size * 1.3;
                    let label_w = |j: usize| -> f32 {
                        let lab = richtext::number_label(depths[j], ords[j]);
                        st.size * (0.55 + 0.55 * lab.chars().count() as f32)
                    };
                    let indent0 = match kind {
                        ListKind::Bullets => st.size * 1.3,
                        ListKind::Numbered => (0..n).map(label_w).fold(st.size * 1.1, f32::max),
                        ListKind::Quote => st.size * 1.1,
                    };
                    let top_y = ui.cursor().top();
                    for (j, u) in bl.units.iter().enumerate() {
                        let k = fidx(j);
                        let mut t = slice(stem, u.start..u.end);
                        let indent = indent0 + depths[j] as f32 * step;
                        let r = ui.horizontal_top(|ui| {
                            ui.add_space(indent);
                            // (Tab nests a bullet or numbered item, so egui must not move focus on it)
                            show_unit(ui, ids[k], &mut t, width - indent, &st, "", blank_base(u.start), rows_ro.as_deref(), *kind != ListKind::Quote)
                        });
                        let sh = r.inner;
                        let rr = sh.out.response.response.rect;
                        line_ys.borrow_mut().insert(bl.lines.start + j, (rr.top(), rr.bottom()));
                        let base_y = rr.top() + asc;
                        // the marker column of this item (its level's indent included)
                        let left = rr.left() - indent0;
                        let task = task_state(&t);
                        match kind {
                            ListKind::Bullets if task.is_some() => {
                                // the box is the item's first char: a click on it ticks / unticks
                                let g0 = sh.out.galley.pos_from_cursor(CCursor::new(CharIndex::from(0usize))).translate(sh.origin.to_vec2());
                                let g1 = sh.out.galley.pos_from_cursor(CCursor::new(CharIndex::from(1usize))).translate(sh.origin.to_vec2());
                                let br = Rect::from_min_max(Pos2::new(g0.left() - 2.0, g0.top()), Pos2::new(g1.left() + 2.0, g0.bottom()));
                                let resp = &sh.out.response.response;
                                if resp.hover_pos().is_some_and(|p| br.contains(p)) {
                                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| br.contains(p)) {
                                    task_click = Some(k);
                                }
                            }
                            ListKind::Bullets => {
                                let c = Pos2::new(left + st.size * 0.5, base_y - st.size * 0.3);
                                match depths[j] % 3 {
                                    0 => {
                                        ui.painter().circle_filled(c, st.size * 0.14, st.color);
                                    }
                                    1 => {
                                        ui.painter().circle_stroke(c, st.size * 0.13, Stroke::new(1.0, st.color));
                                    }
                                    _ => {
                                        ui.painter().rect_filled(Rect::from_center_size(c, Vec2::splat(st.size * 0.22)), 0.0, st.color);
                                    }
                                }
                            }
                            ListKind::Numbered => {
                                let g = ui.painter().layout_no_wrap(richtext::number_label(depths[j], ords[j]), FontId::new(st.size, st.family.clone()), st.color);
                                ui.painter().galley(Pos2::new(rr.left() - st.size * 0.35 - g.size().x, rr.top()), g, st.color);
                            }
                            ListKind::Quote => {}
                        }
                        after_unit(&ctx, &mut m, k, ids[k], &sh, &t, stem, u, &mut edit, &mut clicked, &mut sel_rect);
                        if j + 1 < n {
                            ui.add_space(st.size * 0.2);
                        }
                    }
                    if *kind == ListKind::Quote {
                        let x = ui.min_rect().left() + st.size * 0.35;
                        ui.painter().line_segment([Pos2::new(x, top_y), Pos2::new(x, ui.cursor().top())], Stroke::new(st.size * 0.16, st.color.gamma_multiply(0.35)));
                    }
                }
                BlockKind::Table { header, ncols, sep } => {
                    let ncols = (*ncols).max(1);
                    let rows = bl.units.len() / ncols;
                    let in_table = focused.is_some_and(|f| flat[f].0 == bi) || m.cell.is_some_and(|c| c.0 == bi) && active;
                    // the table's control row: always there (focusing a cell never shifts the
                    // layout) and in the flow, so it never covers the text above or the next row
                    let bar_top = ui.cursor().top();
                    ui.allocate_ui_with_layout(Vec2::new(width, TABLE_BAR_H), egui::Layout::left_to_right(Align::Center), |ui| {
                        ui.set_height(TABLE_BAR_H);
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                        if !in_table {
                            ui.label(RichText::new("Table · click a cell for rows and columns").size(11.0).color(theme::muted_col()));
                            return;
                        }
                        let ok_header = header_toggle_ok(stem, &d, bi);
                        let mut tb = |ui: &mut Ui, l: &str, tip: &str, on: bool, en: bool, op: TOp| {
                            let b = egui::Button::new(RichText::new(l).size(11.0).color(if on { theme::accent_col() } else { theme::text_col() }))
                                .fill(if on { theme::accent_faded() } else { Color32::TRANSPARENT })
                                .stroke(Stroke::new(1.0, if on { theme::accent_col().gamma_multiply(0.5) } else { theme::border() }))
                                .corner_radius(CornerRadius::same(6))
                                .min_size(Vec2::new(0.0, 20.0));
                            if ui.add_enabled(en, b).on_hover_text(tip).clicked() {
                                table_bar = Some((bi, Act::TableOp(op)));
                            }
                        };
                        ui.spacing_mut().button_padding = Vec2::new(5.0, 1.0);
                        tb(ui, "+Row↑", "Insert a row above", false, true, TOp::RowAbove);
                        tb(ui, "+Row↓", "Insert a row below (also Tab in the last cell)", false, true, TOp::RowBelow);
                        tb(ui, "+Col←", "Insert a column to the left", false, true, TOp::ColLeft);
                        tb(ui, "+Col→", "Insert a column to the right", false, true, TOp::ColRight);
                        tb(ui, "−Row", "Delete this row", false, rows > 2, TOp::DelRow);
                        tb(ui, "−Col", "Delete this column", false, ncols > 2, TOp::DelCol);
                        tb(ui, "Header", if ok_header { "First row is a header" } else { "The blanks below make the first row a header" }, *header, ok_header, TOp::Header);
                        tb(ui, "Delete", "Remove the whole table", false, true, TOp::DelTable);
                    });
                    let _ = sep;
                    // column widths from the cells' natural widths
                    let hpad = st.size * 0.45;
                    let vpad = st.size * 0.25;
                    let mut nat = vec![st.size * 3.0; ncols];
                    for (j, u) in bl.units.iter().enumerate() {
                        let t = slice(stem, u.start..u.end);
                        let a = analyze(&t, &math_ok_ui(&ctx, st.size));
                        let job = build_job(ui, &t, f32::INFINITY, &st, &a, &|n| {
                            let rows = rows_ro.as_deref()?;
                            Some(rows.get(blank_base(u.start) + n).cloned().unwrap_or_default().split(" OR ").next().unwrap_or("").to_string() + "  Blank 0")
                        });
                        let w = ui.fonts_mut(|f| f.layout_job(job)).size().x;
                        nat[j % ncols] = nat[j % ncols].max(w + 2.0 * hpad + 4.0);
                    }
                    let widths = crate::equation::table_widths(&nat, width - 1.0);
                    let tw: f32 = widths.iter().sum();
                    let x0 = ui.cursor().left();
                    let y0 = ui.cursor().top();
                    let mut ys = vec![y0];
                    let header_bg = ui.painter().add(Shape::Noop);
                    for r in 0..rows {
                        let row_top = ui.cursor().top();
                        let resp = ui.horizontal_top(|ui| {
                            for c in 0..ncols {
                                let j = r * ncols + c;
                                let u = &bl.units[j];
                                let k = fidx(j);
                                let mut t = slice(stem, u.start..u.end);
                                let cst = if *header && r == 0 { Sty { family: st.bold.clone(), ..st.clone() } } else { st.clone() };
                                ui.allocate_ui_with_layout(Vec2::new(widths[c], 0.0), egui::Layout::top_down(Align::Min), |ui| {
                                    ui.set_width(widths[c]);
                                    ui.add_space(vpad);
                                    let sh = ui
                                        .horizontal_top(|ui| {
                                            ui.add_space(hpad);
                                            show_unit(ui, ids[k], &mut t, widths[c] - 2.0 * hpad, &cst, "", blank_base(u.start), rows_ro.as_deref(), true)
                                        })
                                        .inner;
                                    ui.add_space(vpad);
                                    after_unit(&ctx, &mut m, k, ids[k], &sh, &t, stem, u, &mut edit, &mut clicked, &mut sel_rect);
                                });
                            }
                        });
                        let _ = row_top;
                        ys.push(resp.response.rect.bottom());
                    }
                    let stroke = Stroke::new(1.0, st.color.gamma_multiply(0.45));
                    let p = ui.painter();
                    let bottom = *ys.last().unwrap();
                    if *header && ys.len() > 1 {
                        p.set(header_bg, Shape::rect_filled(Rect::from_min_max(Pos2::new(x0, ys[0]), Pos2::new(x0 + tw, ys[1])), 0.0, theme::accent_faded().gamma_multiply(0.45)));
                    }
                    for y in &ys {
                        p.hline(x0..=x0 + tw, *y, stroke);
                    }
                    for l in bl.lines.clone() {
                        line_ys.borrow_mut().insert(l, (bar_top, bottom));
                    }
                    let mut x = x0;
                    p.vline(x, y0..=bottom, stroke);
                    for w in &widths {
                        x += w;
                        p.vline(x, y0..=bottom, stroke);
                    }
                }
                BlockKind::Heading(level) => {
                    let u = &bl.units[0];
                    let k = fidx(0);
                    let mut t = slice(stem, u.start..u.end);
                    let hs = Sty { size: st.size * crate::equation::heading_scale(*level), family: st.bold.clone(), center: bl.center.is_some(), ..st.clone() };
                    ui.add_space(st.size * 0.1);
                    let sh = show_unit(ui, ids[k], &mut t, width, &hs, "Heading", blank_base(u.start), rows_ro.as_deref(), false);
                    let rr = sh.out.response.response.rect;
                    line_ys.borrow_mut().insert(bl.lines.start, (rr.top(), rr.bottom()));
                    after_unit(&ctx, &mut m, k, ids[k], &sh, &t, stem, u, &mut edit, &mut clicked, &mut sel_rect);
                }
                BlockKind::Divider => {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(width, st.size * 1.0), egui::Sense::hover());
                    line_ys.borrow_mut().insert(bl.lines.start, (r.top(), r.bottom()));
                    let y = r.center().y.round() + 0.5;
                    let hot = resp.hovered() || (active && ui.rect_contains_pointer(r.expand(4.0)));
                    ui.painter().hline(r.left()..=r.right() - if hot { st.size * 1.4 } else { 0.0 }, y, Stroke::new(1.0, st.color.gamma_multiply(if hot { 0.8 } else { 0.5 })));
                    if hot {
                        let br = Rect::from_center_size(Pos2::new(r.right() - st.size * 0.6, y), Vec2::splat(st.size * 1.0));
                        let b = ui.put(br, egui::Button::new(RichText::new("×").size(st.size * 0.8)).frame(false)).on_hover_text("Remove this divider");
                        if b.clicked() {
                            div_act = Some(bl.lines.start);
                        }
                    }
                }
                BlockKind::Side { .. } => {}
            }
        };
        let mut first_top = true;
        for (bi, bl) in d.blocks.iter().enumerate() {
            if bl.panel.is_some() {
                continue;
            }
            if !first_top {
                ui.add_space(st.size * 0.35);
            }
            first_top = false;
            let BlockKind::Side { panels } = &bl.kind else {
                draw(ui, bi, width);
                continue;
            };
            // side by side: one column per panel, the panels' own blocks inside
            let group_top = ui.cursor().top();
            let n = panels.len().max(1);
            let gap = st.size * 1.0;
            let pad = 6.0;
            let colw = ((width - gap * (n as f32 - 1.0)) / n as f32).max(st.size * 3.0);
            let in_group = focused.is_some_and(|f| d.blocks[flat[f].0].panel.is_some_and(|(s, _)| s == bi));
            // a slim control row (always there, so focusing a panel never shifts the layout)
            ui.allocate_ui_with_layout(Vec2::new(width, 18.0), egui::Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let small = |t: &str| egui::Button::new(RichText::new(t).size(11.0).color(if in_group { theme::accent_col() } else { theme::muted_col() })).frame(false);
                if ui.add(small("Stack")).on_hover_text("Put the panels under each other (ends side by side)").clicked() {
                    side_act = Some((bi, SOp::Stack));
                }
                if n < 4 && ui.add(small("+ Panel")).on_hover_text("Add a panel at the right").clicked() {
                    side_act = Some((bi, SOp::AddPanel));
                }
                ui.label(RichText::new("Side by side").size(11.0).color(theme::muted_col()));
            });
            let top_y = ui.cursor().top();
            let x0 = ui.cursor().left();
            let mut boxes: Vec<Rect> = Vec::new();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for pi in 0..n {
                    if pi > 0 {
                        ui.add_space(gap);
                    }
                    let r = ui.allocate_ui_with_layout(Vec2::new(colw, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                        ui.set_width(colw);
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.add_space(pad);
                        let kids: Vec<usize> = d.blocks.iter().enumerate().filter(|(_, b)| b.panel == Some((bi, pi))).map(|(k, _)| k).collect();
                        if kids.is_empty() {
                            let b = ui.add(egui::Button::new(RichText::new("(empty: click to write)").size(st.size * 0.8).color(theme::muted_col())).frame(false));
                            if b.clicked() {
                                side_act = Some((bi, SOp::Fill(pi)));
                            }
                        }
                        for (j, &k) in kids.iter().enumerate() {
                            if j > 0 {
                                ui.add_space(st.size * 0.35);
                            }
                            ui.horizontal_top(|ui| {
                                ui.add_space(pad);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    draw(ui, k, colw - 2.0 * pad);
                                });
                            });
                        }
                        ui.add_space(pad);
                    });
                    boxes.push(r.response.rect);
                }
            });
            let bottom = boxes.iter().map(|r| r.bottom()).fold(top_y, f32::max);
            for l in bl.lines.clone() {
                line_ys.borrow_mut().insert(l, (group_top, bottom));
            }
            let p = ui.painter().clone();
            for (pi, r) in boxes.iter().enumerate() {
                let r = Rect::from_min_max(Pos2::new(r.left(), top_y), Pos2::new(r.right(), bottom));
                let stroke = Stroke::new(1.0, theme::border().gamma_multiply(if in_group { 1.4 } else { 1.0 }));
                let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                for w in pts.windows(2) {
                    p.extend(Shape::dashed_line(&[w[0], w[1]], stroke, 4.0, 3.0));
                }
                if in_group {
                    let br = Rect::from_min_size(Pos2::new(r.right() - 20.0, r.top() + 2.0), Vec2::splat(18.0));
                    if ui.put(br, egui::Button::new(RichText::new("×").size(12.0)).frame(false)).on_hover_text("Remove this panel").clicked() {
                        side_act = Some((bi, SOp::RemovePanel(pi)));
                    }
                }
            }
            let _ = x0;
        }
        });
    });
    let structural = match (div_act, side_act) {
        _ if task_click.is_some() => task_click.and_then(|k| toggle_task(stem, &d, k, m.sel.unwrap_or((0, 0)))),
        (Some(l), _) => Some(delete_line(stem, l)),
        (None, Some((si, op))) => side_op(stem, &d, si, op),
        _ => None,
    };
    if let Some(c) = structural {
        // the structure changed under this frame's units: drop their edits
        bar_act = None;
        table_bar = None;
        edit = None;
        clicked = None;
        sel_rect = None;
        commit(&mut m, stem, c, None, &mut blanks_rows, &mut out);
    }
    if let Some((_, a)) = table_bar {
        bar_act = Some(a);
    }

    // ----- slash menu -----
    let over_slash = m.slash_rect.is_some_and(|r| ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()).is_some_and(|p| r.expand(4.0).contains(p))));
    if focused.is_none() && !over_slash {
        m.slash = None;
    }
    let mut slash_pick: Option<Slash> = None;
    m.slash_rect = None;
    if let (Some(sl), Some(sel), Some(fi), Some(cr)) = (m.slash, m.sel, m.sel_unit.filter(|&k| k < d.flat().len()), m.caret_rect)
        && let Some((q, wh)) = slash_query(stem, &d, fi, sel.1, &sl)
    {
        let all = slash_matches(&q, wh);
        let list = slash_list(&q, wh, sl.tab);
        let hi = sl.hi.min(list.len().saturating_sub(1));
        let mut new_tab = sl.tab;
        let mut new_hi = hi;
        let screen = ui.ctx().content_rect();
        let below = cr.bottom() + 4.0 + 300.0 < screen.bottom();
        let (pos, pivot) = if below { (Pos2::new(cr.left(), cr.bottom() + 4.0), egui::Align2::LEFT_TOP) } else { (Pos2::new(cr.left(), cr.top() - 4.0), egui::Align2::LEFT_BOTTOM) };
        let r = egui::Area::new(id.with("wys_slash"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .pivot(pivot)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                Frame::new()
                    .fill(theme::card())
                    .stroke(Stroke::new(1.0, theme::border()))
                    .corner_radius(CornerRadius::same(10))
                    .shadow(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(40) })
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(230.0);
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                        // group tabs (only groups that have matches)
                        ui.horizontal(|ui| {
                            for (g, name) in SLASH_GROUPS.iter().enumerate() {
                                let g = g as u8;
                                if !all.iter().any(|it| it.group == g) {
                                    continue;
                                }
                                let on = sl.tab == Some(g);
                                if tool_btn(ui, RichText::new(*name).size(12.0), "Show only this group (click again for all)", on, true) {
                                    new_tab = if on { None } else { Some(g) };
                                    new_hi = 0;
                                }
                            }
                        });
                        ui.separator();
                        if list.is_empty() {
                            ui.label(RichText::new("No match").size(12.0).color(theme::muted_col()));
                        }
                        let mut last_group = None;
                        egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                            for (k, it) in list.iter().enumerate() {
                                if last_group != Some(it.group) {
                                    last_group = Some(it.group);
                                    ui.add_space(2.0);
                                    ui.label(RichText::new(SLASH_GROUPS[it.group as usize].to_uppercase()).size(10.5).color(theme::muted_col()));
                                }
                                let on = k == hi;
                                let (rr, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), egui::Sense::click());
                                if on {
                                    ui.painter().rect_filled(rr, CornerRadius::same(7), theme::accent_faded());
                                    if ui.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp)) {
                                        ui.scroll_to_rect(rr, None);
                                    }
                                }
                                ui.painter().text(Pos2::new(rr.left() + 16.0, rr.center().y), egui::Align2::CENTER_CENTER, it.icon, FontId::proportional(13.0), theme::text_col());
                                ui.painter().text(Pos2::new(rr.left() + 34.0, rr.center().y), egui::Align2::LEFT_CENTER, it.name, FontId::new(13.5, theme::bold_family()), theme::text_col());
                                if resp.hovered() {
                                    new_hi = k;
                                }
                                if resp.clicked() {
                                    slash_pick = Some(it.cmd);
                                }
                            }
                        });
                    });
            });
        m.slash_rect = Some(r.response.rect);
        m.slash = Some(SlashSt { at: sl.at, hi: new_hi, tab: new_tab });
        if let Some(cmd) = slash_pick {
            m.slash = None;
            if let Some(done) = slash_apply(stem, sl.at, sel.1, cmd, has_blanks) {
                if let Some(g) = done.eq_at {
                    out.eq = Some(EqAsk { caret: g, span: None, initial: String::new() });
                }
                commit(&mut m, stem, done.change, None, &mut blanks_rows, &mut out);
            }
            bar_act = None;
            edit = None;
            clicked = None;
            sel_rect = None;
        }
    }

    // ----- floating toolbar over a selection (or a blank) -----
    let mut float_act = None;
    let float_keep = m.float_rect.is_some_and(|r| ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()).is_some_and(|p| r.expand(4.0).contains(p))));
    // A click on the bar makes the text field give up focus as it is drawn (egui surrenders
    // focus on a click elsewhere), so on that frame no unit reports a selection: the bar stays
    // where it was while the pointer is on it, or its buttons could never be clicked.
    let float_at = sel_rect.or(if float_keep { m.float_sel } else { None });
    if let Some(sr) = float_at.filter(|_| focused.is_some() || float_keep) {
        let (at, pivot) = float_place(sr, m.float_rect.map(|r| r.size()), fixed_bar, ui.ctx().content_rect());
        let (a, r) = float_bar(ui, id, at, pivot, here, has_blanks, o.structured);
        float_act = a;
        m.float_rect = Some(r);
        m.float_sel = Some(sr);
    } else if !float_keep {
        m.float_rect = None;
        m.float_sel = None;
    }

    // ----- equation chips, blank chips -----
    if let Some((k, a)) = clicked {
        let u = d.unit(flat[k]).clone();
        match a.kind {
            AtomKind::Math(c) => {
                let g0 = d.global(&u, 0);
                out.eq = Some(EqAsk { caret: g0 + a.start, span: Some(g0 + a.start..g0 + a.end), initial: c });
            }
            AtomKind::Blank(_) => {
                set_cursor(&ctx, ids[k], a.start, a.end);
                let g0 = d.global(&u, 0);
                m.sel = Some((g0 + a.start, g0 + a.end));
                m.sel_unit = Some(k);
            }
        }
    }

    // ----- link popover -----
    let mut link_act: Option<Act> = None;
    if let Some(mut ls) = m.link.take() {
        let at = ls.at.or(sel_rect.map(|r| r.left_bottom())).or(fixed_bar.map(|r| r.left_bottom())).unwrap_or(top);
        let mut keep = true;
        let pid = id.with("wys_link_pop");
        egui::Area::new(pid).order(egui::Order::Foreground).fixed_pos(at + Vec2::new(0.0, 4.0)).show(&ctx, |ui| {
            Frame::new()
                .fill(theme::card())
                .stroke(Stroke::new(1.0, theme::border()))
                .corner_radius(CornerRadius::same(10))
                .shadow(egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(40) })
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Link").size(12.0).color(theme::text_col()));
                        let te = ui.add(TextEdit::singleline(&mut ls.url).hint_text("https://… or name@school.edu").desired_width(240.0).id(pid.with("url")));
                        if !ls.focused {
                            te.request_focus();
                            ls.focused = true;
                        }
                        let ok = richtext::normalize_url(&ls.url).is_some();
                        let enter = te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if ui.add_enabled(ok, egui::Button::new("Apply")).clicked() || (enter && ok) {
                            link_act = Some(Act::LinkApply);
                        }
                        if ls.existing && ui.button("Remove").on_hover_text("Keep the text, drop the link").clicked() {
                            link_act = Some(Act::LinkRemove);
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            keep = false;
                        }
                    });
                    if !ls.url.trim().is_empty() && richtext::normalize_url(&ls.url).is_none() {
                        ui.label(RichText::new("Only web (http, https) and e-mail addresses can be links.").size(11.0).color(theme::bad()));
                    } else if ls.err {
                        ui.label(RichText::new("This selection can't be a link: select text on one line, without equations or [ ] brackets.").size(11.0).color(theme::bad()));
                    }
                });
        });
        if let Some(a) = link_act {
            // the selection the popover was opened on, in its unit
            if let Some(u) = d.flat().get(ls.unit).map(|&f| d.unit(f).clone()) {
                let text = slice(stem, u.start..u.end);
                let lc = |g: usize| g.saturating_sub(u.start).min(clen(&text));
                let url = if a == Act::LinkApply { ls.url.clone() } else { String::new() };
                match link_op(&text, lc(ls.sel.0), lc(ls.sel.1), &url) {
                    Some((t, x, y)) => {
                        let s = apply_unit(stem, &u, &t);
                        let g0 = if u.virt.is_some() { d.global(&u, 0) } else { u.start };
                        change = Some(Change { stem: s, anchor: g0 + x, caret: g0 + y, virt: u.virt, blank: None, keep: None });
                        keep = false;
                    }
                    // say why instead of silently doing nothing
                    None => ls.err = true,
                }
            } else {
                keep = false;
            }
        }
        if keep {
            m.link = Some(ls);
        }
    }
    if let Some(c) = change.take() {
        commit(&mut m, stem, c, None, &mut blanks_rows, &mut out);
    }

    // ----- the block rail: "+" and the drag grip beside the hovered top-level block -----
    let mut rail_change: Option<(Change, Option<usize>)> = None;
    if rail {
        let segs = rail_blocks(stem);
        let spans: Vec<Option<(f32, f32)>> = segs
            .iter()
            .map(|(_, r)| {
                let ys: Vec<(f32, f32)> = r.clone().filter_map(|l| line_ys.borrow().get(&l).copied()).collect();
                (!ys.is_empty()).then(|| (ys.iter().map(|y| y.0).fold(f32::MAX, f32::min), ys.iter().map(|y| y.1).fold(f32::MIN, f32::max)))
            })
            .collect();
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let x0 = top.x;
        let hovered = pointer
            .filter(|p| p.x >= x0 && p.x <= x0 + o.width)
            .and_then(|p| spans.iter().position(|s| s.is_some_and(|(a, b)| p.y >= a - 3.0 && p.y <= b + 3.0)));
        if m.drag.is_some_and(|k| k >= segs.len()) {
            m.drag = None;
        }
        if let Some(k) = m.drag.or(hovered)
            && let Some((y0, y1)) = spans[k]
        {
            let row_h = (st.size * 1.4).min(y1 - y0).max(16.0);
            let cy = y0 + row_h / 2.0;
            let plus_r = Rect::from_center_size(Pos2::new(x0 + 8.0, cy), Vec2::new(15.0, 18.0));
            let grip_r = Rect::from_center_size(Pos2::new(x0 + 23.0, cy), Vec2::new(13.0, 18.0));
            let pr = ui.interact(plus_r, id.with("wys_rail_plus"), egui::Sense::click());
            let gr = ui.interact(grip_r, id.with("wys_rail_grip"), egui::Sense::click_and_drag());
            pr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Add a block below"));
            gr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Move this block (drag, or Alt+Up / Alt+Down)"));
            let pr = pr.on_hover_text("Add a block below (or type / at the start of a line)");
            let gr = gr.on_hover_text("Drag to move this block (or Alt+↑ / Alt+↓ with the caret in it). Click to select it");
            let p = ui.painter();
            let ink = theme::muted_col();
            for (r, hot) in [(plus_r, pr.hovered()), (grip_r, gr.hovered() || gr.dragged())] {
                if hot {
                    p.rect_filled(r, 4.0, theme::accent_faded());
                }
            }
            let c = plus_r.center();
            let sk = Stroke::new(1.4, if pr.hovered() { theme::text_col() } else { ink });
            p.line_segment([c - Vec2::new(4.5, 0.0), c + Vec2::new(4.5, 0.0)], sk);
            p.line_segment([c - Vec2::new(0.0, 4.5), c + Vec2::new(0.0, 4.5)], sk);
            let gc = grip_r.center();
            for dx in [-2.5f32, 2.5] {
                for dy in [-5.0f32, 0.0, 5.0] {
                    p.circle_filled(gc + Vec2::new(dx, dy), 1.3, if gr.hovered() || gr.dragged() { theme::text_col() } else { ink });
                }
            }
            if gr.hovered() || gr.dragged() {
                ctx.set_cursor_icon(if gr.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
            }
            if pr.clicked() {
                rail_change = plus_op(stem, k).map(|(c, g)| (c, Some(g)));
            }
            // the block is picked up on the press (the pointer may leave every block before
            // egui calls it a drag; the rail then stays with this block until the release)
            if (gr.is_pointer_button_down_on() || gr.drag_started()) && m.drag.is_none() {
                m.drag = Some(k);
            }
            // where a drop would land: the gap whose neighbours' middles straddle the pointer
            let gap_at = |y: f32| spans.iter().filter(|s| s.is_some_and(|(a, b)| (a + b) / 2.0 < y)).count();
            let drop = ui.input(|i| i.pointer.interact_pos()).map(|p| gap_at(p.y));
            if let (Some(from), Some(g)) = (m.drag, drop)
                && (gr.dragged() || gr.drag_stopped())
            {
                let y = match (g.checked_sub(1).and_then(|i| spans[i]), spans.get(g).copied().flatten()) {
                    (Some((_, b)), Some((a, _))) => (a + b) / 2.0,
                    (Some((_, b)), None) => b + 2.0,
                    (None, Some((a, _))) => a - 2.0,
                    _ => y0,
                };
                if g != from && g != from + 1 {
                    p.line_segment([Pos2::new(x0 + rail_w, y), Pos2::new(x0 + o.width, y)], Stroke::new(2.0, theme::accent_col()));
                }
                if gr.drag_stopped() {
                    m.drag = None;
                    rail_change = move_block(stem, from, g).map(|c| (c, None));
                }
                ctx.request_repaint();
            } else if gr.drag_stopped() {
                m.drag = None;
            } else if gr.clicked() {
                m.drag = None;
                // select the block (a paragraph or heading whole; else the caret goes to its start)
                let (kind, r) = segs[k].clone();
                let a = d.line_start[r.start];
                let b = d.line_end(r.end - 1);
                let want = match kind {
                    SegKind::Para => Want { anchor: a, caret: b, virt: None },
                    SegKind::Heading(_) => Want { anchor: a + richtext::heading_prefix_len(stem.split('\n').nth(r.start).unwrap_or("")), caret: b, virt: None },
                    _ => {
                        let g = d.find(a, None).map(|fi| d.unit(d.flat()[fi]).start).unwrap_or(a);
                        Want { anchor: g, caret: g, virt: None }
                    }
                };
                m.want = Some(want);
                ctx.request_repaint();
            } else if !ui.input(|i| i.pointer.any_down()) {
                m.drag = None;
            }
        } else if !ui.input(|i| i.pointer.any_down()) {
            m.drag = None;
        }
    }
    if let Some((c, slash_at)) = rail_change {
        bar_act = None;
        edit = None;
        commit(&mut m, stem, c, None, &mut blanks_rows, &mut out);
        if let Some(g) = slash_at {
            m.slash = Some(SlashSt { at: g, hi: 0, tab: None });
        }
    }

    // ----- apply this frame's typing / toolbar action -----
    if let Some(a) = bar_act.or(float_act) {
        match a {
            Act::Undo => do_undo(false, &mut m, stem, &mut blanks_rows, &mut out),
            Act::Redo => do_undo(true, &mut m, stem, &mut blanks_rows, &mut out),
            Act::Markup => m.markup = true,
            a => {
                if let Some(c) = run_act(a, &mut m, stem, &d, &mut blanks_rows, &mut out) {
                    commit(&mut m, stem, c, None, &mut blanks_rows, &mut out);
                }
            }
        }
    } else if let Some((k, mut t)) = edit {
        let at = flat[k];
        let u = d.unit(at).clone();
        // typing mode: the typed text takes the pending style
        let mut typed: Option<(usize, Style, Option<usize>)> = None;
        if let Some((p, want)) = m.pending.take()
            && u.virt.is_none()
            && (u.start..=u.end).contains(&p)
        {
            let old = slice(stem, u.start..u.end);
            let lp = p - u.start;
            let grew = clen(&t).saturating_sub(clen(&old));
            if let Some((t2, c)) = style_typed(&old, &t, lp, want) {
                t = t2;
                typed = Some((u.start + c, want, Some(c)));
            } else if grew > 0 && slice(&t, 0..lp) == slice(&old, 0..lp) && slice(&t, lp + grew..clen(&t)) == slice(&old, lp..clen(&old)) {
                // spaces (or text that cannot carry the style): the mode moves along with the caret
                typed = Some((p + grew, want, None));
            }
        }
        let new_stem = match u.kind {
            UKind::Cell { .. } => cell_edit(stem, &d, at.0, at.1, &t),
            UKind::Flow => Some(apply_flow(stem, &d, &u, &t)),
            _ => Some(apply_unit(stem, &u, &t)),
        };
        if let Some(s) = new_stem {
            let (_, lc) = local_sel(&ctx, ids[k], clen(&t));
            let caret = d.global(&u, lc);
            let (_, la0) = (0, local_sel(&ctx, ids[k], clen(&t)).0);
            let anchor = d.global(&u, la0);
            let uid = ids[k];
            let c = Change { stem: s, anchor, caret, virt: None, blank: None, keep: None };
            commit(&mut m, stem, c, Some(uid), &mut blanks_rows, &mut out);
            match typed {
                Some((g, want, Some(_))) => {
                    // the styled text has markers the TextEdit did not type: place the caret
                    m.want = Some(Want { anchor: g, caret: g, virt: None });
                    m.pending = Some((g, want));
                }
                _ => {
                    if let Some((g, want, None)) = typed {
                        m.pending = Some((g, want));
                    }
                    // stay put when the same unit still holds the caret (keeps IME composition intact)
                    let nd = doc(stem, o.structured);
                    let nids = unit_ids(id, &nd);
                    if nd.find(caret, None).map(|f| nids[f]) == Some(uid) && u.virt.is_none() {
                        m.want = None;
                    }
                }
            }
        } else {
            // the edit would break the table: put the old text back
            let (_, lc) = local_sel(&ctx, ids[k], usize::MAX);
            let g = d.global(&u, lc.min(u.end - u.start));
            m.want = Some(Want { anchor: g, caret: g, virt: None });
        }
    }
    if let (Some(b), Some(r)) = (o.blanks.as_deref_mut(), blanks_rows) {
        *b = r;
    }
    // a click on the field's empty space puts the caret at the end of the text
    let body = Rect::from_min_max(top, Pos2::new(top.x + o.width, ui.min_rect().bottom()));
    let bg_click = ui.input(|i| i.pointer.primary_clicked() && i.pointer.interact_pos().is_some_and(|p| body.contains(p)));
    let over_bar = m.float_rect.is_some_and(|r| ui.input(|i| i.pointer.interact_pos().is_some_and(|p| r.contains(p)))) || over_slash || slash_pick.is_some();
    if bg_click && !over_bar && m.want.is_none() && bar_act.is_none() && float_act.is_none() && !ids.iter().any(|u| ctx.memory(|mm| mm.has_focus(*u))) {
        let flat = d.flat();
        if let Some(&last) = flat.iter().rev().find(|f| d.unit(**f).virt.is_none()) {
            let u = d.unit(last);
            m.want = Some(Want { anchor: u.end, caret: u.end, virt: None });
            ctx.request_repaint();
        }
    }
    if ids.iter().any(|u| ctx.memory(|mm| mm.has_focus(*u))) && m.want.is_none() {
        m.retry = None;
    } else if m.retry.is_some() {
        ctx.request_repaint();
    }
    m.active = focused.is_some() || m.float_rect.is_some();
    out.focused = focused.is_some();
    out.caret = m.sel.map(|s| s.1);
    out.rect = Rect::from_min_max(top, Pos2::new(top.x + o.width, ui.min_rect().bottom().max(top.y + o.size)));
    ctx.data_mut(|dd| dd.insert_temp(id, m));
    out
}

/// After a unit drew: remember its edit, chip click, selection rect, row info; snap the caret off
/// collapsed markers and out of atoms.
#[allow(clippy::too_many_arguments)]
fn after_unit(ctx: &egui::Context, m: &mut Mem, k: usize, uid: UiId, sh: &UnitShow, text: &str, stem: &str, u: &Unit, edit: &mut Option<(usize, String)>, clicked: &mut Option<(usize, Atom)>, sel_rect: &mut Option<Rect>) {
    let old = slice(stem, u.start..u.end);
    if text != old && edit.is_none() {
        *edit = Some((k, text.to_string()));
    }
    if let Some(a) = &sh.clicked_atom {
        *clicked = Some((k, a.clone()));
    }
    if !ctx.memory(|mm| mm.has_focus(uid)) {
        return;
    }
    let Some(cr) = sh.out.state.cursor.char_range() else { return };
    let p = usize::from(cr.primary.index);
    let s = usize::from(cr.secondary.index);
    let g = &sh.out.galley;
    let row = g.layout_from_cursor(cr.primary).row;
    m.rows = (row == 0, row + 1 >= g.rows.len());
    m.caret_rect = Some(g.pos_from_cursor(CCursor::new(CharIndex::from(p))).translate(sh.origin.to_vec2()));
    if p != s {
        let (lo, hi) = (p.min(s), p.max(s));
        let r0 = g.pos_from_cursor(CCursor::new(CharIndex::from(lo))).translate(sh.origin.to_vec2());
        let r1 = g.pos_from_cursor(CCursor::new(CharIndex::from(hi))).translate(sh.origin.to_vec2());
        *sel_rect = Some(Rect::from_min_max(Pos2::new(r0.min.x.min(r1.min.x), r0.min.y.min(r1.min.y)), Pos2::new(r0.max.x.max(r1.max.x), r1.max.y)));
    } else if sh.ana.atoms.iter().any(|a| matches!(a.kind, AtomKind::Blank(_)) && (a.start == p || a.end == p)) && text == old {
        // caret beside a blank: offer Unblank
        let r0 = g.pos_from_cursor(CCursor::new(CharIndex::from(p))).translate(sh.origin.to_vec2());
        *sel_rect = Some(r0);
    }
    // caret snapping (navigation only; typing leaves the caret where it typed)
    let prev = m.prev.filter(|(id, _)| *id == uid).map(|(_, c)| c);
    if text == old && p == s && prev != Some(p) {
        let keyboard = ctx.input(|i| i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, key, .. } if matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End))));
        let q = sh.ana.canonical(p, prev, keyboard);
        if q != p {
            set_cursor(ctx, uid, q, q);
            ctx.request_repaint();
        }
        m.prev = Some((uid, q));
    } else {
        m.prev = Some((uid, p));
    }
}

fn push_undo(m: &mut Mem, stem: &str, caret: usize, blanks: Option<&[String]>, now: f64, unit: Option<UiId>, typing: bool) {
    let coalesce = typing && m.last_unit == unit && unit.is_some() && now - m.last_push < 1.2 && !m.undo.is_empty();
    if !coalesce {
        m.undo.push(Snap { stem: stem.to_string(), caret, blanks: blanks.map(|b| b.to_vec()) });
        if m.undo.len() > 200 {
            m.undo.remove(0);
        }
    }
    m.last_push = now;
    m.last_unit = if typing { unit } else { None };
    m.redo.clear();
}

/// Record a change made outside the editor (the equation dialog) for undo, and put the caret
/// at `caret` next frame.
pub fn record_external(ctx: &egui::Context, id: UiId, old: &str, caret: usize, blanks: Option<&[String]>) {
    ctx.data_mut(|d| {
        let m = d.get_temp_mut_or_default::<Mem>(id);
        let now = 0.0;
        push_undo(m, old, caret, blanks, now, None, false);
        m.want = Some(Want { anchor: caret, caret, virt: None });
    });
}

mod blockops;
pub use blockops::*;

#[cfg(test)]
mod tests;
