# queShion · Design V4 handoff

"Study Studio in paper tones": V2's dock layout, V1's cream and brown neutrals, a chalkboard-green accent, and a highlighter-yellow marker used sparingly. Light and dark are both specified. Colours are in `theme_v4.rs`. The HTML boards in `reference/` are the visual source of truth (open them in the Design canvas, or read their CSS).

## Principles
- Paper-adjacent and warm. Never pure white or black UI (the exam page itself is the only near-white surface, in both modes).
- One primary button style: filled accent pill. Everything else is a ghost or outline pill.
- One accent (chalkboard green). The highlighter is a marker, not a second accent: active dock item, "just added" question, "needs AI" tag.
- Line icons only, 1.6–1.8 px stroke, round caps and joins. No emoji.
- Faint 24 px graph-paper grid on the desk (light: ink at about 5.5% alpha, dark: cream at about 4.5%).

## Tokens
| Token | Light | Dark |
|---|---|---|
| desk | #f7f4ed | #211c18 |
| rail (dock) | #332a24 | #14100d (1 px border #4b3f35) |
| card | #fffdf8 | #2b2520 |
| field | #f3eee3 | #211c18 |
| border | #e0d3c2 | #4b3f35 |
| rule | #bda995 | #6d5c4b |
| ink | #332a24 | #f2e8dd |
| text | #5e4c40 | #d3c2b2 |
| muted | #7a6350 | #a9937e |
| accent | #2f6144 | #9dcba0 |
| accent wash | #dfe9d4 | #2d3b2c |
| on accent | #f7fbf3 | #0e2012 |
| highlighter | #f3dc6a | #e9cf55 |
| danger | #a33b2e | #e0826e |
| exam table (behind paper) | #e8dfce | #161210 |
| paper | #fffdf8 both | |

Contrast: on-accent over accent about 7:1 light and 9:1 dark. Dock labels (#e9dfd2 on #332a24) pass AA. Highlighter always carries dark ink text (#332a24).

## Type
- Headings and card titles: Source Serif 4, weight 500. Page title 34–40/38–42, exam and bank titles 15–19, section numbers 22.
- UI: Source Sans 3, 13/18 body, 12 secondary, 11.5 captions.
- Small caps labels: Source Sans 3, 700, 10–12 px, letter-spacing .09em, uppercase, muted.
- Exam page: Source Serif 4, body 12 pt (preview px = pt × 0.8), line-height 1.45.
- Fonts ship in `assets/fonts` already.

## Shape and spacing
- Dock 88 px wide, radius 18, inside a 10 px app padding, 16 px gap to content. Collapses to a 64 px icon rail below ~900 px.
- Cards, tiles, popovers: radius 16–18, 1 px border, no heavy shadow (open items get a soft accent shadow). Fields radius 12. Chips and buttons are full pills.
- Content max-width ~1040 px. Grid gaps 10–14 px.
- Dock nav item: 70 px wide, icon 21 px over a 10.5 px label, radius 13. Active = highlighter fill with dark ink.

## Global chrome
- Dock top to bottom: logo tile, Home, Banks, Exams, Import, Figures, Settings, lock icon ("Saved on this computer"), vertical light/dark switch (sun, moon).
- No top app bar. Each screen starts with a small breadcrumb and a serif title.
- Cards/List toggle is a pill with two segments (grid icon, rows icon); active segment uses accent wash.

## Screens
**Home.** Title "Welcome back." with three stat chips (banks, exams, questions). Three action tiles: New exam (filled accent), New bank, Import ("Anything: text, files, photos"). "Recent exams" with Cards/List: each exam shows a tilted page thumbnail plus facts Questions, Points, Bank, Edited (list view uses columns with a header row). "Your banks" in the same view: question count, topics, edited, and a thin question-type mix bar. Last-export strip at the bottom with Export history link.

**Bank editor.** Breadcrumb, serif bank name, subtitle (84 questions · 6 topics · edited), buttons Import and Add question (primary). Toolbar: search, type filter chips with counts, Cards/List toggle. Question cards: type pill, Q number, points, 2-line stem, topic and difficulty chips. Click a card to expand in place (one open at a time, spans the full row). The editor has two columns. Left: stem field, answers with a tappable circle to mark correct (or model answer for FILL/SHORT/MATCH). Right: type selector, points, difficulty, topics, explanation. Footer: Delete (danger text), "Changes save when you press Done", Cancel, Done. Done or Cancel collapses the card.

**Exam editor** (V1 structure). Top bar: crumb, serif exam name, Saved indicator, menus File / Edit / Format, undo, redo, Export history, Export (primary). Format bar under it, always visible: Heading (Small/Medium/Large → 12/14/17 pt on screen), Text size stepper (9–14 pt), Margins (Narrow 0.75″ / Normal 1″ / Wide 1.25″), Answer blanks; page counter at right. Main area: desk on the left with the paper preview, shelf on the right (340 px): bank dropdown, search, topic chips, rows with grip, type pill, 2-line stem, points, add/in-exam mark. Items already in the exam are muted with a check. Dragging an item tilts it and shows a dashed accent drop slot on the page ("Drop here to add as question 5"). A just-added question gets a highlighter wash on the page. Alternative: the same bar can be a Format popover instead (see `reference/v4-exam-popover-alt.dc.html`).

**Import (unified).** One drop area accepts pasted text, files (Blackboard text, Aiken, CSV, .parrot.json, PDF, Word) and photos, with Choose files / Paste text / Add photo buttons. Format is detected automatically. Sources list shows each source's detected kind and count with a status tag (Ready, Needs AI in highlighter). A "Convert with AI" action covers photos only; copy states nothing leaves the computer until Convert, then only the photo goes to the configured model (Ollama Cloud, gemma4:31b). Review panel groups questions by source (tick to include, duplicates flagged in danger text, new AI rows get a highlighter left edge), then destination bank, default points, skip-duplicates, and a primary "Import N questions" button.

**Equation editor.** Opens inside the question stem (type `/` or press Σ). Panel: live preview on graph paper, Structures (fraction, root, limit, integral, sum), Symbols grid, Examples chips, a LaTeX source line (paste in or copy out), Undo, Clear, Cancel, Insert equation. Equations are inline chips in the stem and render as real typeset math on the exam page; answers accept equations too. The mock builds expressions by clicking; typed entry and box editing are still to design.

## Implementation notes (egui)
- Build `Palette` into `Visuals` once per mode; paint the grid with `painter.line_segment` on the central panel.
- Pills: `Rounding::same(height / 2)`. Cards: 16–18.
- Expand-in-place cards: a collapsing region with `id_salt` per question and a single `open_id` in state so opening one closes the others.
- Cards/List is a single view enum shared by Home and Bank screens, persisted per screen.
- Dark on-accent is dark ink, not cream (contrast).

## Files
- `theme_v4.rs` palettes and radii
- `reference/v4-home.dc.html`, `v4-bank.dc.html`, `v4-exam.dc.html`, `v4-import.dc.html`, `v4-math.dc.html`, plus the popover alternative
