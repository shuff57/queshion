//! verSHion desktop binary: native entry, headless hooks, and the eframe adapter.
use eframe::egui;
use vershion::{
    ai, ditto, examui, figfit, figure_agent, figure_eval, figures, formats, model, platform, store, App, Settings,
};

/// eframe adapter for the host-agnostic [`vershion::App`].
struct Desktop {
    app: App,
}

impl Desktop {
    fn new(ctx: &egui::Context) -> Self {
        Desktop { app: App::new_from(ctx) }
    }
}

impl eframe::App for Desktop {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(ctx, raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}

/// Headless pre-window handling: VERSHION_TESTING=1 clears the store dir
/// (double-guarded), VERSHION_SELFTEST=1 / VERSHION_SEED=1 build the app,
/// seed optionally, and print the count line with no window. Returns true if
/// the process should exit after printing.
fn handle_headless() -> bool {
    let xdg_set = std::env::var_os("XDG_DATA_HOME")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    if vershion::env_var("VERSHION_TESTING").as_deref() == Ok("1") {
        if !xdg_set {
            eprintln!("VERSHION_TESTING=1 refused: XDG_DATA_HOME must be set explicitly");
            std::process::exit(1);
        }
        let dir = store::store_dir();
        if dir.file_name().map(|n| n == "vershion").unwrap_or(false) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    let selftest =
        vershion::env_var("VERSHION_SELFTEST").as_deref() == Ok("1")
            || vershion::env_var("VERSHION_SEED").as_deref() == Ok("1");
    if !selftest {
        return false;
    }
    let ctx = egui::Context::default();
    let mut app = App::new_from(&ctx);
    if vershion::env_var("VERSHION_SEED").as_deref() == Ok("1") {
        app.seed_demo();
    }
    println!(
        "SELFTEST OK banks={} exams={} imports={} exports={}",
        app.store.banks.len(),
        app.store.exams.len(),
        app.store.imports.len(),
        app.store.exports.len()
    );
    true
}

fn main() -> Result<(), eframe::Error> {
    // Copy the pre-rename data dir once, then install the filesystem backend.
    store::migrate_legacy_dir();
    store::init(Box::new(store::FsBackend::from_env()));
    // Headless one-shot: VERSHION_RENDER_FIG=<figure id> [VERSHION_FIG_PARAMS='{"formula":"NH3"}']
    // VERSHION_OUT=<file.png> renders a Figures-tab figure to a PNG (strings/bools/numbers).
    if let Ok(id) = vershion::env_var("VERSHION_RENDER_FIG") {
        let def = figures::registry().into_iter().find(|d| d.id == id).unwrap_or_else(|| panic!("no figure {id}"));
        let over: serde_json::Value = vershion::env_var("VERSHION_FIG_PARAMS").ok().and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
        let p = figures::params_from_json(&def, &over);
        let png = figures::to_png(&(def.render)(&p), 1.5).expect("render");
        std::fs::write(vershion::env_var("VERSHION_OUT").expect("VERSHION_OUT"), png).expect("write");
        return Ok(());
    }
    // Headless one-shot: VERSHION_FIND_MODELS=1 runs the Settings "Find models" check against the configured
    // provider and prints the note and the models kept.
    #[cfg(not(target_arch = "wasm32"))]
    if vershion::env_var("VERSHION_FIND_MODELS").as_deref() == Ok("1") {
        let settings: Settings = store::load_json("settings.json").unwrap_or_default();
        let cfg = ai::AiCfg::from(&settings);
        match ai::check_models(&cfg, &|d, n| eprintln!("checking {d}/{n}")) {
            Ok(c) => {
                println!("{}", ai::checked_note(&c));
                println!("kept: {}", c.ok.join(", "));
            }
            Err(e) => {
                eprintln!("find models failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if vershion::env_var("VERSHION_FIGURE_CATALOG").as_deref() == Ok("1") {
        println!("{}", figure_agent::catalog_text());
        return Ok(());
    }
    // Headless one-shot: VERSHION_FIGURE_EVAL=<cases.json> [VERSHION_EVAL_REPEATS=3] [VERSHION_EVAL_ONLY=<name part>]
    // [VERSHION_EVAL_OUT=<report.json>] runs the figure agent's eval cases against the configured provider.
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(path) = vershion::env_var("VERSHION_FIGURE_EVAL") {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let base = std::path::Path::new(&path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let mut cases = figure_eval::parse_cases(&text).unwrap_or_else(|e| panic!("{e}"));
        if let Ok(only) = vershion::env_var("VERSHION_EVAL_ONLY") {
            cases.retain(|c| c.name.contains(&only) || c.level == only);
        }
        let bad: Vec<String> = cases.iter().flat_map(|c| figure_eval::validate(c, &base).into_iter().map(move |p| format!("{}: {p}", c.name))).collect();
        if !bad.is_empty() {
            eprintln!("invalid cases:\n{}", bad.join("\n"));
            std::process::exit(1);
        }
        let repeats = vershion::env_var("VERSHION_EVAL_REPEATS").ok().and_then(|n| n.parse().ok()).unwrap_or(3);
        let settings: Settings = store::load_json("settings.json").unwrap_or_default();
        let mut cfg = ai::AiCfg::from(&settings);
        // VERSHION_EVAL_MODEL=<model> tries another model without touching settings.json.
        if let Ok(m) = vershion::env_var("VERSHION_EVAL_MODEL") {
            cfg.model = m;
        }
        let mut judge_cfg = cfg.clone();
        if let Ok(m) = vershion::env_var("VERSHION_JUDGE_MODEL") {
            judge_cfg.model = m;
        }
        let mut judge = figure_eval::ModelJudge { cfg: &judge_cfg };
        let judging = vershion::env_var("VERSHION_EVAL_JUDGE").as_deref() == Ok("1");
        // VERSHION_EVAL_TRIES=<n> lets each run take up to n reviewed attempts (needs the judge).
        let tries = vershion::env_var("VERSHION_EVAL_TRIES").ok().and_then(|n| n.parse().ok()).unwrap_or(1);
        let results = figure_eval::run_all_tries(
            &cases,
            repeats,
            &base,
            &mut figure_agent::ModelChat { cfg: &cfg },
            if judging || tries > 1 { Some(&mut judge) } else { None },
            tries,
            |m| eprintln!("running {m}"),
        );
        // Pictures to look at: VERSHION_EVAL_PICS=<dir> saves each final figure and each reference.
        if let Ok(dir) = vershion::env_var("VERSHION_EVAL_PICS") {
            std::fs::create_dir_all(&dir).expect("pics dir");
            for (c, r) in cases.iter().zip(&results) {
                if let Some(Ok(png)) = c.reference.as_ref().map(|x| figure_eval::reference_png(x, &base)) {
                    std::fs::write(format!("{dir}/{}-ref.png", c.name), png).expect("write");
                }
                for (i, run) in r.runs.iter().enumerate() {
                    if let Some(png) = &run.png {
                        std::fs::write(format!("{dir}/{}-{}.png", c.name, i + 1), png).expect("write");
                    }
                }
            }
        }
        println!("{}", figure_eval::format_report(&results));
        if let Ok(out) = vershion::env_var("VERSHION_EVAL_OUT") {
            std::fs::write(out, serde_json::to_string_pretty(&results).unwrap_or_default()).expect("write");
        }
        return Ok(());
    }
    // Headless one-shot: VERSHION_FIGURE_AGENT="<request>" VERSHION_OUT=<file.png> runs the figure
    // agent loop against the configured provider, prints each step and the chosen figure spec.
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(request) = vershion::env_var("VERSHION_FIGURE_AGENT") {
        let settings: Settings = store::load_json("settings.json").unwrap_or_default();
        let cfg = ai::AiCfg::from(&settings);
        let reference = vershion::env_var("VERSHION_FIGURE_IMAGE").ok().map(|p| std::fs::read(&p).unwrap_or_else(|e| panic!("cannot read {p}: {e}")));
        // With a picture, a reviewer checks each figure against it and the agent gets up to 3 tries.
        let mut judge = figure_agent::ModelJudge { cfg: &cfg };
        let run = figure_agent::run_with_retries(
            &request,
            reference.as_deref(),
            &mut figure_agent::ModelChat { cfg: &cfg },
            Some(&mut judge),
            figure_agent::MAX_TRIES,
        );
        if run.tries > 1 {
            eprintln!("took {} attempts", run.tries);
        }
        for s in &run.steps {
            println!("{:<14} {}", s.tool, s.summary);
        }
        if let (Some(png), Ok(path)) = (&run.png, vershion::env_var("VERSHION_OUT")) {
            std::fs::write(path, png).expect("write");
        }
        match run.spec {
            Some(spec) => println!("{}", serde_json::json!({"figure": spec, "note": run.note})),
            None if run.declined.is_some() => println!("declined: {}", run.declined.unwrap_or_default()),
            None => {
                eprintln!("figure agent failed: {}", run.error.unwrap_or_default());
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Headless one-shot: VERSHION_EXPORT=<exam index> VERSHION_OUT=<dir> writes exam.pdf, exam.docx
    // and key.pdf for that exam of the store (the stem-rendering checks read these).
    if let Ok(i) = vershion::env_var("VERSHION_EXPORT") {
        let dir = std::path::PathBuf::from(vershion::env_var("VERSHION_OUT").expect("VERSHION_OUT"));
        std::fs::create_dir_all(&dir).expect("out dir");
        match examui::headless_export(i.parse().unwrap_or(0), &dir) {
            Ok(()) => println!("exported to {}", dir.display()),
            Err(e) => {
                eprintln!("export failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Headless one-shot: VERSHION_FIT=<crop.png> runs the figure-fitting loop on one picture.
    if let Ok(png) = vershion::env_var("VERSHION_FIT") {
        let bytes = std::fs::read(&png).unwrap_or_else(|e| panic!("cannot read {png}: {e}"));
        let settings: Settings = store::load_json("settings.json").unwrap_or_default();
        let cfg = ai::AiCfg::from(&settings);
        match figfit::fit(&bytes, "Give the number of bonding and nonbonding electrons for the structure below:", &cfg) {
            Ok(Some(f)) => println!("fit: {} {:?}", f.spec.id, f.spec.params.iter().filter(|(k, _)| *k == "formula" || *k == "preset").collect::<Vec<_>>()),
            Ok(None) => println!("no fit"),
            Err(e) => println!("error: {e}"),
        }
        return Ok(());
    }
    // Headless one-shot: VERSHION_IMPORT_PDF=<file.pdf> [VERSHION_BANK_NAME=..]
    // runs the Import screen's whole AI path (pages -> model -> Ditto parse -> figure crops)
    // and saves the result as a new bank in the store.
    if let Ok(pdf) = vershion::env_var("VERSHION_IMPORT_PDF") {
        import_pdf_headless(&pdf);
        return Ok(());
    }
    // Headless one-shot: VERSHION_PARSE=<reply text> [VERSHION_PAGE=<page.png> VERSHION_PAGE_NO=<n>]
    // runs the AI-import parser (figures cropped from the page image) and prints
    // {questions, warnings, figures} as JSON; the import test battery scores this.
    if let Ok(path) = vershion::env_var("VERSHION_PARSE") {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let text = String::from_utf8_lossy(&bytes).to_string();
        let page = vershion::env_var("VERSHION_PAGE").ok().and_then(|p| std::fs::read(p).ok());
        let page_no = vershion::env_var("VERSHION_PAGE_NO").ok().and_then(|n| n.parse().ok()).unwrap_or(1);
        let res = if ditto::is_ditto(&text) {
            ditto::import_text(&text, page.as_deref(), page_no, "converted")
        } else {
            formats::detect_and_parse(&bytes, &path).map(|doc| ditto::DittoImport { doc, warnings: Vec::new(), figures: 0 })
        };
        match res {
            Ok(r) => println!(
                "{}",
                serde_json::json!({"questions": r.doc.questions, "warnings": r.warnings, "figures": r.figures, "store": store::store_dir_string()})
            ),
            Err(e) => {
                eprintln!("parse failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Headless one-shot: VERSHION_CONVERT=<file> [VERSHION_HINT="..."] transcribes a
    // photo/scan through the configured provider and prints the Blackboard import text.
    if let Ok(path) = vershion::env_var("VERSHION_CONVERT") {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let mime = match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "pdf" => "application/pdf",
            _ => "",
        };
        let settings: Settings = store::load_json("settings.json").unwrap_or_default();
        let cfg = ai::AiCfg::from(&settings);
        let hint = vershion::env_var("VERSHION_HINT").unwrap_or_default();
        let text = ai::transcribe_checked(&bytes, mime, &cfg, &ai::prompt_for(&hint))
            .map(|raw| ai::extract_import_text(&raw));
        match text {
            Ok(t) => println!("{t}"),
            Err(e) => {
                eprintln!("conversion failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Headless modes...
    if handle_headless() {
        return Ok(());
    }
    // Test hook: VERSHION_WINSIZE=WxH forces the initial inner size (e.g. 860x700).
    let size = vershion::env_var("VERSHION_WINSIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.trim().split_once('x')?;
            Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
        })
        .unwrap_or([1180.0, 780.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("verSHion")
            .with_decorations(true)
            .with_inner_size(size)
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("verSHion", options, Box::new(|cc| Ok(Box::new(Desktop::new(&cc.egui_ctx)))))
}
fn import_pdf_headless(pdf: &str) {
    let bytes = std::fs::read(pdf).unwrap_or_else(|e| panic!("cannot read {pdf}: {e}"));
    let pages = platform::render_pdf_pages(&bytes).unwrap_or_else(|e| panic!("{e}"));
    let settings: Settings = store::load_json("settings.json").unwrap_or_default();
    let cfg = ai::AiCfg::from(&settings);
    let converted: Vec<Result<ai::Converted, String>> = std::thread::scope(|sc| {
        let hs: Vec<_> = pages.iter().enumerate().map(|(i, png)| sc.spawn({ let cfg = &cfg; move || ai::convert_page(png, "image/png", "", cfg, i + 1) })).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("thread panicked".into()))).collect()
    });
    let mut questions = Vec::new();
    for (i, c) in converted.into_iter().enumerate() {
        let c = c.unwrap_or_else(|e| panic!("page {} failed: {e}", i + 1));
        eprintln!("page {}: {} questions, {} cropped, {} redrawn, notes {:?}", i + 1, c.doc.questions.len(), c.cropped, c.redrawn, c.warnings);
        questions.extend(c.doc.questions);
    }
    for q in &mut questions {
        q.points.get_or_insert(1.0);
    }
    let name = vershion::env_var("VERSHION_BANK_NAME").unwrap_or_else(|_| "Imported questions".into());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut st = store::Store::load();
    let n = questions.len();
    st.banks.push(model::QuestionBank { id: model::new_id(), name: name.clone(), questions, created: now, modified: now, details: Default::default() });
    st.save_banks();
    println!("saved bank \"{name}\" with {n} questions to {}", store::store_dir_string());
}
