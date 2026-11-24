use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct Fxaa {
    pub subpixel: f32,
    pub edge_threshold: f32,
}

impl Fxaa {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            subpixel: get_f32_or_default(spec, "subpixel", 0.75),
            edge_threshold: get_f32_or_default(spec, "edge_threshold", 0.125),
        }
    }
}

fn luma(px: &[f32]) -> f32 {
    0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2]
}

impl Effect for Fxaa {
    fn id(&self) -> &'static str {
        "fxaa"
    }

    fn apply(&mut self, frame: &mut Frame, _ctx: &Ctx) {
        let w = frame.width as i64;
        let h = frame.height as i64;
        let mut out = frame.data.clone();

        for y in 0..h {
            for x in 0..w {
                let nw = luma(&frame.pixel(x - 1, y - 1));
                let n = luma(&frame.pixel(x, y - 1));
                let ne = luma(&frame.pixel(x + 1, y - 1));
                let ww = luma(&frame.pixel(x - 1, y));
                let me = luma(&frame.pixel(x, y));
                let ee = luma(&frame.pixel(x + 1, y));
                let sw = luma(&frame.pixel(x - 1, y + 1));
                let ss = luma(&frame.pixel(x, y + 1));
                let se = luma(&frame.pixel(x + 1, y + 1));

                let lmin = nw
                    .min(n)
                    .min(ne)
                    .min(ww)
                    .min(me)
                    .min(ee)
                    .min(sw)
                    .min(ss)
                    .min(se);
                let lmax = nw
                    .max(n)
                    .max(ne)
                    .max(ww)
                    .max(me)
                    .max(ee)
                    .max(sw)
                    .max(ss)
                    .max(se);
                let range = lmax - lmin;
                if range < self.edge_threshold {
                    continue;
                }

                let grad_h = (ww - ee).abs();
                let grad_v = (n - ss).abs();
                let idx = (y as usize * w as usize + x as usize) * 4;

                let sample = if grad_h >= grad_v {
                    let a = frame.pixel(x - 1, y);
                    let b = frame.pixel(x + 1, y);
                    mixed(&a, &b)
                } else {
                    let a = frame.pixel(x, y - 1);
                    let b = frame.pixel(x, y + 1);
                    mixed(&a, &b)
                };

                for c in 0..3 {
                    out[idx + c] = me + (sample[c] - me) * self.subpixel;
                }
            }
        }
        frame.data = out;
    }
}

fn mixed(a: &[f32; 4], b: &[f32; 4]) -> [f32; 4] {
    [
        (a[0] + b[0]) * 0.5,
        (a[1] + b[1]) * 0.5,
        (a[2] + b[2]) * 0.5,
        (a[3] + b[3]) * 0.5,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Frame;

    #[test]
    fn flat_area_unchanged() {
        let img = image::RgbaImage::from_pixel(16, 16, image::Rgba([100, 110, 120, 255]));
        let mut frame = Frame::from_rgba8(&img);
        let mut f = Fxaa {
            subpixel: 1.0,
            edge_threshold: 0.125,
        };
        f.apply(
            &mut frame,
            &Ctx {
                width: 16,
                height: 16,
                frame_index: 0,
            },
        );
        for px in frame.data.chunks_exact(4) {
            assert!((px[0] - 100.0 / 255.0).abs() < 1e-6);
        }
    }

    #[test]
    fn hard_edge_is_softened() {
        let img = image::RgbaImage::from_fn(32, 32, |x, _y| {
            if x < 16 {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        });
        let mut frame = Frame::from_rgba8(&img);
        let before = frame.pixel(15, 15)[0];
        let mut f = Fxaa {
            subpixel: 1.0,
            edge_threshold: 0.05,
        };
        f.apply(
            &mut frame,
            &Ctx {
                width: 32,
                height: 32,
                frame_index: 0,
            },
        );
        let after = frame.pixel(15, 15)[0];
        assert!(
            after > before + 0.1,
            "edge pixel should blend up: {before} -> {after}"
        );
    }
}
