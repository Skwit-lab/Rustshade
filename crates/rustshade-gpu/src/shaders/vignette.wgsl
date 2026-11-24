fn smoothstep_s(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / max(e1 - e0, 1e-6), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    let d = length(in.uv - vec2(0.5)) / 0.70710678;
    let t = smoothstep_s(u.p0.y, u.p0.y + u.p0.z, d);
    return vec4(s.rgb * (1.0 - u.p0.x * t), s.a);
}
