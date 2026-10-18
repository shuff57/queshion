//! The pedigree as an editable structure: a tree rooted at the founder couple. Every person who
//! is born into the family is a `Node` (`me`), optionally with one partner who married in
//! (`mate`) and their children (`kids`, left to right). People are addressed by their pre-order
//! index (me, mate, then each child's subtree), the same order the renderer draws them in.
//!
//! Stored in the `family` param as compact text, e.g. `1:m1-fC1(mA1-<f,f)`:
//! person = sex `m`/`f`/`u`, then flags `A` affected, `C` carrier, `d` deceased, `p` proband,
//! `t`/`T` fraternal/identical twin of the next sibling, and a digit for the disease alleles
//! (bit 1 = first allele, bit 2 = second); node = person, then `-` (or `=` when the partners
//! are related), `<` when the partner is drawn on the left, the partner, then `(children)`.

/// Most generations a family can have (the "Child" button stops there, as on the reference).
pub const MAX_GENS: usize = 4;
/// Guard against pasted junk.
const MAX_PEOPLE: usize = 160;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Person {
    /// 0 male (square), 1 female (circle), 2 unknown (diamond).
    pub sex: u8,
    /// 0 unaffected, 1 carrier, 2 affected.
    pub status: u8,
    pub dead: bool,
    pub proband: bool,
    /// Twin of the next sibling: 0 no, 1 fraternal, 2 identical.
    pub twin: u8,
    /// Disease alleles (autosomal both slots; X-linked [X1, X2]; Y-linked [_, Y]).
    pub a: [u8; 2],
}

impl Person {
    pub fn new(sex: u8) -> Person {
        Person { sex, ..Default::default() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub me: Person,
    pub mate: Option<Person>,
    pub mate_left: bool,
    /// The partners are related (consanguinity: a double line between them).
    pub related: bool,
    pub kids: Vec<Node>,
}

impl Node {
    pub fn single(me: Person) -> Node {
        Node { me, mate: None, mate_left: false, related: false, kids: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Family {
    pub root: Node,
}

/// Where a person sits: the path of child indexes from the founder to their node, and whether
/// they are that node's partner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loc {
    pub path: Vec<usize>,
    pub mate: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// I-1, the person the family grows from (cannot be removed).
    Founder,
    /// A partner from outside the family (no parents drawn).
    MarriedIn,
    /// Born into the family.
    Blood,
}

/// What the selected-person panel may offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    pub role: Role,
    pub has_mate: bool,
    pub has_kids: bool,
    pub child: bool,
    pub partner: bool,
    pub sibling: bool,
    pub remove: bool,
    /// "Unknown" sex is only for someone without a partner.
    pub unknown: bool,
    /// "Twin of the next sibling" applies (a next sibling exists and this one is not already the
    /// second of a twin pair).
    pub twin: bool,
}

fn opposite(sex: u8) -> u8 {
    if sex == 1 { 0 } else { 1 }
}

impl Family {
    /// Every person's location in pre-order (the index order).
    pub fn locs(&self) -> Vec<Loc> {
        fn walk(n: &Node, path: &mut Vec<usize>, out: &mut Vec<Loc>) {
            out.push(Loc { path: path.clone(), mate: false });
            if n.mate.is_some() {
                out.push(Loc { path: path.clone(), mate: true });
            }
            for (i, k) in n.kids.iter().enumerate() {
                path.push(i);
                walk(k, path, out);
                path.pop();
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut Vec::new(), &mut out);
        out
    }

    pub fn len(&self) -> usize {
        fn n(x: &Node) -> usize {
            1 + x.mate.is_some() as usize + x.kids.iter().map(n).sum::<usize>()
        }
        n(&self.root)
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Number of generations.
    pub fn depth(&self) -> usize {
        fn d(x: &Node) -> usize {
            1 + x.kids.iter().map(d).max().unwrap_or(0)
        }
        d(&self.root)
    }

    pub fn locate(&self, idx: usize) -> Option<Loc> {
        self.locs().into_iter().nth(idx)
    }

    pub fn index_of(&self, loc: &Loc) -> Option<usize> {
        self.locs().iter().position(|l| l == loc)
    }

    pub fn node(&self, path: &[usize]) -> Option<&Node> {
        let mut n = &self.root;
        for &i in path {
            n = n.kids.get(i)?;
        }
        Some(n)
    }

    pub fn node_mut(&mut self, path: &[usize]) -> Option<&mut Node> {
        let mut n = &mut self.root;
        for &i in path {
            n = n.kids.get_mut(i)?;
        }
        Some(n)
    }

    /// People in index order.
    pub fn people(&self) -> Vec<&Person> {
        fn walk<'a>(n: &'a Node, out: &mut Vec<&'a Person>) {
            out.push(&n.me);
            if let Some(m) = &n.mate {
                out.push(m);
            }
            for k in &n.kids {
                walk(k, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }

    pub fn person(&self, idx: usize) -> Option<&Person> {
        let l = self.locate(idx)?;
        let n = self.node(&l.path)?;
        if l.mate { n.mate.as_ref() } else { Some(&n.me) }
    }

    pub fn person_mut(&mut self, idx: usize) -> Option<&mut Person> {
        let l = self.locate(idx)?;
        let n = self.node_mut(&l.path)?;
        if l.mate { n.mate.as_mut() } else { Some(&mut n.me) }
    }

    pub fn caps(&self, idx: usize) -> Option<Caps> {
        let l = self.locate(idx)?;
        let n = self.node(&l.path)?;
        let role = if l.mate {
            Role::MarriedIn
        } else if l.path.is_empty() {
            Role::Founder
        } else {
            Role::Blood
        };
        let has_mate = n.mate.is_some();
        let twin = role == Role::Blood && {
            let (par, i) = (&l.path[..l.path.len() - 1], l.path[l.path.len() - 1]);
            let sibs = &self.node(par)?.kids;
            i + 1 < sibs.len() && !(i > 0 && sibs[i - 1].me.twin > 0)
        };
        Some(Caps {
            role,
            has_mate,
            has_kids: !n.kids.is_empty(),
            child: l.path.len() + 1 < MAX_GENS,
            partner: !l.mate && !has_mate,
            sibling: role == Role::Blood,
            remove: role != Role::Founder,
            unknown: !has_mate,
            twin,
        })
    }

    /// Gives a born-in person without a partner one, of the opposite sex, on the outside of
    /// their sibship (left for a first child, as the generator draws it). Returns the partner.
    pub fn add_partner(&mut self, idx: usize) -> Option<usize> {
        let l = self.locate(idx)?;
        if l.mate {
            return None;
        }
        let first = l.path.last() == Some(&0);
        let n = self.node_mut(&l.path)?;
        if n.mate.is_some() {
            return None;
        }
        n.mate = Some(Person::new(opposite(n.me.sex)));
        n.mate_left = first;
        self.index_of(&Loc { path: l.path, mate: true })
    }

    /// Adds a child (on the right) to the couple the person belongs to; someone single gets a
    /// partner first. The child is the opposite sex of the last one (a girl first). Returns the
    /// child, or None past the last generation.
    pub fn add_child(&mut self, idx: usize) -> Option<usize> {
        let l = self.locate(idx)?;
        if l.path.len() + 1 >= MAX_GENS {
            return None;
        }
        if !l.mate && self.node(&l.path)?.mate.is_none() {
            self.add_partner(idx)?;
        }
        let n = self.node_mut(&l.path)?;
        let sex = n.kids.last().map(|k| opposite(k.me.sex)).unwrap_or(1);
        n.kids.push(Node::single(Person::new(sex)));
        let k = n.kids.len() - 1;
        let mut path = l.path;
        path.push(k);
        self.index_of(&Loc { path, mate: false })
    }

    /// Adds a brother or sister (the opposite sex) right after a born-in person (after their
    /// twin, if they are the first of a pair). Returns the new sibling.
    pub fn add_sibling(&mut self, idx: usize) -> Option<usize> {
        let l = self.locate(idx)?;
        if l.mate || l.path.is_empty() {
            return None;
        }
        let (par, i) = (l.path[..l.path.len() - 1].to_vec(), l.path[l.path.len() - 1]);
        let pn = self.node_mut(&par)?;
        let me = &pn.kids[i].me;
        let at = if me.twin > 0 && i + 1 < pn.kids.len() { i + 2 } else { i + 1 };
        let sex = opposite(me.sex);
        pn.kids.insert(at, Node::single(Person::new(sex)));
        let mut path = par;
        path.push(at);
        self.index_of(&Loc { path, mate: false })
    }

    /// Removes someone as the reference does: a born-in person goes with their partner and
    /// descendants; a partner who married in goes with their children. The founder stays.
    pub fn remove(&mut self, idx: usize) -> bool {
        let Some(l) = self.locate(idx) else { return false };
        if l.mate {
            let Some(n) = self.node_mut(&l.path) else { return false };
            n.mate = None;
            n.mate_left = false;
            n.related = false;
            n.kids.clear();
            return true;
        }
        if l.path.is_empty() {
            return false;
        }
        let (par, i) = (&l.path[..l.path.len() - 1], l.path[l.path.len() - 1]);
        let Some(pn) = self.node_mut(par) else { return false };
        pn.kids.remove(i);
        // the earlier sibling was this one's twin: the pair is gone
        if i > 0 {
            pn.kids[i - 1].me.twin = 0;
        }
        true
    }

    /// The "Partners are related" box of either member of a couple.
    pub fn set_related(&mut self, idx: usize, on: bool) -> bool {
        let Some(l) = self.locate(idx) else { return false };
        match self.node_mut(&l.path) {
            Some(n) if n.mate.is_some() => {
                n.related = on;
                true
            }
            _ => false,
        }
    }

    pub fn related(&self, idx: usize) -> bool {
        self.locate(idx).and_then(|l| self.node(&l.path).map(|n| n.related && n.mate.is_some())).unwrap_or(false)
    }

    // ------------------------------------------------------------ text form

    pub fn to_text(&self) -> String {
        fn person(s: &mut String, p: &Person) {
            s.push(['m', 'f', 'u'][p.sex.min(2) as usize]);
            match p.status {
                2 => s.push('A'),
                1 => s.push('C'),
                _ => {}
            }
            if p.dead {
                s.push('d');
            }
            if p.proband {
                s.push('p');
            }
            match p.twin {
                1 => s.push('t'),
                2 => s.push('T'),
                _ => {}
            }
            let bits = p.a[0].min(1) | (p.a[1].min(1) << 1);
            if bits > 0 {
                s.push((b'0' + bits) as char);
            }
        }
        fn node(s: &mut String, n: &Node) {
            person(s, &n.me);
            if let Some(m) = &n.mate {
                s.push(if n.related { '=' } else { '-' });
                if n.mate_left {
                    s.push('<');
                }
                person(s, m);
            }
            if !n.kids.is_empty() {
                s.push('(');
                for (i, k) in n.kids.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    node(s, k);
                }
                s.push(')');
            }
        }
        let mut s = String::from("1:");
        node(&mut s, &self.root);
        s
    }

    /// Reads the text form; None for anything malformed (the figure then falls back to the
    /// generated family).
    pub fn parse(text: &str) -> Option<Family> {
        let body = text.trim().strip_prefix("1:")?;
        let b: Vec<char> = body.chars().filter(|c| !c.is_whitespace()).collect();
        let mut pos = 0usize;
        let mut count = 0usize;
        fn person(b: &[char], pos: &mut usize) -> Option<Person> {
            let sex = match b.get(*pos)? {
                'm' => 0,
                'f' => 1,
                'u' => 2,
                _ => return None,
            };
            *pos += 1;
            let mut p = Person::new(sex);
            while let Some(&c) = b.get(*pos) {
                match c {
                    'A' => p.status = 2,
                    'C' => p.status = 1,
                    'd' => p.dead = true,
                    'p' => p.proband = true,
                    't' => p.twin = 1,
                    'T' => p.twin = 2,
                    '1'..='3' => {
                        let v = c as u8 - b'0';
                        p.a = [v & 1, (v >> 1) & 1];
                    }
                    _ => break,
                }
                *pos += 1;
            }
            Some(p)
        }
        fn node(b: &[char], pos: &mut usize, depth: usize, count: &mut usize) -> Option<Node> {
            if depth > MAX_GENS {
                return None;
            }
            let me = person(b, pos)?;
            *count += 1;
            let mut n = Node::single(me);
            if let Some(&c) = b.get(*pos)
                && (c == '-' || c == '=')
            {
                *pos += 1;
                n.related = c == '=';
                if b.get(*pos) == Some(&'<') {
                    n.mate_left = true;
                    *pos += 1;
                }
                n.mate = Some(person(b, pos)?);
                *count += 1;
            }
            if b.get(*pos) == Some(&'(') {
                *pos += 1;
                loop {
                    n.kids.push(node(b, pos, depth + 1, count)?);
                    match b.get(*pos)? {
                        ',' => *pos += 1,
                        ')' => {
                            *pos += 1;
                            break;
                        }
                        _ => return None,
                    }
                }
            }
            if *count > MAX_PEOPLE {
                return None;
            }
            Some(n)
        }
        let root = node(&b, &mut pos, 0, &mut count)?;
        if pos != b.len() {
            return None;
        }
        Some(Family { root })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fam(t: &str) -> Family {
        Family::parse(t).unwrap_or_else(|| panic!("parse {t}"))
    }

    #[test]
    fn text_round_trips() {
        for t in ["1:m", "1:m-f", "1:m1-fC1(mA1-<f,fdpt,m)", "1:uA=<mCdp3(f-m(m,f(mT,f)))"] {
            let f = fam(t);
            assert_eq!(f.to_text(), t);
            assert_eq!(Family::parse(&f.to_text()), Some(f));
        }
        for bad in ["", "m-f", "1:", "1:x", "1:m-f(", "1:m-f(m,)", "1:m-f)m", "1:m(f(m(f(m(f)))))"] {
            assert!(Family::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn indexes_follow_preorder() {
        let f = fam("1:m-f(m-<f(f),f,m-f)");
        assert_eq!(f.len(), 8);
        assert_eq!(f.depth(), 3);
        let l = f.locs();
        assert_eq!(l[0], Loc { path: vec![], mate: false });
        assert_eq!(l[1], Loc { path: vec![], mate: true });
        assert_eq!(l[2], Loc { path: vec![0], mate: false });
        assert_eq!(l[3], Loc { path: vec![0], mate: true });
        assert_eq!(l[4], Loc { path: vec![0, 0], mate: false });
        assert_eq!(l[5], Loc { path: vec![1], mate: false });
        for i in 0..f.len() {
            assert_eq!(f.index_of(&l[i]), Some(i));
        }
        assert_eq!(f.person(4).unwrap().sex, 1);
    }

    #[test]
    fn caps_match_the_reference_rules() {
        let f = fam("1:m-f(mt-<f(f,m),f,m)");
        let c = |i| f.caps(i).unwrap();
        // founder: child only, cannot be removed
        assert_eq!((c(0).role, c(0).child, c(0).partner, c(0).sibling, c(0).remove, c(0).unknown), (Role::Founder, true, false, false, false, false));
        // married in: child and remove only
        assert_eq!((c(1).role, c(1).partner, c(1).sibling, c(1).remove), (Role::MarriedIn, false, false, true));
        // born in with partner: twin of the next sibling offered, partner not
        assert!(c(2).twin && !c(2).partner && c(2).sibling && !c(2).unknown);
        // III-1: next sibling exists
        assert!(c(4).twin && c(4).partner && c(4).unknown && c(4).child);
        assert!(!c(5).twin, "last of the sibship");
        // II-2 is already the second of a twin pair
        assert!(!c(6).twin);
        // a fourth generation cannot have children
        let g = fam("1:m-f(m-f(m-f(f)))");
        assert!(!g.caps(6).unwrap().child && g.caps(4).unwrap().child);
    }

    #[test]
    fn add_child_partner_sibling() {
        let mut f = fam("1:m-f(m,f)");
        // a child of a single person brings a partner (left for a first child) and is a girl
        let k = f.add_child(2).unwrap();
        assert_eq!(f.to_text(), "1:m-f(m-<f(f),f)");
        assert_eq!(f.locate(k).unwrap(), Loc { path: vec![0, 0], mate: false });
        // the next child is the opposite sex of the last
        let k2 = f.add_child(3).unwrap();
        assert_eq!(f.person(k2).unwrap().sex, 0);
        assert_eq!(f.to_text(), "1:m-f(m-<f(f,m),f)");
        // a partner for the last child goes on the right
        let p = f.add_partner(6).unwrap();
        assert_eq!(f.to_text(), "1:m-f(m-<f(f,m),f-m)");
        assert!(f.caps(p).unwrap().role == Role::MarriedIn);
        assert!(f.add_partner(6).is_none(), "one partner each");
        // a sibling goes right after, opposite sex
        let s = f.add_sibling(2).unwrap();
        assert_eq!(f.to_text(), "1:m-f(m-<f(f,m),f,f-m)");
        assert_eq!(f.locate(s).unwrap().path, vec![1]);
        assert!(f.add_sibling(0).is_none() && f.add_sibling(1).is_none(), "founders have no siblings");
        // after a twin pair, not between it
        let mut t = fam("1:m-f(mt,f,m)");
        let s = t.add_sibling(2).unwrap();
        assert_eq!(t.to_text(), "1:m-f(mt,f,f,m)");
        assert_eq!(t.locate(s).unwrap().path, vec![2]);
        // no children past the fourth generation
        let mut g = fam("1:m-f(m-f(m-f(f)))");
        assert!(g.add_child(6).is_none());
    }

    #[test]
    fn remove_follows_the_reference() {
        let base = fam("1:m-f(m-<f(f,m),ft,m-f(m))");
        // a born-in person goes with partner and children
        let mut f = base.clone();
        assert!(f.remove(2));
        assert_eq!(f.to_text(), "1:m-f(ft,m-f(m))");
        // a partner who married in goes with the children
        let mut f = base.clone();
        assert!(f.remove(3));
        assert_eq!(f.to_text(), "1:m-f(m,ft,m-f(m))");
        // removing the second twin clears the pair
        let mut f = base.clone();
        let i = f.index_of(&Loc { path: vec![2], mate: false }).unwrap();
        assert!(f.remove(i));
        assert_eq!(f.to_text(), "1:m-f(m-<f(f,m),f)");
        // the founder stays; the founder's partner can go (with everyone below)
        let mut f = base.clone();
        assert!(!f.remove(0));
        assert!(f.remove(1));
        assert_eq!(f.to_text(), "1:m");
    }

    #[test]
    fn related_flag_belongs_to_the_couple() {
        let mut f = fam("1:m-f(m,f)");
        assert!(f.set_related(1, true));
        assert!(f.related(0) && f.related(1));
        assert_eq!(f.to_text(), "1:m=f(m,f)");
        assert!(!f.set_related(2, true), "a single person has no couple");
    }
}
