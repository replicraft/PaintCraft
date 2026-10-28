use super::*;
use keys::{Disposition, KeyState};
use paintcraft_platform::tablet::WindowsKey;
fn raw() -> WindowsKey {
    WindowsKey {
        virtual_key: 65,
        scan_code: 30,
        extended: false,
        pressed: true,
        repeat_count: 1,
        was_down: false,
        system_key: false,
        alt_context: false,
        keyboard_layout: 1,
        modifiers: Default::default(),
    }
}
fn fact(ordinal: u64, fact: NativeUi) -> Decoded {
    let observation = Observation {
        installation: 9,
        ordinal,
        fact,
    };
    Decoded {
        evidence: Evidence {
            first: observation,
            last: observation,
        },
        value: Value::Fact(fact),
    }
}
fn key(ordinal: u64, raw: WindowsKey) -> Decoded {
    fact(ordinal, NativeUi::WindowsKey(raw))
}
fn focused() -> KeyState {
    let mut state = KeyState::default();
    state
        .segment(&[fact(
            0,
            NativeUi::WindowsFocus {
                focused: true,
                modifiers: Default::default(),
            },
        )])
        .unwrap();
    state
}
#[test]
fn down_repeat_release_preserve_raw_identity_and_windows_modifiers() {
    let mut state = focused();
    let mut control = raw();
    control.modifiers.control = true;
    let plan = state
        .segment(&[
            key(1, control),
            key(
                2,
                WindowsKey {
                    repeat_count: 2,
                    was_down: true,
                    ..control
                },
            ),
            key(
                3,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    ..control
                },
            ),
        ])
        .unwrap();
    assert_eq!(plan.output_cost, 4);
    assert_eq!(plan.keys.len(), 4);
    for (i, item) in plan.keys.iter().enumerate() {
        let Disposition::Key {
            key,
            pressed,
            repeat,
            modifiers,
        } = item.disposition
        else {
            panic!()
        };
        assert_eq!(key, eframe::egui::Key::A);
        assert_eq!(pressed, i != 3);
        assert_eq!(repeat, i == 1 || i == 2);
        assert!(modifiers.ctrl && modifiers.command && !modifiers.mac_cmd);
        assert!(matches!(item.evidence.first.fact, NativeUi::WindowsKey(k) if k.modifiers.control));
    }
}
#[test]
fn repeated_and_conflicting_downs_refuse_without_corrupting_ledger() {
    let mut state = focused();
    state.segment(&[key(1, raw())]).unwrap();
    for bad in [
        raw(),
        WindowsKey {
            was_down: true,
            virtual_key: 66,
            ..raw()
        },
        WindowsKey {
            was_down: true,
            keyboard_layout: 2,
            ..raw()
        },
    ] {
        let before = state.clone();
        assert_eq!(state.segment(&[key(2, bad)]).unwrap_err(), Fault::Order);
        assert_eq!(state, before);
    }
    assert!(
        state
            .segment(&[key(
                2,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    ..raw()
                }
            )])
            .is_ok()
    );
}
#[test]
fn process_key_release_uses_exact_physical_identity_never_text_join() {
    let mut state = focused();
    assert_eq!(
        state
            .segment(&[key(
                1,
                WindowsKey {
                    virtual_key: 0xe5,
                    ..raw()
                }
            )])
            .unwrap()
            .keys[0]
            .disposition,
        Disposition::ImeOwned
    );
    assert_eq!(
        state
            .segment(&[key(
                2,
                WindowsKey {
                    scan_code: 31,
                    pressed: false,
                    was_down: true,
                    ..raw()
                }
            )])
            .unwrap()
            .keys[0]
            .disposition,
        Disposition::Orphan
    );
    assert_eq!(
        state
            .segment(&[key(
                3,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    ..raw()
                }
            )])
            .unwrap()
            .keys[0]
            .disposition,
        Disposition::ImeOwned
    );
    assert_eq!(
        state.segment(&[key(4, raw())]).unwrap().keys[0].disposition,
        Disposition::Key {
            key: eframe::egui::Key::A,
            pressed: true,
            repeat: false,
            modifiers: Default::default()
        }
    );
}
#[test]
fn layout_focus_and_unobserved_held_tails_never_gain_key_authority() {
    let mut state = focused();
    state.segment(&[key(1, raw())]).unwrap();
    let plan = state
        .segment(&[
            fact(2, NativeUi::WindowsLayout { keyboard_layout: 2 }),
            key(
                3,
                WindowsKey {
                    keyboard_layout: 2,
                    pressed: false,
                    was_down: true,
                    ..raw()
                },
            ),
        ])
        .unwrap();
    assert_eq!(plan.keys[0].disposition, Disposition::PolicyReset);
    assert_eq!(plan.keys[1].disposition, Disposition::Orphan);
    let mut state = focused();
    assert_eq!(
        state
            .segment(&[key(
                1,
                WindowsKey {
                    was_down: true,
                    ..raw()
                }
            )])
            .unwrap()
            .keys[0]
            .disposition,
        Disposition::Orphan
    );
    state
        .segment(&[fact(
            2,
            NativeUi::WindowsFocus {
                focused: false,
                modifiers: Default::default(),
            },
        )])
        .unwrap();
    assert_eq!(
        state
            .segment(&[key(
                3,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    ..raw()
                }
            )])
            .unwrap()
            .keys[0]
            .disposition,
        Disposition::Orphan
    );
}
#[test]
fn system_oem_and_altgr_ambiguity_are_explicit_and_not_text() {
    for (key_data, expected) in [
        (
            WindowsKey {
                system_key: true,
                ..raw()
            },
            Disposition::NativeSystem,
        ),
        (
            WindowsKey {
                virtual_key: 0xba,
                ..raw()
            },
            Disposition::Unmapped,
        ),
        (
            WindowsKey {
                modifiers: paintcraft_input::Modifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                ..raw()
            },
            Disposition::AmbiguousModifiers,
        ),
        (
            WindowsKey {
                scan_code: 0,
                ..raw()
            },
            Disposition::Orphan,
        ),
    ] {
        let mut state = focused();
        assert_eq!(
            state.segment(&[key(1, key_data)]).unwrap().keys[0].disposition,
            expected
        );
    }
}
#[test]
fn combined_decoded_text_and_keys_share_one_expansion_allowance() {
    let mut state = focused();
    let text_observation = Observation {
        installation: 9,
        ordinal: 2,
        fact: NativeUi::WindowsCharacter(paintcraft_platform::tablet::WindowsCharacter {
            kind: WindowsCharacterKind::Unichar,
            value: 65,
            repeat_count: 1,
            system_character: false,
        }),
    };
    let text = || Decoded {
        evidence: Evidence {
            first: text_observation,
            last: text_observation,
        },
        value: Value::Text {
            scalar: 'A',
            repeat: 1,
            system: false,
        },
    };
    let before = state.clone();
    assert_eq!(
        state
            .segment(&[
                key(
                    1,
                    WindowsKey {
                        repeat_count: 4096,
                        ..raw()
                    }
                ),
                text()
            ])
            .unwrap_err(),
        Fault::Capacity
    );
    assert_eq!(state, before);
    assert_eq!(
        state
            .segment(&[
                key(
                    1,
                    WindowsKey {
                        repeat_count: 4095,
                        ..raw()
                    }
                ),
                text()
            ])
            .unwrap()
            .output_cost,
        4096
    );
    let mut state = focused();
    assert_eq!(
        state
            .segment(&[key(
                1,
                WindowsKey {
                    repeat_count: 4097,
                    ..raw()
                }
            )])
            .unwrap_err(),
        Fault::Capacity
    );
}
#[test]
fn bad_suffix_and_reserved_namespace_cannot_publish_a_prefix() {
    let mut state = focused();
    let mut bad = key(2, raw());
    bad.evidence.first.installation = u64::MAX;
    bad.evidence.last.installation = u64::MAX;
    let before = state.clone();
    assert_eq!(
        state.segment(&[key(1, raw()), bad]).unwrap_err(),
        Fault::Order
    );
    assert_eq!(state, before);
    assert_eq!(
        state.segment(&[key(u64::MAX, raw())]).unwrap_err(),
        Fault::Order
    );
    assert_eq!(state, before);
}

#[test]
fn actual_decoder_outputs_and_key_plans_use_a_single_cost_limit() {
    let mut decoder = decode::Decoder::default();
    let mut state = KeyState::default();
    let focus = Observation {
        installation: 9,
        ordinal: 0,
        fact: NativeUi::WindowsFocus {
            focused: true,
            modifiers: Default::default(),
        },
    };
    let pressed = Observation {
        installation: 9,
        ordinal: 1,
        fact: NativeUi::WindowsKey(WindowsKey {
            repeat_count: 4095,
            ..raw()
        }),
    };
    let character = Observation {
        installation: 9,
        ordinal: 2,
        fact: NativeUi::WindowsCharacter(paintcraft_platform::tablet::WindowsCharacter {
            kind: WindowsCharacterKind::Unichar,
            value: 65,
            repeat_count: 1,
            system_character: false,
        }),
    };
    // Decoder admits the 4096 repeated units. Planner also accounts the focus
    // control envelope and refuses the whole segment without ledger mutation.
    let before = state.clone();
    let decoder_before = decoder;
    assert_eq!(
        state
            .decode_segment(&mut decoder, &[focus, pressed, character])
            .unwrap_err(),
        Fault::Capacity
    );
    assert_eq!(state, before);
    assert_eq!(decoder, decoder_before);
    let mut decoder = decode::Decoder::default();
    let pressed = Observation {
        fact: NativeUi::WindowsKey(WindowsKey {
            repeat_count: 4094,
            ..raw()
        }),
        ..pressed
    };
    let prepared = state
        .decode_segment(&mut decoder, &[focus, pressed, character])
        .unwrap();
    assert_eq!(prepared.decoded.len(), 3);
    assert_eq!(prepared.planned.output_cost, 4096);
    assert_eq!(prepared.planned.keys.len(), 4095); // 4094 actual repeats plus focus control.
}

#[test]
fn character_stream_does_not_borrow_key_or_ime_echo_authority() {
    use keys::{TextDisposition, text_disposition};
    let mut state = focused();
    let mut decoder = decode::Decoder::default();
    let input: Vec<_> = [('\u{16}', false), ('é', false), ('a', true)]
        .into_iter()
        .enumerate()
        .map(|(index, (character, system))| Observation {
            installation: 9,
            ordinal: index as u64 + 1,
            fact: NativeUi::WindowsCharacter(paintcraft_platform::tablet::WindowsCharacter {
                kind: WindowsCharacterKind::Utf16,
                value: character as u32,
                repeat_count: 1,
                system_character: system,
            }),
        })
        .collect();
    let prepared = state.decode_segment(&mut decoder, &input).unwrap();
    assert!(prepared.planned.keys.is_empty());
    assert_eq!(prepared.planned.output_cost, 3);
    assert_eq!(
        prepared
            .decoded
            .iter()
            .map(|item| text_disposition(&item.value))
            .collect::<Vec<_>>(),
        [
            Some(TextDisposition::Control),
            Some(TextDisposition::RequiresTarget),
            Some(TextDisposition::NativeSystem)
        ]
    );
    // Unsupported IME_CHAR is not guessed to be an echo, even after real text.
    let before = state.clone();
    let decoder_before = decoder;
    let unsupported = Observation {
        installation: 9,
        ordinal: 4,
        fact: NativeUi::UnsupportedImeCharacter {
            value: 233,
            flags: 0,
        },
    };
    assert_eq!(
        state
            .decode_segment(&mut decoder, &[unsupported])
            .unwrap_err(),
        Fault::Unsupported
    );
    assert_eq!(state, before);
    assert_eq!(decoder, decoder_before);
}

#[test]
fn actual_priority_changes_cancel_gui_authority_without_a_native_up() {
    for (changed, expected) in [
        (
            WindowsKey {
                system_key: true,
                ..raw()
            },
            Disposition::NativeSystem,
        ),
        (
            WindowsKey {
                modifiers: paintcraft_input::Modifiers {
                    alt: true,
                    ..Default::default()
                },
                ..raw()
            },
            Disposition::NativeSystem,
        ),
        (
            WindowsKey {
                modifiers: paintcraft_input::Modifiers {
                    command: true,
                    ..Default::default()
                },
                ..raw()
            },
            Disposition::NativeSystem,
        ),
        (
            WindowsKey {
                modifiers: paintcraft_input::Modifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                ..raw()
            },
            Disposition::AmbiguousModifiers,
        ),
    ] {
        let mut state = focused();
        state.segment(&[key(1, raw())]).unwrap();
        let plan = state
            .segment(&[key(
                2,
                WindowsKey {
                    was_down: true,
                    repeat_count: 2,
                    ..changed
                },
            )])
            .unwrap();
        assert_eq!(plan.output_cost, 3); // Two raw repeats and one policy reset.
        assert_eq!(plan.keys[0].disposition, expected);
        assert_eq!(plan.keys[1].disposition, expected);
        assert!(plan.keys[0].reset_gui && !plan.keys[1].reset_gui);
        assert!(
            plan.keys
                .iter()
                .all(|p| matches!(p.evidence.first.fact, NativeUi::WindowsKey(k) if k.pressed))
        );
        let tail = state
            .segment(&[key(
                3,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    ..raw()
                },
            )])
            .unwrap();
        assert_eq!(tail.keys[0].disposition, Disposition::Orphan);
        assert!(!tail.keys[0].reset_gui);
    }
}

#[test]
fn a_different_actual_native_key_retires_only_still_held_gui_keys() {
    let mut state = focused();
    state.segment(&[key(1, raw())]).unwrap();
    let alt = WindowsKey {
        virtual_key: 0x12,
        scan_code: 56,
        system_key: true,
        modifiers: paintcraft_input::Modifiers {
            alt: true,
            ..Default::default()
        },
        ..raw()
    };
    let plan = state.segment(&[key(2, alt)]).unwrap();
    assert_eq!(plan.keys[0].disposition, Disposition::NativeSystem);
    assert!(plan.keys[0].reset_gui);
    assert_eq!(
        state
            .segment(&[key(
                3,
                WindowsKey {
                    was_down: true,
                    ..raw()
                }
            )])
            .unwrap()
            .keys[0]
            .disposition,
        Disposition::Orphan
    );
    let mut completed = focused();
    completed
        .segment(&[
            key(1, raw()),
            key(
                2,
                WindowsKey {
                    was_down: true,
                    pressed: false,
                    ..raw()
                },
            ),
        ])
        .unwrap();
    assert!(!completed.segment(&[key(3, alt)]).unwrap().keys[0].reset_gui);
}

#[test]
fn repeated_native_keyup_is_malformed_and_cannot_release_prefix() {
    let mut state = focused();
    let before = state.clone();
    assert_eq!(
        state
            .segment(&[
                key(1, raw()),
                key(
                    2,
                    WindowsKey {
                        pressed: false,
                        was_down: true,
                        repeat_count: 2,
                        ..raw()
                    }
                )
            ])
            .unwrap_err(),
        Fault::Order
    );
    assert_eq!(state, before);
    state.segment(&[key(1, raw())]).unwrap();
    let before = state.clone();
    assert_eq!(
        state
            .segment(&[key(
                2,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    repeat_count: 2,
                    ..raw()
                }
            )])
            .unwrap_err(),
        Fault::Order
    );
    assert_eq!(state, before);
    assert!(
        state
            .segment(&[key(
                2,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    ..raw()
                }
            )])
            .is_ok()
    );
}

#[test]
fn nonsystem_f10_keeps_native_menu_priority() {
    let mut state = focused();
    let f10 = WindowsKey {
        virtual_key: 0x79,
        scan_code: 68,
        ..raw()
    };
    let plan = state
        .segment(&[
            key(1, f10),
            key(
                2,
                WindowsKey {
                    pressed: false,
                    was_down: true,
                    ..f10
                },
            ),
        ])
        .unwrap();
    assert!(
        plan.keys
            .iter()
            .all(|key| key.disposition == Disposition::NativeSystem)
    );
    assert!(plan.keys.iter().all(
        |key| matches!(key.evidence.first.fact, NativeUi::WindowsKey(raw) if !raw.system_key)
    ));
    let mut state = focused();
    state.segment(&[key(1, raw())]).unwrap();
    let plan = state.segment(&[key(2, f10)]).unwrap();
    assert!(plan.keys[0].reset_gui);
    assert_eq!(plan.keys[0].disposition, Disposition::NativeSystem);
}
