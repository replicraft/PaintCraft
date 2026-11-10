use super::*;
use manifest::*;
use paintcraft_core::types::json::{self, Value};
#[path = "tests/controls.rs"]
mod controls;
#[path = "tests/ownership.rs"]
mod ownership;
fn rows() -> Vec<Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/source8-service-fixtures/oracle.json"
    )))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn text<'a>(r: &'a Value, key: &str) -> &'a str {
    r.get(key).unwrap().as_str().unwrap()
}
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
fn n(r: &Value, key: &str) -> u64 {
    r.get(key).unwrap().as_i64().unwrap() as u64
}
fn put32(b: &mut [u8; WINDOW], i: usize, n: u32) {
    b[i..i + 4].copy_from_slice(&n.to_le_bytes());
}
fn put64(b: &mut [u8; WINDOW], i: usize, n: u64) {
    b[i..i + 8].copy_from_slice(&n.to_le_bytes());
}
fn packet(row: &Value, service: &LayerBlendService) -> [u8; WINDOW] {
    let mut b = [0; WINDOW];
    b[..4].copy_from_slice(b"PC8W");
    b[4] = 1;
    b[6] = 1;
    b[7] = 4;
    let noise = text(row, "mode") == "Noise";
    if noise {
        b[7] |= 1;
    }
    if row.get("stored_noise").unwrap().as_bool().unwrap() {
        b[7] |= 2;
    }
    put32(&mut b, 8, 1);
    b[16..32].fill(0x33);
    b[32..48].fill(0x55);
    put64(&mut b, 48, 7);
    b[56..72].copy_from_slice(&unhex(text(row, "document")));
    b[72..88].copy_from_slice(&unhex(text(row, "source_state")));
    b[88..104].fill(0x44);
    b[104..136].copy_from_slice(&unhex(text(row, "sha256")));
    b[136..168].copy_from_slice(&service.profile);
    let canvas = row.get("canvas").unwrap().as_array().unwrap();
    put32(&mut b, 168, canvas[0].as_i64().unwrap() as u32);
    put32(&mut b, 172, canvas[1].as_i64().unwrap() as u32);
    put64(&mut b, 176, unhex(text(row, "hex")).len() as u64);
    put64(&mut b, 184, BASE);
    put64(&mut b, 192, if noise { 1048576 } else { 0 });
    put64(
        &mut b,
        200,
        if noise {
            paintcraft_render::noise_work::NoiseAdmission::control_quote()
                .unwrap()
                .counter_bytes
        } else {
            0
        },
    );
    // Existing frozen finite profile, not a production source/W forecast.
    let roles = [
        1 << 20,
        2 << 20,
        1 << 20,
        1 << 20,
        1 << 20,
        2 << 20,
        1 << 20,
        4096,
        4 << 20,
        16384,
        2 << 20,
        service.controls.total().unwrap(),
    ];
    for (i, v) in roles.into_iter().enumerate() {
        put64(&mut b, 208 + i * 8, v);
    }
    b
}
fn command(
    service: &mut LayerBlendService,
    base: &[u8; WINDOW],
    kind: u8,
    request: u32,
    operation: (u64, u64, [u32; 4], u32, u32),
) {
    let (token, generation, visible, level, ordinal) = operation;
    service.window = *base;
    service.window[6] = kind;
    put32(&mut service.window, 8, request);
    put64(&mut service.window, 304, token);
    put64(&mut service.window, 312, generation);
    for (i, v) in visible.into_iter().enumerate() {
        put32(&mut service.window, 320 + i * 4, v);
    }
    put32(&mut service.window, 336, level);
    put32(&mut service.window, 340, ordinal);
}
fn load(service: &mut LayerBlendService, row: &Value) -> [u8; WINDOW] {
    let p = packet(row, service);
    service.window = p;
    assert_eq!(service.stage().unwrap(), 0);
    let wire = unhex(text(row, "hex"));
    assert_eq!(service.source_len().unwrap(), wire.len());
    assert_ne!(service.source_ptr().unwrap(), 0);
    service.stage.as_mut().unwrap().data.copy_from_slice(&wire);
    assert_eq!(service.seal().unwrap(), 1);
    assert!(service.source_ptr().is_err());
    assert!(service.source_len().is_err());
    assert!(service.stage.is_none());
    p
}
fn close(mut service: LayerBlendService, base: &[u8; WINDOW], request: u32) {
    let budget = service.budget.clone();
    command(&mut service, base, 5, request, (0, 0, [0; 4], 0, 0));
    assert_eq!(service.close().unwrap(), 6);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(LayerBlendService::live_instances(), 0);
}
#[test]
fn genuine_full_parent_pixels_global_grid_two_patch_and_original_retirement() {
    for row in rows() {
        let mut service = LayerBlendService::new().unwrap();
        let base = load(&mut service, &row);
        assert_ne!(
            base[32..48],
            base[72..88],
            "public ticket is intentionally not private source state"
        );
        let visible: [u32; 4] = row
            .get("visible")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let level = n(&row, "level") as u32;
        command(&mut service, &base, 2, 2, (1, 1, visible, level, 0));
        assert_eq!(service.begin_view().unwrap(), 2);
        let grid = service.view.as_ref().unwrap().grid;
        let mut rgba = vec![0; grid.size[0] as usize * grid.size[1] as usize * 4];
        let mut request = 3;
        let mut paid = service.work_used(false);
        for ordinal in 0..grid.patches {
            assert_eq!(service.step().unwrap(), 3);
            assert!(service.work_used(false) > paid);
            paid = service.work_used(false);
            let patch = grid.patch(ordinal).unwrap();
            let pixels = service.image.as_ref().unwrap().rgba();
            assert_eq!(pixels.len(), service.rgba_len().unwrap());
            assert_ne!(service.rgba_ptr().unwrap(), 0);
            for y in 0..patch.patch.height() as usize {
                let dst = ((patch.patch.y0 as usize + y) * grid.size[0] as usize
                    + patch.patch.x0 as usize)
                    * 4;
                let width = patch.patch.width() as usize * 4;
                rgba[dst..dst + width].copy_from_slice(&pixels[y * width..(y + 1) * width]);
            }
            assert!(
                service.step().is_err(),
                "cannot render over an outstanding image"
            );
            command(
                &mut service,
                &base,
                3,
                request,
                (1, 1, visible, level, ordinal),
            );
            request += 1;
            assert_eq!(
                service.ack().unwrap(),
                if ordinal + 1 == grid.patches { 4 } else { 2 }
            );
            assert!(service.image.is_none());
        }
        assert_eq!(
            rgba,
            unhex(text(&row, "rgba_hex")),
            "{}",
            text(&row, "name")
        );
        if text(&row, "mode") == "Noise" {
            assert!(service.work_used(true) > 0);
        } else {
            assert_eq!(service.work_used(true), 0);
        }
        close(service, &base, request);
    }
}
