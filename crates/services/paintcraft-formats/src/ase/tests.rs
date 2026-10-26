use super::*;
use std::cell::Cell;
const MIXED: &[u8] = include_bytes!("../../tests/ase-fixtures/mixed.ase");
const EMPTY: &[u8] = include_bytes!("../../tests/ase-fixtures/empty.ase");
#[test]
fn explicit_rgb_adoption_is_bounded_and_leaves_source_values_untouched() {
    let palette = decode(MIXED, &|| Ok(())).unwrap();
    let colors: Vec<_> = palette.swatches().map(|s| s.color.srgba8()).collect();
    assert_eq!(
        colors,
        [
            Some([255, 0, 0, 255]),
            Some([64, 128, 255, 255]),
            None,
            None,
            Some([191, 191, 191, 255]),
            Some([0, 0, 0, 255])
        ]
    );
    assert_eq!(encode(&palette, &|| Ok(())).unwrap(), MIXED);
    for value in [f32::NAN, f32::INFINITY, -0.001, 1.001] {
        assert!(Color::Gray(value).srgba8().is_none());
        assert!(Color::Rgb([0.0, value, 1.0]).srgba8().is_none());
    }
}
fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    [
        kind.to_be_bytes().as_slice(),
        &(payload.len() as u32).to_be_bytes(),
        payload,
    ]
    .concat()
}
fn file(blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = EMPTY.to_vec();
    bytes[8..12].copy_from_slice(&(blocks.len() as u32).to_be_bytes());
    for block in blocks {
        bytes.extend(block);
    }
    bytes
}
#[test]
fn independently_authored_groups_models_types_names_and_bits_are_retained() {
    let palette = decode(MIXED, &|| Ok(())).unwrap();
    assert_eq!(palette.swatch_count(), 6);
    assert_eq!(palette.entries().len(), 4);
    let Entry::Group { name, swatches } = &palette.entries()[1] else {
        panic!("expected group");
    };
    assert_eq!(name, "印刷 / Print");
    assert_eq!(
        swatches[0],
        Swatch {
            name: "赤 🚀".into(),
            color: Color::Rgb([0.25, 0.5, 1.0]),
            kind: Kind::Normal
        }
    );
    assert_eq!(swatches[1].color, Color::Cmyk([1.0, 0.0, 0.0, 0.125]));
    assert_eq!(swatches[1].kind, Kind::Spot);
    assert_eq!(swatches[2].color, Color::Lab([0.6, -35.0, -5.0]));
    assert_eq!(swatches[3].color, Color::Gray(0.75));
    let Entry::Swatch(last) = &palette.entries()[3] else {
        panic!("expected final color");
    };
    assert_eq!(last.color.channels()[0].to_bits(), (-0.0f32).to_bits());
    assert_eq!(encode(&palette, &|| Ok(())).unwrap(), MIXED);
    let empty = decode(EMPTY, &|| Ok(())).unwrap();
    assert_eq!(empty.swatch_count(), 0);
    assert_eq!(encode(&empty, &|| Ok(())).unwrap(), EMPTY);
}
#[test]
fn independent_swatch_writer_fixture_has_exact_named_cmyk_and_gray_values() {
    let bytes = include_bytes!("../../tests/ase-fixtures/swatch-generated.ase");
    let decoded = decode(bytes, &|| Ok(())).unwrap();
    assert_eq!(decoded.swatch_count(), 2);
    let Entry::Swatch(gray) = &decoded.entries()[0] else {
        panic!("expected gray");
    };
    assert_eq!(gray.name, "Independent gray");
    assert_eq!(gray.color, Color::Gray(0.25));
    let Entry::Group { name, swatches } = &decoded.entries()[1] else {
        panic!("expected group");
    };
    assert_eq!(name, "Print");
    assert_eq!(swatches[0].name, "Ink");
    assert_eq!(swatches[0].color, Color::Cmyk([0.125, 0.5, 0.75, 1.0]));
    assert_eq!(swatches[0].kind, Kind::Spot);
    assert_eq!(encode(&decoded, &|| Ok(())).unwrap(), bytes);
}
#[test]
fn every_truncated_prefix_unknown_version_and_appended_payload_are_refused() {
    for end in 0..MIXED.len() {
        assert!(decode(&MIXED[..end], &|| Ok(())).is_err(), "{end}");
    }
    let mut trailing = MIXED.to_vec();
    trailing.push(0);
    assert!(decode(&trailing, &|| Ok(())).is_err());
    let mut version = MIXED.to_vec();
    version[7] = 1;
    assert!(decode(&version, &|| Ok(())).is_err());
    let mut count = MIXED.to_vec();
    count[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        decode(&count, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(decode(&vec![0; BYTES_MAX + 1], &|| Ok(())).is_err());
}
#[test]
fn crossed_boundaries_nested_unclosed_or_orphan_groups_are_refused() {
    let name = [0, 1, 0, 0];
    let start = block(0xc001, &name);
    let end = block(0xc002, &[]);
    for blocks in [
        vec![start.clone()],
        vec![end.clone()],
        vec![start.clone(), start.clone(), end.clone(), end.clone()],
        vec![block(0xc002, &[0])],
        vec![block(0xc003, &[])],
    ] {
        assert!(decode(&file(&blocks), &|| Ok(())).is_err());
    }
    let mut crossed = MIXED.to_vec();
    // First color payload is 26 bytes. A shorter declaration must not allow the
    // backend to consume the next block and then rewind to a forged boundary.
    crossed[14..18].copy_from_slice(&2u32.to_be_bytes());
    assert!(decode(&crossed, &|| Ok(())).is_err());
}
#[test]
fn invalid_names_channels_and_swatch_kinds_never_reach_the_backend() {
    for name in [
        vec![0, 0],
        vec![0, 1, 0, 65],
        vec![0, 2, 0xd8, 0, 0, 0],
        vec![0, 3, 0, 0, 0, 65, 0, 0],
    ] {
        assert!(
            decode(&file(&[block(0xc001, &name), block(0xc002, &[])]), &|| Ok(
                ()
            ))
            .is_err()
        );
    }
    let mut payload = vec![0, 1, 0, 0];
    payload.extend(b"Gray");
    for bits in [
        f32::NAN.to_bits(),
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
    ] {
        let mut body = payload.clone();
        body.extend(bits.to_be_bytes());
        body.extend(2u16.to_be_bytes());
        assert!(decode(&file(&[block(1, &body)]), &|| Ok(())).is_err());
    }
    payload.extend(0.5f32.to_be_bytes());
    payload.extend(3u16.to_be_bytes());
    assert!(decode(&file(&[block(1, &payload)]), &|| Ok(())).is_err());
}
#[test]
fn typed_writer_checks_names_counts_and_nonfinite_samples_before_encoding() {
    let make = |name: String, color| {
        Entry::Swatch(Swatch {
            name,
            color,
            kind: Kind::Normal,
        })
    };
    assert!(
        Palette::new(
            vec![make("x".repeat(NAME_UNITS_MAX + 1), Color::Gray(0.0))],
            &|| Ok(())
        )
        .is_err()
    );
    assert!(Palette::new(vec![make("bad\0name".into(), Color::Gray(0.0))], &|| Ok(())).is_err());
    assert!(Palette::new(vec![make("bad".into(), Color::Gray(f32::NAN))], &|| Ok(())).is_err());
    assert!(
        Palette::new(
            vec![make("".into(), Color::Gray(0.0)); SWATCHES_MAX + 1],
            &|| Ok(())
        )
        .is_err()
    );
    let group = Entry::Group {
        name: String::new(),
        swatches: vec![],
    };
    assert!(Palette::new(vec![group; GROUPS_MAX + 1], &|| Ok(())).is_err());
    assert!(
        Palette::new(
            vec![make("x".repeat(NAME_UNITS_MAX), Color::Gray(0.0)); 256],
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn every_adapter_checkpoint_can_cancel_without_returning_a_partial_resource() {
    let count = Cell::new(0);
    let palette = decode(MIXED, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 0..count.get() {
        let calls = Cell::new(0);
        assert!(
            decode(MIXED, &|| {
                let n = calls.get();
                calls.set(n + 1);
                if n == stop {
                    Err(resource("cancelled"))
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
    }
    count.set(0);
    encode(&palette, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 0..count.get() {
        let calls = Cell::new(0);
        assert!(
            encode(&palette, &|| {
                let n = calls.get();
                calls.set(n + 1);
                if n == stop {
                    Err(resource("cancelled"))
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
    }
}
