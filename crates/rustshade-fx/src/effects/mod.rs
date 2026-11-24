pub mod bloom;
pub mod chromatic_aberration;
pub mod fxaa;
pub mod grade;
pub mod grain;
pub mod lut;
pub mod scanlines;
pub mod sharpen;
pub mod vignette;

use crate::pipeline::PipelineError;
use rustshade_core::EffectSpec;

/// Pulls a float parameter, falling back to the registry default.
pub(crate) fn get_f32_or_default(spec: &EffectSpec, name: &str, default: f32) -> f32 {
    spec.get_f32(name).unwrap_or(default)
}

/// Pulls a required string parameter.
pub(crate) fn get_str<'a>(spec: &'a EffectSpec, name: &str) -> Result<&'a str, PipelineError> {
    spec.get_str(name)
        .ok_or_else(|| PipelineError::MissingParam {
            effect: spec.id.clone(),
            param: name.to_string(),
        })
}
