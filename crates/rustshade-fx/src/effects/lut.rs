use std::path::Path;

/// 3D color LUT loaded from a `.cube` file, applied with trilinear
/// interpolation. Stored display-referred (like the game feeds it to us).
#[derive(Debug, Clone)]
pub struct Lut3D {
    n: u32,
    data: Vec<[f32; 3]>,
}

#[derive(Debug, thiserror::Error)]
pub enum LutError {
    #[error("cannot read LUT {0}: {1}")]
    Io(String, #[source] std::io::Error),
    #[error("LUT {0}: {1}")]
    Parse(String, String),
}

impl Lut3D {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LutError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| LutError::Io(path.display().to_string(), e))?;
        Self::parse(&text).map_err(|e| LutError::Parse(path.display().to_string(), e))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut n = None;
        let mut data: Vec<[f32; 3]> = Vec::new();

        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("TITLE") {
                continue;
            }
            if let Some(rest) = line.strip_prefix("LUT_3D_SIZE") {
                let size: u32 = rest
                    .trim()
                    .parse()
                    .map_err(|_| format!("line {}: bad LUT_3D_SIZE", line_no + 1))?;
                if !(2..=256).contains(&size) {
                    return Err(format!(
                        "line {}: LUT_3D_SIZE {size} out of range 2..=256",
                        line_no + 1
                    ));
                }
                n = Some(size);
                continue;
            }
            if line.starts_with("LUT_1D_SIZE")
                || line.starts_with("DOMAIN_MIN")
                || line.starts_with("DOMAIN_MAX")
                || line.starts_with("LUT_3D_INPUT_ALPHA")
            {
                return Err(format!(
                    "line {}: unsupported directive '{line}'",
                    line_no + 1
                ));
            }
            let mut vals = [0.0f32; 3];
            for (i, tok) in line.split_whitespace().enumerate() {
                if i >= 3 {
                    return Err(format!("line {}: too many columns", line_no + 1));
                }
                vals[i] = tok
                    .parse::<f32>()
                    .map_err(|_| format!("line {}: '{tok}' is not a number", line_no + 1))?;
            }
            data.push(vals);
        }

        let n = n.ok_or("missing LUT_3D_SIZE directive")?;
        let expected = (n * n * n) as usize;
        if data.len() != expected {
            return Err(format!("expected {expected} entries, found {}", data.len()));
        }
        Ok(Self { n, data })
    }

    /// Applies the LUT to an RGB color in [0, 1]^3 with trilinear sampling.
    pub fn apply(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let x = r.clamp(0.0, 1.0) * (self.n - 1) as f32;
        let y = g.clamp(0.0, 1.0) * (self.n - 1) as f32;
        let z = b.clamp(0.0, 1.0) * (self.n - 1) as f32;

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let z0 = z.floor() as u32;
        let x1 = (x0 + 1).min(self.n - 1);
        let y1 = (y0 + 1).min(self.n - 1);
        let z1 = (z0 + 1).min(self.n - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let fz = z - z0 as f32;

        let c000 = self.at(x0, y0, z0);
        let c100 = self.at(x1, y0, z0);
        let c010 = self.at(x0, y1, z0);
        let c110 = self.at(x1, y1, z0);
        let c001 = self.at(x0, y0, z1);
        let c101 = self.at(x1, y0, z1);
        let c011 = self.at(x0, y1, z1);
        let c111 = self.at(x1, y1, z1);

        let mix_axis = |a: [f32; 3], b: [f32; 3], t: f32| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };

        let c00 = mix_axis(c000, c100, fx);
        let c10 = mix_axis(c010, c110, fx);
        let c01 = mix_axis(c001, c101, fx);
        let c11 = mix_axis(c011, c111, fx);
        let c0 = mix_axis(c00, c10, fy);
        let c1 = mix_axis(c01, c11, fy);
        mix_axis(c0, c1, fz)
    }

    #[inline]
    fn at(&self, x: u32, y: u32, z: u32) -> [f32; 3] {
        self.data[(x + y * self.n + z * self.n * self.n) as usize]
    }

    pub fn size(&self) -> u32 {
        self.n
    }

    pub fn raw_data(&self) -> &[[f32; 3]] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_identity_lut() {
        let mut text = String::from("TITLE \"identity\"\nLUT_3D_SIZE 2\n");
        for b in 0..2u32 {
            for g in 0..2u32 {
                for r in 0..2u32 {
                    text.push_str(&format!("{} {} {}\n", r as f32, g as f32, b as f32));
                }
            }
        }
        let lut = Lut3D::parse(&text).unwrap();
        assert_eq!(lut.size(), 2);
        let out = lut.apply(0.5, 0.5, 0.5);
        for v in out {
            assert!((v - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn rejects_bad_size() {
        assert!(Lut3D::parse("LUT_3D_SIZE 1\n0 0 0\n").is_err());
    }

    #[test]
    fn rejects_missing_data() {
        assert!(Lut3D::parse("LUT_3D_SIZE 4\n").is_err());
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let text = "# comment\n\nLUT_3D_SIZE 2\n# another\n1 1 1 # inline\n2 2 2\n3 3 3\n4 4 4\n5 5 5\n6 6 6\n7 7 7\n8 8 8\n";
        let lut = Lut3D::parse(text).unwrap();
        assert_eq!(lut.size(), 2);
    }
}

/// A LUT-backed effect that keeps its parsed table.
#[derive(Debug, Clone)]
pub struct LutEffect {
    pub lut: Lut3D,
    pub opacity: f32,
}

impl crate::pipeline::Effect for LutEffect {
    fn id(&self) -> &'static str {
        "lut"
    }

    fn apply(&mut self, frame: &mut crate::buffer::Frame, _ctx: &crate::pipeline::Ctx) {
        let op = self.opacity.clamp(0.0, 1.0);
        if op <= 0.0 {
            return;
        }
        for px in frame.data.as_chunks_mut::<4>().0 {
            let mapped = self.lut.apply(px[0], px[1], px[2]);
            px[0] += (mapped[0] - px[0]) * op;
            px[1] += (mapped[1] - px[1]) * op;
            px[2] += (mapped[2] - px[2]) * op;
        }
    }
}
