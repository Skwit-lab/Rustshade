/// RGBA frame in f32, interleaved, values nominally in [0, 1].
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>,
}

#[inline]
fn u8_to_f32(v: u8) -> f32 {
    (v as f32) / 255.0
}

#[inline]
fn f32_to_u8(v: f32) -> u8 {
    let v = (v * 255.0 + 0.5).floor();
    v.clamp(0.0, 255.0) as u8
}

const SRGB_ALPHA: f32 = 0.055;

/// sRGB (display) to linear light conversion for values in [0, 1].
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + SRGB_ALPHA) / (1.0 + SRGB_ALPHA)).powf(2.4)
    }
}

/// Linear light to sRGB (display) conversion for values in [0, 1].
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        (1.0 + SRGB_ALPHA) * c.powf(1.0 / 2.4) - SRGB_ALPHA
    }
}

impl Frame {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; (width as usize) * (height as usize) * 4],
        }
    }

    pub fn from_rgba8(img: &image::RgbaImage) -> Self {
        let width = img.width();
        let height = img.height();
        let mut data = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for px in img.pixels() {
            data.push(u8_to_f32(px.0[0]));
            data.push(u8_to_f32(px.0[1]));
            data.push(u8_to_f32(px.0[2]));
            data.push(u8_to_f32(px.0[3]));
        }
        Self {
            width,
            height,
            data,
        }
    }

    /// Converts the RGB channels to linear light; alpha is untouched.
    pub fn to_linear_in_place(&mut self) {
        for px in self.data.as_chunks_mut::<4>().0 {
            px[0] = srgb_to_linear(px[0]);
            px[1] = srgb_to_linear(px[1]);
            px[2] = srgb_to_linear(px[2]);
        }
    }

    /// Converts the RGB channels back to sRGB display values.
    pub fn to_srgb_in_place(&mut self) {
        for px in self.data.as_chunks_mut::<4>().0 {
            px[0] = linear_to_srgb(px[0]);
            px[1] = linear_to_srgb(px[1]);
            px[2] = linear_to_srgb(px[2]);
        }
    }

    /// Expects sRGB input (scene-referred conversions happen in the caller).
    pub fn to_rgba8(&self) -> image::RgbaImage {
        let mut img = image::RgbaImage::new(self.width, self.height);
        for (dst, src) in img.pixels_mut().zip(self.data.as_chunks::<4>().0) {
            *dst = image::Rgba([
                f32_to_u8(src[0]),
                f32_to_u8(src[1]),
                f32_to_u8(src[2]),
                f32_to_u8(src[3]),
            ]);
        }
        img
    }

    /// Bilinear RGBA sample at texture coordinates u, v in [0, 1].
    pub fn sample_bilinear(&self, u: f32, v: f32) -> [f32; 4] {
        let x = u * self.width as f32 - 0.5;
        let y = v * self.height as f32 - 0.5;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let a = self.pixel(x0 as i64, y0 as i64);
        let b = self.pixel(x0 as i64 + 1, y0 as i64);
        let c = self.pixel(x0 as i64, y0 as i64 + 1);
        let d = self.pixel(x0 as i64 + 1, y0 as i64 + 1);
        let mut out = [0.0f32; 4];
        for i in 0..4 {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bot = c[i] + (d[i] - c[i]) * fx;
            out[i] = top + (bot - top) * fy;
        }
        out
    }

    #[inline]
    pub fn pixel(&self, x: i64, y: i64) -> [f32; 4] {
        let cx = x.clamp(0, self.width as i64 - 1) as usize;
        let cy = y.clamp(0, self.height as i64 - 1) as usize;
        let idx = (cy * self.width as usize + cx) * 4;
        [
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_roundtrip_identity() {
        for i in 0..=255u32 {
            let v = (i as f32) / 255.0;
            assert!((linear_to_srgb(srgb_to_linear(v)) - v).abs() < 1e-4);
        }
    }

    #[test]
    fn u8_roundtrip_identity() {
        let img =
            image::RgbaImage::from_fn(4, 2, |x, y| image::Rgba([(x * 60 + y) as u8, 7, 200, 255]));
        let back = Frame::from_rgba8(&img).to_rgba8();
        assert_eq!(back, img);
    }

    #[test]
    fn pixel_clamps_out_of_bounds() {
        let mut frame = Frame::new(2, 2);
        frame.data[0] = 0.25;
        assert_eq!(frame.pixel(-5, -5)[0], 0.25);
        assert_eq!(frame.pixel(10, 10)[0], 0.0);
    }
}
