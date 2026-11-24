@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var tex3: texture_3d<f32>;
@group(0) @binding(3) var samp: sampler;

@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(tex, samp, in.uv, 0.0);
    let dim = u.p1.x;
    let pos = c.rgb * vec3((dim - 1.0) / dim) + vec3(0.5 / dim);
    let m = textureSampleLevel(tex3, samp, pos, 0.0);
    return vec4(mix(c.rgb, m.rgb, vec3(u.p0.x)), c.a);
}
