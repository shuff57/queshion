//! Figure specs: the JSON form a question stores (`FigureSpec`) and the `Params` the renderers
//! take, plus the white flattening applied to rasters before export. No UI dependencies.

use std::collections::BTreeMap;

use super::{FigureDef, ParamDef, Params, Value};
use crate::model::FigureSpec;

pub fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::F(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::I(i) => serde_json::Value::from(*i),
        Value::B(b) => serde_json::Value::Bool(*b),
        Value::S(s) => serde_json::Value::String(s.clone()),
    }
}

/// Reads a stored JSON value using the def's default for the expected type.
fn value_from_json(def: &ParamDef, j: &serde_json::Value) -> Option<Value> {
    match &def.default {
        Value::F(_) => j.as_f64().map(Value::F),
        Value::I(_) => j.as_i64().or_else(|| j.as_f64().map(|f| f.round() as i64)).map(Value::I),
        Value::B(_) => j.as_bool().map(Value::B),
        Value::S(_) => j.as_str().map(|s| Value::S(s.to_string())),
    }
}

pub fn spec_for(id: &str, p: &Params) -> FigureSpec {
    FigureSpec {
        id: id.to_string(),
        params: p.0.iter().map(|(k, v)| (k.clone(), value_to_json(v))).collect::<BTreeMap<_, _>>(),
    }
}

/// Defaults overlaid with whatever the spec stored (unknown keys / wrong types are ignored).
pub fn params_from_spec(def: &FigureDef, spec: &FigureSpec) -> Params {
    let defs = (def.params)();
    let mut p = Params::defaults(&defs);
    for d in &defs {
        if let Some(v) = spec.params.get(d.key).and_then(|j| value_from_json(d, j)) {
            p.0.insert(d.key.to_string(), v);
        }
    }
    super::migrate(def.id, spec, &mut p);
    p.normalize(&defs, false);
    p
}

/// Composites over white and re-encodes as PNG (exports turn transparency black otherwise).
pub fn flatten_white(png: &[u8]) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(png).map_err(|e| e.to_string())?.to_rgba8();
    for px in img.pixels_mut() {
        let a = px[3] as u32;
        for c in 0..3 {
            px[c] = ((px[c] as u32 * a + 255 * (255 - a)) / 255) as u8;
        }
        px[3] = 255;
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}
