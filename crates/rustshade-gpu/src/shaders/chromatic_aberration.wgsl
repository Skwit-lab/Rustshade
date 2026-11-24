@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    let d = in.uv - vec2(0.5);
    let d2 = dot(d, d);
    let mag = d2 * u.p0.x * 32.0;
    let off = vec2(d.x * mag / u.dims.x, d.y * mag / u.dims.y);
    let r = textureSample(tex, samp, in.uv + off).r;
    let b = textureSample(tex, samp, in.uv - off).b;
    return vec4(r, s.g, b, s.a);
}
