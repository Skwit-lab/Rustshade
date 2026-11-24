fn lum3(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

fn smoothstep_s(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / max(e1 - e0, 1e-6), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv).rgb;
    let mask = smoothstep_s(u.p0.x - u.p0.y, u.p0.x + 1e-4, lum3(c));
    return vec4(c * vec3(mask), 1.0);
}
