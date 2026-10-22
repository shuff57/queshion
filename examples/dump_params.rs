fn main() {
    for def in vershion::figures::registry() {
        let p = vershion::figures::start_params(&def);
        let _ = p;
        for d in (def.params)() {
            let opts = match &d.kind { vershion::figures::Kind::Choice(v) => v.join(" | "), _ => String::new() };
            println!("{}\t{}\t{}\t{}", def.id, d.key, d.label, opts);
        }
    }
}
