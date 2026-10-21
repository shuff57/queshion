//! Music figures (the Music tab): a staff written from a text spec, the circle of fifths, a piano
//! keyboard for scales and chords, guitar chord boxes and rhythm counting. Original drawings on
//! the shared pieces (`chart`, `svg::Doc`, `Fills`, `show_answers`, print rules); the clefs,
//! accidentals, rests and noteheads are plain paths, so no music font is needed. What is given
//! (the notes, the chord name, the rhythm) is the question; names, marked keys and counts are the
//! answers, so a student copy (Show answers off) leaves them out and is the same size as its key.
use super::{Example, FigureDef, Params};

mod fifths;
mod glyphs;
mod guitar;
mod keyboard;
mod rhythm;
mod staff;

pub fn defs() -> Vec<FigureDef> {
    vec![staff::def(), fifths::def(), keyboard::def(), guitar::def(), rhythm::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    fifths::relevant(fig, key, p) && keyboard::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    staff::examples(id).or_else(|| fifths::examples(id)).or_else(|| keyboard::examples(id)).or_else(|| guitar::examples(id)).or_else(|| rhythm::examples(id))
}

/// A written pitch: letter 0..=6 (C D E F G A B), accidental -1, 0 or 1, octave 0..=9 (middle C is C4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pitch {
    pub letter: u8,
    pub acc: i8,
    pub octave: i8,
}

const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
const SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

impl Pitch {
    /// Diatonic steps above C0: every letter name is one step.
    pub fn step(&self) -> i32 {
        self.octave as i32 * 7 + self.letter as i32
    }
    /// MIDI number (middle C is 60).
    pub fn midi(&self) -> i32 {
        12 * (self.octave as i32 + 1) + SEMITONES[self.letter as usize] + self.acc as i32
    }
    /// The name without the octave, such as `F#` or `Bb`.
    pub fn name(&self) -> String {
        format!("{}{}", LETTERS[self.letter as usize], match self.acc { 1 => "#", -1 => "b", _ => "" })
    }
}

/// `C4`, `F#4`, `Bb3`; the sharp and flat signs are accepted too.
pub fn parse_pitch(tok: &str) -> Option<Pitch> {
    let mut c = tok.chars();
    let first = c.next()?.to_ascii_uppercase();
    let letter = LETTERS.iter().position(|l| *l == first)? as u8;
    let rest: String = c.collect();
    let (acc, oct) = match rest.chars().next()? {
        '#' | '\u{266f}' => (1, &rest[rest.chars().next()?.len_utf8()..]),
        'b' | '\u{266d}' => (-1, &rest[rest.chars().next()?.len_utf8()..]),
        _ => (0, rest.as_str()),
    };
    let octave: i8 = oct.parse().ok().filter(|o| (0..=9).contains(o))?;
    Some(Pitch { letter, acc, octave })
}

/// A note length: its value in quarter notes, its kind (0 whole, 1 half, 2 quarter, 3 eighth,
/// 4 sixteenth) and whether it is dotted. Written `w h q e s`, with a `.` for a dot (`q.`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dur {
    pub kind: u8,
    pub dotted: bool,
}

impl Dur {
    pub fn quarters(&self) -> f64 {
        [4.0, 2.0, 1.0, 0.5, 0.25][self.kind as usize] * if self.dotted { 1.5 } else { 1.0 }
    }
}

pub fn parse_dur(tok: &str) -> Option<Dur> {
    let t = tok.to_ascii_lowercase();
    let (body, dotted) = match t.strip_suffix('.') { Some(b) => (b, true), None => (t.as_str(), false) };
    let kind = ["w", "h", "q", "e", "s"].iter().position(|k| *k == body)? as u8;
    Some(Dur { kind, dotted })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitches_parse_and_number() {
        let p = parse_pitch("C4").unwrap();
        assert_eq!((p.step(), p.midi(), p.name()), (28, 60, "C".to_string()));
        let p = parse_pitch("F#4").unwrap();
        assert_eq!((p.midi(), p.name()), (66, "F#".to_string()));
        let p = parse_pitch("Bb3").unwrap();
        assert_eq!((p.midi(), p.name()), (58, "Bb".to_string()));
        assert_eq!(parse_pitch("c\u{266f}5").unwrap().midi(), 73);
        assert_eq!(parse_pitch("A0").unwrap().midi(), 21);
        for bad in ["", "H4", "C", "C10", "C-1", "Cx4", "4C", "#4", "C##4"] {
            assert!(parse_pitch(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn durations_parse_and_add_up() {
        assert_eq!(parse_dur("q").unwrap().quarters(), 1.0);
        assert_eq!(parse_dur("H.").unwrap().quarters(), 3.0);
        assert_eq!(parse_dur("e").unwrap().quarters(), 0.5);
        assert_eq!(parse_dur("w").unwrap().quarters(), 4.0);
        assert_eq!(parse_dur("s.").unwrap().quarters(), 0.375);
        for bad in ["", "x", "qq", ".", "3"] {
            assert!(parse_dur(bad).is_none(), "{bad:?}");
        }
    }
}
