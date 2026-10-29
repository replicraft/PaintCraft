//! Actual same-installation float jobs, not in-process decoder substitutes.
use super::*;
use paintcraft_runtime::process;
use std::fs;

#[test]
#[ignore = "requires the test executable beside the verified same-build native helper"]
fn installed_float_settings_have_distinct_stable_converted_document_identities() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let source = root.join(format!("identity-{}.hdr", Uuid::new_v4()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(source.clone());
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let raw = fs::read(fixtures.join("radiance-fixtures/literal-2x2.hdr")).unwrap();
    fs::write(&source, &raw).unwrap();
    let sha: [u8; 32] = Sha256::digest(&raw).into();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let residency = SharedBudget::new(float_max_resident_bytes().unwrap());
    let metadata = SharedBudget::new(8 << 20);
    let transient = SharedBudget::new(float_reservation_bytes(raw.len() as u64).unwrap());
    let make = |curve, negative| {
        let job = Job {
            input: source.clone(),
            identity: SourceIdentity::from_file(fs::File::open(&source).unwrap()).unwrap(),
            length: raw.len() as u64,
            codec: Codec::HdrSdr8,
            owner,
            pixels: super::super::super::float::PIXELS_MAX,
            deadline: Duration::from_secs(10),
            application_version: env!("CARGO_PKG_VERSION").into(),
            expected_sha256: Some(hex(&sha)),
        };
        job.prepare_float(
            precision::Settings {
                curve,
                negative,
                ..precision::Settings::default()
            },
            residency.clone(),
            metadata.clone(),
            ResourceLease::new(
                transient
                    .reserve(float_reservation_bytes(raw.len() as u64).unwrap())
                    .unwrap(),
            ),
            &|| Ok(()),
            &|| Ok(owner),
            |_, _| Ok(()),
            |launch, check| {
                let output = process::run_parent_lease(
                    launch.executable,
                    launch.arguments,
                    launch.request.segments(),
                    process::Limits {
                        input_bytes: super::super::super::float::INPUT_BYTES_MAX + REQUEST_HEADER,
                        output_bytes: super::super::super::float::OUTPUT_BYTES_MAX,
                        deadline: launch.remaining,
                        resource_exit_code: Some(RESOURCE_EXIT),
                    },
                    check,
                )?;
                if !output.status.success() {
                    return Err(invalid("Actual float helper failed"));
                }
                Ok(output.bytes)
            },
        )
        .unwrap()
    };
    let mut docs = Vec::new();
    for (curve, negative, expected) in [
        (
            precision::Curve::Clip,
            precision::Negative::Reject,
            "hdr-clip.rgba",
        ),
        (
            precision::Curve::ReinhardPerChannel,
            precision::Negative::Reject,
            "hdr-reinhard.rgba",
        ),
        (
            precision::Curve::ReinhardPerChannel,
            precision::Negative::Reject,
            "hdr-reinhard.rgba",
        ),
        (
            precision::Curve::Clip,
            precision::Negative::ClipToBlack,
            "hdr-clip.rgba",
        ),
    ] {
        let prepared = make(curve, negative);
        let expected = fs::read(fixtures.join("float-import-fixtures").join(expected)).unwrap();
        for (index, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
            let doc = prepared.document();
            assert_eq!(
                doc.layers[&doc.roots[0]]
                    .raster
                    .get((index % 2) as i64, (index / 2) as i64)
                    .0,
                *pixel
            );
        }
        let digest = prepared.float_assessment().unwrap().digest();
        let allowed: Vec<_> = prepared
            .losses()
            .iter()
            .map(|value| value.to_string())
            .collect();
        let adopted = prepared
            .adopt_float(digest, &allowed, &|| Ok(()), &|| Ok(owner))
            .unwrap();
        assert_eq!(
            adopted.sha256, sha,
            "original source identity remains unchanged"
        );
        docs.push(adopted.document.clone());
    }
    assert_ne!(docs[0].document_id, docs[1].document_id);
    assert_ne!(docs[0].state_id, docs[1].state_id);
    assert_ne!(docs[0].roots, docs[1].roots);
    assert_eq!(docs[1].document_id, docs[2].document_id);
    assert_eq!(docs[1].state_id, docs[2].state_id);
    assert_eq!(docs[1].roots, docs[2].roots);
    // Equal pixels under different authored policies still have distinct provenance.
    assert_ne!(docs[0].document_id, docs[3].document_id);
    assert_ne!(docs[0].state_id, docs[3].state_id);
    assert_eq!(fs::read(&source).unwrap(), raw);
    drop(docs);
    assert_eq!(residency.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, 0);
    assert_eq!(transient.usage().reserved, 0);
}
