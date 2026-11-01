use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    policy::hex,
};
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
struct Input<'a>(&'a [u8]);
impl Input<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let (a, b) = self.0.split_at(N);
        self.0 = b;
        a.try_into().unwrap()
    }
    fn u64(&mut self) -> u64 {
        u64::from_be_bytes(self.take())
    }
    fn u32(&mut self) -> u32 {
        u32::from_be_bytes(self.take())
    }
    fn u16(&mut self) -> u16 {
        u16::from_be_bytes(self.take())
    }
    fn byte(&mut self) -> u8 {
        self.take::<1>()[0]
    }
    fn text(&mut self) -> String {
        let n = self.u16() as usize;
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        std::str::from_utf8(a).unwrap().to_owned()
    }
    fn key(&mut self) -> CoverageKey {
        CoverageKey {
            layer: self.take(),
            kind: self.byte(),
            filter: self.take(),
        }
    }
    fn tail(&mut self, key: CoverageKey, enabled: bool) -> CoverageRow {
        CoverageRow {
            key,
            enabled,
            domain: std::array::from_fn(|_| i64::from_be_bytes(self.take())),
            digest: self.take(),
            bands: self.u64(),
            runs: self.u64(),
            decode_upper_bytes: self.u64(),
        }
    }
}
fn grant(noise: bool) -> Admission7 {
    Admission7 {
        mode: if noise { Mode7::Noise } else { Mode7::Base },
        work_authority: Origin7::CapturedJob,
        workspace_bytes: 64 << 20,
        base_work: 268_435_456,
        noise_work: if noise { 1 << 35 } else { 0 },
    }
}
fn replay(value: &Value) {
    let bytes = unhex(value.get("hex").unwrap().as_str().unwrap());
    let expected = value.get("sha256").unwrap().as_str().unwrap();
    let prefix = bytes.iter().position(|b| *b == 0).unwrap() + 1;
    let domain = &bytes[..prefix];
    let mut input = Input(&bytes[prefix..]);
    let digest = match domain {
        b"paintcraft.worker-layer-masks/v1\0" => {
            let n = input.u32();
            let mut h = IdentityHasher::new(n).unwrap();
            for _ in 0..n {
                h.push(input.take(), input.byte() == 1, input.take())
                    .unwrap();
            }
            h.finish().unwrap()
        }
        b"paintcraft.worker-coverage-bounds/v1\0" => {
            assert_eq!(input.text(), "wasm32-v1");
            assert_eq!(input.u16(), 1);
            let n = input.u32();
            let mut h = CoverageHasher::new(CoverageAbi::Wasm32V1, n).unwrap();
            for _ in 0..n {
                let key = input.key();
                let enabled = input.byte() == 1;
                h.push(input.tail(key, enabled)).unwrap();
            }
            h.finish().unwrap()
        }
        b"paintcraft.worker-mask-inventory/v1\0" => {
            let n = input.u32();
            let mut h = InventoryHasher::new(n).unwrap();
            for _ in 0..n {
                let key = input.key();
                let instance = input.byte() == 1;
                let enabled = input.byte() == 1;
                let row = input.tail(key, enabled);
                h.push(row, instance, input.key()).unwrap();
            }
            h.finish().unwrap()
        }
        b"paintcraft.source7-admission/v2\0" => {
            let mode = if input.byte() == 0 {
                Mode7::Base
            } else {
                Mode7::Noise
            };
            let origin = match input.byte() {
                0 => Origin7::CapturedJob,
                1 => Origin7::StandaloneViewJobs,
                _ => panic!("unknown independent origin"),
            };
            let a = Admission7 {
                mode,
                work_authority: origin,
                workspace_bytes: input.u64(),
                base_work: input.u64(),
                noise_work: input.u64(),
            };
            assert_eq!(input.u64(), a.total().unwrap());
            assert_eq!(input.u16(), 1);
            assert_eq!(input.u16(), 64);
            assert_eq!(input.u64(), 8_388_608);
            assert_eq!(input.u64(), a.noise_slice());
            a.digest().unwrap()
        }
        b"paintcraft.worker-capabilities/v3\0" | b"paintcraft.worker-capability-union/v1\0" => {
            assert_eq!(input.u16(), 3);
            let required = input.take();
            let resolution = input.take();
            let admission = input.take::<32>();
            let a = [
                grant(false),
                grant(true),
                Admission7 {
                    work_authority: Origin7::StandaloneViewJobs,
                    ..grant(false)
                },
                Admission7 {
                    work_authority: Origin7::StandaloneViewJobs,
                    ..grant(true)
                },
            ]
            .into_iter()
            .find(|candidate| candidate.digest().unwrap() == admission)
            .expect("one of four independently framed origins");
            let p = appearance_registration();
            assert_eq!(input.text(), p.feature_id);
            assert_eq!(input.text(), p.resource_contract_id);
            assert_eq!(input.u16(), 1);
            assert_eq!(input.u16(), 1);
            assert_eq!(input.u16(), 1);
            assert_eq!(input.text(), p.workload_class_id);
            assert_eq!(input.u16(), 1);
            if domain == b"paintcraft.worker-capability-union/v1\0" {
                Union7::from_frames(0, required, resolution, a, p, input.take())
                    .unwrap()
                    .capability
            } else {
                let own = LayerMasks7 {
                    stored: input.u32(),
                    enabled: input.u32(),
                    bands: input.u64(),
                    runs: input.u64(),
                    decode_upper_bytes: input.u64(),
                    identity: input.take(),
                    abi: CoverageAbi::Wasm32V1,
                };
                let inventory = Inventory7 {
                    own,
                    occurrences: own.stored,
                    bands: own.bands,
                    runs: own.runs,
                    decode_upper_bytes: own.decode_upper_bytes,
                    source_coverage: input.take(),
                    mask_inventory: input.take(),
                };
                Fact7::from_frames(0, required, resolution, a, p, inventory)
                    .unwrap()
                    .capability
            }
        }
        _ => panic!("unknown independent frame"),
    };
    assert!(input.0.is_empty());
    assert_eq!(hex(&digest), expected);
}
#[test]
fn independent_preimage_frames_cover_source_candidates_and_capability_union() {
    let value = json::parse(include_bytes!(
        "../../../tests/source7-framing-fixtures/framing-vectors.json"
    ))
    .unwrap();
    let mut count = 0;
    for row in value.get("vectors").unwrap().as_array().unwrap() {
        for frame in row.get("frames").unwrap().as_object().unwrap().values() {
            replay(frame);
            count += 1;
        }
    }
    for row in value
        .get("candidateMutationVectors")
        .unwrap()
        .as_array()
        .unwrap()
    {
        for key in [
            "candidateInventory",
            "candidateCapability",
            "unionCapability",
        ] {
            replay(row.get(key).unwrap());
            count += 1;
        }
    }
    for row in value.get("originVectors").unwrap().as_array().unwrap() {
        for frame in row.get("frames").unwrap().as_object().unwrap().values() {
            replay(frame);
            count += 1;
        }
    }
    assert_eq!(count, 72);
}
#[test]
fn registry3_preserves_thirteen_numerical_resolutions_and_independent_appearance_grant() {
    let mut r = Requirements::default();
    for row in numerical::registrations() {
        r.insert(row.profile(), row.codec()).unwrap();
    }
    assert_eq!(r.len(), 13);
    let old = r.digests(&|| Ok(())).unwrap();
    let next = Requirements7::new(r, appearance_registration(), &|| Ok(())).unwrap();
    assert_eq!(next.requirements().digests(&|| Ok(())).unwrap(), old);
    assert!(next.requires_noise().unwrap());
    assert!(grant(false).validate_requirements(&next).is_err());
    assert!(grant(true).validate_requirements(&next).is_ok());
    let empty = Requirements7::new(Requirements::default(), appearance_registration(), &|| {
        Ok(())
    })
    .unwrap();
    assert!(grant(true).validate_requirements(&empty).is_ok());
    let mut unknown = appearance_registration();
    unknown.query_revision = 2;
    assert!(Requirements7::new(Requirements::default(), unknown, &|| Ok(())).is_err());
    assert!(RegistryContract::try_from(4).is_err());
}
#[test]
fn stream_order_duplicates_and_u32_occurrence_width_refuse_before_unbounded_storage() {
    assert_eq!(OCCURRENCE_MAX, 25_700_000);
    assert!(IdentityHasher::new(65_536).is_ok());
    assert!(CoverageHasher::new(CoverageAbi::Wasm32V1, OCCURRENCE_MAX + 1).is_err());
    let mut h = IdentityHasher::new(2).unwrap();
    h.push([1; 16], true, [2; 32]).unwrap();
    assert!(h.push([1; 16], true, [2; 32]).is_err());
    assert!(InventoryHasher::new(1).unwrap().finish().is_err());
}

#[test]
fn standalone_admission_hash_has_explicit_finite_preseal_quote() {
    let value = json::parse(include_bytes!(
        "../../../tests/source7-framing-fixtures/framing-vectors.json"
    ))
    .unwrap();
    for row in value.get("vectors").unwrap().as_array().unwrap() {
        let frame = row.get("frames").unwrap().get("admission").unwrap();
        let literal = unhex(frame.get("hex").unwrap().as_str().unwrap());
        // Independently frozen binary preimage length supplies the source-byte
        // count, not another invocation of Admission7::digest or its encoder.
        assert_eq!(admission_work_quote(), 4096 + 16 * literal.len() as u64);
    }
    let r = Requirements7::new(Requirements::default(), appearance_registration(), &|| {
        Ok(())
    })
    .unwrap();
    assert!(framing_work_quote(&r).unwrap() > admission_work_quote());
}

#[test]
fn authority_origin_is_bound_even_when_mode_and_all_numeric_limits_match() {
    let captured = grant(false);
    let standalone = Admission7 {
        work_authority: Origin7::StandaloneViewJobs,
        ..captured
    };
    assert_ne!(captured.digest().unwrap(), standalone.digest().unwrap());
    assert_eq!(
        captured
            .value()
            .unwrap()
            .get("workAuthority")
            .unwrap()
            .as_str(),
        Some("captured-job")
    );
    assert_eq!(
        standalone
            .value()
            .unwrap()
            .get("workAuthority")
            .unwrap()
            .as_str(),
        Some("standalone-view-jobs")
    );
}
