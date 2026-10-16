//! Pedigree generator. A seeded random family is simulated under the chosen inheritance
//! pattern (so affected/carrier status really follows Mendelian rules), laid out as a tree
//! with roman-numeral generations, numbering and a key.
//!
//! "Changes" syntax: comma-separated `II-3=affected`, `III-2=carrier`, `II-1=unaffected`
//! (person = generation roman numeral, dash, number counted left to right).
use super::*;

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("inherit", "Inheritance", &["Autosomal dominant", "Autosomal recessive", "X-linked recessive", "X-linked dominant", "Y-linked"], "Autosomal recessive"),
        ParamDef::int("gens", "Generations", 2, 4, 3),
        ParamDef::choice("size", "Family size", &["Small", "Medium", "Large"], "Medium"),
        ParamDef::int("seed", "New pedigree (change the number for a different family)", 1, 9999, 3),
        ParamDef::text("changes", "Changes, e.g. II-3=affected, III-2=carrier", ""),
        ParamDef::bool("carriers", "Show carriers (dot in the symbol)", false),
        ParamDef::bool("gen_labels", "Generation labels (I, II, III)", true),
        ParamDef::bool("numbers", "Individual numbers", true),
        ParamDef::bool("key", "Key", true),
        ParamDef::text("title", "Chart title (blank = none)", ""),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode { AD, AR, XR, XD, Y }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status { Unaffected, Carrier, Affected }

#[derive(Clone)]
struct Person {
    male: bool,
    /// disease-allele flags: autosomal both slots; X-linked [X1, X2 or Y(unused)]; Y-linked [_, Y]
    a: [u8; 2],
    gn: usize,
    x: f64,
    status: Status,
}

struct Node {
    person: usize,
    mate: Option<usize>,
    kids: Vec<Node>,
    mate_left: bool,
    width: f64,
}

fn status_of(pe: &Person, m: Mode) -> Status {
    let n = pe.a[0] + pe.a[1];
    match m {
        Mode::AD => if n >= 1 { Status::Affected } else { Status::Unaffected },
        Mode::AR => match n { 2 => Status::Affected, 1 => Status::Carrier, _ => Status::Unaffected },
        Mode::XR => if pe.male { if pe.a[0] == 1 { Status::Affected } else { Status::Unaffected } } else { match n { 2 => Status::Affected, 1 => Status::Carrier, _ => Status::Unaffected } },
        Mode::XD => if pe.male { if pe.a[0] == 1 { Status::Affected } else { Status::Unaffected } } else if n >= 1 { Status::Affected } else { Status::Unaffected },
        Mode::Y => if pe.male && pe.a[1] == 1 { Status::Affected } else { Status::Unaffected },
    }
}

struct Sim {
    people: Vec<Person>,
    mode: Mode,
    rng: Rng,
}

impl Sim {
    fn add(&mut self, male: bool, a: [u8; 2], gn: usize) -> usize {
        let mut pe = Person { male, a, gn, x: 0.0, status: Status::Unaffected };
        pe.status = status_of(&pe, self.mode);
        self.people.push(pe);
        self.people.len() - 1
    }

    fn mate_of(&mut self, who: usize) -> usize {
        let male = !self.people[who].male;
        let gn = self.people[who].gn;
        let a = match self.mode {
            Mode::AR => if self.rng.f() < 0.2 { [1, 0] } else { [0, 0] },
            _ => [0, 0],
        };
        self.add(male, a, gn)
    }

    fn child(&mut self, mom: usize, dad: usize, gn: usize) -> usize {
        let male = self.rng.f() < 0.5;
        let (m, f) = (self.people[mom].clone(), self.people[dad].clone());
        let a = match self.mode {
            Mode::AD | Mode::AR => {
                [m.a[self.rng.below(2)], f.a[self.rng.below(2)]]
            }
            Mode::XR | Mode::XD => {
                let from_mom = m.a[self.rng.below(2)];
                if male { [from_mom, 0] } else { [from_mom, f.a[0]] }
            }
            Mode::Y => if male { [0, f.a[1]] } else { [0, 0] },
        };
        self.add(male, a, gn)
    }

    fn build(&mut self, person: usize, mate: usize, gn: usize, gens: usize, counts: &[usize], mate_left: bool) -> Node {
        let (mom, dad) = if self.people[person].male { (mate, person) } else { (person, mate) };
        let n = counts[gn.min(counts.len() - 1)];
        let mut kids = Vec::new();
        for i in 0..n {
            let c = self.child(mom, dad, gn + 1);
            if gn + 1 + 1 < gens {
                let m = self.mate_of(c);
                kids.push(self.build(c, m, gn + 1, gens, counts, i == 0));
            } else {
                kids.push(Node { person: c, mate: None, kids: vec![], mate_left: false, width: 1.0 });
            }
        }
        let sum: f64 = kids.iter().map(|k| k.width).sum();
        Node { person, mate: Some(mate), kids, mate_left, width: sum.max(2.0) }
    }
}

fn simulate(mode: Mode, gens: usize, size: &str, seed: u64) -> (Sim, Node) {
    let counts: Vec<usize> = match (size, gens) {
        ("Small", _) => vec![2, 2, 2],
        ("Large", 4) => vec![3, 3, 2],
        ("Large", _) => vec![4, 3, 3],
        _ => vec![3, 2, 2],
    };
    let mut best: Option<(Sim, Node)> = None;
    for attempt in 0..60u64 {
        let mut sim = Sim { people: vec![], mode, rng: Rng::new(seed * 1000 + attempt) };
        let (fa, mo) = match mode {
            Mode::AD => if sim.rng.f() < 0.5 { ([1, 0], [0, 0]) } else { ([0, 0], [1, 0]) },
            Mode::AR => ([1, 0], [1, 0]),
            Mode::XR => ([0, 0], [1, 0]),
            Mode::XD => if sim.rng.f() < 0.5 { ([1, 0], [0, 0]) } else { ([0, 0], [1, 0]) },
            Mode::Y => ([0, 1], [0, 0]),
        };
        let f = sim.add(true, fa, 0);
        let m = sim.add(false, mo, 0);
        let root = sim.build(f, m, 0, gens, &counts, false);
        let last = sim.people.iter().filter(|p| p.gn == gens - 1);
        let (mut tot, mut aff) = (0, 0);
        for p in last { tot += 1; if p.status == Status::Affected { aff += 1; } }
        let ok = aff >= 1 && aff < tot;
        if best.is_none() || ok { best = Some((sim, root)); }
        if ok { break; }
    }
    best.unwrap()
}

fn place(sim: &mut Sim, n: &Node, left: f64) {
    let c = left + n.width / 2.0;
    match n.mate {
        Some(m) => {
            // root couple: male left
            let (a, b) = (c - 0.5, c + 0.5);
            let (px, mx) = if n.mate_left { (b, a) } else { (a, b) };
            sim.people[n.person].x = px;
            sim.people[m].x = mx;
        }
        None => sim.people[n.person].x = c,
    }
    let sum: f64 = n.kids.iter().map(|k| k.width).sum();
    let mut x = left + (n.width - sum) / 2.0;
    for k in &n.kids {
        place(sim, k, x);
        x += k.width;
    }
}

fn roman(i: usize) -> &'static str { ["I", "II", "III", "IV", "V"][i.min(4)] }

fn symbol(d: &mut Doc, cx: f64, cy: f64, r: f64, male: bool, st: Status, carriers: bool) {
    let fill = if st == Status::Affected { INK } else { "#fff" };
    if male { d.rect(cx - r, cy - r, 2.0 * r, 2.0 * r, fill, INK, 2.2); } else { d.circle(cx, cy, r, fill, INK, 2.2); }
    if st == Status::Carrier && carriers { d.circle(cx, cy, r * 0.28, INK, INK, 1.0); }
}

pub fn render(p: &Params) -> String {
    let mode = match p.s("inherit") {
        "Autosomal dominant" => Mode::AD,
        "X-linked recessive" => Mode::XR,
        "X-linked dominant" => Mode::XD,
        "Y-linked" => Mode::Y,
        _ => Mode::AR,
    };
    let gens = p.i("gens").clamp(2, 4) as usize;
    let (mut sim, root) = simulate(mode, gens, p.s("size"), p.i("seed").max(1) as u64);
    place(&mut sim, &root, 0.0);

    // numbering per generation by x
    let mut num = vec![0usize; sim.people.len()];
    let mut order: Vec<Vec<usize>> = vec![vec![]; gens];
    for (i, pe) in sim.people.iter().enumerate() { order[pe.gn].push(i); }
    for o in order.iter_mut() {
        o.sort_by(|a, b| sim.people[*a].x.partial_cmp(&sim.people[*b].x).unwrap());
        for (k, &i) in o.iter().enumerate() { num[i] = k + 1; }
    }
    // changes
    let mut notes = Vec::new();
    for tok in p.s("changes").split([',', ';']) {
        let Some((who, what)) = tok.split_once('=') else { continue };
        let Some((g, n)) = who.trim().split_once('-') else { continue };
        let Some(gi) = ["I", "II", "III", "IV"].iter().position(|r| r.eq_ignore_ascii_case(g.trim())) else { continue };
        let Ok(n) = n.trim().parse::<usize>() else { continue };
        let Some(&pi) = order.get(gi).and_then(|o| o.get(n.wrapping_sub(1))) else { continue };
        let st = match what.trim().to_lowercase().as_str() {
            "affected" | "a" => Status::Affected,
            "carrier" | "c" => Status::Carrier,
            "unaffected" | "u" | "normal" => Status::Unaffected,
            _ => continue,
        };
        sim.people[pi].status = st;
        notes.push(pi);
    }

    let slot = 74.0;
    let r = 18.0;
    let gen_h = 112.0;
    let width_slots = root.width;
    let title = p.s("title").trim().to_string();
    let left = 56.0 + if p.b("gen_labels") { 18.0 } else { 0.0 };
    let tree_w = width_slots * slot;
    let key_on = p.b("key");
    let show_car = p.b("carriers");
    let any_car = sim.people.iter().any(|q| q.status == Status::Carrier) && show_car;
    let key_items: Vec<(bool, Status, &str)> = {
        let mut v = vec![(true, Status::Unaffected, "Unaffected male"), (true, Status::Affected, "Affected male"), (false, Status::Unaffected, "Unaffected female"), (false, Status::Affected, "Affected female")];
        if any_car {
            v.push((true, Status::Carrier, "Carrier male"));
            v.push((false, Status::Carrier, "Carrier female"));
        }
        v
    };
    let key_w = 470.0;
    let w = (left + tree_w + 50.0).max(key_w + 60.0 + 20.0).max(520.0);
    let ox = (w - tree_w - left - 50.0).max(0.0) / 2.0;
    let top = 40.0 + if title.is_empty() { 0.0 } else { 36.0 };
    let tree_bottom = top + gen_h * (gens - 1) as f64 + r + 56.0;
    let key_rows = key_items.len().div_ceil(2);
    let h = tree_bottom + if key_on { 30.0 + key_rows as f64 * 40.0 + 20.0 } else { 14.0 };
    let mut d = Doc::new(w, h);
    if !title.is_empty() { bold(&mut d, w / 2.0, 34.0, 20.0, "middle", &title); }

    let px = |q: &Person| ox + left + q.x * slot;
    let py = |g: usize| top + 20.0 + g as f64 * gen_h;

    // connectors first
    fn walk(d: &mut Doc, sim: &Sim, n: &Node, px: &dyn Fn(&Person) -> f64, py: &dyn Fn(usize) -> f64, r: f64, gen_h: f64) {
        let pa = &sim.people[n.person];
        let y = py(pa.gn);
        if let Some(m) = n.mate {
            let pm = &sim.people[m];
            let (xa, xb) = (px(pa).min(px(pm)), px(pa).max(px(pm)));
            d.line(xa + r, y, xb - r, y, INK, 2.2);
            if !n.kids.is_empty() {
                let mid = (xa + xb) / 2.0;
                let bar = y + gen_h / 2.0;
                d.line(mid, y, mid, bar, INK, 2.2);
                let xs: Vec<f64> = n.kids.iter().map(|k| px(&sim.people[k.person])).collect();
                let (lo, hi) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
                d.line(lo.min(mid), bar, hi.max(mid), bar, INK, 2.2);
                for x in xs { d.line(x, bar, x, y + gen_h - r, INK, 2.2); }
            }
        }
        for k in &n.kids { walk(d, sim, k, px, py, r, gen_h); }
    }
    walk(&mut d, &sim, &root, &px, &py, r, gen_h);

    for (i, q) in sim.people.iter().enumerate() {
        let (cx, cy) = (px(q), py(q.gn));
        symbol(&mut d, cx, cy, r, q.male, q.status, show_car);
        if p.b("numbers") {
            txt(&mut d, cx, cy + r + 17.0, 15.0, "middle", SANS, "", &num[i].to_string());
        }
    }
    if p.b("gen_labels") {
        for g in 0..gens {
            txt(&mut d, ox + 24.0, py(g) + 6.0, 19.0, "middle", SERIF, r#"font-weight="bold""#, roman(g));
        }
    }
    if key_on {
        let kx = (w - key_w) / 2.0;
        let ky = tree_bottom + 4.0;
        d.rect(kx, ky, key_w, 20.0 + key_rows as f64 * 40.0 + 4.0, "#fff", INK, 1.8);
        for (i, (male, st, label)) in key_items.iter().enumerate() {
            let (col, row) = (i % 2, i / 2);
            let (x, y) = (kx + 36.0 + col as f64 * 230.0, ky + 30.0 + row as f64 * 40.0);
            symbol(&mut d, x, y, 14.0, *male, *st, true);
            txt(&mut d, x + 28.0, y + 6.0, 16.0, "start", SANS, "", label);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_render() {
        render_case("pedigree", "default", &[]);
        render_case("pedigree", "xr_large", &[("inherit", s("X-linked recessive")), ("size", s("Large")), ("carriers", Value::B(true)), ("title", s("Hemophilia"))]);
        render_case("pedigree", "ad_4gen", &[("inherit", s("Autosomal dominant")), ("gens", Value::I(4)), ("size", s("Small")), ("changes", s("II-1=affected, III-2=unaffected"))]);
        render_case("pedigree", "y", &[("inherit", s("Y-linked")), ("gens", Value::I(2)), ("seed", Value::I(7))]);
    }

    #[test]
    fn genetics_consistent() {
        // autosomal recessive: every affected person has two disease alleles; unaffected parents of
        // an affected child must both be carriers
        for seed in 1..40u64 {
            let (sim, _) = simulate(Mode::AR, 3, "Medium", seed);
            for q in &sim.people { assert_eq!(q.status == Status::Affected, q.a[0] + q.a[1] == 2); }
        }
        // Y-linked: only males can be affected
        let (sim, _) = simulate(Mode::Y, 3, "Medium", 5);
        assert!(sim.people.iter().all(|q| q.male || q.status != Status::Affected));
    }
}
