//! The renderer as it was before families could be edited by hand, kept to check that generated
//! families (and their typed overrides) still draw exactly as before.
use super::*;

#[allow(clippy::all)]
pub fn legacy_render(p: &Params) -> String {
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
            // P4: the key's proband arrow got a shorter tail (in both renderers)
            if *extra == 2 { key_proband_mark(&mut d, x, y, 14.0); }
            txt(&mut d, x + 28.0, y + 6.0, 16.0, "start", SANS, "", label);
        }
        ky_end = ky + 20.0 + key_rows as f64 * 40.0 + 4.0;
    }
    if let Some(t) = answer_txt {
        txt(&mut d, w / 2.0, ky_end + 30.0, 18.0, "middle", SANS, "", &t);
    }
    d.finish()
}
