// Exact integer implementation of paintcraft.core/default-v1.
struct Params {
    x: u32, y: u32, width: u32, height: u32,
    canvas_width: u32, opacity: u32, _pad0: u32, _pad1: u32,
}
@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> composite: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
fn unpack(p: u32) -> vec4<u32> {
    return vec4<u32>(p & 255u, (p >> 8u) & 255u, (p >> 16u) & 255u, p >> 24u);
}
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= params.width || id.y >= params.height { return; }
    let index = (id.y + params.y) * params.canvas_width + id.x + params.x;
    let src = unpack(source[id.y * params.width + id.x]);
    let a = (src.a * params.opacity + 127u) / 255u;
    if a == 0u { return; }
    let dst = unpack(composite[index]);
    let total = a * 255u + dst.a * (255u - a);
    let rgb = (src.rgb * a * 255u + dst.rgb * dst.a * (255u - a) + vec3<u32>(total / 2u)) / total;
    let alpha = (total + 127u) / 255u;
    composite[index] = rgb.r | (rgb.g << 8u) | (rgb.b << 16u) | (alpha << 24u);
}
