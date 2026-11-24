@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let sigma = max(u.p0.x, 0.5) * 0.25;
    let step_uv = select(vec2(1.0 / u.dims.x, 0.0), vec2(0.0, 1.0 / u.dims.y), u.p1.x > 0.5);
    var acc = vec3(0.0);
    var wsum = 0.0;
    for (var i = -6; i <= 6; i = i + 1) {
        let t = f32(i);
        let w = exp(-t * t / (2.0 * sigma * sigma));
        acc = acc + textureSample(tex, samp, in.uv + step_uv * t).rgb * vec3(w);
        wsum = wsum + w;
    }
    return vec4(acc / vec3(wsum), 1.0);
}
