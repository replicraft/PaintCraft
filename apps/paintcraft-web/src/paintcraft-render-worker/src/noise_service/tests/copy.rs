//! Authored finite service controls. Actual encoded Wasm bytes need Chrome proof.
use super::*;
fn options(purpose: &str, bake: bool) -> Value {
    Value::object([
        ("version", Value::Int(1)),
        ("purpose", Value::str(purpose)),
        ("format", Value::str("png")),
        ("quality", Value::Int(90)),
        ("matte", Value::Array(vec![Value::Int(255); 3])),
        ("frontend", Value::str("studio")),
        ("omitMetadata", Value::Bool(false)),
        ("bakeFilters", Value::Bool(bake)),
        ("maskLength", Value::Int(0)),
        ("maskSha256", Value::str(hex(&Sha256::digest([])))),
    ])
}
#[test]
fn foreign_copy_requires_actual_bake_while_visual_print_retains_source() {
    let d = doc(NoiseDistribution::Uniform, None);
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let before = pcnd::write(&s.state.base).unwrap();
    let r = request(&s, 1, true);
    assert_eq!(
        s.prepare_copy(&text(&r), &text(&options("download", false)), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(s.state.frame.is_none());
    assert!(s.state.generation.is_none());
    let admitted = s
        .prepare_copy(&text(&r), &text(&options("print", false)), &|| Ok(()))
        .unwrap();
    assert_eq!(admitted.get("outputVersion"), Some(&Value::Int(1)));
    finish(&mut s);
    assert_eq!(pcnd::write(&s.state.base).unwrap(), before);
    assert!(s.state.frame.as_ref().unwrap().copy.is_some());
    s.retire();
    assert!(s.state.frame.is_none());
}
#[test]
fn codec_components_are_on_same_parent_and_low_base_refuses_without_frame() {
    let d = doc(NoiseDistribution::Gaussian, None);
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let r = request(&s, 1, true);
    let standard = frame::NoiseFrame::quote(
        &s.state.base,
        DisplayRequest::new(&s.state.base, s.state.base.canvas_rect(), 0).unwrap(),
        s.state.profile,
        s.state.policy,
        &|| Ok(()),
    )
    .unwrap()
    .work;
    let admitted = s
        .prepare_copy(&text(&r), &text(&options("download", true)), &|| Ok(()))
        .unwrap();
    let quote = s
        .state
        .profile
        .workload
        .parse_quote(admitted.get("binding").unwrap().get("workQuote").unwrap())
        .unwrap();
    assert_eq!(quote.base, standard.base + 128);
    assert_eq!(quote.noise, standard.noise);
    assert!(
        s.state
            .frame
            .as_ref()
            .unwrap()
            .admission
            .used()
            .unwrap()
            .base
            >= 128
    );
    s.retire();
    let mut low = open(
        &d,
        NoiseLimits {
            bytes: 64 << 20,
            base_work: standard.base,
            noise_work: standard.noise,
        },
    );
    let r = request(&low, 1, true);
    assert_eq!(
        low.prepare_copy(&text(&r), &text(&options("download", true)), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(low.state.frame.is_none());
}
#[test]
fn unsupported_codec_and_private_operations_are_rejected_before_frame() {
    let d = doc(NoiseDistribution::Uniform, None);
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let mut spec = options("download", true);
    set(&mut spec, "format", Value::str("dds"));
    assert_eq!(
        s.prepare_copy(&text(&request(&s, 1, true)), &text(&spec), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(s.state.generation.is_none());
    let r = request(&s, 1, false);
    assert!(
        s.prepare_copy(&text(&r), &text(&options("print", false)), &|| Ok(()))
            .is_err()
    );
    assert!(s.state.frame.is_none());
}

fn selected_options(mask: &[u8]) -> Value {
    let mut value = options("clipboard", false);
    set(&mut value, "maskLength", Value::Int(mask.len() as i64));
    set(
        &mut value,
        "maskSha256",
        Value::str(hex(&Sha256::digest(mask))),
    );
    value
}
#[test]
fn copy_mask_failures_retire_the_real_service_frame_without_js_cleanup() {
    for case in 0..3 {
        let d = doc(NoiseDistribution::Uniform, None);
        let mut service = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
        let original = pcnd::write(&service.state.base).unwrap();
        let expected = if case == 2 {
            vec![0, 128]
        } else {
            vec![0, 255]
        };
        let supplied = match case {
            0 => vec![0],
            1 => vec![255, 0],
            _ => expected.clone(),
        };
        let begin = request(&service, 1, true);
        service
            .prepare_copy(&text(&begin), &text(&selected_options(&expected)), &|| {
                Ok(())
            })
            .unwrap();
        finish(&mut service);
        assert_eq!(
            service.state.frame.as_ref().unwrap().pixels[3],
            255,
            "a zero first mask byte would observably mutate this BASE"
        );
        assert!(service.encode_copy_checked(&supplied, &|| Ok(())).is_err());
        assert!(service.state.frame.is_none());
        assert!(service.state.last_binding.is_none());
        assert!(service.take_frame().is_err());
        assert!(service.step().is_err());
        assert!(service.encode_copy_checked(&supplied, &|| Ok(())).is_err());
        assert!(
            service
                .prepare_copy(&text(&begin), &text(&selected_options(&[0, 255])), &|| Ok(
                    ()
                ))
                .is_err()
        );
        assert_eq!(pcnd::write(&service.state.base).unwrap(), original);
        assert_eq!(service.state.workspace.footprint().live, 0);
        assert_eq!(service.state.output.footprint().live, 0);
        let next = request(&service, 2, true);
        service
            .prepare_copy(
                &text(&next),
                &text(&selected_options(&[0, 255])),
                &|| Ok(()),
            )
            .unwrap();
        finish(&mut service);
        service.retire();
        assert_eq!(pcnd::write(&service.state.base).unwrap(), original);
    }
}
#[test]
fn mask_and_codec_callback_failures_retire_partial_output_and_allow_fresh_generation() {
    // For two PNG pixels: binary prevalidation, application, codec initial,
    // codec final. The latter two run after zeroing the first private pixel.
    for cancel_at in 1..=4 {
        let d = doc(NoiseDistribution::Uniform, None);
        let mut service = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
        let original = pcnd::write(&service.state.base).unwrap();
        let begin = request(&service, 1, true);
        service
            .prepare_copy(&text(&begin), &text(&selected_options(&[0, 255])), &|| {
                Ok(())
            })
            .unwrap();
        finish(&mut service);
        let count = std::cell::Cell::new(0);
        let check = || {
            count.set(count.get() + 1);
            if count.get() == cancel_at {
                Err(Error::new(ErrorCode::OperationCancelled, "copy cancelled"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            service
                .encode_copy_checked(&[0, 255], &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(count.get(), cancel_at);
        assert!(service.take_frame().is_err());
        assert!(service.step().is_err());
        assert!(service.state.frame.is_none());
        assert!(service.state.last_binding.is_none());
        assert_eq!(service.state.workspace.footprint().live, 0);
        assert_eq!(service.state.output.footprint().live, 0);
        assert_eq!(pcnd::write(&service.state.base).unwrap(), original);
        let next = request(&service, 2, true);
        service
            .prepare_copy(&text(&next), &text(&options("print", false)), &|| Ok(()))
            .unwrap();
        finish(&mut service);
        service.retire();
    }
}

#[test]
fn binary_validation_and_application_are_two_priced_passes_on_same_parent() {
    let d = doc(NoiseDistribution::Uniform, None);
    let mut service = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let ordinary = frame::NoiseFrame::quote(
        &service.state.base,
        DisplayRequest::new(&service.state.base, service.state.base.canvas_rect(), 0).unwrap(),
        service.state.profile,
        service.state.policy,
        &|| Ok(()),
    )
    .unwrap()
    .work;
    let begin = request(&service, 1, true);
    let admitted = service
        .prepare_copy(&text(&begin), &text(&selected_options(&[0, 255])), &|| {
            Ok(())
        })
        .unwrap();
    let quoted = service
        .state
        .profile
        .workload
        .parse_quote(admitted.get("binding").unwrap().get("workQuote").unwrap())
        .unwrap();
    assert_eq!(quoted.base, ordinary.base + 128 + 4);
    assert_eq!(quoted.noise, ordinary.noise);
    assert!(
        service
            .state
            .frame
            .as_ref()
            .unwrap()
            .admission
            .used()
            .unwrap()
            .base
            >= 132
    );
    service.retire();
    assert_eq!(service.state.workspace.footprint().live, 0);
    assert_eq!(service.state.output.footprint().live, 0);
}
