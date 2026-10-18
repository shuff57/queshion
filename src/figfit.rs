//! Figure fitting: a picture cropped from an imported page becomes an editable Figures-tab
//! figure when one of the generators can reproduce it. Same shape as viSHual's generation
//! loop: the model proposes a structured spec, the app validates and renders it, the model
//! checks the render against the original, and one repair round fixes what it flagged.
//! When nothing fits, the caller keeps the plain cropped image.
use crate::ai::{ask_images, AiCfg};
use crate::figures::{self, FigureDef, Kind, Value as PV};
use crate::model::FigureSpec;
use serde_json::Value;

pub struct Fit {
    pub spec: FigureSpec,
    pub png: Vec<u8>,
}

fn catalog() -> String {
    figures::registry().iter().map(|d| format!("- {}: {} — {}", d.id, d.title, d.blurb)).collect::<Vec<_>>().join("\n")
}

fn param_help(def: &FigureDef) -> String {
    (def.params)()
        .iter()
        .map(|p| {
            let default = match &p.default {
                PV::F(v) => v.to_string(),
                PV::I(v) => v.to_string(),
                PV::B(v) => v.to_string(),
                PV::S(v) => format!("{v:?}"),
            };
            let kind = match &p.kind {
                Kind::Float { min, max, .. } => format!("number {min}..{max}"),
                Kind::Int { min, max } => format!("integer {min}..{max}"),
                Kind::Bool => "true/false".into(),
                Kind::Choice(o) => format!("one of {}", o.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", ")),
                Kind::Text => "text".into(),
                Kind::Long => "multi-line text".into(),
                Kind::List => "list as text".into(),
                Kind::Color(o) => format!("colour, one of {}", o.join(", ")),
            };
            format!("- {} ({kind}; default {default}): {}", p.key, p.label)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `VERSHION_FIT_DEBUG=1` prints each step of the fitting loop to stderr.
fn debug(msg: &str) {
    if crate::env_var("VERSHION_FIT_DEBUG").is_ok() {
        eprintln!("[figfit] {msg}");
    }
}

fn ask_json(images: &[&[u8]], cfg: &AiCfg, prompt: &str) -> Result<Value, String> {
    let reply = ask_images(images, cfg, prompt, 0.0)?;
    crate::ditto::json_object(&reply).ok_or_else(|| "the model did not answer with JSON".to_string())
}

fn render(def: &FigureDef, params: &Value) -> Result<(FigureSpec, Vec<u8>), String> {
    let mut spec = FigureSpec { id: def.id.to_string(), params: Default::default() };
    for (k, v) in params.as_object().into_iter().flatten() {
        spec.params.insert(k.clone(), v.clone());
    }
    // params_from_spec coerces by declared kind, drops unknown keys, clamps to ranges.
    let p = crate::figuresui::params_from_spec(def, &spec);
    let png = figures::to_png(&(def.render)(&p), 2.0)?;
    let png = crate::figuresui::flatten_white(&png)?;
    Ok((crate::figuresui::spec_for(def.id, &p), png))
}

/// `Ok(None)` when no generator reproduces the picture closely enough.
pub fn fit(crop: &[u8], context: &str, cfg: &AiCfg) -> Result<Option<Fit>, String> {
    if crop.is_empty() {
        return Ok(None);
    }
    let pick = ask_json(
        &[crop],
        cfg,
        &format!(
            "This picture was cut from a test question: \"{}\".\nWhich generator can redraw THIS picture closely (same kind of drawing and same subject, not just the same subject area)?\n{}\n\nAnswer JSON only: {{\"id\": \"<generator id>\" or null, \"reason\": \"...\"}}. Use null unless one clearly fits.",
            context.chars().take(200).collect::<String>(),
            catalog()
        ),
    )?;
    debug(&format!("pick: {pick}"));
    let Some(id) = pick.get("id").and_then(Value::as_str) else { return Ok(None) };
    let Some(def) = figures::registry().into_iter().find(|d| d.id == id) else { return Ok(None) };

    let mut feedback = String::new();
    for _round in 0..3 {
        let spec = ask_json(
            &[crop],
            cfg,
            &format!(
                "Set the \"{}\" generator so it redraws this picture. Question text: \"{}\".\nParameters:\n{}\n\nAnswer JSON only: {{\"params\": {{...}}}}, listing only the parameters that must differ from their defaults, with values of the stated kind. When a parameter offers a list of options and one option names what the picture shows, choose that option instead of typing free text.{feedback}",
                def.title,
                context.chars().take(200).collect::<String>(),
                param_help(&def)
            ),
        )?;
        debug(&format!("params: {spec}"));
        let spec_json = spec.get("params").cloned().unwrap_or(Value::Null).to_string();
        let Ok((spec, png)) = render(&def, spec.get("params").unwrap_or(&Value::Null)) else { break };
        let verdict = ask_json(
            &[crop, &png],
            cfg,
            "Image 1 is the original picture. Image 2 is a redraw made by a generator. Do they show the same thing: same molecule or structure, same labels and values, same layout? Ignore line thickness, font and exact position.\nAnswer JSON only: {\"same\": true or false, \"differences\": \"what is different, as a short instruction for fixing the parameters\"}",
        )?;
        debug(&format!("verdict: {verdict}"));
        if verdict.get("same").and_then(Value::as_bool) == Some(true) {
            return Ok(Some(Fit { spec, png }));
        }
        feedback.push_str(&format!(
            "\nAn earlier attempt with parameters {} was compared with the original and differed: {}",
            spec_json,
            verdict.get("differences").and_then(Value::as_str).unwrap_or("it did not match")
        ));
    }
    Ok(None)
}

/// Browser version of [`fit`], awaiting each model round trip instead of blocking.
#[cfg(target_arch = "wasm32")]
async fn ask_json_async(images: &[&[u8]], cfg: &AiCfg, prompt: &str) -> Result<Value, String> {
    let reply = crate::ai::ask_images_async(images, cfg, prompt, 0.0).await?;
    crate::ditto::json_object(&reply).ok_or_else(|| "the model did not answer with JSON".to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn fit_async(crop: &[u8], context: &str, cfg: &AiCfg) -> Result<Option<Fit>, String> {
    if crop.is_empty() {
        return Ok(None);
    }
    let pick = ask_json_async(
        &[crop],
        cfg,
        &format!(
            "This picture was cut from a test question: \"{}\".\nWhich generator can redraw THIS picture closely (same kind of drawing and same subject, not just the same subject area)?\n{}\n\nAnswer JSON only: {{\"id\": \"<generator id>\" or null, \"reason\": \"...\"}}. Use null unless one clearly fits.",
            context.chars().take(200).collect::<String>(),
            catalog()
        ),
    )
    .await?;
    debug(&format!("pick: {pick}"));
    let Some(id) = pick.get("id").and_then(Value::as_str) else { return Ok(None) };
    let Some(def) = figures::registry().into_iter().find(|d| d.id == id) else { return Ok(None) };

    let mut feedback = String::new();
    for _round in 0..3 {
        let spec = ask_json_async(
            &[crop],
            cfg,
            &format!(
                "Set the \"{}\" generator so it redraws this picture. Question text: \"{}\".\nParameters:\n{}\n\nAnswer JSON only: {{\"params\": {{...}}}}, listing only the parameters that must differ from their defaults, with values of the stated kind. When a parameter offers a list of options and one option names what the picture shows, choose that option instead of typing free text.{feedback}",
                def.title,
                context.chars().take(200).collect::<String>(),
                param_help(&def)
            ),
        )
        .await?;
        debug(&format!("params: {spec}"));
        let spec_json = spec.get("params").cloned().unwrap_or(Value::Null).to_string();
        let Ok((spec, png)) = render(&def, spec.get("params").unwrap_or(&Value::Null)) else { break };
        let verdict = ask_json_async(
            &[crop, &png],
            cfg,
            "Image 1 is the original picture. Image 2 is a redraw made by a generator. Do they show the same thing: same molecule or structure, same labels and values, same layout? Ignore line thickness, font and exact position.\nAnswer JSON only: {\"same\": true or false, \"differences\": \"what is different, as a short instruction for fixing the parameters\"}",
        )
        .await?;
        debug(&format!("verdict: {verdict}"));
        if verdict.get("same").and_then(Value::as_bool) == Some(true) {
            return Ok(Some(Fit { spec, png }));
        }
        feedback.push_str(&format!(
            "\nAn earlier attempt with parameters {} was compared with the original and differed: {}",
            spec_json,
            verdict.get("differences").and_then(Value::as_str).unwrap_or("it did not match")
        ));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_every_figure_and_lewis_exposes_its_presets() {
        assert_eq!(catalog().lines().count(), figures::registry().len());
        let lewis = figures::registry().into_iter().find(|d| d.id == "lewis-structures").unwrap();
        assert!(param_help(&lewis).contains("CH₃COCl, acetyl chloride"));
    }

    #[test]
    fn a_spec_renders_and_unknown_or_out_of_range_params_are_tolerated() {
        let lewis = figures::registry().into_iter().find(|d| d.id == "lewis-structures").unwrap();
        let params = serde_json::json!({"preset": "CH₃COCl, acetyl chloride", "bogus": 3, "lone": "yes"});
        let (spec, png) = render(&lewis, &params).unwrap();
        assert_eq!(spec.id, "lewis-structures");
        assert!(png.starts_with(b"\x89PNG"));
        assert!(!spec.params.contains_key("bogus"));
        assert_eq!(spec.params["preset"], "CH₃COCl, acetyl chloride");
    }
}
