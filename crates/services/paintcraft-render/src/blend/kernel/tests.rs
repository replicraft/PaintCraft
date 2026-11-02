use super::*;
use paintcraft_core::types::json::{self, Value};
fn pixel(v: &Value) -> Rgba {
    Rgba(
        v.as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u8)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
    )
}
#[test]
fn fixed_opaque_and_general_dispatch_match_all_independent_literal_channels() {
    let oracle = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../foundation/paintcraft-core/tests/layer-blend-fixtures/oracle.json"
    )))
    .unwrap();
    let mut cases = 0;
    for row in oracle.get("vectors").unwrap().as_array().unwrap() {
        let back = pixel(row.get("back").unwrap());
        let mut source = pixel(row.get("front").unwrap());
        let mask = row.get("own_mask").unwrap().as_i64().unwrap() as u16;
        source.0[3] = ((u16::from(source.a()) * mask + 127) / 255) as u8;
        source = source.with_opacity(row.get("opacity").unwrap().as_i64().unwrap() as u8);
        for case in row.get("cases").unwrap().as_array().unwrap() {
            let mode = match case.get("mode").and_then(Value::as_str).unwrap() {
                "normal" => Mode::Normal,
                "multiply" => Mode::Multiply,
                "screen" => Mode::Screen,
                _ => unreachable!(),
            };
            let expected = pixel(case.get("rgba").unwrap());
            let kind = if mode == Mode::Normal {
                Kind::Normal
            } else if back.a() == 255 {
                Kind::Opaque(mode)
            } else {
                Kind::General(mode)
            };
            assert_eq!(super::pixel(kind, source, back).unwrap(), expected);
            cases += 1;
        }
    }
    assert_eq!(cases, 54);
    eprintln!(
        "CPU_BLEND_LAYOUT arch={} pointer={} active_control_quote={} quote={}",
        std::env::consts::ARCH,
        std::mem::size_of::<usize>(),
        crate::blend::control_quote(),
        std::mem::size_of::<crate::blend::Quote>()
    );
    assert_eq!(
        super::pixel(
            Kind::Opaque(Mode::Screen),
            Rgba([1, 2, 3, 255]),
            Rgba([0, 0, 0, 128])
        )
        .unwrap_err()
        .code,
        ErrorCode::InvariantViolation
    );
}
