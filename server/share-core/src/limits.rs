//! The payload limits of docs/shared-pool.md ("Limits"). Defaults are the spec's; the Worker may
//! pass raised values (an admin setting), and every field may be given on its own in JSON.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    /// Whole payload, bytes of JSON. Checked before parsing.
    pub body_bytes: usize,
    /// Container nesting depth of the payload JSON. The deepest legal shape has 12 containers
    /// (envelope > bank > questions > question > kind > Multipart > parts > part > kind > Mc >
    /// answers > answer); 16 leaves a little room and nothing more.
    pub max_depth: usize,
    /// Values in the payload JSON (every string, number, bool, null, array and object, skipped
    /// ones included). A 1,000-question bank of 26-choice questions is about 120,000 values; the
    /// default 200,000 keeps the worst shape (256 figure params on every question) under the 40 MB
    /// whole-call ceiling (`tests/memory.rs`, `tests/node/check.mjs`).
    pub max_values: usize,
    /// Objects kept by the reader (skipped unknown subtrees are not kept). The same bank has about
    /// 31,000.
    pub max_objects: usize,
    /// The reader's allocation estimate for the kept tree, bytes (`json::Budget::max_alloc`). With
    /// the value and object counts this bounds the parse's memory; the 4 MB / 1,000-question bank
    /// needs about 14 MB of it. See `tests/memory.rs` for the measured peaks.
    pub parse_budget_bytes: usize,
    pub questions_per_bank: usize,
    /// Stem (and part stem), bytes of UTF-8.
    pub stem_bytes: usize,
    pub explanation_bytes: usize,
    /// One answer text, pair side, distractor, `Short.sample`, or `Blank` accept entry, bytes.
    pub answer_bytes: usize,
    pub mc_answers: usize,
    pub matching_pairs: usize,
    pub matching_distractors: usize,
    pub blanks: usize,
    pub accept_variants: usize,
    pub parts: usize,
    pub topics: usize,
    /// Topics, tags and standards codes, characters each.
    pub tag_chars: usize,
    pub tags: usize,
    pub standards: usize,
    /// Bank name and publish title, characters.
    pub title_chars: usize,
    pub description_bytes: usize,
    /// `BankDetails.author` (and a licence name), characters.
    pub author_chars: usize,
    pub license_url_chars: usize,
    /// `figure.params` serialised, bytes of JSON.
    pub figure_params_bytes: usize,
    /// One text figure param, bytes.
    pub figure_text_bytes: usize,
    pub figure_params: usize,
    /// `points` must lie in 0..=max_points.
    pub max_points: f64,
    /// Ids and other server identifiers, characters (`[A-Za-z0-9_-]` only).
    pub id_chars: usize,
    /// Attribution chain links.
    pub chain_links: usize,
    /// Image width and height, pixels each.
    pub image_side: u32,
    /// Subject / course / grade metadata, characters each.
    pub meta_chars: usize,
}

impl Limits {
    /// The limits with every bound that sizes memory clamped to its default, whatever a caller
    /// passed: they can be lowered, never raised, so a call stays under the 40 MB ceiling that
    /// `tests/memory.rs` and `tests/node/check.mjs` measure at exactly these values. (The other
    /// limits are business rules inside the body limit and may be raised.)
    pub fn sane(&self) -> Limits {
        let d = Limits::default();
        let mut l = self.clone();
        l.body_bytes = l.body_bytes.min(d.body_bytes);
        l.max_depth = l.max_depth.min(d.max_depth);
        l.max_values = l.max_values.min(d.max_values);
        l.max_objects = l.max_objects.min(d.max_objects);
        l.parse_budget_bytes = l.parse_budget_bytes.min(d.parse_budget_bytes);
        l.figure_params = l.figure_params.min(d.figure_params);
        l.figure_params_bytes = l.figure_params_bytes.min(d.figure_params_bytes);
        l
    }
    pub fn budget(&self) -> crate::json::Budget {
        crate::json::Budget {
            max_bytes: self.body_bytes,
            max_depth: self.max_depth,
            max_values: self.max_values,
            max_objects: self.max_objects,
            max_alloc: self.parse_budget_bytes,
        }
    }
}

impl crate::json::Caps for Limits {
    fn cap(&self, c: crate::json::Cap) -> usize {
        use crate::json::Cap::*;
        match c {
            Questions => self.questions_per_bank,
            McAnswers => self.mc_answers,
            Pairs => self.matching_pairs,
            Distractors => self.matching_pairs.saturating_add(self.matching_distractors),
            Blanks => self.blanks,
            Variants => self.accept_variants,
            Parts => self.parts,
            Topics => self.topics,
            Chain => self.chain_links,
            Tags => self.tags,
            Standards => self.standards,
        }
    }
    fn params_cap(&self) -> usize {
        self.figure_params
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            body_bytes: 4 * 1024 * 1024,
            max_depth: 16,
            max_values: 200_000,
            max_objects: 60_000,
            parse_budget_bytes: 20 * 1024 * 1024,
            questions_per_bank: 1000,
            stem_bytes: 8 * 1024,
            explanation_bytes: 8 * 1024,
            answer_bytes: 2 * 1024,
            mc_answers: 26,
            matching_pairs: 50,
            matching_distractors: 26,
            blanks: 50,
            accept_variants: 20,
            parts: 50,
            topics: 20,
            tag_chars: 64,
            tags: 30,
            standards: 30,
            title_chars: 200,
            description_bytes: 2 * 1024,
            author_chars: 200,
            license_url_chars: 2000,
            figure_params_bytes: 16 * 1024,
            figure_text_bytes: 4 * 1024,
            figure_params: 256,
            max_points: 1000.0,
            id_chars: 64,
            chain_links: 32,
            image_side: 4096,
            meta_chars: 100,
        }
    }
}
