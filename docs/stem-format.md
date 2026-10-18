# Stem markup

A question stem is a plain string. These conventions (parsed by `src/richtext.rs`, the single
parser every surface uses) give it structure on the exam page, in PDF/DOCX exports, in the answer
key and in previews. A stem without any of them is one paragraph on one line, as before.

| You write | You get |
|---|---|
| a line break (`\n`) | a hard line break; a blank line starts a new paragraph |
| `• item`, `- item`, `* item` lines | a bullet list |
| two or more `1. item` lines | a numbered list |
| list items indented by two spaces per level (`  • child` under `• parent`) | a nested list (round 7): bullets •, ◦, ▪ and numbers 1. / a. / i. by level |
| `• ☐ item`, `• ☑ item` | a task (checkbox) list: a bullet list whose items start with their box |
| `> text` lines | a quote |
| two or more lines with the same number of ` \| `-separated cells | a table; the first row is a header when a `---\|---` line follows it, or when it has no `___` blanks but later rows do |
| `**bold**`, `*italic*` | bold, italic (a spaced `a * b` stays literal) |
| `~~struck~~` | strikethrough; the opening `~~` must touch the next character and needs a closing `~~` touching the previous one, so a stray `~~` or `a ~~ b` stays literal |
| `_{2}`, `^{2}` | subscript, superscript (digits and signs print as Unicode where they can) |
| `$x^2$` | typeset math (the equation editor) |
| `` `code` `` | inline code (round 7): a monospace face on a light tint; the text between the backticks is literal (no bold, math or links inside). The code is non-empty, has no space next to either backtick and holds no backtick and no `___`; anything else (a lone backtick, `` a ` b ` ``) stays literal |
| `[text](https://...)` | a link: `text` shows blue and underlined in the editor and the preview, prints blue and underlined with a clickable link annotation in the PDF, and is a real hyperlink in the DOCX. Only `http://`, `https://` and `mailto:` addresses count; the text must be non-empty and hold no `[`, `]`, `$`, `{` or `}`, and the address no spaces or parentheses. Anything else (`[x](ftp://...)`, `[x] (https://...)`, an unclosed bracket) stays literal |
| `___` | a blank (unchanged) |
| `# Title`, `## Title`, `### Title` lines | a heading (bold, larger; level 1 is largest). The `#`s must start the line and be followed by a space; a line holding a `___` blank stays text (so `# of protons: ___` is not a heading) |
| a line that is only `---` | a divider (a thin rule across the column) |
| `::: side` ... `::: next` ... `:::` lines | side by side: the blocks between the fence lines are panels laid out in columns (one more panel per `::: next`). Panels hold any block except another side-by-side group. Without the closing `:::` the fence lines are plain text |
| `::: center` ... `:::` lines | centred (round 7): the paragraphs and headings between the fence lines are centred. A centred group may sit in a side-by-side panel; it holds no side-by-side group, no `::: next` and no other centred group (then, or without its closing `:::`, the `::: center` line is plain text) |

```
::: side
Left panel, may hold **lists**, tables, math...
• one
::: next
Right panel
:::
```

Headings, dividers and side-by-side groups were added in round 6. A stem that uses none of them
(the parser's `richtext::uses_extensions` is false) is cut into exactly the blocks it was before,
so it looks the same in the editor, the preview, the PDF and the DOCX; property tests compare the
block structure of random stems with the previous parser.

Links were added in P3. A line without a valid link is cut into exactly the runs it was before
(a property test compares `inline_runs` with a frozen copy of the pre-link run cutter on random
lines full of near misses, and `parse` with the pre-link parse for every stem without a link),
so stems that do not use the syntax look the same everywhere. A stem written before P3 that
happens to contain `[text](https://...)` literally now shows it as a link.

Inline code, centring, list nesting, task lists and Heading 4-6 in the editor were added in
round 7 (see below). For renderers, `richtext::parse` is unchanged: a centred group's blocks
stand in its place and nested items are plain items of their list, so a renderer that does
not know them prints the text left-aligned and flat (never the fence lines). `parse_tree`
returns the same blocks with their layout (`Node::Block { block, center, depths }` and
`Node::Side`), `item_depths` / `item_ordinals` / `number_label` / `bullet_glyph` give the
nesting, and `task_item` tells a task item (its box replaces the bullet on screen).
`richtext::uses_r7` is false for every stem that uses none of it; for those `segment`, `parse`
and every line's `inline_runs` equal frozen copies of the round-6 parser and the P3 run cutter
(property test over 80 000 random stems with near misses: lone and spaced backticks,
`::: centre`, unclosed fences, odd indentation), and every stem in the fixture stores and the
real store (68) reads exactly as before. The PDF and DOCX of all 46 fixture exams are
byte-identical to the round-6 build. Indented list items were already written by the HTML and
Test Parrot importers for nested lists; they now show nested on screen.

**In print (round 7, `src/export.rs`).** The PDF and DOCX render from `parse_tree`:

* `` `code` `` prints in the mono, Liberation Mono at 0.92 of the text size on a light gray tint
  (PDF font F10, embedded only when a document has code; characters beyond WinAnsi go through
  the serif's CID font), and in DOCX as Courier New runs (font table: Liberation Mono as its
  alternate) at the same 0.92 size with `<w:shd w:fill="EBEBEB"/>`.
* A centred group centres each wrapped line of its paragraphs and headings in the width that
  line wrapped to (PDF x offset; DOCX `<w:jc w:val="center"/>`), and a table in it centres in
  the column (DOCX `jc` center in `tblPr`). Lists, quotes and dividers in a group stay where
  they are, as on screen. When a stem opens with a centred block, the DOCX puts the question
  number on its own line first.
* Nested items indent 16 pt per level with that level's bullet or number (•, ◦, ▪; 1. / a. /
  i.). The DOCX uses `w:ilvl` with multilevel bullet and decimal / lowerLetter / lowerRoman
  lists, each level 320 twips further in; a document with no nesting keeps the single-level
  numbering part byte for byte.
* A task item prints its box without a bullet in front, the box where the item's text starts
  (DOCX: a plain paragraph at the bullet's text indent).

Stems without round-7 markup print byte-identically (all 46 fixture exams, exam, key and
DOCX in three key layouts), except that a task list's items lose their bullet.

Previews that need one line (bank cards, lists, search) use `richtext::plain`.

Centring and code in the importers (round 7): Test Parrot paragraphs and headings with
`"align": "center"` (its export; `attrs.align` / `attrs.textAlign` too) and HTML `<center>`,
`align="center"` or `text-align: center` on a paragraph-like block become a centred group;
Test Parrot `inline-code` marks (`inlineCode`, `code` too) and HTML `<code>`, `<kbd>`, `<samp>`,
`<tt>` become `` `code` `` when the markup can hold it (else plain text). A Test Parrot list item
with a boolean `checked` is a task item.

Links in the importers: HTML `<a href="...">` (every HTML source shares one converter) and Test
Parrot `link` marks become `[text](url)` when the address is http, https or mailto (spaces and
parentheses in it are percent-encoded); relative, `javascript:` and other links keep only their
text, as before.

Strikethrough prints as a line through the text in the PDF and as `<w:strike/>` in DOCX. The
importers produce it from HTML `<s>`, `<del>`, `<strike>` and `text-decoration: line-through`
spans (Moodle XML, QTI 1.2/2.1, Canvas, Blackboard, Anki and every other HTML source share one
converter) and from Test Parrot `strike` marks.

Headings print bold at 1.4x / 1.2x / 1.07x the body size (PDF and DOCX), a divider is a gray
rule (a bottom-bordered empty paragraph in DOCX), and a side-by-side group is a set of equal
columns that never splits across a page (a borderless one-row table in DOCX). HTML `<h1>`..`<h6>`
and `<hr>`, and Test Parrot `heading`, `rule` and `side-by-side` nodes, import as these blocks
(see below for `<h4>`..`<h6>`).

Stem markup has headings of levels 1-3 only: a `####` line has always been plain text, and a
stem holding one must keep reading that way, so there is no deeper heading syntax. The slash
menu's Heading 4-6 (round 7) write the same styled line as imported ones. Imported
level 4-6 headings (HTML `<h4>`..`<h6>`, Test Parrot `heading` nodes with level 4-6) are a line
of their own styled so each level prints distinct from level 3 and from the others: level 4
**bold**, level 5 ***bold italic***, level 6 *italic* (body size; level 3 is 1.07x bold). Text
that already carries `*` markers, or several lines, keeps its own styling.

Task (checkbox) lists (round 7: also the slash menu's Task list) are bullet lists whose items
start with their box. They import as such: `• ☑ done` for a
ticked item, `• ☐ open` otherwise (Test Parrot `task-list` / `task-item` nodes, also spelled
`taskList` / `taskItem`, with `checked` on the item or in its `attrs`; HTML `<input
type="checkbox">` in a list item, or TipTap's `<li data-checked>`). They stay ordinary
markup (no new syntax), and the boxes print in the PDF through Source Serif 4's own glyphs
(see `docs/fonts.md`).

## Editing it visually

The question editor (`src/wysiwyg.rs`) edits this markup in place: markers are hidden, bold,
italic, strikethrough, scripts and `$math$` show as they print, lists and quotes are blocks, tables are grids of
cells, and `___` shows as a blank (in Fill-in-the-blank questions as a numbered chip with its answer).
The stored stem is still exactly this text; text you do not touch is never rewritten. A few things
the markup itself decides:

* A numbered list needs a blank line above it when a paragraph line comes first (`1.` lines that
  continue a paragraph print as plain lines). The list buttons add that blank line for you.
* A `|` typed into a table cell is stored as `∣` (it would otherwise split the cell); a table
  changed with the row/column buttons is written back with outer pipes (`| a | b |`) so empty cells
  survive.
* Text the editor cannot show safely (control characters such as Windows line endings, or more
  than 20 000 characters) opens as markup with a notice; the `</>` button switches to markup at any
  time.
* With a collapsed caret (nothing selected) Bold, Italic, Strikethrough, the scripts and Clear do
  not touch the word at the caret: they set the style for the next text typed there (the button
  shows it as on), and moving the caret drops it.
* Answer, model answer, explanation and blank-answer fields use the same editor (inline markup
  only), so formatting there shows as it prints on the answer key.
* **Link** (toolbar, floating bar, Ctrl+K) opens a small popover for the address: with text
  selected it links that text (`example.org` becomes `https://example.org`, an e-mail address
  `mailto:`), with the caret in a link it shows the link's address to change, plus **Remove**
  (keeps the text). A bare caret inserts the address as its own link text. A selection across
  lines, through an equation or holding `[ ]` cannot be a link and the popover says so. Inside a
  link the formatting buttons do nothing (they would break the link); format the text first or
  select the whole link.
* Typing `/` at the start of a word (in a stem) opens the slash menu at the caret: Text,
  Heading 1-6, Quote, Divider, Bullet list, Numbered list, Task list, Image placeholder, Table,
  Math and Side by side, in Text / List / Advanced groups. Heading 4-6 make the line bold /
  bold italic / italic (an empty line gets "Heading 4" selected to type over); Task list makes
  the line an open task item. Letters typed after the `/` filter it (`/si` leaves
  Side by side); Up/Down move, Enter or Tab or a click chooses, Esc (or a space) leaves the `/` as
  typed text. The `/query` text is removed and the command applies to the caret's line. In a
  table cell only Math and Image placeholder are offered, in a side-by-side panel everything
  but another side by side. A `/` in the middle of a word is always a literal slash. In
  one-line answer fields `/` at a word start still opens the equation editor directly.
* **Code** (toolbar and floating bar, Ctrl+E) makes the selected text inline code, or takes
  the code out of code it touches. Only plain text on one line can become code (no equation,
  link, blank or hidden marker inside); with a bare caret it does nothing, as in the
  reference. The editor keeps a code span whole, like a link: formatting never cuts into it.
* **Centre** (toolbar and floating bar, Ctrl+Shift+E) centres the caret's paragraph (every
  paragraph the selection touches) or heading by wrapping it in `::: center` ... `:::`; again
  takes the centring off the whole group. Enter at the end of a centred group's last line
  starts a plain paragraph under it. Lists, tables and dividers are not centred.
* **Nested lists**: Tab in a bullet or numbered item moves it (and the items under it) one
  level in, Shift+Tab one level out. The first item cannot go in, and an item goes at most one
  level deeper than the item above it (5 levels). Enter on an empty nested item, or Backspace
  at its start, moves it out a level. Numbers count per level and are written back so (`1.`
  `1.` `2.` `2.`); they show as 1. / a. / i. Bullets and numbers cannot mix within one list.
* **Task items**: click a box (or Ctrl+Enter in the item) to tick or untick it; Enter
  continues with an open box; Enter on an empty task item ends the list.
* **Block rail** (stems): hovering a top-level block shows `+` and a drag grip in the gutter at
  its left, level with its first line. `+` adds a paragraph under the block and opens the slash
  menu there (Esc leaves the `/`); clicking the grip selects the block (a paragraph or heading
  whole, else the caret goes to its start); dragging the grip moves the block, with a line
  showing where it will land. Alt+Up / Alt+Down move the caret's block the same way from the
  keyboard (the slash menu is the keyboard way to add one). A move adds a blank line only where
  the blocks around a seam would otherwise read as one (two paragraphs, a numbered list under a
  paragraph); every block reads back exactly as it was. A group (side by side, centred) moves
  whole; blocks do not move into or out of a panel.
* A table has its own control row above the grid (+Row / +Col / −Row / −Col / Header /
  Delete while a cell has the caret), so the controls never cover the text above the table or
  its next row.
* Headings edit in place (Enter splits them, Backspace at the start turns one back into text), a
  divider shows a × to remove it (or Backspace at the start of the line below it), and a
  side-by-side group draws its panels as dashed boxes with `+ Panel`, `Stack` (unwrap into the
  panels' contents one after another) and a × per panel. Fence lines are never shown or edited
  as text in the visual editor; the `</>` markup view shows them.

Shortcuts: Ctrl+B bold, Ctrl+I italic, Ctrl+K link, Ctrl+Shift+S (or Ctrl+Alt+X) strikethrough, Ctrl+E inline code, Ctrl+Shift+E centre, Ctrl+, subscript, Ctrl+. superscript, Ctrl+\ clear,
Ctrl+Shift+8 or Ctrl+Alt+8 bullets, Ctrl+Shift+7 or Ctrl+Alt+7 numbered, Ctrl+Shift+B quote,
Ctrl+M equation (`/` at the start of a word opens the slash menu), Ctrl+Shift+K blank / unblank, Ctrl+Z undo,
Ctrl+Shift+Z or Ctrl+Y redo, Tab / Shift+Tab between table cells and to nest list items,
Ctrl+Enter tick a task item, Alt+Up / Alt+Down move the block.

## Tidying stems from older imports

Banks imported before stem rendering existed can hold stems in shapes the parser no longer
writes. The bank page's `…` menu has **Tidy stems…** (`src/tidy.rs`, pure logic): it lists every
question it would change with the stem before and after (rendered as it prints), a tick per
question, and **Apply**. Nothing runs on its own. Apply first copies `banks.json` to
`backups/banks-<date>-<time>.json` in the store (never overwriting an earlier backup); the bank
page then shows an Undo strip that puts the questions back exactly as they were.

What it fixes, repeated until nothing changes (so a second run finds nothing):

* **Line breaks.** Old exports flattened `\n` into ` — `. In a stem with no line break, a ` — `
  becomes a line break when the piece after it reads as its own line (a list item, a table row,
  anything with a `___` blank), or when the piece before it is such a line or ends a sentence
  and the next piece starts with a capital. A dash inside a sentence (`water — a polar
  molecule — boils`) stays.
* **Spacing.** Windows line endings, trailing spaces, runs of empty lines (one stays) and empty
  lines at the start or end.
* **Wrapped lines.** A prose or list line of 40+ characters that ends without punctuation,
  followed by a prose line that starts in lowercase, is rejoined with a space. Lines with blanks,
  tables, headings, dividers and fences (`::: side`, `::: next`, `::: center`, `:::`) are never
  joined, and nor is a line that ends in a `` `code` `` span.
* **Tables.** The label-repeating table of blanks (`KI | Ionic or Covalent? ___ | Name ___`, or a
  header row carrying `___`) becomes a clean table, with the same rules the AI import uses
  (`ditto::normalize_tables`).

Fill-in-the-blank answers stay one per `___`: when a header row's stray blanks are removed, the
matching answer entries go too (the import's trailing padding, or empty entries at those
positions, or answers grouped by row are flattened). When the answers can't say which blanks
were the header's, the table is left alone and the dialog says so. Multipart parts are tidied
one by one and keep their order and kinds. A stem already in the current markup is never
changed (property-tested). Round-7 markup is current markup: nested list items keep their
indentation, centre fences stay on their own lines, and a ` — ` inside a code span is code,
not a flattened line break.
