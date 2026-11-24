use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct Grain {
    pub strength: f32,
    pub seed: f32,
    pub animate: f32,
}

impl Grain {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            strength: get_f32_or_default(spec, "strength", 0.1),
            seed: get_f32_or_default(spec, "seed", 0.0),
            animate: get_f32_or_default(spec, "animate", 1.0),
        }
    }
}

fn hash3(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a ^ b.wrapping_mul(0x9E3779B9) ^ c.wrapping_mul(0x85EBCA6B);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846CA68B);
    h ^= h >> 16;
    h
}

fn noise(a: u32, b: u32, c: u32) -> f32 {
    (hash3(a, b, c) & 0xFFFFFF) as f32 / 0xFFFFFF as f32 * 2.0 - 1.0
}

impl Effect for Grain {
    fn id(&self) -> &'static str {
        "grain"
    }

    fn apply(&mut self, frame: &mut Frame, ctx: &Ctx) {
        if self.strength <= 0.0 {
            return;
        }
        let step = if self.animate > 0.0 {
            (ctx.frame_index as f32 * self.animate) as u32
        } else {
            0
        };
        let seed = self.seed as u32;
        for y in 0..frame.height {
            for x in 0..frame.width {
                let n = noise(x, y, seed ^ step);
                let idx = (y as usize * frame.width as usize + x as usize) * 4;
                let luma = 0.2126 * frame.data[idx]
                    + 0.7152 * frame.data[idx + 1]
                    + 0.0722 * frame.data[idx + 2];
                let mid_weight = 4.0 * luma * (1.0 - luma);
                let weight = self.strength * (0.25 + 0.75 * mid_weight);
                for c in 0..3 {
                    frame.data[idx + c] = (frame.data[idx + c] + n * weight).clamp(0.0, 1.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Frame;

    #[test]
    fn deterministic_for_same_frame() {
        let img = image::RgbaImage::from_fn(8, 8, |x, y| {
            image::Rgba([(x * 10 + y * 5) as u8, 64, 128, 255])
        });
        let mut a = Frame::from_rgba8(&img);
        let mut b = Frame::from_rgba8(&img);
        let mut g = Grain {
            strength: 0.3,
            seed: 1.0,
            animate: 1.0,
        };
        let ctx = Ctx {
            width: 8,
            height: 8,
            frame_index: 5,
        };
        g.apply(&mut a, &ctx);
        g.apply(&mut b, &ctx);
        assert_eq!(a.data, b.data);
    }

    #[test]
    fn animates_between_frames() {
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([128, 128, 128, 255]));
        let mut a = Frame::from_rgba8(&img);
        let mut g = Grain {
            strength: 0.3,
            seed: 1.0,
            animate: 1.0,
        };
        g.apply(
            &mut a,
            &Ctx {
                width: 8,
                height: 8,
                frame_index: 1,
            },
        );
        let f1 = a.data.clone();
        g.apply(
            &mut a,
            &Ctx {
                width: 8,
                height: 8,
                frame_index: 2,
            },
        );
        assert_ne!(f1, a.data);
    }

    #[test]
    fn preserves_midgrey_mean() {
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([128, 128, 128, 255]));
        let mut frame = Frame::from_rgba8(&img);
        let mean_before: f32 =
            frame.data.chunks_exact(4).map(|p| p[0]).sum::<f32>() / (64.0 * 64.0);
        let mut g = Grain {
            strength: 0.5,
            seed: 7.0,
            animate: 0.0,
        };
        g.apply(
            &mut frame,
            &Ctx {
                width: 64,
                height: 64,
                frame_index: 0,
            },
        );
        let mean_after: f32 = frame.data.chunks_exact(4).map(|p| p[0]).sum::<f32>() / (64.0 * 64.0);
        assert!((mean_before - mean_after).abs() < 0.01);
    }
}
