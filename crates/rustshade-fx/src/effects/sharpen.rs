use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct Sharpen {
    pub amount: f32,
    pub radius: f32,
    pub threshold: f32,
}

impl Sharpen {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            amount: get_f32_or_default(spec, "amount", 0.5),
            radius: get_f32_or_default(spec, "radius", 1.0),
            threshold: get_f32_or_default(spec, "threshold", 0.0),
        }
    }
}

fn luma(px: &[f32]) -> f32 {
    0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2]
}

impl Effect for Sharpen {
    fn id(&self) -> &'static str {
        "sharpen"
    }

    fn apply(&mut self, frame: &mut Frame, _ctx: &Ctx) {
        let sigma = (self.radius.max(0.25)) * 0.5;
        let taps = ((sigma * 2.0).ceil() as i64).max(1);

        let mut kernel = Vec::with_capacity((taps * 2 + 1) as usize);
        let mut sum = 0.0;
        for i in -taps..=taps {
            let x = i as f32;
            let v = (-x * x / (2.0 * sigma * sigma)).exp();
            kernel.push(v);
            sum += v;
        }
        for v in kernel.iter_mut() {
            *v /= sum;
        }

        let original = frame.data.clone();
        let mut blur = frame.clone();
        gaussian_axis(frame, &mut blur, &kernel, true);
        gaussian_axis(&blur, frame, &kernel, false);

        let diffs: Vec<f32> = frame
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(original.as_chunks::<4>().0)
            .map(|(blurred, orig)| {
                let mut d = luma(orig) - luma(blurred);
                if d.abs() < self.threshold {
                    d = 0.0;
                }
                d
            })
            .collect();

        for (i, dst) in frame.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let add = diffs[i] * self.amount;
            dst[0] = (dst[0] + add).clamp(0.0, 1.0);
            dst[1] = (dst[1] + add).clamp(0.0, 1.0);
            dst[2] = (dst[2] + add).clamp(0.0, 1.0);
        }
    }
}

fn gaussian_axis(src: &Frame, dst: &mut Frame, kernel: &[f32], horizontal: bool) {
    let w = src.width as usize;
    let h = src.height as usize;
    let mid = (kernel.len() / 2) as i64;
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let mut acc = [0.0f32; 4];
            for (k, kv) in kernel.iter().enumerate() {
                let off = k as i64 - mid;
                let p = if horizontal {
                    src.pixel(x + off, y)
                } else {
                    src.pixel(x, y + off)
                };
                acc[0] += p[0] * kv;
                acc[1] += p[1] * kv;
                acc[2] += p[2] * kv;
                acc[3] += p[3] * kv;
            }
            let idx = (y as usize * w + x as usize) * 4;
            dst.data[idx] = acc[0];
            dst.data[idx + 1] = acc[1];
            dst.data[idx + 2] = acc[2];
            dst.data[idx + 3] = acc[3];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Frame;

    #[test]
    fn increases_edge_variance() {
        let img = image::RgbaImage::from_fn(64, 64, |_x, y| {
            let v = (y as f32 * 4.0).sin() * 30.0 + 128.0;
            image::Rgba([v as u8, v as u8, v as u8, 255])
        });
        let var_before = variance(&Frame::from_rgba8(&img));
        let mut frame = Frame::from_rgba8(&img);
        let mut s = Sharpen {
            amount: 1.5,
            radius: 1.0,
            threshold: 0.0,
        };
        s.apply(
            &mut frame,
            &Ctx {
                width: 64,
                height: 64,
                frame_index: 0,
            },
        );
        let var_after = variance(&frame);
        assert!(
            var_after > var_before * 1.2,
            "variance {} -> {}",
            var_before,
            var_after
        );
    }

    fn variance(frame: &Frame) -> f32 {
        let mean: f32 = frame.data.chunks_exact(4).map(|p| p[0]).sum::<f32>()
            / (frame.width * frame.height) as f32;
        frame
            .data
            .chunks_exact(4)
            .map(|p| (p[0] - mean).powi(2))
            .sum::<f32>()
            / (frame.width * frame.height) as f32
    }

    #[test]
    fn flat_image_is_unchanged() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_pixel(
            16,
            16,
            image::Rgba([90, 90, 90, 255]),
        ));
        let mut s = Sharpen {
            amount: 2.0,
            radius: 1.5,
            threshold: 0.0,
        };
        s.apply(
            &mut frame,
            &Ctx {
                width: 16,
                height: 16,
                frame_index: 0,
            },
        );
        for px in frame.data.chunks_exact(4) {
            assert!((px[0] - 90.0 / 255.0).abs() < 1e-4);
        }
    }
}
