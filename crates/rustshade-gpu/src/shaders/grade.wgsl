fn luma3(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    var col = s.rgb * pow(2.0, u.p0.x);

    let ca = 1.0 + u.p0.y;
    col = (col - vec3(0.5)) * vec3(ca) + vec3(0.5);

    let warm_r = 1.0 + u.p0.w * 0.2;
    let warm_b = 1.0 - u.p0.w * 0.2;
    let tint_r = 1.0 + u.p1.x * 0.1;
    let tint_g = 1.0 - u.p1.x * 0.1;
    col *= vec3(warm_r * tint_r, tint_g, warm_b * tint_r);

    let l = luma3(col);
    col = vec3(l) + (col - vec3(l)) * u.p0.z;
    col = pow(max(col, vec3(0.0)), vec3(1.0 / max(u.p1.y, 1e-4)));
    col = clamp(col + vec3(u.p1.z), vec3(0.0), vec3(1.0));
    return vec4(col, s.a);
}
