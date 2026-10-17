//! Pedigree generator. A seeded random family is simulated under the chosen inheritance
//! pattern (so affected/carrier status really follows Mendelian rules), laid out as a tree
//! with roman-numeral generations, numbering and a key.
//!
//! "Changes" syntax: comma-separated `II-3=affected`, `III-2=carrier`, `II-1=unaffected`
//! (person = generation roman numeral, dash, number counted left to right).
use super::*;

/// One drawn individual (for interaction handles).
#[derive(Clone, Debug)]
pub struct Who { pub x: f64, pub y: f64, pub gi: usize, pub num: usize, pub male: bool, pub status: u8, pub r: f64 }

thread_local! {
    static LAST: std::cell::RefCell<(f64, f64, Vec<Who>)> = const { std::cell::RefCell::new((0.0, 0.0, Vec::new())) };
}

/// Renders and returns (canvas w, h, people) of the figure.
pub fn people_of(p: &Params) -> (f64, f64, Vec<Who>) {
    LAST.with(|l| *l.borrow_mut() = (0.0, 0.0, Vec::new()));
    let _ = render(p);
    LAST.with(|l| l.borrow().clone())
}

/// Set `II-3=affected` style override for one person, replacing any earlier status for them
/// (other flags such as `deceased` or `proband` on the same person are kept).
/// `status`: 0 unaffected, 1 carrier, 2 affected.
pub fn set_status(p: &mut Params, gi: usize, num: usize, status: u8) {
    let who = format!("{}-{}", roman(gi), num);
    let mut toks: Vec<String> = p.s("changes").split([',', ';']).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
    let mut keep: Vec<String> = Vec::new();
    toks.retain(|t| match t.split_once('=') {
        Some((a, b)) if a.trim().eq_ignore_ascii_case(&who) => {
            for f in b.split('+') {
                let f = f.trim().to_lowercase();
                if !matches!(f.as_str(), "affected" | "a" | "carrier" | "c" | "unaffected" | "u" | "normal") && !f.is_empty() { keep.push(f); }
            }
            false
        }
        _ => true,
    });
    let name = ["unaffected", "carrier", "affected"][status.min(2) as usize];
    keep.insert(0, name.to_string());
    toks.push(format!("{who}={}", keep.join("+")));
    p.set_s("changes", &toks.join(", "));
}

/// Whether carriers are drawn (so a click can cycle through the carrier state).
pub fn carriers_visible(p: &Params) -> bool { carrier_style(p) != "Hidden" }

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
        ParamDef::bool("gen_labels", "Generation labels (I, II, III)", true),
        ParamDef::bool("numbers", "Individual numbers", true),
        ParamDef::bool("key", "Key", true),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::float("symbol_size", "Symbol size", 12.0, 26.0, 1.0, 18.0),
        ParamDef::text("gen_prefix", "Generation label style (I, II, III or custom list)", ""),
        // ---- added for parity with the reference controls
        ParamDef::choice("carrier_style", "Carriers", &["Automatic", "Hidden", "Half filled", "Center dot"], "Automatic"),
        ParamDef::choice("genotypes", "Genotypes under each person", &["None", "Answers", "Blanks"], "None"),
        ParamDef::text("allele_letter", "Allele letter for genotypes", "A"),
        ParamDef::bool("answer", "Answer key (the inheritance pattern under the figure)", false),
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
    symbol_ex(d, cx, cy, r, if male { 0 } else { 1 }, st, if carriers { "Center dot" } else { "Hidden" });
}

/// `sex`: 0 male (square), 1 female (circle), 2 unknown (diamond). `car`: carrier style.
fn symbol_ex(d: &mut Doc, cx: f64, cy: f64, r: f64, sex: u8, st: Status, car: &str) {
    let fill = if st == Status::Affected { INK } else { "#fff" };
    let shape = |d: &mut Doc, fill: &str, stroke: &str, sw: f64| match sex {
        0 => { d.rect(cx - r, cy - r, 2.0 * r, 2.0 * r, fill, stroke, sw); }
        1 => { d.circle(cx, cy, r, fill, stroke, sw); }
        _ => { let q = r * 1.25; d.poly(&[(cx, cy - q), (cx + q, cy), (cx, cy + q), (cx - q, cy)], fill, stroke, sw); }
    };
    shape(d, fill, INK, 2.2);
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
                shape(d, "none", INK, 2.2);
            }
            _ => {}
        }
    }
}

fn deceased_mark(d: &mut Doc, cx: f64, cy: f64, r: f64) {
    d.line(cx - r * 1.45, cy + r * 1.45, cx + r * 1.45, cy - r * 1.45, INK, 2.0);
}

fn proband_mark(d: &mut Doc, cx: f64, cy: f64, r: f64) {
    let (tx, ty) = (cx - r * 1.15, cy + r * 0.85);
    let (sx, sy) = (tx - r * 1.3, ty + r * 0.55);
    d.line(sx, sy, tx - 6.0, ty + 2.5, INK, 2.0);
    let (dx, dy) = ((tx - sx), (ty - sy));
    let l = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (dx / l, dy / l);
    d.poly(&[(tx, ty), (tx - ux * 10.0 - uy * 4.5, ty - uy * 10.0 + ux * 4.5), (tx - ux * 10.0 + uy * 4.5, ty - uy * 10.0 - ux * 4.5)], INK, INK, 1.0);
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
    // changes: `II-3=affected`, flags joined with + (`II-3=affected+deceased+proband`, `female`, `unknown`)
    let np = sim.people.len();
    let mut deceased = vec![false; np];
    let mut proband = vec![false; np];
    let mut sex: Vec<u8> = sim.people.iter().map(|q| if q.male { 0 } else { 1 }).collect();
    let mut related = vec![false; np];
    for tok in p.s("changes").split([',', ';']) {
        let Some((who, what)) = tok.split_once('=') else { continue };
        let Some((g, n)) = who.trim().split_once('-') else { continue };
        let Some(gi) = ["I", "II", "III", "IV"].iter().position(|r| r.eq_ignore_ascii_case(g.trim())) else { continue };
        let Ok(n) = n.trim().parse::<usize>() else { continue };
        let Some(&pi) = order.get(gi).and_then(|o| o.get(n.wrapping_sub(1))) else { continue };
        for f in what.split('+') {
            match f.trim().to_lowercase().as_str() {
                "affected" | "a" => sim.people[pi].status = Status::Affected,
                "carrier" | "c" => sim.people[pi].status = Status::Carrier,
                "unaffected" | "u" | "normal" => sim.people[pi].status = Status::Unaffected,
                "deceased" | "dead" | "d" => deceased[pi] = true,
                "proband" | "p" => proband[pi] = true,
                "male" | "m" => sex[pi] = 0,
                "female" | "f" => sex[pi] = 1,
                "unknown" | "?" => sex[pi] = 2,
                "related" | "consanguineous" => related[pi] = true,
                _ => {}
            }
        }
    }
    // family links (for genotype inference)
    let mut parents: Vec<Option<(usize, usize)>> = vec![None; np];
    let mut kids_of: Vec<Vec<usize>> = vec![vec![]; np];
    fn link(n: &Node, parents: &mut Vec<Option<(usize, usize)>>, kids_of: &mut Vec<Vec<usize>>) {
        if let Some(m) = n.mate {
            for k in &n.kids {
                parents[k.person] = Some((n.person, m));
                kids_of[n.person].push(k.person);
                kids_of[m].push(k.person);
            }
        }
        for k in &n.kids { link(k, parents, kids_of); }
    }
    link(&root, &mut parents, &mut kids_of);

    let car_style = carrier_style(p).to_string();
    let show_car = car_style != "Hidden";
    let gmode = p.s("genotypes");
    let letter = p.s("allele_letter").trim().chars().find(|c| c.is_alphabetic()).unwrap_or('A');
    let (up, lo) = (letter.to_uppercase().next().unwrap(), letter.to_lowercase().next().unwrap());
    let st = |i: usize| sim.people[i].status;
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
                else if rel_aff(i, false) || sim.people[i].a[0] + sim.people[i].a[1] == 1 && show_car { plain(format!("{up}{lo}")) }
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

    let slot = 74.0;
    let r = p.f("symbol_size").clamp(12.0, 26.0);
    let geno_on = gmode == "Answers" || gmode == "Blanks";
    let gen_h = 112.0 + if geno_on { 24.0 } else { 0.0 };
    let width_slots = root.width;
    let title = p.s("title").trim().to_string();
    let left = 56.0 + if p.b("gen_labels") { 18.0 } else { 0.0 };
    let tree_w = width_slots * slot;
    let key_on = p.b("key");
    let any_car = sim.people.iter().any(|q| q.status == Status::Carrier) && show_car;
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
    let key_w = 470.0;
    let w = (left + tree_w + 50.0).max(key_w + 60.0 + 20.0).max(520.0);
    let ox = (w - tree_w - left - 50.0).max(0.0) / 2.0;
    let top = 40.0 + if title.is_empty() { 0.0 } else { 36.0 };
    let tree_bottom = top + gen_h * (gens - 1) as f64 + r + 56.0 + if geno_on { 22.0 } else { 0.0 };
    let key_rows = key_items.len().div_ceil(2);
    let answer_txt = if p.b("answer") {
        Some(match mode { Mode::AD => "Autosomal dominant", Mode::AR => "Autosomal recessive", Mode::XR => "X-linked recessive", Mode::XD => "X-linked dominant", Mode::Y => "Y-linked" }.to_string() + " inheritance")
    } else { None };
    let h = tree_bottom + if key_on { 30.0 + key_rows as f64 * 40.0 + 20.0 } else { 14.0 } + if answer_txt.is_some() { 34.0 } else { 0.0 };
    let mut d = Doc::new(w, h);
    if !title.is_empty() { bold(&mut d, w / 2.0, 34.0, 20.0, "middle", &title); }

    let px = |q: &Person| ox + left + q.x * slot;
    let py = |g: usize| top + 20.0 + g as f64 * gen_h;

    // connectors first
    fn walk(d: &mut Doc, sim: &Sim, n: &Node, px: &dyn Fn(&Person) -> f64, py: &dyn Fn(usize) -> f64, r: f64, gen_h: f64, related: &[bool]) {
        let pa = &sim.people[n.person];
        let y = py(pa.gn);
        if let Some(m) = n.mate {
            let pm = &sim.people[m];
            let (xa, xb) = (px(pa).min(px(pm)), px(pa).max(px(pm)));
            if related[n.person] || related[m] {
                d.line(xa + r, y - 3.0, xb - r, y - 3.0, INK, 2.0);
                d.line(xa + r, y + 3.0, xb - r, y + 3.0, INK, 2.0);
            } else {
                d.line(xa + r, y, xb - r, y, INK, 2.2);
            }
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
        for k in &n.kids { walk(d, sim, k, px, py, r, gen_h, related); }
    }
    walk(&mut d, &sim, &root, &px, &py, r, gen_h, &related);

    for (i, q) in sim.people.iter().enumerate() {
        let (cx, cy) = (px(q), py(q.gn));
        symbol_ex(&mut d, cx, cy, r, sex[i], q.status, &car_style);
        if deceased[i] { deceased_mark(&mut d, cx, cy, r); }
        if proband[i] { proband_mark(&mut d, cx, cy, r); }
        let mut ty = cy + r * if sex[i] == 2 { 1.25 } else { 1.0 } + 17.0;
        if p.b("numbers") {
            txt(&mut d, cx, ty, 15.0, "middle", SANS, "", &num[i].to_string());
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
            txt(&mut d, ox + 24.0, py(g) + 6.0, 19.0, "middle", SERIF, r#"font-weight="bold""#, &lab);
        }
    }
    let whos: Vec<Who> = sim.people.iter().enumerate().map(|(i, q)| Who {
        x: px(q), y: py(q.gn), gi: q.gn, num: num[i], male: sex[i] == 0, r,
        status: match q.status { Status::Unaffected => 0, Status::Carrier => 1, Status::Affected => 2 },
    }).collect();
    LAST.with(|l| *l.borrow_mut() = (w, h, whos));
    let mut ky_end = tree_bottom;
    if key_on {
        let kx = (w - key_w) / 2.0;
        let ky = tree_bottom + 4.0;
        d.rect(kx, ky, key_w, 20.0 + key_rows as f64 * 40.0 + 4.0, "#fff", INK, 1.8);
        for (i, (sx, stt, label, extra)) in key_items.iter().enumerate() {
            let (col, row) = (i % 2, i / 2);
            let (x, y) = (kx + 36.0 + col as f64 * 230.0, ky + 30.0 + row as f64 * 40.0);
            symbol_ex(&mut d, x, y, 14.0, *sx, *stt, if car_style == "Hidden" { "Center dot" } else { &car_style });
            if *extra == 1 { deceased_mark(&mut d, x, y, 14.0); }
            if *extra == 2 { proband_mark(&mut d, x, y, 14.0); }
            txt(&mut d, x + 28.0, y + 6.0, 16.0, "start", SANS, "", label);
        }
        ky_end = ky + 20.0 + key_rows as f64 * 40.0 + 4.0;
    }
    if let Some(t) = answer_txt {
        txt(&mut d, w / 2.0, ky_end + 30.0, 18.0, "middle", SANS, "", &t);
    }
    d.finish()
}

/// "Start from an example" entries (reference gallery, on our seeded families).
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
}
