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
| `_{2}`, `^{2}` | subscript, superscript (digits and signs print as Unicode where they can) |
| `$x^2$` | typeset math (the equation editor) |
| `___` | a blank (unchanged) |

Previews that need one line (bank cards, lists, search) use `richtext::plain`.
