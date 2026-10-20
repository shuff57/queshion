//! Pedigree generator. A seeded random family is simulated under the chosen inheritance
//! pattern (so affected/carrier status really follows Mendelian rules), laid out as a tree
//! with roman-numeral generations, numbering and a key. The family can then be changed by hand
//! (`family.rs`: people added, removed and edited one at a time); the edited structure is stored
//! in the `family` param and drawn instead of the generated one.
//!
//! Old "changes" syntax (the retired Overrides box, still honoured): comma-separated
//! `II-3=affected`, `III-2=carrier`, `II-1=unaffected` (person = generation roman numeral, dash,
//! number counted left to right).
use super::*;

mod family;
#[cfg(test)]
mod legacy;

pub use family::{Caps, Family, Person as Member, Role, MAX_GENS};

/// One drawn individual (for interaction handles and the person panel), in index order.
#[derive(Clone, Debug)]
pub struct Who { pub x: f64, pub y: f64, pub gi: usize, pub num: usize, pub male: bool, pub sex: u8, pub status: u8, pub r: f64 }

thread_local! {
    static LAST: std::cell::RefCell<(f64, f64, Vec<Who>)> = const { std::cell::RefCell::new((0.0, 0.0, Vec::new())) };
}

/// Renders and returns (canvas w, h, people) of the figure.
pub fn people_of(p: &Params) -> (f64, f64, Vec<Who>) {
    LAST.with(|l| *l.borrow_mut() = (0.0, 0.0, Vec::new()));
    let _ = render(p);
    LAST.with(|l| l.borrow().clone())
}

/// Whether carriers are drawn (so a click can cycle through the carrier state).
pub fn carriers_visible(p: &Params) -> bool { carrier_style(p) != "Hidden" }

/// The collapsed Symbols / Labels header's second line, worded as the reference's.
pub fn section_summary(title: &str, p: &Params) -> Option<String> {
    match title {
        "Symbols" => Some(match carrier_style(p) {
            "Hidden" => "Carriers hidden".to_string(),
            "Half filled" => "Carriers half filled".to_string(),
            _ => "Carriers shown".to_string(),
        }),
        "Labels" => {
            let on: Vec<&str> = [("gen_labels", "Generations"), ("numbers", "Numbers"), ("key", "Key")].iter().filter(|(k, _)| p.b(k)).map(|(_, l)| *l).collect();
            Some(if on.is_empty() { "None".to_string() } else { on.join(", ") })
        }
        _ => None,
    }
}

fn carrier_style(p: &Params) -> &str {
    match p.s("carrier_style") {
        "Hidden" => "Hidden",
        "Half filled" => "Half filled",
        "Center dot" => "Center dot",
        _ => if p.b("carriers") { "Center dot" } else { "Hidden" },
    }
}

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("inherit", "Inheritance", &["Autosomal dominant", "Autosomal recessive", "X-linked recessive", "X-linked dominant", "Y-linked"], "Autosomal recessive"),
        ParamDef::int("gens", "Generations", 2, 4, 3),
        ParamDef::choice("size", "Family size", &["Small", "Medium", "Large"], "Medium"),
        ParamDef::int("seed", "New pedigree (change the number for a different family)", 1, 9999, 3),
        ParamDef::list("changes", "Overrides (person = status), e.g. II-3=affected", ""),
        ParamDef::bool("carriers", "Show carriers (dot in the symbol)", false),
        ParamDef::bool("gen_labels", "Generation numerals", true).hint("I, II, III down the left."),
        ParamDef::bool("numbers", "Individual numbers", true).hint("1, 2, 3 under each person, left to right."),
        ParamDef::bool("key", "Key", true).hint("What each symbol means, below the pedigree."),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::float("symbol_size", "Symbol size", 12.0, 26.0, 1.0, 18.0),
        ParamDef::text("gen_prefix", "Generation label style (I, II, III or custom list)", ""),
        // ---- added for parity with the reference controls
        ParamDef::choice("carrier_style", "Carriers", &["Automatic", "Hidden", "Half filled", "Center dot"], "Automatic"),
        ParamDef::choice("genotypes", "Genotypes under each person", &["None", "Answers", "Blanks"], "None"),
        ParamDef::text("allele_letter", "Allele letter for genotypes", "A"),
        ParamDef::bool("answer", "Answer key", false).hint("Print the inheritance pattern under the figure."),
        // the family as changed by hand (see family.rs); empty = the generated family
        ParamDef::text("family", "Family changed by hand", ""),
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
    #[cfg_attr(not(test), allow(dead_code))]
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
    status_from(pe.a, pe.male, m)
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

#[cfg(test)]
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

pub fn roman(i: usize) -> &'static str { ["I", "II", "III", "IV", "V"][i.min(4)] }

/// `sex`: 0 male (square), 1 female (circle), 2 unknown (diamond). `car`: carrier style.
fn symbol_ex(d: &mut Doc, cx: f64, cy: f64, r: f64, sex: u8, st: Status, car: &str) {
    let fill = if st == Status::Affected { INK } else { "#fff" };
    let shape = |d: &mut Doc, fill: &str, stroke: &str, sw: f64| match sex {
        0 => { let q = r * 0.94; d.rect(cx - q, cy - q, 2.0 * q, 2.0 * q, fill, stroke, sw); }
        1 => { d.circle(cx, cy, r, fill, stroke, sw); }
        _ => { let q = r * 1.25; d.poly(&[(cx, cy - q), (cx + q, cy), (cx, cy + q), (cx - q, cy)], fill, stroke, sw); }
    };
    shape(d, fill, INK, 2.0);
    if st == Status::Carrier {
        match car {
            "Center dot" => { d.circle(cx, cy, r * 0.28, INK, INK, 1.0); }
            "Half filled" => {
                let path = match sex {
                    0 => format!("M{:.2},{:.2}H{:.2}V{:.2}H{:.2}Z", cx - r, cy - r, cx, cy + r, cx - r),
                    1 => format!("M{cx:.2},{:.2}A{r:.2},{r:.2} 0 0 0 {cx:.2},{:.2}Z", cy - r, cy + r),
                    _ => { let q = r * 1.25; format!("M{cx:.2},{:.2}L{:.2},{cy:.2}L{cx:.2},{:.2}Z", cy - q, cx - q, cy + q) }
                };
                d.path(&path, INK, "none", 0.0);
                shape(d, "none", INK, 2.0);
            }
            _ => {}
        }
    }
}

fn deceased_mark(d: &mut Doc, cx: f64, cy: f64, r: f64) {
    d.line(cx - r * 1.45, cy + r * 1.45, cx + r * 1.45, cy - r * 1.45, INK, 2.0);
}

fn proband_mark(d: &mut Doc, cx: f64, cy: f64, r: f64) {
    proband_arrow(d, cx, cy, r, (1.3, 0.55));
}

/// The key's proband arrow: a shorter tail, so it stays inside the key's border (the full-length
/// arrow of the first key column reached past the box's left edge).
fn key_proband_mark(d: &mut Doc, cx: f64, cy: f64, r: f64) {
    proband_arrow(d, cx, cy, r, (0.8, 0.423));
}

/// Arrow pointing at the person's lower left; its tail starts `tail`·r (x, y) from the tip.
fn proband_arrow(d: &mut Doc, cx: f64, cy: f64, r: f64, tail: (f64, f64)) {
    let (tx, ty) = (cx - r * 1.15, cy + r * 0.85);
    let (sx, sy) = (tx - r * tail.0, ty + r * tail.1);
    d.line(sx, sy, tx - 6.0, ty + 2.5, INK, 2.0);
    let (dx, dy) = ((tx - sx), (ty - sy));
    let l = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (dx / l, dy / l);
    d.poly(&[(tx, ty), (tx - ux * 10.0 - uy * 4.5, ty - uy * 10.0 + ux * 4.5), (tx - ux * 10.0 + uy * 4.5, ty - uy * 10.0 - ux * 4.5)], INK, INK, 1.0);
}

fn mode_of(p: &Params) -> Mode {
    match p.s("inherit") {
        "Autosomal dominant" => Mode::AD,
        "X-linked recessive" => Mode::XR,
        "X-linked dominant" => Mode::XD,
        "Y-linked" => Mode::Y,
        _ => Mode::AR,
    }
}

fn st_u8(s: Status) -> u8 {
    match s {
        Status::Unaffected => 0,
        Status::Carrier => 1,
        Status::Affected => 2,
    }
}

fn st_of(v: u8) -> Status {
    match v {
        2 => Status::Affected,
        1 => Status::Carrier,
        _ => Status::Unaffected,
    }
}

/// The random family for the current setup (inheritance, generations, family size, seed).
fn generated(p: &Params) -> Family {
    let gens = p.i("gens").clamp(2, 4) as usize;
    let (sim, root) = simulate(mode_of(p), gens, p.s("size"), p.i("seed").max(1) as u64);
    fn conv(sim: &Sim, n: &Node) -> family::Node {
        let pe = |i: usize| {
            let q = &sim.people[i];
            family::Person { sex: if q.male { 0 } else { 1 }, status: st_u8(q.status), a: q.a, ..Default::default() }
        };
        family::Node { me: pe(n.person), mate: n.mate.map(pe), mate_left: n.mate_left, related: false, kids: n.kids.iter().map(|k| conv(sim, k)).collect() }
    }
    Family { root: conv(&sim, &root) }
}

/// The family changed by hand (the `family` param), if there is a valid one.
pub fn explicit(p: &Params) -> Option<Family> {
    let t = p.s("family");
    if t.trim().is_empty() { None } else { Family::parse(t) }
}

/// Whether the family on screen was changed by hand (else it is the generated one).
pub fn hand_changed(p: &Params) -> bool {
    explicit(p).is_some()
}

/// The family that is drawn: the one changed by hand, else the generated one, with any typed
/// overrides (`changes`) applied on top.
pub fn family_of(p: &Params) -> Family {
    let mut f = explicit(p).unwrap_or_else(|| generated(p));
    apply_changes(&mut f, p.s("changes"));
    f
}

/// Stores an edited family. Typed overrides are part of it from now on, so they are cleared.
pub fn store(p: &mut Params, f: &Family) {
    p.set_s("family", &f.to_text());
    p.set_s("changes", "");
}

/// Migration of the old "Overrides" box: folds typed overrides into a stored family (drawn
/// exactly the same). Returns whether there was anything to fold.
pub fn migrate_overrides(p: &mut Params) -> bool {
    if p.s("changes").trim().is_empty() {
        return false;
    }
    let f = family_of(p);
    store(p, &f);
    true
}

/// "New pedigree": a fresh random family, dropping any changes made by hand.
pub fn new_pedigree(p: &mut Params, seed: i64) {
    p.set_i("seed", seed.max(1));
    p.set_s("changes", "");
    p.set_s("family", "");
}

/// A change of generations or family size starts a random family again (as on the reference).
pub fn drop_hand_changes(p: &mut Params) {
    p.set_s("changes", "");
    p.set_s("family", "");
}

/// The inheritance changed: a family changed by hand keeps its people and is shaded again to fit
/// the new pattern (as on the reference); a generated one is simply generated again.
pub fn reshade_params(p: &mut Params) {
    if hand_changed(p) {
        let mut f = family_of(p);
        reshade(&mut f, mode_of(p), p.i("seed").max(1) as u64);
        store(p, &f);
    }
}

/// The inheritance pattern's name, lower case ("autosomal recessive").
pub fn mode_name(p: &Params) -> String {
    p.s("inherit").to_lowercase()
}

fn status_from(a: [u8; 2], male: bool, m: Mode) -> Status {
    let n = a[0] + a[1];
    match m {
        Mode::AD => if n >= 1 { Status::Affected } else { Status::Unaffected },
        Mode::AR => match n { 2 => Status::Affected, 1 => Status::Carrier, _ => Status::Unaffected },
        Mode::XR => if male { if a[0] == 1 { Status::Affected } else { Status::Unaffected } } else { match n { 2 => Status::Affected, 1 => Status::Carrier, _ => Status::Unaffected } },
        Mode::XD => if male { if a[0] == 1 { Status::Affected } else { Status::Unaffected } } else if n >= 1 { Status::Affected } else { Status::Unaffected },
        Mode::Y => if male && a[1] == 1 { Status::Affected } else { Status::Unaffected },
    }
}

/// Shades a family (its people fixed) under `mode`: founders get the disease alleles the
/// generator gives them, partners marrying in are mostly clear, and each child draws one allele
/// from each parent. Retries until the youngest generation has some but not all affected.
fn reshade(f: &mut Family, mode: Mode, seed: u64) {
    fn child(mom: &family::Person, dad: &family::Person, male: bool, mode: Mode, rng: &mut Rng) -> [u8; 2] {
        match mode {
            Mode::AD | Mode::AR => [mom.a[rng.below(2)], dad.a[rng.below(2)]],
            Mode::XR | Mode::XD => {
                let from_mom = mom.a[rng.below(2)];
                if male { [from_mom, 0] } else { [from_mom, dad.a[0]] }
            }
            Mode::Y => if male { [0, dad.a[1]] } else { [0, 0] },
        }
    }
    fn mate(mode: Mode, rng: &mut Rng) -> [u8; 2] {
        match mode {
            Mode::AR => if rng.f() < 0.2 { [1, 0] } else { [0, 0] },
            _ => [0, 0],
        }
    }
    fn down(n: &mut family::Node, mode: Mode, rng: &mut Rng) {
        let (mom, dad) = match &n.mate {
            Some(m) if n.me.sex == 0 => (m.clone(), n.me.clone()),
            Some(m) => (n.me.clone(), m.clone()),
            None => (n.me.clone(), n.me.clone()),
        };
        for k in n.kids.iter_mut() {
            k.me.a = child(&mom, &dad, k.me.sex == 0, mode, rng);
            if let Some(km) = &mut k.mate {
                km.a = mate(mode, rng);
            }
            down(k, mode, rng);
        }
    }
    fn shade(n: &mut family::Node, mode: Mode, gn: usize, last: usize, tally: &mut (usize, usize)) {
        let mut one = |q: &mut family::Person| {
            q.status = st_u8(status_from(q.a, q.sex == 0, mode));
            if gn == last {
                tally.0 += 1;
                tally.1 += (q.status == 2) as usize;
            }
        };
        one(&mut n.me);
        if let Some(m) = &mut n.mate {
            one(m);
        }
        for k in n.kids.iter_mut() {
            shade(k, mode, gn + 1, last, tally);
        }
    }
    let last = f.depth() - 1;
    let mut best: Option<family::Node> = None;
    for attempt in 0..60u64 {
        let mut rng = Rng::new(seed.wrapping_mul(1000).wrapping_add(attempt + 500));
        let mut root = f.root.clone();
        let (fa, mo) = match mode {
            Mode::AD | Mode::XD => if rng.f() < 0.5 { ([1, 0], [0, 0]) } else { ([0, 0], [1, 0]) },
            Mode::AR => ([1, 0], [1, 0]),
            Mode::XR => ([0, 0], [1, 0]),
            Mode::Y => ([0, 1], [0, 0]),
        };
        let me_male = root.me.sex == 0;
        root.me.a = if me_male { fa } else { mo };
        if let Some(m) = &mut root.mate {
            m.a = if me_male { mo } else { fa };
        }
        down(&mut root, mode, &mut rng);
        let mut tally = (0, 0);
        shade(&mut root, mode, 0, last, &mut tally);
        let ok = tally.1 >= 1 && tally.1 < tally.0;
        if best.is_none() || ok {
            best = Some(root);
        }
        if ok {
            break;
        }
    }
    if let Some(b) = best {
        f.root = b;
    }
}

/// Where everyone is drawn: generation and x (in slots) per person in index order, plus each
/// couple's links in drawing order.
struct Lay {
    gn: Vec<usize>,
    x: Vec<f64>,
    width: f64,
    nodes: Vec<NodeIx>,
}

struct NodeIx {
    me: usize,
    mate: Option<usize>,
    related: bool,
    kids: Vec<usize>,
    twins: Vec<u8>,
}

/// Tidy layout: every couple (or single person) owns a block as wide as its children's blocks
/// together (at least its own two or one slots), sits centred over it, and the children's
/// blocks are laid side by side, so generations line up and nothing overlaps.
fn layout(f: &Family) -> Lay {
    fn width(n: &family::Node) -> f64 {
        let s: f64 = n.kids.iter().map(width).sum();
        s.max(if n.mate.is_some() { 2.0 } else { 1.0 })
    }
    fn place(n: &family::Node, left: f64, gn: usize, lay: &mut Lay) -> usize {
        let w = width(n);
        let c = left + w / 2.0;
        let me = lay.x.len();
        lay.x.push(c);
        lay.gn.push(gn);
        let mate = n.mate.as_ref().map(|_| {
            lay.x.push(c);
            lay.gn.push(gn);
            me + 1
        });
        if let Some(m) = mate {
            let (a, b) = (c - 0.5, c + 0.5);
            let (px, mx) = if n.mate_left { (b, a) } else { (a, b) };
            lay.x[me] = px;
            lay.x[m] = mx;
        }
        let ni = lay.nodes.len();
        lay.nodes.push(NodeIx { me, mate, related: n.related, kids: vec![], twins: vec![] });
        let sum: f64 = n.kids.iter().map(width).sum();
        let mut x = left + (w - sum) / 2.0;
        for k in &n.kids {
            let ki = place(k, x, gn + 1, lay);
            lay.nodes[ni].kids.push(ki);
            lay.nodes[ni].twins.push(k.me.twin);
            x += width(k);
        }
        me
    }
    let mut lay = Lay { gn: vec![], x: vec![], width: width(&f.root), nodes: vec![] };
    place(&f.root, 0.0, 0, &mut lay);
    lay
}

/// People of each generation, left to right (their numbers are position + 1).
fn numbering(lay: &Lay, gens: usize) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut num = vec![0usize; lay.x.len()];
    let mut order: Vec<Vec<usize>> = vec![vec![]; gens];
    for (i, &g) in lay.gn.iter().enumerate() {
        order[g].push(i);
    }
    for o in order.iter_mut() {
        o.sort_by(|a, b| lay.x[*a].partial_cmp(&lay.x[*b]).unwrap());
        for (k, &i) in o.iter().enumerate() {
            num[i] = k + 1;
        }
    }
    (num, order)
}

/// Applies typed overrides: `II-3=affected`, flags joined with + (`II-3=affected+deceased+proband`,
/// `female`, `unknown`, `related`).
fn apply_changes(f: &mut Family, changes: &str) {
    if changes.trim().is_empty() {
        return;
    }
    let lay = layout(f);
    let (_, order) = numbering(&lay, f.depth());
    for tok in changes.split([',', ';']) {
        let Some((who, what)) = tok.split_once('=') else { continue };
        let Some((g, n)) = who.trim().split_once('-') else { continue };
        let Some(gi) = ["I", "II", "III", "IV"].iter().position(|r| r.eq_ignore_ascii_case(g.trim())) else { continue };
        let Ok(n) = n.trim().parse::<usize>() else { continue };
        let Some(&pi) = order.get(gi).and_then(|o| o.get(n.wrapping_sub(1))) else { continue };
        for fl in what.split('+') {
            let fl = fl.trim().to_lowercase();
            if matches!(fl.as_str(), "related" | "consanguineous") {
                f.set_related(pi, true);
                continue;
            }
            let Some(q) = f.person_mut(pi) else { continue };
            match fl.as_str() {
                "affected" | "a" => q.status = 2,
                "carrier" | "c" => q.status = 1,
                "unaffected" | "u" | "normal" => q.status = 0,
                "deceased" | "dead" | "d" => q.dead = true,
                "proband" | "p" => q.proband = true,
                "male" | "m" => q.sex = 0,
                "female" | "f" => q.sex = 1,
                "unknown" | "?" => q.sex = 2,
                _ => {}
            }
        }
    }
}

/// "II-3" style labels of everyone, in index order.
pub fn labels(p: &Params) -> Vec<String> {
    people_of(p).2.iter().map(|w| format!("{}-{}", roman(w.gi), w.num)).collect()
}

pub fn render(p: &Params) -> String {
    let mode = mode_of(p);
    let fam = family_of(p);
    let lay = layout(&fam);
    let gens = fam.depth();
    let people = fam.people();
    let np = people.len();
    let (num, _) = numbering(&lay, gens);
    let sex: Vec<u8> = people.iter().map(|q| q.sex.min(2)).collect();
    let status: Vec<Status> = people.iter().map(|q| st_of(q.status)).collect();
    let deceased: Vec<bool> = people.iter().map(|q| q.dead).collect();
    let proband: Vec<bool> = people.iter().map(|q| q.proband).collect();
    // family links (for genotype inference)
    let mut parents: Vec<Option<(usize, usize)>> = vec![None; np];
    let mut kids_of: Vec<Vec<usize>> = vec![vec![]; np];
    for n in &lay.nodes {
        if let Some(m) = n.mate {
            for &k in &n.kids {
                parents[k] = Some((n.me, m));
                kids_of[n.me].push(k);
                kids_of[m].push(k);
            }
        }
    }

    let car_style = carrier_style(p).to_string();
    let show_car = car_style != "Hidden";
    let gmode = p.s("genotypes");
    let letter = p.s("allele_letter").trim().chars().find(|c| c.is_alphabetic()).unwrap_or('A');
    let (up, lo) = (letter.to_uppercase().next().unwrap(), letter.to_lowercase().next().unwrap());
    let st = |i: usize| status[i];
    let affected = |i: usize| st(i) == Status::Affected;
    let rel_aff = |i: usize, want: bool| -> bool {
        parents[i].map(|(a, b)| affected(a) == want || affected(b) == want).unwrap_or(false) || kids_of[i].iter().any(|k| affected(*k) == want)
    };
    // genotype as rich parts: answers use what the pedigree shows (blank "_" where it cannot tell)
    let genotype = |i: usize| -> Vec<Part> {
        let male = sex[i] == 0;
        let aff = affected(i);
        let car = st(i) == Status::Carrier;
        let plain = |s: String| vec![Part::plain(&s)];
        let xs = |a: &str, b: &str| -> Vec<Part> {
            let mut v = vec![Part::plain("X"), Part { s: a.into(), sup: true, serif: false, bold: false }];
            if b == "Y" { v.push(Part::plain("Y")); } else { v.push(Part::plain("X")); v.push(Part { s: b.into(), sup: true, serif: false, bold: false }); }
            v
        };
        let (us, ls) = (up.to_string(), lo.to_string());
        match mode {
            Mode::AR => {
                if aff { plain(format!("{lo}{lo}")) }
                else if car || (!show_car && rel_aff(i, true)) { plain(format!("{up}{lo}")) }
                else if show_car { plain(format!("{up}{up}")) } else { plain(format!("{up}_")) }
            }
            Mode::AD => {
                if !aff { plain(format!("{lo}{lo}")) }
                else if rel_aff(i, false) || people[i].a[0] + people[i].a[1] == 1 && show_car { plain(format!("{up}{lo}")) }
                else { plain(format!("{up}_")) }
            }
            Mode::XR => {
                if male { if aff { xs(&ls, "Y") } else { xs(&us, "Y") } }
                else if aff { xs(&ls, &ls) }
                else if car || (!show_car && (parents[i].map(|(a, b)| (sex[a] == 0 && affected(a)) || (sex[b] == 0 && affected(b))).unwrap_or(false) || kids_of[i].iter().any(|k| sex[*k] == 0 && affected(*k)))) { xs(&us, &ls) }
                else if show_car { xs(&us, &us) } else { xs(&us, "_") }
            }
            Mode::XD => {
                if male { if aff { xs(&us, "Y") } else { xs(&ls, "Y") } }
                else if !aff { xs(&ls, &ls) }
                else if parents[i].map(|(a, b)| (sex[a] == 0 && !affected(a)) || (sex[b] == 0 && !affected(b))).unwrap_or(false) || kids_of[i].iter().any(|k| sex[*k] == 0 && !affected(*k)) { xs(&us, &ls) }
                else { xs(&us, "_") }
            }
            Mode::Y => {
                if male { let mut v = vec![Part::plain("XY")]; v.push(Part { s: if aff { us.clone() } else { ls.clone() }, sup: true, serif: false, bold: false }); v }
                else { plain("XX".into()) }
            }
        }
    };

    // the reference's pitch: 62 px between people, 20 more between one family and the next
    let slot = 62.0;
    let fam_gap = 20.0;
    let mut cuts: Vec<f64> = Vec::new();
    for g in 0..gens {
        let mut row: Vec<usize> = (0..np).filter(|&i| lay.gn[i] == g).collect();
        row.sort_by(|a, b| lay.x[*a].partial_cmp(&lay.x[*b]).unwrap_or(std::cmp::Ordering::Equal));
        for pair in row.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let partners = lay.nodes.iter().any(|n| (n.me == a && n.mate == Some(b)) || (n.me == b && n.mate == Some(a)));
            let sibs = parents[a].is_some() && parents[a] == parents[b];
            if !partners && !sibs {
                let m = (lay.x[a] + lay.x[b]) / 2.0;
                if !cuts.iter().any(|c| (c - m).abs() < 0.01) { cuts.push(m); }
            }
        }
    }
    let xmin = lay.x.iter().cloned().fold(f64::MAX, f64::min).min(1e9);
    let sx = |x: f64| (x - xmin) * slot + fam_gap * cuts.iter().filter(|c| **c < x).count() as f64;
    let r = p.f("symbol_size").clamp(12.0, 26.0);
    let geno_on = gmode == "Answers" || gmode == "Blanks";
    let gen_h = 96.0 + if geno_on { 24.0 } else { 0.0 };
    let width_slots = lay.width;
    let title = p.s("title").trim().to_string();
    let left = if p.b("gen_labels") { r + 58.0 } else { r + 8.0 };
    let tree_w = lay.x.iter().map(|x| sx(*x)).fold(0.0, f64::max);
    let _ = width_slots;
    let key_on = p.b("key");
    let any_car = status.contains(&Status::Carrier) && show_car;
    let key_items: Vec<(u8, Status, &str, u8)> = {
        // (sex, status, label, extra: 0 none, 1 deceased, 2 proband)
        let mut v = vec![(0u8, Status::Unaffected, "Unaffected male", 0u8), (0, Status::Affected, "Affected male", 0), (1, Status::Unaffected, "Unaffected female", 0), (1, Status::Affected, "Affected female", 0)];
        if any_car {
            if (0..np).any(|i| st(i) == Status::Carrier && sex[i] == 0) { v.push((0, Status::Carrier, "Carrier male", 0)); }
            if (0..np).any(|i| st(i) == Status::Carrier && sex[i] != 0) { v.push((1, Status::Carrier, "Carrier female", 0)); }
        }
        if sex.contains(&2) { v.push((2, Status::Unaffected, "Sex unknown", 0)); }
        if deceased.iter().any(|x| *x) { v.push((0, Status::Unaffected, "Deceased", 1)); }
        if proband.iter().any(|x| *x) { v.push((0, Status::Unaffected, "Proband", 2)); }
        v
    };
    // the key: two columns, 32 px apart vertically, sized to the longest label
    let lab_w = |t: &str| t.chars().map(|c| super::plot::arial_em(c, false)).sum::<f64>() * 14.0;
    let col_w = 50.0 + key_items.iter().map(|k| lab_w(k.2)).fold(0.0, f64::max) + 10.66;
    let key_w = 2.0 * col_w + 8.5;
    let content_w = left + tree_w + r + 8.0;
    let inner_w = content_w.max(key_w + 1.5);
    let w = inner_w + 32.0;
    let ox = 16.0 + (inner_w - content_w) / 2.0;
    let top = 16.0 + if title.is_empty() { 0.0 } else { 36.0 };
    let tree_bottom = top + 6.0 + r + gen_h * (gens - 1) as f64 + 64.0 + if geno_on { 22.0 } else { 0.0 };
    let key_rows = key_items.len().div_ceil(2);
    let answer_txt = if p.b("answer") {
        Some(match mode { Mode::AD => "Autosomal dominant", Mode::AR => "Autosomal recessive", Mode::XR => "X-linked recessive", Mode::XD => "X-linked dominant", Mode::Y => "Y-linked" }.to_string() + " inheritance")
    } else { None };
    let h = tree_bottom + 4.0 + if key_on { key_rows as f64 * 32.0 + 16.5 + 1.5 + 16.0 } else { 4.0 } + if answer_txt.is_some() { 34.0 } else { 0.0 };
    let mut d = Doc::new(w, h);
    if !title.is_empty() { bold(&mut d, w / 2.0, 34.0, 20.0, "middle", &title); }

    let px = |i: usize| ox + left + sx(lay.x[i]);
    let py = |g: usize| top + 6.0 + r + g as f64 * gen_h;

    // connectors first: partner line (double when related), line of descent, sibship bar, and a
    // drop to each child (twins share one point on the bar; identical twins get a crossbar)
    for n in &lay.nodes {
        let y = py(lay.gn[n.me]);
        let mid = match n.mate {
            Some(m) => {
                let (xa, xb) = (px(n.me).min(px(m)), px(n.me).max(px(m)));
                if n.related {
                    d.line(xa + r, y - 3.0, xb - r, y - 3.0, INK, 2.0);
                    d.line(xa + r, y + 3.0, xb - r, y + 3.0, INK, 2.0);
                } else {
                    d.line(xa + r, y, xb - r, y, INK, 2.0);
                }
                (xa + xb) / 2.0
            }
            None => px(n.me),
        };
        if n.kids.is_empty() {
            continue;
        }
        let bar = y + gen_h * 0.6;
        d.line(mid, if n.mate.is_some() { y } else { y + r }, mid, bar, INK, 2.0);
        let xs: Vec<f64> = n.kids.iter().map(|&k| px(k)).collect();
        // where each child's line leaves the bar (a twin pair shares one point)
        let mut drops: Vec<f64> = Vec::new();
        let mut j = 0;
        while j < xs.len() {
            if n.twins[j] > 0 && j + 1 < xs.len() {
                drops.push((xs[j] + xs[j + 1]) / 2.0);
                j += 2;
            } else {
                drops.push(xs[j]);
                j += 1;
            }
        }
        let (lo, hi) = (drops.iter().cloned().fold(f64::MAX, f64::min), drops.iter().cloned().fold(f64::MIN, f64::max));
        d.line(lo.min(mid), bar, hi.max(mid), bar, INK, 2.0);
        let bottom = y + gen_h - r;
        let mut j = 0;
        while j < xs.len() {
            if n.twins[j] > 0 && j + 1 < xs.len() {
                let (xa, xb) = (xs[j], xs[j + 1]);
                let apex = (xa + xb) / 2.0;
                d.line(apex, bar, xa, bottom, INK, 2.0);
                d.line(apex, bar, xb, bottom, INK, 2.0);
                if n.twins[j] == 2 {
                    let t = 0.45;
                    let yy = bar + (bottom - bar) * t;
                    d.line(apex + (xa - apex) * t, yy, apex + (xb - apex) * t, yy, INK, 2.0);
                }
                j += 2;
            } else {
                d.line(xs[j], bar, xs[j], bottom, INK, 2.0);
                j += 1;
            }
        }
    }

    for i in 0..np {
        let (cx, cy) = (px(i), py(lay.gn[i]));
        symbol_ex(&mut d, cx, cy, r, sex[i], status[i], &car_style);
        if deceased[i] { deceased_mark(&mut d, cx, cy, r); }
        if proband[i] { proband_mark(&mut d, cx, cy, r); }
        let mut ty = cy + r * if sex[i] == 2 { 1.25 } else { 1.0 } + 17.0;
        if p.b("numbers") {
            txt(&mut d, cx, ty, 13.0, "middle", SANS, "", &num[i].to_string());
            ty += 4.0;
        }
        match gmode {
            "Answers" => rich(&mut d, cx, ty + 20.0, 19.0, "middle", &genotype(i), false),
            "Blanks" => { d.line(cx - 22.0, ty + 20.0, cx + 22.0, ty + 20.0, INK, 1.3); }
            _ => {}
        }
    }
    if p.b("gen_labels") {
        for g in 0..gens {
            let custom: Vec<String> = p.s("gen_prefix").split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
            let lab = custom.get(g).cloned().unwrap_or_else(|| roman(g).to_string());
            txt(&mut d, ox, py(g) + 6.0, 17.0, "start", SERIF, r#"font-weight="bold""#, &lab);
        }
    }
    let whos: Vec<Who> = (0..np).map(|i| Who {
        x: px(i), y: py(lay.gn[i]), gi: lay.gn[i], num: num[i], male: sex[i] == 0, sex: sex[i], r,
        status: st_u8(status[i]),
    }).collect();
    LAST.with(|l| *l.borrow_mut() = (w, h, whos));
    let mut ky_end = tree_bottom;
    if key_on {
        let kx = 16.0 + (inner_w - key_w - 1.5) / 2.0;
        let ky = tree_bottom + 4.0;
        let kh = key_rows as f64 * 32.0 + 16.5;
        d.rect(kx + 0.75, ky + 0.75, key_w, kh, "#fff", INK, 1.2);
        for (i, (sxx, stt, label, extra)) in key_items.iter().enumerate() {
            let (col, row) = (i % 2, i / 2);
            let (x, y) = (kx + 27.0 + col as f64 * col_w, ky + 25.0 + row as f64 * 32.0);
            symbol_ex(&mut d, x, y, 11.0, *sxx, *stt, if car_style == "Hidden" { "Center dot" } else { &car_style });
            if *extra == 1 { deceased_mark(&mut d, x, y, 11.0); }
            if *extra == 2 { key_proband_mark(&mut d, x, y, 11.0); }
            txt(&mut d, x + 23.0, y + 4.9, 14.0, "start", SANS, "", label);
        }
        ky_end = ky + kh + 1.5;
    }
    if let Some(t) = answer_txt {
        txt(&mut d, w / 2.0, ky_end + 30.0, 18.0, "middle", SANS, "", &t);
    }
    d.finish()
}

/// "Start from an example" entries (reference gallery, on our seeded families).
/// The reference's "Classic setups" dropdown: the disease, its inheritance, allele letter and
/// title (the reference also fixes the family; ours keeps the generated family, and "New
/// pedigree" draws another).
pub fn classics() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    let setup = |name, mode: &str, letter: &str, title: &str| Example {
        name,
        params: vec![("inherit", st(mode)), ("allele_letter", st(letter)), ("title", st(title))],
    };
    let mut royal = setup("Hemophilia, royal family", "X-linked recessive", "H", "Hemophilia in a royal family");
    royal.params.extend([("gens", Value::I(4)), ("size", st("Large"))]);
    vec![
        royal,
        setup("Huntington’s disease", "Autosomal dominant", "H", "Huntington’s disease"),
        setup("Cystic fibrosis", "Autosomal recessive", "F", "Cystic fibrosis"),
        setup("Albinism", "Autosomal recessive", "A", "Albinism"),
        setup("Red–green color blindness", "X-linked recessive", "B", "Red–green color blindness"),
        setup("Vitamin D–resistant rickets", "X-linked dominant", "R", "Vitamin D–resistant rickets"),
    ]
}

pub fn examples() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    vec![
        Example { name: "Hemophilia pedigree: X-linked recessive over four generations", params: vec![
            ("inherit", st("X-linked recessive")), ("gens", Value::I(4)), ("size", st("Large")), ("carrier_style", st("Half filled")),
            ("allele_letter", st("H")), ("title", st("Hemophilia in a royal family")), ("changes", st("II-4=affected+deceased, IV-5=proband")),
        ] },
        Example { name: "Cystic fibrosis pedigree with blank genotypes (autosomal recessive)", params: vec![
            ("inherit", st("Autosomal recessive")), ("genotypes", st("Blanks")), ("allele_letter", st("F")), ("title", st("Cystic fibrosis")),
        ] },
        Example { name: "Huntington's disease pedigree with genotypes (autosomal dominant)", params: vec![
            ("inherit", st("Autosomal dominant")), ("genotypes", st("Answers")), ("allele_letter", st("H")), ("title", st("Huntington's disease")),
            ("answer", Value::B(true)), ("changes", st("I-1=affected+deceased")),
        ] },
        Example { name: "Red-green color blindness pedigree with X-linked genotypes", params: vec![
            ("inherit", st("X-linked recessive")), ("carrier_style", st("Center dot")), ("genotypes", st("Answers")), ("allele_letter", st("B")),
            ("title", st("Red-green color blindness")),
        ] },
        Example { name: "Y-linked pedigree: a random family over 3 generations", params: vec![("inherit", st("Y-linked")), ("seed", Value::I(2))] },
    ]
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
        for ex in examples() { render_case("pedigree", "example", &ex.params); }
        for m in ["Autosomal dominant", "Autosomal recessive", "X-linked recessive", "X-linked dominant", "Y-linked"] {
            for g in ["Answers", "Blanks"] {
                for c in ["Hidden", "Half filled", "Center dot"] {
                    render_case("pedigree", "grid", &[("inherit", s(m)), ("genotypes", s(g)), ("carrier_style", s(c)), ("changes", s("II-1=unknown+proband, II-2=related+deceased"))]);
                }
            }
        }
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

    fn params_with(over: &[(&str, Value)]) -> Params {
        let mut p = Params::defaults(&params());
        for (k, v) in over {
            p.0.insert(k.to_string(), v.clone());
        }
        p
    }

    /// Every generated family (and its typed overrides) draws byte for byte as the renderer did
    /// before families became editable, so old figures, presets, share links and classic setups
    /// look the same.
    /// The key's proband arrow stays inside the key box (it used to poke through the left
    /// border when Proband landed in the first column); people keep the full-length arrow.
    #[test]
    fn key_proband_arrow_stays_inside_the_key() {
        let num = |t: &str, attr: &str| -> f64 {
            let i = t.find(&format!(" {attr}=\"")).unwrap() + attr.len() + 3;
            t[i..].split('"').next().unwrap().parse().unwrap()
        };
        let mut checked = 0;
        for changes in ["II-1=proband", "II-1=proband, II-2=deceased", "II-1=proband, II-2=deceased, I-1=unknown"] {
            let svg = render(&params_with(&[("changes", s(changes))]));
            let key = svg.split("<rect").filter(|t| t.contains("stroke-width=\"1.2\"")).last().expect("key box");
            let (kx, ky) = (num(key, "x"), num(key, "y"));
            for l in svg.split("<line").skip(1).filter(|l| num(l, "y1") > ky) {
                assert!(num(l, "x1").min(num(l, "x2")) > kx + 3.0, "{changes}: key line at {} vs box {kx}", num(l, "x1"));
                checked += 1;
            }
        }
        assert!(checked > 0, "found the key's proband arrows");
    }

    /// What a drawing says rather than where it puts it: its words and how many of each shape it
    /// has. The geometry follows the reference's spacing now (62 px pitch, 96 px generations), so
    /// the legacy renderer's coordinates no longer match, but the families themselves must.
    fn content(svg: &str) -> (Vec<String>, [usize; 4]) {
        let texts: Vec<String> = svg.split("<text").skip(1).filter_map(|t| t.split_once('>')).filter_map(|(_, r)| r.split("</text>").next()).map(|t| t.to_string()).collect();
        (texts, ["<line", "<rect", "<circle", "<polygon"].map(|t| svg.matches(t).count()))
    }

    #[test]
    fn generated_families_draw_exactly_as_before() {
        let mut n = 0;
        for m in ["Autosomal dominant", "Autosomal recessive", "X-linked recessive", "X-linked dominant", "Y-linked"] {
            for gens in 2..=4 {
                for size in ["Small", "Medium", "Large"] {
                    for seed in [1, 3, 17, 404] {
                        let base = [("inherit", s(m)), ("gens", Value::I(gens)), ("size", s(size)), ("seed", Value::I(seed))];
                        let mut cases = vec![params_with(&base)];
                        let mut extra = |o: &[(&str, Value)]| {
                            let mut v = base.to_vec();
                            v.extend(o.iter().cloned());
                            cases.push(params_with(&v));
                        };
                        extra(&[("genotypes", s("Answers")), ("carrier_style", s("Half filled"))]);
                        extra(&[("genotypes", s("Answers")), ("carrier_style", s("Hidden")), ("answer", Value::B(true)), ("title", s("T"))]);
                        extra(&[("changes", s("II-1=unknown+proband, II-2=related+deceased, I-1=carrier, III-2=affected+female"))]);
                        extra(&[("changes", s("I-2=related, II-3=male+unaffected"))]);
                        for p in cases {
                            assert_eq!(content(&render(&p)), content(&legacy::legacy_render(&p)), "{m} {gens} {size} {seed} {:?}", p.s("changes"));
                            n += 1;
                        }
                    }
                }
            }
        }
        for ex in examples().into_iter().chain(classics()) {
            let p = params_with(&ex.params);
            assert_eq!(content(&render(&p)), content(&legacy::legacy_render(&p)), "{}", ex.name);
        }
        assert!(n > 800);
    }

    /// Storing the drawn family (text form) and reading it back draws the same figure; old
    /// overrides migrate into the stored family without changing a pixel.
    #[test]
    fn stored_family_and_override_migration_draw_the_same() {
        for seed in 1..25 {
            for m in ["Autosomal recessive", "X-linked recessive", "Autosomal dominant"] {
                let mut p = params_with(&[("inherit", s(m)), ("seed", Value::I(seed)), ("gens", Value::I(2 + seed % 3)), ("genotypes", s("Answers")),
                    ("changes", s("II-2=affected+deceased, III-1=proband, II-1=related, I-1=unknown"))]);
                let before = render(&p);
                assert!(migrate_overrides(&mut p));
                assert_eq!(p.s("changes"), "");
                assert!(hand_changed(&p));
                assert_eq!(render(&p), before, "seed {seed} {m}");
                assert!(!migrate_overrides(&mut p), "nothing left to fold");
                // and through the text form again
                let f = Family::parse(p.s("family")).unwrap();
                assert_eq!(f.to_text(), p.s("family"));
            }
        }
        // a broken stored family falls back to the generated one
        let mut p = params_with(&[]);
        let generated_svg = render(&p);
        p.set_s("family", "1:m-f(oops");
        assert_eq!(render(&p), generated_svg);
    }

    fn check_layout(p: &Params, ctx: &str) {
        let (_, _, whos) = people_of(p);
        let fam = family_of(p);
        assert_eq!(whos.len(), fam.len(), "{ctx}");
        let gens = fam.depth();
        for g in 0..gens {
            let mut row: Vec<&Who> = whos.iter().filter(|w| w.gi == g).collect();
            assert!(!row.is_empty(), "{ctx}: generation {g} empty");
            // one row per generation, everyone at least a slot apart, numbered left to right
            assert!(row.iter().all(|w| (w.y - row[0].y).abs() < 1e-9), "{ctx}");
            row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
            for (k, w) in row.iter().enumerate() {
                assert_eq!(w.num, k + 1, "{ctx}");
                if k > 0 {
                    assert!(w.x - row[k - 1].x >= 62.0 - 1e-6, "{ctx}: overlap in generation {g}");
                }
            }
        }
        // a child is drawn one generation below its parents, under the span of the sibship
        let lay = layout(&fam);
        for n in &lay.nodes {
            for &k in &n.kids {
                assert_eq!(lay.gn[k], lay.gn[n.me] + 1, "{ctx}");
            }
        }
    }

    /// Random sequences of edits (child, partner, sibling, remove, flags) keep a tidy layout:
    /// generations on their own rows, nobody overlapping, numbering left to right; every
    /// intermediate family survives the text round trip.
    #[test]
    fn edits_keep_a_tidy_layout() {
        for seed in 1..40u64 {
            let mut p = params_with(&[("seed", Value::I(seed as i64)), ("gens", Value::I(2 + (seed % 3) as i64)), ("size", s(["Small", "Medium", "Large"][seed as usize % 3]))]);
            let mut rng = Rng::new(seed);
            check_layout(&p, "start");
            for step in 0..30 {
                let mut f = family_of(&p);
                let i = rng.below(f.len());
                let what = rng.below(9);
                match what {
                    0 | 1 => { f.add_child(i); }
                    2 => { f.add_partner(i); }
                    3 | 4 => { f.add_sibling(i); }
                    5 => { f.remove(i); }
                    6 => { f.set_related(i, true); }
                    7 => { if f.caps(i).is_some_and(|c| c.twin) { f.person_mut(i).unwrap().twin = 1 + rng.below(2) as u8; } }
                    _ => { let q = f.person_mut(i).unwrap(); q.status = rng.below(3) as u8; q.dead = rng.f() < 0.3; q.proband = rng.f() < 0.2; }
                }
                assert!(f.depth() <= MAX_GENS);
                store(&mut p, &f);
                assert_eq!(family_of(&p), f, "seed {seed} step {step}");
                check_layout(&p, &format!("seed {seed} step {step} op {what}"));
                to_png(&render(&p), 0.3).unwrap();
            }
        }
    }

    #[test]
    fn add_and_remove_change_the_drawing() {
        let mut p = params_with(&[]);
        let n0 = people_of(&p).2.len();
        let mut f = family_of(&p);
        // the last person drawn in the youngest generation gets a partner and a child
        let leaf = (0..f.len()).rev().find(|&i| f.caps(i).unwrap().partner).unwrap();
        let kid = f.add_child(leaf).unwrap();
        store(&mut p, &f);
        let (_, _, whos) = people_of(&p);
        assert_eq!(whos.len(), n0 + 2, "partner + child");
        assert_eq!(whos[kid].gi, whos[leaf].gi + 1);
        assert_eq!(f.depth(), 4);
        // removing the partner takes the child too
        let mate = f.index_of(&family::Loc { path: f.locate(leaf).unwrap().path, mate: true }).unwrap();
        assert!(f.remove(mate));
        store(&mut p, &f);
        assert_eq!(people_of(&p).2.len(), n0);
        assert_eq!(f.depth(), 3);
    }

    /// The panel's edits show with the standard symbols, and the key lists them.
    #[test]
    fn edited_people_get_the_standard_symbols() {
        let mut p = params_with(&[("carrier_style", s("Half filled"))]);
        p.set_s("family", "1:m-f(m=<f(udp,fC,mt,fT,m,f),fA)");
        let svg = render(&p);
        for k in ["Sex unknown", "Deceased", "Proband", "Carrier female", "Affected female"] {
            assert!(svg.contains(k), "key misses {k}");
        }
        assert!(!svg.contains("Carrier male"));
        let (_, _, whos) = people_of(&p);
        assert_eq!(whos.len(), 11);
        assert_eq!(whos[4].sex, 2, "III-1 is drawn as a diamond");
        assert_eq!(whos[5].status, 1);
        assert!(svg.contains("<polygon"), "diamond / proband arrow");
        // related partners: a double line; twins: diagonal lines from one point on the sibship bar
        let lines = svg.matches("<line").count();
        let mut q = p.clone();
        q.set_s("family", "1:m-f(m-<f(u,fC,m,f,m,f),fA)");
        let plain = render(&q);
        assert!(lines > plain.matches("<line").count(), "double line, crossbar, deceased slash add lines");
        // the old click-to-cycle is gone: people are selected in the UI, and the handles only
        // keep the title
        assert_eq!(super::super::handles::handles("pedigree", &p, (600.0, 500.0)).unwrap().len(), 1);
    }

    /// Changing the inheritance of a family changed by hand keeps its people and flags and shades
    /// it again by the rules of the new pattern.
    #[test]
    fn inheritance_change_reshades_a_hand_made_family() {
        let mut p = params_with(&[("gens", Value::I(4)), ("size", s("Large"))]);
        let mut f = family_of(&p);
        f.add_sibling(2);
        f.person_mut(0).unwrap().dead = true;
        f.person_mut(3).unwrap().proband = true;
        store(&mut p, &f);
        for m in ["Y-linked", "X-linked recessive", "Autosomal recessive", "Autosomal dominant", "X-linked dominant"] {
            p.set_s("inherit", m);
            reshade_params(&mut p);
            let g = family_of(&p);
            assert_eq!(g.len(), f.len(), "{m}: same people");
            assert!(g.person(0).unwrap().dead && g.person(3).unwrap().proband, "{m}: flags kept");
            let sexes = |x: &Family| x.people().iter().map(|q| q.sex).collect::<Vec<_>>();
            assert_eq!(sexes(&g), sexes(&f));
            let ppl = g.people();
            match m {
                "Y-linked" => assert!(ppl.iter().all(|q| q.sex == 0 || q.status != 2), "only males affected"),
                "Autosomal recessive" => assert!(ppl.iter().all(|q| (q.status == 2) == (q.a[0] + q.a[1] == 2))),
                "X-linked recessive" => assert!(ppl.iter().all(|q| q.sex != 0 || q.status != 1), "no carrier males"),
                _ => {}
            }
            let last = g.depth() - 1;
            let lay = layout(&g);
            let young: Vec<u8> = (0..g.len()).filter(|&i| lay.gn[i] == last).map(|i| g.person(i).unwrap().status).collect();
            assert!(young.contains(&2) || m == "Y-linked", "{m}: someone affected in the youngest generation");
        }
        // a generated family stays generated
        let mut q = params_with(&[]);
        q.set_s("inherit", "Y-linked");
        reshade_params(&mut q);
        assert!(!hand_changed(&q));
    }

    #[test]
    fn new_pedigree_and_setup_changes_drop_hand_changes() {
        let mut p = params_with(&[("changes", s("II-1=affected"))]);
        migrate_overrides(&mut p);
        assert!(hand_changed(&p));
        let before = render(&p);
        new_pedigree(&mut p, 4242);
        assert!(!hand_changed(&p) && p.s("changes").is_empty() && p.i("seed") == 4242);
        assert_ne!(render(&p), before);
        migrate_overrides(&mut p);
        p.set_s("changes", "I-1=affected");
        drop_hand_changes(&mut p);
        assert!(!hand_changed(&p) && p.s("changes").is_empty());
        let l = labels(&p);
        assert_eq!(l[..2], ["I-1", "I-2"]);
        assert_eq!(l.len(), family_of(&p).len());
    }
}
