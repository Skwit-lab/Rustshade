//! CPU reference implementations of the rustshade effect stack.
//!
//! The GPU backend mirrors these semantics in WGSL; this crate is the
//! authoritative, testable reference and also the fallback when no GPU
//! adapter is available.

pub mod buffer;
pub mod effects;
pub mod pipeline;

pub use buffer::{linear_to_srgb, srgb_to_linear, Frame};
pub use effects::lut::{Lut3D, LutEffect};
pub use pipeline::{build_pipeline, Ctx, Effect, PipelineError};

use std::path::Path;

/// Applies a preset's built effect stack to an RGBA8 (sRGB) image.
pub fn apply_stack(
    stack: &mut [Box<dyn Effect>],
    img: &image::RgbaImage,
    frame_index: u64,
) -> image::RgbaImage {
    let mut frame = Frame::from_rgba8(img);
    let ctx = Ctx {
        width: frame.width,
        height: frame.height,
        frame_index,
    };
    for effect in stack.iter_mut() {
        effect.apply(&mut frame, &ctx);
    }
    frame.to_rgba8()
}

/// Loads a LUT file (.cube).
pub fn load_lut(path: &Path) -> Result<Lut3D, effects::lut::LutError> {
    Lut3D::load(path)
}
