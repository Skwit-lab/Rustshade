use rustshade_core::{validate, EffectSpec, Preset};

use crate::buffer::Frame;
use crate::effects::bloom::Bloom;
use crate::effects::chromatic_aberration::ChromaticAberration;
use crate::effects::fxaa::Fxaa;
use crate::effects::grade::Grade;
use crate::effects::grain::Grain;
use crate::effects::lut::{Lut3D, LutEffect, LutError};
use crate::effects::scanlines::Scanlines;
use crate::effects::sharpen::Sharpen;
use crate::effects::vignette::Vignette;
use std::path::{Path, PathBuf};

/// Per-call context handed to every effect. Kept intentionally open so
/// future passes (depth/normal G-buffer data for SDFGI-style lighting,
/// fullbright helpers) can be added without breaking effect signatures.
#[derive(Debug, Clone, Copy)]
pub struct Ctx {
    pub width: u32,
    pub height: u32,
    pub frame_index: u64,
}

pub trait Effect {
    fn id(&self) -> &'static str;

    /// Optional depth texture from the hooked game (0..1 reversed-Z style).
    /// Only populated by backends that can extract a depth buffer; CPU/GPU
    /// standalone paths pass `None`. Effects like future SDFGI or
    /// depth-fog use this.
    fn uses_depth(&self) -> bool {
        false
    }

    fn apply(&mut self, frame: &mut Frame, ctx: &Ctx);
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("preset invalid: {0:#?}")]
    Invalid(Vec<rustshade_core::EffectError>),
    #[error("{effect}: missing required parameter '{param}'")]
    MissingParam { effect: String, param: String },
    #[error("lut: {0}")]
    Lut(#[from] LutError),
}

/// Builds a runnable effect stack from a validated preset.
pub fn build_pipeline(
    preset: &Preset,
    lut_base_dir: impl AsRef<Path>,
) -> Result<Vec<Box<dyn Effect>>, PipelineError> {
    validate(preset).map_err(PipelineError::Invalid)?;

    let mut stack: Vec<Box<dyn Effect>> = Vec::new();
    let base = lut_base_dir.as_ref();
    for spec in &preset.effects {
        if !spec.enabled {
            continue;
        }
        let effect = build_effect(spec, base)?;
        stack.push(effect);
    }
    Ok(stack)
}

fn build_effect(spec: &EffectSpec, base: &Path) -> Result<Box<dyn Effect>, PipelineError> {
    Ok(match spec.id.as_str() {
        "grade" => Box::new(Grade::from_spec(spec)),
        "bloom" => Box::new(Bloom::from_spec(spec)),
        "sharpen" => Box::new(Sharpen::from_spec(spec)),
        "fxaa" => Box::new(Fxaa::from_spec(spec)),
        "vignette" => Box::new(Vignette::from_spec(spec)),
        "chromatic_aberration" => Box::new(ChromaticAberration::from_spec(spec)),
        "grain" => Box::new(Grain::from_spec(spec)),
        "scanlines" => Box::new(Scanlines::from_spec(spec)),
        "lut" => {
            let path = super::effects::get_str(spec, "path")?;
            let lut_path = resolve_path(Path::new(path), base);
            let lut = Lut3D::load(&lut_path)?;
            Box::new(LutEffect {
                lut,
                opacity: spec.get_f32("opacity").unwrap_or(1.0),
            })
        }
        other => {
            // validate() already caught this; defensive fallback.
            return Err(PipelineError::MissingParam {
                effect: other.to_string(),
                param: "<unknown effect id>".to_string(),
            });
        }
    })
}

fn resolve_path(p: &Path, base: &Path) -> PathBuf {
    if p.is_absolute() || base.as_os_str().is_empty() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustshade_core::Preset;

    #[test]
    fn builds_and_applies_all_effects() {
        let preset = Preset::parse(
            r#"
            [[effects]]
            id = "fxaa"

            [[effects]]
            id = "grade"
            exposure = 0.5

            [[effects]]
            id = "bloom"
            strength = 0.5

            [[effects]]
            id = "sharpen"
            amount = 1.0

            [[effects]]
            id = "vignette"
            amount = 0.5

            [[effects]]
            id = "chromatic_aberration"
            strength = 0.3

            [[effects]]
            id = "grain"
            strength = 0.1

            [[effects]]
            id = "scanlines"
            amount = 0.2
        "#,
        )
        .unwrap();

        let mut stack = build_pipeline(&preset, "").unwrap();
        assert_eq!(stack.len(), 8);

        let img = image::RgbaImage::from_fn(64, 64, |x, y| {
            let v = ((x as f32 + y as f32) / 128.0 * 255.0) as u8;
            image::Rgba([v, 100, 155, 255])
        });
        let mut frame = crate::buffer::Frame::from_rgba8(&img);
        let ctx = Ctx {
            width: 64,
            height: 64,
            frame_index: 3,
        };
        for effect in stack.iter_mut() {
            effect.apply(&mut frame, &ctx);
        }
        // All values stay finite and bounded.
        for v in &frame.data {
            assert!(*v >= -0.01 && *v <= 1.01, "value out of range: {v}");
        }
    }

    #[test]
    fn disabled_effects_are_skipped() {
        let preset = Preset::parse(
            r#"
            [[effects]]
            id = "grade"
            enabled = false
            exposure = 1.0
            "#,
        )
        .unwrap();
        let stack = build_pipeline(&preset, "").unwrap();
        assert!(stack.is_empty());
    }

    #[test]
    fn invalid_preset_fails_to_build() {
        let preset = Preset::parse(
            r#"
            [[effects]]
            id = "does_not_exist"
            "#,
        )
        .unwrap();
        assert!(build_pipeline(&preset, "").is_err());
    }
}
