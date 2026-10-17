# queShion

Turn anything into an exam — a pure-Rust desktop clone of [testparrot.com](https://www.testparrot.com/):
question banks, printable exams, and one-shot AI conversion, all local-first.

- **egui + wgpu** (eframe 0.36) single binary; house theme (cream/terracotta, Source Serif/Sans).
- **Question banks**: six question types (multiple choice, true/false, matching, fill-in-the-blank,
  short answer, multipart), topic tags, difficulty, points, images.
- **Exam editor**: page preview with header line, numbered questions and an answer section; open a
  bank in the side panel and click or drag questions onto the exam; undo/redo; heading/text size,
  margins and ruled blanks per exam.
- **Export**: PDF (hand-rolled, base-14 fonts, embedded images) and DOCX, student test + answer key,
  shuffle question/answer order into named Versions; every export is snapshotted and re-exportable
  from Export History.
- **Imports**: Blackboard test-generator text (`MC/MA/TF/ESS/NUM/MAT/FIB/ORD`, `*` = correct), Aiken,
  CSV and Excel `.xlsx`, Moodle GIFT and Moodle XML, QTI 1.2/2.x/3.0 files and Canvas / Common
  Cartridge / LMS export `.zip`s, Respondus and text2qti text, Quizlet/Anki term lists, Kahoot and
  Brightspace sheets, and real `.parrot.json` Test Parrot packages; paste or file. Grammar and
  mappings: `src/formats.rs` module docs; `QUESHION_PARSE=<file>` prints any file's parse as JSON.
- **AI conversion**: a photo/scan/screenshot is transcribed to import text by
  **Ollama Cloud** (`gemma4:31b` — default), local **Ollama**, **OpenRouter**, or any custom
  OpenAI-compatible endpoint. Keys are read from the environment or the ollama-cloud credentials
  file at request time and never stored.

Data lives in `${XDG_DATA_HOME:-~/.local/share}/queshion/`. Headless helpers:
`QUESHION_SELFTEST=1`, `QUESHION_SEED=1`, `QUESHION_CONVERT=<file>` (prints import text),
`QUESHION_SCREEN=<welcome|home|banks|bank|exams|exam|imports|figures|settings>` opens that
screen directly (for screenshot/test harnesses).

- **Light & dark**: warm dark palette in the same hue family; the sun/moon switch at the
  bottom of the sidebar persists in `settings.json` (`"dark": true`). The exam page preview
  and figure sheets stay paper-white in dark mode (they preview the printed sheet).