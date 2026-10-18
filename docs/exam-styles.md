# Exam paper styles, margins, sections and work space

Format > Paper style (also the "Paper" control on the format bar) picks one of four printed
looks. The style is stored per exam (`Exam.paper_style`, serde default `Standard`, so exams
saved before it existed load unchanged) and is honoured by the page preview, the PDF and the
DOCX export, and by the regenerable `.doc.json` snapshots.

The four styles and their one-line descriptions come from Test Parrot's Format > Paper style
menu (observed on testparrot.com, 2026-10-17, in a throwaway browser session with the
"Quiz 5A - Ch. 4" exam imported). What each one changes was read off Test Parrot's page
preview in each style; where verSHion differs, the row says so.

| | Standard | Classic | Condensed | Exam Board |
|---|---|---|---|---|
| Test Parrot blurb | Circle the letter or T/F; no blanks before numbers. | A blank before each objective number; ruled lines for written answers. | Saves paper: tighter spacing, answers across where they fit, closer-ruled lines. | A4: 1 (a) (i) labels, dotted lines, points in brackets at the right. |
| Page size | Letter | Letter | Letter | A4 (595 x 842 pt) |
| Question number | `3.` | `3.`, with a `______` answer blank in front of multiple-choice and true/false numbers | `3.` | `3` (no dot) |
| Choice letters | `A.` | `a.` (lowercase) | `A.` | `A` (no dot) |
| Multipart labels | `a.` | `a.` | `a.` | `(a)` |
| Choice layout | one per line | one per line | across in 2-4 columns when every choice fits its column | one per line |
| Answer lines (short answer, fill-in, multipart) | ruled | ruled | ruled, closer (1.55 x body instead of 2.0) | dotted |
| Work space | blank room | ruled lines | blank room, closer pitch | dotted lines |
| Points | `(2 points)` at the right of the first line | same | same | `[2]` at the right |
| Spacing | 1.42 x body leading, 0.6 x body after each question | same | 1.25 x leading, 0.3 x body after each question | same as Standard |
| Page foot | page number (preview) | same | same | "Turn over" at the foot of every page but the last |
| Typeface | Source Serif 4 (PDF, preview); Times New Roman / Liberation Serif (DOCX) | same | same | same |

Deviation from Test Parrot: in Standard, Test Parrot leaves a short-answer question with no
work space bare (no lines at all) and only ruled lines appear in Classic. verSHion keeps its
existing default of three ruled lines under a short answer in every style, so exams printed
before paper styles existed print exactly as they did; the work space handle adds room on top.

Every style prints in the same serif as Test Parrot's printed look, with real bold, italic and
bold italic faces: Source Serif 4 embedded in the PDF and drawn by the page preview, and
Times New Roman (alternate: Liberation Serif) in the DOCX. The fonts and their licences are
described in `docs/fonts.md`. The switch from Helvetica changed only the typeface; leading,
spacing and rule pitch are still multiples of the body size. A pagination check
(2026-10-18) exported 16 fixture exams (three exams plus an exam with all 29 fixture questions,
in all four styles) as exam and key PDFs. All 32 documents kept their page counts and put
every question on the same page as with Helvetica.

Matching questions keep their two-column layout in every style (only number and letter
formats change). The DOCX cannot draw "Turn over" on all pages but the last (Word headers and
footers have no "not last page" condition), so only the PDF prints it.

## Margins

Format > Margins... (or the "Margins" button on the format bar) opens a small dialog, like
Test Parrot's: "All sides" in inches, or "Set each side" for Top / Right / Bottom / Left, plus
0.5 / 0.75 / 1 / 1.25 inch presets. While it is open the page preview draws dashed red guide
lines at the margins of every page. Uniform margins are stored in `Exam.margin_mm` (the
existing field); per-side margins in `Exam.margins` (serde default `None`).

## Sections

"Insert new section below" on a question's actions menu starts a new section at the next
question. A section has a title and directions (both optional), edited in place on the page,
and printed as a bold title line and italic directions before its first question in the PDF
and DOCX. Numbering continues across sections. Sections are positional: the heading is stored
on the item where the section starts (`ExamItem.section`), so moving a question across a
section boundary moves it into the other section (as on Test Parrot), while removing a
section's first question keeps the section. The hover rail beside a heading moves the whole
section up or down, merges it into the section above, or deletes it with its questions (one
undo step each). Shuffle question order (per question menu, Edit menu, and the export
dialog's shuffled versions) shuffles within each section; headings stay where they are.

## Work space

Each question has `ExamItem.work_lines` (0..=23 lines, Test Parrot's range). Hovering the
bottom edge of a question shows a handle; dragging it sets the line count (the label reads
"N lines of work space"). The actions menu has "Add work space" (3 lines) and "Remove work
space". Work space prints as blank room, or as ruled / dotted lines in Classic / Exam Board.

**Per-part work space (multipart questions).** Each part of a multipart question can have its
own work space, stored on the exam item as `ExamItem.part_work` (one line count per part,
0..=23; serde default empty, and left out of the file when every part has none, so exams saved
before it load and save unchanged). On the exam page each part has its own handle under it
(hover just below the part; the label reads "a. N lines of work space"), and the question's
actions menu has **Work space per part** with an Add / Remove entry per part. In the exports
the question's text is then cut at the parts (`DocQ.text` holds one entry per part and
`DocQ.space_after` the lines under each); the PDF draws each part's space right under it
(ruled or dotted in Classic / Exam Board, blank room otherwise, and counted in keep-together),
the DOCX adds the same empty or ruled paragraphs after the part, and the snapshot `.doc.json`
keeps it. A question with no per-part space is mapped exactly as before (one text entry), so
its pages do not change. The answer key ignores it.

DOCX ruled lines (answer lines, work space, the header blanks) carry a matching `between`
border as well as their bottom border: Word and LibreOffice merge the borders of consecutive
paragraphs that share them, which had printed a run of ruled lines as a single line.

## Per-question answer order

"Shuffle answer order" on a multiple-choice question stores this exam's own order
(`ExamItem.answer_order`, indices into the bank question's answers) instead of editing the
bank question, so other exams using the same question are unaffected. Locked answers keep
their slot. A stale order (the bank question gained or lost answers) falls back to the bank's
order. Edit > Restore bank answer order clears every stored order.

## Answer columns (round 7)

Multiple-choice questions can print their choices in 1, 2 or 4 columns, like Test Parrot's
"Answer format > Answer columns" (observed 2026-10-18: a three-way 1 column / 2 columns /
4 columns choice in each multiple-choice question's actions menu). In verSHion the question's
actions menu on the exam page has an **Answer format** row with the same three-segment switch
(`examui::answer_columns_switch`, shared with the bank Pop-over).

- Stored per exam item as `ExamItem.answer_columns` (serde default 1, not written when 1, so
  exams saved before it load and save unchanged). 1 means the paper style's own layout: one
  choice per line, or Condensed's automatic across-layout.
- 2 and 4 print equal-width columns filled **column-major**, as Test Parrot does: 4 choices in
  2 columns read A C / B D (`layout::choice_grid`). A long choice wraps inside its column, and a
  row never splits across pages.
- The page preview, the PDF (`DocQ.answer_columns`) and the DOCX (a borderless fixed-width
  table) all follow it. True/false and matching questions keep their own layouts.

## Print follows the page (round 7)

The PDF and DOCX now lay the title area and pictures out the way the exam page preview does:

- **Title area.** The header line (Name / Class / Date) at the top with a thin rule under it,
  the title centred, then a muted centred "N questions · M points" line. Continuation pages
  repeat the header line with its rule. Answer keys keep the plain title line.
- **Pictures.** A question's picture prints at the stem's left edge (not centred), at its
  natural size at 96 dpi, scaled down to the stem column and to at most 3 inches tall
  (`layout::picture_size_pt`; the preview draws the same box). When the stem still has a
  `[picture]` marker, the picture prints at the marker (text before, picture, text after), as
  Test Parrot does; otherwise it prints under the stem, before choices and answer lines.
- **Blanks.** A `___` run (three or more underscores) prints at least 4.5 em long, the width the
  preview draws a blank; longer runs keep their length (`export::widen_blanks`). Plain stems with
  blanks now also take the preview's rich path, so the blank shows as a writing line on screen.
- Points labels keep each style's placement (`(2 points)` at the right; `[2]` on Exam Board),
  as documented in the table above; the preview's inline "(2 pts)" is a screen-only cue.

Pagination check (2026-10-18): the 16 fixture exams (the P2 fixtures in all four styles)
were exported before and after. All 16 compact-list keys are byte-identical. 9 of the 16 exams
keep every question on the same page; 7 move by one question at a page break, all because
pictures are now smaller (the fixture's 637x512 diagram printed 368 pt wide, now 268 x 216 pt)
or because of the taller title area. Quiz 5A Standard now breaks before question 4, the same
place the page preview breaks.

## DOCX pictures (round 7)

The DOCX embeds each question's picture as an inline drawing (`word/media/imageN.png|jpeg`,
an image relationship and the content type), sized and placed as in the PDF. PNG and JPEG embed
byte for byte; other formats are re-encoded to PNG. Only a picture whose bytes do not decode
prints the `[picture]` placeholder; `[picture]` markers left in a stem without a picture still
print as written.

## Answer key layouts (round 7)

The export dialog has a **Key layout** choice under "Answer key":

- **Answer section** (the default, Test Parrot's printed key): the title, "Answer Section",
  each section's title (or "Section N"), then every question's number with a short blank and
  the answer written on it: the letter(s) for multiple choice, True/False, one blank per left
  item for matching. Written answers (short answer, fill-in, multipart) print under an empty
  blank.
- **Marked test**: the whole test as the students get it, with the correct choices checked and
  in bold, and "Answer: ..." printed before the answer lines of written questions.
- **Compact list**: the original "N. answer" lines.

The layout is stored in the `.doc.json` snapshot (`key_layout`, default list), so snapshots
written before round 7 regenerate exactly as they printed. The headless hook takes
`VERSHION_KEY_LAYOUT=list|section|marked` (default section) and also writes `key.docx`.
The export dialog now also has a **Cancel** button and a note that Export History is kept only
on this computer and is not an archival backup (Test Parrot's wording, adapted). The Exams
screen's card menu has **Open** above Rename and Delete, as on Test Parrot.
