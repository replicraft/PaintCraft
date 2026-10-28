use paintcraft_core::types::{Rgba, brush::BrushMedia, resources::SharedBudget};
use paintcraft_input::{Modifiers, PointerButton, tablet::*};
use paintcraft_tools::{Action, Preview, Tool, ToolState};
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
        position: (sequence as f64 * 3.0, 8.0),
        transform: Transform {
            owner: 3,
            epoch: 4,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        buttons: Buttons(Buttons::TIP),
        modifiers: Modifiers::default(),
        pressure,
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
#[test]
fn pressure_profile_is_explicit_and_captures_opacity_color_and_source_button() {
    let mut tools = ToolState::default();
    tools.tool = Tool::Brush;
    tools.color = Rgba([1, 2, 3, 200]);
    tools.secondary = Rgba([11, 22, 33, 150]);
    tools.options.brush_opacity = 40;
    tools.options.size = 12;
    tools.options.pen.curve = paintcraft_core::types::pen::PressureCurve::Firm;
    let mut down = sample(1, Phase::Down, Some(0.2));
    down.buttons = Buttons(Buttons::TIP | Buttons::BARREL_UPPER);
    assert!(
        tools
            .begin_pen(Uuid::from_u128(1), down, &SharedBudget::new(16 << 20))
            .unwrap()
    );
    let Some(Preview::PenStroke(prefix)) = tools.preview() else {
        panic!()
    };
    assert_eq!(prefix.style().color, Rgba([11, 22, 33, 60]));
    assert_eq!(prefix.style().diameter, 12);
    assert_eq!(prefix.style().settings, tools.options.pen);
    tools.color = Rgba([255; 4]);
    tools.options.size = 1;
    tools.options.brush_opacity = 1;
    tools.options.pen.size_pressure = false;
    tools.options.pen.curve = paintcraft_core::types::pen::PressureCurve::Soft;
    assert!(tools.pointer_up_button(PointerButton::Secondary).is_none());
    assert!(tools.pen_in_progress());
    let Some((_, Action::PenStroke(released))) =
        tools.append_pen(sample(2, Phase::Up, Some(0.8))).unwrap()
    else {
        panic!()
    };
    assert_eq!(released.style(), prefix.style());
    assert!(!tools.pen_in_progress());
}
#[test]
fn missing_pressure_keeps_all_nine_original_media_actions_exact() {
    let layer = Uuid::from_u128(1);
    for media in BrushMedia::ALL {
        let mut actual = ToolState::default();
        actual.tool = Tool::Brush;
        let mut reference = ToolState::default();
        reference.tool = Tool::Brush;
        actual.options.media = media;
        reference.options.media = media;
        assert!(
            !actual
                .begin_pen(
                    layer,
                    sample(1, Phase::Down, None),
                    &SharedBudget::new(1 << 20)
                )
                .unwrap()
        );
        for tools in [&mut actual, &mut reference] {
            tools.pointer_down_subpixel(layer, (512, 768), PointerButton::Primary);
            tools.pointer_move_subpixel((2048, 2560));
        }
        assert_eq!(actual.pointer_up(), reference.pointer_up(), "{media:?}");
    }
}
#[test]
fn cancellation_and_new_text_draft_retire_pen_without_committing_or_replacing_drafts() {
    let mut tools = ToolState::default();
    tools.tool = Tool::Brush;
    let budget = SharedBudget::new(1 << 20);
    let layer = Uuid::from_u128(1);
    tools
        .begin_pen(layer, sample(1, Phase::Down, Some(0.5)), &budget)
        .unwrap();
    tools.cancel_gesture();
    assert!(!tools.pen_in_progress());
    assert!(
        tools
            .append_pen(sample(2, Phase::Up, None))
            .unwrap()
            .is_none()
    );
    tools.begin_text(layer, (1, 2));
    assert!(
        tools
            .begin_pen(layer, sample(3, Phase::Down, Some(0.5)), &budget)
            .is_err()
    );
    assert!(tools.is_editing_text());
    tools.cancel_text();
    tools
        .begin_pen(layer, sample(4, Phase::Down, Some(0.5)), &budget)
        .unwrap();
    tools.begin_text(layer, (3, 4));
    assert!(!tools.pen_in_progress());
    assert!(tools.is_editing_text());
}

#[test]
fn inverted_end_uses_captured_erase_policy_without_replacing_selected_media() {
    use paintcraft_core::types::policy::{MaskIntent, MaskMode};
    for (eraser_color, mode) in [
        (Rgba::TRANSPARENT, MaskMode::DestinationOut),
        (Rgba([251, 247, 239, 255]), MaskMode::Replace),
    ] {
        let mut tools = ToolState::default();
        tools.tool = Tool::Brush;
        tools.options.media = BrushMedia::ALL[1];
        tools.options.eraser_color = eraser_color;
        let captured_options = tools.options;
        let mut down = sample(1, Phase::Down, None);
        down.identity.kind = DeviceKind::Eraser;
        down.buttons = Buttons(Buttons::ERASER | Buttons::BARREL);
        assert!(
            tools
                .begin_pen(Uuid::from_u128(1), down, &SharedBudget::new(16 << 20))
                .unwrap()
        );
        assert_eq!(tools.gesture_button(), Some(PointerButton::Primary));
        let Some(Preview::PenStroke(stroke)) = tools.preview() else {
            panic!()
        };
        assert_eq!(stroke.style().intent, MaskIntent::Erase);
        assert_eq!(stroke.style().mode, mode);
        assert_eq!(stroke.samples()[0].pressure, None);
        assert_eq!(
            stroke.style().color,
            if mode == MaskMode::DestinationOut {
                Rgba([0, 0, 0, 255])
            } else {
                eraser_color
            }
        );
        tools.cancel_gesture();
        assert_eq!(tools.tool, Tool::Brush);
        assert_eq!(tools.options, captured_options);
    }
}
