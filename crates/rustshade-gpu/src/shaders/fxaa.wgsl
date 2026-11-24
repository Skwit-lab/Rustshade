fn lum3(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

@fragment
fn fs_main(in: VSOutput) -> @location(0) vec4<f32> {
    let px = 1.0 / u.dims;
    let cNW = textureSample(tex, samp, in.uv + vec2(-px.x, -px.y));
    let cN  = textureSample(tex, samp, in.uv + vec2(0.0, -px.y));
    let cNE = textureSample(tex, samp, in.uv + vec2(px.x, -px.y));
    let cW  = textureSample(tex, samp, in.uv + vec2(-px.x, 0.0));
    let cM  = textureSample(tex, samp, in.uv);
    let cE  = textureSample(tex, samp, in.uv + vec2(px.x, 0.0));
    let cSW = textureSample(tex, samp, in.uv + vec2(-px.x, px.y));
    let cS  = textureSample(tex, samp, in.uv + vec2(0.0, px.y));
    let cSE = textureSample(tex, samp, in.uv + vec2(px.x, px.y));

    let lNW = lum3(cNW.rgb); let lN = lum3(cN.rgb); let lNE = lum3(cNE.rgb);
    let lW = lum3(cW.rgb); let lM = lum3(cM.rgb); let lE = lum3(cE.rgb);
    let lSW = lum3(cSW.rgb); let lS = lum3(cS.rgb); let lSE = lum3(cSE.rgb);

    let lmin = min(lNW, min(lN, min(lNE, min(lW, min(lM, min(lE, min(lSW, min(lS, lSE))))))));
    let lmax = max(lNW, max(lN, max(lNE, max(lW, max(lM, max(lE, max(lSW, max(lS, lSE))))))));

    if (lmax - lmin < u.p0.y) {
        return cM;
    }

    let gradH = abs(lW - lE);
    let gradV = abs(lN - lS);
    var sample = vec3(0.0);
    if (gradH >= gradV) {
        sample = (cW.rgb + cE.rgb) * vec3(0.5);
    } else {
        sample = (cN.rgb + cS.rgb) * vec3(0.5);
    }
    let out = vec3(lM) + (sample - vec3(lM)) * u.p0.x;
    return vec4(out, cM.a);
}
