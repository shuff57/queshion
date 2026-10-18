//! Import format parsers — turn pasted or uploaded files into bank questions.
//!
//! [`detect_and_parse`] looks at the bytes first: a zip archive (`PK..`) is item 0 below,
//! an old binary Office file (.xls/.doc) is refused with a save-as hint, and everything
//! else is decoded as text (item 12) and tried as items 1–11 in order. The headless hook
//! `VERSHION_PARSE=<file>` (main.rs) runs exactly this path and prints the questions as
//! JSON, so every format can be checked without the GUI. The mapping conventions shared
//! by the new formats: an auto-graded short answer (GIFT `{=a =b}`, Moodle shortanswer,
//! QTI text entry) is a `Blank` whose one blank accepts every positive-credit string; a
//! numeric answer is a `Blank` accepting the value plus value∓tolerance (or a range's
//! center, low and high), like the NUM tag; essays/file uploads are `Short`; a
//! mid-sentence answer slot becomes a `___` in the stem; HTML question text becomes stem
//! markup (`docs/stem-format.md`: paragraphs, `<br>`, bold/italic, sub/superscripts,
//! lists, tables with `---|---` under a `<th>` row, TeX `\(..\)` → `$..$`) and images
//! become `[picture]`/`[picture: alt]` placeholders — when the image bytes travel with
//! the file (a zip entry, a Moodle base64 `<file>`, a `data:` URI) the first one is also
//! stored as the question's picture. In archives and XML exports, a question of an
//! unsupported type is skipped and the import fails only when nothing is left (naming the
//! first problem); the line-based text formats fail on the first bad question.
//!
//! 0. **Zip archives**: an Excel workbook (`xl/workbook.xml`, item 4), a Word .docx
//!    (`word/document.xml`: paragraphs become lines, then the text formats below), a
//!    Test Parrot package zip (its JSON, item 1), else a **QTI content package** — a
//!    Canvas "QTI export" `.zip`, a Common Cartridge `.imscc`, a D2L/Blackboard/other QTI
//!    zip. Every `.xml`/`.qti` entry whose root is `questestinterop` or `assessmentItem`
//!    is read (item 3), in the order an `assessmentTest`'s `assessmentItemRef`s give,
//!    then `imsmanifest.xml` resources, then by name; an item ident seen before is
//!    skipped (Common Cartridge carries each quiz twice); Canvas `assessment_meta.xml`
//!    and other files are ignored. Images in the archive resolve against the item file's
//!    folder, with `$IMS-CC-FILEBASE$`, `%20` escapes and a unique-file-name fallback.
//!    The doc name is the quiz (assessment/test) title, else the manifest title, else the
//!    filename; when one archive holds several quizzes, each question's topic is its
//!    quiz title.
//! 1. **Test Parrot package / real question-bank record** (`.parrot.json`, also any JSON
//!    whose bytes mention `test-parrot/package` or `test-parrot/question-bank`): the real
//! 1. **Test Parrot package / real question-bank record** (`.parrot.json`, also any JSON
//!    whose bytes mention `test-parrot/package` or `test-parrot/question-bank`): the real
//!    export format of testparrot.com —
//!    `{"format":"test-parrot/package","formatVersion":"0.1.0","generator":{..},"requiredFeatures":[..],"questionBanks":[{"id":"bank","record":{ <question-bank record> }}],"exams":[..]}`
//!    with a question bank record of
//!    `{"format":"test-parrot/question-bank","formatVersion":"0.10.0","generator":{..},"requiredFeatures":[..],"bank":{"name":"..","questions":[..]},"media":[..]}`
//!    (a bare record parses the same way; schemas at testparrot.com/formats). All
//!    questions of all banks land in one `ImportDoc`; ids are regenerated; the map to
//!    `model::Kind` is best effort: multiple-choice → Mc (choice texts, correct and locked
//!    kept); true-false → Tf (the first choice is True; exactly one carries `correct`);
//!    matching → Matching (each prompt whose `answer` names a `wordBank` id pairs prompt
//!    text with that answer text; wordBank answers no prompt references are distractors);
//!    fill-in-the-blank → Blank (each `blank` node in the stem prints as `___`, in order,
//!    and its answer text is that blank's accepted string — the real format has no
//!    alternatives concept, so one string per blank); short-answer → Short
//!    (`suggestedAnswer` text); multipart → Multipart (each `part` is a `model::Part`; a
//!    part carrying subparts — which `PartKind` cannot nest — flattens to its lead-in stem
//!    plus one part per subpart); `easy`/`medium`/`hard` → Low/Medium/High, `topics` and
//!    integer `points` are kept. Rich text becomes stem markup (`docs/stem-format.md`):
//!    paragraphs are separated by a blank line, hard breaks are line breaks, lists `• `/`1. `
//!    lines, quotes `> ` lines, tables pipe rows (a `---|---` line under a marked header row),
//!    bold/italic `**`/`*`, sub/superscripts Unicode digits or `_{..}`/`^{..}`. Stem media become `[picture]`/`[picture: alt]`
//!    placeholders — the assets normally travel as files beside the JSON inside the
//!    package zip, so their bytes are not in the JSON; a media entry that also carries
//!    inline base64 (`data`/`bytes`/`base64` property) is attached through
//!    `store::put_image` instead. A package exam record becomes a best-effort
//!    `model::Exam` in `doc.exams`: items in `positions` order referencing the regenerated
//!    question ids, with `bank_id` left empty for the shell to fill when it creates the
//!    bank from this ImportDoc; sections, columns, answerOrder, hiddenAnswers, paper style
//!    and margins are ignored.
//! 2. **Legacy Test Parrot JSON** (any JSON whose `kind` is `test-parrot-question-bank`):
//!    an earlier placeholder shape, `{"kind":"test-parrot-question-bank", ..., "name":"..",
//!    "questions":[model::Question..]}` — kept working as an alias; ids regenerate; the
//!    doc name comes from the JSON, else the filename stem.
//! 3. **Moodle XML and QTI files** (text starting with `<`; a `.xml`/`.qti` file is
//!    claimed even when it is neither, with a helpful error).
//!    * Moodle XML — `<quiz>` of `<question type="..">`: `multichoice` → Mc (answers with
//!      `fraction` > 0 are correct, so `<single>false` with 50/50 is a multi-answer),
//!      `truefalse` → Tf, `shortanswer` → Blank, `numerical` → Blank (value ± its
//!      `<tolerance>`), `matching` → Matching (a subquestion with empty text is a
//!      distractor; empty question text gets the MAT default stem), `essay` → Short
//!      (sample from `graderinfo`, else `responsetemplate`), `multianswer` (Cloze:
//!      `{1:SHORTANSWER:=a~%50%b}`, `{:NUMERICAL:=5:0.1}`, `{:MULTICHOICE:~x~=y}`) and
//!      `ddwtos`/`gapselect` (`[[n]]` gaps) → Blank with one `___` per sub-question.
//!      `category` questions set the topic of the questions after them (the last
//!      meaningful path segment, `$course$`/`top`/"Default for…" skipped);
//!      `defaultgrade` → points, `generalfeedback` → explanation. `description`,
//!      `calculated*`, `random` and drag-onto-image types are skipped.
//!    * QTI 1.2 — `<questestinterop>` items (Canvas, Blackboard, D2L, Respondus
//!      exports). The type comes from Canvas `question_type` / Blackboard
//!      `bbmd_questiontype` metadata, else from the item's shape. The correct
//!      answers are the `varequal`s of `respcondition`s that score (`setvar` > 0 or a
//!      non-numeric value like `SCORE.max`, or a `title="correct"`), never those under
//!      `<not>`; numeric `vargte`/`varlte` give a range. Mapping: one `response_lid` →
//!      Mc (several correct, or `rcardinality="Multiple"`, = multi-answer) or Tf (true_false
//!      type, or two choices True/False); several `response_lid`s each with its own
//!      prompt → Matching (choices no prompt uses are distractors), unless every prompt
//!      appears in the stem as `[name]` (Canvas fill_in_multiple_blanks /
//!      multiple_dropdowns) → Blank, one `___` per `[name]`; `response_num` /
//!      numerical → Blank; `response_str` → Blank of its accepted strings, or Short when
//!      none (essay, file upload). Stem HTML is read from `mattext`/`mat_formattedtext`;
//!      `points_possible` → points; Canvas `general_fb` feedback → explanation;
//!      text-only, calculated and hot-spot items are skipped.
//!    * QTI 2.x and 3.0 — `<assessmentItem>` / `<qti-assessment-item>` (QTI 3 names
//!      and kebab-case attributes are read the same way). Correct values come from
//!      `responseDeclaration/correctResponse` (and positive `mapping` keys).
//!      `choiceInteraction` → Mc (`maxChoices` ≠ 1 or multiple cardinality = multi;
//!      `fixed` choices are locked) or Tf; `textEntryInteraction`,
//!      `inlineChoiceInteraction` and `gapMatchInteraction` gaps → Blank with a `___` at
//!      each spot; `matchInteraction`/`associateInteraction` → Matching (unused second-set
//!      choices are distractors); `orderInteraction` → Blank of the comma-joined order
//!      (like the ORD tag); `extendedTextInteraction`/upload/drawing → Short; two or more
//!      block interactions → Multipart (choice/text parts, each part's stem its
//!      `<prompt>`). Feedback and rubric blocks are dropped; `MAXSCORE` → points.
//! 4. **Excel `.xlsx`** (read with the zip reader plus a small XML pass: shared, inline
//!    and formula strings, numbers, booleans). Each worksheet in tab order is tried as:
//!    the CSV grammar of item 5 (same header columns, same rules, same errors — the
//!    rows are re-quoted as CSV), the Kahoot template (item 6), then a two-column
//!    term/definition list (item 10); the first sheet that reads wins.
//! 5. **CSV** (`.csv`, or any text whose first line looks like a header naming `type` and
//!    `stem`): comma-separated cells with double quotes (`""` inside a quoted cell for a
//!    literal quote; a quoted cell may span lines and keeps its line breaks). Header columns, in any order:
//!    `type`, `stem`, `points`, `topic` (split into topics on `;`, `,`, `|`), answer columns
//!    named `a`…`h`, `correct` (letters like `b` or `a,c`; digits `1`… also accepted) and
//!    `correct_text`. An empty `type` cell defaults to `mc`. Types: `mc`/`ma` (answers from
//!    the letter columns; correct marks from `correct` and/or a `correct_text` cell that
//!    matches one answer exactly), `tf` (first char of `correct_text`/`correct`: T/Y →
//!    true, F/N → false; A/B also mean true/false), `short`/`ess`/`essay` and
//!    `blank`/`fib`/`num` (the `correct_text` cell is the accepted string / sample answer).
//!    An unparseable `points` cell is ignored (`None`); structural problems (no stem, no
//!    correct mark, unknown type) fail the whole import naming the row. A `.csv` file
//!    whose first line is not such a header is not CSV — the text parsers get their try.
//! 6. **Kahoot template** (CSV text or a sheet): a header row anywhere with `Question…`,
//!    `Answer 1…`–`Answer 4…` and `Correct answer(s)…` cells; each row below with a
//!    question is Mc, its correct answers named by number (`2`, `1,3`); two answers
//!    True/False with one correct → Tf. Time limits are ignored.
//! 7. **Brightspace (D2L) CSV**: claimed when a record starts with `NewQuestion`; each
//!    `NewQuestion,<type>` row starts a question. `MC`/`MS` → Mc (`Option,<weight>,<text>`,
//!    positive weight = correct), `TF` → Tf (the `TRUE`/`FALSE` row with positive weight),
//!    `SA` → Blank (`Answer,<weight>,<text>`), `WR` → Short (`AnswerKey`), `M` → Matching
//!    (`Choice,<n>,left` with `Match,<n>,right`; unmatched Match rows are distractors),
//!    `O` → Blank of the comma-joined `Item`s, `FIB` → Blank (`Text`/`Blank` rows in
//!    order). `QuestionText` is the stem (HTML converted), `Points` → points,
//!    `Feedback` → explanation; other types are skipped.
//! 8. **Moodle GIFT** (`.gift`, or text with a `$CATEGORY:` line or a GIFT answer block
//!    — `{` + `=`/`~`/`#` answers or T/F — and no Blackboard type tags; braces right
//!    after `_`/`^` are stem markup, not GIFT). Questions are separated by blank lines
//!    (a blank line inside an open `{…}` does not split); `//` lines are comments;
//!    `::title::` is dropped; `[html]` text is converted from HTML (`[moodle]`,
//!    `[plain]`, `[markdown]` are kept as written); escapes `\~ \= \# \{ \} \:` give the
//!    literal char and `\n` a line break; `$CATEGORY: a/b/c` sets the topic of the
//!    questions after it. The answer block `{…}` decides the kind: `{}` → Short (essay);
//!    `{T}`/`{TRUE}`/`{F}`/`{FALSE}` → Tf; `{=a ~b ~c}` → Mc (`=` correct, `~` wrong,
//!    `~%50%x` a positive partial weight counts as correct, so several make a
//!    multi-answer; negative weights are wrong); `{=a =b}` (only `=`) → Blank accepting
//!    every positive answer; `{#3.14:0.01}`, `{#1..5}`, `{#=1822:0 =%50%1822:2}` → Blank
//!    of numeric variants; `{=left -> right …}` → Matching (`= -> x` is a distractor).
//!    `#feedback` after an answer is dropped, `####general feedback` → explanation. A
//!    block in mid-sentence ("The symbol is {=Na} on the table.") leaves a `___` in the
//!    stem for Mc/Blank kinds. Text with no answer block is a description: skipped.
//! 9. **Respondus and text2qti numbered text**: numbered question blocks (`1)`/`1.`),
//!    claimed when a format marker appears (`Type:`, `Points:`, `Title:`,
//!    `Quiz title:`, `Answer:` lines, `[*]`/`[ ]` boxes, a `= number` answer, a `____` or
//!    `^^^^` line) or when every multiple-choice block is numbered; Blackboard-tagged
//!    text is never claimed. Settings lines above the number apply to that question (a
//!    settings-only block to the next one); `Quiz title:` names the doc; `Points:` →
//!    points. Choices are `a)`/`a.` lines, `*` marks the correct one(s) (or an
//!    `Answer: b` / `Answer: a, c` line). Respondus `Type:` values: none/`MC` → Mc (or
//!    Tf for two choices True/False), `MR` → multi-answer Mc, `TF`, `E` → Short (the
//!    first choice line, if any, is the sample answer), `F` → Blank accepting every
//!    choice line, `MT` → Matching (`a) left = right`), `O` → Blank of the ordered
//!    choices; others (FMB…) fail with a message. text2qti: `[*] x`/`[ ] x` →
//!    multi-answer Mc, `*   answer` lines (no letter) → Blank, `= 2.5`, `= 2.5 +- 0.1`,
//!    `= [2, 3]` → numeric Blank, `____` → Short (essay), `^^^^` → Short (file upload),
//!    `...` general feedback → explanation; per-answer feedback (`+`, `-`, `@`, `~`
//!    lines after the answers start) is dropped.
//! 10. **Flashcard / term lists** (Quizlet export, Anki notes export, two-column lists):
//!     every line `term<TAB>definition` (extra tab columns such as Anki tags ignored;
//!     Anki `#separator:`/`#html:` header lines honored; Quizlet's one-line
//!     `t<TAB>d;t<TAB>d` form split on `;`), or two or more lines that ALL split into
//!     exactly two non-empty cells on ` - `, `;` or `,` (quote-aware). A header row of
//!     words like term/definition or front/back is skipped; HTML cells are converted.
//!     Mapping decision: a vocabulary list is matched, a Q&A deck is answered — when most
//!     fronts end in `?` or run past 60 characters, or there are fewer than 3 cards,
//!     each card is a Short question (stem = front, sample = back); otherwise the cards
//!     become Matching questions of at most 10 term → definition pairs each (split
//!     evenly, stem "Match each term with its definition.").
//! 10b. **Blackboard upload file** (tab-delimited "Upload Questions" form): every line
//!     `TAG<TAB>stem<TAB>…` with a Blackboard type tag (MC, MA, TF, ESS, SR, FIB,
//!     FIB_PLUS, FIL, NUM, MAT, ORD; OP/HS/JUMBLED_SENTENCE/QUIZ_BOWL skipped); see
//!     [`try_bb_upload`]. Checked before term lists, which never claim tagged rows.
//! 11. **Question-block text**: Blackboard test-generator text (also what the AI converter
//!     returns and what "Paste your questions" takes) plus untagged Aiken blocks (real
//!     Aiken's `ANSWER: B` line after the choices marks the correct letter). See
//!     [`parse_blackboard`] for the grammar. The doc name is the filename stem, or
//!     "Pasted questions" for an untitled paste. A stem written over several lines keeps
//!     its line breaks (MC/MA, TF, NUM and Aiken stems). An MC block whose choices carry no
//!     letters takes Test Parrot's plain-line form (`stem` / `Venus` / `*Mercury` / `Mars`:
//!     first line = stem, every later line a choice, `*` = correct; see
//!     `plain_line_choices`). Two hardenings from a live e2e corpus:
//!     a TF statement may have its answer glued onto the stem line's tail ("…liquid iron.
//!     False" — a bare, case-insensitive t/f/true/false word after its statement is the
//!     answer, stripped from the stem), and in a FIB stem a run of three-or-more
//!     underscores is exactly ONE blank (collapsed back to the canonical `___`).
//! 12. Text in any of the above arrives through one decoder: UTF-8 first (BOM stripped),
//!     UTF-16 with a BOM, then Latin-1 byte-for-byte, so accented files import instead of
//!     failing.
//!
//! Every imported question gets fresh ids, `created`/`modified` timestamps, and
//! `points: None` (CSV/Excel/package/Moodle/QTI/Respondus/text2qti/Brightspace may fill
//! points), `difficulty: None` (package may fill), `topics: []` (CSV/Excel/package and
//! the GIFT/Moodle categories may fill), `explanation: None` (GIFT/Moodle/QTI/text2qti/
//! Brightspace feedback may fill), `image: None` (a package with inline media, or an
//! archive/Moodle/`data:` image, fills it).

use std::collections::{HashMap, HashSet};

use crate::model::{Answer, Difficulty, Exam, ExamItem, Kind, Pair, Part, PartKind, Question};

/// One successfully parsed import: a suggested bank name, its questions — and, for Test
/// Parrot packages that carry exams, their best-effort `model::Exam`s (empty otherwise;
/// the shell fills each item's empty `bank_id` with the bank it creates from this doc).
#[derive(Debug)]
pub struct ImportDoc {
    pub name: String,
    pub questions: Vec<Question>,
    pub exams: Vec<Exam>,
}

/// The not-recognized fallback error: names every format `detect_and_parse` reads.
const GENERIC: &str = "Not a recognized question file — expected Blackboard test text, Aiken, CSV or Excel (.xlsx), Moodle GIFT or Moodle XML, \
     QTI or a Canvas/LMS export (.zip), Respondus or text2qti text, a Quizlet/Anki term list, Kahoot or Brightspace sheets, \
     or a Test Parrot question-bank JSON";

const PACKAGE_FORMAT: &str = "test-parrot/package";
const RECORD_FORMAT: &str = "test-parrot/question-bank";
const LEGACY_KIND: &str = "test-parrot-question-bank";
/// model::Exam's default printed header (matches the model's documented default).
pub(crate) const DEFAULT_HEADER: &str = "Name: __________________ Class: ___________ Date: ___________";
/// Block type tags, matched case-insensitively as the whole first line (trailing `. : )` trimmed).
const TAGS: [&str; 8] = ["mc", "ma", "tf", "ess", "num", "mat", "fib", "ord"];
/// Default stem for a MAT block that has no stem text of its own.
const MAT_DEFAULT_STEM: &str = "Match each item on the left with its correct match on the right.";
/// Alternative separator inside one FIB answer line (exact, case-sensitive).
const OR: &str = " OR ";
/// Fill-in-the-blank marker a FIB stem must contain.
const BLANK: &str = "___";
/// Arrow separator accepted inside MAT pair lines: `left -> right`.
const ARROW: &str = " -> ";

pub fn detect_and_parse(data: &[u8], filename: &str) -> Result<ImportDoc, String> {
    let ext = ext_of(filename);
    let name = doc_name(filename);
    if is_zip(data) {
        return parse_zip(data, &name);
    }
    if data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return Err(OLE_ERR.to_string());
    }
    parse_text(&decode_text(data), &name, &ext)
}

/// The text formats, in detection order (see the module docs).
fn parse_text(text: &str, name: &str, ext: &str) -> Result<ImportDoc, String> {
    let name = name.to_string();
    if crate::ditto::is_ditto(text) {
        return crate::ditto::import_text(text, None, 0, &name).map(|r| r.doc);
    }
    if let Some(result) = try_json(text, &name, ext == "json") {
        return result;
    }
    if let Some(result) = try_xml(text, &name, matches!(ext, "xml" | "qti")) {
        return result;
    }
    if let Some(result) = try_csv(text) {
        return result.map(|questions| ImportDoc { name, questions, exams: Vec::new() });
    }
    if let Some(result) = try_kahoot_text(text, &name) {
        return result;
    }
    if let Some(result) = try_brightspace(text, &name) {
        return result;
    }
    if ext == "gift" || is_gift(text) {
        return parse_gift(text, &name);
    }
    if let Some(result) = try_numbered(text, &name) {
        return result;
    }
    if let Some(result) = try_bb_upload(text, &name) {
        return result;
    }
    if let Some(result) = try_term_list(text, &name) {
        return result;
    }
    parse_blocks(text, &name)
}

// ---------- Blackboard upload file (tab-delimited) ----------

/// Every Blackboard "Upload Questions" type tag (the first cell of a row).
const BB_UPLOAD_TAGS: [&str; 15] =
    ["mc", "ma", "tf", "ess", "sr", "fib", "fib_plus", "fil", "num", "mat", "ord", "op", "jumbled_sentence", "quiz_bowl", "hs"];

/// The rows of a Blackboard upload file: every non-empty line holds a tab and its first
/// cell is an upload type tag. `None` when any line is not such a row.
fn bb_upload_rows(text: &str) -> Option<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    for l in text.lines() {
        let l = l.trim_end_matches(['\r', '\n']);
        if l.trim().is_empty() {
            continue;
        }
        let cells: Vec<String> = l.split('\t').map(|c| c.trim().to_string()).collect();
        if cells.len() < 2 || !BB_UPLOAD_TAGS.contains(&cells[0].to_ascii_lowercase().as_str()) {
            return None;
        }
        rows.push(cells);
    }
    (!rows.is_empty()).then_some(rows)
}

/// Blackboard's tab-delimited upload form, one question per row:
/// `MC|MA <stem> (<answer> correct|incorrect)…`, `TF <stem> true|false`, `ESS|SR <stem>
/// [sample]`, `FIL <stem>` (file response → Short), `FIB <stem> <answer>…` (one blank
/// accepting every answer; `___` appended to the stem when it has none, or one answer per
/// blank when the counts match), `FIB_PLUS <stem with [x]> x <ans>… <empty> y <ans>…`,
/// `NUM <stem> <answer> [tolerance]`, `MAT <stem> (<left> <right>)…`, `ORD <stem>
/// <item>…` (Blank of the comma-joined order). `OP`, `HS`, `JUMBLED_SENTENCE` and
/// `QUIZ_BOWL` have no gradable answer here and are skipped.
fn try_bb_upload(text: &str, name: &str) -> Option<Result<ImportDoc, String>> {
    let rows = bb_upload_rows(text)?;
    let mut questions = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match bb_upload_row(row) {
            Ok(Some(q)) => questions.push(q),
            Ok(None) => {}
            Err(e) => return Some(Err(format!("row {}: {e}", i + 1))),
        }
    }
    if questions.is_empty() {
        return Some(Err("The Blackboard upload file has no importable rows (OP, HS, JUMBLED_SENTENCE and QUIZ_BOWL are not supported).".into()));
    }
    Some(Ok(ImportDoc { name: name.to_string(), questions, exams: Vec::new() }))
}

fn bb_upload_row(row: &[String]) -> Result<Option<Question>, String> {
    let tag = row[0].to_ascii_lowercase();
    let stem = row[1].clone();
    // Trailing empty cells (a spreadsheet's padding) carry nothing.
    let mut rest: Vec<String> = row[2..].to_vec();
    while rest.last().is_some_and(|c| c.is_empty()) {
        rest.pop();
    }
    if stem.is_empty() {
        return Err(format!("{} row has no question text", row[0]));
    }
    let kind = match tag.as_str() {
        "mc" | "ma" => {
            if rest.len() < 2 {
                return Err("MC needs answer, correct/incorrect pairs after the question".into());
            }
            let answers: Vec<Answer> = rest
                .chunks(2)
                .filter(|c| !c[0].is_empty())
                .map(|c| Answer {
                    text: c[0].clone(),
                    correct: c.get(1).is_some_and(|f| f.eq_ignore_ascii_case("correct")),
                    locked: false,
                })
                .collect();
            if answers.is_empty() {
                return Err("MC row has no answers".into());
            }
            Kind::Mc { answers }
        }
        "tf" => {
            let a = rest.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let correct = match a.as_str() {
                "true" | "t" => true,
                "false" | "f" => false,
                _ => return Err("TF row needs true or false after the statement".into()),
            };
            Kind::Tf { correct }
        }
        "ess" | "sr" => Kind::Short { sample: rest.first().cloned().unwrap_or_default() },
        "fil" => Kind::Short { sample: String::new() },
        "fib" => {
            let answers: Vec<String> = rest.into_iter().filter(|a| !a.is_empty()).collect();
            if answers.is_empty() {
                return Err("FIB row has no accepted answers".into());
            }
            let stem = normalize_blanks(&stem);
            let blanks = blank_count(&stem);
            let (stem, accept) = if blanks > 1 && blanks == answers.len() {
                (stem, answers.into_iter().map(|a| vec![a]).collect())
            } else if blanks == 0 {
                (format!("{stem} {BLANK}"), vec![answers])
            } else {
                (stem, vec![answers])
            };
            return Ok(Some(make_question(stem, Kind::Blank { accept })));
        }
        "fib_plus" => {
            let mut accept: Vec<Vec<String>> = Vec::new();
            let mut stem = stem;
            for group in rest.split(|c| c.is_empty()) {
                let Some((var, answers)) = group.split_first() else { continue };
                let marker = format!("[{var}]");
                if !stem.contains(&marker) || answers.is_empty() {
                    continue;
                }
                stem = stem.replacen(&marker, BLANK, 1);
                accept.push(answers.to_vec());
            }
            if accept.is_empty() {
                return Err("FIB_PLUS row has no [variable] with answers".into());
            }
            return Ok(Some(make_question(stem, Kind::Blank { accept })));
        }
        "num" => {
            let ans = rest.first().ok_or("NUM row needs an answer")?;
            let v = parse_f64(ans).ok_or_else(|| format!("NUM answer {ans:?} is not a number"))?;
            let tol = rest.get(1).and_then(|t| parse_f64(t)).map(f64::abs).unwrap_or(0.0);
            let variants = if tol > 0.0 { num_range(v - tol, v + tol) } else { vec![fmt_num(v)] };
            Kind::Blank { accept: vec![variants] }
        }
        "mat" => {
            if rest.len() < 2 {
                return Err("MAT row needs left, right pairs after the question".into());
            }
            let pairs: Vec<Pair> = rest.chunks(2).filter(|c| c.len() == 2).map(|c| Pair { left: c[0].clone(), right: c[1].clone() }).collect();
            Kind::Matching { pairs, distractors: Vec::new() }
        }
        "ord" => {
            let items: Vec<String> = rest.into_iter().filter(|a| !a.is_empty()).collect();
            if items.is_empty() {
                return Err("ORD row has no items".into());
            }
            Kind::Blank { accept: vec![vec![items.join(", ")]] }
        }
        _ => return Ok(None),
    };
    Ok(Some(make_question(stem, kind)))
}

/// The Blackboard test-generator format (plus untagged Aiken blocks), used for a paste.
/// The full grammar is documented in the module docs: question blocks separated by blank
/// lines, a type tag on a block's own first line, `*` marks the correct choice, and the
/// same stems/stems-wrap and pairing conventions described there.
pub fn parse_blackboard(text: &str) -> Result<ImportDoc, String> {
    parse_blocks(text, "Pasted questions")
}

fn parse_blocks(text: &str, name: &str) -> Result<ImportDoc, String> {
    let mut questions: Vec<Question> = Vec::new();
    let mut failed: Option<String> = None;
    let mut claimed = 0usize; // count only blocks we tried as questions, for messages
    for block in split_blocks(text) {
        let Some((first, rest)) = block.split_first() else { unreachable!("split_blocks skips empties") };
        if let Some(tag) = tag_of(first) {
            claimed += 1;
            match parse_tagged(tag, rest).map(|(stem, kind)| make_question(stem, kind)) {
                Ok(q) => questions.push(q),
                Err(e) => failed = failed.or_else(|| Some(format!("question {claimed}: {e}"))),
            }
        } else if rest.iter().filter(|l| is_choice(l)).count() >= 2 {
            claimed += 1;
            match parse_aiken(&block) {
                Ok(q) => questions.push(q),
                Err(e) => failed = failed.or_else(|| Some(format!("question {claimed}: {e}"))),
            }
        }
        // Anything else is prose (a title line, section headers): skipped.
    }
    if let Some(e) = failed {
        return Err(e);
    }
    if questions.is_empty() {
        return Err(GENERIC.to_string());
    }
    Ok(ImportDoc { name: name.to_string(), questions, exams: Vec::new() })
}

/// The whole first line, case-insensitively, optionally ending with `. : )`, is the tag.
fn tag_of(line: &str) -> Option<&'static str> {
    let core = line.trim().trim_end_matches(['.', ')', ':']).trim().to_ascii_lowercase();
    TAGS.iter().find(|t| **t == core).copied()
}

// ---------- tagged blocks ----------

fn parse_tagged(tag: &str, lines: &[String]) -> Result<(String, Kind), String> {
    match tag {
        "mc" | "ma" => {
            let (stem, answers) = parse_mc(lines)?;
            Ok((stem, Kind::Mc { answers }))
        }
        "tf" => {
            let mut stem_lines: Vec<String> = Vec::new();
            let mut correct: Option<bool> = None;
            for line in lines {
                let low = line.to_ascii_lowercase();
                if correct.is_none() {
                    // Primary rule: a line of its own whose (punctuation-stripped, once)
                    // whole text is the answer word: T, true, false…
                    match low.trim_end_matches([',', ';', ':', '.', '!', '?']) {
                        "t" | "true" => {
                            correct = Some(true);
                            continue;
                        }
                        "f" | "false" => {
                            correct = Some(false);
                            continue;
                        }
                        _ => {}
                    }
                }
                stem_lines.push(line.clone());
            }
            // Hardening (live corpus): the model may append the answer INLINE at the end
            // of the stem line ("…made of liquid iron. False"); a bare t/f/true/false
            // word there is stripped and taken as the answer.
            if correct.is_none()
                && let Some(last) = stem_lines.last_mut()
            {
                correct = inline_tf(last).map(|(prefix, flag)| {
                    *last = prefix;
                    flag
                });
            }
            let stem = stem_lines.join("\n");
            if stem.is_empty() {
                return Err("no stem — write the statement on the line after the TF tag, and T or F on a later line".into());
            }
            let correct = correct.ok_or("no answer — add a T or F (its own line, or after the statement)")?;
            Ok((stem, Kind::Tf { correct }))
        }
        "ess" => {
            let (stem, sample) = first_and_rest(lines, "short-answer")?;
            Ok((stem, Kind::Short { sample: sample.join("\n") }))
        }
        "num" => {
            let mut stem = String::new();
            let mut accept: Option<Vec<String>> = None;
            for line in lines {
                if accept.is_none()
                    && let Some(variants) = parse_num_answer(line)
                {
                    accept = Some(variants);
                } else if stem.is_empty() {
                    stem = line.clone();
                } else {
                    stem.push('\n');
                    stem.push_str(line);
                }
            }
            if stem.is_empty() {
                return Err("no stem — the first line after the NUM tag must be the question text".into());
            }
            let accept = accept.ok_or("no numeric answer — add a line with just the number (like 4.5 or 1.5±0.02)")?;
            Ok((stem, Kind::Blank { accept: vec![accept] }))
        }
        "fib" => {
            let (raw_stem, answers) = first_and_rest(lines, "fill-in-the-blank")?;
            // Harden: any run of 3+ underscores is ONE blank; collapse runs so the stem
            // prints the canonical `___` marker the rest of the app counts.
            let stem = normalize_blanks(&raw_stem);
            let blanks = blank_count(&stem);
            if blanks == 0 {
                return Err("the stem has no ___ blank markers — write ___ where the answer goes".into());
            }
            if answers.len() < blanks {
                return Err(format!(
                    "the stem has {blanks} {BLANK} markers but only {} answer lines — one line per blank; alternatives on a line are separated by {OR}",
                    answers.len()
                ));
            }
            if answers.len() > blanks {
                return Err(format!("the stem has {blanks} {BLANK} markers but {} answer lines — one line per blank", answers.len()));
            }
            let accept = answers
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let alts: Vec<String> = line.split(OR).map(str::trim).filter(|a| !a.is_empty()).map(str::to_string).collect();
                    if alts.is_empty() { Err(format!("no accepted answer for blank {}", i + 1)) } else { Ok(alts) }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((stem, Kind::Blank { accept }))
        }
        "mat" => {
            let split_at = lines.iter().position(|l| l.trim() == "-");
            let (pair_lines, distractor_lines): (&[String], &[String]) = match split_at {
                Some(i) => (&lines[..i], &lines[(i + 1)..]),
                None => (lines, &[]),
            };
            if pair_lines.is_empty() {
                return Err(
                    "no matching pairs — write pairs of lines (left item, right item), then a line containing only `-`, then any distractor lines".into(),
                );
            }
            let pairs: Vec<Pair> = if pair_lines.iter().all(|l| l.contains(ARROW) || l.contains('→')) {
                pair_lines
                    .iter()
                    .map(|l| {
                        let (left, right) = l
                            .split_once(ARROW)
                            .or_else(|| l.split_once('→'))
                            .ok_or_else(|| "no arrow".to_string())?;
                        Ok(Pair { left: left.trim().to_string(), right: right.trim().to_string() })
                    })
                    .collect::<Result<Vec<_>, String>>()?
            } else if pair_lines.len() % 2 == 0 {
                pair_lines.chunks(2).map(|c| Pair { left: c[0].clone(), right: c[1].clone() }).collect()
            } else {
                return Err(
                    "an odd number of pair lines — write pairs as alternating left/right lines, or one `left -> right` line per pair".into(),
                );
            };
            let distractors = distractor_lines.iter().map(|l| l.trim().to_string()).collect();
            Ok((MAT_DEFAULT_STEM.to_string(), Kind::Matching { pairs, distractors }))
        }
        "ord" => {
            let (stem, answers) = first_and_rest(lines, "ordering")?;
            if answers.is_empty() {
                return Err("no answer lines — write the answers one per line, in their correct order".into());
            }
            // Storing choice: one fill-in-the-blank accepting the comma-joined ordered list.
            let ordered = answers.join(", ");
            Ok((stem, Kind::Blank { accept: vec![vec![ordered]] }))
        }
        _ => Err("unknown type tag".into()), // unreachable: tags come from TAGS
    }
}

/// First line = stem; the rest = that kind's content lines.
fn first_and_rest<'a>(lines: &'a [String], kind: &str) -> Result<(String, &'a [String]), String> {
    match lines.split_first() {
        Some((stem, rest)) => Ok((stem.clone(), rest)),
        None => Err(format!("no stem — write the {kind} text on the line after the tag")),
    }
}

/// The TF hardening: a stem line that ENDS in the answer ("…liquid iron. False") — the
/// last word (case-insensitive, trailing punctuation allowed) is a bare t/f/true/false
/// word after whitespace; returns the line cut back to its stem (stem punctuation kept)
/// and the answer. `None` when the tail is not a bare answer word.
fn inline_tf(line: &str) -> Option<(String, bool)> {
    let t = line.trim_end();
    let (prefix, word) = t.rsplit_once(char::is_whitespace)?;
    let word = word.trim_end_matches([',', ';', ':', '.', '!', '?']);
    let flag = if word.eq_ignore_ascii_case("t") || word.eq_ignore_ascii_case("true") {
        true
    } else if word.eq_ignore_ascii_case("f") || word.eq_ignore_ascii_case("false") {
        false
    } else {
        return None;
    };
    let prefix = prefix.trim_end();
    if prefix.is_empty() {
        return None; // a lone word is not an inline tail on a stem
    }
    Some((prefix.to_string(), flag))
}

/// Counts fill-in-the-blank markers: a run of THREE OR MORE underscores is one blank.
/// (The AI prompt says `___`, but live models frequently type a run like `__________`;
/// a literal `___` match count would read one run as several blanks.)
fn blank_count(stem: &str) -> usize {
    let mut count = 0usize;
    let mut run = 0usize;
    for b in stem.bytes() {
        if b == b'_' {
            run += 1;
        } else {
            count += usize::from(run >= 3);
            run = 0;
        }
    }
    count + usize::from(run >= 3)
}

/// Collapses every run of 3+ underscores into exactly one `___` marker, so downstream
/// code that counts literal `___` markers sees the blanks this parser counted.
fn normalize_blanks(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut run = 0usize;
    for c in stem.chars() {
        if c == '_' {
            run += 1;
            continue;
        }
        out.push_str(&if run >= 3 { BLANK.to_string() } else { "_".repeat(run) });
        run = 0;
        out.push(c);
    }
    out.push_str(&if run >= 3 { BLANK.to_string() } else { "_".repeat(run) });
    out
}

/// MC/MA lines: choices (correct-star or letter-dot) collected in order; everything else
/// is stem — the first such line, later ones kept as their own lines (`\n`), so a stem's line
/// breaks, bullets and table rows survive (see `docs/stem-format.md`).
fn parse_mc(lines: &[String]) -> Result<(String, Vec<Answer>), String> {
    let mut stem = String::new();
    let mut answers: Vec<Answer> = Vec::new();
    for line in lines {
        match parse_choice(line) {
            Some((correct, text)) if !text.is_empty() => answers.push(Answer { text, correct, locked: false }),
            Some((..)) => {} // a bare `a.` or lone `*`: dropped as an empty choice
            None => {
                if stem.is_empty() {
                    stem = line.clone();
                } else {
                    stem.push('\n');
                    stem.push_str(line);
                }
            }
        }
    }
    if stem.is_empty() {
        return Err("no stem — put the question text after the tag (choice lines start with * or a letter like `a.`)".into());
    }
    if answers.is_empty() {
        return Err("no answer choices — put each choice on its own line and mark the correct one with a leading *".into());
    }
    if let Some(plain) = plain_line_choices(lines) {
        return Ok(plain);
    }
    Ok((stem, answers))
}

/// The plain-line choice form (Test Parrot's own paste placeholder): no choice carries a
/// letter, only the correct one(s) a leading `*`, and the wrong ones are bare lines —
/// `Which planet…?` / `Venus` / `*Mercury` / `Mars`. Claimed when no choice line is
/// lettered and a bare line follows a starred one, or only one starred line exists among
/// several bare ones (a one-choice MC is never meant). The stem is the first line plus
/// any directly following stem-markup lines (table rows `|…`, bullets `- `/`• `); every
/// other line is a choice. `None` = the ordinary lettered/star form.
fn plain_line_choices(lines: &[String]) -> Option<(String, Vec<Answer>)> {
    let lettered = |l: &str| {
        let w = l.trim();
        let w = w.strip_prefix('*').map_or(w, str::trim_start);
        let b = w.as_bytes();
        b.len() >= 2 && (b'a'..=b'j').contains(&b[0].to_ascii_lowercase()) && (b[1] == b'.' || b[1] == b')')
    };
    if lines.iter().any(|l| lettered(l)) {
        return None;
    }
    let starred = |l: &str| l.trim().strip_prefix('*').is_some_and(|r| !r.trim().is_empty());
    let first_star = lines.iter().position(|l| starred(l))?;
    let stars = lines.iter().filter(|l| starred(l)).count();
    let bare_after = lines[first_star..].iter().any(|l| !starred(l) && !l.trim().is_empty());
    let bare = lines.iter().filter(|l| !starred(l)).count();
    if !(bare_after || (stars == 1 && bare >= 2)) {
        return None;
    }
    if starred(&lines[0]) {
        return None; // no stem line at all
    }
    let markup = |l: &str| {
        let t = l.trim_start();
        t.starts_with('|') || t.starts_with("- ") || t.starts_with("• ")
    };
    let mut stem_end = 1;
    while stem_end < lines.len() && markup(&lines[stem_end]) {
        stem_end += 1;
    }
    let choices: Vec<Answer> = lines[stem_end..]
        .iter()
        .filter_map(|l| {
            let t = l.trim();
            let (correct, text) = match t.strip_prefix('*') {
                Some(r) => (true, r.trim()),
                None => (false, t),
            };
            (!text.is_empty()).then(|| Answer { text: text.to_string(), correct, locked: false })
        })
        .collect();
    if choices.len() < 2 {
        return None;
    }
    Some((lines[..stem_end].join("\n"), choices))
}

/// Is this line a choice in an MC/MA or Aiken block (with a non-empty choice text)?
fn is_choice(line: &str) -> bool {
    parse_choice(line).is_some_and(|(_, text)| !text.is_empty())
}

/// A choice line starts with `*` (optional space after; the star marks it correct and stays
/// off the text) or with a letter-dot/letter-paren pair like `a.` / `B)` (letters A…I).
/// A plain stem line returns `None`. Deliberately empty choices (`a.`, lone `*`) come back
/// with empty text for the caller to drop — this also keeps them out of the stem.
fn parse_choice(line: &str) -> Option<(bool, String)> {
    let mut work = line.trim();
    if work.is_empty() {
        return None;
    }
    let mut correct = false;
    if let Some(rest) = work.strip_prefix('*') {
        correct = true;
        work = rest.trim_start();
        if work.is_empty() {
            return None;
        }
    }
    let bytes = work.as_bytes();
    let lower = bytes[0].to_ascii_lowercase();
    let letter_dot = bytes.len() >= 2 && (b'a'..=b'j').contains(&lower) && (bytes[1] == b'.' || bytes[1] == b')');
    if letter_dot {
        Some((correct, work[2..].trim().to_string()))
    } else if correct {
        Some((true, work.to_string()))
    } else {
        None
    }
}

/// An untagged Aiken block: stem lines, then lettered choices; the correct one starts with `*`.
fn parse_aiken(block: &[String]) -> Result<Question, String> {
    let choice_at = block.iter().position(|l| is_choice(l));
    match choice_at {
        Some(0) => Err("an Aiken question needs stem lines before its choices".into()),
        Some(i) => {
            let stem = block[..i].join("\n");
            let mut answers: Vec<Answer> = Vec::new();
            for line in &block[i..] {
                // Real Aiken: `ANSWER: B` after the choices names the correct letter(s).
                if let Some(keys) = answer_key(line) {
                    for k in keys {
                        match answers.get_mut(k) {
                            Some(a) => a.correct = true,
                            None => return Err(format!("the ANSWER line names choice {} but only {} choices are given", k + 1, answers.len())),
                        }
                    }
                    continue;
                }
                match parse_choice(line) {
                    Some((correct, text)) if !text.is_empty() => answers.push(Answer { text, correct, locked: false }),
                    _ => return Err(format!("not a choice line after the stem: {line:?}")),
                }
            }
            if !answers.iter().any(|a| a.correct) {
                return Err("no correct choice — mark it by putting a * in front of the choice line".into());
            }
            Ok(make_question(stem, Kind::Mc { answers }))
        }
        None => Err("no choice lines found".into()),
    }
}

// ---------- NUM answers ----------

/// Accept-string variants for one NUM answer line: a plain number (`3.5`), a fraction
/// (`3/8` — kept as printed plus its decimal value), or a range `value±tolerance`
/// (also `+/-`/`+-`), recorded as center, center−tol, center+tol; the expanded pair is
/// formatted to the tolerance's decimals so `1.5±0.02` yields `1.5`, `1.48`, `1.52`.
fn parse_num_answer(line: &str) -> Option<Vec<String>> {
    let w = line.trim();
    if w.is_empty() {
        return None;
    }
    // Fraction like 3/8 (exactly one slash, integer parts).
    if w.split('/').count() == 2 {
        let Some((n, d)) = w.split_once('/') else { unreachable!("count of one slash") };
        let parts = (n.trim().parse::<i64>(), d.trim().parse::<i64>());
        if let (Ok(a), Ok(b)) = parts
            && b != 0
        {
            return Some(vec![format!("{a}/{b}"), fmt_num(a as f64 / b as f64)]);
        }
        return None;
    }
    let (base, tol) = split_range(w);
    let Some(v) = parse_f64(base) else { return None };
    let mut out = vec![fmt_num(v)];
    if let Some(tol) = tol
        && let Some(t) = parse_f64(&tol)
    {
        let dp = decimals(base).max(decimals(&tol));
        out.push(fmt_dp(v - t.abs(), dp));
        out.push(fmt_dp(v + t.abs(), dp));
    }
    Some(out)
}

/// `1.5±0.02`, `1.5 +/- 0.02`, `1.5+-0.02` → (value text, tolerance text).
fn split_range(w: &str) -> (&str, Option<String>) {
    for sep in ["±", "+/-", "+-"] {
        if let Some(i) = w.find(sep) {
            let (left, right) = (&w[..i], &w[(i + sep.len())..]);
            if parse_f64(left).is_some() {
                return (left.trim(), Some(right.trim().to_string()));
            }
        }
    }
    (w, None)
}

/// Plain finite number only (no `inf`/`NaN`/mixed text).
fn parse_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() { return None; }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn fmt_num(v: f64) -> String {
    format!("{v}") // Rust's shortest display drops the fraction of integral values
}

fn fmt_dp(v: f64, dp: usize) -> String {
    if dp == 0 { format!("{}", v.round() as i64) } else { format!("{v:.dp$}") }
}

/// Decimal places in a plain decimal literal: `1.48` → 2.
fn decimals(s: &str) -> usize {
    match s.split_once('.') {
        Some((_, frac)) => frac.len(),
        None => 0,
    }
}

// ---------- CSV ----------

/// Header-driven CSV only; returns `None` when the first line does not read as a header
/// naming both `type` and `stem` (so a `.csv` that is really Blackboard text still parses).
fn try_csv(text: &str) -> Option<Result<Vec<Question>, String>> {
    let records = csv_records(text);
    let lines: Vec<&str> = records.iter().map(String::as_str).filter(|l| !l.trim().is_empty()).collect();
    let Some(first) = lines.first() else { return None };
    let Some(cols) = csv_cols(&split_csv_line(first)) else { return None };

    let mut questions: Vec<Question> = Vec::new();
    for (n, line) in lines.iter().enumerate().skip(1) {
        let row = n; // 1-based data-row number, for messages
        let cells = split_csv_line(line);
        let get = |col: Option<usize>| col.and_then(|i| cells.get(i)).map(|s| s.trim().to_string()).unwrap_or_default();
        let stem = get(Some(cols.stem));
        if stem.is_empty() {
            return Some(Err(format!("CSV row {row}: the stem is empty")));
        }
        let ty = {
            let t = get(Some(cols.ty));
            let norm = t.to_ascii_lowercase().chars().filter(char::is_ascii_alphanumeric).collect::<String>();
            if norm.is_empty() { "mc".to_string() } else { norm }
        };
        let kind = match ty.as_str() {
            "mc" | "ma" | "multiplechoice" | "multipleanswers" | "multipleanswer" => {
                let mut answers: Vec<Answer> = cols
                    .answers
                    .iter()
                    .filter_map(|c| *c)
                    .map(|i| get(Some(i)))
                    .filter(|t| !t.is_empty())
                    .map(|text| Answer { text, correct: false, locked: false })
                    .collect();
                if answers.is_empty() {
                    return Some(Err(format!("CSV row {row}: an MC row has no answers — fill the a…h columns")));
                }
                let letters = get(cols.correct);
                let text_mark = get(cols.correct_text);
                if !letters.is_empty() {
                    for i in correct_letters(&letters) {
                        match answers.get_mut(i) {
                            Some(a) => a.correct = true,
                            None => {
                                return Some(Err(format!(
                                    "CSV row {row}: the `correct` column ({letters:?}) names answer {} but only {} answers are given",
                                    i + 1,
                                    answers.len()
                                )));
                            }
                        }
                    }
                }
                if !text_mark.is_empty() {
                    match answers.iter_mut().find(|a| a.text.eq_ignore_ascii_case(&text_mark)) {
                        Some(a) => a.correct = true,
                        None => {
                            return Some(Err(format!(
                                "CSV row {row}: `correct_text` {text_mark:?} does not match any of the answers exactly"
                            )));
                        }
                    }
                }
                if !answers.iter().any(|a| a.correct) {
                    return Some(Err(format!(
                        "CSV row {row}: no correct answer marked — fill `correct` (like `b` or `a,c`) or `correct_text`"
                    )));
                }
                Kind::Mc { answers }
            }
            "tf" | "truefalse" | "t" => {
                let mark = first_non_empty(get(cols.correct_text), get(cols.correct));
                let correct = match mark.chars().next() {
                    Some(c) => match c.to_ascii_lowercase() {
                        't' | 'y' | 'a' => true,
                        'f' | 'n' | 'b' => false,
                        _ => return Some(Err(format!("CSV row {row}: a TF row needs T/F (or Y/N, A/B); got {mark:?}"))),
                    },
                    None => return Some(Err(format!("CSV row {row}: a TF row needs its answer in `correct` or `correct_text` (T or F)"))),
                };
                Kind::Tf { correct }
            }
            "short" | "ess" | "essay" | "shortanswer" | "essays" => {
                let sample = first_non_empty(get(cols.correct_text), get(cols.correct));
                if sample.is_empty() {
                    return Some(Err(format!("CSV row {row}: a short-answer row needs `correct_text` (a sample answer)")));
                }
                Kind::Short { sample }
            }
            "blank" | "fib" | "num" | "numeric" | "fillintheblank" | "numericanswer" => {
                let text = first_non_empty(get(cols.correct_text), get(cols.correct));
                if text.is_empty() {
                    return Some(Err(format!("CSV row {row}: a blank row needs `correct_text` (the accepted answer)")));
                }
                Kind::Blank { accept: vec![vec![text]] }
            }
            other => {
                return Some(Err(format!(
                    "CSV row {row}: unsupported type {other:?} — supported: mc, ma, tf, short/ess, blank/fib/num; matching must be imported as Blackboard text"
                )));
            }
        };
        let raw_points = get(cols.points);
        let points = if raw_points.is_empty() { None } else { raw_points.parse::<f64>().ok().filter(|p| p.is_finite()) };
        let topics: Vec<String> = get(cols.topic).split([';', ',', '|']).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect();
        let mut q = make_question(stem, kind);
        q.points = points;
        q.topics = topics;
        questions.push(q);
    }
    if questions.is_empty() {
        return Some(Err("The CSV header names questions but there are no data rows.".into()));
    }
    Some(Ok(questions))
}

/// Reads the header row into column positions. `None` unless `type` and `stem` both appear.
fn csv_cols(header: &[String]) -> Option<CsvCols> {
    let mut ty: Option<usize> = None;
    let mut stem: Option<usize> = None;
    let mut points: Option<usize> = None;
    let mut topic: Option<usize> = None;
    let mut correct: Option<usize> = None;
    let mut correct_text: Option<usize> = None;
    let mut answers = [None::<usize>; 8];
    for (i, cell) in header.iter().enumerate() {
        let key = cell.trim().to_ascii_lowercase().replace([' ', '-', '_'], "");
        match key.as_str() {
            "type" | "kind" | "qtype" => ty = ty.or(Some(i)),
            "stem" | "question" | "questiontext" => stem = stem.or(Some(i)),
            "points" | "pointvalue" | "value" => points = points.or(Some(i)),
            "topic" | "topics" | "categories" => topic = topic.or(Some(i)),
            "correct" | "answer" | "key" | "answers" => correct = correct.or(Some(i)),
            "correcttext" | "answertext" | "keytext" => correct_text = correct_text.or(Some(i)),
            "a" | "b" | "c" | "d" | "e" | "f" | "g" | "h" => {
                let idx = key.as_bytes()[0] - b'a';
                answers[idx as usize] = answers[idx as usize].or(Some(i));
            }
            _ => {}
        }
    }
    match (ty, stem) {
        (Some(ty), Some(stem)) => Some(CsvCols { ty, stem, points, topic, answers, correct, correct_text }),
        _ => None,
    }
}

struct CsvCols {
    ty: usize,
    stem: usize,
    points: Option<usize>,
    topic: Option<usize>,
    answers: [Option<usize>; 8],
    correct: Option<usize>,
    correct_text: Option<usize>,
}

/// `correct` cell letters/digits → answer indexes (`b` → 1, `a,c` → [0, 2], `1` → 0).
fn correct_letters(s: &str) -> Vec<usize> {
    s.chars()
        .filter_map(|c| {
            if c.is_whitespace() || c == ',' || c == ';' {
                return None;
            }
            if let Some(d) = c.to_digit(10) {
                if d >= 1 { Some(d as usize - 1) } else { None }
            } else {
                let lower = c.to_ascii_lowercase();
                if ('a'..='h').contains(&lower) { Some(lower as usize - 'a' as usize) } else { None }
            }
        })
        .collect()
}

/// First non-empty of two option-like cells.
fn first_non_empty(a: String, b: String) -> String {
    if a.is_empty() { b } else { a }
}

/// CSV records: lines, except that a newline inside a double-quoted cell stays in that cell
/// (spreadsheets write multi-line cells that way). `\r\n` reads as `\n`.
fn csv_records(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in text.replace("\r\n", "\n").chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes; // a doubled `""` toggles twice: still inside
                cur.push(c);
            }
            '\n' if !in_quotes => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|r| r.trim_end_matches('\r').to_string()).collect()
}

/// Splits one CSV record, honoring double quotes and doubled `""` inside them; a quoted cell
/// may hold newlines (its line breaks are kept).
fn split_csv_line(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => cells.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    cells.push(cur);
    cells
}

// ---------- JSON: Test Parrot package, record, and legacy bank ----------

/// The Test Parrot JSON entry point. A `.json` file is claimed even when it fails;
/// otherwise only content that really looks Test Parrot (`test-parrot/package` or
/// `test-parrot/question-bank` in the bytes) or a legacy `"kind"` bank is claimed.
fn try_json(text: &str, fallback_name: &str, forced: bool) -> Option<Result<ImportDoc, String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let claimed = forced || trimmed.contains("test-parrot/");
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(v) => {
            match v.get("format").and_then(serde_json::Value::as_str).unwrap_or("") {
                PACKAGE_FORMAT => return Some(parse_package(&v, fallback_name)),
                RECORD_FORMAT => return Some(record_import(&v, fallback_name)),
                _ => {}
            }
            if v.get("kind").and_then(serde_json::Value::as_str) == Some(LEGACY_KIND) {
                return Some(legacy_bank(&v, fallback_name));
            }
            if claimed {
                return Some(Err(
                    "This JSON mentions Test Parrot formats but carries none: expected \"format\": \"test-parrot/package\" or \"test-parrot/question-bank\".".into(),
                ));
            }
            None
        }
        Err(e) => {
            if claimed {
                Some(Err(format!(
                    "That file is not valid JSON ({e}). A Test Parrot package looks like {{\"format\": \"test-parrot/package\", \"questionBanks\": [..], \"exams\": [..]}}"
                )))
            } else {
                None // not a JSON file we claim — let the text parsers take a look
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct JsonBank {
    #[serde(default)]
    name: String,
    #[serde(default)]
    questions: Vec<Question>,
}

/// The legacy invented shape (`"kind": "test-parrot-question-bank"`): a full bank.
fn legacy_bank(v: &serde_json::Value, fallback_name: &str) -> Result<ImportDoc, String> {
    let bank: JsonBank = match serde_json::from_value(v.clone()) {
        Ok(b) => b,
        Err(e) => return Err(format!("The Test Parrot question-bank JSON is malformed: {e}")),
    };
    if bank.questions.is_empty() {
        return Err("The question bank holds no questions — its \"questions\" list is empty or missing.".into());
    }
    let questions = bank.questions.into_iter().map(|mut q| { q.id = crate::model::new_id(); q }).collect();
    let name = if bank.name.trim().is_empty() { fallback_name.to_string() } else { bank.name };
    Ok(ImportDoc { name, questions, exams: Vec::new() })
}

/// A bare Question Bank Record: `{"format": .., "bank": {"name", "questions"}, "media"}`.
fn record_import(v: &serde_json::Value, fallback_name: &str) -> Result<ImportDoc, String> {
    let media = build_media(v);
    let bank = v.get("bank").ok_or("The Question Bank Record has no \"bank\" object")?;
    let (name, pairs) = tp_bank_questions(bank, &media)?;
    let name = if name.trim().is_empty() { fallback_name.to_string() } else { name };
    Ok(ImportDoc { name, questions: pairs.into_iter().map(|(_, q)| q).collect(), exams: Vec::new() })
}

/// Media assets of one record, keyed by their digest (`sha256:…` without the prefix).
fn build_media(record: &serde_json::Value) -> std::collections::HashMap<String, serde_json::Value> {
    let mut map = std::collections::HashMap::new();
    if let Some(list) = record.get("media").and_then(serde_json::Value::as_array) {
        for entry in list {
            if let Some(id) = entry.get("id").and_then(serde_json::Value::as_str).and_then(|a| a.strip_prefix("sha256:")) {
                map.insert(id.to_string(), entry.clone());
            }
        }
    }
    map
}

/// The inline base64 an exporter may embed beside a media entry (`data`, `bytes`, or
/// `base64` key); the 0.10.0 schema names none of these keys, so attachment is a bonus.
fn media_bytes(entry: &serde_json::Value) -> Option<Vec<u8>> {
    ["data", "bytes", "base64"]
        .iter()
        .find_map(|k| entry.get(*k).and_then(serde_json::Value::as_str))
        .and_then(base64_decode)
}

/// A stem's first resolvable picture becomes the question's image; the rest stay text.
fn attach_first_image(images: &[String], media: &std::collections::HashMap<String, serde_json::Value>, q: &mut Question) {
    for hex in images {
        let Some(entry) = media.get(hex.as_str()) else { continue };
        if let Some(bytes) = media_bytes(entry)
            && let Ok(img) = crate::store::put_image(&bytes)
        {
            q.image = Some(img);
            return;
        }
    }
}

// ---------- Test Parrot question mapping ----------

/// One whole package: every bank's questions into one doc, plus best-effort exams.
fn parse_package(v: &serde_json::Value, fallback_name: &str) -> Result<ImportDoc, String> {
    let banks = arr(v, "questionBanks");
    if banks.is_empty() {
        return Err("The Test Parrot package has no \"questionBanks\".".into());
    }
    let mut name = String::new();
    let mut questions: Vec<Question> = Vec::new();
    // "bankId/packageQuestionId" from the package → this import's fresh question id.
    let mut lookup: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut first_err: Option<String> = None;
    for entry in banks {
        let record = entry.get("record").ok_or("A package \"questionBanks\" entry has no embedded \"record\"")?;
        let bank = record.get("bank").ok_or("An embedded question-bank record has no \"bank\" object")?;
        let media = build_media(record);
        match tp_bank_questions(bank, &media) {
            Ok((bank_name, pairs)) => {
                if name.is_empty() {
                    name = bank_name;
                }
                let bank_id = entry.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
                for (pkg_qid, q) in pairs {
                    lookup.insert(format!("{bank_id}/{pkg_qid}"), q.id.clone());
                    questions.push(q);
                }
            }
            Err(e) => first_err = first_err.or_else(|| Some(e)),
        }
    }
    if questions.is_empty() {
        return Err(first_err.unwrap_or_else(|| "the package's question banks hold no importable questions".into()));
    }
    let name = if name.trim().is_empty() { fallback_name.to_string() } else { name };
    Ok(ImportDoc { name, questions, exams: package_exams(v, &lookup) })
}

/// A bank object's questions, in authored order, as (package id, fresh Question) pairs.
/// All-or-nothing per bank: a question that fails names itself in the error.
fn tp_bank_questions(bank: &serde_json::Value, media: &std::collections::HashMap<String, serde_json::Value>) -> Result<(String, Vec<(String, Question)>), String> {
    let raw = bank.get("questions").and_then(serde_json::Value::as_array).ok_or("The bank has no \"questions\" array")?;
    let mut pairs: Vec<(String, Question)> = Vec::new();
    let mut first_err: Option<String> = None;
    for item in raw {
        match tp_question(item, media) {
            Ok(q) => pairs.push(q),
            Err(e) => first_err = first_err.or_else(|| Some(e)),
        }
    }
    if pairs.is_empty() {
        return Err(first_err.unwrap_or_else(|| "the bank holds no questions".into()));
    }
    Ok((field_str(bank, "name").unwrap_or("").to_string(), pairs))
}

fn field_str<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(serde_json::Value::as_str)
}

/// One record question → (package id, model::Question); Kind mapping is best effort.
fn tp_question(item: &serde_json::Value, media: &std::collections::HashMap<String, serde_json::Value>) -> Result<(String, Question), String> {
    let pkg_id = field_str(item, "id").filter(|s| !s.is_empty()).ok_or("every question needs an id")?.to_string();
    let ty = field_str(item, "type").ok_or_else(|| format!("question {pkg_id} has no type"))?.to_string();
    let stem_node = item.get("stem").ok_or_else(|| format!("question {pkg_id} has no stem document"))?;
    let flat = flatten_doc(stem_node);
    let kind = match ty.as_str() {
        "multiple-choice" => tp_choice_answers(item).map(|answers| Kind::Mc { answers }),
        "true-false" => tp_true_false(item),
        "matching" => tp_matching(item),
        "fill-in-the-blank" if flat.blanks.is_empty() => {
            Err(format!("question {pkg_id} has no ___ blanks in its stem"))
        }
        "fill-in-the-blank" => Ok(Kind::Blank { accept: flat.blanks.iter().map(|b| vec![b.clone()]).collect() }),
        "short-answer" => Ok(Kind::Short { sample: item.get("suggestedAnswer").map(doc_text).unwrap_or_default() }),
        "multipart" => tp_parts(item).map(|parts| Kind::Multipart { parts }),
        other => Err(format!("question {pkg_id}: unsupported Test Parrot type {other:?}")),
    }
    .map_err(|e| format!("question {pkg_id}: {e}"))?;
    let mut q = make_question(flat.text, kind);
    attach_first_image(&flat.images, media, &mut q);
    q.difficulty = match field_str(item, "difficulty") {
        Some("easy") => Some(Difficulty::Low),
        Some("medium") => Some(Difficulty::Medium),
        Some("hard") => Some(Difficulty::High),
        _ => None,
    };
    q.topics = arr(item, "topics").iter().filter_map(serde_json::Value::as_str).filter(|t| !t.is_empty()).map(str::to_string).collect();
    q.points = item.get("points").and_then(serde_json::Value::as_f64).filter(|p| p.is_finite()).filter(|p| *p > 0.0);
    Ok((pkg_id, q))
}

/// The `choices` array of a question, part or subpart → model answers (all three share
/// the same choice shape: `content` document, `correct`, optional `locked`).
fn tp_choice_answers(item: &serde_json::Value) -> Result<Vec<Answer>, String> {
    let Some(choices) = item.get("choices").and_then(serde_json::Value::as_array) else {
        return Err("the question has no \"choices\" array".into());
    };
    let answers: Vec<Answer> = choices
        .iter()
        .map(|c| Answer {
            text: doc_text(c),
            correct: c.get("correct").and_then(serde_json::Value::as_bool).unwrap_or(false),
            locked: c.get("locked").and_then(serde_json::Value::as_bool).unwrap_or(false),
        })
        .collect();
    if answers.is_empty() { Err("the question has no choices".into()) } else { Ok(answers) }
}

fn tp_true_false(item: &serde_json::Value) -> Result<Kind, String> {
    let choices = item
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .ok_or("a True/False question needs exactly two choices (True, False)")?;
    if choices.len() != 2 {
        return Err(format!(
            "a True/False question needs exactly two choices (True, False); this record has {}",
            choices.len()
        ));
    }
    match choices.iter().position(|c| c.get("correct").and_then(serde_json::Value::as_bool) == Some(true)) {
        Some(0) => Ok(Kind::Tf { correct: true }),
        Some(1) => Ok(Kind::Tf { correct: false }),
        _ => Err("a True/False question needs exactly one of its two choices marked correct".into()),
    }
}

fn tp_matching(item: &serde_json::Value) -> Result<Kind, String> {
    let prompts = item.get("prompts").and_then(serde_json::Value::as_array).ok_or("a matching question has no \"prompts\"")?;
    let word_bank = item.get("wordBank").and_then(serde_json::Value::as_array).ok_or("a matching question has no \"wordBank\"")?;
    if prompts.is_empty() {
        return Err("a matching question needs at least one prompt".into());
    }
    let mut pairs: Vec<Pair> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for p in prompts {
        // Best effort: a prompt that names no wordBank answer is skipped.
        let Some(ans) = p.get("answer").and_then(serde_json::Value::as_str) else { continue };
        let Some(right) = word_bank.iter().find(|w| field_str(w, "id") == Some(ans)) else { continue };
        pairs.push(Pair { left: doc_text(p), right: doc_text(right) });
        used.push(ans.to_string());
    }
    if pairs.is_empty() {
        return Err("no matching pair resolves — every prompt's \"answer\" must name a wordBank id".into());
    }
    let distractors: Vec<String> = word_bank
        .iter()
        .filter(|w| field_str(w, "id").is_none_or(|id| !used.iter().any(|u| u == id)))
        .map(doc_text)
        .collect();
    Ok(Kind::Matching { pairs, distractors })
}

fn tp_parts(item: &serde_json::Value) -> Result<Vec<Part>, String> {
    let raw = item
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .filter(|p| !p.is_empty())
        .ok_or("a multipart question needs at least one part")?;
    let mut out: Vec<Part> = Vec::new();
    for p in raw {
        tp_one_part(p, &mut out)?;
    }
    Ok(out)
}

/// One part → one `model::Part`; a part carrying subparts (a lead-in stem, no answer of
/// its own) becomes its lead-in stem plus one part per subpart, since `PartKind` cannot
/// nest. Subparts' own points are not representable in `model::Part` and are dropped.
fn tp_one_part(p: &serde_json::Value, out: &mut Vec<Part>) -> Result<(), String> {
    let part_id = field_str(p, "id").unwrap_or("?").to_string();
    let stem = p.get("stem").map(doc_text).unwrap_or_default();
    let subparts = arr(p, "subparts");
    if subparts.is_empty() {
        let kind = tp_answer_kind(p).map_err(|e| format!("part {part_id}: {e}"))?;
        out.push(Part { stem, kind });
        return Ok(());
    }
    out.push(Part { stem, kind: PartKind::Short { sample: String::new() } });
    for sp in subparts {
        let sp_id = field_str(sp, "id").unwrap_or("?").to_string();
        match tp_answer_kind(sp) {
            Ok(kind) => out.push(Part { stem: sp.get("stem").map(doc_text).unwrap_or_default(), kind }),
            Err(e) => return Err(format!("subpart {sp_id}: {e}")),
        }
    }
    Ok(())
}

/// A part's or subpart's own answer: multiple-choice choices, else a suggested answer.
fn tp_answer_kind(item: &serde_json::Value) -> Result<PartKind, String> {
    if item.get("choices").is_some() {
        return Ok(PartKind::Mc { answers: tp_choice_answers(item)? });
    }
    if let Some(sa) = item.get("suggestedAnswer") {
        return Ok(PartKind::Short { sample: doc_text(sa) });
    }
    Err("the part answers nothing (needs choices or a suggested answer)".into())
}

/// Package exam records → best-effort `model::Exam`s: items in `positions` order, resolved
/// against this import's fresh question ids, with `bank_id` empty for the shell to fill.
/// Sections, columns, answerOrder, hiddenAnswers, margins and paper style are ignored, and
/// exams with no resolvable position are skipped.
fn package_exams(v: &serde_json::Value, lookup: &std::collections::HashMap<String, String>) -> Vec<Exam> {
    let mut out = Vec::new();
    let Some(exams) = v.get("exams").and_then(serde_json::Value::as_array) else { return out };
    for record in exams {
        let Some(name) = field_str(record, "name").filter(|n| !n.is_empty()) else { continue };
        let mut items: Vec<ExamItem> = Vec::new();
        for pos in arr(record, "positions") {
            let Some(bank) = pos.pointer("/question/bank").and_then(serde_json::Value::as_str) else { continue };
            let Some(qid) = pos.pointer("/question/question").and_then(serde_json::Value::as_str) else { continue };
            let Some(q) = lookup.get(&format!("{bank}/{qid}")) else { continue };
            items.push(ExamItem::new(String::new(), q.clone()));
        }
        if items.is_empty() {
            continue;
        }
        let header = record
            .get("header")
            .and_then(|h| h.get("first"))
            .and_then(serde_json::Value::as_str)
            .filter(|h| !h.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_HEADER.to_string());
        out.push(Exam { paper_style: Default::default(), margins: None,
            id: crate::model::new_id(),
            name: name.to_string(),
            header,
            title: None,
            items,
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            // Fresh import: zero is "never edited since import".
            modified: 0,
        });
    }
    out
}

// ---------- Test Parrot document flattening ----------

/// One flattened stem/suggested-answer document: newline-joined block text, the `___`
/// answers in stem order, and the image assets the document shows, in order.
#[derive(Default)]
struct Flat {
    text: String,
    blanks: Vec<String>,
    images: Vec<String>,
}

/// A document → stem markup (`docs/stem-format.md`): each block is one chunk, chunks are
/// separated by a blank line (so paragraphs stay paragraphs), lists become `• ` / `1. ` lines,
/// tables pipe rows (with a `---|---` line under a marked header row), quotes `> ` lines.
fn flatten_doc(document: &serde_json::Value) -> Flat {
    let mut chunks: Vec<String> = Vec::new();
    let mut blanks: Vec<String> = Vec::new();
    let mut images: Vec<String> = Vec::new();
    for block in children(document) {
        flatten_block(block, &mut chunks, &mut blanks, &mut images);
    }
    Flat { text: chunks.join("\n\n"), blanks, images }
}

/// A node with `content` flattened to plain text — choices, prompts, answers; a document
/// keeps its blocks as lines, anything else flattens inline.
fn doc_text(v: &serde_json::Value) -> String {
    // Choice/prompt/wordBank/part entries wrap their document in `content`; the value may
    // also already be the document itself (stray inline node).
    let document = if node_type(v) == "document" { v } else { v.get("content").unwrap_or(&serde_json::Value::Null) };
    if !document.is_object() {
        return String::new();
    }
    flatten_doc(document).text
}

fn children(node: &serde_json::Value) -> &[serde_json::Value] {
    node.get("content").and_then(serde_json::Value::as_array).map_or(&[], |c| c.as_slice())
}

/// A JSON array field, or an empty slice when it is absent or not an array.
fn arr<'a>(v: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    v.get(key).and_then(serde_json::Value::as_array).map_or(&[], |c| c.as_slice())
}

fn node_type<'a>(node: &'a serde_json::Value) -> &'a str {
    node.get("type").and_then(serde_json::Value::as_str).unwrap_or("")
}

fn asset_hex(node: &serde_json::Value) -> Option<&str> {
    node.get("asset").and_then(serde_json::Value::as_str).and_then(|a| a.strip_prefix("sha256:"))
}

/// Block-level node types (everything else is inline content).
fn is_block(t: &str) -> bool {
    matches!(
        t,
        "paragraph" | "heading" | "blockquote" | "panel" | "document" | "bullet-list" | "ordered-list" | "list-item" | "code-block" | "rule" | "table" | "side-by-side" | "block-image" | "side-by-side-panel" | "horizontal-rule" | "divider"
    ) || is_task_list(t) || is_task_item(t)
}

/// Task (checkbox) lists: Test Parrot's slash menu offers one; the node names follow its
/// kebab-case vocabulary, with the camel-case and snake-case spellings other editors use.
fn is_task_list(t: &str) -> bool {
    matches!(t, "task-list" | "taskList" | "task_list")
}
fn is_task_item(t: &str) -> bool {
    matches!(t, "task-item" | "taskItem" | "task_item")
}

/// A task item's tick: `checked` on the node or in its `attrs`.
fn task_checked(item: &serde_json::Value) -> bool {
    item.get("checked").or_else(|| item.get("attrs").and_then(|a| a.get("checked"))).and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// A plain list item that carries a tick anyway (Milkdown, which Test Parrot's editor is,
/// keeps `checked: true|false` on `list_item`; `null` is an ordinary item).
fn has_tick(item: &serde_json::Value) -> bool {
    item.get("checked").or_else(|| item.get("attrs").and_then(|a| a.get("checked"))).is_some_and(serde_json::Value::is_boolean)
}

/// The box a task item prints with: ☑ ticked, ☐ open (both in Source Serif 4).
fn task_box(checked: bool) -> &'static str {
    if checked { "\u{2611} " } else { "\u{2610} " }
}

/// One block → chunks of markup (one per paragraph-like unit); empty paragraphs (spacers)
/// add nothing. Unknown or stray inline nodes at block level become a paragraph when they
/// say anything.
fn flatten_block(block: &serde_json::Value, chunks: &mut Vec<String>, blanks: &mut Vec<String>, images: &mut Vec<String>) {
    let push = |chunks: &mut Vec<String>, s: String| {
        let s = s.trim_end().to_string();
        if !s.trim().is_empty() {
            chunks.push(s);
        }
    };
    match node_type(block) {
        "paragraph" => {
            let mut line = String::new();
            for n in children(block) {
                flatten_inline(n, &mut line, blanks, images);
            }
            push(chunks, if tp_centered(block) { center_markup(&line) } else { line });
        }
        "heading" => {
            let mut line = String::new();
            for n in children(block) {
                flatten_inline(n, &mut line, blanks, images);
            }
            let level = block
                .get("level")
                .or_else(|| block.get("attrs").and_then(|a| a.get("level")))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1);
            let h = heading_markup(line.trim(), level);
            push(chunks, if tp_centered(block) { center_markup(&h) } else { h });
        }
        "blockquote" => {
            let mut inner: Vec<String> = Vec::new();
            for c in children(block) {
                flatten_block(c, &mut inner, blanks, images);
            }
            let quoted: Vec<String> = inner.join("\n").lines().filter(|l| !l.trim().is_empty()).map(|l| format!("> {l}")).collect();
            push(chunks, quoted.join("\n"));
        }
        "side-by-side" => {
            // one panel per child (`side-by-side-panel` / `panel`; anything else is a panel of its own)
            let mut panels: Vec<String> = Vec::new();
            for c in children(block) {
                let mut inner: Vec<String> = Vec::new();
                if matches!(node_type(c), "side-by-side-panel" | "panel") {
                    for k in children(c) {
                        flatten_block(k, &mut inner, blanks, images);
                    }
                } else {
                    flatten_block(c, &mut inner, blanks, images);
                }
                panels.push(inner.join("\n\n"));
            }
            push(chunks, side_markup(&panels));
        }
        t if is_task_item(t) => {
            for c in children(block) {
                flatten_block(c, chunks, blanks, images);
            }
        }
        t if is_task_list(t) => {
            let mut lines: Vec<String> = Vec::new();
            list_lines(block, 0, &mut lines, blanks, images);
            push(chunks, lines.join("\n"));
        }
        "panel" | "side-by-side-panel" | "document" | "list-item" => {
            for c in children(block) {
                flatten_block(c, chunks, blanks, images);
            }
        }
        "bullet-list" | "ordered-list" => {
            let mut lines: Vec<String> = Vec::new();
            list_lines(block, 0, &mut lines, blanks, images);
            push(chunks, lines.join("\n"));
        }
        "code-block" => {
            let mut line = String::new();
            flatten_inline(block, &mut line, blanks, images);
            push(chunks, line);
        }
        "rule" | "horizontal-rule" | "divider" => chunks.push("---".to_string()),
        "table" => {
            let t = table_lines(block, blanks, images);
            push(chunks, t);
        }
        "inline-image" | "block-image" => {
            if let Some(hex) = asset_hex(block) {
                images.push(hex.to_string());
            }
            chunks.push(picture_text(node_alt(block)));
        }
        _ => {
            let mut line = String::new();
            flatten_inline(block, &mut line, blanks, images);
            push(chunks, line);
        }
    }
}

/// A node's content as ONE line (list items, table cells): block children joined by a space,
/// hard breaks as spaces, nested lists left to the caller.
fn one_line(node: &serde_json::Value, blanks: &mut Vec<String>, images: &mut Vec<String>, skip_lists: bool) -> String {
    let mut out = String::new();
    let add = |out: &mut String, s: &str| {
        let s = s.replace('\n', " ");
        let s = s.trim();
        if s.is_empty() {
            return;
        }
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push_str(s);
    };
    let mut inline = String::new();
    for c in children(node) {
        let t = node_type(c);
        if skip_lists && (matches!(t, "bullet-list" | "ordered-list") || is_task_list(t)) {
            continue;
        }
        if is_block(t) {
            add(&mut out, &std::mem::take(&mut inline));
            let mut sub: Vec<String> = Vec::new();
            flatten_block(c, &mut sub, blanks, images);
            for s in sub {
                // Nested quote/list/heading markers inside one line read as plain text.
                let s: Vec<String> = s
                    .lines()
                    .filter(|l| crate::richtext::fence(l).is_none() && !crate::richtext::divider(l))
                    .map(|l| l.trim_start_matches("> "))
                    .map(|l| l.get(crate::richtext::heading_prefix_len(l)..).unwrap_or(l).to_string())
                    .collect();
                add(&mut out, &s.join(" "));
            }
        } else {
            flatten_inline(c, &mut inline, blanks, images);
        }
    }
    add(&mut out, &inline);
    out
}

/// A bullet/ordered list → `• item` / `1. item` lines; nested lists follow their item,
/// indented two spaces per level.
fn list_lines(list: &serde_json::Value, depth: usize, lines: &mut Vec<String>, blanks: &mut Vec<String>, images: &mut Vec<String>) {
    let ordered = node_type(list) == "ordered-list";
    let tasks = is_task_list(node_type(list));
    let start = list.get("start").and_then(serde_json::Value::as_u64).unwrap_or(1) as usize;
    let indent = "  ".repeat(depth);
    for (i, item) in children(list).iter().enumerate() {
        // A task list is a bullet list whose items start with their box.
        let marker = if ordered {
            format!("{}. ", start + i)
        } else if tasks || is_task_item(node_type(item)) || has_tick(item) {
            format!("• {}", task_box(task_checked(item)))
        } else {
            "• ".to_string()
        };
        let text = if node_type(item) == "list-item" || is_task_item(node_type(item)) {
            one_line(item, blanks, images, true)
        } else {
            let mut s = String::new();
            flatten_inline(item, &mut s, blanks, images);
            s.replace('\n', " ").trim().to_string()
        };
        lines.push(format!("{indent}{marker}{text}").trim_end().to_string());
        for c in children(item) {
            if matches!(node_type(c), "bullet-list" | "ordered-list") || is_task_list(node_type(c)) {
                list_lines(c, depth + 1, lines, blanks, images);
            }
        }
    }
}

/// A table → pipe rows, padded to one cell count (merged cells span with empty cells); a
/// `---|---` line follows the first row when the source marks it a header. Rows whose first
/// or last cell is empty get outer pipes so the empty cell survives. A one-column table is
/// just its lines.
fn table_lines(table: &serde_json::Value, blanks: &mut Vec<String>, images: &mut Vec<String>) -> String {
    let flag = |v: &serde_json::Value| v.get("header").and_then(serde_json::Value::as_bool) == Some(true);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut first_is_header = false;
    for (r, row) in children(table).iter().enumerate() {
        let cells = children(row);
        let mut out: Vec<String> = Vec::new();
        for cell in cells {
            out.push(one_line(cell, blanks, images, false).replace('|', "/"));
            let span = cell.get("colspan").and_then(serde_json::Value::as_u64).unwrap_or(1).max(1);
            for _ in 1..span {
                out.push(String::new());
            }
        }
        if r == 0 {
            first_is_header = flag(row) || (!cells.is_empty() && cells.iter().all(flag));
        }
        rows.push(out);
    }
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols < 2 {
        return rows.iter().map(|r| r.join(" ")).collect::<Vec<_>>().join("\n");
    }
    for r in &mut rows {
        r.resize(cols, String::new());
    }
    let outer = rows.iter().any(|r| r[0].is_empty() || r[cols - 1].is_empty());
    let fmt = |cells: &[String]| if outer { format!("| {} |", cells.join(" | ")) } else { cells.join(" | ") };
    let mut lines: Vec<String> = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        lines.push(fmt(row));
        if r == 0 && first_is_header && rows.len() > 1 {
            lines.push(vec!["---"; cols].join("|"));
        }
    }
    lines.join("\n")
}

/// A heading's markup: `#`..`###` for levels 1-3. Stem markup has no deeper heading (a `####`
/// line has always been plain text, and stems that hold one must keep reading that way), so
/// levels 4-6 print distinct from level 3 and from each other with inline styles on a line of
/// their own: 4 bold, 5 bold italic, 6 italic. Text the heading syntax cannot carry (several
/// lines, a `___` blank) falls back to bold text; text that already has `*` markers stays as is.
fn heading_markup(core: &str, level: u64) -> String {
    if level >= 4 && !core.is_empty() && !core.contains('\n') && !core.contains('*') {
        let mark = match level {
            4 => "**",
            5 => "***",
            _ => "*",
        };
        let styled = format!("{mark}{core}{mark}");
        // only when it reads back as one styled run (e.g. `* a` edge spaces would not)
        if crate::richtext::inline(&styled).iter().all(|sp| sp.text.is_empty() || (sp.bold == (level <= 5) && sp.italic == (level >= 5))) {
            return styled;
        }
        return core.to_string();
    }
    let n = level.clamp(1, 3) as usize;
    let h = format!("{} {core}", "#".repeat(n));
    if core.is_empty() {
        String::new()
    } else if !core.contains('\n') && crate::richtext::heading(&h).is_some() {
        h
    } else if core.contains('\n') || core.contains('*') {
        core.to_string()
    } else {
        format!("**{core}**")
    }
}

/// Side-by-side panels' markup; nested groups inside a panel are unwrapped (no nesting), and
/// a single panel is just its content.
fn side_markup(panels: &[String]) -> String {
    // nested side-by-side fences go; a centred group inside a panel stays
    let clean_panel = |p: &str| -> String {
        use crate::richtext::Fence;
        let mut in_center = false;
        let mut out: Vec<&str> = Vec::new();
        for l in p.lines() {
            match crate::richtext::fence(l) {
                None => out.push(l),
                Some(Fence::Center) if !in_center => {
                    in_center = true;
                    out.push(l);
                }
                Some(Fence::Close) if in_center => {
                    in_center = false;
                    out.push(l);
                }
                Some(_) => {}
            }
        }
        out.join("\n").trim_matches('\n').to_string()
    };
    let clean: Vec<String> = panels.iter().map(|p| clean_panel(p)).collect();
    if clean.len() < 2 {
        return clean.join("\n\n");
    }
    let mut out = vec![crate::richtext::FENCE_OPEN.to_string()];
    for (k, p) in clean.iter().enumerate() {
        if k > 0 {
            out.push(crate::richtext::FENCE_NEXT.to_string());
        }
        out.push(p.clone());
    }
    out.push(crate::richtext::FENCE_CLOSE.to_string());
    out.join("\n")
}

/// A Test Parrot paragraph or heading aligned to the centre: `"align": "center"` on the node
/// (its export) or in its `attrs` (`align`, or `textAlign` as TipTap writes it).
fn tp_centered(node: &serde_json::Value) -> bool {
    let attrs = node.get("attrs");
    [node.get("align"), attrs.and_then(|a| a.get("align")), attrs.and_then(|a| a.get("textAlign"))]
        .into_iter()
        .flatten()
        .any(|v| v.as_str().is_some_and(|a| a.eq_ignore_ascii_case("center") || a.eq_ignore_ascii_case("centre")))
}

/// `body` (blocks of markup) as a centred group: `::: center` ... `:::` around it. Centred
/// groups do not nest, so fence lines already inside are dropped; empty text stays empty.
pub(crate) fn center_markup(body: &str) -> String {
    use crate::richtext::Fence;
    let inner: Vec<&str> = body.lines().filter(|l| !matches!(crate::richtext::fence(l), Some(Fence::Center | Fence::Close))).collect();
    let inner = inner.join("\n");
    let inner = inner.trim_matches('\n');
    if inner.trim().is_empty() {
        return String::new();
    }
    format!("{}\n{inner}\n{}", crate::richtext::FENCE_CENTER, crate::richtext::FENCE_CLOSE)
}

fn node_alt(node: &serde_json::Value) -> &str {
    node.get("alt").and_then(serde_json::Value::as_str).or_else(|| node.get("caption").and_then(serde_json::Value::as_str)).unwrap_or("")
}

fn picture_text(alt: &str) -> String {
    if alt.is_empty() { "[picture]".to_string() } else { format!("[picture: {alt}]") }
}

/// One node's inline text: text, hard breaks, `___` blanks (each collects its answer),
/// math in `$…$`, pictures as placeholders (asset collected); other types recurse into
/// their `content`, so paragraphs, links and marks flatten transparently.
fn flatten_inline(node: &serde_json::Value, out: &mut String, blanks: &mut Vec<String>, images: &mut Vec<String>) {
    match node_type(node) {
        "text" => out.push_str(&marked_text(node)),
        "hard-break" => out.push('\n'),
        "blank" => {
            out.push_str(BLANK);
            let mut answer = String::new();
            for c in children(node) {
                flatten_inline(c, &mut answer, blanks, images);
            }
            blanks.push(answer);
        }
        "inline-math" | "display-math" => {
            out.push('$');
            out.push_str(node.get("source").and_then(serde_json::Value::as_str).unwrap_or(""));
            out.push('$');
        }
        "block-image" | "inline-image" => {
            if let Some(hex) = asset_hex(node) {
                images.push(hex.to_string());
            }
            out.push_str(&picture_text(node_alt(node)));
        }
        _ => {
            for c in children(node) {
                flatten_inline(c, out, blanks, images);
            }
        }
    }
}

/// A text node with its marks as stem markup: subscript/superscript as Unicode digits when
/// `richtext::unicode_script` covers the text, else `_{..}`/`^{..}`; italic `*..*` and bold
/// `**..**` wrap the text with its edge whitespace kept outside the markers (so they parse).
fn marked_text(node: &serde_json::Value) -> String {
    let text = node.get("text").and_then(serde_json::Value::as_str).unwrap_or("");
    let marks: Vec<&str> = arr(node, "marks").iter().filter_map(|m| m.as_str().or_else(|| m.get("type").and_then(serde_json::Value::as_str))).collect();
    let has = |names: &[&str]| marks.iter().any(|m| names.contains(m));
    if marks.is_empty() || text.trim().is_empty() {
        return text.to_string();
    }
    let lead = &text[..text.len() - text.trim_start().len()];
    let trail = &text[text.trim_end().len()..];
    let mut core = text.trim().to_string();
    // inline code (Test Parrot `inline-code`, Milkdown `inlineCode`, TipTap `code`) when the
    // markup can hold it (no backtick or `___` inside)
    if has(&["inline-code", "inlineCode", "inline_code", "code"]) && code_ok(&core) {
        core = format!("`{core}`");
    }
    let sub = has(&["subscript", "sub"]);
    let sup = has(&["superscript", "sup"]);
    if sub || sup {
        core = crate::richtext::unicode_script(&core, sup).unwrap_or_else(|| if sup { format!("^{{{core}}}") } else { format!("_{{{core}}}") });
    }
    if has(&["strike", "strikethrough", "s", "del"]) {
        core = format!("~~{core}~~");
    }
    if has(&["em", "emphasis", "italic"]) {
        core = format!("*{core}*");
    }
    if has(&["strong", "bold"]) {
        core = format!("**{core}**");
    }
    // A link mark ({"type": "link", "attrs": {"href": ...}}) wraps everything.
    if let Some(href) = arr(node, "marks").iter().find(|m| m.get("type").and_then(serde_json::Value::as_str) == Some("link")).and_then(|m| m.get("attrs")?.get("href")?.as_str()) {
        core = link_markup(&core, href);
    }
    format!("{lead}{core}{trail}")
}

/// Can `core` be written as `` `core` `` and read back as exactly that code?
fn code_ok(core: &str) -> bool {
    let l = crate::richtext::inline(&format!("`{core}`"));
    l.len() == 1 && l[0].code && l[0].text == core
}

/// RFC 4648 base64, standard alphabet. Whitespace ignored, trailing padding optional;
/// any other byte, or a stray 1-symbol tail chunk, fails.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let dec = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let body: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let body = trim_eq_pad(&body);
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        let mut digits: Vec<u8> = Vec::with_capacity(4);
        for &c in chunk {
            match dec(c) {
                Some(x) => digits.push(x),
                None => return None, // a stray '=' or real garbage inside the body
            }
        }
        let mut n = 0u32;
        for &x in &digits {
            n = (n << 6) | u32::from(x);
        }
        match digits.len() {
            4 => {
                out.push((n >> 16) as u8);
                out.push((n >> 8) as u8);
                out.push(n as u8);
            }
            3 => {
                out.push((n >> 10) as u8);
                out.push((n >> 2) as u8);
            }
            2 => out.push(((n >> 4) & 0xFF) as u8),
            0 => continue, // nothing but whitespace in this chunk
            _ => return None, // a stranded single symbol decodes to nothing
        }
    }
    Some(out)
}

/// Strips trailing '=' padding symbols (after whitespace removal).
fn trim_eq_pad(body: &[u8]) -> &[u8] {
    let mut end = body.len();
    while end > 0 && body[end - 1] == b'=' {
        end -= 1;
    }
    &body[..end]
}
// ---------- text decoding + helpers ----------

/// UTF-8 first (BOM stripped), else Latin-1 byte-for-byte: every byte is its own code point.
fn decode_text(data: &[u8]) -> String {
    // UTF-16 with a byte-order mark (Respondus and some Windows exports save this way).
    for (bom, le) in [([0xFF, 0xFE], true), ([0xFE, 0xFF], false)] {
        if let Some(rest) = data.strip_prefix(&bom) {
            let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|&c| if le { u16::from_le_bytes(c) } else { u16::from_be_bytes(c) }).collect();
            return String::from_utf16_lossy(&units);
        }
    }
    let body = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    match std::str::from_utf8(body) {
        Ok(s) => s.to_string(),
        Err(_) => body.iter().map(|&b| b as char).collect(),
    }
}

/// Filename stem for the suggested bank name; no filename means pasted questions.
fn doc_name(filename: &str) -> String {
    if filename.trim().is_empty() {
        return "Pasted questions".to_string();
    }
    let file = filename.rsplit(['/', '\\']).next().unwrap_or(filename).trim();
    let stem = match file.rsplit_once('.') {
        Some((s, _ext)) if !s.is_empty() => s,
        _ => file,
    };
    let stem = stem.trim();
    if stem.is_empty() { "Imported questions".to_string() } else { stem.to_string() }
}

/// Blocks are runs of non-blank lines; blank lines separate the questions.
fn split_blocks(text: &str) -> Vec<Vec<String>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            if !cur.is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(line.to_string());
        }
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    blocks
}

pub(crate) fn make_question(stem: String, kind: Kind) -> Question {
    let now = now_secs();
    Question {
        id: crate::model::new_id(),
        stem,
        kind,
        points: None,
        difficulty: None,
        topics: Vec::new(),
        image: None,
        explanation: None,
        figure: None,
        created: now,
        modified: now,
    }
}

/// `store::now()` is private, so parsers timestamp with `SystemTime` directly.
fn now_secs() -> i64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

// =====================================================================================
// Formats 6-15: Moodle GIFT, flashcard lists, Excel, Kahoot, Brightspace, Moodle XML,
// QTI 1.2/2.x/3.0 files and zips (Canvas, Common Cartridge), Respondus and text2qti text.
// =====================================================================================

/// Old binary Office files (.xls/.doc) are OLE2 compound documents, not readable here.
const OLE_ERR: &str = "That is an old binary Office file (.xls/.doc). Save it as .xlsx (or CSV) or .docx and add it again.";
/// Stem of a Matching question built from a flashcard (term/definition) list.
const TERM_MATCH_STEM: &str = "Match each term with its definition.";
/// Header cells a flashcard list may start with (that row is skipped).
const TERM_HEADERS: [&str; 12] = ["term", "terms", "definition", "definitions", "front", "back", "word", "meaning", "question", "answer", "prompt", "card"];
/// Most pairs per Matching question when a flashcard list is split into several.
const TERM_GROUP: usize = 10;

/// Lower-case file extension ("" when there is none).
fn ext_of(filename: &str) -> String {
    let file = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

fn is_zip(data: &[u8]) -> bool {
    data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06")
}

/// Pushes `s` as a chunk when it says anything.
fn push_chunk(chunks: &mut Vec<String>, s: String) {
    if !s.trim().is_empty() {
        chunks.push(s);
    }
}

/// Many lines (or chunks) as one line: lines trimmed, quote markers dropped, joined by spaces.
fn single_line(s: &str) -> String {
    s.lines().map(|l| l.trim().trim_start_matches("> ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Keeps the first copy of each string, in order.
fn dedup(v: &mut Vec<String>) {
    let mut seen = HashSet::new();
    v.retain(|s| seen.insert(s.clone()));
}

/// `a/b/../c` joined onto a base directory, `..`/`.` resolved (zip entry paths).
fn join_path(base: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// The directory part of a zip entry path ("" at the root).
fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// `%20`-style escapes decoded (invalid escapes kept literally).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Two choices reading True/False (one marked) and not a multi-answer: a TF question.
fn tf_from(answers: &[Answer], multi: bool) -> Option<Kind> {
    let word = |t: &str| t.trim().trim_end_matches(['.', '!']).to_ascii_lowercase();
    if multi || answers.len() != 2 || answers.iter().filter(|a| a.correct).count() != 1 {
        return None;
    }
    let w: Vec<String> = answers.iter().map(|a| word(&a.text)).collect();
    if !(w.contains(&"true".to_string()) && w.contains(&"false".to_string())) {
        return None;
    }
    let right = answers.iter().find(|a| a.correct)?;
    Some(Kind::Tf { correct: word(&right.text) == "true" })
}

/// A numeric answer given as a value and an absolute tolerance → accept variants
/// (center, center−tol, center+tol; just the value when the tolerance is zero).
fn num_with_tolerance(value: &str, tol: &str) -> Option<Vec<String>> {
    let (v, t) = (value.trim(), tol.trim());
    match parse_f64(t) {
        Some(x) if x != 0.0 => parse_num_answer(&format!("{v}±{t}")),
        _ => parse_num_answer(v),
    }
}

/// A numeric range `lo..hi` → center, lo, hi (formatted like the NUM tag's ranges).
fn num_range(lo: f64, hi: f64) -> Vec<String> {
    let mut v = vec![fmt_clean((lo + hi) / 2.0), fmt_clean(lo), fmt_clean(hi)];
    dedup(&mut v);
    v
}

/// A computed value without float noise: rounded to 9 decimals, trailing zeros dropped
/// (`(9.7 + 9.9) / 2` prints `9.8`, not `9.799999999999999`).
fn fmt_clean(v: f64) -> String {
    let s = format!("{v:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".to_string() } else { s.to_string() }
}

// ---------- XML (roxmltree) helpers ----------

type XNode<'a, 'i> = roxmltree::Node<'a, 'i>;

fn xml_doc(text: &str) -> Result<roxmltree::Document<'_>, String> {
    let opt = roxmltree::ParsingOptions { allow_dtd: true, ..roxmltree::ParsingOptions::default() };
    roxmltree::Document::parse_with_options(text, opt).map_err(|e| e.to_string())
}

/// An element's normalized local name: lower-case, a QTI 3 `qti-` prefix and dashes
/// dropped (`qti-simple-choice` and `simpleChoice` both read `simplechoice`).
fn qname(n: XNode<'_, '_>) -> String {
    if !n.is_element() {
        return String::new();
    }
    let l = n.tag_name().name().to_ascii_lowercase();
    l.strip_prefix("qti-").unwrap_or(&l).replace('-', "")
}

fn kid<'a, 'i>(n: XNode<'a, 'i>, name: &str) -> Option<XNode<'a, 'i>> {
    n.children().find(|c| qname(*c) == name)
}

fn kids<'a, 'i>(n: XNode<'a, 'i>, name: &str) -> Vec<XNode<'a, 'i>> {
    n.children().filter(|c| qname(*c) == name).collect()
}

fn desc<'a, 'i>(n: XNode<'a, 'i>, name: &str) -> Option<XNode<'a, 'i>> {
    n.descendants().find(|c| qname(*c) == name)
}

fn descs<'a, 'i>(n: XNode<'a, 'i>, name: &str) -> Vec<XNode<'a, 'i>> {
    n.descendants().filter(|c| qname(*c) == name).collect()
}

/// All text under a node, concatenated.
fn xtext(n: XNode<'_, '_>) -> String {
    n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect()
}

/// A camelCase attribute, also found under its QTI 3 kebab-case spelling.
fn xattr<'a>(n: XNode<'a, '_>, camel: &str) -> Option<&'a str> {
    n.attribute(camel).or_else(|| {
        let mut k = String::new();
        for c in camel.chars() {
            if c.is_ascii_uppercase() {
                k.push('-');
                k.push(c.to_ascii_lowercase());
            } else {
                k.push(c);
            }
        }
        n.attribute(k.as_str())
    })
}

// ---------- HTML → stem markup ----------

/// A tolerant HTML tree: what Moodle, Canvas and Blackboard put in question text.
#[derive(Clone, Debug)]
enum HNode {
    Text(String),
    El { name: String, attrs: Vec<(String, String)>, kids: Vec<HNode> },
}

impl HNode {
    fn attr(&self, key: &str) -> Option<&str> {
        match self {
            HNode::El { attrs, .. } => attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str()),
            HNode::Text(_) => None,
        }
    }
}

const VOID: [&str; 14] = ["br", "img", "hr", "input", "meta", "link", "col", "area", "base", "wbr", "source", "embed", "param", "track"];

const HTML_BLOCKS: [&str; 41] = [
    "p", "div", "section", "article", "center", "body", "html", "header", "footer", "main", "form", "fieldset", "figure", "figcaption", "dl", "dt", "dd",
    "address", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "table", "tr", "thead", "tbody", "tfoot", "caption", "blockquote", "pre", "hr", "nav",
    "aside", "head", "title", "prompt",
];

fn is_html_block(name: &str) -> bool {
    HTML_BLOCKS.contains(&name)
}

/// A paragraph-like HTML block set to the centre: `<center>`, `align="center"`, or a
/// `text-align: center` style (tables, rows and list parts are left as they are).
fn html_centered(node: &HNode) -> bool {
    let HNode::El { name, .. } = node else { return false };
    if name == "center" {
        return true;
    }
    const CAN: [&str; 16] = ["p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "section", "article", "header", "footer", "main", "figure", "figcaption", "address"];
    CAN.contains(&name.as_str())
        && (node.attr("align").is_some_and(|a| a.trim().eq_ignore_ascii_case("center"))
            || node.attr("style").is_some_and(|st| st.to_ascii_lowercase().replace([' ', '\t'], "").contains("text-align:center")))
}

struct OpenEl {
    name: String,
    attrs: Vec<(String, String)>,
    kids: Vec<HNode>,
}

fn close_top(stack: &mut Vec<OpenEl>) {
    if stack.len() > 1 {
        let o = stack.pop().expect("len > 1");
        stack.last_mut().expect("root stays").kids.push(HNode::El { name: o.name, attrs: o.attrs, kids: o.kids });
    }
}

fn flush_text(text: &mut String, stack: &mut [OpenEl]) {
    if !text.is_empty() {
        let t = decode_entities(&std::mem::take(text));
        stack.last_mut().expect("root stays").kids.push(HNode::Text(t));
    }
}

/// Index of the `>` closing a tag that starts at `s[0] == '<'` (quoted `>` skipped).
fn find_tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, b) in s.bytes().enumerate().skip(1) {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => return Some(i),
            None => {}
        }
    }
    None
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let ks = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        if ks == i {
            i += 1;
            continue;
        }
        let key = s[ks..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut val = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                val = s[vs..i].to_string();
                i += 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                val = s[vs..i].to_string();
            }
        }
        out.push((key, decode_entities(&val)));
    }
    out
}

/// Lenient HTML parse: unknown end tags are ignored, unclosed elements close at the end,
/// `<p>`/`<li>`/`<tr>`/`<td>` close their open sibling, comments/doctype/script dropped,
/// CDATA kept as text, a `<` that starts no tag stays literal text.
fn parse_html(src: &str) -> Vec<HNode> {
    let mut stack: Vec<OpenEl> = vec![OpenEl { name: String::new(), attrs: Vec::new(), kids: Vec::new() }];
    let mut text = String::new();
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        if !rest.starts_with('<') {
            let next = rest.find('<').unwrap_or(rest.len());
            text.push_str(&rest[..next]);
            i += next;
            continue;
        }
        if rest.starts_with("<!--") {
            flush_text(&mut text, &mut stack);
            i += rest.find("-->").map_or(rest.len(), |p| p + 3);
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            flush_text(&mut text, &mut stack);
            let end = rest.find("]]>").unwrap_or(rest.len());
            stack.last_mut().expect("root stays").kids.push(HNode::Text(rest[9..end].to_string()));
            i += (end + 3).min(rest.len());
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            i += rest.find('>').map_or(rest.len(), |p| p + 1);
            continue;
        }
        let is_end = rest.starts_with("</");
        let ns = if is_end { 2 } else { 1 };
        let starts_name = rest.as_bytes().get(ns).is_some_and(u8::is_ascii_alphabetic);
        if !starts_name {
            text.push('<');
            i += 1;
            continue;
        }
        let Some(gt) = find_tag_end(rest) else {
            text.push_str(rest); // no `>` anywhere after: the rest is plain text
            break;
        };
        let name_len = rest[ns..].bytes().take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'-' | b'_' | b'.')).count();
        let raw_name = &rest[ns..ns + name_len];
        let name = raw_name.rsplit(':').next().unwrap_or(raw_name).to_ascii_lowercase();
        flush_text(&mut text, &mut stack);
        if is_end {
            if let Some(pos) = stack.iter().rposition(|o| o.name == name)
                && pos > 0
            {
                while stack.len() > pos {
                    close_top(&mut stack);
                }
            }
            i += gt + 1;
            continue;
        }
        let inner = rest[ns + name_len..gt].trim_end();
        let self_close = inner.ends_with('/');
        let attrs = parse_attrs(inner.trim_end_matches('/'));
        let sibling = |n: &str| match name.as_str() {
            "td" | "th" => matches!(n, "td" | "th"),
            "dt" | "dd" => matches!(n, "dt" | "dd"),
            "p" | "li" | "tr" | "option" => n == name,
            _ => false,
        };
        if stack.len() > 1 && sibling(&stack.last().expect("len > 1").name) {
            close_top(&mut stack);
        }
        if name == "script" || name == "style" {
            let after = &rest[gt + 1..];
            let close = format!("</{name}");
            let skip = after.to_ascii_lowercase().find(&close).map_or(after.len(), |p| p + after[p..].find('>').map_or(after.len() - p, |q| q + 1));
            i += gt + 1 + skip;
            continue;
        }
        if self_close || VOID.contains(&name.as_str()) {
            stack.last_mut().expect("root stays").kids.push(HNode::El { name, attrs, kids: Vec::new() });
        } else {
            stack.push(OpenEl { name, attrs, kids: Vec::new() });
        }
        i += gt + 1;
    }
    flush_text(&mut text, &mut stack);
    while stack.len() > 1 {
        close_top(&mut stack);
    }
    stack.pop().map(|o| o.kids).unwrap_or_default()
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let after = &rest[p + 1..];
        match after.find(';').filter(|&e| e <= 10).and_then(|e| entity(&after[..e]).map(|c| (e, c))) {
            Some((e, c)) => {
                out.push_str(&c);
                rest = &after[e + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<String> {
    if let Some(num) = name.strip_prefix('#') {
        let v = match num.strip_prefix(['x', 'X']) {
            Some(h) => u32::from_str_radix(h, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(v).map(|c| if c == '\u{a0}' { ' ' } else { c }).map(String::from);
    }
    let c = match name {
        "amp" => "&", "lt" => "<", "gt" => ">", "quot" => "\"", "apos" => "'", "nbsp" | "ensp" | "emsp" | "thinsp" => " ",
        "ndash" => "–", "mdash" => "—", "hellip" => "…", "lsquo" => "‘", "rsquo" => "’", "ldquo" => "“", "rdquo" => "”",
        "times" => "×", "divide" => "÷", "deg" => "°", "plusmn" => "±", "minus" => "−", "le" => "≤", "ge" => "≥", "ne" => "≠",
        "asymp" => "≈", "infin" => "∞", "sqrt" => "√", "pi" => "π", "Pi" => "Π", "alpha" => "α", "beta" => "β", "gamma" => "γ",
        "Delta" => "Δ", "delta" => "δ", "epsilon" => "ε", "theta" => "θ", "lambda" => "λ", "mu" => "μ", "micro" => "µ", "rho" => "ρ",
        "sigma" => "σ", "Sigma" => "Σ", "tau" => "τ", "phi" => "φ", "omega" => "ω", "Omega" => "Ω", "sup1" => "¹", "sup2" => "²",
        "sup3" => "³", "frac12" => "½", "frac14" => "¼", "frac34" => "¾", "middot" => "·", "bull" => "•", "copy" => "©", "reg" => "®",
        "trade" => "™", "rarr" => "→", "larr" => "←", "harr" => "↔", "rArr" => "⇒", "hArr" => "⇔", "cent" => "¢", "pound" => "£",
        "euro" => "€", "yen" => "¥", "sect" => "§", "para" => "¶", "laquo" => "«", "raquo" => "»", "iexcl" => "¡", "iquest" => "¿",
        "szlig" => "ß", "aring" => "å", "Aring" => "Å", "oslash" => "ø", "Oslash" => "Ø", "aelig" => "æ", "AElig" => "Æ",
        "shy" | "zwj" | "zwnj" | "lrm" | "rlm" => "",
        _ => {
            const DIAC: [(&str, &str); 6] = [
                ("acute", "aáeéiíoóuúyýAÁEÉIÍOÓUÚYÝ"),
                ("grave", "aàeèiìoòuùAÀEÈIÌOÒUÙ"),
                ("circ", "aâeêiîoôuûAÂEÊIÎOÔUÛ"),
                ("uml", "aäeëiïoöuüyÿAÄEËIÏOÖUÜ"),
                ("tilde", "aãnñoõAÃNÑOÕ"),
                ("cedil", "cçCÇ"),
            ];
            let mut it = name.chars();
            let base = it.next()?;
            let suffix = it.as_str();
            let (_, table) = DIAC.iter().find(|(s, _)| *s == suffix)?;
            let t: Vec<char> = table.chars().collect();
            let pos = t.chunks(2).position(|p| p[0] == base)?;
            return Some(t[pos * 2 + 1].to_string());
        }
    };
    Some(c.to_string())
}

/// TeX delimiters `\( \)`, `\[ \]` and `$$ $$` → the stem markup's `$…$` math.
fn tex_to_dollars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let found = [("\\(", "\\)"), ("\\[", "\\]"), ("$$", "$$")]
            .iter()
            .filter_map(|(o, c)| rest.find(o).map(|p| (p, *o, *c)))
            .min_by_key(|x| x.0);
        let Some((p, o, c)) = found else { break };
        let after = &rest[p + o.len()..];
        let Some(e) = after.find(c) else { break };
        out.push_str(&rest[..p]);
        out.push('$');
        out.push_str(after[..e].trim());
        out.push('$');
        rest = &after[e + c.len()..];
    }
    out.push_str(rest);
    out
}

/// Whitespace inside a line collapsed, leading/trailing blank lines dropped, runs of
/// blank lines kept as one (a paragraph break).
fn tidy(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in s.split('\n') {
        let mut l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.contains(BLANK) {
            for p in ['.', ',', ';', ':', '!', '?', ')'] {
                l = l.replace(&format!("{BLANK} {p}"), &format!("{BLANK}{p}"));
            }
        }
        if l.is_empty() && out.last().is_none_or(String::is_empty) {
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out.join("\n")
}

/// `text` linked to `href` as `[text](url)` (edge whitespace kept outside) when the address is
/// http, https or mailto and the result reads back as that link; otherwise the text alone
/// (relative, `javascript:` and other links drop to plain text, as before links existed).
fn link_markup(text: &str, href: &str) -> String {
    let core = text.trim();
    let url: String = href.trim().chars().map(|c| match c {
        ' ' => "%20".to_string(),
        '(' => "%28".to_string(),
        ')' => "%29".to_string(),
        c => c.to_string(),
    }).collect();
    if core.is_empty() || core.contains('\n') || !crate::richtext::link_url_ok(&url) {
        return text.to_string();
    }
    let lead = &text[..text.len() - text.trim_start().len()];
    let trail = &text[text.trim_end().len()..];
    let md = format!("[{core}]({url})");
    let ch: Vec<char> = md.chars().collect();
    match crate::richtext::link_at(&ch, 0) {
        Some((_, end)) if end + 1 == ch.len() => format!("{lead}{md}{trail}"),
        _ => text.to_string(),
    }
}

fn looks_html(t: &str) -> bool {
    let l = t.to_ascii_lowercase();
    l.contains('<') && l.contains('>') && (l.contains("</") || l.contains("<br") || l.contains("<img") || l.contains("<p"))
}

/// Renders an HTML tree as stem markup (`docs/stem-format.md`) and collects image `src`s.
#[derive(Default)]
struct HtmlOut {
    images: Vec<String>,
}

impl HtmlOut {
    /// Nodes → markup: paragraph-like blocks separated by a blank line.
    fn render(&mut self, nodes: &[HNode]) -> String {
        let mut chunks = Vec::new();
        self.blocks(nodes, &mut chunks);
        tex_to_dollars(&chunks.join("\n\n"))
    }

    fn blocks(&mut self, nodes: &[HNode], chunks: &mut Vec<String>) {
        let mut line = String::new();
        for n in nodes {
            match n {
                HNode::El { name, .. } if is_html_block(name) => {
                    push_chunk(chunks, tidy(&std::mem::take(&mut line)));
                    self.block(n, chunks);
                }
                _ => self.inline(n, &mut line),
            }
        }
        push_chunk(chunks, tidy(&line));
    }

    fn block(&mut self, node: &HNode, chunks: &mut Vec<String>) {
        // `<center>`, `align="center"` or `text-align: center`: the block's markup, centred
        if html_centered(node) {
            let mut sub = Vec::new();
            self.block_inner(node, &mut sub);
            push_chunk(chunks, center_markup(&sub.join("\n\n")));
            return;
        }
        self.block_inner(node, chunks);
    }

    fn block_inner(&mut self, node: &HNode, chunks: &mut Vec<String>) {
        let HNode::El { name, kids, .. } = node else { return };
        match name.as_str() {
            "head" | "title" => {}
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let mut s = String::new();
                for k in kids {
                    self.inline(k, &mut s);
                }
                let s = tidy(&s);
                if !s.is_empty() {
                    let level = name[1..].parse::<u64>().unwrap_or(1);
                    chunks.push(heading_markup(&s, level));
                }
            }
            "ul" | "ol" => {
                let mut lines = Vec::new();
                self.list(node, 0, &mut lines);
                push_chunk(chunks, lines.join("\n"));
            }
            "table" | "tr" | "thead" | "tbody" | "tfoot" => {
                let mut rows: Vec<(Vec<String>, bool)> = Vec::new();
                self.rows(node, name == "thead", &mut rows);
                let header = rows.first().is_some_and(|r| r.1);
                push_chunk(chunks, pipe_table(rows.into_iter().map(|r| r.0).collect(), header));
            }
            "li" => push_chunk(chunks, format!("• {}", self.one_line(kids, false))),
            "blockquote" => {
                let mut inner = Vec::new();
                self.blocks(kids, &mut inner);
                let q: Vec<String> = inner.join("\n").lines().filter(|l| !l.trim().is_empty()).map(|l| format!("> {l}")).collect();
                push_chunk(chunks, q.join("\n"));
            }
            "pre" => {
                let raw: String = raw_text(kids);
                push_chunk(chunks, raw.trim_matches('\n').trim_end().to_string());
            }
            "hr" => chunks.push("---".to_string()),
            _ => self.blocks(kids, chunks),
        }
    }

    fn inline(&mut self, n: &HNode, out: &mut String) {
        match n {
            HNode::Text(t) => {
                for c in t.chars() {
                    if c.is_whitespace() {
                        if !out.ends_with(' ') && !out.ends_with('\n') {
                            out.push(' ');
                        }
                    } else {
                        out.push(c);
                    }
                }
            }
            HNode::El { name, kids, .. } => match name.as_str() {
                "br" => out.push('\n'),
                "b" | "strong" => self.wrap(kids, out, "**"),
                "s" | "del" | "strike" => self.wrap(kids, out, "~~"),
                "code" | "kbd" | "samp" | "tt" => self.wrap(kids, out, "`"),
                "span" | "font" if n.attr("style").is_some_and(|st| st.to_ascii_lowercase().replace(' ', "").contains("text-decoration:line-through") || st.to_ascii_lowercase().contains("line-through")) => {
                    self.wrap(kids, out, "~~")
                }
                "i" | "em" | "cite" | "var" => self.wrap(kids, out, "*"),
                "a" => {
                    let mut s = String::new();
                    for k in kids {
                        self.inline(k, &mut s);
                    }
                    out.push_str(&link_markup(&s, n.attr("href").unwrap_or("")));
                }
                "sub" | "sup" => {
                    let mut s = String::new();
                    for k in kids {
                        self.inline(k, &mut s);
                    }
                    let core = s.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !core.is_empty() {
                        let sup = name == "sup";
                        out.push_str(&crate::richtext::unicode_script(&core, sup).unwrap_or_else(|| if sup { format!("^{{{core}}}") } else { format!("_{{{core}}}") }));
                    }
                }
                "img" => {
                    if let Some(tex) = n.attr("data-equation-content").filter(|t| !t.trim().is_empty()) {
                        out.push_str(&format!("${}$", tex.trim()));
                        return;
                    }
                    if let Some(src) = n.attr("src") {
                        self.images.push(src.to_string());
                    }
                    out.push_str(&picture_text(n.attr("alt").unwrap_or("").trim()));
                }
                "math" => out.push_str(&math_text(n)),
                "input" if n.attr("type").is_some_and(|t| t.eq_ignore_ascii_case("checkbox")) => {
                    out.push_str(task_box(n.attr("checked").is_some()));
                }
                "script" | "style" | "head" => {}
                _ if is_html_block(name) => {
                    let mut sub = Vec::new();
                    self.block(n, &mut sub);
                    let s = single_line(&sub.join("\n"));
                    if !s.is_empty() {
                        if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('\n') {
                            out.push(' ');
                        }
                        out.push_str(&s);
                        out.push(' ');
                    }
                }
                _ => {
                    for k in kids {
                        self.inline(k, out);
                    }
                }
            },
        }
    }

    /// Bold/italic markers around the trimmed core, edge whitespace kept outside.
    fn wrap(&mut self, kids: &[HNode], out: &mut String, mark: &str) {
        let mut s = String::new();
        for k in kids {
            self.inline(k, &mut s);
        }
        let core = s.trim();
        if core.is_empty() || core.contains('\n') {
            out.push_str(&s);
            return;
        }
        let lead = &s[..s.len() - s.trim_start().len()];
        let trail = &s[s.trim_end().len()..];
        // strikethrough markers only count when they hug the text (`~~ a~~` would stay literal)
        if mark == "~~" && !crate::richtext::inline(&format!("{mark}{core}{mark}")).iter().filter(|sp| !sp.text.is_empty()).all(|sp| sp.strike) {
            out.push_str(&s);
            return;
        }
        // code only when it reads back as exactly that code (no backtick, no blank inside)
        if mark == "`" && !code_ok(core) {
            out.push_str(&s);
            return;
        }
        out.push_str(&format!("{lead}{mark}{core}{mark}{trail}"));
    }

    /// Content as ONE line (list items, table cells, choices).
    fn one_line(&mut self, kids: &[HNode], skip_lists: bool) -> String {
        let filtered: Vec<HNode> = kids
            .iter()
            .filter(|k| !(skip_lists && matches!(k, HNode::El { name, .. } if name == "ul" || name == "ol")))
            .cloned()
            .collect();
        let mut chunks = Vec::new();
        self.blocks(&filtered, &mut chunks);
        tex_to_dollars(&single_line(&chunks.join("\n")))
    }

    fn list(&mut self, node: &HNode, depth: usize, lines: &mut Vec<String>) {
        let HNode::El { name, kids, .. } = node else { return };
        let ordered = name == "ol";
        let start = node.attr("start").and_then(|s| s.trim().parse::<usize>().ok()).unwrap_or(1);
        let indent = "  ".repeat(depth);
        let mut i = 0;
        for item in kids {
            let HNode::El { name: iname, kids: ikids, .. } = item else { continue };
            if iname != "li" {
                if iname == "ul" || iname == "ol" {
                    self.list(item, depth + 1, lines);
                }
                continue;
            }
            let mut marker = if ordered { format!("{}. ", start + i) } else { "• ".to_string() };
            // TipTap-style task items: `<li data-checked="true">` (a checkbox input inside the
            // item prints its own box).
            if let Some(c) = item.attr("data-checked") {
                marker.push_str(task_box(c == "true" || c.is_empty()));
            }
            i += 1;
            lines.push(format!("{indent}{marker}{}", self.one_line(ikids, true)).trim_end().to_string());
            for c in ikids {
                if matches!(c, HNode::El { name, .. } if name == "ul" || name == "ol") {
                    self.list(c, depth + 1, lines);
                }
            }
        }
    }

    fn rows(&mut self, node: &HNode, in_head: bool, rows: &mut Vec<(Vec<String>, bool)>) {
        let HNode::El { name, kids, .. } = node else { return };
        match name.as_str() {
            "tr" => {
                let mut cells = Vec::new();
                let mut all_th = true;
                for c in kids {
                    if let HNode::El { name: cn, kids: ck, .. } = c
                        && (cn == "td" || cn == "th")
                    {
                        all_th &= cn == "th";
                        cells.push(self.one_line(ck, false).replace('|', "/"));
                        let span = c.attr("colspan").and_then(|s| s.trim().parse::<usize>().ok()).unwrap_or(1).clamp(1, 20);
                        for _ in 1..span {
                            cells.push(String::new());
                        }
                    }
                }
                if !cells.is_empty() {
                    rows.push((cells, in_head || all_th));
                }
            }
            "table" | "thead" | "tbody" | "tfoot" => {
                for k in kids {
                    self.rows(k, in_head || name == "thead", rows);
                }
            }
            _ => {}
        }
    }
}

/// Text of nodes with whitespace kept (`<pre>`); `<br>` is a line break.
fn raw_text(nodes: &[HNode]) -> String {
    let mut s = String::new();
    for n in nodes {
        match n {
            HNode::Text(t) => s.push_str(t),
            HNode::El { name, .. } if name == "br" => s.push('\n'),
            HNode::El { kids, .. } => s.push_str(&raw_text(kids)),
        }
    }
    s
}

/// MathML: its TeX annotation as `$…$` when present, else its text.
fn math_text(n: &HNode) -> String {
    fn find_tex(n: &HNode) -> Option<String> {
        let HNode::El { name, kids, .. } = n else { return None };
        if name == "annotation" && n.attr("encoding").is_some_and(|e| e.to_ascii_lowercase().contains("tex")) {
            return Some(raw_text(kids).trim().to_string());
        }
        kids.iter().find_map(find_tex)
    }
    fn plain(n: &HNode, out: &mut String) {
        match n {
            HNode::Text(t) => out.push_str(t.trim()),
            HNode::El { name, kids, .. } if name != "annotation" && name != "annotation-xml" => kids.iter().for_each(|k| plain(k, out)),
            HNode::El { .. } => {}
        }
    }
    if let Some(t) = find_tex(n).filter(|t| !t.is_empty()) {
        return format!("${t}$");
    }
    let mut s = String::new();
    plain(n, &mut s);
    s
}

/// Rows → pipe-table markup (padded rows, outer pipes when an edge cell is empty, a
/// `---|---` line under a header row); a one-column table is just its lines.
fn pipe_table(mut rows: Vec<Vec<String>>, first_is_header: bool) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols < 2 {
        return rows.iter().map(|r| r.join(" ").trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
    }
    for r in &mut rows {
        r.resize(cols, String::new());
    }
    let outer = rows.iter().any(|r| r[0].is_empty() || r[cols - 1].is_empty());
    let mut lines = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        lines.push(if outer { format!("| {} |", r.join(" | ")) } else { r.join(" | ") });
        if i == 0 && first_is_header && rows.len() > 1 {
            lines.push(vec!["---"; cols].join("|"));
        }
    }
    lines.join("\n")
}

/// HTML → (stem markup, image srcs).
fn html_to_markup(src: &str) -> (String, Vec<String>) {
    let mut h = HtmlOut::default();
    let s = h.render(&parse_html(src));
    (s, h.images)
}

/// HTML → one line of markup (choices, matching items).
fn html_inline(src: &str) -> String {
    HtmlOut::default().one_line(&parse_html(src), false)
}

// ---------- images from archives / inline data ----------

/// Bytes for an image `src`: a `data:` URI, or a file of the archive (`media`, keys are
/// lower-case entry paths) resolved against `base`, with the Canvas/Common Cartridge
/// `$IMS-CC-FILEBASE$` and Moodle `@@PLUGINFILE@@` prefixes understood; a unique file
/// name anywhere in the archive is the fallback.
fn resolve_media(src: &str, media: &HashMap<String, Vec<u8>>, base: &str) -> Option<Vec<u8>> {
    if let Some(rest) = src.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',')?;
        return if meta.ends_with(";base64") { base64_decode(data) } else { None };
    }
    if media.is_empty() {
        return None;
    }
    let mut p = percent_decode(src.split(['?', '#']).next().unwrap_or(src));
    for pre in ["$IMS-CC-FILEBASE$/", "$IMS_CC_FILEBASE$/", "@@PLUGINFILE@@/"] {
        if let Some(r) = p.strip_prefix(pre) {
            p = r.to_string();
        }
    }
    if p.contains("://") {
        return None;
    }
    let full = match p.strip_prefix('/') {
        Some(abs) => abs.to_string(),
        None => join_path(base, &p),
    };
    if let Some(b) = media.get(&full.to_ascii_lowercase()) {
        return Some(b.clone());
    }
    let file = full.rsplit('/').next()?.to_ascii_lowercase();
    let mut hits = media.iter().filter(|(k, _)| k.rsplit('/').next() == Some(file.as_str()));
    let first = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    Some(first.1.clone())
}

/// The first resolvable image becomes the question's picture (stored like a package's).
fn attach_src(q: &mut Question, srcs: &[String], media: &HashMap<String, Vec<u8>>, base: &str) {
    for s in srcs {
        if let Some(bytes) = resolve_media(s, media, base)
            && let Ok(img) = store_image(&bytes)
        {
            q.image = Some(img);
            return;
        }
    }
}

/// `store::put_image`, except that unit tests never write into the user's real store
/// (the `tests/import_formats.rs` integration test checks the real write against a
/// temporary store directory through the headless `VERSHION_PARSE` hook).
fn store_image(bytes: &[u8]) -> Result<crate::model::ImageRef, String> {
    #[cfg(test)]
    {
        Ok(crate::model::ImageRef { file: format!("media/unit-test-{}.png", bytes.len()), width: 1, height: 1 })
    }
    #[cfg(not(test))]
    {
        crate::store::put_image(bytes)
    }
}

// ---------- Moodle GIFT ----------

/// Byte index of the first unescaped char matching `pred` at or after `from`.
fn find_unescaped(s: &str, from: usize, pred: impl Fn(char) -> bool) -> Option<usize> {
    let mut esc = false;
    for (i, c) in s[from..].char_indices() {
        if esc {
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if pred(c) {
            return Some(from + i);
        }
    }
    None
}

/// Byte index of the first unescaped occurrence of `pat`.
fn find_unescaped_str(s: &str, pat: &str) -> Option<usize> {
    let mut esc = false;
    for (i, c) in s.char_indices() {
        if esc {
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if s[i..].starts_with(pat) {
            return Some(i);
        }
    }
    None
}

/// TeX commands starting with `n` that GIFT's `\n` (line break) must not swallow.
const TEX_N: [&str; 24] = [
    "neq", "ne", "nu", "nabla", "not", "notin", "neg", "newline", "ni", "nmid", "nleq", "ngeq", "nless", "ngtr", "nexists", "natural", "nearrow", "nwarrow",
    "napprox", "ncong", "nsim", "nparallel", "nrightarrow", "nleftarrow",
];

/// GIFT escapes `\~ \= \# \{ \} \: \\` → the literal char; `\n` → a line break (unless it
/// starts a TeX command such as `\neq`, kept as written); other backslashes stay.
fn gift_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.peek().copied() {
            Some(n) if "~=#{}:\\".contains(n) => {
                out.push(n);
                it.next();
            }
            Some('n') => {
                let word: String = it.clone().take_while(|c| c.is_ascii_alphabetic()).collect();
                if TEX_N.contains(&word.as_str()) {
                    out.push('\\'); // a TeX command such as \neq: kept as written
                } else {
                    out.push('\n');
                    it.next();
                }
            }
            _ => out.push('\\'),
        }
    }
    out
}

/// Unescaped `{` minus `}` on one line.
fn brace_delta(line: &str) -> i32 {
    let mut d = 0;
    let mut esc = false;
    for c in line.chars() {
        if esc {
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == '{' {
            d += 1;
        } else if c == '}' {
            d -= 1;
        }
    }
    d
}

/// GIFT source → question chunks: `//` comment lines dropped, blank lines separate
/// questions (except inside an open `{…}` block), each `$CATEGORY:` line its own chunk.
fn gift_chunks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        if t.is_empty() {
            if depth <= 0 {
                if !cur.trim().is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                depth = 0;
            } else {
                cur.push('\n');
            }
            continue;
        }
        if depth <= 0 && t.starts_with("$CATEGORY:") {
            if !cur.trim().is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(t.to_string());
            continue;
        }
        depth += brace_delta(line);
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(line.trim_end());
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Does this text read as GIFT? A `$CATEGORY:` line, or an answer block `{…}` holding
/// `=`/`~`/`#` answers or T/F (an empty `{}` counts after a `::title::`). Braces right
/// after `_`/`^` (the stem markup's sub/superscripts) never count, and text with
/// Blackboard type-tag blocks is never GIFT.
fn is_gift(text: &str) -> bool {
    let body = text.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    if split_blocks(&body).iter().any(|b| tag_of(&b[0]).is_some()) {
        return false;
    }
    if body.lines().any(|l| l.trim_start().starts_with("$CATEGORY:")) {
        return true;
    }
    let titled = body.trim_start().starts_with("::");
    let mut from = 0;
    while let Some(open) = find_unescaped(&body, from, |c| c == '{') {
        let Some(close) = find_unescaped(&body, open + 1, |c| c == '}') else { break };
        from = close + 1;
        if matches!(body[..open].chars().last(), Some('_' | '^')) {
            continue;
        }
        let c = body[open + 1..close].trim();
        let head = c.split('#').next().unwrap_or("").trim().to_ascii_uppercase();
        if c.starts_with(['=', '~', '#']) || matches!(head.as_str(), "T" | "F" | "TRUE" | "FALSE") || (c.is_empty() && titled) {
            return true;
        }
    }
    false
}

fn parse_gift(text: &str, name: &str) -> Result<ImportDoc, String> {
    let mut questions = Vec::new();
    let mut topic: Option<String> = None;
    for chunk in gift_chunks(text) {
        let c = chunk.trim();
        if let Some(cat) = c.strip_prefix("$CATEGORY:") {
            topic = category_topic(cat.trim());
            continue;
        }
        match gift_question(c) {
            Ok(Some(mut q)) => {
                if let Some(t) = &topic {
                    q.topics = vec![t.clone()];
                }
                questions.push(q);
            }
            Ok(None) => {} // a description (no answer block) or a bare title
            Err(e) => return Err(format!("GIFT question {}: {e}", questions.len() + 1)),
        }
    }
    if questions.is_empty() {
        return Err("No GIFT questions found: each question needs an answer block in { } (like {=right ~wrong}, {T} or {}).".into());
    }
    Ok(ImportDoc { name: name.to_string(), questions, exams: Vec::new() })
}

/// A category path's meaningful last segment (`$course$/top/Unit 3` → `Unit 3`).
fn category_topic(path: &str) -> Option<String> {
    path.split('/')
        .map(str::trim)
        .rfind(|s| !s.is_empty() && !s.starts_with('$') && !s.eq_ignore_ascii_case("top") && !s.to_ascii_lowercase().starts_with("default for"))
        .map(str::to_string)
}

fn gift_question(chunk: &str) -> Result<Option<Question>, String> {
    let mut rest = chunk.trim();
    if let Some(after) = rest.strip_prefix("::") {
        match find_unescaped_str(after, "::") {
            Some(e) => rest = after[e + 2..].trim_start(),
            None => return Err("the ::title:: has no closing ::".into()),
        }
    }
    let mut html = false;
    for tag in ["[html]", "[moodle]", "[plain]", "[markdown]"] {
        if rest.len() >= tag.len() && rest.is_char_boundary(tag.len()) && rest[..tag.len()].eq_ignore_ascii_case(tag) {
            html = tag == "[html]";
            rest = rest[tag.len()..].trim_start();
            break;
        }
    }
    let Some(open) = find_unescaped(rest, 0, |c| c == '{') else { return Ok(None) };
    let close = find_unescaped(rest, open + 1, |c| c == '}').ok_or("the answer block { has no closing }")?;
    let text = |s: &str| {
        let u = gift_unescape(s.trim());
        if html { html_to_markup(&u).0 } else { tex_to_dollars(&u) }
    };
    let item = |s: &str| {
        let u = gift_unescape(s.trim());
        if html { html_inline(&u) } else { tex_to_dollars(&u) }
    };
    let (kind, inline_blank, general) = gift_answers(&rest[open + 1..close], &item)?;
    let (before, after) = (text(&rest[..open]), text(&rest[close + 1..]));
    let stem = if after.is_empty() {
        before
    } else if inline_blank {
        let gap = if after.starts_with(['.', ',', ';', ':', '!', '?', ')']) { "" } else { " " };
        format!("{} {BLANK}{gap}{after}", before.trim_end()).trim().to_string()
    } else {
        format!("{before} {after}").trim().to_string()
    };
    let stem = match (&kind, stem.is_empty()) {
        (Kind::Matching { .. }, true) => MAT_DEFAULT_STEM.to_string(),
        (_, true) => return Err("no question text before the answer block".into()),
        _ => stem,
    };
    let mut q = make_question(stem, kind);
    q.explanation = general.filter(|g| !g.is_empty());
    Ok(Some(q))
}

/// One GIFT answer: `=` or `~`, an optional `%weight%`, the text (feedback cut off).
struct GiftAns {
    mark: char,
    weight: Option<f64>,
    text: String,
}

impl GiftAns {
    fn positive(&self) -> bool {
        match self.mark {
            '=' => self.weight.is_none_or(|w| w > 0.0),
            _ => self.weight.is_some_and(|w| w > 0.0),
        }
    }
}

/// Splits answer-block text at unescaped `=`/`~`; `#feedback` after an answer is dropped.
fn gift_split(c: &str) -> Result<Vec<GiftAns>, String> {
    let mut out: Vec<GiftAns> = Vec::new();
    let mut starts: Vec<(usize, char)> = Vec::new();
    let mut esc = false;
    for (i, ch) in c.char_indices() {
        if esc {
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '=' || ch == '~' {
            starts.push((i, ch));
        }
    }
    if let Some(&(first, _)) = starts.first()
        && !c[..first].trim().is_empty()
    {
        return Err(format!("unexpected text {:?} before the first = or ~ answer", c[..first].trim()));
    }
    for (k, &(i, mark)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map_or(c.len(), |s| s.0);
        let mut raw = &c[i + 1..end];
        if let Some(h) = find_unescaped(raw, 0, |ch| ch == '#') {
            raw = &raw[..h];
        }
        let mut raw = raw.trim();
        let mut weight = None;
        if let Some(w) = raw.strip_prefix('%')
            && let Some(p) = w.find('%')
        {
            weight = Some(w[..p].trim().parse::<f64>().map_err(|_| format!("bad answer weight %{}%", &w[..p]))?);
            raw = w[p + 1..].trim();
        }
        out.push(GiftAns { mark, weight, text: raw.to_string() });
    }
    Ok(out)
}

/// A GIFT numeric answer (`3.14:0.01`, `1..5`, or a plain value) → accept variants.
fn gift_num(raw: &str) -> Result<Vec<String>, String> {
    let t = gift_unescape(raw.trim());
    let t = t.trim();
    if let Some((lo, hi)) = t.split_once("..")
        && let (Some(a), Some(b)) = (parse_f64(lo), parse_f64(hi))
    {
        return Ok(num_range(a, b));
    }
    if let Some((v, tol)) = t.split_once(':')
        && let Some(variants) = num_with_tolerance(v, tol)
    {
        return Ok(variants);
    }
    parse_num_answer(t).ok_or_else(|| format!("{t:?} is not a number (write 3.14, 3.14:0.01 or 1..5)"))
}

/// The answer block → (kind, whether a mid-sentence block is a blank, general feedback).
fn gift_answers(content: &str, item: &dyn Fn(&str) -> String) -> Result<(Kind, bool, Option<String>), String> {
    let c = content.trim();
    let (c, general) = match find_unescaped_str(c, "####") {
        Some(p) => (c[..p].trim(), Some(gift_unescape(c[p + 4..].trim()))),
        None => (c, None),
    };
    if c.is_empty() {
        return Ok((Kind::Short { sample: String::new() }, false, general));
    }
    let head = match find_unescaped(c, 0, |ch| ch == '#') {
        Some(h) => &c[..h],
        None => c,
    };
    match head.trim().to_ascii_uppercase().as_str() {
        "T" | "TRUE" => return Ok((Kind::Tf { correct: true }, false, general)),
        "F" | "FALSE" => return Ok((Kind::Tf { correct: false }, false, general)),
        _ => {}
    }
    if let Some(body) = c.strip_prefix('#') {
        let body = body.trim();
        let mut accept = Vec::new();
        if body.starts_with('=') || body.starts_with('~') {
            for a in gift_split(body)?.into_iter().filter(GiftAns::positive) {
                accept.extend(gift_num(&a.text)?);
            }
        } else {
            let value = match find_unescaped(body, 0, |ch| ch == '#') {
                Some(h) => &body[..h],
                None => body,
            };
            accept = gift_num(value)?;
        }
        dedup(&mut accept);
        if accept.is_empty() {
            return Err("the numeric answer block has no correct answer".into());
        }
        return Ok((Kind::Blank { accept: vec![accept] }, true, general));
    }
    let answers = gift_split(c)?;
    if answers.is_empty() {
        return Err("the answer block has no = or ~ answers".into());
    }
    if answers.iter().all(|a| a.mark == '=') && answers.iter().any(|a| find_unescaped_str(&a.text, "->").is_some()) {
        let mut pairs = Vec::new();
        let mut distractors = Vec::new();
        for a in &answers {
            let p = find_unescaped_str(&a.text, "->").ok_or_else(|| format!("matching answer {:?} has no ->", a.text))?;
            let (left, right) = (item(&a.text[..p]), item(&a.text[p + 2..]));
            if right.is_empty() {
                return Err(format!("matching answer {:?} has nothing after ->", a.text));
            }
            if left.is_empty() { distractors.push(right) } else { pairs.push(Pair { left, right }) }
        }
        if pairs.is_empty() {
            return Err("a matching question needs at least one `=left -> right` pair".into());
        }
        return Ok((Kind::Matching { pairs, distractors }, false, general));
    }
    if answers.iter().any(|a| a.mark == '~') {
        let answers: Vec<Answer> = answers.iter().map(|a| Answer { text: item(&a.text), correct: a.positive(), locked: false }).collect();
        if !answers.iter().any(|a| a.correct) {
            return Err("no correct answer: mark it with = (or a positive weight like ~%50%)".into());
        }
        return Ok((Kind::Mc { answers }, true, general));
    }
    let mut accept: Vec<String> = answers.iter().filter(|a| a.positive()).map(|a| item(&a.text)).filter(|t| !t.is_empty()).collect();
    dedup(&mut accept);
    if accept.is_empty() {
        return Err("the short-answer block has no accepted answer".into());
    }
    Ok((Kind::Blank { accept: vec![accept] }, true, general))
}

// ---------- flashcard lists (Quizlet / Anki / two-column lists) ----------

/// A quote-aware split on one delimiter char (doubled `""` is a literal quote).
fn split_delim(line: &str, delim: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' if cur.trim().is_empty() => {
                cur.clear();
                in_quotes = true;
            }
            c if c == delim && !in_quotes => cells.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    cells.push(cur);
    cells
}

/// Term/definition pairs, or `None` when the text is not such a list. Accepted: every
/// line `term<TAB>definition` (Quizlet's default export, Anki's notes export; extra
/// columns such as Anki tags are ignored, `#key:value` Anki header lines honored), or
/// at least two lines that ALL split into exactly two non-empty cells on ` - `, `;` or
/// `,` (quote-aware). A first row of header words (term/definition, front/back…) is
/// skipped; Quizlet's one-line `term<TAB>def;term<TAB>def` export is split on `;`.
fn term_pairs(text: &str) -> Option<Vec<(String, String)>> {
    let mut declared: Option<char> = None;
    let mut html = false;
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let t = l.trim_matches(|c: char| c == ' ' || c == '\r');
        if t.trim().is_empty() {
            continue;
        }
        if lines.is_empty()
            && let Some(h) = t.strip_prefix('#')
            && let Some((k, v)) = h.split_once(':')
        {
            match k.trim().to_ascii_lowercase().as_str() {
                "separator" => {
                    declared = match v.trim().to_ascii_lowercase().as_str() {
                        "tab" => Some('\t'),
                        "comma" => Some(','),
                        "semicolon" => Some(';'),
                        "pipe" => Some('|'),
                        "colon" => Some(':'),
                        "space" => Some(' '),
                        other => other.chars().next(),
                    }
                }
                "html" => html = v.trim().eq_ignore_ascii_case("true"),
                _ => {}
            }
            continue;
        }
        lines.push(t.to_string());
    }
    if lines.len() == 1 && lines[0].contains('\t') && lines[0].contains(';') {
        let parts: Vec<String> = lines[0].split(';').map(str::to_string).filter(|p| !p.trim().is_empty()).collect();
        if parts.iter().all(|p| p.contains('\t')) {
            lines = parts;
        }
    }
    if lines.is_empty() {
        return None;
    }
    // Rows led by a Blackboard type tag (`MC<TAB>stem…`, `TF<TAB>…`) are questions, not
    // flashcards: a list where most rows are tagged is never a term list.
    let tagged = lines
        .iter()
        .filter(|l| l.split_once('\t').is_some_and(|(a, _)| BB_UPLOAD_TAGS.contains(&a.trim().to_ascii_lowercase().as_str())))
        .count();
    if tagged * 2 >= lines.len() {
        return None;
    }
    let cell = |s: &str| {
        let s = s.trim();
        if html || looks_html(s) { html_inline(s) } else { s.to_string() }
    };
    let split_all = |delim: char, exact: bool| -> Option<Vec<(String, String)>> {
        lines
            .iter()
            .map(|l| {
                let cells = split_delim(l, delim);
                if (exact && cells.len() != 2) || cells.len() < 2 {
                    return None;
                }
                let (a, b) = (cell(&cells[0]), cell(&cells[1]));
                (!a.is_empty() && !b.is_empty()).then_some((a, b))
            })
            .collect()
    };
    let mut pairs = if let Some(d) = declared {
        split_all(d, false)?
    } else if lines.iter().all(|l| l.contains('\t')) {
        split_all('\t', false)?
    } else if lines.len() < 2 {
        return None;
    } else if lines.iter().all(|l| l.contains(" - ")) {
        lines
            .iter()
            .map(|l| l.split_once(" - ").map(|(a, b)| (cell(a), cell(b))).filter(|(a, b)| !a.is_empty() && !b.is_empty()))
            .collect::<Option<Vec<_>>>()?
    } else if let Some(p) = split_all(';', true) {
        p
    } else {
        split_all(',', true)?
    };
    let is_header = |s: &str| TERM_HEADERS.contains(&s.trim().to_ascii_lowercase().as_str());
    if pairs.first().is_some_and(|(a, b)| is_header(a) && is_header(b)) {
        pairs.remove(0);
    }
    (!pairs.is_empty()).then_some(pairs)
}

/// Flashcard pairs → questions. A Q&A deck (most fronts end in `?` or run past 60
/// characters), or a list of fewer than 3 cards, becomes one Short question per card
/// (stem = front, sample answer = back). Otherwise it is a vocabulary list: Matching
/// questions of at most 10 term → definition pairs each, the cards split evenly.
fn term_questions(pairs: Vec<(String, String)>) -> Vec<Question> {
    let qa = pairs.iter().filter(|(l, _)| l.trim_end().ends_with('?') || l.chars().count() > 60).count() * 2 > pairs.len();
    if qa || pairs.len() < 3 {
        return pairs.into_iter().map(|(stem, sample)| make_question(stem, Kind::Short { sample })).collect();
    }
    let groups = pairs.len().div_ceil(TERM_GROUP);
    let size = pairs.len().div_ceil(groups);
    pairs
        .chunks(size)
        .map(|c| {
            let pairs = c.iter().map(|(l, r)| Pair { left: l.clone(), right: r.clone() }).collect();
            make_question(TERM_MATCH_STEM.to_string(), Kind::Matching { pairs, distractors: Vec::new() })
        })
        .collect()
}

fn try_term_list(text: &str, name: &str) -> Option<Result<ImportDoc, String>> {
    let pairs = term_pairs(text)?;
    Some(Ok(ImportDoc { name: name.to_string(), questions: term_questions(pairs), exams: Vec::new() }))
}

// ---------- Respondus / text2qti numbered text ----------

/// `1.  stem` / `12) stem` → the stem text (`1.5` is a number, not a label).
fn numbered_stem(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let rest = &t[digits..];
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let r = rest.trim();
    (!r.is_empty()).then_some(r)
}

const NUMBERED_META: [&str; 12] = [
    "type", "title", "points", "quiz title", "quiz description", "shuffle answers", "show correct answers", "one question at a time", "can't go back",
    "feedback is shown", "text title", "text",
];

/// A Respondus/text2qti setting line: `Type: MC`, `Points: 2`, `Quiz title: …`.
fn meta_line(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once(':')?;
    let key = k.trim().to_ascii_lowercase();
    NUMBERED_META.contains(&key.as_str()).then(|| (key, v.trim().to_string()))
}

/// `Answer: b` / `ANSWER: A, C` → choice indexes.
fn answer_key(line: &str) -> Option<Vec<usize>> {
    let t = line.trim();
    if t.len() < 7 || !t.is_char_boundary(7) || !t[..7].eq_ignore_ascii_case("answer:") {
        return None;
    }
    let keys = correct_letters(&t[7..]);
    (!keys.is_empty()).then_some(keys)
}

/// A strict lettered choice: optional `*`, ONE letter, `.` or `)`, then a space.
fn letter_choice(t: &str) -> Option<(bool, String)> {
    let (correct, w) = match t.strip_prefix('*') {
        Some(r) => (true, r.trim_start()),
        None => (false, t),
    };
    let b = w.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && (b[1] == b'.' || b[1] == b')') && b[2].is_ascii_whitespace() {
        return Some((correct, w[2..].trim().to_string()));
    }
    None
}

/// text2qti numeric answer line `= 2.5`, `= 2.5 +- 0.1` or `= [2, 3]` → accept variants.
fn numeric_line(t: &str) -> Option<Vec<String>> {
    let r = t.strip_prefix('=')?.trim();
    if let Some(inner) = r.strip_prefix('[').and_then(|x| x.strip_suffix(']'))
        && let Some((a, b)) = inner.split_once(',')
        && let (Some(lo), Some(hi)) = (parse_f64(a), parse_f64(b))
    {
        return Some(num_range(lo, hi));
    }
    parse_num_answer(r)
}

fn is_rule(t: &str, c: char) -> bool {
    t.chars().count() >= 3 && t.chars().all(|x| x == c)
}

/// Respondus "standard" text and text2qti markdown, claimed when numbered question
/// blocks appear with a format marker (a `Type:`/`Points:`/`Title:`/`Quiz title:`/
/// `Answer:` line, `[*]` boxes, a `= number` answer, a `____` or `^^^^` line), or when
/// every multiple-choice block is numbered; Blackboard-tagged text is never claimed.
fn try_numbered(text: &str, name: &str) -> Option<Result<ImportDoc, String>> {
    let blocks = split_blocks(text);
    if blocks.iter().any(|b| tag_of(&b[0]).is_some()) {
        return None;
    }
    let first_body = |b: &[String]| b.iter().find(|l| meta_line(l).is_none()).cloned();
    let numbered = |b: &[String]| first_body(b).is_some_and(|l| numbered_stem(&l).is_some());
    if !blocks.iter().any(|b| numbered(b)) {
        return None;
    }
    let marker = text.lines().any(|l| {
        let t = l.trim();
        let low = t.to_ascii_lowercase();
        ["type:", "points:", "title:", "quiz title:", "answer:"].iter().any(|p| low.starts_with(p))
            || t.starts_with("[*]")
            || t.starts_with("[ ]")
            || numeric_line(t).is_some()
            || is_rule(t, '_')
            || is_rule(t, '^')
    });
    let choice_blocks: Vec<&Vec<String>> = blocks.iter().filter(|b| b.iter().filter(|l| is_choice(l)).count() >= 2).collect();
    let all_numbered = !choice_blocks.is_empty() && choice_blocks.iter().all(|b| numbered(b));
    if !(marker || all_numbered) {
        return None;
    }
    Some(parse_numbered(&blocks, name))
}

#[derive(Default, Clone)]
struct NumMeta {
    ty: Option<String>,
    points: Option<f64>,
}

fn parse_numbered(blocks: &[Vec<String>], name: &str) -> Result<ImportDoc, String> {
    let mut doc_name = name.to_string();
    let mut pending = NumMeta::default();
    let mut questions = Vec::new();
    let mut n = 0usize;
    for block in blocks {
        let mut meta = std::mem::take(&mut pending);
        let mut i = 0;
        while i < block.len() && numbered_stem(&block[i]).is_none() {
            let Some((k, v)) = meta_line(&block[i]) else { break };
            match k.as_str() {
                "quiz title" if !v.is_empty() => doc_name = v,
                "type" => meta.ty = Some(v.to_ascii_lowercase()),
                "points" => meta.points = v.parse::<f64>().ok().filter(|p| p.is_finite() && *p > 0.0),
                _ => {}
            }
            i += 1;
        }
        if i >= block.len() {
            pending = meta; // a settings-only block applies to the next question
            continue;
        }
        let Some(first) = numbered_stem(&block[i]) else { continue }; // prose
        n += 1;
        let mut q = numbered_question(first, &block[i + 1..], &meta).map_err(|e| format!("question {n}: {e}"))?;
        q.points = meta.points;
        questions.push(q);
    }
    if questions.is_empty() {
        return Err(GENERIC.to_string());
    }
    Ok(ImportDoc { name: doc_name, questions, exams: Vec::new() })
}

fn numbered_question(first: &str, rest: &[String], meta: &NumMeta) -> Result<Question, String> {
    let mut stem = vec![first.to_string()];
    let mut choices: Vec<(bool, String)> = Vec::new();
    let mut bracket = false;
    let mut stars: Vec<String> = Vec::new();
    let mut nums: Vec<String> = Vec::new();
    let mut essay = false;
    let mut key: Option<Vec<usize>> = None;
    let mut explanation: Option<String> = None;
    for line in rest {
        let t = line.trim();
        let started = !choices.is_empty() || !stars.is_empty() || !nums.is_empty() || essay;
        if let Some(r) = t.strip_prefix("[*]") {
            bracket = true;
            choices.push((true, r.trim().to_string()));
        } else if let Some(r) = t.strip_prefix("[ ]").or_else(|| t.strip_prefix("[]")) {
            bracket = true;
            choices.push((false, r.trim().to_string()));
        } else if let Some(k) = answer_key(t) {
            key = Some(k);
        } else if is_rule(t, '_') || is_rule(t, '^') {
            essay = true;
        } else if let Some(r) = t.strip_prefix("...") {
            explanation = Some(r.trim().to_string()).filter(|s| !s.is_empty());
        } else if let Some(v) = numeric_line(t) {
            nums.extend(v);
        } else if let Some(c) = letter_choice(t) {
            choices.push(c);
        } else if let Some(r) = t.strip_prefix('*').filter(|r| r.starts_with(char::is_whitespace)) {
            stars.push(r.trim().to_string());
        } else if started && (t.starts_with('+') || t.starts_with('-') || t.starts_with('@') || t.starts_with('~')) {
            // per-answer feedback: not stored
        } else if !started {
            stem.push(t.to_string());
        } else if let Some(last) = choices.last_mut() {
            last.1.push(' ');
            last.1.push_str(t);
        }
    }
    let ty = meta.ty.clone().unwrap_or_default();
    let texts = |c: &[(bool, String)]| c.iter().map(|x| x.1.clone()).collect::<Vec<_>>();
    let kind = match ty.as_str() {
        "e" | "essay" => Kind::Short { sample: choices.first().map(|c| c.1.clone()).unwrap_or_default() },
        "f" | "fib" | "fill" => {
            let mut accept = if choices.is_empty() { stars.clone() } else { texts(&choices) };
            dedup(&mut accept);
            if accept.is_empty() {
                return Err("a Type: F question needs its accepted answers as lettered lines".into());
            }
            Kind::Blank { accept: vec![accept] }
        }
        "mt" | "matching" => {
            let mut pairs = Vec::new();
            let mut distractors = Vec::new();
            for (_, c) in &choices {
                let (l, r) = c.split_once('=').ok_or_else(|| format!("matching line {c:?} needs `left = right`"))?;
                let (l, r) = (l.trim().to_string(), r.trim().to_string());
                if l.is_empty() { distractors.push(r) } else { pairs.push(Pair { left: l, right: r }) }
            }
            if pairs.is_empty() {
                return Err("a Type: MT question needs `a) left = right` lines".into());
            }
            Kind::Matching { pairs, distractors }
        }
        "o" | "ord" | "ordering" => {
            if choices.is_empty() {
                return Err("a Type: O question needs its items, in order, as lettered lines".into());
            }
            Kind::Blank { accept: vec![vec![texts(&choices).join(", ")]] }
        }
        "" | "mc" | "ma" | "mr" | "tf" => {
            if essay && choices.is_empty() {
                Kind::Short { sample: String::new() }
            } else if !nums.is_empty() {
                dedup(&mut nums);
                Kind::Blank { accept: vec![nums] }
            } else if !stars.is_empty() && choices.is_empty() {
                dedup(&mut stars);
                Kind::Blank { accept: vec![stars] }
            } else if choices.is_empty() {
                return Err("no answers: add lettered choices (a) …, *b) …), [*]/[ ] boxes, `* answer`, `= number` or a ____ line".into());
            } else {
                let mut answers: Vec<Answer> = choices.iter().map(|(c, t)| Answer { text: t.clone(), correct: *c, locked: false }).collect();
                if let Some(k) = key {
                    for i in k {
                        answers.get_mut(i).ok_or_else(|| format!("the Answer: line names choice {} but only {} are given", i + 1, choices.len()))?.correct = true;
                    }
                }
                if !answers.iter().any(|a| a.correct) {
                    return Err("no correct choice: mark it with a leading * (or [*], or an Answer: line)".into());
                }
                let multi = matches!(ty.as_str(), "ma" | "mr") || bracket;
                match tf_from(&answers, multi) {
                    Some(tf) => tf,
                    None if ty == "tf" => return Err("a Type: TF question needs the two choices True and False, one marked *".into()),
                    None => Kind::Mc { answers },
                }
            }
        }
        other => return Err(format!("Respondus Type: {} is not supported (use MC, MR, TF, E, F, MT or O)", other.to_ascii_uppercase())),
    };
    let mut q = make_question(stem.join("\n"), kind);
    q.explanation = explanation;
    Ok(q)
}

// ---------- Brightspace (D2L) question CSV ----------

/// Brightspace's row-per-field CSV: claimed when a record starts with `NewQuestion`.
fn try_brightspace(text: &str, name: &str) -> Option<Result<ImportDoc, String>> {
    let rows: Vec<Vec<String>> = csv_records(text).iter().filter(|r| !r.trim().is_empty()).map(|r| split_csv_line(r).into_iter().map(|c| c.trim().to_string()).collect()).collect();
    if !rows.iter().any(|r| r.first().is_some_and(|c| c.eq_ignore_ascii_case("newquestion"))) {
        return None;
    }
    let mut questions = Vec::new();
    let mut groups: Vec<&[Vec<String>]> = Vec::new();
    let starts: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| r[0].eq_ignore_ascii_case("newquestion")).map(|(i, _)| i).collect();
    for (k, &s) in starts.iter().enumerate() {
        groups.push(&rows[s..starts.get(k + 1).copied().unwrap_or(rows.len())]);
    }
    for (n, g) in groups.iter().enumerate() {
        match brightspace_question(g) {
            Ok(Some(q)) => questions.push(q),
            Ok(None) => {}
            Err(e) => return Some(Err(format!("Brightspace question {}: {e}", n + 1))),
        }
    }
    if questions.is_empty() {
        return Some(Err("The Brightspace CSV holds no supported questions (MC, MS, TF, SA, WR, M, O, FIB).".into()));
    }
    Some(Ok(ImportDoc { name: name.to_string(), questions, exams: Vec::new() }))
}

fn brightspace_question(rows: &[Vec<String>]) -> Result<Option<Question>, String> {
    let ty = rows[0].get(1).map(|t| t.to_ascii_uppercase()).unwrap_or_default();
    let cell = |r: &Vec<String>, i: usize| r.get(i).cloned().unwrap_or_default();
    let weight = |s: &str| s.trim().parse::<f64>().ok();
    let mut stem = String::new();
    let mut points = None;
    let mut explanation = None;
    let mut options: Vec<(f64, String)> = Vec::new();
    let mut tf: Option<bool> = None;
    let mut answer_key = String::new();
    let mut choices: Vec<(String, String)> = Vec::new();
    let mut matches: Vec<(String, String)> = Vec::new();
    let mut fib: Vec<(bool, String)> = Vec::new(); // (is blank, text)
    for r in &rows[1..] {
        let key = r[0].to_ascii_lowercase();
        match key.as_str() {
            "questiontext" => stem = cell(r, 1),
            "points" => points = weight(&cell(r, 1)).filter(|p| *p > 0.0),
            "feedback" => explanation = Some(cell(r, 1)).filter(|s| !s.is_empty()),
            "option" | "answer" => options.push((weight(&cell(r, 1)).unwrap_or(0.0), cell(r, 2))),
            "true" | "false" => {
                if weight(&cell(r, 1)).is_some_and(|w| w > 0.0) {
                    tf = Some(key == "true");
                }
            }
            "answerkey" => answer_key = cell(r, 1),
            "choice" => choices.push((cell(r, 1), cell(r, 2))),
            "match" => matches.push((cell(r, 1), cell(r, 2))),
            "item" => options.push((0.0, cell(r, 1))),
            "text" => fib.push((false, cell(r, 1))),
            "blank" => {
                let ans = r[1..].iter().find(|c| !c.is_empty() && weight(c).is_none()).cloned().unwrap_or_default();
                fib.push((true, ans));
            }
            _ => {}
        }
    }
    let rich = |s: &str| if looks_html(s) { html_to_markup(s).0 } else { s.to_string() };
    let kind = match ty.as_str() {
        "MC" | "MS" => {
            let answers: Vec<Answer> = options.iter().map(|(w, t)| Answer { text: rich(t), correct: *w > 0.0, locked: false }).collect();
            if !answers.iter().any(|a| a.correct) {
                return Err("no Option row has a positive weight".into());
            }
            Kind::Mc { answers }
        }
        "TF" => Kind::Tf { correct: tf.ok_or("neither the TRUE nor the FALSE row has a positive weight")? },
        "SA" => {
            let mut accept: Vec<String> = options.iter().filter(|(w, t)| *w > 0.0 && !t.is_empty()).map(|(_, t)| t.clone()).collect();
            dedup(&mut accept);
            if accept.is_empty() {
                return Err("no Answer row has a positive weight".into());
            }
            Kind::Blank { accept: vec![accept] }
        }
        "WR" => Kind::Short { sample: answer_key },
        "M" => {
            let mut used = HashSet::new();
            let mut pairs = Vec::new();
            for (id, left) in &choices {
                if let Some((_, right)) = matches.iter().find(|(mid, _)| mid == id) {
                    used.insert(id.clone());
                    pairs.push(Pair { left: left.clone(), right: right.clone() });
                }
            }
            if pairs.is_empty() {
                return Err("no Choice row has a Match row with the same number".into());
            }
            let distractors = matches.iter().filter(|(id, _)| !used.contains(id)).map(|(_, r)| r.clone()).collect();
            Kind::Matching { pairs, distractors }
        }
        "O" => Kind::Blank { accept: vec![vec![options.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>().join(", ")]] },
        "FIB" => {
            let mut s = String::new();
            let mut accept = Vec::new();
            for (blank, t) in &fib {
                if *blank {
                    s.push_str(&format!(" {BLANK} "));
                    accept.push(vec![t.clone()]);
                } else {
                    if t.starts_with(['.', ',', ';', ':', '!', '?', ')']) {
                        s.truncate(s.trim_end().len());
                    }
                    s.push_str(t);
                    s.push(' ');
                }
            }
            if accept.is_empty() {
                return Err("a FIB question has no Blank row".into());
            }
            stem = s.split_whitespace().collect::<Vec<_>>().join(" ");
            Kind::Blank { accept }
        }
        _ => return Ok(None),
    };
    let stem = rich(stem.trim());
    if stem.is_empty() {
        return Err("no QuestionText row".into());
    }
    let mut q = make_question(stem, kind);
    q.points = points;
    q.explanation = explanation;
    Ok(Some(q))
}

// ---------- Kahoot spreadsheet template ----------

/// Kahoot's quiz template (`Question` | `Answer 1…4` | `Time limit` | `Correct
/// answer(s)`), its header on any row; `None` when no such header row exists.
fn kahoot_rows(rows: &[Vec<String>]) -> Option<Result<Vec<Question>, String>> {
    let norm = |s: &str| s.to_ascii_lowercase().chars().filter(char::is_ascii_alphanumeric).collect::<String>();
    let (h, header) = rows.iter().enumerate().find(|(_, r)| {
        r.iter().any(|c| norm(c).starts_with("question")) && r.iter().any(|c| norm(c).starts_with("answer1")) && r.iter().any(|c| norm(c).starts_with("correctanswer"))
    })?;
    let col = |p: &str| header.iter().position(|c| norm(c).starts_with(p));
    let qc = col("question")?;
    let cc = col("correctanswer")?;
    let ac: Vec<Option<usize>> = (1..=4).map(|k| col(&format!("answer{k}"))).collect();
    let mut out = Vec::new();
    for (n, r) in rows.iter().enumerate().skip(h + 1) {
        let get = |i: usize| r.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        let stem = get(qc);
        if stem.is_empty() {
            continue;
        }
        let keys = correct_letters(&get(cc));
        let mut answers = Vec::new();
        for (k, c) in ac.iter().enumerate() {
            let text = c.map(get).unwrap_or_default();
            if !text.is_empty() {
                answers.push(Answer { text, correct: keys.contains(&k), locked: false });
            }
        }
        if answers.is_empty() || !answers.iter().any(|a| a.correct) {
            return Some(Err(format!("Kahoot row {}: needs answers and a correct answer number (1-4)", n + 1)));
        }
        let kind = tf_from(&answers, false).unwrap_or(Kind::Mc { answers });
        out.push(make_question(stem, kind));
    }
    if out.is_empty() {
        return Some(Err("The Kahoot sheet has a header row but no questions under it.".into()));
    }
    Some(Ok(out))
}

fn try_kahoot_text(text: &str, name: &str) -> Option<Result<ImportDoc, String>> {
    if !text.to_ascii_lowercase().contains("correct answer") {
        return None;
    }
    let rows: Vec<Vec<String>> = csv_records(text).iter().map(|r| split_csv_line(r)).collect();
    kahoot_rows(&rows).map(|r| r.map(|questions| ImportDoc { name: name.to_string(), questions, exams: Vec::new() }))
}

// ---------- zip archives: .xlsx, .docx, QTI / Canvas / Common Cartridge, package zips ----------

type Zip<'a> = zip::ZipArchive<std::io::Cursor<&'a [u8]>>;

/// Entry bytes (capped at 64 MB per entry).
fn zip_bytes(z: &mut Zip<'_>, name: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let f = z.by_name(name).ok()?;
    let size = f.size();
    if size > 64 << 20 {
        return None;
    }
    let mut buf = Vec::with_capacity(size as usize);
    f.take(64 << 20).read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn zip_text(z: &mut Zip<'_>, name: &str) -> Option<String> {
    zip_bytes(z, name).map(|b| decode_text(&b))
}

fn parse_zip(data: &[u8], name: &str) -> Result<ImportDoc, String> {
    let mut z = Zip::new(std::io::Cursor::new(data)).map_err(|_| "That file looks like a zip archive but it could not be opened.".to_string())?;
    let names: Vec<String> = z.file_names().map(str::to_string).collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    if has("xl/workbook.xml") {
        return parse_xlsx(&mut z, &names, name);
    }
    if has("word/document.xml") {
        let xml = zip_text(&mut z, "word/document.xml").ok_or("that Word file has no document body")?;
        return parse_text(&docx_plain(&xml), name, "docx");
    }
    for n in names.iter().filter(|n| n.to_ascii_lowercase().ends_with(".json")) {
        if let Some(t) = zip_text(&mut z, n)
            && t.contains("test-parrot/")
        {
            return parse_text(&t, name, "json");
        }
    }
    qti_zip(&mut z, &names, name)
}

/// A .docx body as plain text: one line per paragraph, `<w:br/>` a line break, tabs spaces.
fn docx_plain(xml: &str) -> String {
    let Ok(doc) = xml_doc(xml) else { return String::new() };
    let mut lines = Vec::new();
    for p in descs(doc.root_element(), "p") {
        let mut s = String::new();
        for n in p.descendants() {
            match qname(n).as_str() {
                "t" => s.push_str(&xtext(n)),
                "tab" => s.push(' '),
                "br" | "cr" => s.push('\n'),
                _ => {}
            }
        }
        lines.push(s);
    }
    lines.join("\n")
}

// ---------- Excel .xlsx ----------

fn parse_xlsx(z: &mut Zip<'_>, names: &[String], name: &str) -> Result<ImportDoc, String> {
    let shared = zip_text(z, "xl/sharedStrings.xml").map(|t| xlsx_shared(&t)).unwrap_or_default();
    let mut sheets: Vec<String> = Vec::new();
    if let (Some(wb), rels) = (zip_text(z, "xl/workbook.xml"), zip_text(z, "xl/_rels/workbook.xml.rels")) {
        sheets = xlsx_sheet_paths(&wb, rels.as_deref());
    }
    if sheets.is_empty() {
        let mut ws: Vec<String> = names.iter().filter(|n| n.starts_with("xl/worksheets/") && n.ends_with(".xml")).cloned().collect();
        ws.sort();
        sheets = ws;
    }
    let mut first_err: Option<String> = None;
    for path in sheets {
        let Some(xml) = zip_text(z, &path) else { continue };
        let rows = xlsx_rows(&xml, &shared)?;
        match rows_to_doc(rows, name) {
            Ok(d) => return Ok(d),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    Err(first_err.unwrap_or_else(|| "The workbook has no worksheets.".into()))
}

/// sharedStrings.xml → its strings (rich-text runs joined; phonetic runs skipped).
fn xlsx_shared(xml: &str) -> Vec<String> {
    let Ok(doc) = xml_doc(xml) else { return Vec::new() };
    kids(doc.root_element(), "si")
        .into_iter()
        .map(|si| si.descendants().filter(|d| qname(*d) == "t" && !d.ancestors().any(|a| qname(a) == "rph")).map(xtext).collect())
        .collect()
}

/// Worksheet paths in workbook (tab) order, through the workbook relationships.
fn xlsx_sheet_paths(workbook: &str, rels: Option<&str>) -> Vec<String> {
    let Ok(wb) = xml_doc(workbook) else { return Vec::new() };
    let mut targets: HashMap<String, String> = HashMap::new();
    if let Some(r) = rels
        && let Ok(rd) = xml_doc(r)
    {
        for rel in descs(rd.root_element(), "relationship") {
            if let (Some(id), Some(t)) = (rel.attribute("Id"), rel.attribute("Target")) {
                let path = match t.strip_prefix('/') {
                    Some(abs) => abs.to_string(),
                    None => join_path("xl", t),
                };
                targets.insert(id.to_string(), path);
            }
        }
    }
    descs(wb.root_element(), "sheet")
        .into_iter()
        .filter_map(|s| s.attributes().find(|a| a.name() == "id").and_then(|a| targets.get(a.value()).cloned()))
        .collect()
}

/// A1-style cell reference → zero-based column.
fn xlsx_col(r: &str) -> Option<usize> {
    let letters: String = r.chars().take_while(char::is_ascii_alphabetic).collect();
    if letters.is_empty() {
        return None;
    }
    Some(letters.to_ascii_uppercase().bytes().fold(0usize, |acc, b| acc * 26 + (b - b'A' + 1) as usize) - 1)
}

/// A worksheet's rows of cell text (shared, inline and formula strings, numbers, booleans).
fn xlsx_rows(xml: &str, shared: &[String]) -> Result<Vec<Vec<String>>, String> {
    let doc = xml_doc(xml).map_err(|e| format!("a worksheet of the workbook could not be read ({e})"))?;
    let mut rows = Vec::new();
    for row in descs(doc.root_element(), "row") {
        let mut cells: Vec<String> = Vec::new();
        for c in kids(row, "c") {
            let col = c.attribute("r").and_then(xlsx_col).unwrap_or(cells.len());
            if col > 1000 {
                continue;
            }
            let v = kid(c, "v").map(xtext).unwrap_or_default();
            let text = match c.attribute("t").unwrap_or("n") {
                "s" => v.trim().parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                "inlineStr" => kid(c, "is").map(|is| descs(is, "t").into_iter().map(xtext).collect()).unwrap_or_default(),
                "b" => if v.trim() == "1" { "TRUE".into() } else { "FALSE".into() },
                "e" => String::new(),
                _ => v,
            };
            if cells.len() <= col {
                cells.resize(col + 1, String::new());
            }
            cells[col] = text;
        }
        rows.push(cells);
    }
    Ok(rows)
}

/// Spreadsheet rows → questions: the CSV header grammar first (exactly as a .csv), then
/// the Kahoot template, then a two-column term/definition list.
fn rows_to_doc(rows: Vec<Vec<String>>, name: &str) -> Result<ImportDoc, String> {
    let rows: Vec<Vec<String>> = rows.into_iter().filter(|r| r.iter().any(|c| !c.trim().is_empty())).collect();
    if rows.is_empty() {
        return Err("The spreadsheet is empty.".into());
    }
    let doc = |questions| ImportDoc { name: name.to_string(), questions, exams: Vec::new() };
    let quote = |c: &String| format!("\"{}\"", c.replace('"', "\"\""));
    let csv: String = rows.iter().map(|r| r.iter().map(quote).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join("\n");
    if let Some(r) = try_csv(&csv) {
        return r.map(doc);
    }
    if let Some(r) = kahoot_rows(&rows) {
        return r.map(doc);
    }
    let two: Vec<&Vec<String>> = rows.iter().filter(|r| r.iter().filter(|c| !c.trim().is_empty()).count() >= 2).collect();
    if two.len() == rows.len() {
        let tsv: String = rows.iter().map(|r| r.iter().filter(|c| !c.trim().is_empty()).take(2).map(|c| c.replace(['\t', '\n'], " ")).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n");
        if let Some(pairs) = term_pairs(&tsv) {
            return Ok(doc(term_questions(pairs)));
        }
    }
    Err("The spreadsheet's first row must be a header naming `type` and `stem` (the CSV import's columns), the Kahoot template's header, or the sheet must be two columns of term and definition.".into())
}

// ---------- XML dispatch ----------

/// Moodle XML (`<quiz>` of `<question>`s) or a QTI file (`<questestinterop>`,
/// `<assessmentItem>`, QTI 3 `<qti-assessment-item>`). A `.xml` file is claimed even
/// when it is neither (with a helpful error); other text only when it parses as one.
fn try_xml(text: &str, name: &str, forced: bool) -> Option<Result<ImportDoc, String>> {
    let t = text.trim_start();
    if !t.starts_with('<') {
        return None;
    }
    let doc = match xml_doc(t) {
        Ok(d) => d,
        Err(e) => return forced.then(|| Err(format!("That XML file could not be read ({e})."))),
    };
    let root = doc.root_element();
    match qname(root).as_str() {
        "quiz" if root.children().any(|c| qname(c) == "question") => Some(parse_moodle(root, name)),
        "questestinterop" | "assessmentitem" => {
            let mut acc = QtiAcc::default();
            qti_root(root, &mut acc, "");
            Some(acc.finish(name))
        }
        _ if forced => Some(Err(
            "This XML is neither Moodle XML (<quiz>) nor QTI (<questestinterop>/<assessmentItem>). Export the quiz as Moodle XML, QTI, or a Canvas export zip.".into(),
        )),
        _ => None,
    }
}

// ---------- Moodle XML ----------

fn parse_moodle(root: XNode<'_, '_>, name: &str) -> Result<ImportDoc, String> {
    let mut topic: Option<String> = None;
    let mut questions = Vec::new();
    let mut first_err: Option<String> = None;
    let mut skipped: Vec<String> = Vec::new();
    for (i, q) in root.children().filter(|c| qname(*c) == "question").enumerate() {
        let ty = q.attribute("type").unwrap_or("").to_ascii_lowercase();
        if ty == "category" {
            topic = kid(q, "category").and_then(|c| kid(c, "text")).and_then(|t| category_topic(xtext(t).trim()));
            continue;
        }
        match moodle_question(q, &ty) {
            Ok(Some(mut qq)) => {
                if let Some(t) = &topic {
                    qq.topics = vec![t.clone()];
                }
                questions.push(qq);
            }
            Ok(None) => skipped.push(ty),
            Err(e) => {
                let label = kid(q, "name").and_then(|n| kid(n, "text")).map(|t| xtext(t).trim().to_string()).unwrap_or_default();
                first_err.get_or_insert(format!("Moodle question {} {label:?}: {e}", i + 1));
            }
        }
    }
    if questions.is_empty() {
        dedup(&mut skipped);
        return Err(first_err.unwrap_or_else(|| format!("This Moodle XML holds no importable questions (only: {}).", skipped.join(", "))));
    }
    Ok(ImportDoc { name: name.to_string(), questions, exams: Vec::new() })
}

/// A Moodle text field (`<x format=".."><text>..</text></x>`) → (markup, image srcs).
fn moodle_rich(n: XNode<'_, '_>) -> (String, Vec<String>) {
    let text = kid(n, "text").map(xtext).unwrap_or_default();
    match n.attribute("format").unwrap_or("html") {
        "html" => html_to_markup(&text),
        _ if looks_html(&text) => html_to_markup(&text),
        _ => (tex_to_dollars(text.trim()), Vec::new()),
    }
}

fn moodle_inline(n: XNode<'_, '_>) -> String {
    single_line(&moodle_rich(n).0)
}

/// The base64 `<file name=..>`s embedded in a Moodle text field, keyed by name.
fn moodle_files(n: XNode<'_, '_>) -> HashMap<String, Vec<u8>> {
    kids(n, "file")
        .into_iter()
        .filter(|f| f.attribute("encoding").is_none_or(|e| e.eq_ignore_ascii_case("base64")))
        .filter_map(|f| Some((f.attribute("name")?.to_ascii_lowercase(), base64_decode(&xtext(f))?)))
        .collect()
}

/// One Moodle answer: (fraction, inline text, node).
fn moodle_answers<'a, 'i>(q: XNode<'a, 'i>) -> Vec<(f64, String, XNode<'a, 'i>)> {
    kids(q, "answer").into_iter().map(|a| (a.attribute("fraction").and_then(|f| f.trim().parse().ok()).unwrap_or(0.0), moodle_inline(a), a)).collect()
}

fn moodle_question(q: XNode<'_, '_>, ty: &str) -> Result<Option<Question>, String> {
    let qt = kid(q, "questiontext");
    let raw_html = qt.and_then(|n| kid(n, "text")).map(xtext).unwrap_or_default();
    let (mut stem, srcs) = qt.map(moodle_rich).unwrap_or_default();
    let answers = moodle_answers(q);
    let kind = match ty {
        "multichoice" => {
            let answers: Vec<Answer> = answers.iter().map(|(f, t, _)| Answer { text: t.clone(), correct: *f > 0.0, locked: false }).collect();
            if answers.is_empty() {
                return Err("no answers".into());
            }
            if !answers.iter().any(|a| a.correct) {
                return Err("no answer has a positive fraction".into());
            }
            Kind::Mc { answers }
        }
        "truefalse" => {
            let right = answers.iter().find(|(f, _, _)| *f > 0.0).ok_or("neither true nor false has fraction 100")?;
            Kind::Tf { correct: right.1.trim().eq_ignore_ascii_case("true") }
        }
        "shortanswer" => {
            let mut accept: Vec<String> = answers.iter().filter(|(f, t, _)| *f > 0.0 && !t.is_empty()).map(|(_, t, _)| t.clone()).collect();
            dedup(&mut accept);
            if accept.is_empty() {
                return Err("no accepted answer (fraction > 0)".into());
            }
            Kind::Blank { accept: vec![accept] }
        }
        "numerical" => {
            let mut accept = Vec::new();
            for (f, t, a) in &answers {
                if *f <= 0.0 || t.trim() == "*" {
                    continue;
                }
                let tol = kid(*a, "tolerance").map(xtext).unwrap_or_default();
                accept.extend(num_with_tolerance(t, &tol).ok_or_else(|| format!("{t:?} is not a number"))?);
            }
            dedup(&mut accept);
            if accept.is_empty() {
                return Err("no correct numeric answer".into());
            }
            Kind::Blank { accept: vec![accept] }
        }
        "matching" => {
            let mut pairs = Vec::new();
            let mut distractors = Vec::new();
            for s in kids(q, "subquestion") {
                let left = moodle_inline(s);
                let right = kid(s, "answer").and_then(|a| kid(a, "text")).map(|t| single_line(xtext(t).trim())).unwrap_or_default();
                if right.is_empty() {
                    continue;
                }
                if left.is_empty() { distractors.push(right) } else { pairs.push(Pair { left, right }) }
            }
            if pairs.is_empty() {
                return Err("no subquestion pairs".into());
            }
            if stem.is_empty() {
                stem = MAT_DEFAULT_STEM.to_string();
            }
            Kind::Matching { pairs, distractors }
        }
        "essay" => {
            let sample = kid(q, "graderinfo").map(moodle_rich).map(|r| r.0).filter(|s| !s.is_empty()).or_else(|| kid(q, "responsetemplate").map(moodle_rich).map(|r| r.0)).unwrap_or_default();
            Kind::Short { sample }
        }
        "multianswer" => {
            let (html, accept) = cloze(&raw_html)?;
            stem = html_to_markup(&html).0;
            Kind::Blank { accept }
        }
        "ddwtos" | "gapselect" => {
            let choices: Vec<String> = q.children().filter(|c| matches!(qname(*c).as_str(), "dragbox" | "selectoption")).map(|c| kid(c, "text").map(|t| single_line(xtext(t).trim())).unwrap_or_default()).collect();
            let (html, accept) = gap_markers(&raw_html, &choices)?;
            stem = html_to_markup(&html).0;
            Kind::Blank { accept }
        }
        _ => return Ok(None), // description, calculated*, random, drag-drop onto image…
    };
    if stem.trim().is_empty() {
        return Err("empty question text".into());
    }
    let mut out = make_question(stem, kind);
    out.points = kid(q, "defaultgrade").and_then(|g| xtext(g).trim().parse::<f64>().ok()).filter(|p| p.is_finite() && *p > 0.0);
    out.explanation = kid(q, "generalfeedback").map(|g| moodle_rich(g).0).filter(|s| !s.trim().is_empty());
    if let Some(qt) = qt {
        attach_src(&mut out, &srcs, &moodle_files(qt), "");
    }
    Ok(Some(out))
}

/// Moodle Cloze (`{1:SHORTANSWER:=Paris~%50%paris}`, `{:MULTICHOICE:~a~=b}`,
/// `{2:NUMERICAL:=5:0.1}`) → the HTML with each sub-question a `___` blank, plus the
/// accepted answers per blank (positive-weight answers; MC: the correct choice texts).
fn cloze(html: &str) -> Result<(String, Vec<Vec<String>>), String> {
    let mut out = String::new();
    let mut accept = Vec::new();
    let mut rest = html;
    while let Some(open) = find_unescaped(rest, 0, |c| c == '{') {
        let Some(close) = find_unescaped(rest, open + 1, |c| c == '}') else { break };
        let inner = &rest[open + 1..close];
        let mut parts = inner.splitn(3, ':');
        let (w, ty, body) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next());
        let is_cloze = w.trim().chars().all(|c| c.is_ascii_digit()) && !ty.is_empty() && ty.chars().all(|c| c.is_ascii_alphabetic() || c == '_') && body.is_some();
        out.push_str(&rest[..open]);
        if !is_cloze {
            out.push_str(&rest[open..=close]);
            rest = &rest[close + 1..];
            continue;
        }
        let numeric = matches!(ty.to_ascii_uppercase().as_str(), "NUMERICAL" | "NM");
        let mut ok = Vec::new();
        for raw in split_unescaped(body.unwrap_or(""), '~') {
            let mut a = raw.trim();
            if let Some(h) = find_unescaped(a, 0, |c| c == '#') {
                a = a[..h].trim();
            }
            let positive = if let Some(r) = a.strip_prefix('=') {
                a = r;
                true
            } else if let Some(r) = a.strip_prefix('%')
                && let Some(p) = r.find('%')
            {
                let w = r[..p].trim().parse::<f64>().unwrap_or(0.0);
                a = &r[p + 1..];
                w > 0.0
            } else {
                false
            };
            if !positive {
                continue;
            }
            let text = gift_unescape(a.trim());
            if numeric {
                let (v, t) = text.split_once(':').unwrap_or((&text, "0"));
                ok.extend(num_with_tolerance(v, t).ok_or_else(|| format!("Cloze numeric answer {text:?} is not a number"))?);
            } else {
                ok.push(single_line(&html_to_markup(&text).0));
            }
        }
        dedup(&mut ok);
        if ok.is_empty() {
            return Err(format!("Cloze blank {} has no correct answer", accept.len() + 1));
        }
        accept.push(ok);
        out.push_str(&format!(" {BLANK} "));
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    if accept.is_empty() {
        return Err("a Cloze question with no {…} sub-questions".into());
    }
    Ok((out, accept))
}

fn split_unescaped(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = find_unescaped(s, from, |c| c == sep) {
        out.push(&s[from..p]);
        from = p + sep.len_utf8();
    }
    out.push(&s[from..]);
    out
}

/// `[[n]]` gap markers (drag-and-drop into text, select missing words) → `___` blanks,
/// each accepting choice n (1-based).
fn gap_markers(html: &str, choices: &[String]) -> Result<(String, Vec<Vec<String>>), String> {
    let mut out = String::new();
    let mut accept = Vec::new();
    let mut rest = html;
    while let Some(p) = rest.find("[[") {
        let Some(e) = rest[p + 2..].find("]]") else { break };
        let num = &rest[p + 2..p + 2 + e];
        out.push_str(&rest[..p]);
        match num.trim().parse::<usize>() {
            Ok(n) if n >= 1 => {
                let c = choices.get(n - 1).ok_or_else(|| format!("gap [[{n}]] names a choice that does not exist"))?;
                accept.push(vec![c.clone()]);
                out.push_str(&format!(" {BLANK} "));
            }
            _ => out.push_str(&rest[p..p + 4 + e]),
        }
        rest = &rest[p + 4 + e..];
    }
    out.push_str(rest);
    if accept.is_empty() {
        return Err("no [[n]] gaps in the question text".into());
    }
    Ok((out, accept))
}

// ---------- QTI ----------

/// Questions gathered from one or more QTI documents.
#[derive(Default)]
struct QtiAcc {
    questions: Vec<Question>,
    /// The assessment (quiz) title each question came from, parallel to `questions`.
    origin: Vec<String>,
    seen: HashSet<String>,
    title: String,
    first_err: Option<String>,
    media: HashMap<String, Vec<u8>>,
}

type QtiResult = Result<Option<(Question, Vec<String>)>, String>;

impl QtiAcc {
    /// Adds one item's result; a repeated item ident (Common Cartridge zips carry each
    /// quiz twice) is ignored, a failed item is skipped with its error remembered.
    fn take(&mut self, ident: &str, label: &str, base: &str, origin: &str, r: QtiResult) {
        if !ident.is_empty() && !self.seen.insert(ident.to_string()) {
            return;
        }
        match r {
            Ok(Some((mut q, srcs))) => {
                attach_src(&mut q, &srcs, &self.media, base);
                self.questions.push(q);
                self.origin.push(origin.to_string());
            }
            Ok(None) => {}
            Err(e) => {
                self.first_err.get_or_insert_with(|| format!("QTI item {label:?}: {e}"));
            }
        }
    }

    fn finish(mut self, name: &str) -> Result<ImportDoc, String> {
        if self.questions.is_empty() {
            return Err(self.first_err.unwrap_or_else(|| "The QTI file holds no importable questions.".into()));
        }
        let distinct: HashSet<&String> = self.origin.iter().filter(|o| !o.is_empty()).collect();
        if distinct.len() > 1 {
            for (q, o) in self.questions.iter_mut().zip(&self.origin) {
                if !o.is_empty() && q.topics.is_empty() {
                    q.topics.push(o.clone());
                }
            }
        }
        let name = if self.title.trim().is_empty() { name.to_string() } else { self.title.trim().to_string() };
        Ok(ImportDoc { name, questions: self.questions, exams: Vec::new() })
    }
}

fn qti_root(root: XNode<'_, '_>, acc: &mut QtiAcc, base: &str) {
    match qname(root).as_str() {
        "questestinterop" => {
            for item in descs(root, "item") {
                let quiz = item.ancestors().find(|a| matches!(qname(*a).as_str(), "assessment" | "objectbank"));
                let origin = quiz.and_then(|a| a.attribute("title")).unwrap_or("").trim().to_string();
                if acc.title.is_empty() && !origin.is_empty() {
                    acc.title = origin.clone();
                }
                let ident = item.attribute("ident").unwrap_or("");
                let label = item.attribute("title").unwrap_or(ident).to_string();
                acc.take(ident, &label, base, &origin, qti1_item(item));
            }
        }
        "assessmentitem" => {
            let ident = xattr(root, "identifier").unwrap_or("");
            let label = xattr(root, "title").unwrap_or(ident).to_string();
            acc.take(ident, &label, base, "", qti2_item(root));
        }
        _ => {}
    }
}

fn qti_zip(z: &mut Zip<'_>, names: &[String], name: &str) -> Result<ImportDoc, String> {
    let mut xmls: Vec<(String, String)> = Vec::new();
    let mut media: HashMap<String, Vec<u8>> = HashMap::new();
    let mut total = 0usize;
    for n in names {
        let low = n.to_ascii_lowercase();
        if low.ends_with('/') {
            continue;
        }
        if low.ends_with(".xml") || low.ends_with(".qti") {
            if let Some(t) = zip_text(z, n) {
                xmls.push((n.clone(), t));
            }
        } else if [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"].iter().any(|e| low.ends_with(e))
            && total < 64 << 20
            && let Some(b) = zip_bytes(z, n)
        {
            total += b.len();
            media.insert(low, b);
        }
    }
    let docs: Vec<(&str, roxmltree::Document<'_>)> = xmls.iter().filter_map(|(p, t)| xml_doc(t.trim_start()).ok().map(|d| (p.as_str(), d))).collect();
    let mut acc = QtiAcc { media, ..QtiAcc::default() };
    let mut order: Vec<String> = Vec::new();
    let mut manifest_title: Option<String> = None;
    for (p, d) in &docs {
        let r = d.root_element();
        if qname(r) == "assessmenttest" {
            if acc.title.is_empty() {
                acc.title = xattr(r, "title").unwrap_or("").to_string();
            }
            for refn in descs(r, "assessmentitemref") {
                if let Some(h) = xattr(refn, "href") {
                    order.push(join_path(dir_of(p), &percent_decode(h)));
                }
            }
        }
    }
    for (p, d) in &docs {
        if p.to_ascii_lowercase().ends_with("imsmanifest.xml") {
            let r = d.root_element();
            manifest_title = desc(r, "title").map(|t| xtext(t).trim().to_string()).filter(|t| !t.is_empty());
            for res in descs(r, "resource") {
                for h in res.attribute("href").into_iter().chain(kids(res, "file").into_iter().filter_map(|f| f.attribute("href"))) {
                    order.push(join_path(dir_of(p), &percent_decode(h)));
                }
            }
        }
    }
    let mut rest: Vec<&str> = docs.iter().map(|(p, _)| *p).collect();
    rest.sort_unstable();
    order.extend(rest.into_iter().map(str::to_string));
    let mut done: HashSet<String> = HashSet::new();
    let mut found = false;
    for p in order {
        if !done.insert(p.to_ascii_lowercase()) {
            continue;
        }
        let Some((path, d)) = docs.iter().find(|(dp, _)| dp.eq_ignore_ascii_case(&p)) else { continue };
        let root = d.root_element();
        if matches!(qname(root).as_str(), "questestinterop" | "assessmentitem") {
            found = true;
            qti_root(root, &mut acc, dir_of(path));
        }
    }
    if !found {
        return Err("This zip holds no question files: expected QTI XML (an export from Canvas, Blackboard, D2L or another LMS), an .xlsx, a .docx or a Test Parrot package.".into());
    }
    if acc.title.is_empty()
        && let Some(t) = manifest_title
    {
        acc.title = t;
    }
    acc.finish(name)
}

// ---------- QTI 1.2 (Canvas, Blackboard, D2L, Respondus exports) ----------

/// Item metadata fields (`question_type`, `points_possible`…), lower-case labels.
fn qti1_meta(item: XNode<'_, '_>) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for f in descs(item, "qtimetadatafield") {
        if let (Some(l), Some(e)) = (kid(f, "fieldlabel"), kid(f, "fieldentry")) {
            m.insert(xtext(l).trim().to_ascii_lowercase(), xtext(e).trim().to_string());
        }
    }
    for tag in ["bbmd_questiontype", "qmd_itemtype"] {
        if let Some(b) = desc(item, tag) {
            m.entry(tag.to_string()).or_insert_with(|| xtext(b).trim().to_string());
        }
    }
    m
}

/// A node's material texts in order (`mattext`/`mat_formattedtext`/`matemtext`, HTML
/// converted; `matimage` a `[picture]`), image srcs collected.
fn qti1_material(n: XNode<'_, '_>, srcs: &mut Vec<String>) -> String {
    let mut chunks = Vec::new();
    for c in n.descendants() {
        match qname(c).as_str() {
            "mattext" | "matemtext" | "mat_formattedtext" => {
                let t = xtext(c);
                let html = xattr(c, "texttype").or(c.attribute("type")).is_some_and(|t| t.to_ascii_lowercase().contains("html")) || looks_html(&t);
                let s = if html {
                    let (s, imgs) = html_to_markup(&t);
                    srcs.extend(imgs);
                    s
                } else {
                    tex_to_dollars(t.trim())
                };
                push_chunk(&mut chunks, s);
            }
            "matimage" => {
                if let Some(u) = c.attribute("uri") {
                    srcs.push(u.to_string());
                }
                chunks.push(picture_text(""));
            }
            _ => {}
        }
    }
    chunks.join("\n\n")
}

struct Resp1 {
    ident: String,
    kind: char,
    multiple: bool,
    prompt: String,
    labels: Vec<(String, String)>,
}

fn qti1_item(item: XNode<'_, '_>) -> QtiResult {
    let meta = qti1_meta(item);
    let qtype = ["question_type", "bbmd_questiontype", "qmd_itemtype", "cc_profile"]
        .iter()
        .find_map(|k| meta.get(*k))
        .map(|t| t.to_ascii_lowercase().replace([' ', '-', '/'], "_"))
        .unwrap_or_default();
    if qtype.contains("text_only") || qtype.contains("calculated") || qtype.contains("hot_spot") || qtype.contains("hotspot") {
        return Ok(None);
    }
    let pres = desc(item, "presentation").ok_or("no <presentation>")?;
    let response_like = |n: &str| n.starts_with("response_") || n.starts_with("render_") || n == "flow_label" || n == "material";
    let mut srcs = Vec::new();
    let mut chunks = Vec::new();
    for m in pres.descendants().filter(|d| qname(*d) == "material") {
        if m.ancestors().skip(1).take_while(|a| *a != pres).any(|a| response_like(&qname(a))) {
            continue;
        }
        push_chunk(&mut chunks, qti1_material(m, &mut srcs));
    }
    let mut stem = chunks.join("\n\n");
    let mut resps: Vec<Resp1> = Vec::new();
    for r in pres.descendants() {
        let kind = match qname(r).as_str() {
            "response_lid" => 'l',
            "response_str" => 's',
            "response_num" => 'n',
            _ => continue,
        };
        let mut ignore = Vec::new();
        let prompt = kids(r, "material").into_iter().map(|m| single_line(&qti1_material(m, &mut ignore))).collect::<Vec<_>>().join(" ");
        let labels = descs(r, "response_label").into_iter().map(|l| (l.attribute("ident").unwrap_or("").to_string(), single_line(&qti1_material(l, &mut ignore)))).collect();
        resps.push(Resp1 {
            ident: r.attribute("ident").unwrap_or("").to_string(),
            kind,
            multiple: r.attribute("rcardinality").is_some_and(|c| c.eq_ignore_ascii_case("multiple")),
            prompt,
            labels,
        });
    }
    let (eq, ranges) = qti1_correct(item);
    let correct_for = |ident: &str| -> Vec<String> {
        if resps.len() == 1 {
            eq.values().flatten().cloned().collect()
        } else {
            eq.get(ident).cloned().unwrap_or_default()
        }
    };
    let lids: Vec<&Resp1> = resps.iter().filter(|r| r.kind == 'l').collect();
    let has = |k: &str| qtype.contains(k);
    let blanks = has("fill_in_multiple_blanks") || has("multiple_dropdowns") || (!lids.is_empty() && lids.iter().all(|r| !r.prompt.is_empty() && stem.contains(&format!("[{}]", r.prompt))));
    let kind = if blanks {
        let mut placed: Vec<(usize, &Resp1)> = lids.iter().filter_map(|r| stem.find(&format!("[{}]", r.prompt)).map(|p| (p, *r))).collect();
        placed.sort_by_key(|x| x.0);
        let mut accept = Vec::new();
        for (_, r) in &placed {
            let ok = correct_for(&r.ident);
            let mut v: Vec<String> = r.labels.iter().filter(|(id, _)| ok.contains(id)).map(|(_, t)| t.clone()).collect();
            if v.is_empty() {
                v = r.labels.iter().map(|(_, t)| t.clone()).collect();
            }
            dedup(&mut v);
            if v.is_empty() {
                return Err(format!("blank [{}] has no answers", r.prompt));
            }
            accept.push(v);
            stem = stem.replacen(&format!("[{}]", r.prompt), BLANK, 1);
        }
        if accept.is_empty() {
            return Err("no [blank] markers in the question text".into());
        }
        Kind::Blank { accept }
    } else if has("matching") || (lids.len() >= 2 && lids.iter().all(|r| !r.prompt.is_empty())) {
        let mut rights: Vec<(String, String)> = Vec::new();
        for r in &lids {
            for l in &r.labels {
                if !rights.iter().any(|x| x.0 == l.0) {
                    rights.push(l.clone());
                }
            }
        }
        let mut pairs = Vec::new();
        let mut used: HashSet<String> = HashSet::new();
        for r in &lids {
            let Some(ans) = eq.get(&r.ident).and_then(|v| v.first()) else { continue };
            let Some((_, right)) = r.labels.iter().find(|(id, _)| id == ans) else { continue };
            used.insert(right.clone());
            pairs.push(Pair { left: r.prompt.clone(), right: right.clone() });
        }
        if pairs.is_empty() {
            return Err("no matching pair has a correct answer".into());
        }
        let mut distractors: Vec<String> = rights.into_iter().map(|r| r.1).filter(|t| !used.contains(t) && !t.is_empty()).collect();
        dedup(&mut distractors);
        if stem.trim().is_empty() {
            stem = MAT_DEFAULT_STEM.to_string();
        }
        Kind::Matching { pairs, distractors }
    } else if has("numerical") || has("numeric") || resps.iter().any(|r| r.kind == 'n') {
        let mut accept: Vec<String> = Vec::new();
        for v in eq.values().flatten() {
            accept.extend(parse_num_answer(v).unwrap_or_else(|| vec![v.clone()]));
        }
        for (lo, hi) in &ranges {
            match (lo, hi) {
                (Some(a), Some(b)) => accept.extend(num_range(*a, *b)),
                (Some(x), None) | (None, Some(x)) => accept.push(fmt_num(*x)),
                (None, None) => {}
            }
        }
        dedup(&mut accept);
        if accept.is_empty() {
            return Err("no correct numeric answer".into());
        }
        Kind::Blank { accept: vec![accept] }
    } else if has("essay") || has("file_upload") {
        Kind::Short { sample: String::new() }
    } else if lids.len() == 1 {
        let r = lids[0];
        let ok = correct_for(&r.ident);
        let answers: Vec<Answer> = r.labels.iter().map(|(id, t)| Answer { text: t.clone(), correct: ok.contains(id), locked: false }).collect();
        if answers.is_empty() {
            return Err("a choice question with no choices".into());
        }
        if !answers.iter().any(|a| a.correct) {
            return Err("no choice is marked correct".into());
        }
        let multi = r.multiple || has("multiple_answer");
        if has("true_false") || has("either_or") {
            match tf_from(&answers, false) {
                Some(k) => k,
                None => Kind::Tf { correct: answers[0].correct },
            }
        } else {
            tf_from(&answers, multi).unwrap_or(Kind::Mc { answers })
        }
    } else if resps.iter().any(|r| r.kind == 's') {
        let mut accept: Vec<String> = eq.values().flatten().filter(|v| !v.is_empty()).cloned().collect();
        dedup(&mut accept);
        if accept.is_empty() || has("essay") { Kind::Short { sample: String::new() } } else { Kind::Blank { accept: vec![accept] } }
    } else {
        return Err(format!("unsupported question type {qtype:?}"));
    };
    if stem.trim().is_empty() {
        return Err("empty question text".into());
    }
    let mut q = make_question(stem, kind);
    q.points = meta.get("points_possible").and_then(|p| p.parse::<f64>().ok()).filter(|p| p.is_finite() && *p > 0.0);
    q.explanation = descs(item, "itemfeedback")
        .into_iter()
        .find(|f| f.attribute("ident").is_some_and(|i| i == "general_fb"))
        .map(|f| qti1_material(f, &mut Vec::new()))
        .filter(|s| !s.trim().is_empty());
    Ok(Some((q, srcs)))
}

/// Correct responses of a QTI 1.2 item from its positive-score `respcondition`s:
/// `varequal` values per response ident (those under `<not>` skipped), and numeric
/// `vargte`/`varlte` ranges.
type Ranges = Vec<(Option<f64>, Option<f64>)>;

fn qti1_correct(item: XNode<'_, '_>) -> (HashMap<String, Vec<String>>, Ranges) {
    let mut eq: HashMap<String, Vec<String>> = HashMap::new();
    let mut ranges = Vec::new();
    for rc in descs(item, "respcondition") {
        let sv = kids(rc, "setvar");
        let positive = if sv.is_empty() {
            rc.attribute("title").is_some_and(|t| {
                let t = t.to_ascii_lowercase();
                t.contains("correct") && !t.contains("incorrect")
            })
        } else {
            sv.iter().any(|s| {
                let act = s.attribute("action").unwrap_or("Set").to_ascii_lowercase();
                let v = xtext(*s).trim().to_string();
                act != "subtract" && v.parse::<f64>().map_or(!v.is_empty(), |x| x > 0.0)
            })
        };
        if !positive {
            continue;
        }
        let Some(cv) = kid(rc, "conditionvar") else { continue };
        let (mut lo, mut hi) = (None, None);
        for d in cv.descendants().filter(|d| d.is_element()) {
            if d.ancestors().skip(1).take_while(|a| *a != cv).any(|a| qname(a) == "not") || qname(d) == "not" {
                continue;
            }
            let v = xtext(d).trim().to_string();
            match qname(d).as_str() {
                "varequal" => eq.entry(d.attribute("respident").unwrap_or("").to_string()).or_default().push(v),
                "vargte" | "vargt" => lo = v.parse().ok(),
                "varlte" | "varlt" => hi = v.parse().ok(),
                _ => {}
            }
        }
        if lo.is_some() || hi.is_some() {
            ranges.push((lo, hi));
        }
    }
    (eq, ranges)
}

// ---------- QTI 2.x / 3.0 ----------

struct Decl {
    multiple: bool,
    correct: Vec<String>,
    mapped: Vec<String>,
}

fn qti2_decls(item: XNode<'_, '_>) -> HashMap<String, Decl> {
    let mut m = HashMap::new();
    for d in kids(item, "responsedeclaration") {
        let Some(id) = xattr(d, "identifier") else { continue };
        let multiple = xattr(d, "cardinality").is_some_and(|c| c != "single");
        let correct = kid(d, "correctresponse").map(|c| kids(c, "value").into_iter().map(|v| xtext(v).trim().to_string()).collect()).unwrap_or_default();
        let mapped = kid(d, "mapping")
            .map(|mp| {
                kids(mp, "mapentry")
                    .into_iter()
                    .filter(|e| xattr(*e, "mappedValue").and_then(|v| v.trim().parse::<f64>().ok()).is_some_and(|v| v > 0.0))
                    .filter_map(|e| xattr(e, "mapKey").map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        m.insert(id.to_string(), Decl { multiple, correct, mapped });
    }
    m
}

/// QTI 2 body XML → HTML tree: inline interactions and gaps become `___`, block
/// interactions keep only their `<prompt>` (and only when `prompts`), feedback and
/// choice lists are dropped, image `<object>`s become `<img>`.
fn qti2_h(n: XNode<'_, '_>, prompts: bool) -> Option<HNode> {
    if n.is_text() {
        return n.text().map(|t| HNode::Text(t.to_string()));
    }
    if !n.is_element() {
        return None;
    }
    let q = qname(n);
    match q.as_str() {
        "textentryinteraction" | "inlinechoiceinteraction" | "gap" => return Some(HNode::Text(format!(" {BLANK} "))),
        "feedbackblock" | "feedbackinline" | "modalfeedback" | "rubricblock" | "templateblock" | "templateinline" | "printedvariable" | "stylesheet" | "gaptext"
        | "gapimg" | "simplechoice" | "simplematchset" | "simpleassociablechoice" | "inlinechoice" => return None,
        _ => {}
    }
    let div = |kids: Vec<HNode>| Some(HNode::El { name: "div".into(), attrs: Vec::new(), kids });
    if q == "gapmatchinteraction" {
        return div(n.children().filter(|c| prompts || qname(*c) != "prompt").filter_map(|c| qti2_h(c, prompts)).collect());
    }
    if q.ends_with("interaction") {
        if !prompts {
            return None;
        }
        return div(kid(n, "prompt").map(|p| p.children().filter_map(|c| qti2_h(c, prompts)).collect()).unwrap_or_default());
    }
    let mut attrs: Vec<(String, String)> = n.attributes().map(|a| (a.name().to_ascii_lowercase(), a.value().to_string())).collect();
    let mut name = n.tag_name().name().to_ascii_lowercase();
    if name == "object" && n.attribute("type").is_some_and(|t| t.starts_with("image")) {
        name = "img".into();
        if let Some(d) = n.attribute("data") {
            attrs.push(("src".into(), d.to_string()));
        }
    }
    Some(HNode::El { name, attrs, kids: n.children().filter_map(|c| qti2_h(c, prompts)).collect() })
}

fn xinline(n: XNode<'_, '_>) -> String {
    HtmlOut::default().one_line(&n.children().filter_map(|c| qti2_h(c, true)).collect::<Vec<_>>(), false)
}

fn qti2_block(b: XNode<'_, '_>, decl: Option<&Decl>) -> Result<Kind, String> {
    let correct: Vec<String> = decl.map(|d| if d.correct.is_empty() { d.mapped.clone() } else { d.correct.clone() }).unwrap_or_default();
    let choices = |tag: &str| -> Vec<(String, String, bool)> {
        descs(b, tag).into_iter().map(|c| (xattr(c, "identifier").unwrap_or("").to_string(), xinline(c), xattr(c, "fixed") == Some("true"))).collect()
    };
    let text_of = |list: &[(String, String, bool)], id: &str| list.iter().find(|c| c.0 == id).map(|c| c.1.clone());
    match qname(b).as_str() {
        "choiceinteraction" => {
            let list = choices("simplechoice");
            if list.is_empty() {
                return Err("a choice interaction with no choices".into());
            }
            let multi = xattr(b, "maxChoices").is_some_and(|m| m.trim() != "1") || decl.is_some_and(|d| d.multiple);
            let answers: Vec<Answer> = list.iter().map(|(id, t, fixed)| Answer { text: t.clone(), correct: correct.contains(id), locked: *fixed }).collect();
            if !answers.iter().any(|a| a.correct) {
                return Err("no choice is in the correct response".into());
            }
            Ok(tf_from(&answers, multi).unwrap_or(Kind::Mc { answers }))
        }
        "orderinteraction" => {
            let list = choices("simplechoice");
            let ordered: Vec<String> = correct.iter().filter_map(|id| text_of(&list, id)).collect();
            if ordered.is_empty() {
                return Err("an order interaction with no correct order".into());
            }
            Ok(Kind::Blank { accept: vec![vec![ordered.join(", ")]] })
        }
        "matchinteraction" | "associateinteraction" => {
            let sets = kids(b, "simplematchset");
            let (left, right): (Vec<_>, Vec<_>) = if sets.len() >= 2 {
                let set = |s: XNode<'_, '_>| kids(s, "simpleassociablechoice").into_iter().map(|c| (xattr(c, "identifier").unwrap_or("").to_string(), xinline(c), false)).collect::<Vec<_>>();
                (set(sets[0]), set(sets[1]))
            } else {
                let all = choices("simpleassociablechoice");
                (all.clone(), all)
            };
            let mut pairs = Vec::new();
            let mut used = HashSet::new();
            for v in &correct {
                let Some((a, c)) = v.split_once(char::is_whitespace) else { continue };
                let (a, c) = (a.trim(), c.trim());
                let found = text_of(&left, a).zip(text_of(&right, c)).map(|p| (p, c)).or_else(|| text_of(&left, c).zip(text_of(&right, a)).map(|p| (p, a)));
                if let Some(((l, r), rid)) = found {
                    used.insert(rid.to_string());
                    pairs.push(Pair { left: l, right: r });
                }
            }
            if pairs.is_empty() {
                return Err("a match interaction with no correct pairs".into());
            }
            let distractors = if sets.len() >= 2 { right.iter().filter(|c| !used.contains(&c.0)).map(|c| c.1.clone()).collect() } else { Vec::new() };
            Ok(Kind::Matching { pairs, distractors })
        }
        "extendedtextinteraction" | "uploadinteraction" | "drawinginteraction" => Ok(Kind::Short { sample: correct.join("\n") }),
        other => Err(format!("unsupported QTI interaction <{other}>")),
    }
}

fn qti2_item(item: XNode<'_, '_>) -> QtiResult {
    let decls = qti2_decls(item);
    let body = kid(item, "itembody").ok_or("no <itemBody>")?;
    let decl = |n: XNode<'_, '_>| xattr(n, "responseIdentifier").and_then(|r| decls.get(r));
    let all: Vec<XNode<'_, '_>> = body.descendants().filter(|d| qname(*d).ends_with("interaction")).collect();
    if all.is_empty() {
        return Ok(None); // an information-only item
    }
    let inline = ["textentryinteraction", "inlinechoiceinteraction", "gapmatchinteraction"];
    let blocks: Vec<XNode<'_, '_>> = all.iter().copied().filter(|n| !inline.contains(&qname(*n).as_str())).collect();
    let multipart = blocks.len() >= 2;
    let mut h = HtmlOut::default();
    let mut stem = h.render(&body.children().filter_map(|c| qti2_h(c, !multipart)).collect::<Vec<_>>());
    let kind = if blocks.is_empty() {
        let mut accept = Vec::new();
        for d in body.descendants() {
            match qname(d).as_str() {
                "textentryinteraction" => {
                    let mut v = decl(d).map(|x| x.correct.iter().chain(&x.mapped).cloned().collect::<Vec<_>>()).unwrap_or_default();
                    dedup(&mut v);
                    accept.push(v);
                }
                "inlinechoiceinteraction" => {
                    let ok = decl(d).map(|x| x.correct.clone()).unwrap_or_default();
                    accept.push(kids(d, "inlinechoice").into_iter().filter(|c| xattr(*c, "identifier").is_some_and(|i| ok.iter().any(|o| o == i))).map(xinline).collect());
                }
                "gap" => {
                    let gid = xattr(d, "identifier").unwrap_or("");
                    let text = d.ancestors().find(|a| qname(*a) == "gapmatchinteraction").and_then(|g| {
                        let gt = decl(g)?.correct.iter().find_map(|p| {
                            let (a, b) = p.split_once(char::is_whitespace)?;
                            (b.trim() == gid).then(|| a.trim().to_string())
                        })?;
                        descs(g, "gaptext").into_iter().find(|t| xattr(*t, "identifier") == Some(gt.as_str())).map(xinline)
                    });
                    accept.push(text.into_iter().collect());
                }
                _ => {}
            }
        }
        if accept.is_empty() || accept.iter().any(Vec::is_empty) {
            return Err("a blank has no correct response".into());
        }
        Kind::Blank { accept }
    } else if multipart {
        let mut parts = Vec::new();
        for b in &blocks {
            let pstem = kid(*b, "prompt").map(|p| HtmlOut::default().render(&p.children().filter_map(|c| qti2_h(c, true)).collect::<Vec<_>>())).unwrap_or_default();
            let kind = match qti2_block(*b, decl(*b))? {
                Kind::Mc { answers } => PartKind::Mc { answers },
                Kind::Tf { correct } => PartKind::Tf { correct },
                Kind::Short { sample } => PartKind::Short { sample },
                _ => return Err("a multi-interaction item may hold only choice and text-response parts".into()),
            };
            parts.push(Part { stem: pstem, kind });
        }
        Kind::Multipart { parts }
    } else {
        qti2_block(blocks[0], decl(blocks[0]))?
    };
    if stem.trim().is_empty() {
        match kind {
            Kind::Matching { .. } => stem = MAT_DEFAULT_STEM.to_string(),
            _ => stem = xattr(item, "title").unwrap_or("").trim().to_string(),
        }
    }
    if stem.is_empty() {
        return Err("empty question text".into());
    }
    let mut q = make_question(stem, kind);
    q.points = kids(item, "outcomedeclaration")
        .into_iter()
        .find(|o| xattr(*o, "identifier") == Some("MAXSCORE"))
        .and_then(|o| desc(o, "value"))
        .and_then(|v| xtext(v).trim().parse::<f64>().ok())
        .filter(|p| p.is_finite() && *p > 0.0);
    Ok(Some((q, h.images)))
}

// ---------- format label (the import screen's source chip) ----------

/// What `detect_and_parse` will read this file as, for the formats added beside the
/// original Blackboard/Aiken/CSV/Test Parrot ones (`None` = one of those, or unknown).
pub fn format_label(data: &[u8], filename: &str) -> Option<&'static str> {
    let ext = ext_of(filename);
    if is_zip(data) {
        let z = Zip::new(std::io::Cursor::new(data)).ok()?;
        let names: Vec<&str> = z.file_names().collect();
        return if names.contains(&"xl/workbook.xml") {
            Some("Excel spreadsheet")
        } else if names.contains(&"word/document.xml") {
            Some("Word document")
        } else {
            Some("QTI / Canvas package")
        };
    }
    let text = decode_text(&data[..data.len().min(1 << 20)]);
    let t = text.trim_start();
    if t.starts_with('<') {
        let head = &t[..t.len().min(4000)];
        if head.contains("<quiz") && head.contains("<question") {
            return Some("Moodle XML");
        }
        if head.contains("questestinterop") || head.contains("assessmentItem") || head.contains("qti-assessment-item") {
            return Some("QTI file");
        }
    }
    if try_csv(&text).is_some() {
        return None;
    }
    if text.to_ascii_lowercase().contains("correct answer") && kahoot_rows(&csv_records(&text).iter().map(|r| split_csv_line(r)).collect::<Vec<_>>()).is_some() {
        return Some("Kahoot sheet");
    }
    if text.lines().any(|l| l.trim_start().to_ascii_lowercase().starts_with("newquestion")) {
        return Some("Brightspace CSV");
    }
    if ext == "gift" || is_gift(&text) {
        return Some("Moodle GIFT");
    }
    if try_numbered(&text, "").is_some() {
        return Some("Respondus / text2qti text");
    }
    if bb_upload_rows(&text).is_some() {
        return Some("Blackboard upload file (tab-delimited)");
    }
    if term_pairs(&text).is_some() {
        return Some("Flashcard list (Quizlet/Anki)");
    }
    None
}

/// The file format's own version, when it states one: a Test Parrot JSON's `formatVersion`
/// (or a legacy `version`), or the QTI generation of a QTI XML file. Shown on the import preview.
pub fn format_version(data: &[u8]) -> Option<String> {
    let text = decode_text(&data[..data.len().min(1 << 22)]);
    let t = text.trim_start();
    if t.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(t).ok()?;
        return match v.get("formatVersion").or_else(|| v.get("version"))? {
            serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        };
    }
    if t.starts_with('<') {
        let head = &t[..t.len().min(4000)];
        if head.contains("qti-assessment-item") {
            return Some("QTI 3.0".to_string());
        }
        if head.contains("assessmentItem") {
            return Some("QTI 2.1".to_string());
        }
        if head.contains("questestinterop") {
            return Some("QTI 1.2".to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    fn parsed(text: &str) -> Vec<Question> {
        parse_blackboard(text).unwrap().questions
    }

    #[test]
    fn mc_star_marks_correct_and_choices_keep_letter_order() {
        let doc = parse_blackboard("MC\nWhat is 2+2?\n*b. 4\na. 3\nc. 5\nd. 6\n").unwrap();
        assert_eq!(doc.name, "Pasted questions");
        let q = &doc.questions[0];
        assert_eq!(q.stem, "What is 2+2?");
        let Kind::Mc { answers } = &q.kind else { panic!("not MC: {:?}", q.kind) };
        assert_eq!(answers.len(), 4);
        assert_eq!(answers[0].text, "4");
        assert!(answers[0].correct);
        assert!(!answers[1].correct);
        // All imported questions carry the default metadata.
        assert_eq!(q.points, None);
        assert_eq!(q.difficulty, None);
        assert!(q.topics.is_empty());
        assert!(q.explanation.is_none());
        assert_eq!(q.created, q.modified);
    }

    #[test]
    fn ma_two_correct() {
        let qs = parsed("MA\nPick every even number.\na. 2\n*b. 4\nc. 5\n*d. 6\n");
        let Kind::Mc { answers } = &qs[0].kind else { panic!("not MC") };
        assert_eq!(answers.iter().filter(|a| a.correct).count(), 2);
        assert_eq!(answers.len(), 4);
        assert_eq!(answers[1].text, "4");
        assert_eq!(answers[3].text, "6");
    }

    #[test]
    fn tf_accepts_word_and_letter_forms() {
        let qs = parsed("TF\nThe sky is blue.\nTRUE\n\nTF\nThe moon is made of cheese.\nFalse\n");
        assert!(matches!(&qs[0].kind, Kind::Tf { correct: true }));
        assert!(matches!(&qs[1].kind, Kind::Tf { correct: false }));
        assert_eq!(qs[1].stem, "The moon is made of cheese.");
    }

    #[test]
    fn ess_collects_sample_answer_lines() {
        let qs = parsed("ESS\nExplain gravity in one sentence.\nMasses attract each other.\nThat is about it.\n");
        let Kind::Short { sample } = &qs[0].kind else { panic!("not Short") };
        assert_eq!(sample, "Masses attract each other.\nThat is about it.");
        // A bare ESS with no sample is fine too.
        parsed("ESS\nExplain gravity.\n");
    }

    #[test]
    fn num_plain_fraction_and_range() {
        let qs = parsed("NUM\nHow many days are in a week?\n7\n\nNUM\nPi to 2 dp\n3.14±0.001\n\nNUM\nWhat is 3/8?\n3/8\n");
        let Kind::Blank { accept } = &qs[0].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["7".to_string()]]);
        let accept = { let Kind::Blank { accept } = &qs[1].kind else { panic!("not Blank") }; accept };
        assert_eq!(accept, &[vec!["3.14".to_string(), "3.139".to_string(), "3.141".to_string()]]);
        let Kind::Blank { accept } = &qs[2].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["3/8".to_string(), "0.375".to_string()]]);
    }

    #[test]
    fn fib_two_blanks_with_alts() {
        let qs = parsed("FIB\nThe capital of ___ sits on a ___ river.\nParis OR paris\nnile\n");
        let Kind::Blank { accept } = &qs[0].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["Paris".to_string(), "paris".to_string()], vec!["nile".to_string()]]);
    }

    #[test]
    fn fib_requires_one_line_per_blank() {
        let err = parse_blackboard("FIB\nTwo ___ here. And ___ too.\nParis\n").expect_err("missing blank");
        assert!(err.contains("___ markers"), "{err}");
    }

    #[test]
    fn mat_alternating_pairs_with_distractors() {
        let qs = parsed("MAT\nCat\nKitten\nDog\nPuppy\n-\nEgg\nBasket\n");
        let Kind::Matching { pairs, distractors } = &qs[0].kind else { panic!("not Matching") };
        assert_eq!(
            serde_json::to_value(pairs).unwrap(),
            serde_json::json!([{"left": "Cat", "right": "Kitten"}, {"left": "Dog", "right": "Puppy"}])
        );
        assert_eq!(distractors, &["Egg".to_string(), "Basket".to_string()]);
        assert_eq!(qs[0].stem, MAT_DEFAULT_STEM);
    }

    #[test]
    fn mat_arrow_lines_are_accepted() {
        let qs = parsed("MAT\nParis -> France\nTokyo -> Japan\n");
        let Kind::Matching { pairs, distractors } = &qs[0].kind else { panic!("not Matching") };
        assert_eq!(pairs.len(), 2);
        assert!(distractors.is_empty());
    }

    #[test]
    fn ord_joins_the_ordered_answers_into_one_blank() {
        let qs = parsed("ORD\nPut these in order by size.\nElephant\nDog\nAnt\n");
        let Kind::Blank { accept } = &qs[0].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["Elephant, Dog, Ant".to_string()]]);
        assert_eq!(qs[0].stem, "Put these in order by size.");
    }

    #[test]
    fn aiken_blocks_without_tags_parse_too() {
        let doc = parse_blackboard("What is the capital of France?\nA. London\n*B. Paris\nC. Berlin\n\nBigger question?\nA. yes\nB. no\n*A. yes\n").unwrap();
        assert_eq!(doc.questions.len(), 2);
        let Kind::Mc { answers } = &doc.questions[0].kind else { panic!("not MC") };
        assert_eq!(answers[1].text, "Paris");
        assert!(answers[1].correct);
        assert_eq!(doc.questions[1].stem, "Bigger question?");
    }

    #[test]
    fn prose_blocks_and_mixed_documents_parse() {
        let qs = parsed("Chapter 3 review\n\nMC\nWhich is odd?\na. 1\nb. 3\nd. 4\n*c. 5\n");
        assert_eq!(qs.len(), 1);
    }

    /// The exact corpus of a live e2e run (Ollama gemma4:31b) that hardened two rules:
    /// the TF answer glued onto the stem line's tail, and an underscore RUN as one blank.
    const CORPUS: &str = "MC\nWhich layer of Earth is directly beneath the crust?\n\
        *A. Mantle\nB. Inner core\nC. Outer core\nD. Lithosphere\n\n\
        TF\nThe Earth's inner core is made of liquid iron. False\n\n\
        FIB\nThe liquid layer of the core is the __________ core.\nouter\n\n\
        MC\nTectonic plates float on the:\nA. Inner core\n*B. Asthenosphere\nC. Mesosphere\nD. Continental crust\n";

    #[test]
    fn corpus_tf_answer_glued_to_the_stem_tail_is_extracted() {
        let qs = parsed(CORPUS);
        assert_eq!(qs.len(), 4);
        let q = &qs[1];
        assert_eq!(q.stem, "The Earth's inner core is made of liquid iron.");
        assert!(matches!(&q.kind, Kind::Tf { correct: false }), "{:?}", q.kind);
    }

    #[test]
    fn corpus_fib_underscore_run_is_exactly_one_blank() {
        let qs = parsed(CORPUS);
        let q = &qs[2];
        // The run of 10 underscores collapses to the canonical single `___` marker.
        assert_eq!(q.stem, "The liquid layer of the core is the ___ core.");
        let Kind::Blank { accept } = &q.kind else { panic!("not Blank: {:?}", q.kind) };
        assert_eq!(accept, &[vec!["outer".to_string()]]);
        // The neighboring questions keep parsing from the same corpus, too.
        let Kind::Mc { answers } = &qs[3].kind else { panic!("not MC") };
        assert!(answers[1].correct);
        assert_eq!(answers[1].text, "Asthenosphere");
    }

    #[test]
    fn blank_or_failure_reports_the_question_number() {
        let err = parse_blackboard("MC\nQ1?\na. 1\n*b. 2\n\nTF\nA broken question with no answer\n\nMC\nQ3?\n*a. 1\n").expect_err("missing T/F");
        assert!(err.starts_with("question 2:"), "{err}");
        assert!(err.contains("T or F"), "{err}");
    }

    #[test]
    fn not_recognized_gives_the_helpful_generic_error() {
        for bad in ["", "hello world", "just some notes\nover two lines"] {
            let err = parse_blackboard(bad).expect_err("garbage rejected");
            assert_eq!(err, GENERIC);
        }
        let err = detect_and_parse(b"hello", "junk.txt").expect_err("garbage rejected");
        assert_eq!(err, GENERIC);
    }

    // ---------- detect_and_parse ----------

    #[test]
    fn detect_runs_blackboard_and_names_the_stem() {
        let doc = detect_and_parse("TF\nThe sky is blue.\nTRUE\n".as_bytes(), "unit-3-test.txt").unwrap();
        assert_eq!(doc.name, "unit-3-test");
        assert!(matches!(&doc.questions[0].kind, Kind::Tf { correct: true }));
    }

    #[test]
    fn detect_decodes_latin1_and_utf8_bom() {
        let doc = detect_and_parse(b"ESS\nWhat is d\xe9j\xe0 vu?\nSeen before.\n", "accents.txt").unwrap();
        assert!(doc.questions[0].stem.contains("déjà"), "{:?}", doc.questions[0].stem);
        let mut bom = b"\xEF\xBB\xBF".to_vec();
        bom.extend(b"TF\nTwo plus two is four.\nTRUE\n");
        assert!(detect_and_parse(&bom, "bom.txt").is_ok());
    }

    #[test]
    fn csv_rows_with_quotes_and_markings() {
        let csv = "\n\
type,stem,points,topic,a,b,c,d,correct
MC,\"What is \"\"2+2\"\"?\",1,math,4,5,6,7,a
,Pick the evens,,math,2,3,4,5,\"a,c\"
TF,The sky is blue,,science,,,,,TRUE
";
        let doc = detect_and_parse(csv.as_bytes(), "quiz-1.csv").unwrap();
        assert_eq!(doc.questions.len(), 3);
        let q = &doc.questions[0];
        assert_eq!(q.stem, "What is \"2+2\"?");
        assert_eq!(q.points, Some(1.0));
        assert_eq!(q.topics, &["math"]);
        let Kind::Mc { answers } = &q.kind else { panic!("not MC") };
        assert_eq!(answers[0].text, "4");
        assert!(answers[0].correct);
        // Empty type defaults to MC; `a,c` marks two correct.
        let Kind::Mc { answers } = &doc.questions[1].kind else { panic!("not MC") };
        assert_eq!(answers.iter().filter(|a| a.correct).map(|a| a.text.clone()).collect::<Vec<_>>(), &["2", "4"]);
        assert!(matches!(&doc.questions[2].kind, Kind::Tf { correct: true }));
    }

    #[test]
    fn csv_correct_text_marks_or_builds_blanks() {
        let csv = "type,stem,correct_text\nTF,Fire is hot.,true\nblank,The Sun rises in the ___.,east\nESS,Explain tides.,Moon gravity\n";
        let doc = detect_and_parse(csv.as_bytes(), "quiz-2.csv").unwrap();
        assert!(matches!(&doc.questions[0].kind, Kind::Tf { correct: true }));
        assert_eq!(doc.questions[1].topics, Vec::<String>::new());
        assert!(matches!(&doc.questions[1].kind, Kind::Blank { accept: a } if a == &vec![vec!["east".to_string()]]));
        assert!(matches!(&doc.questions[2].kind, Kind::Short { sample } if sample == "Moon gravity"));
    }

    #[test]
    fn json_bank_round_trips() {
        let q = make_question("Stem".into(), Kind::Tf { correct: true });
        let bank = serde_json::json!({
            "kind": LEGACY_KIND, "version": 1, "name": "Chapter 1", "questions": [q],
        });
        let data = serde_json::to_vec(&bank).unwrap();
        let doc = detect_and_parse(&data, "chapter-1.json").unwrap();
        assert_eq!(doc.name, "Chapter 1");
        assert!(matches!(&doc.questions[0].kind, Kind::Tf { correct: true }));
        assert_ne!(doc.questions[0].id, "regenerated-ids-are-required"); // ids are fresh
        // Pasted JSON with no filename keeps the bank's own name.
        let pasted = detect_and_parse(&data, "").unwrap();
        assert_eq!(pasted.name, "Chapter 1");
    }

    #[test]
    fn json_claimed_even_with_a_paste_filename_when_kind_matches() {
        let bank = serde_json::json!({"kind": LEGACY_KIND, "questions": []});
        let err = detect_and_parse(serde_json::to_vec(&bank).unwrap().as_slice(), "notes.txt").expect_err("empty bank");
        assert!(err.contains("no questions"), "{err}");
    }

    // ---------- Test Parrot package / record (.parrot.json) ----------

    /// A minimal but schema-shaped package: one bank (MC + fill-in-blank), one exam whose
    /// positions land q2 then q1 and skip an unresolvable q9.
    fn package_json() -> serde_json::Value {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let para = |t: &str| serde_json::json!({"type": "paragraph", "content": [text(t)]});
        let doc = move |t: &str| serde_json::json!({"type": "document", "content": [para(t)]});
        serde_json::json!({
            "format": "test-parrot/package", "formatVersion": "0.1.0",
            "generator": {"name": "testparrot-test", "version": "1"}, "requiredFeatures": [],
            "questionBanks": [{"id": "b1", "record": {
                "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
                "generator": {"name": "testparrot-test", "version": "1"}, "requiredFeatures": [],
                "bank": {"name": "Periodic table", "questions": [
                    {"id": "q1", "type": "multiple-choice", "stem": doc("Which gas is noble?"), "choices": [
                        {"id": "q1-c1", "content": doc("Helium"), "correct": true},
                        {"id": "q1-c2", "content": doc("Oxygen"), "correct": false},
                    ]},
                    {"id": "q2", "type": "fill-in-the-blank", "difficulty": "hard", "topics": ["atoms"], "points": 3,
                     "stem": {"type": "document", "content": [{"type": "paragraph", "content": [
                        {"type": "text", "text": "Helium has "}, {"type": "blank", "content": [text("4")]},
                        {"type": "text", "text": " neutrons; hydrogen "}, {"type": "blank", "content": [text("0")]}, {"type": "text", "text": "."}
                     ]}]},
                    }
                ]},
                "media": []
            }}],
            "exams": [{"format": "test-parrot/exam", "formatVersion": "0.5.0", "name": "Atoms test",
                       "sections": [{"title": "Part A", "instructions": "Answer all."}],
                       "positions": [
                           {"question": {"bank": "b1", "question": "q2"}, "section": 0, "columns": 2},
                           {"question": {"bank": "b1", "question": "q1"}, "section": 0},
                           {"question": {"bank": "b1", "question": "q9"}, "section": 0},
                       ]}]
        })
    }

    fn bytes_of(v: &serde_json::Value) -> Vec<u8> { serde_json::to_vec(v).unwrap() }

    #[test]
    fn package_maps_questions_and_best_effort_exam() {
        let data = bytes_of(&package_json());
        let doc = detect_and_parse(&data, "atoms.parrot.json").unwrap();
        assert_eq!(doc.name, "Periodic table");
        assert_eq!(doc.questions.len(), 2);
        let q1 = &doc.questions[0];
        let Kind::Mc { answers } = &q1.kind else { panic!("not MC: {:?}", q1.kind) };
        assert_eq!(answers[0].text, "Helium");
        assert!(answers[0].correct);
        assert!(!answers[1].correct);
        let q2 = &doc.questions[1];
        assert_eq!(q2.stem, "Helium has ___ neutrons; hydrogen ___.");
        let Kind::Blank { accept } = &q2.kind else { panic!("not Blank: {:?}", q2.kind) };
        assert_eq!(accept, &[vec!["4".to_string()], vec!["0".to_string()]]);
        assert_eq!(q2.points, Some(3.0));
        assert_eq!(q2.difficulty, Some(Difficulty::High));
        assert_eq!(q2.topics, &["atoms"]);
        // Exam positions resolve in their own order, unresolvable ones skipped.
        assert_eq!(doc.exams.len(), 1);
        let e = &doc.exams[0];
        assert_eq!(e.name, "Atoms test");
        assert_eq!(e.items.len(), 2);
        assert_eq!(e.items[0].question_id, q2.id);
        assert_eq!(e.items[1].question_id, q1.id);
        assert!(e.items.iter().all(|it| it.bank_id.is_empty()));
        assert_eq!(e.header, DEFAULT_HEADER);
    }

    #[test]
    fn record_matching_pairs_and_distractors() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let doc = move |t: &str| serde_json::json!({"type": "document", "content": [{"type": "paragraph", "content": [text(t)]}]});
        let rec = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Match practice", "questions": [{"id": "q1", "type": "matching",
                "stem": doc("Match the animals."),
                "prompts": [
                    {"id": "q1-p1", "content": doc("Cat"), "answer": "q1-a2"},
                    {"id": "q1-p2", "content": doc("Dog"), "answer": "q1-a1"},
                ],
                "wordBank": [
                    {"id": "q1-a1", "content": doc("Puppy")},
                    {"id": "q1-a2", "content": doc("Kitten")},
                    {"id": "q1-a3", "content": doc("Egg")},
                ]}]},
            "media": [],
        });
        let parsed_doc = detect_and_parse(&bytes_of(&rec), "match.json").unwrap();
        assert_eq!(parsed_doc.name, "Match practice");
        let q = &parsed_doc.questions[0];
        let Kind::Matching { pairs, distractors } = &q.kind else { panic!("not Matching: {:?}", q.kind) };
        assert_eq!(pairs[0].left, "Cat");
        assert_eq!(pairs[0].right, "Kitten");
        assert_eq!(pairs[1].left, "Dog");
        assert_eq!(pairs[1].right, "Puppy");
        assert_eq!(distractors, &["Egg".to_string()]);
        // Unlike Blackboard text, a record keeps the authored stem.
        assert_eq!(q.stem, "Match the animals.");
    }

    #[test]
    fn record_multipart_flattens_subparts() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let para_doc = move |t: &str| serde_json::json!({"type": "document", "content": [{"type": "paragraph", "content": [text(t)]}]});
        let rec = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Big questions", "questions": [{"id": "q1", "type": "multipart",
                "stem": para_doc("Solve these."),
                "parts": [
                    {"id": "q1-s1", "type": "multiple-choice", "stem": para_doc("(a) Pick one."), "choices": [
                        {"id": "q1-s1-c1", "content": para_doc("Red"), "correct": false},
                        {"id": "q1-s1-c2", "content": para_doc("Blue"), "correct": true},
                    ]},
                    {"id": "q1-s2", "stem": para_doc("(b) Do both parts."),
                     "subparts": [
                        {"id": "q1-s2-s1", "type": "multiple-choice", "stem": para_doc("(i) Yes or no."), "choices": [
                            {"id": "q1-s2-s1-c1", "content": para_doc("Yes"), "correct": true},
                            {"id": "q1-s2-s1-c2", "content": para_doc("No"), "correct": false},
                        ]},
                        {"id": "q1-s2-s2", "type": "short-answer", "stem": para_doc("(ii) Why?"), "suggestedAnswer": para_doc("Because physics.")},
                     ]},
                ]}]},
            "media": [],
        });
        let parsed_doc = detect_and_parse(&bytes_of(&rec), "multi.json").unwrap();
        let q = &parsed_doc.questions[0];
        let Kind::Multipart { parts } = &q.kind else { panic!("not Multipart: {:?}", q.kind) };
        assert_eq!(parts.len(), 4); // (a), lead-in, (i), (ii)
        assert!(matches!(&parts[0].kind, PartKind::Mc { .. }));
        assert!(matches!(&parts[1].kind, PartKind::Short { .. }));
        assert!(matches!(&parts[2].kind, PartKind::Mc { .. }));
        assert_eq!(parts[0].stem, "(a) Pick one.");
        assert_eq!(parts[1].stem, "(b) Do both parts.");
        assert_eq!(parts[3].stem, "(ii) Why?");
        if let PartKind::Short { sample } = &parts[3].kind {
            assert_eq!(sample, "Because physics.");
        } else {
            panic!("not Short");
        }
    }

    #[test]
    fn record_short_answer_tf_and_metadata() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let doc = move |t: &str| serde_json::json!({"type": "document", "content": [{"type": "paragraph", "content": [text(t)]}]});
        let rec = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Space", "questions": [
                {"id": "q1", "type": "true-false", "stem": doc("Space is empty."), "choices": [
                    {"id": "q1-c1", "content": doc("True"), "correct": true},
                    {"id": "q1-c2", "content": doc("False"), "correct": false},
                ]},
                {"id": "q2", "type": "short-answer", "difficulty": "easy", "points": 2, "topics": ["gravity"], "stem": doc("What pulls tides?"), "suggestedAnswer": doc("Moon gravity")},
            ]},
            "media": [],
        });
        let parsed_doc = detect_and_parse(&bytes_of(&rec), "space.json").unwrap();
        assert!(matches!(&parsed_doc.questions[0].kind, Kind::Tf { correct: true }));
        let q = &parsed_doc.questions[1];
        assert!(matches!(&q.kind, Kind::Short { sample } if sample == "Moon gravity"));
        assert_eq!(q.difficulty, Some(Difficulty::Low));
        assert_eq!(q.points, Some(2.0));
        assert_eq!(q.topics, &["gravity"]);
    }

    #[test]
    fn record_errors_name_the_question() {
        let no_flag = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Space", "questions": [
                {"id": "q1", "type": "true-false", "stem": {"type": "document", "content": []}, "choices": [
                    {"id": "q1-c1", "content": {"type": "document", "content": []}, "correct": false},
                    {"id": "q1-c2", "content": {"type": "document", "content": []}, "correct": false},
                ]}
            ]},
            "media": [],
        });
        let err = detect_and_parse(&bytes_of(&no_flag), "space.json").expect_err("unmarked pair");
        assert!(err.starts_with("question q1:"), "{err}");
        // Unsupported types name themselves too.
        let bad = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Space", "questions": [{"id": "q2", "type": "poll", "stem": {"type": "document", "content": []}}]},
            "media": [],
        });
        let err = detect_and_parse(&bytes_of(&bad), "space.json").expect_err("bad type");
        assert!(err.starts_with("question q2:"), "{err}");
    }

    #[test]
    fn package_claimed_even_when_the_json_is_broken() {
        let err = detect_and_parse(b"{ \"format\": \"test-parrot/package\", \"questionBanks\":", "b.parrot.json").expect_err("malformed");
        assert!(err.contains("not valid JSON"), "{err}");
        // A non-json file that merely says the format name in prose is not test-parrot.
        // Blackboard text that merely mentions packages still parses; 1 question.
        let doc = detect_and_parse(b"plain text about packages\n\nMC\nQ?\na. 1\n*b. 2\n", "notes.txt").unwrap();
        assert_eq!(doc.questions.len(), 1);
        let err = detect_and_parse(b"notes\n\"test-parrot/package\" in prose but broken json", "r.txt").expect_err("marker but junk");
        assert!(err.contains("not valid JSON"), "{err}");
    }

    #[test]
    fn base64_decode_matches_vectors() {
        assert_eq!(base64_decode(""), Some(Vec::<u8>::new()));
        assert_eq!(base64_decode("Zg=="), Some(b"f".to_vec()));
        assert_eq!(base64_decode("Zm8="), Some(b"fo".to_vec()));
        assert_eq!(base64_decode("aGVsbG8="), Some(b"hello".to_vec()));
        assert_eq!(base64_decode("Zm9vYmFy"), Some(b"foobar".to_vec()));
        assert_eq!(base64_decode("Z m 9 v"), Some(b"foo".to_vec())); // interior whitespace tolerated
        assert_eq!(base64_decode("Z!!="), None);
        assert_eq!(base64_decode("Z"), None); // a stranded single symbol
    }

    // ---------- stem markup from imports ----------

    fn tp_stem(content: serde_json::Value) -> String {
        flatten_doc(&serde_json::json!({"type": "document", "content": content})).text
    }

    #[test]
    fn package_table_becomes_a_pipe_table_with_header_and_subscripts() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let sub = |t: &str| serde_json::json!({"type": "text", "text": t, "marks": [{"type": "subscript"}]});
        let cell = |h: bool, c: Vec<serde_json::Value>| serde_json::json!({"type": "table-cell", "header": h, "content": [{"type": "paragraph", "content": c}]});
        let stem = tp_stem(serde_json::json!([
            {"type": "paragraph", "content": [text("Name the following compounds.")]},
            {"type": "table", "content": [
                {"type": "table-row", "header": true, "content": [cell(true, vec![text("Formula")]), cell(true, vec![text("Ionic or Covalent?")]), cell(true, vec![text("Name")])]},
                {"type": "table-row", "content": [cell(false, vec![text("Mg(NO"), sub("2"), text(")"), sub("2")]), cell(false, vec![text("Ionic")]), cell(false, vec![text("magnesium nitrite")])]},
                {"type": "table-row", "content": [cell(false, vec![text("SO"), sub("3")]), cell(false, vec![text("Covalent")]), cell(false, vec![text("sulfur trioxide")])]},
            ]},
        ]));
        assert_eq!(
            stem,
            "Name the following compounds.\n\nFormula | Ionic or Covalent? | Name\n---|---|---\nMg(NO₂)₂ | Ionic | magnesium nitrite\nSO₃ | Covalent | sulfur trioxide"
        );
        let blocks = crate::richtext::parse(&stem);
        assert!(matches!(&blocks[1], crate::richtext::Block::Table { header: true, rows } if rows.len() == 3 && rows[0].len() == 3), "{blocks:?}");
    }

    #[test]
    fn package_lists_marks_quotes_and_breaks_become_markup() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let marked = |t: &str, m: &str| serde_json::json!({"type": "text", "text": t, "marks": [{"type": m}]});
        let item = |c: Vec<serde_json::Value>| serde_json::json!({"type": "list-item", "content": [{"type": "paragraph", "content": c}]});
        let stem = tp_stem(serde_json::json!([
            {"type": "paragraph", "content": [text("Do all:"), {"type": "hard-break"}, text("then "), marked("stop ", "em"), text("here.")]},
            {"type": "paragraph", "content": []},
            {"type": "bullet-list", "content": [item(vec![text("Draw it.")]), item(vec![text("Is it "), marked("polar", "strong"), text("?")])]},
            {"type": "ordered-list", "content": [item(vec![text("first")]), item(vec![text("x"), marked("2", "superscript"), text(" and e"), marked("ab", "superscript")])]},
            {"type": "blockquote", "content": [{"type": "paragraph", "content": [text("quoted")]}]},
            {"type": "paragraph", "content": [text("Solve "), {"type": "inline-math", "source": "x^2=4"}, text(".")]},
        ]));
        assert_eq!(
            stem,
            "Do all:\nthen *stop* here.\n\n• Draw it.\n• Is it **polar**?\n\n1. first\n2. x² and e^{ab}\n\n> quoted\n\nSolve $x^2=4$."
        );
        let b = crate::richtext::parse(&stem);
        use crate::richtext::Block;
        assert!(matches!(&b[0], Block::Para(l) if l.len() == 2));
        assert!(matches!(&b[1], Block::Bullets(l) if l.len() == 2));
        assert!(matches!(&b[2], Block::Numbered(l) if l.len() == 2));
        assert!(matches!(&b[3], Block::Quote(_)));
        assert!(b.iter().any(|blk| matches!(blk, Block::Bullets(l) if l[1].iter().any(|s| s.bold && s.text == "polar"))));
        assert!(matches!(&b[0], Block::Para(l) if l[1].iter().any(|s| s.italic && s.text == "stop")));
    }

    #[test]
    fn package_table_without_header_and_with_empty_edge_cells() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let cell = |t: &str| serde_json::json!({"type": "table-cell", "content": [{"type": "paragraph", "content": if t.is_empty() { serde_json::json!([]) } else { serde_json::json!([text(t)]) }}]});
        let stem = tp_stem(serde_json::json!([{"type": "table", "content": [
            {"type": "table-row", "content": [cell("a"), cell("b")]},
            {"type": "table-row", "content": [cell("c"), cell("")]},
        ]}]));
        assert_eq!(stem, "| a | b |\n| c |  |");
        assert!(matches!(&crate::richtext::parse(&stem)[0], crate::richtext::Block::Table { header: false, rows } if rows.len() == 2 && rows[1].len() == 2));
    }

    #[test]
    fn real_ch4_package_imports_its_compound_table_as_a_table() {
        let Some(home) = std::env::var_os("HOME") else { return };
        let path = std::path::Path::new(&home).join("Downloads/ch4-quiz-5a-5b.parrot.json");
        let Ok(data) = std::fs::read(&path) else { return }; // fixture only on the author's box
        let doc = detect_and_parse(&data, "ch4-quiz-5a-5b.parrot.json").unwrap();
        let tables: Vec<&Question> = doc.questions.iter().filter(|q| q.stem.contains("Ionic or Covalent?")).collect();
        assert!(!tables.is_empty());
        for q in tables {
            let blocks = crate::richtext::parse(&q.stem);
            assert!(
                blocks.iter().any(|b| matches!(b, crate::richtext::Block::Table { header: true, rows } if rows.len() >= 3 && rows[0].len() == 3)),
                "{:?}",
                q.stem
            );
        }
        let bullets = doc.questions.iter().find(|q| q.stem.contains("Draw a valid Lewis structure")).unwrap();
        assert!(bullets.stem.contains("• Draw a valid Lewis structure."), "{:?}", bullets.stem);
        assert!(bullets.stem.contains("**polar**"), "{:?}", bullets.stem);
    }

    #[test]
    fn blackboard_and_aiken_multi_line_stems_keep_their_line_breaks() {
        let q = parsed("MC\nFor each compound:\n• Draw it.\n• Name it.\n*a. ok\nb. no\n");
        assert_eq!(q[0].stem, "For each compound:\n• Draw it.\n• Name it.");
        let q = parsed("TF\nRead this.\nThe sky is blue.\nT\n");
        assert_eq!(q[0].stem, "Read this.\nThe sky is blue.");
        let q = parsed("TF\nRead this.\nThe sky is green. False\n");
        assert_eq!(q[0].stem, "Read this.\nThe sky is green.");
        assert!(matches!(q[0].kind, Kind::Tf { correct: false }));
        let q = parsed("NUM\nA cup holds\n8 ounces. How many mL?\n236.6\n");
        assert_eq!(q[0].stem, "A cup holds\n8 ounces. How many mL?");
        let q = parsed("Which gas?\nPick one:\na. O2\n*b. CO2\n");
        assert_eq!(q[0].stem, "Which gas?\nPick one:");
    }

    #[test]
    fn csv_quoted_cells_may_span_lines() {
        let csv = "type,stem,a,b,correct\r\nmc,\"Pick one:\r\n• the gas\r\n• the \"\"best\"\" one\",O2,CO2,b\nmc,Plain?,x,y,a\n";
        let qs = try_csv(csv).unwrap().unwrap();
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[0].stem, "Pick one:\n• the gas\n• the \"best\" one");
        assert_eq!(qs[1].stem, "Plain?");
        let Kind::Mc { answers } = &qs[0].kind else { panic!() };
        assert!(answers[1].correct);
    }
}
/// Tests for formats 0 and 3-10 (GIFT, flashcards, Excel, Kahoot, Brightspace, Moodle XML,
/// QTI files and zips, Respondus, text2qti) against the fixtures in `tests/import_fixtures/`.
#[cfg(test)]
mod import_format_tests {
    use super::*;
    use crate::model::Kind;

    macro_rules! fixture {
        ($f:literal) => {
            include_bytes!(concat!("../tests/import_fixtures/", $f)).as_slice()
        };
    }

    fn doc(bytes: &[u8], name: &str) -> ImportDoc {
        detect_and_parse(bytes, name).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    fn mc(q: &Question) -> Vec<(String, bool)> {
        let Kind::Mc { answers } = &q.kind else { panic!("not MC: {:?}", q.kind) };
        answers.iter().map(|a| (a.text.clone(), a.correct)).collect()
    }

    fn blank(q: &Question) -> Vec<Vec<String>> {
        let Kind::Blank { accept } = &q.kind else { panic!("not Blank: {:?}", q.kind) };
        accept.clone()
    }

    fn pairs(q: &Question) -> (Vec<(String, String)>, Vec<String>) {
        let Kind::Matching { pairs, distractors } = &q.kind else { panic!("not Matching: {:?}", q.kind) };
        (pairs.iter().map(|p| (p.left.clone(), p.right.clone())).collect(), distractors.clone())
    }

    fn sv(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn kinds(d: &ImportDoc) -> Vec<&'static str> {
        d.questions.iter().map(|q| q.kind.qkind().label()).collect()
    }

    // ---------- GIFT ----------

    #[test]
    fn gift_fixture_maps_every_question_type() {
        let d = doc(fixture!("moodle_sample.gift"), "moodle_sample.gift");
        assert_eq!(d.name, "moodle_sample");
        let q = &d.questions;
        assert_eq!(q.len(), 14, "{:#?}", q.iter().map(|q| &q.stem).collect::<Vec<_>>());
        assert_eq!(q[0].stem, "Which gas makes up most of Earth's atmosphere?");
        assert_eq!(mc(&q[0]), vec![("Nitrogen".into(), true), ("Oxygen".into(), false), ("Carbon dioxide".into(), false), ("Argon".into(), false)]);
        assert_eq!(q[0].topics, sv(&["Unit 3 Chemistry"]));
        // ~%50% weights make a multi-answer; negative weights stay wrong.
        assert_eq!(mc(&q[1]).iter().filter(|a| a.1).count(), 2);
        assert!(matches!(q[2].kind, Kind::Tf { correct: true }));
        assert!(matches!(q[3].kind, Kind::Tf { correct: false }));
        assert_eq!(blank(&q[4]), vec![sv(&["Shakespeare", "William Shakespeare", "Bill"])]);
        assert_eq!(q[5].stem, "The chemical symbol for sodium is ___ on the periodic table.");
        assert_eq!(blank(&q[5]), vec![sv(&["Na"])]);
        assert_eq!(blank(&q[6]), vec![sv(&["3.14", "3.135", "3.145"])]);
        assert_eq!(blank(&q[7]), vec![sv(&["3", "1", "5"])]);
        assert_eq!(blank(&q[8]), vec![sv(&["1822", "1820", "1824"])]);
        let (p, dis) = pairs(&q[9]);
        assert_eq!(p[2], ("Japan".into(), "Tokyo".into()));
        assert_eq!(dis, sv(&["Lima"]));
        assert!(matches!(&q[10].kind, Kind::Short { sample } if sample.is_empty()));
        assert_eq!(q[11].stem, "In GIFT, what does = mean inside { braces }?");
        assert_eq!(q[11].explanation.as_deref(), Some("A backslash escapes GIFT's special characters: ~ = # { }."));
        assert_eq!(q[12].stem, "Water is H₂O and it is **essential**.\n\nWhich state of water is ice?");
        assert_eq!(q[13].stem, "What is 2 + 2?");
        assert_eq!(q[13].topics, sv(&["Unit 4"]));
    }

    #[test]
    fn gift_detection_never_steals_blackboard_text_or_stem_markup() {
        assert!(!is_gift("MC\nWhat is H_{2}O?\n*a. water\nb. salt\n"));
        assert!(!is_gift("Water is H_{2}O and x^{2} grows.\nA. yes\n*B. no\n"));
        assert!(!is_gift("TF\nSets like {T} exist.\nT\n"), "a tagged Blackboard block is never GIFT");
        assert!(is_gift("Is the sky blue?{T}"));
        assert!(is_gift("::Q1:: Explain.{}"));
        assert!(is_gift("2+2 = {=4 ~5}"));
        // A pasted GIFT question (no filename) is detected too.
        let d = doc(b"// comment\nWho? {=me ~you}\n", "");
        assert_eq!(d.name, "Pasted questions");
        assert_eq!(mc(&d.questions[0]), vec![("me".into(), true), ("you".into(), false)]);
    }

    #[test]
    fn gift_blank_line_inside_a_block_and_escapes_and_newlines() {
        let d = parse_gift("Pick one {\n=a\n\n~b\n}\n\nLine one\\nline two {T}\n\nTeX \\neq stays {F}\n", "g").unwrap();
        assert_eq!(d.questions.len(), 3);
        assert_eq!(mc(&d.questions[0]).len(), 2);
        assert_eq!(d.questions[1].stem, "Line one\nline two");
        assert_eq!(d.questions[2].stem, "TeX \\neq stays");
    }

    #[test]
    fn gift_errors_name_the_question() {
        let e = parse_gift("Q1 {=a ~b}\n\nQ2 has no closing {=a ~b\n", "g").unwrap_err();
        assert!(e.starts_with("GIFT question 2:") && e.contains("closing }"), "{e}");
        let e = parse_gift("Only wrong answers {~a ~b}\n", "g").unwrap_err();
        assert!(e.contains("no correct answer"), "{e}");
        let e = parse_gift("Just prose, no blocks.\n", "g").unwrap_err();
        assert!(e.contains("answer block"), "{e}");
        let e = parse_gift("Number? {#abc}\n", "g").unwrap_err();
        assert!(e.contains("not a number"), "{e}");
    }

    // ---------- flashcard lists ----------

    #[test]
    fn quizlet_tab_list_becomes_one_matching_question() {
        let d = doc(fixture!("quizlet_terms.txt"), "quizlet_terms.txt");
        assert_eq!(d.questions.len(), 1);
        assert_eq!(d.questions[0].stem, TERM_MATCH_STEM);
        let (p, dis) = pairs(&d.questions[0]);
        assert_eq!(p.len(), 5);
        assert_eq!(p[1], ("mitochondria".into(), "the powerhouse of the cell".into()));
        assert!(dis.is_empty());
    }

    #[test]
    fn anki_qa_deck_becomes_short_questions_with_html_converted() {
        let d = doc(fixture!("anki_notes.txt"), "anki_notes.txt");
        assert_eq!(kinds(&d), vec!["Short answer"; 3]);
        assert_eq!(d.questions[1].stem, "What is **2 + 2**?");
        assert!(matches!(&d.questions[1].kind, Kind::Short { sample } if sample == "4"), "tags column ignored");
    }

    #[test]
    fn two_column_csv_skips_its_header_and_honors_quotes() {
        let d = doc(fixture!("vocab_list.csv"), "vocab_list.csv");
        let (p, _) = pairs(&d.questions[0]);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0].0, "atom");
        assert_eq!(p[2].1, "an atom with a charge, positive or negative");
    }

    #[test]
    fn long_lists_split_evenly_short_lists_and_dashes() {
        let text: String = (1..=11).map(|i| format!("term{i}\tdef{i}\n")).collect();
        let d = doc(text.as_bytes(), "t.txt");
        let sizes: Vec<usize> = d.questions.iter().map(|q| pairs(q).0.len()).collect();
        assert_eq!(sizes, vec![6, 5]);
        let d = doc(b"cat - a small feline\ndog - a loyal canine\n", "d.txt");
        assert_eq!(kinds(&d), vec!["Short answer"; 2], "fewer than 3 cards are Short questions");
        assert_eq!(d.questions[0].stem, "cat");
        let d = doc(b"a\tb;c\td;e\tf", "q.txt");
        assert_eq!(pairs(&d.questions[0]).0.len(), 3, "Quizlet's one-line ; rows");
        // Prose stays unrecognized.
        assert_eq!(detect_and_parse(b"hello world", "junk.txt").unwrap_err(), GENERIC);
        assert!(term_pairs("only one line, with a comma").is_none());
    }

    // ---------- Respondus / text2qti / Aiken ----------

    #[test]
    fn respondus_fixture_covers_its_types() {
        let d = doc(fixture!("respondus.txt"), "respondus.txt");
        assert_eq!(kinds(&d), vec!["Multiple choice", "Multiple choice", "True/False", "Short answer", "Fill in the blank", "Matching", "Fill in the blank", "Multiple choice"]);
        let q = &d.questions;
        assert_eq!(q[0].stem, "Who determined the exact speed of light?");
        assert_eq!(q[0].points, Some(2.0));
        assert_eq!(mc(&q[1]).iter().filter(|a| a.1).count(), 3);
        assert!(matches!(&q[3].kind, Kind::Short { sample } if sample.starts_with("The scientific study")));
        assert_eq!(blank(&q[4]), vec![sv(&["Balboa", "Vasco Nunez de Balboa"])]);
        assert_eq!(pairs(&q[5]).0[2], ("Cow".into(), "Calf".into()));
        assert_eq!(blank(&q[6]), vec![sv(&["Mercury, Venus, Earth"])]);
        assert_eq!(mc(&q[7])[1], ("Jupiter".into(), true), "Answer: b marks the key");
        assert_eq!(q[7].points, None, "Points: applies to its own question only");
    }

    #[test]
    fn text2qti_fixture_covers_its_types() {
        let d = doc(fixture!("text2qti.md"), "text2qti.md");
        assert_eq!(d.name, "Unit 5 review");
        let q = &d.questions;
        assert_eq!(q.len(), 7);
        assert_eq!(mc(&q[0]), vec![("6".into(), false), ("1".into(), false), ("5".into(), true)]);
        assert_eq!(q[0].explanation.as_deref(), Some("Count on your fingers."));
        assert_eq!(q[0].points, Some(2.0));
        assert_eq!(mc(&q[1]).iter().filter(|a| a.1).count(), 2);
        assert_eq!(blank(&q[2]), vec![sv(&["five", "Five"])]);
        assert_eq!(blank(&q[3]), vec![sv(&["1.4", "1.35", "1.45"])]);
        assert_eq!(blank(&q[4]), vec![sv(&["2.5", "2", "3"])]);
        assert!(matches!(q[5].kind, Kind::Short { .. }) && matches!(q[6].kind, Kind::Short { .. }));
    }

    #[test]
    fn aiken_answer_lines_mark_the_key() {
        let d = doc(fixture!("aiken.txt"), "aiken.txt");
        assert_eq!(d.questions.len(), 2);
        assert_eq!(mc(&d.questions[0])[3], ("Must be this one!".into(), true));
        assert_eq!(mc(&d.questions[1])[0], ("Moodle".into(), true));
        let e = parse_blackboard("Q?\nA. x\nB. y\nANSWER: E\n").unwrap_err();
        assert!(e.contains("ANSWER line names choice 5"), "{e}");
    }

    #[test]
    fn numbered_text_rules() {
        // Plain numbered MC blocks: the number is no longer part of the stem.
        let d = doc(b"1. First?\n*a. yes\nb. no\n\n2) Second?\na) x\n*b) y\n", "n.txt");
        assert_eq!(d.questions[0].stem, "First?");
        assert_eq!(d.questions[1].stem, "Second?");
        // Blackboard-tagged text is never taken for Respondus.
        assert!(try_numbered("MC\n1. Q?\n*a. x\nb. y\n", "x").is_none());
        let e = doc_err(b"Type: FMB\n1) The [a] is [b].\na. x\n");
        assert!(e.contains("Type: FMB is not supported"), "{e}");
        let e = doc_err(b"Points: 1\n1) No answers here.\n");
        assert!(e.contains("question 1: no answers"), "{e}");
    }

    fn doc_err(bytes: &[u8]) -> String {
        detect_and_parse(bytes, "x.txt").expect_err("should fail")
    }

    // ---------- Brightspace, Kahoot ----------

    #[test]
    fn brightspace_csv_fixture() {
        let d = doc(fixture!("brightspace.csv"), "brightspace.csv");
        assert_eq!(kinds(&d), vec!["Multiple choice", "True/False", "Multiple choice", "Fill in the blank", "Short answer", "Matching", "Fill in the blank", "Fill in the blank"]);
        let q = &d.questions;
        assert_eq!(mc(&q[0])[1], ("6".into(), true));
        assert_eq!(q[0].points, Some(2.0));
        assert_eq!(q[0].explanation.as_deref(), Some("Carbon has six protons."));
        assert!(matches!(q[1].kind, Kind::Tf { correct: false }));
        assert_eq!(pairs(&q[5]).1, sv(&["Sn"]));
        assert_eq!(q[7].stem, "The lightest element is ___.");
        assert_eq!(blank(&q[7]), vec![sv(&["Hydrogen"])]);
    }

    #[test]
    fn kahoot_template_as_csv_and_xlsx() {
        let csv = "Quiz template,,,,,,,\n,Question - max 120 characters,Answer 1 - max 75 characters,Answer 2 - max 75 characters,Answer 3,Answer 4,Time limit (sec),Correct answer(s) - choose at least one\n1,Pick two,A,B,C,,20,\"1,3\"\n";
        let d = doc(csv.as_bytes(), "kahoot.csv");
        assert_eq!(mc(&d.questions[0]), vec![("A".into(), true), ("B".into(), false), ("C".into(), true)]);
        let d = doc(fixture!("kahoot.xlsx"), "kahoot.xlsx");
        assert_eq!(kinds(&d), vec!["Multiple choice", "Multiple choice", "True/False"]);
        assert_eq!(mc(&d.questions[0])[1], ("Blue".into(), true));
        assert_eq!(mc(&d.questions[1]).iter().filter(|a| a.1).count(), 2);
    }

    // ---------- Excel ----------

    #[test]
    fn xlsx_uses_the_csv_grammar_on_the_first_sheet_that_reads() {
        let d = doc(fixture!("questions.xlsx"), "questions.xlsx");
        assert_eq!(d.name, "questions");
        assert_eq!(kinds(&d), vec!["Multiple choice", "Multiple choice", "True/False", "Short answer", "Fill in the blank", "Multiple choice"]);
        let q = &d.questions;
        assert_eq!(mc(&q[0])[1], ("Jupiter".into(), true));
        assert_eq!(q[0].points, Some(2.0));
        assert_eq!(q[1].points, Some(1.5));
        assert_eq!(q[1].topics, sv(&["astronomy", "planets"]));
        assert_eq!(q[2].stem, "The Moon has no atmosphere to speak of.", "inline string cell");
        assert_eq!(blank(&q[4]), vec![sv(&["8"])], "numeric cell");
        assert_eq!(mc(&q[5])[0], ("red".into(), true), "empty type defaults to MC, digit key");
    }

    #[test]
    fn xlsx_two_column_vocab_and_bad_sheets() {
        let d = doc(fixture!("vocab.xlsx"), "vocab.xlsx");
        assert_eq!(pairs(&d.questions[0]).0[1], ("condensation".into(), "gas turning into a liquid".into()));
        let e = rows_to_doc(vec![vec!["just".into()], vec!["notes".into()]], "x").unwrap_err();
        assert!(e.contains("header naming `type` and `stem`"), "{e}");
        let e = rows_to_doc(vec![sv(&["type", "stem"]), sv(&["mc", "No answers?"])], "x").unwrap_err();
        assert!(e.starts_with("CSV row 1:"), "CSV rules and errors apply: {e}");
        assert_eq!(xlsx_col("A1"), Some(0));
        assert_eq!(xlsx_col("AB12"), Some(27));
    }

    // ---------- Moodle XML ----------

    #[test]
    fn moodle_xml_fixture_maps_types_categories_and_html() {
        let d = doc(fixture!("moodle_quiz.xml"), "moodle_quiz.xml");
        let q = &d.questions;
        assert_eq!(
            kinds(&d),
            vec![
                "Multiple choice", "Multiple choice", "True/False", "Fill in the blank", "Fill in the blank", "Fill in the blank", "Matching", "Short answer",
                "Fill in the blank", "Fill in the blank", "Multiple choice",
            ],
            "description and calculated are skipped"
        );
        assert_eq!(q[0].stem, "Which organelle is the **powerhouse** of the cell?\n\n[picture: A cell diagram]");
        assert_eq!(mc(&q[0])[0], ("Mitochondrion".into(), true));
        assert_eq!(q[0].points, Some(2.0));
        assert_eq!(q[0].explanation.as_deref(), Some("Mitochondria make ATP."));
        assert_eq!(q[0].topics, sv(&["Cells"]));
        assert!(q[0].image.is_none(), "no embedded file in this fixture");
        assert_eq!(mc(&q[1]).iter().filter(|a| a.1).count(), 2);
        assert!(matches!(q[2].kind, Kind::Tf { correct: true }));
        assert_eq!(blank(&q[3]), vec![sv(&["cell membrane", "plasma membrane"])]);
        assert_eq!(q[4].topics, sv(&["Math in biology"]));
        assert_eq!(blank(&q[4]), vec![sv(&["46"])]);
        assert_eq!(q[5].stem, "About what percent of a cell is water? (±5)");
        assert_eq!(blank(&q[5]), vec![sv(&["70", "65", "75"])]);
        assert_eq!(pairs(&q[6]), (vec![("Ribosome".into(), "Makes proteins".into()), ("Nucleus".into(), "Holds DNA".into())], sv(&["Digests sugar"])));
        assert_eq!(q[7].stem, "State the three parts of cell theory.\n\n1. All living things…\n2. The cell is…");
        assert!(matches!(&q[7].kind, Kind::Short { sample } if sample.starts_with("All living things are made of cells")));
        assert_eq!(q[8].stem, "The ___ holds DNA and a cell has ___ of them. Plants store energy as ___.");
        assert_eq!(blank(&q[8]), vec![sv(&["nucleus", "nucleous"]), sv(&["1"]), sv(&["starch"])]);
        assert_eq!(q[9].stem, "Animal cells have ___ but no ___.");
        assert_eq!(blank(&q[9]), vec![sv(&["mitochondria"]), sv(&["chloroplasts"])]);
        assert_eq!(q[10].stem, "Use the table.\n\nElement | Symbol\n---|---\nIron | Fe\nLead | Pb\n\n• Pick the *metal* with symbol Pb.\n\nUse $x^2$ if needed.");
    }

    #[test]
    fn moodle_embedded_file_resolves_and_utf16_decodes() {
        let xml = String::from_utf8_lossy(fixture!("moodle_image.xml")).to_string();
        let d = xml_doc(&xml).unwrap();
        let qt = desc(d.root_element(), "questiontext").unwrap();
        let (stem, srcs) = moodle_rich(qt);
        assert_eq!(stem, "This leaf is green.\n\n[picture: leaf]");
        let bytes = resolve_media(&srcs[0], &moodle_files(qt), "").expect("@@PLUGINFILE@@ + %20 resolve to the <file>");
        assert!(bytes.starts_with(b"\x89PNG"));
        let parsed = doc(fixture!("moodle_image.xml"), "moodle_image.xml");
        assert!(parsed.questions[0].image.is_some());
        // The same file saved as UTF-16 (with BOM) reads the same.
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(doc(&utf16, "moodle_image.xml").questions[0].stem, stem);
    }

    #[test]
    fn xml_files_that_are_neither_get_a_helpful_error() {
        let e = detect_and_parse(b"<?xml version=\"1.0\"?><rss><channel/></rss>", "feed.xml").unwrap_err();
        assert!(e.contains("neither Moodle XML"), "{e}");
        let e = detect_and_parse(b"<quiz><question", "broken.xml").unwrap_err();
        assert!(e.contains("could not be read"), "{e}");
        let e = detect_and_parse(b"<quiz><question type=\"description\"><questiontext><text>Hi</text></questiontext></question></quiz>", "d.xml").unwrap_err();
        assert!(e.contains("only: description"), "{e}");
        assert!(detect_and_parse(&[0xD0, 0xCF, 0x11, 0xE0, 0, 0], "old.xls").unwrap_err().contains(".xlsx"));
        assert!(detect_and_parse(b"PK\x03\x04garbage", "x.zip").unwrap_err().contains("could not be opened"));
    }

    // ---------- QTI 1.2 ----------

    fn canvas_checks(d: &ImportDoc) {
        assert_eq!(d.name, "Atoms quiz");
        let q = &d.questions;
        assert_eq!(
            kinds(d),
            vec![
                "Multiple choice", "True/False", "Multiple choice", "Fill in the blank", "Short answer", "Matching", "Fill in the blank", "Fill in the blank",
                "Fill in the blank", "Short answer",
            ],
            "text_only skipped"
        );
        assert_eq!(q[0].stem, "Which particle has a **negative** charge?\n\n[picture: atom.png]");
        assert_eq!(mc(&q[0])[1], ("Electron".into(), true));
        assert_eq!(q[0].points, Some(2.0));
        assert_eq!(q[0].explanation.as_deref(), Some("Electrons carry the negative charge."));
        assert!(matches!(q[1].kind, Kind::Tf { correct: true }));
        assert_eq!(mc(&q[2]), vec![("Protons".into(), true), ("Neutrons".into(), true), ("Electrons".into(), false)], "<not> answers stay wrong");
        assert_eq!(blank(&q[3]), vec![sv(&["Na", "na"])]);
        assert_eq!(pairs(&q[5]), (vec![("Fe".into(), "Iron".into()), ("Pb".into(), "Lead".into())], sv(&["Tin"])));
        assert_eq!(q[6].stem, "What is g in m/s²?");
        assert_eq!(blank(&q[6]), vec![sv(&["9.8", "9.7", "9.9"])]);
        assert_eq!(q[7].stem, "Roses are ___, violets are ___.");
        assert_eq!(blank(&q[7]), vec![sv(&["red", "Red"]), sv(&["blue"])]);
        assert_eq!(blank(&q[8]), vec![sv(&["moo"]), sv(&["meow"])], "dropdowns keep only the correct choice");
    }

    #[test]
    fn canvas_qti12_single_file() {
        let d = doc(fixture!("qti12_canvas_quiz.xml"), "qti12_canvas_quiz.xml");
        canvas_checks(&d);
        assert!(d.questions[0].image.is_none(), "a lone XML has no image bytes");
    }

    #[test]
    fn canvas_export_zip_dedupes_and_attaches_the_image() {
        let d = doc(fixture!("canvas_qti_export.zip"), "canvas_qti_export.zip");
        canvas_checks(&d);
        assert!(d.questions[0].image.is_some(), "$IMS-CC-FILEBASE$/Quiz%20Files/atom.png resolves inside the zip");
        assert!(d.questions.iter().all(|q| q.topics.is_empty()), "one quiz: no quiz-title topics");
    }

    #[test]
    fn blackboard_style_qti12_without_type_metadata() {
        let d = doc(fixture!("qti12_blackboard.xml"), "pool.xml");
        assert_eq!(d.name, "Blackboard pool");
        assert_eq!(d.questions[0].stem, "What is H₂O?");
        assert_eq!(mc(&d.questions[0]), vec![("Water".into(), true), ("Salt".into(), false)], "SCORE.max scores, the `other` 0 does not");
        assert!(matches!(d.questions[1].kind, Kind::Tf { correct: true }));
        assert_eq!(blank(&d.questions[2]), vec![sv(&["8"])]);
    }

    // ---------- QTI 2.x / 3.0 ----------

    #[test]
    fn qti21_package_follows_the_test_order_and_maps_interactions() {
        let d = doc(fixture!("qti21_package.zip"), "qti21_package.zip");
        assert_eq!(d.name, "Science check-in");
        assert_eq!(
            kinds(&d),
            vec!["Multiple choice", "Matching", "Fill in the blank", "Fill in the blank", "Fill in the blank", "Short answer", "Fill in the blank", "Multipart"]
        );
        let q = &d.questions;
        assert_eq!(q[0].stem, "Which piece of glassware is shown?\n\n[picture: glassware]");
        let Kind::Mc { answers } = &q[0].kind else { panic!() };
        assert!(answers[1].correct && answers[2].locked, "fixed=\"true\" locks the choice");
        assert_eq!(q[0].points, Some(2.0));
        assert!(q[0].image.is_some(), "../images/flask.png resolves from items/");
        assert_eq!(pairs(&q[1]), (vec![("Mass".into(), "kilogram".into()), ("Length".into(), "metre".into())], sv(&["second"])));
        assert_eq!(q[1].stem, "Match each quantity to its SI unit.");
        assert_eq!(q[2].stem, "The capital of France is ___.");
        assert_eq!(blank(&q[2]), vec![sv(&["Paris", "paris"])]);
        assert_eq!(blank(&q[3]), vec![sv(&["Atom, Cell, Organ"])]);
        assert_eq!(q[4].stem, "Water freezes at ___ degrees Celsius.");
        assert_eq!(blank(&q[4]), vec![sv(&["0"])]);
        assert_eq!(q[5].stem, "Explain why the sky is blue.");
        assert_eq!(q[6].stem, "Drag the words into the gaps.\n\nPlants give off ___ and take in ___.");
        assert_eq!(blank(&q[6]), vec![sv(&["oxygen"]), sv(&["carbon dioxide"])]);
        let Kind::Multipart { parts } = &q[7].kind else { panic!() };
        assert_eq!(q[7].stem, "Read about the experiment, then answer both parts.");
        assert_eq!(parts[0].stem, "Was the result significant?");
        assert!(matches!(parts[0].kind, PartKind::Tf { correct: true }));
        assert!(matches!(parts[1].kind, PartKind::Short { .. }));
    }

    #[test]
    fn qti21_single_multiple_response_and_qti3() {
        let d = doc(fixture!("qti21_choice_multiple.xml"), "x.xml");
        assert_eq!(d.name, "x", "a lone item has no quiz title");
        assert_eq!(d.questions[0].stem, "Which of these are **metals**?");
        assert_eq!(mc(&d.questions[0]), vec![("Copper".into(), true), ("Sulfur".into(), false), ("Zinc".into(), true)]);
        assert_eq!(d.questions[0].points, Some(3.0));
        let d = doc(fixture!("qti3_item.xml"), "qti3_item.xml");
        assert_eq!(mc(&d.questions[0]), vec![("KCl".into(), false), ("NaCl".into(), true), ("CaCO₃".into(), false)]);
    }

    #[test]
    fn qti_zip_without_questions_and_docx_in_a_zip() {
        use std::io::Write;
        let build = |files: &[(&str, &str)]| {
            let mut buf = Vec::new();
            {
                let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
                for (n, body) in files {
                    z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                    z.write_all(body.as_bytes()).unwrap();
                }
                z.finish().unwrap();
            }
            buf
        };
        let e = detect_and_parse(&build(&[("readme.txt", "hi")]), "x.zip").unwrap_err();
        assert!(e.contains("no question files"), "{e}");
        let docx = build(&[("word/document.xml", "<w:document xmlns:w=\"w\"><w:body><w:p><w:r><w:t>TF</w:t></w:r></w:p><w:p><w:r><w:t>The sky is blue.</w:t></w:r></w:p><w:p><w:r><w:t>T</w:t></w:r></w:p></w:body></w:document>")]);
        assert!(matches!(doc(&docx, "q.docx").questions[0].kind, Kind::Tf { correct: true }));
        // Several quizzes in one archive: each question is tagged with its quiz title.
        let quiz = |title: &str, id: &str| {
            format!("<questestinterop><assessment title=\"{title}\"><section><item ident=\"{id}\"><presentation><material><mattext>Q {id}?</mattext></material><response_lid ident=\"r\"><render_choice><response_label ident=\"a\"><material><mattext>yes</mattext></material></response_label><response_label ident=\"b\"><material><mattext>no</mattext></material></response_label></render_choice></response_lid></presentation><resprocessing><respcondition><conditionvar><varequal respident=\"r\">a</varequal></conditionvar><setvar>1</setvar></respcondition></resprocessing></item></section></assessment></questestinterop>")
        };
        let (a, b) = (quiz("Quiz A", "1"), quiz("Quiz B", "2"));
        let d = doc(&build(&[("a/a.xml", &a), ("b/b.xml", &b)]), "course.imscc");
        assert_eq!(d.questions.iter().map(|q| q.topics.clone()).collect::<Vec<_>>(), vec![sv(&["Quiz A"]), sv(&["Quiz B"])]);
    }

    // ---------- HTML → markup ----------

    fn tp_stem(content: serde_json::Value) -> String {
        flatten_doc(&serde_json::json!({"type": "document", "content": content})).text
    }

    #[test]
    fn struck_text_imports_as_strikethrough() {
        // HTML (Moodle, QTI, Canvas, Blackboard, Anki all read HTML through the same converter)
        assert_eq!(html_inline("Mass is <s>9 g</s> 7 g"), "Mass is ~~9 g~~ 7 g");
        assert_eq!(html_inline("<del>old</del> new <strike>gone</strike>"), "~~old~~ new ~~gone~~");
        assert_eq!(html_inline("<span style=\"text-decoration: line-through;\">x</span> y"), "~~x~~ y");
        assert_eq!(html_inline("<span style=\"color:red\">red</span>"), "red");
        // edge whitespace stays outside the markers; empty or tilde-led runs stay plain
        assert_eq!(html_inline("a<s> b </s>c"), "a ~~b~~ c");
        assert_eq!(html_inline("<s> </s>x"), "x");
        assert_eq!(html_inline("<s>~x</s>"), "~x");
        let (stem, _) = html_to_markup("<p>Cross out the <del>wrong</del> word.</p>");
        assert!(crate::richtext::inline(&stem).iter().any(|sp| sp.strike && sp.text == "wrong"), "{stem:?}");
        // Moodle XML question text
        let xml = r#"<quiz><question type="essay"><name><text>q</text></name><questiontext format="html"><text><![CDATA[<p>Not <strike>this</strike></p>]]></text></questiontext></question></quiz>"#;
        let d = doc(xml.as_bytes(), "q.xml");
        assert_eq!(d.questions[0].stem, "Not ~~this~~");
        // Test Parrot marks
        let t = tp_stem(serde_json::json!([{"type": "paragraph", "content": [{"type": "text", "text": "old", "marks": ["strike"]}, {"type": "text", "text": " new"}]}]));
        assert_eq!(t, "~~old~~ new");
    }

    #[test]
    fn centred_text_and_inline_code_import() {
        // a stem as Test Parrot's bank export writes it (round 7: r7/pw/exported-bank.parrot.json)
        let t = tp_stem(serde_json::json!([
            {"type": "paragraph", "align": "center", "content": [{"type": "text", "text": "Centered intro line"}]},
            {"type": "paragraph", "content": [{"type": "text", "text": "Run "}, {"type": "text", "text": "print(x)", "marks": [{"type": "inline-code"}]}, {"type": "text", "text": " now."}]},
            {"type": "heading", "level": 4, "content": [{"type": "text", "text": "Sub heading four"}]},
            {"type": "bullet-list", "content": [
                {"type": "list-item", "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "Parent bullet"}]},
                    {"type": "bullet-list", "content": [{"type": "list-item", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Child bullet"}]}]}]}
                ]}
            ]}
        ]));
        assert_eq!(t, "::: center\nCentered intro line\n:::\n\nRun `print(x)` now.\n\n**Sub heading four**\n\n\u{2022} Parent bullet\n  \u{2022} Child bullet");
        let n = crate::richtext::parse_tree(&t);
        assert!(matches!(&n[0], crate::richtext::Node::Block { center: true, .. }));
        assert!(crate::richtext::parse(&t).iter().any(|b| matches!(b, crate::richtext::Block::Para(ls) if ls[0].iter().any(|sp| sp.code && sp.text == "print(x)"))));
        assert!(matches!(n.last(), Some(crate::richtext::Node::Block { depths, .. }) if depths == &vec![0, 1]));
        // TipTap / Milkdown spellings: attrs.textAlign / attrs.align, `inlineCode` / `code` marks
        let t2 = tp_stem(serde_json::json!([
            {"type": "heading", "attrs": {"level": 2, "textAlign": "center"}, "content": [{"type": "text", "text": "Title"}]},
            {"type": "paragraph", "attrs": {"align": "left"}, "content": [{"type": "text", "text": "a", "marks": [{"type": "inlineCode"}, {"type": "bold"}]}, {"type": "text", "text": " b`c", "marks": ["code"]}]}
        ]));
        assert_eq!(t2, "::: center\n## Title\n:::\n\n**`a`** b`c", "a code run the markup cannot hold stays plain");
        // a list item with a boolean `checked` is a task item (Milkdown keeps it there)
        let t3 = tp_stem(serde_json::json!([{"type": "bullet-list", "content": [
            {"type": "list-item", "attrs": {"checked": true}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Done"}]}]},
            {"type": "list-item", "attrs": {"checked": null}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Plain"}]}]}
        ]}]));
        assert_eq!(t3, "\u{2022} \u{2611} Done\n\u{2022} Plain");
        // HTML (every HTML source shares the converter)
        assert_eq!(html_to_markup("<p style=\"text-align: center\">Title</p><p>Use <code>ls -l</code> then <kbd>Enter</kbd>.</p>").0, "::: center\nTitle\n:::\n\nUse `ls -l` then `Enter`.");
        assert_eq!(html_to_markup("<center><h2>T</h2><p>sub</p></center>").0, "::: center\n## T\n\nsub\n:::");
        assert_eq!(html_to_markup("<div align=\"center\"><center>x</center></div>").0, "::: center\nx\n:::", "groups do not nest");
        assert_eq!(html_inline("<code>a`b</code> <code> </code>c"), "a`b c");
        // a centred group in a side-by-side panel survives the panel clean-up
        let s = side_markup(&["::: center\nL\n:::".to_string(), "R".to_string()]);
        assert_eq!(s, "::: side\n::: center\nL\n:::\n::: next\nR\n:::");
        assert!(matches!(&crate::richtext::parse_tree(&s)[0], crate::richtext::Node::Side(p) if matches!(&p[0][0], crate::richtext::Node::Block { center: true, .. })));
    }

    #[test]
    fn task_lists_import_as_boxed_bullets() {
        // Test Parrot task lists (kebab-case like its other nodes; camelCase accepted too)
        let t = tp_stem(serde_json::json!([
            {"type": "paragraph", "content": [{"type": "text", "text": "Check:"}]},
            {"type": "task-list", "content": [
                {"type": "task-item", "checked": true, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Goggles on"}]}]},
                {"type": "task-item", "attrs": {"checked": false}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Gloves "}, {"type": "text", "text": "on", "marks": ["bold"]}]}]}
            ]},
            {"type": "taskList", "content": [{"type": "taskItem", "attrs": {"checked": true}, "content": [{"type": "text", "text": "Done"}]}]}
        ]));
        assert_eq!(t, "Check:\n\n\u{2022} \u{2611} Goggles on\n\u{2022} \u{2610} Gloves **on**\n\n\u{2022} \u{2611} Done");
        let b = crate::richtext::parse(&t);
        assert!(matches!(&b[1], crate::richtext::Block::Bullets(items) if items.len() == 2), "a real bullet list: {b:?}");
        // HTML task lists: checkbox inputs, or TipTap's data-checked items
        assert_eq!(html_to_markup("<ul><li><input type=\"checkbox\" checked> Ruler</li><li><input type=\"checkbox\"> Pencil</li></ul>").0, "\u{2022} \u{2611} Ruler\n\u{2022} \u{2610} Pencil");
        assert_eq!(html_to_markup("<ul data-type=\"taskList\"><li data-checked=\"true\"><p>A</p></li><li data-checked=\"false\"><p>B</p></li></ul>").0, "\u{2022} \u{2611} A\n\u{2022} \u{2610} B");
        // the boxes print (Source Serif 4 has them, P3 CID fonts)
        assert!(crate::fonts::charmap(crate::fonts::Style::Regular).get('\u{2610}').is_some() && crate::fonts::charmap(crate::fonts::Style::Regular).get('\u{2611}').is_some());
    }

    #[test]
    fn links_import_as_link_markup() {
        assert_eq!(html_inline("See <a href=\"https://ex.org/a?b=1\">the <b>notes</b></a> now"), "See [the **notes**](https://ex.org/a?b=1) now");
        assert_eq!(html_inline("Mail <a href=\"mailto:t@s.org\"> me </a>."), "Mail [me](mailto:t@s.org) .");
        // spaces and parentheses in the address are percent-encoded so the link parses
        assert_eq!(html_inline("<a href=\"https://en.wikipedia.org/wiki/Ion_(physics)\">ion</a>"), "[ion](https://en.wikipedia.org/wiki/Ion_%28physics%29)");
        // relative, javascript and other schemes stay plain text, as before
        assert_eq!(html_inline("<a href=\"/course/view.php\">course</a>"), "course");
        assert_eq!(html_inline("<a href=\"javascript:alert(1)\">x</a>"), "x");
        assert_eq!(html_inline("<a>anchor</a>"), "anchor");
        // link text that could not read back as a link (math) stays plain
        assert_eq!(html_inline("<a href=\"https://a.org\">$x$ [1]</a>"), "$x$ [1]");
        let (stem, _) = html_to_markup("<p>Read <a href=\"https://ex.org\">this</a>.</p>");
        assert!(crate::richtext::inline(&stem).iter().any(|sp| sp.text == "this" && sp.link.as_deref() == Some("https://ex.org")), "{stem:?}");
        // Test Parrot link marks
        let t = tp_stem(serde_json::json!([{"type": "paragraph", "content": [{"type": "text", "text": "site", "marks": [{"type": "link", "attrs": {"href": "https://s.org"}}, "bold"]}, {"type": "text", "text": " here"}]}]));
        assert_eq!(t, "[**site**](https://s.org) here");
    }

    #[test]
    fn headings_dividers_and_side_by_side_import() {
        // h4..h6 print distinct from h3 (and each other): bold, bold italic, italic lines
        assert_eq!(html_to_markup("<h1>Part A</h1><p>x</p><hr><h4>deep</h4>").0, "# Part A\n\nx\n\n---\n\n**deep**");
        assert_eq!(html_to_markup("<h3>Three</h3><h4>Four</h4><h5>Five</h5><h6>Six</h6>").0, "### Three\n\n**Four**\n\n***Five***\n\n*Six*");
        // a heading with a blank keeps the old bold form
        assert_eq!(html_to_markup("<h2>Fill ___ in</h2>").0, "**Fill ___ in**");
        let t = tp_stem(serde_json::json!([
            {"type": "heading", "level": 2, "content": [{"type": "text", "text": "Data"}]},
            {"type": "rule"},
            {"type": "side-by-side", "content": [
                {"type": "side-by-side-panel", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "left"}]}]},
                {"type": "side-by-side-panel", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "right"}]}, {"type": "paragraph", "content": [{"type": "text", "text": "more"}]}]}
            ]}
        ]));
        assert_eq!(t, "## Data\n\n---\n\n::: side\nleft\n::: next\nright\n\nmore\n:::");
        let b = crate::richtext::parse(&t);
        assert!(matches!(&b[0], crate::richtext::Block::Heading(2, _)));
        assert_eq!(b[1], crate::richtext::Block::Divider);
        assert!(matches!(&b[2], crate::richtext::Block::Side(p) if p.len() == 2 && p[1].len() == 2));
    }

    #[test]
    fn html_converter_handles_messy_html() {
        let (s, imgs) = html_to_markup("<p>A&nbsp;&amp;&nbsp;B &lt; C &eacute;&#233;&#x41;</p><br><p>x < 3 and <i>y</i><sup>2</sup> = <b>z</b> <span>H<sub>2</sub>O</span><p>unclosed<ul><li>one<li>two<ol><li>nested</ol></ul><script>bad()</script><img src=\"a.png\"><img class=\"equation_image\" data-equation-content=\"\\frac{1}{2}\" src=\"eq.png\"><pre>  keep\n  this</pre>");
        assert_eq!(s, "A & B < C ééA\n\nx < 3 and *y*² = **z** H₂O\n\nunclosed\n\n• one\n• two\n  1. nested\n\n[picture]$\\frac{1}{2}$\n\n  keep\n  this");
        assert_eq!(imgs, sv(&["a.png"]));
        assert_eq!(html_inline("<p>one</p><p>two <b>three</b></p>"), "one two **three**");
        let (t, _) = html_to_markup("<table><tr><td></td><td>b</td></tr><tr><td>c</td><td colspan=2>d|e</td></tr></table>");
        assert_eq!(t, "|  | b |  |\n| c | d/e |  |");
        assert_eq!(tex_to_dollars("a \\(x\\) b \\[y\\] c $$z$$"), "a $x$ b $y$ c $z$");
        assert_eq!(fmt_clean((9.7 + 9.9) / 2.0), "9.8");
        assert_eq!(join_path("items", "../images/a b.png"), "images/a b.png");
        assert_eq!(percent_decode("Quiz%20Files/%E2%82%AC.png%"), "Quiz Files/€.png%");
    }

    // ---------- source labels ----------

    #[test]
    fn format_labels_name_each_new_format() {
        let cases: [(&[u8], &str, Option<&str>); 16] = [
            (fixture!("moodle_sample.gift"), "moodle_sample.gift", Some("Moodle GIFT")),
            (fixture!("quizlet_terms.txt"), "q.txt", Some("Flashcard list (Quizlet/Anki)")),
            (fixture!("anki_notes.txt"), "a.txt", Some("Flashcard list (Quizlet/Anki)")),
            (fixture!("vocab_list.csv"), "v.csv", Some("Flashcard list (Quizlet/Anki)")),
            (fixture!("respondus.txt"), "r.txt", Some("Respondus / text2qti text")),
            (fixture!("text2qti.md"), "t.md", Some("Respondus / text2qti text")),
            (fixture!("brightspace.csv"), "b.csv", Some("Brightspace CSV")),
            (fixture!("moodle_quiz.xml"), "m.xml", Some("Moodle XML")),
            (fixture!("qti12_canvas_quiz.xml"), "c.xml", Some("QTI file")),
            (fixture!("qti3_item.xml"), "q3.xml", Some("QTI file")),
            (fixture!("canvas_qti_export.zip"), "c.zip", Some("QTI / Canvas package")),
            (fixture!("qti21_package.zip"), "p.zip", Some("QTI / Canvas package")),
            (fixture!("questions.xlsx"), "q.xlsx", Some("Excel spreadsheet")),
            (fixture!("kahoot.xlsx"), "k.xlsx", Some("Excel spreadsheet")),
            (b"MC\nQ?\n*a. x\nb. y\n", "bb.txt", None),
            (b"type,stem,a,correct\nmc,Q?,x,a\n", "c.csv", None),
        ];
        for (bytes, name, want) in cases {
            assert_eq!(format_label(bytes, name), want, "{name}");
        }
    }

    /// Truncated, mutated and multibyte-salted fixtures return Ok or Err — never panic
    /// (the tolerant HTML/GIFT/QTI scanners slice strings by byte index).
    #[test]
    fn damaged_inputs_never_panic() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/import_fixtures");
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let bytes = std::fs::read(&p).unwrap();
            let step = (bytes.len() / 150).max(1);
            for cut in (0..bytes.len()).step_by(step) {
                let _ = detect_and_parse(&bytes[..cut], &name);
            }
            for _ in 0..60 {
                let mut b = bytes.clone();
                for _ in 0..4 {
                    if b.is_empty() {
                        break;
                    }
                    let i = (rnd() as usize) % b.len();
                    match rnd() % 4 {
                        0 => b[i] = (rnd() % 256) as u8,
                        1 => b.splice(i..i, "é€{}<>[[".bytes()).for_each(drop),
                        2 => b.splice(i..i, b"<p><b>&#x1F600;&bogus;</".iter().copied()).for_each(drop),
                        _ => {
                            b.truncate(i);
                        }
                    }
                }
                let _ = detect_and_parse(&b, &name);
                let _ = format_label(&b, &name);
            }
        }
        for junk in ["{", "}{", "{#", "{=", "::", "::a", "[[1]]", "<", "</", "<a", "<!--", "<![CDATA[", "&#xFFFFFFFF;", "1) x\n= [", "\\", "{1:MC:", "\u{0}"] {
            let _ = detect_and_parse(junk.as_bytes(), "j.gift");
            let _ = detect_and_parse(junk.as_bytes(), "j.txt");
            let _ = html_to_markup(junk);
            let _ = cloze(junk);
        }
    }

    /// Every fixture in the folder parses (a new fixture file cannot be forgotten here).
    #[test]
    fn every_fixture_parses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/import_fixtures");
        let mut n = 0;
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if name.ends_with(".py") {
                continue;
            }
            let d = detect_and_parse(&std::fs::read(&p).unwrap(), &name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!d.questions.is_empty(), "{name}");
            n += 1;
        }
        assert!(n >= 19, "found only {n} fixtures");
    }

    #[test]
    fn format_version_reads_json_and_qti() {
        assert_eq!(format_version(br#"{"format":"test-parrot/package","formatVersion":"0.5.0"}"#).as_deref(), Some("0.5.0"));
        assert_eq!(format_version(br#"{"kind":"test-parrot-question-bank","version":1}"#).as_deref(), Some("1"));
        assert_eq!(format_version(b"<questestinterop><item/></questestinterop>").as_deref(), Some("QTI 1.2"));
        assert_eq!(format_version(b"MC\nWhat?\n*a\nb\n"), None);
    }

    #[test]
    fn plain_line_star_choices_are_choices_not_stem() {
        // Test Parrot's own paste placeholder: wrong choices are bare lines.
        let parsed = |t: &str| parse_blackboard(t).unwrap().questions;
        let qs = parsed("MC\nWhich planet is closest to the Sun?\nVenus\n*Mercury\nMars\n\nTF\nThe Sun is a star.\nT\n");
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[0].stem, "Which planet is closest to the Sun?");
        let Kind::Mc { answers } = &qs[0].kind else { panic!("not MC") };
        let texts: Vec<&str> = answers.iter().map(|a| a.text.as_str()).collect();
        assert_eq!(texts, ["Venus", "Mercury", "Mars"]);
        assert_eq!(answers.iter().map(|a| a.correct).collect::<Vec<_>>(), [false, true, false]);
        assert!(matches!(qs[1].kind, Kind::Tf { correct: true }));
        // Correct choice last: one star among several bare lines is still the plain form.
        let q = &parsed("MC\nPick one\nRed\nBlue\n*Green\n")[0];
        assert_eq!(q.stem, "Pick one");
        let Kind::Mc { answers } = &q.kind else { panic!() };
        assert_eq!(answers.len(), 3);
        assert!(answers[2].correct);
        // A table under the stem stays in the stem.
        let q = &parsed("MC\nUse the table.\n| x | y |\n| 1 | 2 |\nOne\n*Two\n")[0];
        assert_eq!(q.stem, "Use the table.\n| x | y |\n| 1 | 2 |");
        // Lettered choices keep the old rule (multi-line stems allowed).
        let q = &parsed("MC\nLine one\nline two\n*a. yes\nb. no\n")[0];
        assert_eq!(q.stem, "Line one\nline two");
        // All-starred choices after a multi-line stem keep the old rule too.
        let q = &parsed("MC\nLine one\nline two\n*yes\n*also\n")[0];
        assert_eq!(q.stem, "Line one\nline two");
    }

    #[test]
    fn blackboard_tab_delimited_upload_file() {
        let text = "MC\tWhich planet is closest to the Sun?\tVenus\tincorrect\tMercury\tcorrect\tMars\tincorrect\n\
                    TF\tThe Sun is a star.\ttrue\n\
                    FIB\tWater is made of hydrogen and ___.\toxygen\n\
                    ESS\tExplain gravity.\tAttraction of masses.\n\
                    NUM\tPi to two places?\t3.14\t0.01\n\
                    MAT\tMatch.\tNa\tsodium\tK\tpotassium\n\
                    ORD\tOrder these.\tone\ttwo\tthree\n\
                    FIB_PLUS\tThe [a] is [b].\ta\tsky\t\tb\tblue\tazure\n\
                    OP\tHow do you feel?\n";
        assert_eq!(format_label(text.as_bytes(), "pool.txt"), Some("Blackboard upload file (tab-delimited)"));
        let d = doc(text.as_bytes(), "pool.txt");
        assert_eq!(d.questions.len(), 8, "OP skipped");
        let Kind::Mc { answers } = &d.questions[0].kind else { panic!("not MC: {:?}", d.questions[0].kind) };
        assert_eq!(answers.len(), 3);
        assert!(answers[1].correct && !answers[0].correct && !answers[2].correct);
        assert_eq!(answers[1].text, "Mercury");
        assert!(matches!(d.questions[1].kind, Kind::Tf { correct: true }));
        assert!(matches!(&d.questions[2].kind, Kind::Blank { accept } if accept == &vec![vec!["oxygen".to_string()]]));
        assert!(matches!(&d.questions[3].kind, Kind::Short { sample } if sample == "Attraction of masses."));
        assert!(matches!(&d.questions[4].kind, Kind::Blank { accept } if accept[0].contains(&"3.15".to_string())));
        assert!(matches!(&d.questions[5].kind, Kind::Matching { pairs, .. } if pairs.len() == 2 && pairs[1].right == "potassium"));
        assert!(matches!(&d.questions[6].kind, Kind::Blank { accept } if accept[0][0] == "one, two, three"));
        assert_eq!(d.questions[7].stem, "The ___ is ___.");
        assert!(matches!(&d.questions[7].kind, Kind::Blank { accept } if accept.len() == 2 && accept[1].len() == 2));
        // A FIB stem with no blank gets one appended.
        let d = doc(b"FIB\tThe capital of France is\tParis\n", "x.txt");
        assert_eq!(d.questions[0].stem, "The capital of France is ___");
    }

    #[test]
    fn term_list_never_claims_tagged_rows() {
        // A two-column list whose first cells are type tags is not flashcards.
        assert!(term_pairs("MC\tstem one\nTF\tstem two\n").is_none());
        // Ordinary vocabulary tab lists are still term lists.
        assert!(term_pairs("cat\tgato\ndog\tperro\nbird\tpajaro\n").is_some());
    }
}
