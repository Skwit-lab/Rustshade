//! wgpu-based GPU backend. Applies the same preset format as the CPU
//! reference implementation, using one fullscreen pass per effect
//! (bloom is a 4-pass chain).

mod renderer;

pub use renderer::{apply_to_image, GpuError, GpuRenderer};
