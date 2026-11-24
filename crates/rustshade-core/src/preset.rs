use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Interpret effect workspace as linear light instead of display (sRGB)
    /// values. Off by default so CPU and shader paths match ReShade semantics.
    #[serde(default)]
    pub linear_workspace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffectSpec {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Raw parameters as written in the preset; validated against the
    /// registry (unknown ones are rejected there), then interpreted by a
    /// backend (CPU fx or GPU shaders).
    #[serde(flatten)]
    pub params: toml::Table,
}

fn default_enabled() -> bool {
    true
}

impl EffectSpec {
    /// Named parameter accessor with type coercion (ints are accepted where
    /// floats are declared).
    pub fn get_f32(&self, name: &str) -> Option<f32> {
        let v = self.params.get(name)?;
        if let Some(f) = v.as_float() {
            Some(f as f32)
        } else {
            v.as_integer().map(|i| i as f32)
        }
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.params.get(name)?.as_bool()
    }

    pub fn get_str(&self, name: &str) -> Option<&str> {
        self.params.get(name)?.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    #[serde(default)]
    pub settings: Settings,
    /// Applied in array order, first to last.
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
}

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("cannot read preset {0}: {1}")]
    Io(String, #[source] std::io::Error),
    #[error("invalid preset {0}")]
    Parse(String, #[source] toml::de::Error),
    #[error("preset has errors:\n{0}")]
    Invalid(String),
}

impl Preset {
    pub fn parse(s: &str) -> Result<Self, PresetError> {
        let preset: Preset =
            toml::from_str(s).map_err(|e| PresetError::Parse("<memory>".into(), e))?;
        Ok(preset)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, PresetError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| PresetError::Io(path.display().to_string(), e))?;
        let preset: Preset =
            toml::from_str(&text).map_err(|e| PresetError::Parse(path.display().to_string(), e))?;
        Ok(preset)
    }

    pub fn default_preset() -> Self {
        toml::from_str(include_str!("../assets/default_preset_inline.toml"))
            .expect("built-in preset is valid")
    }
}
