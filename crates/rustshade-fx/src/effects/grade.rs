use super::get_f32_or_default;
use crate::buffer::Frame;
use crate::pipeline::{Ctx, Effect};
use rustshade_core::EffectSpec;

#[derive(Debug, Clone)]
pub struct Grade {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    pub gamma: f32,
    pub brightness: f32,
}

impl Grade {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        Self {
            exposure: get_f32_or_default(spec, "exposure", 0.0),
            contrast: get_f32_or_default(spec, "contrast", 0.0),
            saturation: get_f32_or_default(spec, "saturation", 1.0),
            temperature: get_f32_or_default(spec, "temperature", 0.0),
            tint: get_f32_or_default(spec, "tint", 0.0),
            gamma: get_f32_or_default(spec, "gamma", 1.0),
            brightness: get_f32_or_default(spec, "brightness", 0.0),
        }
    }
}

const CONTRAST_PIVOT: f32 = 0.5;

impl Effect for Grade {
    fn id(&self) -> &'static str {
        "grade"
    }

    fn apply(&mut self, frame: &mut Frame, _ctx: &Ctx) {
        let exp_mult = f32::powf(2.0, self.exposure);
        let contrast_amt = 1.0 + self.contrast;
        let warm_r = 1.0 + self.temperature * 0.2;
        let warm_b = 1.0 - self.temperature * 0.2;
        let tint_r = 1.0 + self.tint * 0.1;
        let tint_g = 1.0 - self.tint * 0.1;
        let inv_gamma = 1.0 / self.gamma.max(1e-4);

        for px in frame.data.as_chunks_mut::<4>().0 {
            let mut r = px[0] * exp_mult;
            let mut g = px[1] * exp_mult;
            let mut b = px[2] * exp_mult;

            r = (r - CONTRAST_PIVOT) * contrast_amt + CONTRAST_PIVOT;
            g = (g - CONTRAST_PIVOT) * contrast_amt + CONTRAST_PIVOT;
            b = (b - CONTRAST_PIVOT) * contrast_amt + CONTRAST_PIVOT;

            r *= warm_r * tint_r;
            g *= tint_g;
            b *= warm_b * tint_g;

            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            r = luma + (r - luma) * self.saturation;
            g = luma + (g - luma) * self.saturation;
            b = luma + (b - luma) * self.saturation;

            r = f32::powf(r.max(0.0), inv_gamma);
            g = f32::powf(g.max(0.0), inv_gamma);
            b = f32::powf(b.max(0.0), inv_gamma);

            px[0] = (r + self.brightness).clamp(0.0, 1.0);
            px[1] = (g + self.brightness).clamp(0.0, 1.0);
            px[2] = (b + self.brightness).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Frame;
    use crate::pipeline::Effect;

    fn ctx(w: u32, h: u32) -> Ctx {
        Ctx {
            width: w,
            height: h,
            frame_index: 0,
        }
    }

    #[test]
    fn neutral_grade_is_identity() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_fn(4, 4, |x, y| {
            image::Rgba([((x * 40 + y * 10) % 256) as u8, 128, 255, 255])
        }));
        let before = frame.data.clone();
        let mut grade = Grade::from_spec(&preset_spec());
        grade.apply(&mut frame, &ctx(4, 4));
        for (a, b) in frame.data.iter().zip(before.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    fn preset_spec() -> rustshade_core::EffectSpec {
        let mut table = toml::Table::new();
        rustshade_core::EffectSpec {
            id: "grade".into(),
            enabled: true,
            params: table,
        }
    }

    #[test]
    fn exposure_brightens() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([100, 100, 100, 255]),
        ));
        let mut grade = Grade {
            exposure: 1.0,
            contrast: 0.0,
            saturation: 1.0,
            temperature: 0.0,
            tint: 0.0,
            gamma: 1.0,
            brightness: 0.0,
        };
        grade.apply(&mut frame, &ctx(2, 2));
        // 100/255 * 2 = 0.784.. -> ~200
        assert!(frame.data[0] > 0.75 && frame.data[0] < 0.82);
    }

    #[test]
    fn brightness_clamps() {
        let mut frame = Frame::from_rgba8(&image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([255, 0, 128, 255]),
        ));
        let mut grade = Grade {
            exposure: 0.0,
            contrast: 0.0,
            saturation: 1.0,
            temperature: 0.0,
            tint: 0.0,
            gamma: 1.0,
            brightness: 0.5,
        };
        grade.apply(&mut frame, &ctx(2, 2));
        assert_eq!(frame.data[0], 1.0);
        assert_eq!(frame.data[1], 0.5);
    }
}
