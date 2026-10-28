use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    policy::{MaskIntent, MaskMode},
    resources::SharedBudget,
};
use paintcraft_input::{Modifiers, tablet::*};
use paintcraft_tools::pen_stroke::{PenStroke, Style};
use std::cell::Cell;
use uuid::Uuid;
fn sample(sequence: u64, phase: Phase, pressure: Option<f32>) -> Sample {
    Sample {
        identity: Identity {
            device: 1,
            pointer: 2,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 1,
            micros: sequence * 100,
            clock: Clock::BrowserPerformance,
        },
        phase,
        position: (8.0 + sequence as f64, 8.0),
        transform: Transform {
            owner: 1,
            epoch: 2,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        buttons: Buttons(Buttons::TIP),
        modifiers: Modifiers::default(),
        pressure,
        tilt: Some(Tilt::Degrees(12.0, -8.0)),
        twist_degrees: Some(41.0),
        contact: Some((2.0, 3.0)),
        provenance: Provenance::Actual,
    }
}
fn style(mode: MaskMode) -> Style {
    Style {
        layer: Uuid::from_u128(1),
        diameter: 8,
        color: Rgba([23, 59, 151, 191]),
        mode,
        intent: MaskIntent::Brush,
        settings: Default::default(),
    }
}

#[test]
fn captured_curves_match_explicit_remapped_geometry_and_never_change_source_samples() {
    use paintcraft_core::types::pen::PressureCurve;
    let budget = SharedBudget::new(32 << 20);
    let clip = Rect::new(0, 0, 64, 32).unwrap();
    let source = [
        sample(1, Phase::Down, Some(0.25)),
        sample(2, Phase::Move, Some(0.8)),
        sample(3, Phase::Up, Some(0.1)),
    ];
    let mut masks = Vec::new();
    for curve in PressureCurve::ALL {
        let mut captured = style(MaskMode::SourceOver);
        captured.settings.curve = curve;
        let mut stroke = PenStroke::begin(captured, source[0], &budget).unwrap();
        for point in &source[1..] {
            stroke.append(*point).unwrap();
        }
        let mut reference = None;
        for mut point in source {
            let q = (f64::from(point.pressure.unwrap()) * 65535.0).round() as u16;
            // Mapping to a Q16-representable input exercises the existing linear
            // raster path as the independent reference, including the quote.
            point.pressure = Some(f32::from(curve.map(q)) / 65535.0);
            if let Some(stroke) = &mut reference {
                PenStroke::append(stroke, point).unwrap();
            } else {
                reference =
                    Some(PenStroke::begin(style(MaskMode::SourceOver), point, &budget).unwrap());
            }
        }
        let preview = stroke.clone().resolve(clip, &budget, &|| Ok(())).unwrap();
        let released = stroke.resolve(clip, &budget, &|| Ok(())).unwrap();
        let expected = reference
            .unwrap()
            .resolve(clip, &budget, &|| Ok(()))
            .unwrap();
        assert_eq!(preview.spans(), released.spans());
        assert_eq!(released.spans(), expected.spans());
        assert_eq!(stroke.samples(), &source);
        masks.push(released.spans().to_vec());
    }
    assert_ne!(masks[0], masks[1]);
    assert_ne!(masks[0], masks[2]);
}

#[test]
fn disabling_size_pressure_matches_missing_width_without_losing_raw_zero_or_axes() {
    let budget = SharedBudget::new(16 << 20);
    let mut captured = style(MaskMode::SourceOver);
    captured.settings.size_pressure = false;
    let mut disabled =
        PenStroke::begin(captured, sample(1, Phase::Down, Some(0.0)), &budget).unwrap();
    disabled.append(sample(2, Phase::Up, Some(0.4))).unwrap();
    let mut constant = PenStroke::begin(
        style(MaskMode::SourceOver),
        sample(1, Phase::Down, None),
        &budget,
    )
    .unwrap();
    constant.append(sample(2, Phase::Up, None)).unwrap();
    let clip = Rect::new(0, 0, 32, 32).unwrap();
    assert_eq!(
        disabled.resolve(clip, &budget, &|| Ok(())).unwrap().spans(),
        constant.resolve(clip, &budget, &|| Ok(())).unwrap().spans()
    );
    assert_eq!(disabled.samples()[0].pressure, Some(0.0));
    assert_eq!(disabled.samples()[0].twist_degrees, Some(41.0));
}
#[test]
fn immutable_sample_prefix_and_style_survive_pressure_rotation_and_button_changes() {
    let budget = SharedBudget::new(16 << 20);
    let first = sample(1, Phase::Down, Some(0.25));
    let mut stroke = PenStroke::begin(style(MaskMode::SourceOver), first, &budget).unwrap();
    let preview = stroke.clone();
    let mut next = sample(2, Phase::Move, Some(0.75));
    next.position = first.position;
    next.twist_degrees = Some(72.0);
    next.buttons = Buttons(Buttons::TIP | Buttons::BARREL);
    stroke.append(next).unwrap();
    assert_eq!(preview.samples(), &[first]);
    assert_eq!(stroke.samples(), &[first, next]);
    assert_eq!(stroke.style(), style(MaskMode::SourceOver));
    let resolved = stroke
        .resolve(Rect::new(0, 0, 32, 32).unwrap(), &budget, &|| Ok(()))
        .unwrap();
    assert!(!resolved.spans().is_empty());
    assert!(budget.usage().reserved > 0);
    drop((preview, stroke, resolved));
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn the_same_captured_preview_and_release_resolve_identical_commands_for_every_blend_policy() {
    for mode in [
        MaskMode::SourceOver,
        MaskMode::Replace,
        MaskMode::DestinationOut,
    ] {
        let budget = SharedBudget::new(16 << 20);
        let mut stroke =
            PenStroke::begin(style(mode), sample(1, Phase::Down, None), &budget).unwrap();
        stroke.append(sample(2, Phase::Move, Some(0.3))).unwrap();
        stroke.append(sample(3, Phase::Up, Some(0.0))).unwrap();
        let snapshot = stroke.clone();
        let clip = Rect::new(0, 0, 32, 32).unwrap();
        let preview = snapshot.resolve(clip, &budget, &|| Ok(())).unwrap();
        let released = stroke.resolve(clip, &budget, &|| Ok(())).unwrap();
        assert_eq!(
            preview.operations(&|| Ok(())).unwrap(),
            released.operations(&|| Ok(())).unwrap()
        );
        assert_eq!(preview.spans(), released.spans());
        assert_eq!(preview.style(), style(mode));
    }
}
#[test]
fn denial_cancellation_and_invalid_source_never_change_captured_input_or_leak_charges() {
    let budget = SharedBudget::new(16 << 20);
    let mut stroke = PenStroke::begin(
        style(MaskMode::SourceOver),
        sample(1, Phase::Down, None),
        &budget,
    )
    .unwrap();
    stroke.append(sample(2, Phase::Up, Some(0.75))).unwrap();
    let before = stroke.samples().to_vec();
    let clip = Rect::new(0, 0, 32, 32).unwrap();
    assert_eq!(
        stroke
            .resolve(clip, &SharedBudget::new(0), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(stroke.append(sample(3, Phase::Move, None)).is_err());
    let calls = Cell::new(0);
    drop(
        stroke
            .resolve(clip, &budget, &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    let retained = budget.usage().reserved;
    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        let error = stroke
            .resolve(clip, &budget, &|| {
                count.set(count.get() + 1);
                if count.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel pen"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(stroke.samples(), before);
        assert_eq!(budget.usage().reserved, retained);
    }
    let hostile = Rect {
        x0: i64::MIN,
        y0: 0,
        x1: i64::MAX,
        y1: 4,
    };
    assert_eq!(
        stroke
            .resolve(hostile, &budget, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn missing_pressure_zero_pressure_prediction_and_cow_denial_have_distinct_results() {
    let budget = SharedBudget::new(16 << 20);
    let clip = Rect::new(0, 0, 32, 32).unwrap();
    let missing = PenStroke::begin(
        style(MaskMode::SourceOver),
        sample(1, Phase::Down, None),
        &budget,
    )
    .unwrap();
    let zero = PenStroke::begin(
        style(MaskMode::SourceOver),
        sample(1, Phase::Down, Some(0.0)),
        &budget,
    )
    .unwrap();
    assert!(
        !missing
            .resolve(clip, &budget, &|| Ok(()))
            .unwrap()
            .spans()
            .is_empty()
    );
    assert!(
        zero.resolve(clip, &budget, &|| Ok(()))
            .unwrap()
            .spans()
            .is_empty()
    );
    let predicted = Sample {
        provenance: Provenance::Predicted,
        ..sample(1, Phase::Down, None)
    };
    assert!(PenStroke::begin(style(MaskMode::SourceOver), predicted, &budget).is_err());
    let small = SharedBudget::new((std::mem::size_of::<Sample>() * 64) as u64);
    let mut stroke = PenStroke::begin(
        style(MaskMode::SourceOver),
        sample(1, Phase::Down, None),
        &small,
    )
    .unwrap();
    let snapshot = stroke.clone();
    let retained = small.usage().reserved;
    assert_eq!(
        stroke
            .append(sample(2, Phase::Move, Some(0.5)))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(stroke.samples(), snapshot.samples());
    assert_eq!(small.usage().reserved, retained);
    drop((stroke, snapshot));
    assert_eq!(small.usage().reserved, 0);
}

#[test]
fn preview_cow_does_not_grow_capacity_until_the_captured_buffer_is_full() {
    let budget = SharedBudget::new(1 << 20);
    let mut stroke = PenStroke::begin(
        style(MaskMode::SourceOver),
        sample(1, Phase::Down, None),
        &budget,
    )
    .unwrap();
    let bytes = stroke.declared_bytes();
    for sequence in 2..=24 {
        let snapshot = stroke.clone();
        stroke
            .append(sample(sequence, Phase::Move, Some(0.5)))
            .unwrap();
        assert_eq!(stroke.declared_bytes(), bytes);
        assert_eq!(snapshot.samples().len() as u64, sequence - 1);
        assert_eq!(stroke.samples().len() as u64, sequence);
    }
    assert!(budget.usage().peak <= bytes * 2);
}

#[test]
fn ordinary_two_second_240hz_fat_stroke_has_bounded_measured_logical_peak() {
    let budget = SharedBudget::new(16 << 20);
    let mut initial = sample(1, Phase::Down, Some(1.0));
    initial.position = (50.0, 64.0);
    initial.time.micros = 0;
    let mut captured = style(MaskMode::SourceOver);
    captured.diameter = 100;
    let mut stroke = PenStroke::begin(captured, initial, &budget).unwrap();
    for i in 1..480 {
        let mut next = sample(
            i + 1,
            if i == 479 { Phase::Up } else { Phase::Move },
            Some(1.0),
        );
        next.position = (50.0 + i as f64 * 0.25, 64.0);
        next.time.micros = i * 1_000_000 / 240;
        stroke.append(next).unwrap();
    }
    let resolved = stroke
        .resolve(Rect::new(0, 0, 256, 128).unwrap(), &budget, &|| Ok(()))
        .unwrap();
    assert_eq!(stroke.samples().len(), 480);
    assert!(!resolved.spans().is_empty());
    assert!(budget.usage().peak < 10 << 20);
    eprintln!(
        "480 actual samples, diameter100: declared peak={} bytes, retained={} bytes, spans={}",
        budget.usage().peak,
        budget.usage().reserved,
        resolved.spans().len()
    );
    drop((stroke, resolved));
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn geometry_only_duplicate_coalescing_preserves_every_raw_tilt_and_timestamp_sample() {
    let budget = SharedBudget::new(1 << 20);
    let first = sample(1, Phase::Down, Some(1.0));
    let mut stroke = PenStroke::begin(style(MaskMode::SourceOver), first, &budget).unwrap();
    for i in 2..=100 {
        let mut next = sample(i, if i == 100 { Phase::Up } else { Phase::Move }, None);
        next.position = first.position;
        next.twist_degrees = Some(i as f32);
        stroke.append(next).unwrap();
    }
    let clip = Rect::new(0, 0, 32, 32).unwrap();
    let resolved = stroke.resolve(clip, &budget, &|| Ok(())).unwrap();
    let single = PenStroke::begin(style(MaskMode::SourceOver), first, &budget)
        .unwrap()
        .resolve(clip, &budget, &|| Ok(()))
        .unwrap();
    assert_eq!(resolved.spans(), single.spans());
    assert_eq!(stroke.samples().len(), 100);
    assert_eq!(stroke.samples()[99].twist_degrees, Some(100.0));
}

#[test]
fn ten_second_240hz_fat_stroke_keeps_full_samples_within_64mib_logical_budget() {
    let budget = SharedBudget::new(64 << 20);
    let mut first = sample(1, Phase::Down, Some(1.0));
    first.position = (50.0, 64.0);
    first.time.micros = 0;
    let mut captured = style(MaskMode::SourceOver);
    captured.diameter = 100;
    let mut stroke = PenStroke::begin(captured, first, &budget).unwrap();
    for i in 1..2400 {
        let mut next = sample(
            i + 1,
            if i == 2399 { Phase::Up } else { Phase::Move },
            Some(1.0),
        );
        next.position = (50.0 + i as f64 * 0.25, 64.0);
        next.time.micros = i * 1_000_000 / 240;
        stroke.append(next).unwrap();
    }
    let resolved = stroke
        .resolve(Rect::new(0, 0, 768, 128).unwrap(), &budget, &|| Ok(()))
        .unwrap();
    assert_eq!(stroke.samples().len(), 2400);
    assert!(budget.usage().peak < 48 << 20);
    eprintln!(
        "2400 actual samples, diameter100: declared peak={} bytes, retained={} bytes, spans={}",
        budget.usage().peak,
        budget.usage().reserved,
        resolved.spans().len()
    );
    drop((stroke, resolved));
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn late_mask_capacity_denial_discards_its_private_prefix_and_retains_full_source() {
    let source = SharedBudget::new(1 << 20);
    let mut first = sample(1, Phase::Down, Some(0.5));
    first.position = (20.0, 20.0);
    let mut captured = style(MaskMode::SourceOver);
    captured.diameter = 40;
    let mut stroke = PenStroke::begin(captured, first, &source).unwrap();
    let mut end = sample(2, Phase::Up, Some(1.0));
    end.position = (50.0, 35.0);
    stroke.append(end).unwrap();
    let before = stroke.samples().to_vec();
    let clip = Rect::new(0, 0, 96, 64).unwrap();
    let enough = SharedBudget::new(16 << 20);
    let resolved = stroke.resolve(clip, &enough, &|| Ok(())).unwrap();
    assert!(resolved.spans().len() > 32);
    let peak = enough.usage().peak;
    let insufficient = SharedBudget::new(peak - 1);
    assert_eq!(
        stroke
            .resolve(clip, &insufficient, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(insufficient.usage().peak > 0); // scratch was admitted before output growth failed
    assert_eq!(insufficient.usage().reserved, 0);
    assert_eq!(stroke.samples(), before);
    drop((stroke, resolved));
    assert_eq!(source.usage().reserved, 0);
    assert_eq!(enough.usage().reserved, 0);
}
