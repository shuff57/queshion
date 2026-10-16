//! Small chemistry data tables shared by the generators.

/// (symbol, name, standard atomic mass) for Z = 1..=36.
pub const ELEMENTS: [(&str, &str, f64); 36] = [
    ("H", "Hydrogen", 1.008), ("He", "Helium", 4.003), ("Li", "Lithium", 6.94), ("Be", "Beryllium", 9.012),
    ("B", "Boron", 10.81), ("C", "Carbon", 12.011), ("N", "Nitrogen", 14.007), ("O", "Oxygen", 15.999),
    ("F", "Fluorine", 18.998), ("Ne", "Neon", 20.180), ("Na", "Sodium", 22.990), ("Mg", "Magnesium", 24.305),
    ("Al", "Aluminum", 26.982), ("Si", "Silicon", 28.085), ("P", "Phosphorus", 30.974), ("S", "Sulfur", 32.06),
    ("Cl", "Chlorine", 35.45), ("Ar", "Argon", 39.948), ("K", "Potassium", 39.098), ("Ca", "Calcium", 40.078),
    ("Sc", "Scandium", 44.956), ("Ti", "Titanium", 47.867), ("V", "Vanadium", 50.942), ("Cr", "Chromium", 51.996),
    ("Mn", "Manganese", 54.938), ("Fe", "Iron", 55.845), ("Co", "Cobalt", 58.933), ("Ni", "Nickel", 58.693),
    ("Cu", "Copper", 63.546), ("Zn", "Zinc", 65.38), ("Ga", "Gallium", 69.723), ("Ge", "Germanium", 72.630),
    ("As", "Arsenic", 74.922), ("Se", "Selenium", 78.971), ("Br", "Bromine", 79.904), ("Kr", "Krypton", 83.798),
];

/// Dropdown labels like "6 C - Carbon" for the element choice.
pub const ELEMENT_CHOICES: [&str; 36] = [
    "1 H - Hydrogen", "2 He - Helium", "3 Li - Lithium", "4 Be - Beryllium", "5 B - Boron", "6 C - Carbon",
    "7 N - Nitrogen", "8 O - Oxygen", "9 F - Fluorine", "10 Ne - Neon", "11 Na - Sodium", "12 Mg - Magnesium",
    "13 Al - Aluminum", "14 Si - Silicon", "15 P - Phosphorus", "16 S - Sulfur", "17 Cl - Chlorine", "18 Ar - Argon",
    "19 K - Potassium", "20 Ca - Calcium", "21 Sc - Scandium", "22 Ti - Titanium", "23 V - Vanadium",
    "24 Cr - Chromium", "25 Mn - Manganese", "26 Fe - Iron", "27 Co - Cobalt", "28 Ni - Nickel", "29 Cu - Copper",
    "30 Zn - Zinc", "31 Ga - Gallium", "32 Ge - Germanium", "33 As - Arsenic", "34 Se - Selenium",
    "35 Br - Bromine", "36 Kr - Krypton",
];

/// Atomic number from a choice string ("6 C - Carbon") or a bare symbol.
pub fn z_of(choice: &str) -> usize {
    if let Some(n) = choice.split_whitespace().next().and_then(|t| t.parse::<usize>().ok()) {
        return n.clamp(1, 36);
    }
    ELEMENTS.iter().position(|e| e.0 == choice).map(|i| i + 1).unwrap_or(6)
}

pub fn symbol(z: usize) -> &'static str { ELEMENTS[z.clamp(1, 36) - 1].0 }
pub fn name(z: usize) -> &'static str { ELEMENTS[z.clamp(1, 36) - 1].1 }
pub fn mass_number(z: usize) -> usize { ELEMENTS[z.clamp(1, 36) - 1].2.round() as usize }

/// Aufbau filling order as (n, l).
pub const AUFBAU: [(u8, u8); 10] = [(1, 0), (2, 0), (2, 1), (3, 0), (3, 1), (4, 0), (3, 2), (4, 1), (5, 0), (4, 2)];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sub { pub n: u8, pub l: u8, pub e: u8 }

impl Sub {
    pub fn label(&self) -> String { format!("{}{}", self.n, ['s', 'p', 'd', 'f'][self.l as usize]) }
}

fn fill(e: i64) -> Vec<Sub> {
    let mut left = e.max(0);
    let mut out = Vec::new();
    for &(n, l) in AUFBAU.iter() {
        if left <= 0 { break; }
        let cap = 2 * (2 * l as i64 + 1);
        let k = left.min(cap);
        out.push(Sub { n, l, e: k as u8 });
        left -= k;
    }
    out
}

/// Ground-state configuration (aufbau order) of the species with atomic number `z` and `charge`.
pub fn config(z: usize, charge: i64) -> Vec<Sub> {
    let mut subs = fill(z as i64);
    // Known 4th-period exceptions.
    let set = |subs: &mut Vec<Sub>, s4: u8, d3: u8| {
        for s in subs.iter_mut() {
            if s.n == 4 && s.l == 0 { s.e = s4; }
            if s.n == 3 && s.l == 2 { s.e = d3; }
        }
    };
    if z == 24 { set(&mut subs, 1, 5); }
    if z == 29 { set(&mut subs, 1, 10); }
    if charge > 0 {
        for _ in 0..charge {
            // remove from the highest n, then highest l
            let mut best: Option<usize> = None;
            for (i, s) in subs.iter().enumerate() {
                if s.e == 0 { continue; }
                match best {
                    None => best = Some(i),
                    Some(b) => if (s.n, s.l) > (subs[b].n, subs[b].l) { best = Some(i) },
                }
            }
            if let Some(b) = best { subs[b].e -= 1; }
        }
        subs.retain(|s| s.e > 0);
    } else if charge < 0 {
        subs = fill(z as i64 - charge);
    }
    subs
}

/// Electrons per principal shell.
pub fn shells(z: usize, charge: i64) -> Vec<u32> {
    let mut v = vec![0u32; 7];
    for s in config(z, charge) { v[s.n as usize - 1] += s.e as u32; }
    while v.last() == Some(&0) { v.pop(); }
    v
}

pub fn noble_core(z: usize) -> Option<(usize, &'static str)> {
    [(36, "Kr"), (18, "Ar"), (10, "Ne"), (2, "He")].into_iter().find(|&(c, _)| z > c)
}

pub fn sup(n: u32) -> String {
    n.to_string().chars().map(|c| ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'][c.to_digit(10).unwrap() as usize]).collect()
}
pub fn sub(n: u32) -> String {
    n.to_string().chars().map(|c| ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'][c.to_digit(10).unwrap() as usize]).collect()
}

pub fn config_text(subs: &[Sub], core: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(c) = core { s.push_str(&format!("[{c}] ")); }
    for sb in subs { s.push_str(&format!("{}{} ", sb.label(), sup(sb.e as u32))); }
    s.trim_end().to_string()
}

/// Valence electrons, electronegativity, period for atoms used by the Lewis generator.
pub fn atom_info(sym: &str) -> Option<(i32, f64, i32)> {
    Some(match sym {
        "H" => (1, 2.20, 1), "He" => (2, 0.0, 1),
        "Li" => (1, 0.98, 2), "Be" => (2, 1.57, 2), "B" => (3, 2.04, 2), "C" => (4, 2.55, 2), "N" => (5, 3.04, 2),
        "O" => (6, 3.44, 2), "F" => (7, 3.98, 2), "Ne" => (8, 0.0, 2),
        "Na" => (1, 0.93, 3), "Mg" => (2, 1.31, 3), "Al" => (3, 1.61, 3), "Si" => (4, 1.90, 3), "P" => (5, 2.19, 3),
        "S" => (6, 2.58, 3), "Cl" => (7, 3.16, 3), "Ar" => (8, 0.0, 3),
        "Ge" => (4, 2.01, 4), "As" => (5, 2.18, 4), "Se" => (6, 2.55, 4), "Br" => (7, 2.96, 4), "Kr" => (8, 3.0, 4),
        "I" => (7, 2.66, 5), "Xe" => (8, 2.6, 5),
        _ => return None,
    })
}

/// Isotopes (mass number, abundance %) for the mass-spectrum generator.
pub const ISOTOPES: [(&str, &[(u32, f64)]); 17] = [
    ("Lithium", &[(6, 7.59), (7, 92.41)]),
    ("Boron", &[(10, 19.9), (11, 80.1)]),
    ("Carbon", &[(12, 98.93), (13, 1.07)]),
    ("Nitrogen", &[(14, 99.64), (15, 0.36)]),
    ("Oxygen", &[(16, 99.76), (17, 0.04), (18, 0.20)]),
    ("Neon", &[(20, 90.48), (21, 0.27), (22, 9.25)]),
    ("Magnesium", &[(24, 78.99), (25, 10.00), (26, 11.01)]),
    ("Silicon", &[(28, 92.23), (29, 4.68), (30, 3.09)]),
    ("Sulfur", &[(32, 94.99), (33, 0.75), (34, 4.25), (36, 0.01)]),
    ("Chlorine", &[(35, 75.76), (37, 24.24)]),
    ("Argon", &[(36, 0.34), (38, 0.06), (40, 99.60)]),
    ("Potassium", &[(39, 93.26), (40, 0.01), (41, 6.73)]),
    ("Calcium", &[(40, 96.94), (42, 0.65), (43, 0.14), (44, 2.09), (46, 0.004), (48, 0.19)]),
    ("Copper", &[(63, 69.15), (65, 30.85)]),
    ("Zinc", &[(64, 49.2), (66, 27.7), (67, 4.0), (68, 18.5), (70, 0.6)]),
    ("Bromine", &[(79, 50.69), (81, 49.31)]),
    ("Silver", &[(107, 51.84), (109, 48.16)]),
];

/// Binding energies (MJ/mol) per subshell, for Z = 1..=18, in order 1s,2s,2p,3s,3p.
pub fn pes_energies(z: usize) -> Vec<(u8, u8, f64)> {
    let t: [&[f64]; 18] = [
        &[1.31], &[2.37], &[6.26, 0.52], &[11.5, 0.90], &[19.3, 1.36, 0.80], &[28.6, 1.72, 1.09],
        &[39.6, 2.45, 1.40], &[52.6, 3.12, 1.31], &[67.2, 3.88, 1.68], &[84.0, 4.68, 2.08],
        &[104.0, 6.84, 3.67, 0.50], &[126.0, 9.07, 6.46, 0.74], &[151.0, 12.1, 9.07, 1.09, 0.58],
        &[177.0, 15.1, 12.1, 1.46, 0.79], &[208.0, 18.7, 15.7, 1.95, 1.06], &[239.0, 22.7, 19.6, 2.05, 1.00],
        &[273.0, 26.8, 23.8, 2.44, 1.25], &[309.0, 31.5, 28.5, 2.82, 1.52],
    ];
    let z = z.clamp(1, 18);
    let cfg = config(z, 0);
    cfg.iter().zip(t[z - 1].iter()).map(|(s, &en)| (s.n * 10 + s.l, s.e, en)).collect()
}
