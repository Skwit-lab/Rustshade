/// Declared kinds of effect parameters. All parameters are stored as floats
/// in uniforms; strings and bools special-case themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    F32,
    Bool,
    String,
}

#[derive(Debug, Clone, Copy)]
pub struct ParamDef {
    pub name: &'static str,
    pub kind: ParamKind,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub doc: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct EffectDef {
    pub id: &'static str,
    pub doc: &'static str,
    pub params: &'static [ParamDef],
}

const fn f32p(name: &'static str, default: f32, min: f32, max: f32, doc: &'static str) -> ParamDef {
    ParamDef {
        name,
        kind: ParamKind::F32,
        default,
        min,
        max,
        doc,
    }
}

const fn strp(name: &'static str, doc: &'static str) -> ParamDef {
    ParamDef {
        name,
        kind: ParamKind::String,
        default: 0.0,
        min: 0.0,
        max: 0.0,
        doc,
    }
}

pub const GRADE: EffectDef = EffectDef {
    id: "grade",
    doc: "Exposure, contrast, saturation, temperature/tint, gamma and brightness.",
    params: &[
        f32p(
            "exposure",
            0.0,
            -3.0,
            3.0,
            "Exposure in stops (2^x multiplier).",
        ),
        f32p(
            "contrast",
            0.0,
            -1.0,
            1.0,
            "Contrast around mid-grey, -1 to 1.",
        ),
        f32p(
            "saturation",
            1.0,
            0.0,
            3.0,
            "Saturation multiplier, 1 is neutral.",
        ),
        f32p("temperature", 0.0, -1.0, 1.0, "-1 cool, +1 warm."),
        f32p("tint", 0.0, -1.0, 1.0, "-1 green, +1 magenta."),
        f32p("gamma", 1.0, 0.2, 5.0, "Gamma exponent, 1 is neutral."),
        f32p("brightness", 0.0, -1.0, 1.0, "Additive brightness."),
    ],
};

pub const BLOOM: EffectDef = EffectDef {
    id: "bloom",
    doc: "Soft glow of bright areas (multi-pass on the GPU backend).",
    params: &[
        f32p("strength", 0.5, 0.0, 2.0, "Glow intensity."),
        f32p("threshold", 0.75, 0.0, 1.0, "Luminance where bloom starts."),
        f32p(
            "knee",
            0.3,
            0.0,
            1.0,
            "Soft knee width below the threshold.",
        ),
        f32p("radius", 4.0, 1.0, 16.0, "Blur radius in half-res pixels."),
    ],
};

pub const SHARPEN: EffectDef = EffectDef {
    id: "sharpen",
    doc: "Unsharp-mask sharpening.",
    params: &[
        f32p("amount", 0.5, 0.0, 3.0, "Sharpening strength."),
        f32p("radius", 1.0, 0.5, 2.0, "Blur radius in pixels."),
        f32p(
            "threshold",
            0.0,
            0.0,
            0.2,
            "Skip differences below this to avoid noise amplification.",
        ),
    ],
};

pub const FXAA: EffectDef = EffectDef {
    id: "fxaa",
    doc: "Fast approximate anti-aliasing on the (already rendered) frame.",
    params: &[
        f32p(
            "subpixel",
            0.75,
            0.0,
            1.0,
            "Amount of sub-pixel aliasing removal.",
        ),
        f32p(
            "edge_threshold",
            0.125,
            0.01,
            1.0,
            "Minimum luma delta treated as an edge.",
        ),
    ],
};

pub const VIGNETTE: EffectDef = EffectDef {
    id: "vignette",
    doc: "Darkens the frame towards the corners.",
    params: &[
        f32p("amount", 0.5, 0.0, 2.0, "Corner darkening strength."),
        f32p(
            "inner",
            0.35,
            0.0,
            1.0,
            "Relative radius where the falloff starts.",
        ),
        f32p("softness", 0.6, 0.01, 1.0, "Falloff softness."),
    ],
};

pub const CHROMATIC_ABERRATION: EffectDef = EffectDef {
    id: "chromatic_aberration",
    doc: "Radially shifts red/blue channels away from the center.",
    params: &[f32p(
        "strength",
        0.2,
        0.0,
        1.0,
        "Maximum corner offset, in fraction of a 32px ring.",
    )],
};

pub const GRAIN: EffectDef = EffectDef {
    id: "grain",
    doc: "Deterministic film grain.",
    params: &[
        f32p("strength", 0.1, 0.0, 1.0, "Noise intensity."),
        f32p("seed", 0.0, 0.0, 100000.0, "Noise seed."),
        f32p(
            "animate",
            1.0,
            0.0,
            1.0,
            "How strongly the pattern moves per frame.",
        ),
    ],
};

pub const SCANLINES: EffectDef = EffectDef {
    id: "scanlines",
    doc: "CRT-style horizontal scanlines.",
    params: &[
        f32p("amount", 0.2, 0.0, 1.0, "Line darkness."),
        f32p("period", 3.0, 2.0, 12.0, "Line spacing in pixels."),
        f32p("phase", 0.0, 0.0, 1.0, "Vertical phase offset 0..1."),
    ],
};

pub const LUT: EffectDef = EffectDef {
    id: "lut",
    doc: "3D color LUT (.cube file, trilinear interpolation).",
    params: &[
        strp("path", "Path to a .cube LUT file."),
        f32p(
            "opacity",
            1.0,
            0.0,
            1.0,
            "Blend between original and LUT output.",
        ),
    ],
};

pub static REGISTRY: &[EffectDef] = &[
    GRADE,
    BLOOM,
    SHARPEN,
    FXAA,
    VIGNETTE,
    CHROMATIC_ABERRATION,
    GRAIN,
    SCANLINES,
    LUT,
];

pub fn effect_def(id: &str) -> Option<&'static EffectDef> {
    REGISTRY.iter().find(|e| e.id == id)
}
