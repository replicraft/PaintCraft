// Exact integer implementation of paintcraft.core/default-v1.
struct Params {
    x: u32, y: u32, width: u32, height: u32,
    canvas_width: u32, opacity: u32, blend_mode: u32, opaque_backdrop: u32,
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
    if params.blend_mode == 0u {
    let total = a * 255u + dst.a * (255u - a);
    let rgb = (src.rgb * a * 255u + dst.rgb * dst.a * (255u - a) + vec3<u32>(total / 2u)) / total;
    let alpha = (total + 127u) / 255u;
    composite[index] = rgb.r | (rgb.g << 8u) | (rgb.b << 16u) | (alpha << 24u);
        return;
    }
    let inverse_a = 255u - a;
    if params.opaque_backdrop != 0u {
        let normal = src.rgb * a + dst.rgb * inverse_a;
        var numerator: vec3<u32>;
        if params.blend_mode == 1u {
            numerator = normal * 255u - src.rgb * a * (vec3<u32>(255u) - dst.rgb);
        } else {
            numerator = normal * 255u + dst.rgb * a * (vec3<u32>(255u) - src.rgb);
        }
        let rgb = (numerator + vec3<u32>(32512u)) / 65025u;
        composite[index] = rgb.r | (rgb.g << 8u) | (rgb.b << 16u) | (255u << 24u);
        return;
    }
    // All nonnegative intermediates + half denominator <=4236541312<u32::MAX.
    let total = a * 255u + dst.a * inverse_a;
    let denominator = total * 255u;
    var mixed: vec3<u32>;
    if params.blend_mode == 1u {
        mixed = src.rgb * dst.rgb;
    } else {
        mixed = vec3<u32>(65025u) - (vec3<u32>(255u) - src.rgb) * (vec3<u32>(255u) - dst.rgb);
    }
    let uncovered = dst.rgb * dst.a * inverse_a + src.rgb * a * (255u - dst.a);
    let numerator = uncovered * 255u + mixed * (a * dst.a);
    let rgb = (numerator + vec3<u32>(denominator / 2u)) / denominator;
    let alpha = (total + 127u) / 255u;
    composite[index] = rgb.r | (rgb.g << 8u) | (rgb.b << 16u) | (alpha << 24u);
}
