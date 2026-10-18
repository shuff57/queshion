# verSHion Ditto package — `.ditto.json` (format 0.1.0)

A `.ditto.json` file is one UTF-8 JSON document holding **one question bank** and, when the
source was a test, **one exam** laid out as printed. It is the interchange format verSHion's
AI conversion produces and the app imports and exports. It is modeled on Test Parrot's
`.parrot.json` package, but flatter, and it carries what verSHion needs to crop figures out
of the source page.

```json
{
  "format": "queshion/ditto",
  "formatVersion": "0.1.0",
  "generator": { "name": "verSHion", "version": "gemma4:31b" },
  "bank": { "name": "Ch4 Quiz 5A", "questions": [ /* Question, ... */ ] },
  "exams": [ /* zero or one Exam */ ],
  "report": { "unconverted": [], "notes": [] }
}
```

`format` and `formatVersion` are required. Unknown extra members are ignored. `exams` and
`report` may be omitted.

## Question

Common members:

| member | type | notes |
|---|---|---|
| `id` | string | unique in the file, e.g. `"q1"`; verSHion assigns its own ids on import |
| `type` | string | `multiple-choice`, `true-false`, `matching`, `fill-in-the-blank`, `short-answer`, `multipart` |
| `stem` | string | exactly what the source prints at the question's number, minus the number and any `[2]` mark. Written in verSHion stem markup (`docs/stem-format.md`): each printed line is a `\n` line, bullets are `• item` lines, numbered lists `1. item` lines, tables pipe rows (see below). Equations are written `$linear$` (e.g. `$x^2+1$`). Subscripts and superscripts in plain text use Unicode (H₂O, m²) |
| `label` | string, optional | the number the source prints ("3", "5B-2"). Informational only |
| `points` | positive integer, optional | only when the source prints a mark. Not on `multipart` itself |
| `difficulty` | `"easy"`/`"medium"`/`"hard"`, optional | only if the source states it |
| `topics` | string[], optional | only if the source states them |
| `explanation` | string, optional | only if the source prints one |
| `figure` | Figure, optional | one picture that belongs to the stem (see below) |

Per type:

- **multiple-choice** — `choices`: `[{ "text": "Ge", "correct": false }, ...]`, in printed order
  (at least two). Several `correct: true` = multiple answer. Mark `correct` only where the source
  gives the answer (answer key, circled or starred choice).
- **true-false** — `answer`: `true`/`false`; omit when the source gives no answer.
- **matching** — `pairs`: `[{ "left": "Na", "right": "sodium" }, ...]`; `distractors`: extra right-hand
  items that match nothing (optional). If the source gives no key, pair items in printed order
  and say so in `report.notes`.
- **fill-in-the-blank** — the stem holds `___` (three underscores) where each blank goes, and
  `blanks` holds one entry per `___`, in order: an array of accepted answers, e.g.
  `[["Ionic"], ["magnesium nitrite", "magnesium(II) nitrite"]]`. A blank with no key is `[]`.
  Every question that asks for several short answers laid out in a table is ONE fill-in-the-blank
  whose stem holds the table as pipe rows: the header row as printed WITHOUT blanks, then each
  body row with a bare `___` in each answer cell, e.g.
  `Formula | Ionic or Covalent? | Name` / `KI | ___ | ___` / `SO₃ | ___ | ___`.
  `blanks` then reads row by row, cell by cell.
- **short-answer** — `suggestedAnswer`: string (the source's worked or sample answer, or `""`).
- **multipart** — one number with lettered parts, or questions sharing one passage/figure.
  The shared material is the `stem` (and `figure`). `parts`: `[{ "stem": "a. NH₃", "type":
  "short-answer"|"multiple-choice"|"true-false", "suggestedAnswer"|"choices"|"answer": ... }]`.
  Part stems keep their own text but drop the `a.`/`(i)` label only when that label is part of
  the numbering rather than the wording.

### Figure

```json
"figure": { "box": [0.18, 0.21, 0.83, 0.43], "alt": "Lewis structure of CH₃COCl", "page": 1 }
```

- `box` = `[x0, y0, x1, y1]` as **fractions of the page image** (0–1), origin top-left,
  `x0 < x1`, `y0 < y1`. Draw it tightly around the picture, including its labels and captions
  if they are part of the picture, and excluding the question text and answer lines.
- `page` = 1-based page of the source. Optional when converting one page at a time (verSHion
  fills it in).
- `alt` = a short description for accessibility. Never copy handwriting into it.
- Only pictures the question needs: a diagram, graph, map, photo, or structure drawing.
  Equations, tables, typed text and answer lines are NOT figures; write them as text/`$math$`.
- verSHion crops the picture from its own raster of the page and attaches it to the question.
  A figure whose box cannot be cropped (off-page, degenerate) is dropped with a warning; the
  question is still imported.

## Exam (when the source is a test)

```json
{ "name": "Quiz 5A – Ch. 4",
  "sections": [ { "title": "", "instructions": "For multiple choice questions: Circle the letter." } ],
  "positions": [ { "question": "q1", "section": 0 }, { "question": "q2", "section": 0 } ] }
```

One position per converted question, in printed order, `question` = that question's `id`,
`section` = index into `sections`. No exam when the source is a question pool.

## Report

```json
"report": {
  "unconverted": [ { "page": 2, "label": "4", "reason": "answer drawn by hand, no printed question" } ],
  "notes": [ "No answer key printed; no choices marked correct." ]
}
```

## Import behavior (what verSHion guarantees)

- **Tolerant.** Each question is validated alone. One bad question is skipped and reported; it
  never rejects the rest of the page or file.
- **Table repair.** A model often repeats the column heading in every row
  (`KI | Ionic or Covalent? ___ | Name ___`) or puts blanks in the header row
  (`Formula | Ionic or Covalent? ___ | Name ___`). On import such a table is rewritten to the
  clean shape (header without blanks — synthesised with an empty first cell when the rows had
  none — and bare `___` cells); the header's stray blanks are dropped from the marker count, and
  `blanks` is realigned: answers grouped per row are flattened, trailing empty entries are
  padding, and entries for the dropped header blanks are removed. Applies to every stem,
  including multipart part stems.
- Stems never contain the question number, a `[n]` mark, or directions printed above a group.
- Multi-page sources are converted one page per request and merged in page order.
- File extension `.ditto.json`; MIME `application/json`.
