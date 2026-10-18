# Stem markup

A question stem is a plain string. These conventions (parsed by `src/richtext.rs`, the single
parser every surface uses) give it structure on the exam page, in PDF/DOCX exports, in the answer
key and in previews. A stem without any of them is one paragraph on one line, as before.

| You write | You get |
|---|---|
| a line break (`\n`) | a hard line break; a blank line starts a new paragraph |
| `• item`, `- item`, `* item` lines | a bullet list |
| two or more `1. item` lines | a numbered list |
| `> text` lines | a quote |
| two or more lines with the same number of ` \| `-separated cells | a table; the first row is a header when a `---\|---` line follows it, or when it has no `___` blanks but later rows do |
| `**bold**`, `*italic*` | bold, italic (a spaced `a * b` stays literal) |
| `~~struck~~` | strikethrough; the opening `~~` must touch the next character and needs a closing `~~` touching the previous one, so a stray `~~` or `a ~~ b` stays literal |
| `_{2}`, `^{2}` | subscript, superscript (digits and signs print as Unicode where they can) |
| `$x^2$` | typeset math (the equation editor) |
| `[text](https://...)` | a link: `text` shows blue and underlined in the editor and the preview, prints blue and underlined with a clickable link annotation in the PDF, and is a real hyperlink in the DOCX. Only `http://`, `https://` and `mailto:` addresses count; the text must be non-empty and hold no `[`, `]`, `$`, `{` or `}`, and the address no spaces or parentheses. Anything else (`[x](ftp://...)`, `[x] (https://...)`, an unclosed bracket) stays literal |
| `___` | a blank (unchanged) |
| `# Title`, `## Title`, `### Title` lines | a heading (bold, larger; level 1 is largest). The `#`s must start the line and be followed by a space; a line holding a `___` blank stays text (so `# of protons: ___` is not a heading) |
| a line that is only `---` | a divider (a thin rule across the column) |
| `::: side` ... `::: next` ... `:::` lines | side by side: the blocks between the fence lines are panels laid out in columns (one more panel per `::: next`). Panels hold any block except another side-by-side group. Without the closing `:::` the fence lines are plain text |

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

Previews that need one line (bank cards, lists, search) use `richtext::plain`.

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
stem holding one must keep reading that way, so there is no deeper heading syntax. Imported
level 4-6 headings (HTML `<h4>`..`<h6>`, Test Parrot `heading` nodes with level 4-6) are a line
of their own styled so each level prints distinct from level 3 and from the others: level 4
**bold**, level 5 ***bold italic***, level 6 *italic* (body size; level 3 is 1.07x bold). Text
that already carries `*` markers, or several lines, keeps its own styling.

Task (checkbox) lists import as bullet lists whose items start with their box: `• ☑ done` for a
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
  Heading 1-3, Quote, Divider, Bullet list, Numbered list, Image placeholder, Table, Math and Side
  by side, in Text / List / Advanced groups. Letters typed after the `/` filter it (`/si` leaves
  Side by side); Up/Down move, Enter or Tab or a click chooses, Esc (or a space) leaves the `/` as
  typed text. The `/query` text is removed and the command applies to the caret's line. In a
  table cell only Math and Image placeholder are offered, in a side-by-side panel everything
  but another side by side. A `/` in the middle of a word is always a literal slash. In
  one-line answer fields `/` at a word start still opens the equation editor directly.
* Headings edit in place (Enter splits them, Backspace at the start turns one back into text), a
  divider shows a × to remove it (or Backspace at the start of the line below it), and a
  side-by-side group draws its panels as dashed boxes with `+ Panel`, `Stack` (unwrap into the
  panels' contents one after another) and a × per panel. Fence lines are never shown or edited
  as text in the visual editor; the `</>` markup view shows them.

Shortcuts: Ctrl+B bold, Ctrl+I italic, Ctrl+K link, Ctrl+Shift+S strikethrough, Ctrl+, subscript, Ctrl+. superscript, Ctrl+\ clear,
Ctrl+Shift+8 or Ctrl+Alt+8 bullets, Ctrl+Shift+7 or Ctrl+Alt+7 numbered, Ctrl+Shift+B quote,
Ctrl+M equation (`/` at the start of a word opens the slash menu), Ctrl+Shift+K blank / unblank, Ctrl+Z undo,
Ctrl+Shift+Z or Ctrl+Y redo, Tab / Shift+Tab between table cells.

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
  tables, headings, dividers and fences are never joined.
* **Tables.** The label-repeating table of blanks (`KI | Ionic or Covalent? ___ | Name ___`, or a
  header row carrying `___`) becomes a clean table, with the same rules the AI import uses
  (`ditto::normalize_tables`).

Fill-in-the-blank answers stay one per `___`: when a header row's stray blanks are removed, the
matching answer entries go too (the import's trailing padding, or empty entries at those
positions, or answers grouped by row are flattened). When the answers can't say which blanks
were the header's, the table is left alone and the dialog says so. Multipart parts are tidied
one by one and keep their order and kinds. A stem already in the current markup is never
changed (property-tested).
