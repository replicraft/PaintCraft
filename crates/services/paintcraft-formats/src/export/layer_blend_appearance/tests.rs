use super::super::layer_appearance::appearance_registration;
use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    policy::hex,
};
fn data() -> Value {
    json::parse(include_bytes!(
        "../../../tests/source8-framing-fixtures/vectors.json"
    ))
    .unwrap()
}
fn at<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap()
}
fn uint(v: &Value) -> u64 {
    u64::try_from(v.as_i64().unwrap()).unwrap()
}
fn bytes(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
fn hash(v: &Value) -> [u8; 32] {
    bytes(v.as_str().unwrap()).try_into().unwrap()
}
fn expected(v: &Value, k: &str) -> [u8; 32] {
    hash(at(at(at(v, "frames"), k), "sha256"))
}
fn grant(v: &Value) -> Admission8 {
    let noise = uint(at(v, "mode")) == 1;
    Admission8 {
        mode: if noise { Mode7::Noise } else { Mode7::Base },
        work_authority: if uint(at(v, "origin")) == 0 {
            Origin7::CapturedJob
        } else {
            Origin7::StandaloneViewJobs
        },
        workspace_bytes: 64 << 20,
        base_work: 268435456,
        noise_work: if noise { 1 << 35 } else { 0 },
    }
}
fn required(v: &Value) -> Requirements8 {
    let mut r = Requirements::default();
    if uint(at(v, "mode")) == 1 {
        r.insert("add-noise-rgb-v1", "params-v1").unwrap();
    }
    if at(v, "name")
        .as_str()
        .unwrap()
        .starts_with("disabled-filter")
    {
        r.insert("invert-rgb-v1", "params-v1").unwrap();
    }
    Requirements8::new(r, appearance_registration(), blend_registration(), &|| {
        Ok(())
    })
    .unwrap()
}
fn inventory(v: &Value, candidate: bool) -> Inventory7 {
    let own = at(v, "own");
    let abi = if at(v, "coverageAbi").as_str() == Some("wasm32-v1") {
        CoverageAbi::Wasm32V1
    } else {
        CoverageAbi::Native64V1
    };
    let rows = at(
        v,
        if candidate {
            "candidateRows"
        } else {
            "sourceRows"
        },
    )
    .as_array()
    .unwrap();
    let sum = |k| {
        rows.iter()
            .map(|r| uint(at(if candidate { at(r, "row") } else { r }, k)))
            .sum()
    };
    Inventory7 {
        own: LayerMasks7 {
            stored: uint(at(own, "stored")) as u32,
            enabled: uint(at(own, "enabled")) as u32,
            bands: uint(at(own, "bands")),
            runs: uint(at(own, "runs")),
            decode_upper_bytes: uint(at(own, "decodeUpperBytes")),
            identity: hash(at(own, "identitySha256")),
            abi,
        },
        occurrences: rows.len() as u32,
        bands: sum("bands"),
        runs: sum("runs"),
        decode_upper_bytes: sum("upper"),
        source_coverage: expected(v, "sourceBounds"),
        mask_inventory: expected(
            v,
            if candidate {
                "candidateInventory"
            } else {
                "sourceInventory"
            },
        ),
    }
}
#[test]
fn independent_eleven_source_candidate_union_frames_match_exact_hashes_and_capture() {
    let d = data();
    for v in at(&d, "vectors").as_array().unwrap() {
        let a = grant(v);
        assert_eq!(a.digest().unwrap(), expected(v, "admission"));
        let r = required(v);
        let (required, resolution) = r.requirements().digests(&|| Ok(())).unwrap();
        assert_eq!(required, expected(v, "required"));
        assert_eq!(resolution, expected(v, "resolution"));
        for candidate in [false, true] {
            let inv = inventory(v, candidate);
            let fact =
                Fact8::capture(&r, a, inv, expected(v, "blendIdentity"), &|| Ok(())).unwrap();
            assert_eq!(
                fact.capability,
                expected(
                    v,
                    if candidate {
                        "candidateCapability"
                    } else {
                        "sourceCapability"
                    }
                )
            );
            fact.validate_binding(a, appearance_registration(), blend_registration())
                .unwrap();
            assert_eq!(
                Fact8::from_frames(
                    NumericalFrames8 {
                        count: r.requirements().len(),
                        required,
                        resolution
                    },
                    a,
                    appearance_registration(),
                    blend_registration(),
                    inv,
                    expected(v, "blendIdentity")
                )
                .unwrap(),
                fact
            );
            assert_eq!(
                at(&fact.value(), "layerBlendSha256").as_str(),
                Some(hex(&fact.layer_blend).as_str())
            );
        }
        let union = Union8::capture(
            &r,
            a,
            expected(v, "sourceBounds"),
            expected(v, "blendIdentity"),
            inventory(v, false).own.abi,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(union.capability, expected(v, "union"));
        union
            .validate_binding(
                a,
                appearance_registration(),
                blend_registration(),
                inventory(v, false).own.abi,
            )
            .unwrap();
    }
}
#[test]
fn zero_own_is_metadata_valid_for_eight_and_all_binding_components_are_authenticated() {
    let d = data();
    let v = &at(&d, "vectors").as_array().unwrap()[0];
    let a = grant(v);
    let r = required(v);
    let f = Fact8::capture(
        &r,
        a,
        inventory(v, false),
        expected(v, "blendIdentity"),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(f.own.stored, 0);
    for changed in [
        Fact8 { count: 1, ..f },
        Fact8 {
            layer_blend: [0; 32],
            ..f
        },
        Fact8 {
            mask_inventory: [0; 32],
            ..f
        },
        Fact8 {
            source_coverage: [0; 32],
            ..f
        },
        Fact8 {
            own: LayerMasks7 {
                abi: CoverageAbi::Native64V1,
                ..f.own
            },
            ..f
        },
    ] {
        assert!(
            changed
                .validate_binding(a, appearance_registration(), blend_registration())
                .is_err()
        );
    }
}

#[test]
fn exact_registrations_and_fixed_mode_tags_refuse_unknown_price_or_revision() {
    let b = blend_registration();
    let changes = [
        BlendRegistration {
            context_setup_per_root: 255,
            ..b
        },
        BlendRegistration {
            opaque_extra_work: 31,
            ..b
        },
        BlendRegistration {
            general_work: 255,
            ..b
        },
        BlendRegistration {
            work_revision: 2,
            ..b
        },
        BlendRegistration {
            arithmetic_revision: 2,
            ..b
        },
        BlendRegistration {
            resource_contract_revision: 2,
            ..b
        },
    ];
    for bad in changes {
        assert_eq!(
            bad.validate().unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(
            Requirements8::new(
                Requirements::default(),
                appearance_registration(),
                bad,
                &|| Ok(())
            )
            .is_err()
        );
    }
    let mut mask = appearance_registration();
    mask.query_revision = 2;
    assert!(Requirements8::new(Requirements::default(), mask, b, &|| Ok(())).is_err());
    use paintcraft_core::types::LayerBlendMode;
    assert_eq!(
        [
            LayerBlendMode::Normal as u8,
            LayerBlendMode::Multiply as u8,
            LayerBlendMode::Screen as u8
        ],
        [0, 1, 2]
    );
    assert_eq!(
        at(&b.value(), "context_setup_per_root").as_str(),
        Some("256")
    );
}
#[test]
fn origin_abi_and_fact_union_widths_do_not_silently_alias_old_contracts() {
    let d = data();
    let v = &at(&d, "vectors").as_array().unwrap()[0];
    let a = grant(v);
    let r = required(v);
    let inv = inventory(v, false);
    let (required, resolution) = r.requirements().digests(&|| Ok(())).unwrap();
    let frames = NumericalFrames8 {
        count: 257,
        required,
        resolution,
    };
    assert!(
        Fact8::from_frames(
            frames,
            a,
            appearance_registration(),
            blend_registration(),
            inv,
            expected(v, "blendIdentity")
        )
        .is_err()
    );
    let u = Union8::from_frames(
        frames,
        a,
        appearance_registration(),
        blend_registration(),
        inv.source_coverage,
        expected(v, "blendIdentity"),
        inv.own.abi,
    )
    .unwrap();
    assert_eq!(u.count, 257);
    assert!(
        u.validate_binding(
            a,
            appearance_registration(),
            blend_registration(),
            CoverageAbi::Native64V1
        )
        .is_err()
    );
    assert!(
        Union8::from_frames(
            NumericalFrames8 {
                count: 258,
                ..frames
            },
            a,
            appearance_registration(),
            blend_registration(),
            inv.source_coverage,
            expected(v, "blendIdentity"),
            inv.own.abi
        )
        .is_err()
    );
    assert_ne!(
        a.digest().unwrap(),
        Admission8 {
            work_authority: Origin7::StandaloneViewJobs,
            ..a
        }
        .digest()
        .unwrap()
    );
    assert_ne!(
        a.digest().unwrap(),
        super::super::layer_appearance::Admission7 {
            mode: a.mode,
            work_authority: a.work_authority,
            workspace_bytes: a.workspace_bytes,
            base_work: a.base_work,
            noise_work: a.noise_work
        }
        .digest()
        .unwrap()
    );
    for bad in [
        Admission8 { noise_work: 1, ..a },
        Admission8 {
            workspace_bytes: Admission8::WORKSPACE_MAX + 1,
            ..a
        },
        Admission8 {
            base_work: Admission8::BASE_MAX + 1,
            ..a
        },
    ] {
        assert!(bad.validate().is_err());
    }
}
#[test]
fn thirteen_pair_metadata_and_old_numerical_digests_stay_exact_legacy_noise_stays_closed() {
    use super::super::prepared_capabilities::{registrations, resolve_for};
    assert_eq!(registrations().len(), 13);
    assert_eq!(
        RegistryContract::try_from(4).unwrap(),
        RegistryContract::LayerAppearance4
    );
    for revision in [0, 5, u16::MAX] {
        assert!(RegistryContract::try_from(revision).is_err());
    }
    let mut old = Requirements::default();
    for entry in &registrations()[..12] {
        old.insert(entry.profile(), entry.codec()).unwrap();
    }
    let digest = old.digests(&|| Ok(())).unwrap();
    assert_eq!(
        hex(&digest.0),
        "585a38dbdd4ff739357095aba8ca932ef90785942361e7335b772de331e553c1"
    );
    assert_eq!(
        hex(&digest.1),
        "19c1306ec948ec8a0ae131be6fe4418e8bb9fc805b41c3bbfbbb6d4bd0539c1b"
    );
    for entry in registrations() {
        assert!(std::ptr::eq(
            resolve_for(
                RegistryContract::LayerAppearance4,
                entry.profile(),
                entry.codec()
            )
            .unwrap(),
            entry
        ));
    }
    let r = Requirements8::new(
        old,
        appearance_registration(),
        blend_registration(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(r.requirements().digests(&|| Ok(())).unwrap(), digest);
    assert!(resolve_for(RegistryContract::Legacy1, "add-noise-rgb-v1", "params-v1").is_err());
    assert!(
        resolve_for(
            RegistryContract::LayerAppearance4,
            "layer-blend-modes.v1",
            "params-v1"
        )
        .is_err()
    );
}
#[test]
fn framing_quote_covers_literal_extents_and_has_no_implicit_caller_debit() {
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
    let d = data();
    for v in at(&d, "vectors").as_array().unwrap() {
        let r = required(v);
        let frames = at(v, "frames");
        let bytes = |k| uint(at(at(frames, k), "bytes"));
        let hash_inputs = bytes("required")
            + bytes("resolution")
            + bytes("admission")
            + bytes("sourceCapability").max(bytes("union"));
        let b = SharedBudget::new(0);
        let scope = b.scope(AllocationClass::SharedBudget);
        let a = WorkspaceAdmission::new(scope.clone(), 0, 0);
        let q = with_workspace(a.clone(), || framing_work_quote(&r)).unwrap();
        assert!(q >= 4096 + 16 * hash_inputs);
        let fact = Fact8::capture(
            &r,
            grant(v),
            inventory(v, false),
            expected(v, "blendIdentity"),
            &|| Ok(()),
        )
        .unwrap();
        let serialized = json::canonical(&fact.value()).len()
            + json::canonical(&appearance_registration().value()).len()
            + json::canonical(&blend_registration().value()).len();
        assert!(q >= 4096 + 16 * (hash_inputs + serialized as u64));
        assert_eq!(a.work_used(), 0);
        assert_eq!(scope.footprint().live, 0);
        assert!(admission_work_quote() >= 4096 + 16 * bytes("admission"));
    }
}
#[test]
fn capture_cancellation_and_base_noise_requirement_checks_create_no_execution_owner() {
    let d = data();
    let v = &at(&d, "vectors").as_array().unwrap()[2];
    let r = required(v);
    let a = grant(v);
    assert!(a.validate_requirements(&r).is_ok());
    assert!(
        Admission8 {
            mode: Mode7::Base,
            noise_work: 0,
            ..a
        }
        .validate_requirements(&r)
        .is_err()
    );
    let error = Fact8::capture(
        &r,
        a,
        inventory(v, false),
        expected(v, "blendIdentity"),
        &|| {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "metadata cancelled",
            ))
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}
