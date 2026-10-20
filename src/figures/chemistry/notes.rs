//! Inline notes the reference chemistry generators set in muted type under a control: how a choice
//! behaves, why an option is missing, or a warning that a setting has no effect right now. The
//! wording is the reference's. `key_note` gives the note (and whether it is a warning) that
//! follows param `key`, for the figure's current state.

use crate::figures::Params;

pub struct Note {
    pub text: String,
    pub warn: bool,
}

fn note(text: impl Into<String>) -> Option<Note> {
    Some(Note { text: text.into(), warn: false })
}
fn warn(text: impl Into<String>) -> Option<Note> {
    Some(Note { text: text.into(), warn: true })
}

pub fn key_note(fig: &str, key: &str, p: &Params) -> Option<Note> {
    match (fig, key) {
        ("bohr-model", "gained_lost") if p.b("gained_lost") && p.b("empty") => warn("Gained and lost electrons don’t show on empty rings."),
        ("bohr-model", "gained_lost") => note(format!(
            "Compared with a neutral {} atom: gained electrons in their own color, lost ones as empty spots where they were.",
            super::data::name(super::data::z_of(p.s("element"))).to_lowercase()
        )),
        ("bohr-model", "nucleus") => match p.s("nucleus") {
            "Balls" => note("Each proton and neutron drawn as a ball."),
            "Protons and neutrons" => note("The counts written in a circle."),
            "Blank" => note("An empty circle, for students to fill in."),
            _ => None,
        },
        ("bohr-model", "placement") => match p.s("placement") {
            "Evenly spaced" => note("Spread evenly around each ring."),
            "Paired" => note("The first four singly at top, right, bottom and left, then in pairs, as in Lewis structures."),
            _ => None,
        },
        ("bohr-model", "electron_symbol") if p.s("nucleus") != "Balls" => note("Proton and neutron looks show only when the nucleus is drawn as balls."),
        ("bohr-model", "brackets") if p.b("brackets") && p.i("charge") == 0 => note("A neutral atom isn’t put in brackets."),

        ("punnett-square", "parents") => note(if p.s("parents").chars().count() > 60 {
            "That\u{2019}s longer than a cross needs."
        } else if p.s("cross") == "X-linked" {
            "Mother and father with an \u{d7} between, each X with its allele: X^H X^h \u{d7} X^H Y."
        } else if p.s("cross") == "Dihybrid" || p.s("dominance") == "Complete" {
            "The first parent goes across the top. Capitals are dominant."
        } else {
            "Type ^ for a superscript and _ for a subscript."
        }),
        ("punnett-square", "parent_labels") if p.s("cross") != "X-linked" && matches!(p.s("parent_labels"), "\u{2640} \u{2642}" | "Mother") => note("The first parent is the mother."),
        ("punnett-square", "cells") if p.s("cells") == "Some blank" => note("Click the cells students fill in."),
        ("punnett-square", "shading") => note("Gray, hatching and dots, so phenotypes stay apart in a photocopy. Blank cells aren\u{2019}t shaded."),

        ("gel-electrophoresis", "agarose") => {
            let (lo, hi) = match p.s("agarose") { "0.8%" => ("800", "10,000"), "1.5%" => ("200", "3000"), "2%" => ("100", "2000"), _ => ("500", "10,000") };
            note(format!("A {} gel separates about {lo}\u{2013}{hi} bp. Smaller pieces run farther; a stronger gel spreads out smaller ones.", p.s("agarose")))
        }
        ("gel-electrophoresis", "look") => match p.s("look") {
            "Blue stain" => note("Blue bands on a pale gel, as a classroom DNA stain leaves them."),
            "Glowing" => note("Glowing bands on a dark gel, as a stained gel looks under UV or blue light. For slides."),
            _ => note("Dark bands on a white gel, for a black-and-white copier."),
        },
        ("gel-electrophoresis", "ruler") if p.b("ruler") => note("In cm and mm, with 0 at the bottom of the wells."),
        ("gel-electrophoresis", "size_units") if p.s("ladder_sizes") != "None" && {
            let l = crate::figures::biology::gel::layout_of(p);
            let n = l.lanes.len();
            l.lanes.iter().enumerate().any(|(i, x)| x.ladder && i > 0 && i + 1 < n)
        } => note("Sizes are written beside a ladder in the first or last lane only."),
        ("gel-electrophoresis", "electrode_style") => match p.s("electrode_style") {
            "Named" => note("The signs, with Cathode beside the \u{2212} and Anode beside the +."),
            "Blank" => note("Empty circles at each end, for students to mark \u{2212} and +."),
            "None" => note("No electrodes."),
            _ => note("A black \u{2212} at the wells and a red + at the far end."),
        },
        ("gel-electrophoresis", "answer_kind") => note("The answer key lists each sample lane\u{2019}s band sizes, and the ladder\u{2019}s when its sizes are blank."),

        ("pedigree", "carrier_style") => {
            let hidden = matches!(p.s("carrier_style"), "Hidden") || (p.s("carrier_style") == "Automatic" && !p.b("carriers"));
            note(if hidden {
                "Carriers look unaffected, as students usually see them."
            } else if matches!(p.s("inherit"), "Autosomal dominant" | "X-linked dominant" | "Y-linked") {
                "A dominant or Y-linked trait has no carriers."
            } else {
                "Heterozygotes who don\u{2019}t show the trait."
            })
        }
        ("pedigree", "genotypes") => note(match p.s("genotypes") {
            "Answers" => "What students can work out from the pedigree, with a blank where an allele can\u{2019}t be told (A_).",
            "Blanks" => "A line under each person for students to write the genotype.",
            _ => "No genotypes.",
        }),

        ("heating-cooling-curve", "substance") => super::curves::substance_note(p).and_then(note),
        ("line-spectrum", "brightness") => match p.s("brightness") {
            "All equal" => note("Every line drawn equally bright, so only where they are matters."),
            _ => note("Element lines as bright as NIST lists them against the element\u{2019}s strongest (roughly), faint ones never fainter than 30%. Your own lines are all full strength."),
        },
        ("heating-cooling-curve", "scale") => note("How long each segment runs along the x-axis."),

        ("length-reading", "marks") if p.s("units").starts_with("Imperial") => note("Lengths land on a mark and are read as fractions of an inch."),
        ("length-reading", "marks") if p.s("ruler").trim_end_matches(" cm").parse::<f64>().is_ok_and(|n| n < 50.0) => {
            note("5 cm and 10 cm marks are for the 50 cm and 100 cm rulers.")
        }
        ("length-reading", "mag_show") if p.s("mag_show") != "Ruler only" => note("One on the object’s right end, and one on its left end when that isn’t lined up with 0."),

        ("ph-reading", "ph") => match p.s("instrument") {
            "Analog meter" => note("Marked every 0.2, read to 0.01 with the estimated digit."),
            "pH paper" => note("Read to the whole number of the matching color. Print in color: in black and white the strip can't be matched to the chart."),
            _ => None,
        },

        ("lewis-structures", "formula") if p.s("formula").trim().is_empty() && p.s("preset").starts_with("None") => note("Type a formula first."),
        ("lewis-structures", "shape") => note(super::lewis::shape_note(p)),
        ("lewis-structures", "bond_style") => note(super::lewis::bonds_note(p)),
        ("lewis-structures", "question") => note(super::lewis::question_note(p)),
        ("lewis-structures", "central") if p.s("resonance") != "Show one" => note("Changes need one structure. Show one resonance structure to change it."),
        ("lewis-structures", "central") if p.s("question") != "Full structure" => {
            note("Changes need the full structure. Set Question to Full structure to make a “find the mistake” question.")
        }

        ("line-spectrum", "label_style") => note("Your own strips and mixtures show the names you give them. Leave a name empty for no label."),
        ("line-spectrum", "title") => note("The answer key names what’s in each mixture, and the elements when their labels are blank. Add a mixture or blank the labels to use it."),

        ("orbital-diagram", "edits") if p.b("blank") => note("Electrons are hidden, so the figure shows empty orbitals."),
        ("orbital-diagram", "title") => note("The answer key always gives the symbol and configuration."),

        ("photoelectron-spectrum", "compare") => note("Binding energies are the free atom’s, from the textbook table (H to Ca) and Lotz (1970) for Sc to Xe."),
        ("photoelectron-spectrum", "scale") if p.s("scale") == "Logarithmic" => note("Each step along the axis is ten times the energy, so core and valence peaks both fit."),
        ("photoelectron-spectrum", "numbered") => note("The answer key always names the element and its configuration."),
        ("photoelectron-spectrum", "units") => note("1 MJ/mol is 10.36 eV. Binding energy falls from left to right, as AP draws it."),

        ("particle-diagram", "layout") if p.s("layout") == "Lattice" => note("Atoms or ions packed in a grid, for a solid."),
        ("particle-diagram", "boxes") if p.s("boxes") == "Before and after" => {
            note("The same kinds of particle in both, each with a count before and after, for a reaction or a change of state.")
        }
        ("particle-diagram", "key_list") if p.s("key_list") == "Each atom" => note("Each different atom once, for students to write formulas from."),
        _ => None,
    }
}

/// The note under the Lewis page's Answer key switch.
pub fn lewis_answer_note(p: &Params) -> &'static str { super::lewis::answer_note(p) }
