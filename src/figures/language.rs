//! Language and humanities figures (the Language and humanities tab): a plot arc, graphic
//! organisers (T-chart, KWL, Venn), a dated timeline, a cause-and-effect web, a family tree, a
//! sentence diagram and a boxes-and-arrows structure chart. Original drawings on the shared pieces
//! (`svg::Doc`, `Fills`, `show_answers`, print rules). What the teacher supplies (the events, the
//! items, the sentence) is the question; the stage names, the sorting, the dates or the parts of
//! speech are the answers, so a student copy (Show answers off) is the same size as its key.
use super::{Example, FigureDef, Params};

mod causeweb;
mod family;
mod organiser;
mod plotarc;
mod sentence;
mod structure;
mod timeline;

pub fn defs() -> Vec<FigureDef> {
    vec![plotarc::def(), organiser::def(), timeline::def(), causeweb::def(), family::def(), sentence::def(), structure::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    organiser::relevant(fig, key, p) && timeline::relevant(fig, key, p) && family::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    plotarc::examples(id)
        .or_else(|| organiser::examples(id))
        .or_else(|| timeline::examples(id))
        .or_else(|| causeweb::examples(id))
        .or_else(|| family::examples(id))
        .or_else(|| sentence::examples(id))
        .or_else(|| structure::examples(id))
}

/// Split text into lines of at most `n` characters at spaces (a longer word is cut), keeping at most
/// `max` lines; the last kept line ends in an ellipsis when text was dropped.
pub fn wrap(s: &str, n: usize, max: usize) -> Vec<String> {
    let n = n.max(4);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let mut word: String = word.to_string();
        while word.chars().count() > n {
            let head: String = word.chars().take(n).collect();
            word = word.chars().skip(n).collect();
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            lines.push(head);
        }
        if cur.is_empty() {
            cur = word;
        } else if cur.chars().count() + 1 + word.chars().count() <= n {
            cur.push(' ');
            cur.push_str(&word);
        } else {
            lines.push(std::mem::replace(&mut cur, word));
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if max == 0 {
        return Vec::new();
    }
    if lines.len() > max {
        lines.truncate(max);
        let last = lines.last_mut().unwrap();
        *last = format!("{}\u{2026}", last.chars().take(n.saturating_sub(1)).collect::<String>());
    }
    lines
}

/// A fixed shuffle of `0..n` that mixes neighbours up, so a list to sort is not already in order.
pub fn shuffled(n: usize) -> Vec<usize> {
    let gcd = |mut a: usize, mut b: usize| { while b != 0 { (a, b) = (b, a % b); } a };
    let k = [11usize, 7, 5, 3].into_iter().find(|k| gcd(*k, n.max(1)) == 1).unwrap_or(1);
    (0..n).map(|j| (j * k + 1) % n.max(1)).collect()
}

/// The non-empty trimmed lines of a text, each cut to `len` characters.
pub fn lines_of(s: &str, len: usize) -> Vec<String> {
    s.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| l.chars().take(len).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_at_spaces_and_cuts() {
        assert_eq!(wrap("the quick brown fox", 9, 5), vec!["the quick", "brown fox"]);
        assert_eq!(wrap("abcdefghij", 4, 5), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a b c d e f", 3, 2), vec!["a b", "c d\u{2026}"]);
        assert!(wrap("", 10, 3).is_empty());
        assert!(wrap("   ", 10, 3).is_empty());
        assert_eq!(wrap("x", 0, 0), Vec::<String>::new());
    }

    #[test]
    fn shuffle_is_a_permutation() {
        for n in 0..=24 {
            let mut s = shuffled(n);
            s.sort();
            assert_eq!(s, (0..n).collect::<Vec<_>>());
        }
    }
}
