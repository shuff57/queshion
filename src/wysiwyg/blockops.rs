//! Round-7 structure edits: inline code, centring, list nesting, task items, Heading 4-6, and
//! moving or adding whole blocks (the hover rail). Pure functions on the markup string, like
//! the rest of the editor's edits: each returns the new stem and the selection to restore.

use super::*;

// -------------------------------------------------------------------------------------
// Inline code
// -------------------------------------------------------------------------------------

/// The `` `code` `` spans of a unit's text (any line): local chars of each opening and
/// closing backtick.
pub fn code_spans(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut base = 0;
    for line in text.split('\n') {
        for r in richtext::inline_runs(line) {
            if r.code && r.kind == RunKind::Text {
                out.push((base + r.start - 1, base + r.end));
            }
        }
        base += clen(line) + 1;
    }
    out
}

/// The visible text of one line as every renderer reads it.
fn shown(line: &str) -> String {
    richtext::line_text(&richtext::inline(line))
}

/// Inline code on the selection `a..b` (local chars of a unit's text): code it touches loses
/// its backticks, otherwise the selection (edge spaces left out) becomes `` `code` ``. Only
/// plain text on one line can become code (no equation, link, blank or hidden marker inside);
/// a bare caret does nothing (as in the reference). Returns the new text and selection.
pub fn code_op(text: &str, a: usize, b: usize) -> Option<(String, usize, usize)> {
    let (a, b) = (a.min(b), a.max(b));
    if a == b {
        return None;
    }
    let hit: Vec<(usize, usize)> = code_spans(text).into_iter().filter(|&(o, c)| o < b && a <= c).collect();
    if !hit.is_empty() {
        let mut t = text.to_string();
        let mut gone: Vec<usize> = Vec::new();
        for &(o, c) in hit.iter().rev() {
            t = splice(&t, c..c + 1, "");
            t = splice(&t, o..o + 1, "");
            gone.extend([o, c]);
        }
        let shift = |p: usize| p - gone.iter().filter(|&&g| g < p).count();
        let (first, last) = (hit[0].0, hit[hit.len() - 1].1);
        return Some((t, shift(a.min(first)), shift(b.max(last + 1))));
    }
    let ch: Vec<char> = text.chars().collect();
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
    // every selected char is shown plain text (no marker, equation, link or code)
    let ls = ch[..x].iter().rposition(|&c| c == '\n').map_or(0, |p| p + 1);
    let le = (y..ch.len()).find(|&j| ch[j] == '\n').unwrap_or(ch.len());
    let line: String = ch[ls..le].iter().collect();
    let runs = richtext::inline_runs(&line);
    let plain_at = |p: usize| runs.iter().any(|r| r.start <= p && p < r.end && r.kind == RunKind::Text && !r.link && !r.code);
    if !(x - ls..y - ls).all(plain_at) {
        return None;
    }
    let t = splice(&splice(text, y..y, "`"), x..x, "`");
    let new_line = slice(&t, ls..le + 2);
    (code_spans(&t).contains(&(x, y + 1)) && shown(&new_line) == shown(&line)).then_some((t, x + 1, y + 1))
}

// -------------------------------------------------------------------------------------
// Centring
// -------------------------------------------------------------------------------------

/// Centre button with the selection `sel` (global) in flat unit `fi`: a block in a centred
/// group leaves it (the whole group's fences go); otherwise the caret's paragraph (every
/// paragraph the selection touches) or the whole list / table / heading / divider is wrapped
/// in `::: center` ... `:::`.
pub fn center_op(stem: &str, d: &Doc, fi: usize, sel: (usize, usize)) -> Option<Change> {
    let at = d.flat()[fi];
    let bl = &d.blocks[at.0];
    let u = d.unit(at);
    let mut lines = lines_of(stem);
    if let Some(g) = &bl.center {
        let open_at = d.line_start[g.start];
        let w = clen(&lines[g.start]) + 1;
        lines.remove(g.end - 1);
        lines.remove(g.start);
        let mv = |p: usize| if p > open_at { p.saturating_sub(w).max(open_at) } else { p };
        return Some(Change { stem: lines.join("\n"), anchor: mv(sel.0), caret: mv(sel.1), virt: None, blank: None, keep: None });
    }
    if u.virt.is_some() {
        return None;
    }
    let (lo, hi) = (sel.0.min(sel.1), sel.0.max(sel.1));
    let (first, last) = match bl.kind {
        BlockKind::Flow => {
            let (mut s, mut e) = (d.line_of(lo).max(bl.lines.start), d.line_of(hi).min(bl.lines.end - 1));
            let blank = |l: usize| lines[l].trim().is_empty();
            if (s..=e).any(|l| !blank(l)) {
                while blank(s) {
                    s += 1;
                }
                while blank(e) {
                    e -= 1;
                }
            }
            while s > bl.lines.start && !blank(s - 1) && !blank(s) {
                s -= 1;
            }
            while e + 1 < bl.lines.end && !blank(e + 1) && !blank(e) {
                e += 1;
            }
            (s, e)
        }
        BlockKind::Side { .. } => return None,
        _ => (bl.lines.start, bl.lines.end - 1),
    };
    lines.insert(last + 1, richtext::FENCE_CLOSE.to_string());
    lines.insert(first, richtext::FENCE_CENTER.to_string());
    let from = d.line_start[first];
    let w = clen(richtext::FENCE_CENTER) + 1;
    let mv = |p: usize| if p >= from { p + w } else { p };
    let s = lines.join("\n");
    let nd = doc(&s, true);
    nd.blocks.iter().any(|b| b.center == Some(first..last + 3)).then(|| Change { stem: s, anchor: mv(sel.0), caret: mv(sel.1), virt: None, blank: None, keep: None })
}

/// Is the caret's block centred?
pub fn is_centered(d: &Doc, fi: usize) -> bool {
    d.flat().get(fi).is_some_and(|&(b, _)| d.blocks[b].center.is_some())
}

// -------------------------------------------------------------------------------------
// List nesting
// -------------------------------------------------------------------------------------

/// Leading indentation columns of a line (a tab counts two).
fn indent_cols(l: &str) -> usize {
    l.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 2 } else { 1 }).sum()
}

/// The nesting level of each item line of list block `bl`.
pub fn block_depths(stem: &str, bl: &EBlock) -> Vec<u8> {
    let lines: Vec<&str> = stem.split('\n').collect();
    richtext::item_depths(&lines[bl.lines.clone()])
}

/// Tab (`deeper`) / Shift+Tab in a bullet or numbered item: the item and the items nested
/// under it move one level in or out (two spaces per level). The first item cannot go in, an
/// item at most one level deeper than the one above it, at most `richtext::MAX_DEPTH`.
pub fn indent_op(stem: &str, d: &Doc, fi: usize, sel: (usize, usize), deeper: bool) -> Option<Change> {
    let at = d.flat()[fi];
    let u = d.unit(at).clone();
    let UKind::Item { list, line } = u.kind else { return None };
    if list == ListKind::Quote {
        return None;
    }
    let bl = &d.blocks[at.0];
    let mut lines = lines_of(stem);
    let depths = block_depths(stem, bl);
    let j = line - bl.lines.start;
    let dj = depths[j];
    let nd = if deeper {
        if j == 0 || dj > depths[j - 1] || dj >= richtext::MAX_DEPTH {
            return None;
        }
        dj + 1
    } else {
        dj.checked_sub(1)?
    };
    let mut end = j + 1;
    while end < depths.len() && depths[end] > dj {
        end += 1;
    }
    let base = indent_cols(&lines[bl.lines.start]) / 2;
    for k in j..end {
        let l = bl.lines.start + k;
        let newd = (depths[k] as i32 + nd as i32 - dj as i32).max(0) as usize;
        let body = lines[l].trim_start().to_string();
        lines[l] = format!("{}{body}", "  ".repeat(base + newd));
    }
    if list == ListKind::Numbered {
        renumber(&mut lines, bl.lines.clone());
    }
    let ls = line_starts(&lines);
    let p = richtext::item_prefix_len(list.seg(), &lines[line]);
    let n = clen(&lines[line]) - p;
    let mv = |g: usize| ls[line] + p + g.saturating_sub(u.start).min(n);
    Some(Change { stem: lines.join("\n"), anchor: mv(sel.0), caret: mv(sel.1), virt: None, blank: None, keep: None })
}

/// The nesting level of list item unit `fi` (None outside bullet / numbered items).
pub fn item_depth(stem: &str, d: &Doc, fi: usize) -> Option<u8> {
    let at = d.flat()[fi];
    let UKind::Item { list, line } = d.unit(at).kind else { return None };
    if list == ListKind::Quote {
        return None;
    }
    let bl = &d.blocks[at.0];
    block_depths(stem, bl).get(line - bl.lines.start).copied()
}

// -------------------------------------------------------------------------------------
// Task items (`• ☐ open`, `• ☑ done`: bullet items that start with their box)
// -------------------------------------------------------------------------------------

pub const TASK_OPEN: char = '☐';
pub const TASK_DONE: char = '☑';

/// A task item's text: whether its box is ticked (None: not a task item).
pub fn task_state(text: &str) -> Option<bool> {
    let mut c = text.chars();
    let done = match c.next()? {
        TASK_OPEN => false,
        TASK_DONE | '☒' => true,
        _ => return None,
    };
    matches!(c.next(), None | Some(' ')).then_some(done)
}

/// Tick or untick the task item in flat unit `fi`.
pub fn toggle_task(stem: &str, d: &Doc, fi: usize, sel: (usize, usize)) -> Option<Change> {
    let u = d.unit(d.flat()[fi]).clone();
    let done = task_state(&slice(stem, u.start..u.end))?;
    let b = if done { TASK_OPEN } else { TASK_DONE };
    let s = splice(stem, u.start..u.start + 1, &b.to_string());
    Some(Change { stem: s, anchor: sel.0, caret: sel.1, virt: None, blank: None, keep: None })
}

/// Turn the caret's line into a task item (a bullet item starting with an open box).
pub fn make_task(stem: &str, d: &Doc, fi: usize, g: usize) -> Option<Change> {
    let u = d.unit(d.flat()[fi]).clone();
    let c = if matches!(u.kind, UKind::Item { list: ListKind::Bullets, .. }) {
        Change::at(stem.to_string(), g)
    } else {
        toggle_list(stem, d, fi, (g, g), ListKind::Bullets)?
    };
    let nd = doc(&c.stem, true);
    let nfi = nd.find(c.caret, None)?;
    let nu = nd.unit(nd.flat()[nfi]).clone();
    if !matches!(nu.kind, UKind::Item { list: ListKind::Bullets, .. }) {
        return None;
    }
    if task_state(&slice(&c.stem, nu.start..nu.end)).is_some() {
        return Some(c);
    }
    let s = splice(&c.stem, nu.start..nu.start, &format!("{TASK_OPEN} "));
    let mv = |p: usize| if p >= nu.start { p + 2 } else { p };
    Some(Change { stem: s, anchor: mv(c.anchor), caret: mv(c.caret), virt: None, blank: None, keep: None })
}

// -------------------------------------------------------------------------------------
// Heading 4-6: a line of its own styled like an imported h4-h6 (docs/stem-format.md)
// -------------------------------------------------------------------------------------

/// The bold / italic a heading of level 4, 5 or 6 prints with.
pub fn minor_heading_style(level: u8) -> (bool, bool) {
    match level {
        4 => (true, false),
        5 => (true, true),
        _ => (false, true),
    }
}

/// `text` (one line) with every char bold / italic as heading `level` (4-6) prints, the
/// other styles kept; an empty line gets a placeholder. Returns the line and the local range
/// of its text (to select), or None when the markup cannot carry it.
pub fn minor_heading_line(text: &str, level: u8) -> Option<(String, usize, usize)> {
    let (b, i) = minor_heading_style(level);
    let t = text.trim();
    let t = if visibly_empty(t) { format!("Heading {level}") } else { t.to_string() };
    let (mut items, _) = decode_line(&t);
    for it in &mut items {
        if let Item::Ch { b: ib, i: ii, .. } = it {
            (*ib, *ii) = (b, i);
        }
    }
    let (enc, es) = encode_checked(&items)?;
    let x = raw_of(&items, &es, &enc, 0, false);
    let y = raw_of(&items, &es, &enc, items.len(), true);
    Some((enc, x, y))
}

// -------------------------------------------------------------------------------------
// Whole blocks: the hover rail ("+" and the drag grip) and Alt+Up / Alt+Down
// -------------------------------------------------------------------------------------

/// The stem's top-level blocks (what the rail offers): each segment's kind and lines.
pub fn rail_blocks(stem: &str) -> Vec<(SegKind, Range<usize>)> {
    let lines: Vec<&str> = stem.split('\n').collect();
    richtext::segment(&lines).into_iter().map(|s| (s.kind, s.start..s.end)).collect()
}

/// The top-level block holding source line `line` (None on a blank line between blocks).
pub fn rail_block_of(stem: &str, line: usize) -> Option<usize> {
    rail_blocks(stem).iter().position(|(_, r)| r.contains(&line))
}

/// Move top-level block `from` into gap `to` (0 = before the first block, n = after the last).
/// Blank lines are added only where the blocks around a seam would otherwise read as one
/// (two paragraphs, a numbered list under a paragraph, rows of the same width ...): every
/// block must read back exactly as it was. The caret goes to the start of the moved block.
pub fn move_block(stem: &str, from: usize, to: usize) -> Option<Change> {
    let segs = rail_blocks(stem);
    let n = segs.len();
    if from >= n || to > n || to == from || to == from + 1 {
        return None;
    }
    let lines = lines_of(stem);
    let text_of = |ls: &[String], r: &Range<usize>| ls[r.clone()].join("\n");
    let mut order: Vec<usize> = (0..n).filter(|&k| k != from).collect();
    let ins = if to > from { to - 1 } else { to };
    order.insert(ins, from);
    let expected: Vec<(SegKind, String)> = order.iter().map(|&k| (segs[k].0, text_of(&lines, &segs[k].1))).collect();
    let r = segs[from].1.clone();
    let moved: Vec<String> = lines[r.clone()].to_vec();
    // the stem without the block (the gap it leaves at `r.start`)
    let mut rest: Vec<String> = lines[..r.start].to_vec();
    rest.extend_from_slice(&lines[r.end..]);
    let target = if to == n { rest.len() } else if segs[to].1.start > r.start { segs[to].1.start - r.len() } else { segs[to].1.start };
    let blank = |v: &[String], i: usize| v.get(i).is_none_or(|l| l.trim().is_empty());
    for extra in 0..8u8 {
        let (fill, pre, post) = (extra & 1 != 0, extra & 2 != 0, extra & 4 != 0);
        let mut v = rest.clone();
        let mut t = target;
        // the gap: two blank lines meeting there become one; or a blank line keeps the
        // neighbours apart
        let mut gap = r.start;
        if gap > 0 && gap < v.len() && blank(&v, gap - 1) && blank(&v, gap) && !fill {
            v.remove(gap);
            if t > gap {
                t -= 1;
            }
        } else if fill && gap > 0 && gap < v.len() && !blank(&v, gap - 1) && !blank(&v, gap) {
            v.insert(gap, String::new());
            if t >= gap {
                t += 1;
            }
            gap += 1;
        } else if fill {
            continue;
        }
        // a blank line left at either end of the stem goes with the block
        if gap == v.len() && gap > 0 && blank(&v, gap - 1) && v.len() > 1 {
            v.remove(gap - 1);
            if t >= gap {
                t -= 1;
            }
        } else if gap == 0 && !v.is_empty() && blank(&v, 0) && v.len() > 1 {
            v.remove(0);
            t = t.saturating_sub(1);
        }
        let mut piece: Vec<String> = Vec::new();
        if pre && t > 0 && !blank(&v, t - 1) {
            piece.push(String::new());
        } else if pre {
            continue;
        }
        piece.extend(moved.iter().cloned());
        if post && t < v.len() && !blank(&v, t) {
            piece.push(String::new());
        } else if post {
            continue;
        }
        let lead = piece.iter().take_while(|l| l.is_empty()).count();
        let at_line = t + lead;
        v.splice(t..t, piece);
        let got: Vec<(SegKind, String)> = rail_blocks(&v.join("\n")).iter().map(|(k, r)| (*k, text_of(&v, r))).collect();
        if got == expected {
            let ls = line_starts(&v);
            return Some(Change::at(v.join("\n"), ls[at_line]));
        }
    }
    None
}

/// The "+" of block `k`: a new paragraph under it holding a `/` (the editor opens the slash
/// menu on it). Returns the edit (caret after the `/`) and the `/`'s global char.
pub fn plus_op(stem: &str, k: usize) -> Option<(Change, usize)> {
    let segs = rail_blocks(stem);
    let end = segs.get(k)?.1.end;
    let mut lines = lines_of(stem);
    // the slash line: after one blank line (the one already there, or a new one)
    let at = if end < lines.len() && lines[end].trim().is_empty() {
        end + 1
    } else {
        lines.insert(end, String::new());
        end + 1
    };
    lines.insert(at, "/".to_string());
    if at + 1 < lines.len() && !lines[at + 1].trim().is_empty() {
        lines.insert(at + 1, String::new());
    }
    let ls = line_starts(&lines);
    let g = ls[at];
    Some((Change::at(lines.join("\n"), g + 1), g))
}
