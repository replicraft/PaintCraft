// Exact portable unsigned pairs: low word first. No shader-int64 or float quotient.
fn wide_add(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
    let low = a.x + b.x;
    return vec2<u32>(low, a.y + b.y + select(0u, 1u, low < a.x));
}
fn wide_half(a: vec2<u32>) -> vec2<u32> {
    return vec2<u32>((a.x >> 1u) | (a.y << 31u), a.y >> 1u);
}
fn wide_product(a: u32, b: u32) -> vec2<u32> {
    let a0 = a & 65535u;
    let a1 = a >> 16u;
    let b0 = b & 65535u;
    let b1 = b >> 16u;
    let w0 = a0 * b0;
    let t = a1 * b0 + (w0 >> 16u);
    let w1 = t & 65535u;
    let w2 = t >> 16u;
    let upper = a0 * b1 + w1;
    return vec2<u32>((upper << 16u) | (w0 & 65535u), a1 * b1 + w2 + (upper >> 16u));
}
fn wide_scale(a: vec2<u32>, byte: u32) -> vec2<u32> {
    let low = wide_product(a.x, byte);
    return vec2<u32>(low.x, low.y + a.y * byte);
}
fn wide_le(a: vec2<u32>, b: vec2<u32>) -> bool {
    return a.y < b.y || (a.y == b.y && a.x <= b.x);
}
// The admitted display policy bounds the quotient to 0..255 and products to u64.
fn wide_byte_quotient(n: vec2<u32>, d: vec2<u32>) -> u32 {
    var quotient = 0u;
    var bit = 128u;
    loop {
        if bit == 0u { break; }
        let candidate = quotient | bit;
        if wide_le(wide_scale(d, candidate), n) { quotient = candidate; }
        bit = bit >> 1u;
    }
    return quotient;
}
fn display_premultiply(c: u32, a: u32) -> u32 {
    let product = c * a + 128u;
    return (product + (product >> 8u)) >> 8u;
}
fn display_color(pixel: vec4<u32>) -> vec4<f32> {
    return vec4<f32>(vec4<u32>(
        display_premultiply(pixel.r, pixel.a), display_premultiply(pixel.g, pixel.a),
        display_premultiply(pixel.b, pixel.a), pixel.a)) / 255.0;
}
fn unpack_color(pixel: u32) -> vec4<u32> {
    return vec4<u32>(pixel & 255u, (pixel >> 8u) & 255u, (pixel >> 16u) & 255u, pixel >> 24u);
}
