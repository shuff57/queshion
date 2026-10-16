# Brand — Test Parrot clone → SH-named app

Angle: teacher tool; local-first; question banks; print; versions; AI conversion.
Name family: SH-initial compounds (user's initials). Second family (per user direction,
2026-10-15): **SH must sit in the MIDDLE of the name** — the S–H pair lands mid-word
(ChalkShed, Workshed, Freshsheet, Examshed, Marksheet, Examshell…).

## Middle-SH candidates, screened (web search + RDAP, 2026-10-15)

| Candidate | SH position | Web/category collision | Domain | Verdict |
| --- | --- | --- | --- | --- |
| **Chalkshed** | mid-word (chalk**sh**ed) | **nothing found anywhere** | **.com AND .app AVAILABLE** | **top pick** |
| Freshsheet | mid-word (fre**sh**sheet — SH twice) | fresh-sheet.com (farm SaaS, different category); freshsheet.app registered 5 days ago by someone else; freshsheet.com taken | — | keep as runner-up; recent .app registration is a sniper flag |
| Examshed | mid-word (exam**sh**ed) | web-clean, but sounds/looks too close to QuizShed (quizshed.com — edtech quiz platform, same category) | .com available | cut — the trademark-style sound test fails |
| Workshed | mid-word (work**sh**ed) | two small software shops named Workshed | — | cut (same broad category) |
| QuizSheaf / Papershelf | — | QuizShed (edtech); PaperShelf (paper manager) + thepapershelf.com | — | cut |
| Marksheet | dead-center (mar**sh**eet) | generic term, saturated by school ERPs (Inkwelly, Academia, marksheet apps) | — | cut — generic + crowded |
| Examshell | mid-word (exam**sh**ell) | dormant Exam Shell apps (O/A-Level practice) | — | cut |

## Keep from round one (SH opens the name)

| Shufflery | SH opens | clean web | shufflery.com taken; shufflery.app available | top pick of round one |
| Shufflemill | SH opens | clean web | shufflemill.com available | all-clean fallback |

## Recommended: **Chalkshed** — "the shed where exams get built"

- Chalk = the teacher's oldest tool; shed = the workshop where things get made. The app
  is exactly that: a quiet workshop where question banks and printable exams get built.
- SH sits dead-center; one hearing, spelled the way it sounds, no bad meaning in major languages.
- `chalkshed.com` and `chalkshed.app` both open as of 2026-10-15.
- Nothing named Chalkshed exists in any category found.
- Runner-up: Freshsheet (warmer, real words, but domain situation muddier) / Shufflery (round-one pick, SH opens instead).

## Candidates screened (screening checks run 2026-10-15, web search + whois/RDAP)

| Candidate | Web/category collision | Domain | Verdict |
| --- | --- | --- | --- |
| Shufflecraft | ShuffleCraft Studio (shufflecraftstudio.com), small dev studio — same broad category | .com taken 2022 | in-suite fit but a real studio shares the name; pass if kept inside teacher.dev's Craft namespace |
| Sheaf | **sheaf.co.in — AI study planner/quiz app — direct category collision** | — | out |
| Shelfmark | shelfmark.com (AI manufacturing inspection) + 4k-star self-hosted app | taken | out |
| Shipshape | shipshape.ai, funded home-management platform | taken | out |
| Shuffledeck | Student card-game startup + SwiftUI ShuffleDeck lib (weak, adjacent) | — | out |
| **Shufflery** | **nothing found — no app, no studio** | shufflery.com TAKEN; **shufflery.app available** | **top pick** |
| **Shufflemill** | **nothing found** | **shufflemill.com available** | pick if .com is needed |
| Sheafcraft | nothing found | sheafcraft.com available | sheaf meaning + craft suffix; backup |

## Recommended: **Shufflery** — "a place where exams get shuffled"

- Reads as a real place noun (distillery, bakery) for the workshop where papers are made.
- SH is the name, not a decoration; teacher-safe, one hearing, no bad meaning in major languages.
- Desktop app first — `shufflery.app` is open and honest for a desktop tool.
- Alternate: **Shufflemill** (everything available incl. .com) or **Sheafcraft** (sheaf = bundle of papers).

## Trademark screening — TO RUN (not run yet; do before any public launch)

| check | where | status |
| --- | --- | --- |
| US trademark (class 9, 42) | tmsearch.uspto.gov "Shufflery" | to run |
| EU trademark | euipo.europa.eu eSearch | to run |
| global | WIPO Global Brand Database | to run |
| App Store / Play exact name | store search | to run (desktop app today) |
| handles (X, IG, GitHub) | exact | to run |

Screening only, not legal clearance; a lawyer search before selling anything.

## Logo brief (when wanted)

- Idea: a single sheet of paper mid-shuffle, fanned into three sheets — the versions feature drawn as a mark; SH could be abstracted into the paper edge (two strokes = S/H suggestion).
- Type: symbol + wordmark, wordmark in the app's serif.
- Must work at 16px and 1024x1024, no transparency for icon builds, SVG + favicon + 1200x630 social card.
- Must not resemble Test Parrot's parrot mark: no bird, no red/green plumage pair.

## Palette / voice / sweep

Offered as follow-up (skill steps 3–6) — the app currently uses the Test Parrot house palette; a rebrand pass would shift the accent hue family and sweep all strings. Ask when wanted.
## Decision (2026-10-15)

User picked **queShion** — over the screening's top pick Chalkshed, and over the other SH
families (Shufflery, Freshsheet, etc.). queShion wasn't from those rounds: the "sh" sound is
carried by `-tion` (ques**tion**), not by the literal letters S–H.
Renamed 2026-10-15: crate `queshion`, app title "queShion", store dir `~/.local/share/queshion/`,
env vars `QUESHION_*`; repo dir stays `~/testparrot`. GitHub repo: `queshion` (public).
