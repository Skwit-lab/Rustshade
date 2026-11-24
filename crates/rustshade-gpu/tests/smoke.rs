use rustshade_core::Preset;

fn has_adapter() -> bool {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).is_ok()
}

fn make_spec(id: &str, params: &[(&str, f64)]) -> rustshade_core::EffectSpec {
    let mut table = toml::Table::new();
    for (k, v) in params {
        table.insert(k.to_string(), toml::Value::Float(*v));
    }
    rustshade_core::EffectSpec {
        id: id.into(),
        enabled: true,
        params: table,
    }
}

fn test_image() -> image::RgbaImage {
    image::RgbaImage::from_fn(64, 64, |x, y| {
        let v = ((x as f32 / 64.0) * 255.0) as u8;
        image::Rgba([v, 80, 200 - v, 255])
    })
}

#[test]
fn gpu_pipeline_runs_when_adapter_available() {
    if !has_adapter() {
        eprintln!("skipping: no GPU adapter in this environment");
        return;
    }
    let effects = vec![
        make_spec("fxaa", &[("subpixel", 0.5)]),
        make_spec("grade", &[("exposure", 0.2), ("saturation", 1.1)]),
        make_spec("bloom", &[("strength", 0.4)]),
        make_spec("sharpen", &[("amount", 0.5)]),
        make_spec("vignette", &[("amount", 0.3)]),
        make_spec("grain", &[("strength", 0.05)]),
        make_spec("scanlines", &[("amount", 0.1)]),
    ];
    let img = test_image();
    let result = rustshade_gpu::GpuRenderer::new()
        .unwrap()
        .apply(&effects, &img, 1);
    let out = result.expect("gpu apply should succeed");
    assert_eq!(out.width(), 64);
    assert_eq!(out.height(), 64);

    // Preset round-trip through the registry must stay valid too.
    let preset = Preset {
        settings: Default::default(),
        effects,
    };
    rustshade_core::validate(&preset).expect("effects should be valid");
}

#[test]
fn gpu_identity_returns_same_pixels() {
    if !has_adapter() {
        eprintln!("skipping: no GPU adapter in this environment");
        return;
    }
    let img = test_image();
    let effects = vec![make_spec("grade", &[("exposure", 0.0)])];
    let out = rustshade_gpu::GpuRenderer::new()
        .unwrap()
        .apply(&effects, &img, 0)
        .unwrap();
    for (a, b) in out.pixels().zip(img.pixels()) {
        let d = (i16::from(a.0[0]) - i16::from(b.0[0])).abs();
        assert!(d <= 1, "identity grade drifted by {d} at {:?}", a);
    }
}
