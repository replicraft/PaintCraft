use paintcraft_core::types::{
    ErrorCode, Rect, Rgba,
    brush::BrushMedia,
    pen::{Orientation, PressureCurve},
    resources::SharedBudget,
};
use paintcraft_input::{Modifiers, PointerButton, tablet::*};
use paintcraft_tools::{
    Action, Preview, Tool, ToolState,
    pen_stroke::{PenStroke, Style},
};
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
            clock: Clock::HostReceipt,
        },
        phase,
        position: (16.0 + sequence as f64 * 6.0, 24.0),
        transform: Transform {
            owner: 3,
            epoch: 4,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        buttons: Buttons(Buttons::TIP),
        modifiers: Modifiers::default(),
        pressure,
        tilt: Some(Tilt::Degrees(30.0, 15.0)),
        twist_degrees: Some(80.0),
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn tools(media: BrushMedia) -> ToolState {
    let mut tools = ToolState::default();
    tools.tool = Tool::Brush;
    tools.options.media = media;
    tools.options.size = 24;
    tools.color = Rgba([13, 47, 179, 190]);
    tools.options.brush_opacity = 40;
    tools
}
fn stroke(
    media: BrushMedia,
    orientation: Orientation,
    tilt: Option<Tilt>,
    twist: Option<f32>,
) -> PenStroke {
    let mut tools = tools(media);
    tools.options.pen.media_dynamics = true;
    tools.options.pen.orientation = orientation;
    tools.options.pen.size_pressure = false;
    let mut down = sample(1, Phase::Down, Some(1.0));
    down.position = (32.0, 32.0);
    down.tilt = tilt;
    down.twist_degrees = twist;
    assert!(
        tools
            .begin_pen(Uuid::from_u128(1), down, &SharedBudget::new(8 << 20))
            .unwrap()
    );
    let Some(Preview::PenStroke(stroke)) = tools.preview() else {
        panic!()
    };
    stroke
}

#[test]
fn historical_textured_tools_keep_the_exact_legacy_action_even_when_pressure_is_reported() {
    let layer = Uuid::from_u128(1);
    for media in BrushMedia::ALL.into_iter().skip(1) {
        let mut actual = tools(media);
        let mut reference = tools(media);
        assert!(
            !actual
                .begin_pen(
                    layer,
                    sample(1, Phase::Down, Some(0.2)),
                    &SharedBudget::new(8 << 20)
                )
                .unwrap()
        );
        for tools in [&mut actual, &mut reference] {
            tools.pointer_down_subpixel(layer, (12 * 256, 20 * 256), PointerButton::Primary);
            tools.pointer_move_subpixel((54 * 256, 31 * 256));
        }
        assert_eq!(actual.pointer_up(), reference.pointer_up(), "{media:?}");
    }
}

#[test]
fn opacity_switch_modulates_the_captured_cap_independently_of_size_and_color_alpha() {
    let clip = Rect::new(0, 0, 64, 64).unwrap();
    let budget = SharedBudget::new(32 << 20);
    for media in BrushMedia::ALL {
        let mut tools = tools(media);
        tools.options.pen.media_dynamics = true;
        tools.options.pen.size_pressure = false;
        tools.options.pen.opacity_pressure = true;
        let mut down = sample(1, Phase::Down, Some(0.25));
        down.position = (32.0, 32.0);
        assert!(tools.begin_pen(Uuid::from_u128(1), down, &budget).unwrap());
        let Some(Preview::PenStroke(quarter)) = tools.preview() else {
            panic!()
        };
        assert_eq!(quarter.style().color.a(), 76);
        let mut full_style = quarter.style();
        // Keep the same explicit dab policy, changing only actual source value.
        let mut full_down = down;
        full_down.pressure = Some(1.0);
        let full = PenStroke::begin(full_style, full_down, &budget).unwrap();
        let to_pixels = |stroke: &PenStroke| {
            stroke
                .resolve(clip, &budget, &|| Ok(()))
                .unwrap()
                .spans()
                .iter()
                .flat_map(|span| (span.x0..=span.x1).map(move |x| ((x, span.y), span.coverage)))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let expected = to_pixels(&full)
            .into_iter()
            .map(|(point, coverage)| (point, ((u32::from(coverage) * 16384 + 32767) / 65535) as u8))
            .filter(|(_, coverage)| *coverage != 0)
            .collect();
        assert_eq!(to_pixels(&quarter), expected, "{media:?}");
        full_style.settings.opacity_pressure = false;
        let disabled = PenStroke::begin(full_style, down, &budget).unwrap();
        // Historical Plain uses its exact swept resolver rather than the
        // explicitly opted-in capped-dab policy; textured tips share the walker.
        if media != BrushMedia::Plain {
            assert_eq!(to_pixels(&disabled), to_pixels(&full), "{media:?}");
        }
    }
}

#[test]
fn every_opted_in_medium_keeps_its_own_tip_and_captured_preview_release_opacity() {
    let budget = SharedBudget::new(32 << 20);
    let clip = Rect::new(0, 0, 80, 64).unwrap();
    let mut masks = Vec::new();
    for media in BrushMedia::ALL {
        let mut tools = tools(media);
        tools.options.pen.media_dynamics = true;
        tools.options.pen.opacity_pressure = true;
        tools.options.pen.curve = PressureCurve::Soft;
        assert!(
            tools
                .begin_pen(
                    Uuid::from_u128(1),
                    sample(1, Phase::Down, Some(0.2)),
                    &budget
                )
                .unwrap()
        );
        let last = sample(2, Phase::Move, Some(0.8));
        assert!(tools.append_pen(last).unwrap().is_none());
        let Some(Preview::PenStroke(preview)) = tools.preview() else {
            panic!()
        };
        tools.options.media = BrushMedia::Oil;
        tools.options.pen.opacity_pressure = false;
        tools.options.brush_opacity = 100;
        let mut up = last;
        up.phase = Phase::Up;
        up.sequence = 3;
        up.time.micros = 300;
        let Some((_, Action::PenStroke(released))) = tools.append_pen(up).unwrap() else {
            panic!()
        };
        assert_eq!(preview.style(), released.style());
        assert_eq!(released.style().media, media);
        assert_eq!(released.style().color.a(), 76);
        let before = preview.resolve(clip, &budget, &|| Ok(())).unwrap();
        let after = released.resolve(clip, &budget, &|| Ok(())).unwrap();
        assert_eq!(
            before.operations(&|| Ok(())).unwrap(),
            after.operations(&|| Ok(())).unwrap()
        );
        assert!(!after.spans().is_empty());
        assert!(
            !masks.contains(&after.spans().to_vec()),
            "{media:?} was replaced by another medium"
        );
        masks.push(after.spans().to_vec());
        assert_eq!(
            released.samples(),
            &[sample(1, Phase::Down, Some(0.2)), last, up]
        );
    }
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn tilt_and_twist_rotate_only_applicable_nibs_with_missing_and_upright_fallbacks() {
    let clip = Rect::new(0, 0, 64, 64).unwrap();
    let budget = SharedBudget::new(32 << 20);
    for media in [
        BrushMedia::CalligraphyBrush,
        BrushMedia::CalligraphyPen,
        BrushMedia::Marker,
    ] {
        let mask = |orientation, tilt, twist| {
            stroke(media, orientation, tilt, twist)
                .resolve(clip, &budget, &|| Ok(()))
                .unwrap()
                .spans()
                .to_vec()
        };
        let fixed = mask(Orientation::Historical, None, None);
        assert_eq!(fixed, mask(Orientation::Tilt, None, None));
        assert_eq!(fixed, mask(Orientation::Twist, None, None));
        assert_eq!(
            fixed,
            mask(Orientation::Tilt, Some(Tilt::Degrees(0.0, 0.0)), None)
        );
        assert_eq!(
            fixed,
            mask(Orientation::Tilt, Some(Tilt::Normalized(0.0, 0.0)), None)
        );
        let twist = mask(Orientation::Twist, None, Some(90.0));
        assert_eq!(
            twist,
            mask(Orientation::Tilt, Some(Tilt::Degrees(0.0, 45.0)), None)
        );
        assert_eq!(
            twist,
            mask(Orientation::Tilt, Some(Tilt::Normalized(0.0, 0.5)), None)
        );
        assert_ne!(fixed, twist, "{media:?}");
        assert_eq!(
            twist,
            mask(Orientation::Tilt, Some(Tilt::Degrees(0.0, 1e-20)), None)
        );
        for x in [-90.0, 90.0] {
            for y in [-90.0, -30.0, 30.0, 90.0] {
                assert_eq!(
                    fixed,
                    mask(Orientation::Tilt, Some(Tilt::Degrees(x, y)), None)
                );
                assert_eq!(
                    fixed,
                    mask(Orientation::Tilt, Some(Tilt::Degrees(y, x)), None)
                );
            }
        }
        // Unequal components distinguish projected-angle normalization from
        // interpreting the raw normalized pair as an azimuth vector.
        assert_eq!(
            mask(Orientation::Tilt, Some(Tilt::Normalized(0.5, 0.25)), None),
            mask(Orientation::Tilt, Some(Tilt::Degrees(45.0, 22.5)), None),
            "{media:?} normalized quarter-turn conversion"
        );
    }
    let media = BrushMedia::Oil;
    assert_eq!(
        stroke(media, Orientation::Historical, None, None)
            .resolve(clip, &budget, &|| Ok(()))
            .unwrap()
            .spans(),
        stroke(media, Orientation::Twist, None, Some(90.0))
            .resolve(clip, &budget, &|| Ok(()))
            .unwrap()
            .spans()
    );
}

#[test]
fn dynamic_mapping_admission_and_cancel_preserve_the_raw_capture_and_release_all_scratch() {
    let stroke = stroke(
        BrushMedia::Marker,
        Orientation::Tilt,
        Some(Tilt::Degrees(20.0, 35.0)),
        None,
    );
    let original = stroke.samples().to_vec();
    let clip = Rect::new(0, 0, 64, 64).unwrap();
    let small = SharedBudget::new(1);
    assert_eq!(
        stroke.resolve(clip, &small, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(small.usage().reserved, 0);
    let budget = SharedBudget::new(8 << 20);
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        stroke
            .resolve(clip, &budget, &|| {
                calls.set(calls.get() + 1);
                if calls.get() > 8 {
                    Err(paintcraft_core::types::Error::new(
                        ErrorCode::OperationCancelled,
                        "stop",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(stroke.samples(), original);
}

#[test]
fn direct_textured_style_requires_opt_in_and_source_over_while_eraser_stays_plain() {
    use paintcraft_core::types::policy::{MaskIntent, MaskMode};
    let style = Style {
        layer: Uuid::from_u128(1),
        diameter: 16,
        color: Rgba([255; 4]),
        mode: MaskMode::SourceOver,
        intent: MaskIntent::Brush,
        settings: Default::default(),
        media: BrushMedia::Oil,
    };
    let budget = SharedBudget::new(8 << 20);
    assert!(PenStroke::begin(style, sample(1, Phase::Down, Some(1.0)), &budget).is_err());
    let mut tools = tools(BrushMedia::Marker);
    tools.options.pen.media_dynamics = true;
    let mut down = sample(1, Phase::Down, Some(0.5));
    down.identity.kind = DeviceKind::Eraser;
    assert!(tools.begin_pen(Uuid::from_u128(1), down, &budget).unwrap());
    let Some(Preview::PenStroke(eraser)) = tools.preview() else {
        panic!()
    };
    assert_eq!(eraser.style().media, BrushMedia::Plain);
    assert_eq!(eraser.style().intent, MaskIntent::Erase);
}

#[test]
fn low_pressure_two_and_ten_second_240hz_traces_keep_full_capture_with_bounded_logical_peak() {
    use paintcraft_core::types::policy::{MaskIntent, MaskMode};
    for media in [
        BrushMedia::Plain,
        BrushMedia::CalligraphyBrush,
        BrushMedia::CalligraphyPen,
        BrushMedia::Marker,
    ] {
        for count in [480, 2400] {
            let budget = SharedBudget::new(16 << 20);
            let captured = Style {
                layer: Uuid::from_u128(1),
                diameter: 100,
                color: Rgba([0, 0, 0, 255]),
                mode: MaskMode::SourceOver,
                intent: MaskIntent::Brush,
                settings: paintcraft_core::types::pen::Settings {
                    opacity_pressure: true,
                    media_dynamics: true,
                    ..Default::default()
                },
                media,
            };
            let mut first = sample(1, Phase::Down, Some(0.01));
            first.position = (50.0, 64.0);
            first.time.micros = 0;
            let mut stroke = PenStroke::begin(captured, first, &budget).unwrap();
            for i in 1..count {
                let mut next = sample(
                    i + 1,
                    if i == count - 1 {
                        Phase::Up
                    } else {
                        Phase::Move
                    },
                    Some(0.01),
                );
                next.position = (50.0 + i as f64 * 0.25, 64.0);
                next.time.micros = i * 1_000_000 / 240;
                stroke.append(next).unwrap();
            }
            let resolved = stroke
                .resolve(Rect::new(0, 0, 768, 128).unwrap(), &budget, &|| Ok(()))
                .unwrap();
            assert_eq!(stroke.samples().len(), count as usize);
            assert!(!resolved.spans().is_empty(), "{media:?} {count}");
            assert!(budget.usage().peak < 16 << 20);
            eprintln!(
                "low pressure0.01 {media:?} size100 {count}actual samples: logical_peak={} retained={} spans={}",
                budget.usage().peak,
                budget.usage().reserved,
                resolved.spans().len()
            );
            drop((stroke, resolved));
            assert_eq!(budget.usage().reserved, 0);
        }
    }
}
