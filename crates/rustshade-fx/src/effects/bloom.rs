use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct Bloom {
    pub strength: f32,
    pub threshold: f32,
    pub knee: f32,
    pub radius: f32,
}

impl Bloom {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            strength: get_f32_or_default(spec, "strength", 0.5),
            threshold: get_f32_or_default(spec, "threshold", 0.75),
            knee: get_f32_or_default(spec, "knee", 0.3),
            radius: get_f32_or_default(spec, "radius", 4.0),
        }
    }
}

fn luma(px: &[f32]) -> f32 {
    0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Effect for Bloom {
    fn id(&self) -> &'static str {
        "bloom"
    }

    fn apply(&mut self, frame: &mut Frame, _ctx: &Ctx) {
        if self.strength <= 0.0 {
            return;
        }
        let w = frame.width as usize;
        let h = frame.height as usize;

        let mut bright = vec![0.0f32; w * h * 4];
        for (i, px) in bright.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let src = &frame.data[i * 4..i * 4 + 4];
            let l = luma(src);
            let mask = smoothstep(self.threshold - self.knee, self.threshold + 1e-4, l);
            px[0] = src[0] * mask;
            px[1] = src[1] * mask;
            px[2] = src[2] * mask;
        }

        let hw = (w / 2).max(1);
        let hh = (h / 2).max(1);
        let mut small = downsample(&bright, w, h, hw, hh);

        let sigma = (self.radius * 0.5).max(0.5);
        let taps = ((sigma * 2.5).ceil() as i64).clamp(1, 24);
        let mut kernel = Vec::with_capacity((taps * 2 + 1) as usize);
        let mut sum = 0.0;
        for i in -taps..=taps {
            let v = (-(i as f32) * (i as f32) / (2.0 * sigma * sigma)).exp();
            kernel.push(v);
            sum += v;
        }
        for v in kernel.iter_mut() {
            *v /= sum;
        }

        small = blur_axis(&small, &kernel, true, hw, hh);
        small = blur_axis(&small, &kernel, false, hw, hh);

        for y in 0..frame.height as usize {
            for x in 0..w {
                let u = (x as f32 + 0.5) / w as f32;
                let v = (y as f32 + 0.5) / h as f32;
                let su = (u * hw as f32 - 0.5).clamp(0.0, hw as f32 - 0.001);
                let sv = (v * hh as f32 - 0.5).clamp(0.0, hh as f32 - 0.001);
                let x0 = su.floor() as usize;
                let y0 = sv.floor() as usize;
                let fx = su.fract();
                let fy = sv.fract();
                let x1 = (x0 + 1).min(hw - 1);
                let y1 = (y0 + 1).min(hh - 1);
                let idx = (y * w + x) * 4;
                for c in 0..3 {
                    let top = small[(y0 * hw + x0) * 4 + c] * (1.0 - fx)
                        + small[(y0 * hw + x1) * 4 + c] * fx;
                    let bot = small[(y1 * hw + x0) * 4 + c] * (1.0 - fx)
                        + small[(y1 * hw + x1) * 4 + c] * fx;
                    let glow = (top + (bot - top) * fy) * self.strength;
                    frame.data[idx + c] = (frame.data[idx + c] + glow).clamp(0.0, 1.0);
                }
            }
        }
    }
}

fn downsample(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; dw * dh * 4];
    for y in 0..dh {
        for x in 0..dw {
            let sx0 = (x * 2).min(sw - 1);
            let sy0 = (y * 2).min(sh - 1);
            let sx1 = (x * 2 + 1).min(sw - 1);
            let sy1 = (y * 2 + 1).min(sh - 1);
            for c in 0..3 {
                let a = src[(sy0 * sw + sx0) * 4 + c];
                let b = src[(sy0 * sw + sx1) * 4 + c];
                let e = src[(sy1 * sw + sx0) * 4 + c];
                let f = src[(sy1 * sw + sx1) * 4 + c];
                out[(y * dw + x) * 4 + c] = (a + b + e + f) * 0.25;
            }
        }
    }
    out
}

fn blur_axis(src: &[f32], kernel: &[f32], horizontal: bool, w: usize, h: usize) -> Vec<f32> {
    let mid = (kernel.len() / 2) as i64;
    let mut out = src.to_vec();
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            for (k, kv) in kernel.iter().enumerate() {
                let off = k as i64 - mid;
                let idx = if horizontal {
                    ((y as i64 * w as i64 + (x as i64 + off).clamp(0, w as i64 - 1)) as usize) * 4
                } else {
                    (((y as i64 + off).clamp(0, h as i64 - 1) * w as i64 + x as i64) as usize) * 4
                };
                acc[0] += src[idx] * kv;
                acc[1] += src[idx + 1] * kv;
                acc[2] += src[idx + 2] * kv;
            }
            let idx = (y * w + x) * 4;
            out[idx] = acc[0];
            out[idx + 1] = acc[1];
            out[idx + 2] = acc[2];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Frame;

    #[test]
    fn bright_spot_gains_glow() {
        let img = image::RgbaImage::from_fn(64, 64, |x, y| {
            if (x / 4 == 8 && y / 4 == 8) {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([0, 0, 0, 255])
            }
        });
        let mut frame = Frame::from_rgba8(&img);
        let mut bloom = Bloom {
            strength: 1.0,
            threshold: 0.5,
            knee: 0.0,
            radius: 6.0,
        };
        bloom.apply(
            &mut frame,
            &Ctx {
                width: 64,
                height: 64,
                frame_index: 0,
            },
        );
        // far-away pixel receives glow
        let far = frame.data[((40 * 64 + 40) * 4) as usize];
        assert!(far > 0.001, "glow should reach distant pixels, got {far}");
        // bright spot itself stays clamped at 1
        let spot = frame.data[((32 * 64 + 32) * 4) as usize];
        assert_eq!(spot, 1.0);
    }

    #[test]
    fn dark_frame_is_untouched() {
        let img = image::RgbaImage::from_pixel(32, 32, image::Rgba([10, 10, 10, 255]));
        let mut frame = Frame::from_rgba8(&img);
        let before = frame.data.clone();
        let mut bloom = Bloom {
            strength: 1.0,
            threshold: 0.5,
            knee: 0.0,
            radius: 4.0,
        };
        bloom.apply(
            &mut frame,
            &Ctx {
                width: 32,
                height: 32,
                frame_index: 0,
            },
        );
        assert_eq!(frame.data, before);
    }
}
