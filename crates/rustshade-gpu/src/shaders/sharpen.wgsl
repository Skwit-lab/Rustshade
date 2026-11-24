fn lum3(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    let sigma = max(u.p0.y, 0.25) * 0.5;
    var blur = vec3(0.0);
    var wsum = 0.0;
    for (var j = -2; j <= 2; j = j + 1) {
        for (var i = -2; i <= 2; i = i + 1) {
            let wx = exp(-f32(i) * f32(i) / (2.0 * sigma * sigma));
            let wy = exp(-f32(j) * f32(j) / (2.0 * sigma * sigma));
            let w = wx * wy;
            let uv2 = in.uv + vec2(f32(i) / u.dims.x, f32(j) / u.dims.y);
            blur = blur + textureSample(tex, samp, uv2).rgb * vec3(w);
            wsum = wsum + w;
        }
    }
    blur = blur / vec3(wsum);
    var diff = lum3(s.rgb) - lum3(blur);
    if (abs(diff) < u.p0.z) {
        diff = 0.0;
    }
    return vec4(clamp(s.rgb + vec3(diff * u.p0.x), vec3(0.0), vec3(1.0)), s.a);
}
