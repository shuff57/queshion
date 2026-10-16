//! Punnett square generator: monohybrid, dihybrid and X-linked crosses with complete,
//! incomplete or co-dominance, optional shading and genotype/phenotype ratios.
//!
//! Parents syntax (the "Parents" text): two genotypes separated by "x" (or the multiplication
//! sign). Monohybrid `Tt x Tt`; dihybrid `TtRr x TtRr` (one letter per gene, capital = dominant);
//! X-linked `XCXc x XCY` (X followed by the allele letter, or Y).
//! Phenotype names: comma-separated per gene, genes separated by `;`. Complete dominance wants
//! `dominant, recessive`; incomplete/codominance wants `AA, Aa, aa` or just two names (the
//! heterozygote is then named automatically).
use super::*;
use std::collections::BTreeMap;

/// Grid geometry of the last render (for interaction handles).
#[derive(Clone, Debug, Default)]
pub struct Layout { pub w: f64, pub gx: f64, pub p1_y: f64, pub gy: f64, pub cell: f64, pub nc: usize, pub nr: usize, pub gsz: f64, pub ox: f64, pub pb_y: f64 }

thread_local! {
    static LAST: std::cell::RefCell<Option<Layout>> = const { std::cell::RefCell::new(None) };
}

/// Renders and returns the layout (None when the parents cannot be read).
pub fn layout_of(p: &Params) -> Option<Layout> {
    LAST.with(|l| *l.borrow_mut() = None);
    let _ = render(p);
    LAST.with(|l| l.borrow().clone())
}

pub const SHADES: [&str; 6] = ["#bfdbfe", "#fde68a", "#bbf7d0", "#fecaca", "#e9d5ff", "#fed7aa"];

/// Cross kind as an index: 0 mono, 1 di, 2 x-linked.
pub fn cross_of(p: &Params) -> u8 { match p.s("cross") { "Dihybrid" => 1, "X-linked" => 2, _ => 0 } }

/// Toggle the case of one allele (`which` 0/1) of gene `gene` in parent `parent` (0 = top, 1 = side)
/// and write the parents string back. Genotype cycle per click: AA -> Aa -> aa -> AA.
pub fn cycle_gene(p: &mut Params, parent: usize, gene: usize) {
    let cross = match p.s("cross") { "Dihybrid" => Cross::Di, "X-linked" => Cross::XLinked, _ => Cross::Mono };
    let Some((a, b)) = split_parents(p.s("parents")) else { return };
    let (Ok(mut pa), Ok(mut pb)) = (parse_parent(&a, cross), parse_parent(&b, cross)) else { return };
    let tgt = if parent == 0 { &mut pa } else { &mut pb };
    let Some(g) = tgt.get_mut(gene) else { return };
    let flip = |c: char| if c.is_uppercase() { c.to_lowercase().next().unwrap() } else { c.to_uppercase().next().unwrap() };
    if g[1] == 'Y' {
        g[0] = flip(g[0]);
    } else {
        let up = g.iter().filter(|c| c.is_uppercase()).count();
        match up {
            2 => g[1] = flip(g[1]),
            1 => { if g[0].is_uppercase() { g[0] = flip(g[0]); } else { g[1] = flip(g[1]); } if g.iter().filter(|c| c.is_uppercase()).count() == 1 { g.swap(0, 1); } }
            _ => { g[0] = flip(g[0]); g[1] = flip(g[1]); }
        }
        // keep the dominant allele first for readability
        if g[1].is_uppercase() && !g[0].is_uppercase() { g.swap(0, 1); }
    }
    let fmt = |v: &[Pair]| -> String {
        if cross == Cross::XLinked {
            let g = v[0];
            if g[1] == 'Y' { format!("X{}Y", g[0]) } else { format!("X{}X{}", g[0], g[1]) }
        } else {
            v.iter().map(|g| format!("{}{}", g[0], g[1])).collect()
        }
    };
    let s = format!("{} x {}", fmt(&pa), fmt(&pb));
    p.set_s("parents", &s);
}

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("cross", "Cross", &["Monohybrid", "Dihybrid", "X-linked"], "Monohybrid"),
        ParamDef::choice("dominance", "Dominance", &["Complete", "Incomplete", "Codominance"], "Complete"),
        ParamDef::text("parents", "Parents (first goes across the top), e.g. Tt x Tt", "Tt x Tt"),
        ParamDef::text("phenotypes", "Phenotype names, e.g. tall, short (genes split by ;)", "tall, short"),
        ParamDef::bool("filled", "Fill in the squares (off = blank for students)", true),
        ParamDef::choice("shading", "Shading", &["None", "Each phenotype, with a key", "First phenotype only", "Last phenotype only"], "None"),
        ParamDef::bool("geno_ratio", "Show genotype ratio", true),
        ParamDef::bool("pheno_ratio", "Show phenotype ratio", true),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::text("question", "Question (blank = none)", ""),
        ParamDef::color("shade_color", "Shade colour (first / last phenotype only)", &SHADES, SHADES[0]),
        ParamDef::bool("outline_heavy", "Heavy outer border", true),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cross { Mono, Di, XLinked }

/// One locus: two allele characters (`Y` marks the Y chromosome in X-linked crosses).
type Pair = [char; 2];

fn skip(c: char) -> bool { c.is_whitespace() || c == '^' || c == '{' || c == '}' || c == '_' }

fn parse_parent(s: &str, cross: Cross) -> Result<Vec<Pair>, String> {
    let cs: Vec<char> = s.chars().filter(|c| !skip(*c)).collect();
    match cross {
        Cross::XLinked => {
            let mut al = Vec::new();
            let mut i = 0;
            while i < cs.len() {
                match cs[i] {
                    'Y' => { al.push('Y'); i += 1; }
                    'X' => match cs.get(i + 1) {
                        Some(c) if c.is_alphabetic() && *c != 'X' && *c != 'Y' => { al.push(*c); i += 2; }
                        _ => return Err(format!("X needs an allele letter in \"{s}\" (e.g. XC)")),
                    },
                    c => return Err(format!("unexpected \"{c}\" in \"{s}\"")),
                }
            }
            if al.len() != 2 || al[0] == 'Y' { return Err(format!("need two sex chromosomes in \"{s}\"")); }
            Ok(vec![[al[0], al[1]]])
        }
        _ => {
            let genes = if cross == Cross::Mono { 1 } else { 2 };
            if cs.len() != genes * 2 || !cs.iter().all(|c| c.is_alphabetic()) {
                return Err(format!("need {} letters in \"{s}\"", genes * 2));
            }
            let mut v = Vec::new();
            for g in 0..genes {
                let (a, b) = (cs[g * 2], cs[g * 2 + 1]);
                if a.to_lowercase().ne(b.to_lowercase()) { return Err(format!("letters {a}{b} are not the same gene")); }
                v.push([a, b]);
            }
            Ok(v)
        }
    }
}

fn split_parents(s: &str) -> Option<(String, String)> {
    if let Some((a, b)) = s.split_once('×').or_else(|| s.split_once('*')) {
        return Some((a.into(), b.into()));
    }
    if let Some((a, b)) = s.split_once(" x ") { return Some((a.into(), b.into())); }
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 2 { return Some((parts[0].into(), parts[1].into())); }
    None
}

fn gametes(parent: &[Pair], cross: Cross) -> Vec<Vec<char>> {
    match cross {
        Cross::Di => {
            let (a, b) = (parent[0], parent[1]);
            vec![vec![a[0], b[0]], vec![a[0], b[1]], vec![a[1], b[0]], vec![a[1], b[1]]]
        }
        _ => vec![vec![parent[0][0]], vec![parent[0][1]]],
    }
}

/// Dominant first, then lower case, Y last.
fn order(a: char, b: char) -> Pair {
    let rank = |c: char| if c == 'Y' { 3 } else if c.is_uppercase() { 0 } else { 1 };
    if rank(a) <= rank(b) { [a, b] } else { [b, a] }
}

fn geno_parts(genes: &[Pair], cross: Cross) -> Vec<Part> {
    let mut v = Vec::new();
    for g in genes {
        for &c in g {
            if cross == Cross::XLinked {
                if c == 'Y' { v.push(Part::geno("Y", false)); } else { v.push(Part::geno("X", false)); v.push(Part::geno(&c.to_string(), true)); }
            } else {
                v.push(Part::geno(&c.to_string(), false));
            }
        }
    }
    v
}

fn gamete_parts(g: &[char], cross: Cross) -> Vec<Part> {
    let pairs: Vec<Pair> = g.iter().map(|c| [*c, '\0']).collect();
    let mut v = Vec::new();
    for p in pairs {
        let c = p[0];
        if cross == Cross::XLinked {
            if c == 'Y' { v.push(Part::geno("Y", false)); } else { v.push(Part::geno("X", false)); v.push(Part::geno(&c.to_string(), true)); }
        } else {
            v.push(Part::geno(&c.to_string(), false));
        }
    }
    v
}

fn parts_len(p: &[Part]) -> usize { p.iter().map(|x| x.s.chars().count()).sum() }

struct Child {
    genes: Vec<Pair>,
    male: bool,
}

fn key_of(c: &Child) -> String { c.genes.iter().map(|g| format!("{}{}", g[0], g[1])).collect() }

/// Phenotype class index per gene: complete 0 dominant / 1 recessive; otherwise 0 / 1 het / 2 recessive.
fn class_idx(g: Pair, dom: &str) -> usize {
    let alleles: Vec<char> = g.iter().copied().filter(|c| *c != 'Y').collect();
    let up = alleles.iter().filter(|c| c.is_uppercase()).count();
    if dom == "Complete" {
        if up > 0 { 0 } else { 1 }
    } else if alleles.len() == 2 {
        2 - up
    } else if up == 1 { 0 } else { 2 }
}

fn pheno_name(names: &[Vec<String>], gi: usize, idx: usize, g: Pair, dom: &str) -> String {
    let gene_names: &[String] = names.get(gi).map(|v| v.as_slice()).unwrap_or(&[]);
    let letter_up = g.iter().find(|c| c.is_alphabetic() && **c != 'Y').copied().unwrap_or('A');
    let (u, l) = (letter_up.to_uppercase().next().unwrap(), letter_up.to_lowercase().next().unwrap());
    if dom == "Complete" {
        if gene_names.len() >= 2 { return gene_names[idx].clone(); }
        return if idx == 0 { format!("{u}_") } else { format!("{l}{l}") };
    }
    if gene_names.len() >= 3 { return gene_names[idx].clone(); }
    if gene_names.len() == 2 {
        return match idx {
            0 => gene_names[0].clone(),
            2 => gene_names[1].clone(),
            _ => if dom == "Codominance" { format!("{} and {}", gene_names[0], gene_names[1]) } else { "intermediate".into() },
        };
    }
    match idx { 0 => format!("{u}{u}"), 1 => format!("{u}{l}"), _ => format!("{l}{l}") }
}

const FILLS: [&str; 6] = ["#bfdbfe", "#fde68a", "#bbf7d0", "#fecaca", "#e9d5ff", "#fed7aa"];

pub fn render(p: &Params) -> String {
    let cross = match p.s("cross") { "Dihybrid" => Cross::Di, "X-linked" => Cross::XLinked, _ => Cross::Mono };
    let dom = p.s("dominance").to_string();
    let title = p.s("title").trim().to_string();
    let question = p.s("question").trim().to_string();

    let parsed = split_parents(p.s("parents"))
        .ok_or_else(|| "Parents: separate the two genotypes with \"x\"".to_string())
        .and_then(|(a, b)| Ok((parse_parent(&a, cross)?, parse_parent(&b, cross)?)));
    let (pa, pb) = match parsed {
        Ok(v) => v,
        Err(e) => {
            let mut d = Doc::new(560.0, 140.0);
            bold(&mut d, 280.0, 60.0, 17.0, "middle", "Cannot read the parents");
            txt(&mut d, 280.0, 90.0, 13.0, "middle", SANS, "", &e);
            return d.finish();
        }
    };
    let ga = gametes(&pa, cross);
    let gb = gametes(&pb, cross);
    let (nc, nr) = (ga.len(), gb.len());

    // children
    let mut kids: Vec<Vec<Child>> = Vec::new();
    for rb in &gb {
        let mut row = Vec::new();
        for ca in &ga {
            let genes: Vec<Pair> = (0..ca.len()).map(|i| order(ca[i], rb[i])).collect();
            let male = genes.iter().any(|g| g.contains(&'Y'));
            row.push(Child { genes, male });
        }
        kids.push(row);
    }

    // phenotype names
    let names: Vec<Vec<String>> = p.s("phenotypes").split(';')
        .map(|g| g.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).collect();
    let pheno = |c: &Child| -> (Vec<usize>, String) {
        let idxs: Vec<usize> = c.genes.iter().map(|g| class_idx(*g, &dom)).collect();
        let mut nm: Vec<String> = c.genes.iter().enumerate().map(|(gi, g)| pheno_name(&names, gi, idxs[gi], *g, &dom)).collect();
        let mut full = nm.join(" ");
        if cross == Cross::XLinked {
            nm.push(if c.male { "male".into() } else { "female".into() });
            full = nm.join(" ");
        }
        (idxs, full)
    };

    // tallies
    let mut geno_count: BTreeMap<(Vec<i32>, String), (i64, Vec<Part>)> = BTreeMap::new();
    let mut ph_count: BTreeMap<(Vec<usize>, bool, String), i64> = BTreeMap::new();
    for row in &kids {
        for c in row {
            let caps: Vec<i32> = c.genes.iter().map(|g| -(g.iter().filter(|x| x.is_uppercase()).count() as i32)).collect();
            let e = geno_count.entry((caps, key_of(c))).or_insert((0, geno_parts(&c.genes, cross)));
            e.0 += 1;
            let (idx, nm) = pheno(c);
            *ph_count.entry((idx, c.male, nm)).or_insert(0) += 1;
        }
    }
    let classes: Vec<(Vec<usize>, bool, String)> = ph_count.keys().cloned().collect();
    let class_of = |c: &Child| -> usize {
        let (idx, nm) = pheno(c);
        classes.iter().position(|k| k.0 == idx && k.1 == c.male && k.2 == nm).unwrap_or(0)
    };
    let shade_mode = p.s("shading").to_string();
    let shade_col: &str = SHADES.iter().copied().find(|c| c.eq_ignore_ascii_case(p.s("shade_color"))).unwrap_or(SHADES[0]);
    let shaded = |ci: usize| -> Option<&'static str> {
        match shade_mode.as_str() {
            s if s.starts_with("Each") => Some(FILLS[ci % FILLS.len()]),
            s if s.starts_with("First") => (ci == 0).then_some(shade_col),
            s if s.starts_with("Last") => (ci + 1 == classes.len()).then_some(shade_col),
            _ => None,
        }
    };

    // geometry
    let cell = match (cross, nc) { (_, 4) => 82.0, _ => 100.0 };
    let gx_off = 120.0;
    let content_w = gx_off + cell * nc as f64;
    let mut lines_w = 0.0f64;
    let gdiv = geno_count.values().map(|v| v.0).fold(0, gcd);
    let pdiv = ph_count.values().copied().fold(0, gcd);
    let geno_ratio_len: usize = geno_count.values().map(|(n, parts)| (n / gdiv.max(1)).to_string().len() + parts_len(parts) + 5).sum::<usize>() + 16;
    let pheno_ratio_len: usize = ph_count.iter().map(|(k, n)| (n / pdiv.max(1)).to_string().len() + k.2.chars().count() + 4).sum::<usize>() + 16;
    if p.b("geno_ratio") { lines_w = lines_w.max(geno_ratio_len as f64 * 8.6); }
    if p.b("pheno_ratio") { lines_w = lines_w.max(pheno_ratio_len as f64 * 9.2); }
    let w = (content_w.max(lines_w) + 90.0).max(580.0);
    let ox = (w - content_w) / 2.0;
    let cx_mid = w / 2.0;

    let mut y = 16.0;
    let mut head: Vec<(f64, f64, String, bool)> = Vec::new(); // y, size, text, bold
    if !title.is_empty() { y += 24.0; head.push((y, 21.0, title, true)); y += 12.0; }
    for l in wrap(&question, ((w - 60.0) / 8.0) as usize) { y += 22.0; head.push((y, 15.0, l, false)); }
    if !head.is_empty() { y += 14.0; }
    let p1_y = y + 28.0;
    let gam_y = p1_y + 38.0;
    let gy = gam_y + 14.0;
    let gx = ox + gx_off;
    let grid_h = cell * nr as f64;
    let mut yb = gy + grid_h + 34.0;
    let mut d_h = yb;
    let rows_extra = p.b("geno_ratio") as i32 + p.b("pheno_ratio") as i32;
    d_h += rows_extra as f64 * 30.0;
    let key_on = shade_mode.starts_with("Each");
    let key_cols = if w > 900.0 { 3 } else { 2 };
    let key_rows = classes.len().div_ceil(key_cols);
    if key_on { d_h += 18.0 + key_rows as f64 * 26.0; }
    d_h += 16.0;

    let mut d = Doc::new(w, d_h);
    for (hy, sz, t, b) in &head {
        txt(&mut d, cx_mid, *hy, *sz, "middle", SANS, if *b { r#"font-weight="bold""# } else { "" }, t);
    }
    // parent labels
    rich(&mut d, gx + cell * nc as f64 / 2.0, p1_y, 28.0, "middle", &geno_parts(&pa, cross), true);
    rich(&mut d, ox + 30.0, gy + grid_h / 2.0 + 10.0, 28.0, "middle", &geno_parts(&pb, cross), true);
    let gsz = if nc == 4 { 20.0 } else { 26.0 };
    for (i, g) in ga.iter().enumerate() {
        rich(&mut d, gx + cell * (i as f64 + 0.5), gam_y, gsz, "middle", &gamete_parts(g, cross), true);
    }
    for (j, g) in gb.iter().enumerate() {
        rich(&mut d, gx - 14.0, gy + cell * (j as f64 + 0.5) + gsz * 0.35, gsz, "end", &gamete_parts(g, cross), true);
    }
    // cells
    let csz = if nc == 4 { 19.0 } else { 28.0 };
    for (j, row) in kids.iter().enumerate() {
        for (i, c) in row.iter().enumerate() {
            let (x, yy) = (gx + cell * i as f64, gy + cell * j as f64);
            let fill = shaded(class_of(c)).unwrap_or("#ffffff");
            d.rect(x, yy, cell, cell, fill, "#555", 1.2);
            if p.b("filled") {
                rich(&mut d, x + cell / 2.0, yy + cell / 2.0 + csz * 0.35, csz, "middle", &geno_parts(&c.genes, cross), true);
            }
        }
    }
    d.rect(gx, gy, cell * nc as f64, grid_h, "none", INK, if p.b("outline_heavy") { 3.0 } else { 1.6 });
    LAST.with(|l| *l.borrow_mut() = Some(Layout { w, gx, p1_y, gy, cell, nc, nr, gsz, ox, pb_y: gy + grid_h / 2.0 }));
    for i in 1..nc { d.line(gx + cell * i as f64, gy, gx + cell * i as f64, gy + grid_h, INK, 1.6); }
    for j in 1..nr { d.line(gx, gy + cell * j as f64, gx + cell * nc as f64, gy + cell * j as f64, INK, 1.6); }

    // ratios
    let rx = cx_mid;
    if p.b("geno_ratio") {
        let mut parts = vec![Part::b("Genotype ratio: ")];
        for (i, ((_, _), (n, gp))) in geno_count.iter().enumerate() {
            if i > 0 { parts.push(Part::plain(" : ")); }
            parts.push(Part::plain(&format!("{} ", n / gdiv.max(1))));
            parts.extend(gp.iter().cloned());
        }
        rich(&mut d, rx, yb, 19.0, "middle", &parts, true);
        yb += 30.0;
    }
    if p.b("pheno_ratio") {
        let mut parts = vec![Part::b("Phenotype ratio: ")];
        for (i, (k, n)) in ph_count.iter().enumerate() {
            if i > 0 { parts.push(Part::plain(" : ")); }
            parts.push(Part::plain(&format!("{} {}", n / pdiv.max(1), k.2)));
        }
        rich(&mut d, rx, yb, 19.0, "middle", &parts, true);
        yb += 30.0;
    }
    // key
    if key_on {
        let colw = (w - 80.0) / key_cols as f64;
        for (ci, k) in classes.iter().enumerate() {
            let (col, row) = (ci % key_cols, ci / key_cols);
            let (x, yy) = (40.0 + col as f64 * colw, yb + 4.0 + row as f64 * 26.0);
            d.rect(x, yy, 18.0, 18.0, FILLS[ci % FILLS.len()], INK, 1.2);
            txt(&mut d, x + 26.0, yy + 14.0, 14.0, "start", SANS, "", &k.2);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ratios() {
        let svg = render_case("punnett-square", "default", &[]);
        assert!(svg.contains("1 "));
        assert!(svg.contains("3 tall"));
    }

    #[test]
    fn dihybrid_ratio_9331() {
        let svg = render_case("punnett-square", "dihybrid", &[
            ("cross", s("Dihybrid")), ("parents", s("TtRr x TtRr")),
            ("phenotypes", s("tall, short; round, wrinkled")), ("shading", s("Each phenotype, with a key")),
            ("title", s("Dihybrid cross")), ("question", s("Two pea plants heterozygous for height and seed shape are crossed. Complete the square and give the ratio.")),
        ]);
        assert!(svg.contains("9 tall round : 3 tall wrinkled : 3 short round : 1 short wrinkled")
            || svg.contains("9 tall round") && svg.contains("1 short wrinkled"));
    }

    #[test]
    fn xlinked_and_incomplete() {
        let svg = render_case("punnett-square", "xlinked", &[
            ("cross", s("X-linked")), ("parents", s("XCXc x XCY")), ("phenotypes", s("normal vision, colour blind")),
            ("shading", s("Last phenotype only")),
        ]);
        assert!(svg.contains("colour blind male"));
        let svg = render_case("punnett-square", "incomplete", &[
            ("dominance", s("Incomplete")), ("parents", s("Rr x Rr")), ("phenotypes", s("red, white")),
            ("shading", s("Each phenotype, with a key")),
        ]);
        assert!(svg.contains("2 intermediate") && svg.contains("1 red") && svg.contains("1 white"));
        render_case("punnett-square", "blank", &[("filled", Value::B(false)), ("parents", s("TT x tt")), ("geno_ratio", Value::B(false))]);
        render_case("punnett-square", "bad", &[("parents", s("Tt"))]);
    }
}
