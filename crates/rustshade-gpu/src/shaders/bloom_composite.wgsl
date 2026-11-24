@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var tex2: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(tex, samp, in.uv, 0.0);
    let glow = textureSampleLevel(tex2, samp, in.uv, 0.0);
    return vec4(clamp(c.rgb + glow.rgb * vec3(u.p0.x), vec3(0.0), vec3(1.0)), c.a);
}
