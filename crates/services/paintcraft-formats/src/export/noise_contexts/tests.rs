use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{AddNoiseRgbV1, BoxBlurV1, LevelsRgbV1, NoiseDistribution},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_render::noise_work::{NoiseJobPolicy, NoiseLimits};
fn workload() -> Workload {
    Workload::browser(NoiseJobPolicy::BrowserV1.max_limits()).unwrap()
}
fn unhex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|n| u8::from_str_radix(std::str::from_utf8(n).unwrap(), 16).unwrap())
        .collect()
}
fn hex(value: &[u8]) -> String {
    value.iter().map(|b| format!("{b:02x}")).collect()
}
fn document(distribution: NoiseDistribution, enabled: bool) -> Document {
    let mut doc = paintcraft_document::import_image(
        2,
        1,
        &[20, 115, 200, 255, 10, 11, 128, 0],
        "export8-context",
    )
    .unwrap();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        uuid::Uuid::from_u128(13),
        enabled,
        Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(0, 65535, false, distribution).unwrap()),
    )])
    .unwrap();
    doc
}
#[test]
fn independent_literal_region_and_program_framing_are_exact_and_abi_foreign_controls_refuse() {
    let region_bytes = unhex(
        "00080001000600020000000000000000000000010000000100000000000000010000000000000000000000000000000a0000000000000400000000000000040a0000000000000400000000000000008066f46c5fd8e98527e7b1da485080498a9fb637d6c3d81f868fbdff117a78f5fd00000000000000000000000000000000",
    );
    assert_eq!(
        hex(&Sha256::digest(&region_bytes)),
        "f030d5cbdec24f1a69013062a47c16fe3282b308238188ff350775dc57eb3141"
    );
    let region = SourceNoiseRegionContext::decode(&region_bytes, workload(), PAYLOAD_MAX).unwrap();
    assert_eq!(region.encode().unwrap().as_slice(), region_bytes);
    assert_eq!(
        region.work(),
        WorkComponents {
            base: 10,
            noise: 1024
        }
    );
    let program_bytes = unhex(
        "0008000100060002000000010000000100000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a0000000000000400000000000000040a00000000000000000000000000000000290d54fb7465ca9a7082a81caec5877a2c193533000765a4f4cc3c2af2a1a1d3707c80ef7af987cea18668d72f78a1018cc958a51d815a668d4b7dbdb680326166f46c5fd8e98527e7b1da485080498a9fb637d6c3d81f868fbdff117a78f5fde39d8163f4610ac2fdee90588318eadd0b32e67e0a2932354ec4c2474678d5080000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_eq!(
        hex(&Sha256::digest(&program_bytes)),
        "4086a60fbdabeb77a8afd42d2be9e615d82db21e078edb8b5a2eaf5822e8f4b2"
    );
    // Independent grammar vector deliberately has zero/foreign control sizes;
    // it is NOT an admitted same-ABI resource quote.
    let framed = SourceNoiseProgramContext::from_wire(&program_bytes).unwrap();
    assert_eq!(framed.wire().as_slice(), program_bytes);
    assert_eq!(
        SourceNoiseProgramContext::decode(&program_bytes, region, workload())
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    for offset in [0, 2, 4, 6, 112, 127] {
        let mut bad = region_bytes.clone();
        bad[offset] ^= 1;
        assert!(SourceNoiseRegionContext::decode(&bad, workload(), PAYLOAD_MAX).is_err());
    }
    for offset in [0, 2, 4, 6, 224, 255] {
        let mut bad = program_bytes.clone();
        bad[offset] ^= 1;
        assert!(SourceNoiseProgramContext::from_wire(&bad).is_err());
    }
    let mut bad = region_bytes.clone();
    bad[63] ^= 1;
    assert!(SourceNoiseRegionContext::decode(&bad, workload(), PAYLOAD_MAX).is_err());
}
#[test]
fn captured_policy_costs_keep_noise_out_of_base_and_no_prepared_noise_program_exists() {
    for (distribution, cost) in [
        (NoiseDistribution::Uniform, 2048),
        (NoiseDistribution::Gaussian, 16384),
    ] {
        let doc = document(distribution, true);
        // Upstream snapshot/image base is intentionally excluded in this isolated
        // component test; this is not complete codec payload admission.
        let region = SourceNoiseRegionContext::capture(&doc, 0, workload(), &|| Ok(())).unwrap();
        assert_eq!(
            region.work(),
            WorkComponents {
                base: 14,
                noise: cost
            }
        );
        assert_eq!(region.workspace_bytes(), 0);
        assert!(region.encode().is_err());
        let program =
            SourceNoiseProgramContext::capture(&doc, region, workload(), &|| Ok(())).unwrap();
        assert_eq!(program.unique_programs(), 0);
        assert_eq!(program.compile_work(), 0);
        assert_eq!(program.retained_bytes(), 0);
        let bytes = crate::pcnd::write(&doc).unwrap();
        let region = region
            .finalize_source(bytes.len(), workload(), PAYLOAD_MAX)
            .unwrap();
        let encoded = program.encode(region, workload()).unwrap();
        assert_eq!(
            SourceNoiseProgramContext::decode(&encoded, region, workload()).unwrap(),
            program
        );
        let required = Requirements::capture(&doc, &|| Ok(())).unwrap();
        program
            .validate_requirements(&required, workload(), &|| Ok(()))
            .unwrap();
        let mut forged = encoded;
        forged[192] ^= 1;
        let parsed = SourceNoiseProgramContext::decode(&forged, region, workload()).unwrap();
        assert_eq!(
            parsed
                .validate_requirements(&required, workload(), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        // Encoder scratch belongs to the aggregate source payload phase; the
        //64MiB appearance limit is not a new global encoder cap.
        assert!(
            program
                .phase_payload(region, workload(), (64 << 20) + 1, PAYLOAD_MAX)
                .is_ok()
        );
        assert_eq!(
            program
                .phase_payload(region, workload(), 0, PAYLOAD_MAX)
                .unwrap(),
            region.payload_bytes()
                + program.workspace_peak().unwrap()
                + NoiseContextControlQuote::capture()
                    .unwrap()
                    .metadata_bytes()
                    .unwrap()
        );
        let q = program
            .phase_payload(region, workload(), 0, PAYLOAD_MAX)
            .unwrap();
        assert_eq!(
            program
                .phase_payload(region, workload(), 0, q - 1)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert!(
            region
                .finalize_source(bytes.len(), workload(), PAYLOAD_MAX)
                .is_err()
        );
    }
}
#[test]
fn disabled_or_neutral_intent_never_bypasses_legacy_or_depleted_base() {
    let doc = document(NoiseDistribution::Gaussian, false);
    assert_eq!(
        SourceSpatialContext::capture_catalog(&doc, 0, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        SourceProgramContext::capture(&doc, SourceSpatialContext::default(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let full = SourceNoiseRegionContext::capture(&doc, 0, workload(), &|| Ok(())).unwrap();
    assert_eq!(full.stored_noise(), 1);
    assert_eq!(full.visible_noise(), 0);
    assert_eq!(full.work().noise, 0);
    let low = Workload::browser(NoiseLimits {
        base_work: full.work().base - 1,
        ..workload().limits()
    })
    .unwrap();
    assert_eq!(
        SourceNoiseRegionContext::capture(&doc, 0, low, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let active = document(NoiseDistribution::Gaussian, true);
    let low = Workload::browser(NoiseLimits {
        noise_work: 1,
        ..workload().limits()
    })
    .unwrap();
    assert_eq!(
        SourceNoiseRegionContext::capture(&active, 0, low, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn compile_is_base_and_same_target_control_bytes_are_bound() {
    let mut doc = document(NoiseDistribution::Uniform, true);
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.filters = FilterStack::new(vec![
        layer.filters.as_slice()[0].clone(),
        FilterInstance::new(
            uuid::Uuid::from_u128(14),
            true,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 999, 0, 255).unwrap()),
        ),
    ])
    .unwrap();
    let region = SourceNoiseRegionContext::capture(&doc, 0, workload(), &|| Ok(())).unwrap();
    let program = SourceNoiseProgramContext::capture(&doc, region, workload(), &|| Ok(())).unwrap();
    assert_eq!(program.unique_programs(), 1);
    assert!(program.compile_work() > 0);
    assert_eq!(
        program.work().base,
        region.work().base + program.compile_work()
    );
    assert_eq!(program.work().noise, region.work().noise);
    let controls = NoiseContextControlQuote::capture().unwrap();
    assert_eq!(
        controls.region_value_bytes,
        std::mem::size_of::<SourceNoiseRegionContext>() as u64
    );
    assert_eq!(
        controls.program_value_bytes,
        std::mem::size_of::<SourceNoiseProgramContext>() as u64
    );
    assert_eq!(
        controls.counter_bytes,
        NoiseAdmission::control_quote().unwrap().counter_bytes
    );
    let region = region
        .finalize_source(
            crate::pcnd::write(&doc).unwrap().len(),
            workload(),
            PAYLOAD_MAX,
        )
        .unwrap();
    let mut wire = program.encode(region, workload()).unwrap();
    wire[87] ^= 1;
    assert!(SourceNoiseProgramContext::decode(&wire, region, workload()).is_err());
}
#[test]
fn shifted_layers_and_reverse_halos_follow_the_full_source_local64_topology() {
    let mut doc =
        paintcraft_document::import_image(129, 2, &vec![80; 129 * 2 * 4], "export8-halo").unwrap();
    doc.canvas = (132, 2);
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.origin = (3, 0);
    layer.filters = FilterStack::new(vec![
        FilterInstance::new(
            uuid::Uuid::from_u128(1),
            true,
            Descriptor::AddNoiseRgbV1(
                AddNoiseRgbV1::new(10, 65535, true, NoiseDistribution::Gaussian).unwrap(),
            ),
        ),
        FilterInstance::new(
            uuid::Uuid::from_u128(2),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ),
    ])
    .unwrap();
    let region = SourceNoiseRegionContext::capture(&doc, 0, workload(), &|| Ok(())).unwrap();
    let local = Rect::new(0, 0, 129, 2).unwrap();
    let actual = noise_work::quote_layer(&doc.layers[&doc.roots[0]], local, &|| Ok(())).unwrap();
    assert_eq!(region.work().noise, actual.work.noise);
    assert_eq!(region.work().base, actual.work.base + 258);
    assert_eq!(region.workspace_bytes(), actual.bytes);
    let required = Requirements::capture(&doc, &|| Ok(())).unwrap();
    assert_eq!(required.len(), 2);
    let error = SourceNoiseRegionContext::capture(&doc, 0, workload(), &|| {
        Err(Error::new(ErrorCode::OperationCancelled, "stop"))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}
