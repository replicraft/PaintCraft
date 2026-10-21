// Match ecolor 0.37 Color32::from_rgba_unmultiplied exactly in encoded sRGB.
@group(0) @binding(0) var<storage, read> straight: array<u32>;
@group(0) @binding(1) var display_image: texture_storage_2d<rgba8unorm, write>;
fn premultiply(channel: u32, alpha: u32) -> u32 {
    let product = channel * alpha + 128u;
    return (product + (product >> 8u)) >> 8u;
}
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(display_image);
    if id.x >= size.x || id.y >= size.y { return; }
    let pixel = straight[id.y * size.x + id.x];
    let alpha = pixel >> 24u;
    let color = vec4<u32>(
        premultiply(pixel & 255u, alpha),
        premultiply((pixel >> 8u) & 255u, alpha),
        premultiply((pixel >> 16u) & 255u, alpha), alpha);
    textureStore(display_image, vec2<i32>(id.xy), vec4<f32>(color) / 255.0);
}
