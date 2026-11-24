@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    let period = max(u.p0.y, 2.0);
    let t = (in.uv.y * u.dims.y + u.p0.z * period) * 6.28318530718 / period;
    let factor = (1.0 - u.p0.x * 0.5) + u.p0.x * 0.5 * cos(t);
    return vec4(s.rgb * vec3(factor), s.a);
}
