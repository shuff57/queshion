//! Round-trip and behaviour tests for the WYSIWYG editor's pure model.

use super::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.below(v.len())]
    }
}

/// Random markup over the tokens the parser reacts to.
fn markup(r: &mut Rng, max: usize) -> String {
    const TOK: &[&str] = &[
        "a", "word", "b", " ", " ", "*", "**", "_{", "^{", "}", "$", "x^2", "___", "|", " | ", "\n", "\n", "• ", "- ", "* ", "1. ",
        "2. ", "> ", ">", "---|---", "H", "_", "^", "{", "2", "+", "é", "  ", "\n\n", "| ", " |", "1/2", "$x$", "**b**", "*i*",
        "Na_{2}", "x^{2}", "$\\frac{1}{2}$",
    ];
    let n = r.below(max);
    (0..n).map(|_| r.pick(TOK)).collect()
}

/// Structured-looking stems: lines of lists, tables and paragraphs.
fn stem(r: &mut Rng) -> String {
    let mut lines = Vec::new();
    for _ in 0..r.below(8) + 1 {
        let inl = |r: &mut Rng| markup(r, 6).replace('\n', " ");
        let l = match r.below(7) {
            0 => format!("• {}", inl(r)),
            1 => format!("{}. {}", r.below(3) + 1, inl(r)),
            2 => format!("> {}", inl(r)),
            3 => format!("{} | {}", inl(r), inl(r)),
            4 => "---|---".to_string(),
            5 => String::new(),
            _ => inl(r),
        };
        lines.push(l);
    }
    lines.join("\n")
}

fn ok_math(c: &str) -> bool {
    !c.trim().is_empty() && !c.starts_with(char::is_whitespace) && !c.ends_with(char::is_whitespace)
}

/// The stem rebuilt from the units' texts and the text between them.
fn reassemble(s: &str, d: &Doc) -> String {
    let mut out = String::new();
    let mut at = 0;
    for f in d.flat() {
        let u = d.unit(f);
        if u.virt.is_some() {
            continue;
        }
        out.push_str(&slice(s, at..u.start));
        out.push_str(&slice(s, u.start..u.end));
        at = u.end;
    }
    out.push_str(&slice(s, at..clen(s)));
    out
}

#[test]
fn property_units_reassemble_the_stem_exactly() {
    let mut r = Rng(0x1234_5678_9abc_def1);
    for n in 0..20_000 {
        let s = if n % 2 == 0 { stem(&mut r) } else { markup(&mut r, 40) };
        let d = doc(&s, true);
        let flat = d.flat();
        // in order, inside the text, never overlapping
        let mut last = 0;
        for f in &flat {
            let u = d.unit(*f);
            if u.virt.is_some() {
                continue;
            }
            assert!(u.start >= last && u.end >= u.start && u.end <= clen(&s), "{s:?} {u:?}");
            last = u.end;
        }
        assert_eq!(reassemble(&s, &d), s);
        // writing every unit back unchanged is the identity
        for f in &flat {
            let u = d.unit(*f);
            assert_eq!(apply_unit(&s, u, &slice(&s, u.start..u.end)), s, "{s:?}");
        }
        assert_eq!(check(&s), Ok(()), "self-check failed on {s:?}");
    }
}

#[test]
fn property_editing_one_unit_changes_only_that_unit() {
    let mut r = Rng(99);
    const ALPHA: &[&str] = &["a", "b", "z", " ", "**", "*", "x", "$x$", "_{2}", "^{3}", "___", "é"];
    for _ in 0..5_000 {
        let s = stem(&mut r);
        let d = doc(&s, true);
        let flat = d.flat();
        let real: Vec<usize> = (0..flat.len()).filter(|&k| d.unit(flat[k]).virt.is_none()).collect();
        if real.is_empty() {
            continue;
        }
        let k = real[r.below(real.len())];
        let u = d.unit(flat[k]).clone();
        // inline-only text that starts with a letter (so it cannot start a list or a table)
        let mut t = String::from("q");
        for _ in 0..r.below(6) {
            t.push_str(r.pick(ALPHA));
        }
        let t = t.trim_end().to_string();
        let new = match u.kind {
            UKind::Cell { .. } => match cell_edit(&s, &d, flat[k].0, flat[k].1, &t) {
                Some(n) => n,
                None => continue,
            },
            UKind::Flow => apply_flow(&s, &d, &u, &t),
            _ => apply_unit(&s, &u, &t),
        };
        let nd = doc(&new, true);
        let nflat = nd.flat();
        assert_eq!(nflat.len(), flat.len(), "structure changed: {s:?} -> {new:?}");
        for j in 0..flat.len() {
            let (a, b) = (d.unit(flat[j]), nd.unit(nflat[j]));
            if a.virt.is_some() {
                continue;
            }
            let want = if j == k { t.clone() } else { slice(&s, a.start..a.end) };
            let got = slice(&new, b.start..b.end);
            if j == k && matches!(u.kind, UKind::Flow) {
                // (a blank line may be added to keep a numbered list below a list)
                assert!(got == want || got == format!("{want}\n"), "{s:?} -> {new:?}: {got:?}");
            } else if j != k {
                assert_eq!(got.trim(), want.trim(), "unit {j} of {s:?} -> {new:?}");
            }
        }
    }
}

#[test]
fn units_of_a_structured_stem() {
    let s = "Name the compounds.\n• first **bold**\n• second\nFormula | Name\n---|---\nKI | ___\n\nAfter.";
    let d = doc(s, true);
    let kinds: Vec<&BlockKind> = d.blocks.iter().map(|b| &b.kind).collect();
    assert!(matches!(kinds[0], BlockKind::Flow));
    assert!(matches!(kinds[1], BlockKind::List(ListKind::Bullets)));
    assert!(matches!(kinds[2], BlockKind::Table { header: true, ncols: 2, sep: Some(4) }));
    assert!(matches!(kinds[3], BlockKind::Flow));
    let texts: Vec<String> = d.flat().iter().map(|f| {
        let u = d.unit(*f);
        slice(s, u.start..u.end)
    }).collect();
    assert_eq!(texts, vec!["Name the compounds.", "first **bold**", "second", "Formula", "Name", "KI", "___", "\nAfter."]);
    // a stem that ends with a table gets a virtual paragraph to type after it
    let d = doc("a | b\nc | d", true);
    assert_eq!(d.blocks.first().unwrap().units[0].virt, Some(Virt::Before));
    assert_eq!(d.blocks.last().unwrap().units[0].virt, Some(Virt::After));
    let u = d.blocks.last().unwrap().units[0].clone();
    assert_eq!(apply_unit("a | b\nc | d", &u, "more"), "a | b\nc | d\nmore");
    // answer fields: one flow unit whatever the text
    let d = doc("• not a list here", false);
    assert_eq!(d.flat().len(), 1);
}

#[test]
fn analysis_collapses_markers_and_makes_atoms() {
    let a = analyze("a **b** $x^2$ ___ _{2}", &ok_math);
    let hidden: usize = a.role.iter().filter(|r| **r == Role::Hidden).count();
    assert_eq!(hidden, 4 + 3, "** ** and _{{ }}");
    assert_eq!(a.atoms.len(), 2);
    assert!(matches!(&a.atoms[0].kind, AtomKind::Math(c) if c == "x^2"));
    assert!(matches!(a.atoms[1].kind, AtomKind::Blank(0)));
    // a money-like `$ 5 $` is not an atom: shown literally
    let b = analyze("costs $ 5 $", &ok_math);
    assert!(b.atoms.is_empty() && b.role.contains(&Role::Bad));
}

#[test]
fn property_canonical_caret_is_never_inside_an_atom_and_skips_markers() {
    let mut r = Rng(5);
    for _ in 0..5_000 {
        let s = markup(&mut r, 20).replace('\n', " ");
        let a = analyze(&s, &ok_math);
        let n = clen(&s);
        for p in 0..=n {
            for prev in [None, Some(0), Some(n), Some(p.saturating_sub(1)), Some((p + 1).min(n))] {
                let q = a.canonical(p, prev, true);
                assert!(q <= n && a.inside_atom(q).is_none(), "{s:?} p={p} q={q}");
            }
        }
    }
    // arrowing right over "a**b**c" from after "a" lands after "b", never stuck on a marker
    let a = analyze("a**b**c", &ok_math);
    assert_eq!(a.canonical(2, Some(1), true), 4);
    assert_eq!(a.canonical(3, Some(1), false), 1);
    // left from after "b" goes back to after "a"
    assert_eq!(a.canonical(3, Some(4), true), 1);
    // over an atom in one step
    let a = analyze("x $y$ z", &ok_math);
    assert_eq!(a.canonical(3, Some(2), true), 5);
    assert_eq!(a.canonical(4, Some(5), true), 2);
}

#[test]
fn property_decode_encode_roundtrip_reads_back() {
    let mut r = Rng(11);
    let mut ok = 0;
    for _ in 0..20_000 {
        let s = markup(&mut r, 16).replace('\n', " ");
        let (items, _) = decode_line(&s);
        if let Some((e, _)) = encode_checked(&items) {
            ok += 1;
            assert!(same_items(&decode_line(&e).0, &items), "{s:?} -> {e:?}");
        }
    }
    assert!(ok > 10_000, "the encoder should handle most lines, got {ok}");
}

#[test]
fn property_toggling_twice_restores_the_text_and_styles() {
    let mut r = Rng(21);
    let mut applied = 0;
    for _ in 0..10_000 {
        let s = markup(&mut r, 14).replace('\n', " ");
        let n = clen(&s);
        let (a, b) = (r.below(n + 1), r.below(n + 1));
        let op = [IOp::Bold, IOp::Italic, IOp::Sub, IOp::Sup][r.below(4)];
        let Some((t1, x, y)) = inline_op(&s, a.min(b), a.max(b), op) else { continue };
        let Some((t2, _, _)) = inline_op(&t1, x, y, op) else { continue };
        applied += 1;
        if a.min(b) == a.max(b) {
            continue; // a placeholder was inserted
        }
        let (i0, _) = decode_line(&s);
        let (i1, _) = decode_line(&t1);
        let (i2, _) = decode_line(&t2);
        assert_eq!(i0.len(), i1.len(), "{s:?} -> {t1:?}");
        // sub/sup are exclusive, so toggling sub over a superscript does not restore it
        if matches!(op, IOp::Bold | IOp::Italic) {
            assert!(same_items(&i0, &i2) || !same_items(&i0, &i1), "{s:?} -> {t1:?} -> {t2:?}");
        }
    }
    assert!(applied > 3_000, "{applied}");
}

#[test]
fn formatting_ops_examples() {
    // bold over a selection, spaces stay outside
    assert_eq!(inline_op("the most one", 3, 9, IOp::Bold).unwrap().0, "the **most** one");
    // toggling off
    assert_eq!(inline_op("the **most** one", 6, 10, IOp::Bold).unwrap().0, "the most one");
    // caret in a word: the word
    assert_eq!(inline_op("make it red", 9, 9, IOp::Italic).unwrap().0, "make it *red*");
    // no word: a selected placeholder
    let (t, a, b) = inline_op("x ", 2, 2, IOp::Bold).unwrap();
    assert_eq!(t, "x **bold text**");
    assert_eq!(slice(&t, a..b), "bold text");
    // sub replaces sup, scripts use braces
    assert_eq!(inline_op("H2O", 1, 2, IOp::Sub).unwrap().0, "H_{2}O");
    assert_eq!(inline_op("x^{2}", 3, 4, IOp::Sub).unwrap().0, "x_{2}");
    // across two lines of a paragraph
    assert_eq!(inline_op("ab\ncd", 1, 4, IOp::Bold).unwrap().0, "a**b**\n**c**d");
    // clear formatting
    assert_eq!(inline_op("**a** *b* c_{2}", 0, 14, IOp::Clear).unwrap().0, "a b c2");
    // italic cannot open after a letter: refused rather than written wrongly
    assert_eq!(inline_op("abc", 1, 2, IOp::Italic), None);
}

#[test]
fn make_blank_and_unblank() {
    let (t, caret, bs, word) = make_blank("Water boils at 100 C", 15, 18).unwrap();
    assert_eq!(t, "Water boils at ___ C");
    assert_eq!(word, "100");
    assert_eq!((bs, caret), (15, 18));
    // bold word: the answer is the plain word and the marks go
    let (t, _, _, word) = make_blank("the **red** fox", 6, 9).unwrap();
    assert_eq!((t.as_str(), word.as_str()), ("the ___ fox", "red"));
    // trailing spaces in the selection stay as text
    let (t, _, _, word) = make_blank("a big dog", 2, 6).unwrap();
    assert_eq!((t.as_str(), word.as_str()), ("a ___ dog", "big"));
    // unblank puts the first accepted answer back
    let s = "Water boils at ___ C";
    let d = doc(s, true);
    let u = d.unit(d.flat()[0]).clone();
    let rows = Some(vec!["100 OR one hundred".to_string()]);
    let c = run_unblank(s, &d, &u, s, 18, 18, &rows).unwrap();
    assert_eq!(c.stem, "Water boils at 100 C");
}

#[test]
fn blanks_follow_their_markers() {
    let rows = vec!["one".to_string(), "two".to_string(), "three".to_string()];
    let old = "a ___ b ___ c ___";
    // text typed before the first blank keeps every row
    assert_eq!(remap_blanks(old, "xx a ___ b ___ c ___", &rows), rows);
    // deleting the middle blank drops its row only
    assert_eq!(remap_blanks(old, "a ___ b  c ___", &rows), vec!["one".to_string(), "three".to_string()]);
    // a new blank in the middle gets an empty row in its place
    assert_eq!(remap_blanks(old, "a ___ b ___ x ___ c ___", &rows), vec!["one", "two", "", "three"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
    // a table rewritten with outer pipes keeps every answer
    let s = "A | B\n1 | ___\n2 | ___";
    let d = doc(s, true);
    let bi = d.blocks.iter().position(|b| matches!(b.kind, BlockKind::Table { .. })).unwrap();
    let c = table_op(s, &d, bi, 1, 0, TOp::RowBelow).unwrap();
    let two = vec!["x".to_string(), "y".to_string()];
    assert_eq!(remap_blanks(s, &c.stem, &two), two);
    let d2 = doc(s, true);
    let c2 = table_op(s, &d2, bi, 1, 0, TOp::DelRow).unwrap();
    assert_eq!(remap_blanks(s, &c2.stem, &two), vec!["y".to_string()]);
    // identical rows: the explicit cell map keeps the right answers
    let s = "A | B | C\n___ | ___ | ___\n___ | ___ | ___\n___ | ___ | ___";
    let d = doc(s, true);
    let bi = d.blocks.iter().position(|b| matches!(b.kind, BlockKind::Table { .. })).unwrap();
    let c = table_op(s, &d, bi, 2, 0, TOp::DelRow).unwrap();
    assert_eq!(c.keep, Some([0, 1, 2, 6, 7, 8].iter().map(|&k| Some(k)).collect()));
    let c = table_op(s, &d, bi, 1, 1, TOp::DelCol).unwrap();
    assert_eq!(c.keep, Some([0, 2, 3, 5, 6, 8].iter().map(|&k| Some(k)).collect()));
    let c = table_op(s, &d, bi, 1, 0, TOp::ColLeft).unwrap();
    assert_eq!(c.keep, Some((0..9).map(Some).collect()));
    // toggling a list over two lines keeps both
    let s = "a ___\nb ___";
    let d = doc(s, true);
    let c = toggle_list(s, &d, 0, (0, 9), ListKind::Bullets).unwrap();
    assert_eq!(remap_blanks(s, &c.stem, &two), two);
    // property: inserting plain text anywhere outside blanks keeps rows and order
    let mut r = Rng(3);
    for _ in 0..2_000 {
        let n = clen(old);
        let mut p = r.below(n + 1);
        while blank_starts(old).iter().any(|&b| b < p && p < b + 3) {
            p = r.below(n + 1);
        }
        let new = splice(old, p..p, "zz");
        assert_eq!(remap_blanks(old, &new, &rows), rows, "{new:?}");
    }
}

#[test]
fn lists_enter_and_backspace() {
    let s = "Do:\n• one\n• two";
    let d = doc(s, true);
    // Enter at the end of "one" continues the list
    let fi = 1;
    let c = enter_op(s, &d, fi, 3, 3, true).unwrap();
    assert_eq!(c.stem, "Do:\n• one\n• \n• two");
    // Enter on that empty item leaves the list
    let d2 = doc(&c.stem, true);
    let c2 = enter_op(&c.stem, &d2, 2, 0, 0, true).unwrap();
    assert_eq!(c2.stem, "Do:\n• one\n\n• two");
    // Enter in the middle splits the item, keeping bold on both sides
    let s = "• a **bold** b";
    let d = doc(s, true);
    let c = enter_op(s, &d, 1, 6, 6, true).unwrap();
    assert_eq!(c.stem, "• a **bo**\n• **ld** b");
    // numbered lists renumber
    let s = "1. a\n2. b";
    let d = doc(s, true);
    let c = enter_op(s, &d, 1, 1, 1, true).unwrap();
    assert_eq!(c.stem, "1. a\n2. \n3. b");
    // Backspace at the start of an item takes it out of the list
    let s = "• a\n• b";
    let d = doc(s, true);
    let c = backspace_op(s, &d, 2).unwrap();
    assert_eq!(c.stem, "• a\nb");
    // ... and at the start of the paragraph that follows, joins it to the item above
    let d = doc(&c.stem, true);
    let fi = d.flat().iter().position(|f| d.unit(*f).kind == UKind::Flow && d.unit(*f).virt.is_none()).unwrap();
    let c2 = backspace_op(&c.stem, &d, fi).unwrap();
    assert_eq!(c2.stem, "• ab");
    // Enter in a paragraph keeps a bold run whole on both lines
    let s = "**ab**";
    let d = doc(s, true);
    let c = enter_op(s, &d, 0, 3, 3, true).unwrap();
    assert_eq!(c.stem, "**a**\n**b**");
    // "1." lines inside a paragraph continue and renumber
    let s = "Steps:\n1. a\n2. b";
    let d = doc(s, true);
    let c = enter_op(s, &d, 0, 11, 11, true).unwrap();
    assert_eq!(c.stem, "Steps:\n1. a\n2. \n3. b");
    assert_eq!(c.caret, 15);
    // fields that take no line breaks ignore Enter
    let d = doc("x", false);
    assert_eq!(enter_op("x", &d, 0, 1, 1, false), None);
}

#[test]
fn list_toggles() {
    let s = "one\ntwo\nthree";
    let d = doc(s, true);
    let c = toggle_list(s, &d, 0, (0, 7), ListKind::Bullets).unwrap();
    assert_eq!(c.stem, "• one\n• two\nthree");
    let d = doc(&c.stem, true);
    // bullets -> numbered converts the whole list
    let c2 = toggle_list(&c.stem, &d, 1, (2, 2), ListKind::Numbered).unwrap();
    assert_eq!(c2.stem, "1. one\n2. two\nthree");
    // same kind again lifts the item
    let d = doc(&c2.stem, true);
    let c3 = toggle_list(&c2.stem, &d, 1, (3, 3), ListKind::Numbered).unwrap();
    assert_eq!(c3.stem, "one\n1. two\nthree");
    // numbered under a paragraph line: a blank line keeps it a list
    let s = "Intro\n- a\n- b";
    let d = doc(s, true);
    let c = toggle_list(s, &d, 1, (8, 8), ListKind::Numbered).unwrap();
    assert_eq!(c.stem, "Intro\n\n1. a\n2. b");
    assert!(matches!(richtext::parse(&c.stem)[1], richtext::Block::Numbered(_)));
    assert_eq!(slice(&c.stem, c.caret..c.caret + 1), "a");
}

#[test]
fn tables_insert_edit_rows_columns() {
    let s = "Name these.";
    let d = doc(s, true);
    let c = insert_table(s, &d, 0, 11, true);
    assert_eq!(c.stem, "Name these.\n| Column 1 | Column 2 |\n|---|---|\n| ___ | ___ |\n| ___ | ___ |");
    assert_eq!(slice(&c.stem, c.anchor..c.caret), "Column 1");
    assert!(matches!(&richtext::parse(&c.stem)[1], richtext::Block::Table { header: true, rows } if rows.len() == 3));
    let d = doc(&c.stem, true);
    let bi = d.blocks.iter().position(|b| matches!(b.kind, BlockKind::Table { .. })).unwrap();
    // add a row below the header and a column, then delete them again
    let a = table_op(&c.stem, &d, bi, 0, 0, TOp::RowBelow).unwrap();
    let da = doc(&a.stem, true);
    assert_eq!(table_cells(&a.stem, &da, bi).len(), 4);
    let b = table_op(&a.stem, &da, bi, 1, 0, TOp::DelRow).unwrap();
    let db = doc(&b.stem, true);
    assert_eq!(table_cells(&b.stem, &db, bi), table_cells(&c.stem, &d, bi));
    let e = table_op(&b.stem, &db, bi, 0, 1, TOp::ColRight).unwrap();
    let de = doc(&e.stem, true);
    assert_eq!(table_cells(&e.stem, &de, bi)[0], vec!["Column 1", "Column 2", ""]);
    let f = table_op(&e.stem, &de, bi, 0, 2, TOp::DelCol).unwrap();
    let df = doc(&f.stem, true);
    assert_eq!(table_cells(&f.stem, &df, bi), table_cells(&c.stem, &d, bi));
    // never below 2 rows / 2 columns
    assert_eq!(table_op(&f.stem, &df, bi, 0, 0, TOp::DelCol), None);
    // the implied header (blanks below) cannot be switched off
    let s2 = "A | B\n___ | ___";
    let d2 = doc(s2, true);
    let tb = d2.blocks.iter().position(|b| matches!(b.kind, BlockKind::Table { .. })).unwrap();
    assert!(!header_toggle_ok(s2, &d2, tb));
    // emptying an edge cell keeps the row (the table is rewritten with outer pipes)
    let s3 = "a | b\nc | d";
    let d3 = doc(s3, true);
    let t3 = d3.blocks.iter().position(|b| matches!(b.kind, BlockKind::Table { .. })).unwrap();
    let n3 = cell_edit(s3, &d3, t3, 0, "").unwrap();
    let nd3 = doc(&n3, true);
    let t3b = nd3.blocks.iter().position(|b| matches!(b.kind, BlockKind::Table { .. })).unwrap();
    assert_eq!(table_cells(&n3, &nd3, t3b), vec![vec!["", "b"], vec!["c", "d"]]);
    // a typed pipe stays inside the cell
    let n4 = cell_edit(s3, &d3, t3, 1, "x|y").unwrap();
    assert_eq!(n4, "a | x∣y\nc | d");
    // delete table
    let g = table_op(&f.stem, &df, bi, 0, 0, TOp::DelTable).unwrap();
    assert_eq!(g.stem, "Name these.");
}

#[test]
fn property_split_line_keeps_text_and_styles() {
    let mut r = Rng(77);
    for _ in 0..10_000 {
        let s = markup(&mut r, 14).replace('\n', " ");
        let n = clen(&s);
        let p = r.below(n + 1);
        let (l, rr) = split_line(&s, p, p);
        let (i0, _) = decode_line(&s);
        let mut both = decode_line(&l).0;
        both.extend(decode_line(&rr).0);
        let joined = format!("{l}{rr}");
        assert!(same_items(&both, &i0) || joined == s, "{s:?} at {p}: {l:?} | {rr:?}");
    }
}

#[test]
fn check_refuses_what_it_cannot_show() {
    assert!(check("a\r\nb").is_err());
    assert!(check(&"x".repeat(20_001)).is_err());
    assert!(check("plain").is_ok());
    assert!(check("Formula | Name\nKI | ___\n• a\n1. x\n2. y\n> q\n$x^2$ **b**").is_ok());
}

#[test]
fn property_structure_ops_never_panic_and_stay_editable() {
    let mut r = Rng(0xfeed_beef);
    for _ in 0..4_000 {
        let s = stem(&mut r);
        let d = doc(&s, true);
        let flat = d.flat();
        let fi = r.below(flat.len());
        let u = d.unit(flat[fi]).clone();
        let n = u.end - u.start;
        let (a, b) = (r.below(n + 1), r.below(n + 1));
        let (ga, gb) = (d.global(&u, a), d.global(&u, b));
        let text = slice(&s, u.start..u.end);
        let mut outs: Vec<String> = Vec::new();
        if let Some(c) = enter_op(&s, &d, fi, a.min(b), a.max(b), true) {
            outs.push(c.stem);
        }
        if let Some(c) = backspace_op(&s, &d, fi) {
            outs.push(c.stem);
        }
        if let Some(c) = delete_op(&s, &d, fi) {
            outs.push(c.stem);
        }
        for k in [ListKind::Bullets, ListKind::Numbered, ListKind::Quote] {
            if let Some(c) = toggle_list(&s, &d, fi, (ga, gb), k) {
                outs.push(c.stem);
            }
        }
        outs.push(insert_table(&s, &d, fi, gb, r.below(2) == 0).stem);
        if u.virt.is_none() {
            for op in [IOp::Bold, IOp::Italic, IOp::Sub, IOp::Sup, IOp::Clear] {
                if let Some((t, x, y)) = inline_op(&text, a, b, op) {
                    assert!(x <= y && y <= clen(&t), "{text:?} {op:?} -> {t:?} {x} {y}");
                    outs.push(apply_unit(&s, &u, &t));
                }
            }
            if let Some((t, caret, bs, _)) = make_blank(&text, a, b) {
                assert_eq!(slice(&t, bs..bs + 3), "___", "{text:?} -> {t:?}");
                assert!(caret <= clen(&t));
                outs.push(apply_unit(&s, &u, &t));
            }
        }
        if let Some(bi) = d.blocks.iter().position(|b| matches!(b.kind, BlockKind::Table { .. })) {
            for op in [TOp::RowAbove, TOp::RowBelow, TOp::ColLeft, TOp::ColRight, TOp::DelRow, TOp::DelCol, TOp::Header, TOp::DelTable] {
                if let Some(c) = table_op(&s, &d, bi, 0, 0, op) {
                    if let Some(k) = &c.keep {
                        assert_eq!(k.len(), blank_starts(&c.stem).len());
                    }
                    outs.push(c.stem);
                }
            }
        }
        for o in outs {
            assert_eq!(check(&o), Ok(()), "{s:?} -> {o:?}");
            let a = analyze(&o, &ok_math);
            assert_eq!(a.role.len(), clen(&o));
        }
    }
}

#[test]
fn typing_mode_styles_only_the_typed_text() {
    let bold = Style { b: true, ..Style::default() };
    // Ctrl+B with the caret after "hello": the word is left alone, the typed text is bold
    let (t, c) = style_typed("hello world", "hellox world", 5, bold).unwrap();
    assert_eq!(t, "hello**x** world");
    assert_eq!(slice(&t, 0..c), "hello**x");
    // continuing to type at that caret stays bold by inheritance
    let t2 = splice(&t, c..c, "y");
    assert_eq!(decode_line(&t2).0.iter().filter(|it| matches!(it, Item::Ch { b: true, .. })).count(), 2);
    // a caret inside a word: still only the typed text
    assert_eq!(style_typed("make it red", "make it rXed", 9, bold).unwrap().0, "make it r**X**ed");
    // italic cannot open right after a letter: the typed text is left as typed (None)
    assert_eq!(style_typed("make it red", "make it rXed", 9, Style { i: true, ..Style::default() }), None);
    // turning bold off after a bold word: the typed text is plain
    let (t, c) = style_typed("**ab**", "**abx**", 4, Style::default()).unwrap();
    assert_eq!(t, "**ab**x");
    assert_eq!(c, 7);
    // subscript and strikethrough
    assert_eq!(style_typed("H O", "H2 O", 1, Style { sub: true, ..Style::default() }).unwrap().0, "H_{2} O");
    assert_eq!(style_typed("a b", "a old b", 2, Style { s: true, ..Style::default() }).unwrap().0, "a ~~old~~ b");
    // spaces only, or not an insert at the caret: nothing to do
    assert_eq!(style_typed("ab", "a b", 1, bold), None);
    assert_eq!(style_typed("ab", "xab", 1, bold), None);
    assert_eq!(style_typed("ab", "a", 1, bold), None);
}

#[test]
fn style_toggles_like_the_buttons() {
    let s = Style::default().toggled(IOp::Bold);
    assert!(s.b && !s.i);
    assert!(!s.toggled(IOp::Bold).b);
    let s = Style { sup: true, ..Style::default() }.toggled(IOp::Sub);
    assert!(s.sub && !s.sup);
    assert_eq!(Style { b: true, s: true, ..Style::default() }.toggled(IOp::Clear), Style::default());
}

#[test]
fn strikethrough_roundtrips_through_the_editor_model() {
    assert_eq!(inline_op("the old one", 4, 7, IOp::Strike).unwrap().0, "the ~~old~~ one");
    assert_eq!(inline_op("the ~~old~~ one", 6, 9, IOp::Strike).unwrap().0, "the old one");
    // with bold and italic
    let t = inline_op("**a b**", 2, 5, IOp::Strike).unwrap().0;
    let (items, _) = decode_line(&t);
    assert!(items.iter().filter(|it| !it.is_ws()).all(|it| matches!(it, Item::Ch { b: true, s: true, .. })), "{t:?}");
    assert_eq!(inline_op("~~**x**~~ y", 0, 11, IOp::Clear).unwrap().0, "x y");
    // random lines with strike markers decode/encode back to the same items
    let mut r = Rng(5);
    let tok = ["a", "b", " ", "~~", "~~x~~", "**", "*", "_{", "}", "2", "word"];
    let mut ok = 0;
    for _ in 0..10_000 {
        let n = r.below(12);
        let s: String = (0..n).map(|_| r.pick(&tok)).collect();
        let (items, _) = decode_line(&s);
        if let Some((e, _)) = encode_checked(&items) {
            ok += 1;
            assert!(same_items(&decode_line(&e).0, &items), "{s:?} -> {e:?}");
        }
    }
    assert!(ok > 5_000, "{ok}");
}

#[test]
fn floating_bar_never_covers_the_fixed_toolbar() {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
    let fixed = Rect::from_min_max(Pos2::new(380.0, 138.0), Pos2::new(820.0, 172.0));
    let size = Some(Vec2::new(280.0, 34.0));
    // selection on the first line, right under the fixed toolbar: the bar goes below it
    let sel = Rect::from_min_max(Pos2::new(382.0, 186.0), Pos2::new(430.0, 204.0));
    let (at, pivot) = float_place(sel, size, Some(fixed), screen);
    assert_eq!(pivot, egui::Align2::LEFT_TOP);
    let bar = Rect::from_min_size(at, size.unwrap());
    assert!(!bar.intersects(fixed) && bar.top() > sel.bottom());
    // further down there is room above
    let sel = Rect::from_min_max(Pos2::new(382.0, 300.0), Pos2::new(430.0, 318.0));
    let (at, pivot) = float_place(sel, size, Some(fixed), screen);
    assert_eq!(pivot, egui::Align2::LEFT_BOTTOM);
    assert!(at.y < sel.top());
    // no fixed toolbar (answer fields): above unless that leaves the window
    let sel = Rect::from_min_max(Pos2::new(10.0, 12.0), Pos2::new(40.0, 30.0));
    assert_eq!(float_place(sel, size, None, screen).1, egui::Align2::LEFT_TOP);
}
