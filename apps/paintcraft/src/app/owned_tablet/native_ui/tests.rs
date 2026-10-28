use super::*;
use decode::Decoder;
use targets::{Binding, Target};
fn at(ordinal: u64, fact: NativeUi) -> Observation {
    Observation {
        installation: 9,
        ordinal,
        fact,
    }
}
fn character(ordinal: u64, value: u32) -> Observation {
    at(
        ordinal,
        NativeUi::WindowsCharacter(paintcraft_platform::tablet::WindowsCharacter {
            kind: WindowsCharacterKind::Utf16,
            value,
            repeat_count: 1,
            system_character: false,
        }),
    )
}
#[test]
fn bad_suffix_never_exposes_prefix_or_mutates_decoder() {
    let mut decoder = Decoder::default();
    let before = decoder;
    assert_eq!(
        decoder.segment(&[character(1, 65), character(2, 0xdc00)]),
        Err(Fault::Text)
    );
    assert_eq!(decoder, before);
    assert_eq!(decoder.segment(&[character(1, 66)]).unwrap().len(), 1);
}
#[test]
fn surrogate_evidence_keeps_both_observations_across_segments() {
    let mut decoder = Decoder::default();
    let high = character(4, 0xd83d);
    let low = character(11, 0xde00);
    assert!(decoder.segment(&[high]).unwrap().is_empty());
    let before = decoder;
    assert_eq!(
        decoder.segment(&[at(
            8,
            NativeUi::WindowsFocus {
                focused: false,
                modifiers: Default::default()
            }
        )]),
        Err(Fault::Text)
    );
    assert_eq!(decoder, before);
    let result = decoder.segment(&[low]).unwrap();
    assert_eq!(
        result[0].evidence,
        Evidence {
            first: high,
            last: low
        }
    );
    assert_eq!(
        result[0].value,
        Value::Text {
            scalar: '😀',
            repeat: 1,
            system: false
        }
    );
}
#[test]
fn unresolved_tail_cannot_release_same_segment_prefix() {
    let mut decoder = Decoder::default();
    assert_eq!(
        decoder.segment(&[character(1, 65), character(2, 0xd800)]),
        Err(Fault::Order)
    );
    assert_eq!(decoder, Decoder::default());
}
#[test]
fn frame_is_complete_and_validated_before_output() {
    let mut decoder = Decoder::default();
    let start = at(
        2,
        NativeUi::ImeFrameStart {
            composition: 1,
            component: ImeComponent::PreeditUtf16,
            count: 1,
            cursor_native: Some(1),
            native_flags: 8,
        },
    );
    assert_eq!(
        decoder.segment(&[
            at(1, NativeUi::ImeBegin { composition: 1 }),
            start,
            at(
                3,
                NativeUi::ImeData {
                    index: 0,
                    value: 65
                }
            )
        ]),
        Err(Fault::Order)
    );
    assert_eq!(decoder, Decoder::default());
    let output = decoder
        .segment(&[
            at(1, NativeUi::ImeBegin { composition: 1 }),
            start,
            at(
                3,
                NativeUi::ImeData {
                    index: 0,
                    value: 65,
                },
            ),
            at(4, NativeUi::ImeFrameEnd { composition: 1 }),
        ])
        .unwrap();
    assert!(matches!(&output[1].value, Value::Component { text: Some(text), .. } if text == "A"));
}
#[test]
fn aggregate_repeat_budget_and_installation_are_transactional() {
    let mut decoder = Decoder::default();
    let mut repeated = character(1, 65);
    if let NativeUi::WindowsCharacter(ref mut c) = repeated.fact {
        c.repeat_count = 4096;
    }
    assert_eq!(
        decoder.segment(&[repeated, character(2, 66)]),
        Err(Fault::Capacity)
    );
    assert_eq!(decoder, Decoder::default());
    decoder.segment(&[repeated]).unwrap();
    let mut wrong = character(2, 66);
    wrong.installation = 8;
    assert_eq!(decoder.segment(&[wrong]), Err(Fault::Order));
}
fn target() -> Target {
    Target {
        widget: eframe::egui::Id::unique("target"),
        context: 77,
        incarnation: 1,
        session: 2,
        interface: 3,
        modal_generation: 4,
        window_generation: 5,
    }
}
#[test]
fn begin_binding_cannot_be_reselected_at_result_or_after_revoke() {
    let mut state = Binding::new(9).unwrap();
    state.begin(9, 1).unwrap();
    assert_eq!(state.result(9, 1, target()), Err(Fault::Target));
    state.bind(target()).unwrap();
    let mut other = target();
    other.incarnation += 1;
    assert_eq!(state.result(9, 1, other), Err(Fault::Target));
    assert_eq!(state.bind(other), Err(Fault::Target));
    state.end(9, 1).unwrap();
    assert_eq!(state.result(9, 1, target()), Ok(target()));
    assert_eq!(state.preedit(9, 1, target()), Err(Fault::Composition));
    state.revoke();
    assert_eq!(state.result(9, 1, target()), Err(Fault::Target));
    assert_eq!(state.bind(target()), Err(Fault::Target));
    state.begin(9, 2).unwrap();
    state.bind(other).unwrap();
    assert_eq!(state.result(9, 1, target()), Err(Fault::Composition));
}

#[test]
fn key_and_unicode_expansion_boundaries_are_exact() {
    use paintcraft_platform::tablet::{WindowsCharacter, WindowsKey};
    let key = WindowsKey {
        virtual_key: 65,
        scan_code: 30,
        extended: false,
        pressed: true,
        repeat_count: 4096,
        was_down: true,
        system_key: false,
        alt_context: false,
        keyboard_layout: 1,
        modifiers: Default::default(),
    };
    assert!(
        Decoder::default()
            .segment(&[at(1, NativeUi::WindowsKey(key))])
            .is_ok()
    );
    let mut decoder = Decoder::default();
    assert_eq!(
        decoder.segment(&[at(
            1,
            NativeUi::WindowsKey(WindowsKey {
                repeat_count: 4097,
                ..key
            })
        )]),
        Err(Fault::Capacity)
    );
    assert_eq!(decoder, Decoder::default());
    let unicode = WindowsCharacter {
        kind: WindowsCharacterKind::Unichar,
        value: 0x1f600,
        repeat_count: 2048,
        system_character: false,
    };
    assert!(
        decoder
            .segment(&[at(1, NativeUi::WindowsCharacter(unicode))])
            .is_ok()
    );
    let before = decoder;
    assert_eq!(
        decoder.segment(&[at(
            2,
            NativeUi::WindowsCharacter(WindowsCharacter {
                repeat_count: 2049,
                ..unicode
            })
        )]),
        Err(Fault::Capacity)
    );
    assert_eq!(decoder, before);
}

#[test]
fn native_cancel_end_and_new_composition_are_not_interchangeable() {
    let mut decoder = Decoder::default();
    decoder
        .segment(&[
            at(1, NativeUi::ImeBegin { composition: 1 }),
            at(2, NativeUi::ImeEnd { composition: 1 }),
        ])
        .unwrap();
    let result = [
        at(
            3,
            NativeUi::ImeFrameStart {
                composition: 1,
                component: ImeComponent::ResultUtf16,
                count: 0,
                cursor_native: None,
                native_flags: 1,
            },
        ),
        at(4, NativeUi::ImeFrameEnd { composition: 1 }),
    ];
    assert!(decoder.segment(&result).is_ok());
    decoder
        .segment(&[at(5, NativeUi::ImeNativeCancel { composition: 1 })])
        .unwrap();
    let before = decoder;
    let mut late = result;
    late[0].ordinal = 6;
    late[1].ordinal = 7;
    assert_eq!(decoder.segment(&late), Err(Fault::Composition));
    assert_eq!(decoder, before);
    decoder
        .segment(&[at(6, NativeUi::ImeBegin { composition: 2 })])
        .unwrap();
    late[0].ordinal = 7;
    late[1].ordinal = 8;
    assert_eq!(decoder.segment(&late), Err(Fault::Composition));
}

#[test]
fn target_context_and_source_installation_cannot_be_borrowed() {
    let mut binding = Binding::new(9).unwrap();
    binding.begin(9, 1).unwrap();
    binding.bind(target()).unwrap();
    let mut changed = target();
    changed.context += 1;
    assert_eq!(binding.result(9, 1, changed), Err(Fault::Target));
    assert_eq!(binding.result(10, 1, target()), Err(Fault::Composition));
    let before = binding;
    assert_eq!(binding.begin(10, 2), Err(Fault::Composition));
    assert_eq!(binding, before);
    assert_eq!(binding.begin(9, 1), Err(Fault::Composition));
}

#[test]
fn first_zero_is_real_and_duplicate_zero_is_transactional() {
    let mut decoder = Decoder::default();
    let first = character(0, 65);
    let decoded = decoder.segment(&[first]).unwrap();
    assert_eq!(decoded[0].evidence, Evidence { first, last: first });
    let before = decoder;
    assert_eq!(decoder.segment(&[first]), Err(Fault::Order));
    assert_eq!(decoder, before);
    assert!(decoder.segment(&[character(1, 66)]).is_ok());
    let mut fresh = Decoder::default();
    assert_eq!(fresh.segment(&[first, first]), Err(Fault::Order));
    assert_eq!(fresh, Decoder::default());
}

#[test]
fn checked_namespace_sentinels_refuse_without_state_mutation() {
    let first = character(0, 65);
    for bad in [
        Observation {
            installation: u64::MAX,
            ..character(1, 66)
        },
        character(u64::MAX, 66),
        at(
            1,
            NativeUi::ImeBegin {
                composition: u64::MAX,
            },
        ),
    ] {
        let mut decoder = Decoder::default();
        assert!(decoder.segment(&[first, bad]).is_err());
        assert_eq!(decoder, Decoder::default());
        decoder.segment(&[first]).unwrap();
        let before = decoder;
        assert!(decoder.segment(&[bad]).is_err());
        assert_eq!(decoder, before);
    }
    assert_eq!(Binding::new(u64::MAX), Err(Fault::Order));
    let mut binding = Binding::new(9).unwrap();
    for (installation, composition) in [(u64::MAX, 1), (9, u64::MAX)] {
        let before = binding;
        assert_eq!(
            binding.begin(installation, composition),
            Err(Fault::Composition)
        );
        assert_eq!(binding, before);
    }
}
