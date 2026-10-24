@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> parameters: vec4<f32>;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = textureLoad(source, vec2<i32>(position.xy), 0);
    let luminance = dot(pixel.rgb, vec3<f32>(0.213, 0.715, 0.072));
    let rgb = vec3<f32>(luminance) + parameters.x * (pixel.rgb - vec3<f32>(luminance));
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(pixel.a)), pixel.a);
}
