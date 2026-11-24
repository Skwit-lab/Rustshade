use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct Scanlines {
    pub amount: f32,
    pub period: f32,
    pub phase: f32,
}

impl Scanlines {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            amount: get_f32_or_default(spec, "amount", 0.2),
            period: get_f32_or_default(spec, "period", 3.0),
            phase: get_f32_or_default(spec, "phase", 0.0),
        }
    }
}

impl Effect for Scanlines {
    fn id(&self) -> &'static str {
        "scanlines"
    }

    fn apply(&mut self, frame: &mut Frame, _ctx: &Ctx) {
        if self.amount <= 0.0 {
            return;
        }
        let period = self.period.max(2.0);
        let base = 1.0 - self.amount * 0.5;
        let amp = self.amount * 0.5;
        for y in 0..frame.height {
            let t = (y as f32 + self.phase * period) * std::f32::consts::TAU / period;
            let factor = base + amp * t.cos();
            for x in 0..frame.width {
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
    fn creates_horizontal_variation() {
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 200, 200, 255]));
        let mut frame = Frame::from_rgba8(&img);
        let mut s = Scanlines {
            amount: 0.5,
            period: 2.0,
            phase: 0.0,
        };
        s.apply(
            &mut frame,
            &Ctx {
                width: 8,
                height: 8,
                frame_index: 0,
            },
        );
        let row0 = frame.data[0];
        let row1 = frame.data[(8 * 4) as usize];
        assert!(
            row0 > row1,
            "row0 {row0} should be brighter than row1 {row1}"
        );
    }

    #[test]
    fn preserves_constant_rows() {
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 200, 200, 255]));
        let mut frame = Frame::from_rgba8(&img);
        let mut s = Scanlines {
            amount: 0.5,
            period: 2.0,
            phase: 0.0,
        };
        s.apply(
            &mut frame,
            &Ctx {
                width: 8,
                height: 8,
                frame_index: 0,
            },
        );
        // all pixels in row 3 identical
        let v = frame.data[(3 * 8 * 4) as usize];
        for x in 0..8usize {
            assert!((frame.data[(3 * 8 + x) * 4] - v).abs() < 1e-6);
        }
    }
}
