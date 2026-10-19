fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/vs_svgs".to_string());
    std::fs::create_dir_all(&out).unwrap();
    for def in vershion::figures::registry() {
        let p = vershion::figures::start_params(&def);
        let svg = (def.render)(&p);
        std::fs::write(format!("{out}/{}.svg", def.id), svg).unwrap();
    }
}
