//! Preset/config core for rustshade.
//!
//! Defines the TOML preset format, the effect parameter registry used for
//! validation by all backends (CPU fx, GPU, hooks), and a cheap mtime-based
//! hot-reload helper.

mod preset;
mod registry;
mod validate;

pub use preset::{EffectSpec, Preset, PresetError, Settings};
pub use registry::{effect_def, EffectDef, ParamDef, ParamKind, REGISTRY};
pub use validate::{validate, EffectError};

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A loaded preset plus the metadata needed to detect file changes.
pub struct LoadedPreset {
    pub preset: Preset,
    path: PathBuf,
    mtime: Option<SystemTime>,
}

impl LoadedPreset {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PresetError> {
        let path = path.as_ref();
        let preset = Preset::load(path)?;
        let mtime = signed_mtime(path);
        Ok(Self {
            preset,
            path: path.to_path_buf(),
            mtime,
        })
    }

    /// Re-reads the preset from disk if the file changed on disk.
    /// Returns `true` when the preset was updated.
    pub fn reload_if_changed(&mut self) -> Result<bool, PresetError> {
        let mtime = signed_mtime(&self.path);
        if mtime == self.mtime {
            return Ok(false);
        }
        self.preset = Preset::load(&self.path)?;
        self.mtime = mtime;
        Ok(true)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn signed_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}
