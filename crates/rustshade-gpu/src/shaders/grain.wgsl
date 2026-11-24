@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    let x = in.uv.x * 1000.0;
    let y = in.uv.y * 1000.0;
    let f = u.frame * u.p0.z;
    let n = fract(sin(dot(vec3(x, y, u.p0.y + f * 57.0), vec3(12.9898, 78.233, 45.164))) * 43758.5453) * 2.0 - 1.0;
    let l = 0.2126 * s.r + 0.7152 * s.g + 0.0722 * s.b;
    let mid = 4.0 * l * (1.0 - l);
    let w = u.p0.x * (0.25 + 0.75 * mid);
    return vec4(clamp(s.rgb + vec3(n * w), vec3(0.0), vec3(1.0)), s.a);
}
