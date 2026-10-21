// Shared layout with scaled::Params. Each output invocation owns its accumulator.
struct Params {
    native_origin: vec2<u32>, native_size: vec2<u32>,
    sample_origin: vec2<u32>, output_size: vec2<u32>,
    output_origin: vec2<u32>, step: u32, edge: u32,
}
struct Sums { values: array<vec2<u32>, 4>, }
@group(0) @binding(0) var<storage, read> native_pixels: array<u32>;
@group(0) @binding(1) var<storage, read_write> sums: array<Sums>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var display_image: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(4) var<storage, read> cpu_sums: array<Sums>;

@compute @workgroup_size(8, 8)
fn fill_known(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= params.output_size) { return; }
    let span = params.edge * params.step;
    let tile = (params.native_origin + id.xy * params.step) / span - params.native_origin / span;
    let columns = (params.native_origin.x % span + params.native_size.x + span - 1u) / span;
    let color = cpu_sums[tile.y * columns + tile.x].values[0];
    if color.y != 0u {
        textureStore(display_image, vec2<i32>(id.xy), display_color(unpack_color(color.x)));
    }
}

@compute @workgroup_size(8, 8)
fn accumulate(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= params.output_size) { return; }
    let cell = params.sample_origin + id.xy * params.step;
    let start = max(cell, params.native_origin);
    let end = min(cell + vec2<u32>(params.step), params.native_origin + params.native_size);
    if any(start >= end) { return; }
    // A native block is at most 256-square, so even sum(channel*alpha) fits u32.
    var partial = vec4<u32>(0u);
    for (var y = start.y; y < end.y; y = y + 1u) {
        for (var x = start.x; x < end.x; x = x + 1u) {
            let pixel = unpack_color(native_pixels[(y - params.native_origin.y) * params.native_size.x + x - params.native_origin.x]);
            partial = partial + vec4<u32>(pixel.rgb * pixel.a, pixel.a);
        }
    }
    let index = id.y * params.output_size.x + id.x;
    for (var c = 0u; c < 4u; c = c + 1u) {
        sums[index].values[c] = wide_add(sums[index].values[c], vec2<u32>(partial[c], 0u));
    }
}

@compute @workgroup_size(8, 8)
fn resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= params.output_size) { return; }
    let index = id.y * params.output_size.x + id.x;
    let valid = min(vec2<u32>(params.step), params.native_size - id.xy * params.step);
    let count = wide_product(valid.x, valid.y);
    let alpha_sum = wide_add(sums[index].values[3], cpu_sums[index].values[3]);
    let alpha = wide_byte_quotient(wide_add(alpha_sum, wide_half(count)), count);
    var pixel = vec4<u32>(0u);
    if alpha != 0u {
        for (var c = 0u; c < 3u; c = c + 1u) {
            let total = wide_add(sums[index].values[c], cpu_sums[index].values[c]);
            pixel[c] = wide_byte_quotient(wide_add(total, wide_half(alpha_sum)), alpha_sum);
        }
        pixel.a = alpha;
    }
    textureStore(display_image, vec2<i32>(id.xy + params.output_origin), display_color(pixel));
}
