struct Parameters {
    // The read rectangle: its top-left pixel in the texture and its size.
    origin: vec2<i32>,
    source: vec2<i32>,
    // 0: mirror at the source's edge; 1: transparent beyond it.
    edge: i32,
    radius: i32,
    // Reduce: source pixels per output pixel. Blur: standard deviation in pixels.
    amount: f32,
    _pad: f32,
    step: vec2<i32>,
    _tail: vec2<i32>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> parameters: Parameters;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn mirrored(i: i32, n: i32) -> i32 {
    let period = 2 * n;
    var m = i % period;
    if (m < 0) {
        m += period;
    }
    return select(m, period - 1 - m, m >= n);
}

fn fetch(p: vec2<i32>) -> vec4<f32> {
    let n = parameters.source;
    if (parameters.edge == 0) {
        let m = vec2<i32>(mirrored(p.x, n.x), mirrored(p.y, n.y));
        return textureLoad(source, parameters.origin + m, 0);
    }
    if (any(p < vec2<i32>(0)) || any(p >= n)) {
        return vec4<f32>(0.0);
    }
    return textureLoad(source, parameters.origin + p, 0);
}

/// Area-weighted average of the source pixels one output pixel covers.
@fragment
fn reduce(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let scale = parameters.amount;
    let start = floor(position.xy) * scale;
    let end = start + vec2<f32>(scale);
    var sum = vec4<f32>(0.0);
    for (var y = i32(floor(start.y)); y < i32(ceil(end.y)); y++) {
        let wy = min(end.y, f32(y + 1)) - max(start.y, f32(y));
        for (var x = i32(floor(start.x)); x < i32(ceil(end.x)); x++) {
            let wx = min(end.x, f32(x + 1)) - max(start.x, f32(x));
            sum += wx * wy * fetch(vec2<i32>(x, y));
        }
    }
    return sum / (scale * scale);
}

/// One direction of a separable Gaussian.
@fragment
fn blur(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let center = vec2<i32>(floor(position.xy));
    let k = -0.5 / (parameters.amount * parameters.amount);
    var sum = vec4<f32>(0.0);
    var weights = 0.0;
    for (var i = -parameters.radius; i <= parameters.radius; i++) {
        let w = exp(k * f32(i * i));
        sum += w * fetch(center + i * parameters.step);
        weights += w;
    }
    return sum / weights;
}
