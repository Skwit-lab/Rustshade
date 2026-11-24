use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct ChromaticAberration {
    pub strength: f32,
}

impl ChromaticAberration {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            strength: get_f32_or_default(spec, "strength", 0.2),
        }
    }
}

impl Effect for ChromaticAberration {
    fn id(&self) -> &'static str {
        "chromatic_aberration"
    }

    fn apply(&mut self, frame: &mut Frame, _ctx: &Ctx) {
        let w = frame.width as f32;
        let h = frame.height as f32;
        let mut out = frame.data.clone();
        for y in 0..frame.height {
            for x in 0..frame.width {
                let u = (x as f32 + 0.5) / w;
                let v = (y as f32 + 0.5) / h;
                let dx = u - 0.5;
                let dy = v - 0.5;
                let dist2 = dx * dx + dy * dy;
                // Maximum corner offset is strength * 16 pixels.
                let mag = dist2 * self.strength * 32.0;
                let uoff = dx * mag / w;
                let voff = dy * mag / h;

                let idx = (y as usize * frame.width as usize + x as usize) * 4;
                let r = frame.sample_bilinear(u + uoff, v + voff);
                let b = frame.sample_bilinear(u - uoff, v - voff);
                out[idx] = r[0];
                out[idx + 2] = b[2];
            }
        }
        frame.data = out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Frame;

    #[test]
    fn zero_strength_is_identity() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_fn(32, 32, |x, y| {
            image::Rgba([x as u8 * 8, y as u8 * 8, 60, 255])
        }));
        let before = frame.data.clone();
        let mut ca = ChromaticAberration { strength: 0.0 };
        ca.apply(
            &mut frame,
            &Ctx {
                width: 32,
                height: 32,
                frame_index: 0,
            },
        );
        for (a, b) in frame.data.iter().zip(&before) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn center_is_untouched() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_pixel(
            32,
            32,
            image::Rgba([80, 80, 80, 255]),
        ));
        let mut ca = ChromaticAberration { strength: 0.5 };
        ca.apply(
            &mut frame,
            &Ctx {
                width: 32,
                height: 32,
                frame_index: 0,
            },
        );
        let c = frame.data[((16 * 32 + 16) * 4) as usize];
        assert!((c - 80.0 / 255.0).abs() < 1e-6);
    }
}
