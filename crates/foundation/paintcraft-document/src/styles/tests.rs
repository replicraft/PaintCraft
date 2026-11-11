use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    styles::{ColorOverlayRgbV1, EffectBlendMode},
};
use std::cell::Cell;

fn descriptor(color: [u8; 3], opacity: u8, mode: EffectBlendMode) -> StyleDescriptor {
    StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(color, opacity, mode))
}
fn instance(id: u128) -> StyleInstance {
    StyleInstance::new(
        Uuid::from_u128(id),
        true,
        descriptor([13, 127, 241], 128, EffectBlendMode::Normal),
    )
    .unwrap()
}
fn from_oracle(value: &Value) -> StyleInstance {
    let get = |name| value.get(name).unwrap();
    let byte = |name| u8::try_from(get(name).as_i64().unwrap()).unwrap();
    let color = get("color")
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::try_from(v.as_i64().unwrap()).unwrap())
        .collect::<Vec<_>>();
    StyleInstance::new(
        Uuid::parse_str(get("id").as_str().unwrap()).unwrap(),
        get("enabled").as_bool().unwrap(),
        descriptor(
            color.try_into().unwrap(),
            byte("opacity"),
            EffectBlendMode::from_code(byte("blend")).unwrap(),
        ),
    )
    .unwrap()
}

#[test]
fn independent_framing_fixtures_pin_complete_ordered_intent() {
    // Python frames/expectations were frozen before this Rust candidate existed.
    let fixture = json::parse(include_bytes!(
        "../../tests/style-stack-fixtures/vectors.json"
    ))
    .unwrap();
    let cases = fixture.get("cases").unwrap().as_array().unwrap();
    assert_eq!(cases.len(), 15);
    for case in cases {
        let entries = case.get("entries").unwrap().as_array().unwrap();
        let stack = StyleStack::new(entries.iter().map(from_oracle).collect()).unwrap();
        assert_eq!(
            crate::hex(&stack.digest()),
            case.get("sha256").unwrap().as_str().unwrap(),
            "{}",
            case.get("name").unwrap().as_str().unwrap()
        );
        assert_eq!(stack.len(), entries.len());
    }
}

#[test]
fn nonnil_identity_is_required_without_changing_existing_filter_policy() {
    let p = descriptor([0; 3], 0, EffectBlendMode::Normal);
    assert_eq!(
        StyleInstance::new(Uuid::nil(), false, p).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        instance(1).with_id(Uuid::nil()).unwrap_err().code,
        ErrorCode::InputSchema
    );
    // Defensive stack validation does not rely only on the public constructor.
    let invalid = StyleInstance {
        id: Uuid::nil(),
        enabled: false,
        descriptor: p,
    };
    assert_eq!(
        StyleStack::new(vec![invalid]).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert!(
        crate::FilterStack::new(vec![crate::FilterInstance::new(
            Uuid::nil(),
            false,
            paintcraft_core::types::filters::Descriptor::InvertRgbV1,
        )])
        .is_ok()
    );
}

#[test]
fn bounded_counts_and_per_stack_uniqueness_include_disabled_instances() {
    let complete: Vec<_> = (1..=STYLES_MAX)
        .map(|i| instance(i as u128).with_enabled(false))
        .collect();
    assert_eq!(StyleStack::new(complete.clone()).unwrap().len(), 256);
    let mut too_many = complete.clone();
    too_many.push(instance(257));
    assert_eq!(
        StyleStack::new(too_many).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut duplicate = complete;
    duplicate[255] = instance(1).with_enabled(false);
    assert_eq!(
        StyleStack::new(duplicate).unwrap_err().code,
        ErrorCode::InputSchema
    );
    // Same UUID in another independent stack is valid; references are layer-qualified.
    assert_eq!(
        StyleStack::new(vec![instance(1)]).unwrap(),
        StyleStack::new(vec![instance(1)]).unwrap()
    );
}

#[test]
fn replacements_preserve_source_instance_and_change_complete_stack_identity() {
    let original = instance(1);
    let changed = original.with_descriptor(descriptor([1, 2, 3], 0, EffectBlendMode::Screen));
    let disabled = changed.with_enabled(false);
    let duplicate = disabled.with_id(Uuid::from_u128(2)).unwrap();
    assert_eq!(original, instance(1));
    assert_eq!(changed.id(), original.id());
    assert_eq!(changed.enabled(), original.enabled());
    assert_eq!(disabled.id(), original.id());
    assert_eq!(duplicate.descriptor(), disabled.descriptor());
    assert_eq!(duplicate.enabled(), disabled.enabled());
    assert_ne!(duplicate.id(), disabled.id());
    let source = StyleStack::new(vec![original]).unwrap();
    let before = source.clone();
    for replacement in [changed, disabled, duplicate] {
        assert_ne!(
            source.digest(),
            StyleStack::new(vec![replacement]).unwrap().digest()
        );
    }
    assert_eq!(source, before);
}

#[test]
fn excessive_input_capacity_is_compacted_and_capacity_weight_is_actual() {
    for count in [0, 1, 256] {
        let mut input = Vec::with_capacity(4096);
        input.extend((1..=count).map(|i| instance(i as u128)));
        let stack = StyleStack::new(input).unwrap();
        assert_eq!(stack.len(), count);
        assert!(stack.instances.capacity() <= STYLES_MAX);
        assert_eq!(
            stack.storage_bytes(),
            stack.instances.capacity() * size_of::<StyleInstance>()
        );
        assert!(stack.storage_bytes() >= count * size_of::<StyleInstance>());
        assert_eq!(stack.iter().len(), count);
        assert_eq!(
            stack
                .iter()
                .rev()
                .map(StyleInstance::id)
                .collect::<Vec<_>>(),
            (1..=count)
                .rev()
                .map(|i| Uuid::from_u128(i as u128))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn every_constructor_cancellation_discards_private_result_without_changing_source() {
    let source = StyleStack::new(vec![instance(1), instance(2), instance(3)]).unwrap();
    let before = source.clone();
    for large_capacity in [false, true] {
        let input = || {
            let mut entries = Vec::with_capacity(if large_capacity { 4096 } else { 3 });
            entries.extend(source.iter().copied());
            entries
        };
        let calls = Cell::new(0);
        let successful = StyleStack::new_checked(input(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(successful.digest(), source.digest());
        for stop in 1..=calls.get() {
            let observed = Cell::new(0);
            let error = StyleStack::new_checked(input(), &|| {
                observed.set(observed.get() + 1);
                if observed.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel style construction",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(observed.get(), stop);
            assert_eq!(source, before);
        }
    }
    let calls = Cell::new(0);
    StyleStack::new_checked(Vec::new(), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let observed = Cell::new(0);
        assert_eq!(
            StyleStack::new_checked(Vec::new(), &|| {
                observed.set(observed.get() + 1);
                if observed.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel empty"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
}

#[test]
fn empty_style_identity_has_a_separate_namespace_and_no_execution_grant() {
    let stack = StyleStack::default();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
    assert_eq!(stack.storage_bytes(), 0);
    assert_eq!(stack, StyleStack::new(Vec::new()).unwrap());
    assert_ne!(stack.digest(), crate::FilterStack::default().digest());
    assert_eq!(
        instance(1).descriptor().profile_id(),
        ColorOverlayRgbV1::PROFILE
    );
    assert_eq!(
        instance(1).descriptor().semantic_parameter_bytes(),
        [13, 127, 241, 128, 0]
    );
}

#[test]
fn observe_style_vocabulary_layout_without_a_portable_abi_claim() {
    eprintln!(
        "STYLE_LAYOUT arch={} pointer={} descriptor={} instance={} stack={}",
        std::env::consts::ARCH,
        size_of::<usize>(),
        size_of::<StyleDescriptor>(),
        size_of::<StyleInstance>(),
        size_of::<StyleStack>()
    );
}
