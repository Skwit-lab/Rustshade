use crate::preset::{EffectSpec, Preset};
use crate::registry::{effect_def, ParamKind};

#[derive(Debug, Clone, thiserror::Error)]
pub enum EffectError {
    #[error("  - unknown effect id '{0}' (see `rustshade list`)")]
    UnknownEffect(String),
    #[error("  - effect '{effect}': unknown parameter '{param}'")]
    UnknownParam { effect: String, param: String },
    #[error(
        "  - effect '{effect}': parameter '{param}' must be a number in [{min}, {max}], got {got}"
    )]
    BadNumber {
        effect: String,
        param: String,
        min: f32,
        max: f32,
        got: f32,
    },
    #[error("  - effect '{effect}': parameter '{param}' must be a boolean")]
    BadBool { effect: String, param: String },
    #[error("  - effect '{effect}': parameter '{param}' must be a string")]
    BadString { effect: String, param: String },
}

fn check_effect(spec: &EffectSpec) -> Vec<EffectError> {
    let Some(def) = effect_def(&spec.id) else {
        return vec![EffectError::UnknownEffect(spec.id.clone())];
    };
    let mut errs = Vec::new();
    for (k, v) in &spec.params {
        let Some(pdef) = def.params.iter().find(|p| p.name == *k) else {
            errs.push(EffectError::UnknownParam {
                effect: def.id.to_string(),
                param: k.to_string(),
            });
            continue;
        };
        match pdef.kind {
            ParamKind::F32 => {
                let got = v
                    .as_float()
                    .map(|f| f as f32)
                    .or_else(|| v.as_integer().map(|i| i as f32));
                match got {
                    Some(f) if f >= pdef.min && f <= pdef.max => {}
                    Some(f) => errs.push(EffectError::BadNumber {
                        effect: def.id.to_string(),
                        param: pdef.name.to_string(),
                        min: pdef.min,
                        max: pdef.max,
                        got: f,
                    }),
                    None => errs.push(EffectError::BadNumber {
                        effect: def.id.to_string(),
                        param: pdef.name.to_string(),
                        min: pdef.min,
                        max: pdef.max,
                        got: f32::NAN,
                    }),
                }
            }
            ParamKind::Bool => {
                if v.as_bool().is_none() {
                    errs.push(EffectError::BadBool {
                        effect: def.id.to_string(),
                        param: pdef.name.to_string(),
                    });
                }
            }
            ParamKind::String => {
                if v.as_str().is_none() {
                    errs.push(EffectError::BadString {
                        effect: def.id.to_string(),
                        param: pdef.name.to_string(),
                    });
                }
            }
        }
    }
    errs
}

/// Validates every effect against the registry. Returns all errors, not just
/// the first one, so users can fix a preset in one pass.
pub fn validate(preset: &Preset) -> Result<(), Vec<EffectError>> {
    let errs: Vec<EffectError> = preset.effects.iter().flat_map(check_effect).collect();
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}
