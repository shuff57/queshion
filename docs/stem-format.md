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
| `___` | a blank (unchanged) |

Previews that need one line (bank cards, lists, search) use `richtext::plain`.

Strikethrough prints as a line through the text in the PDF and as `<w:strike/>` in DOCX. The
importers do not produce it (struck text in an imported file comes in as plain text).

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

Shortcuts: Ctrl+B bold, Ctrl+I italic, Ctrl+Shift+S strikethrough, Ctrl+, subscript, Ctrl+. superscript, Ctrl+\ clear,
Ctrl+Shift+8 or Ctrl+Alt+8 bullets, Ctrl+Shift+7 or Ctrl+Alt+7 numbered, Ctrl+Shift+B quote,
Ctrl+M (or `/` at the start of a word) equation, Ctrl+Shift+K blank / unblank, Ctrl+Z undo,
Ctrl+Shift+Z or Ctrl+Y redo, Tab / Shift+Tab between table cells.
