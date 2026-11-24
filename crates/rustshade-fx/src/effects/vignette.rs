use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct Vignette {
    pub amount: f32,
    pub inner: f32,
    pub softness: f32,
}

impl Vignette {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            amount: get_f32_or_default(spec, "amount", 0.5),
            inner: get_f32_or_default(spec, "inner", 0.35),
            softness: get_f32_or_default(spec, "softness", 0.6),
        }
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Effect for Vignette {
    fn id(&self) -> &'static str {
        "vignette"
    }

    fn apply(&mut self, frame: &mut Frame, _ctx: &Ctx) {
        let w = frame.width as f32;
        let h = frame.height as f32;
        let cx = w * 0.5;
        let cy = h * 0.5;
        let norm = (0.5f32 * 0.5 + 0.5 * 0.5).sqrt(); // 0.7071, corner distance
        for y in 0..frame.height {
            for x in 0..frame.width {
                let dx = (x as f32 + 0.5 - cx) / w;
                let dy = (y as f32 + 0.5 - cy) / h;
                let d = (dx * dx + dy * dy).sqrt() / norm;
                let t = smoothstep(self.inner, self.inner + self.softness, d);
                let factor = 1.0 - self.amount * t;
                let idx = (y as usize * frame.width as usize + x as usize) * 4;
                frame.data[idx] *= factor;
                frame.data[idx + 1] *= factor;
                frame.data[idx + 2] *= factor;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Frame;

    #[test]
    fn darkens_corners_not_center() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_pixel(
            64,
            64,
            image::Rgba([200, 200, 200, 255]),
        ));
        let mut v = Vignette {
            amount: 0.8,
            inner: 0.0,
            softness: 0.7,
        };
        v.apply(
            &mut frame,
            &Ctx {
                width: 64,
                height: 64,
                frame_index: 0,
            },
        );
        let w = 64usize;
        let center = frame.data[((w * 32 + 32) * 4) as usize];
        let corner = frame.data[0];
        assert!(center > 0.75, "center should stay bright, got {center}");
        assert!(corner < 0.55, "corner should darken, got {corner}");
    }

    #[test]
    fn zero_amount_is_identity() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([123, 45, 200, 255]),
        ));
        let before = frame.data.clone();
        let mut v = Vignette {
            amount: 0.0,
            inner: 0.0,
            softness: 0.5,
        };
        v.apply(
            &mut frame,
            &Ctx {
                width: 8,
                height: 8,
                frame_index: 0,
            },
        );
        assert_eq!(frame.data, before);
    }
}
