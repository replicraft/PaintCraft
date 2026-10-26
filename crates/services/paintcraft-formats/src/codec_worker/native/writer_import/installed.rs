//! Called only inside the actual copied private schema3 installation fixture.
use super::*;
use paintcraft_runtime::process;
use std::{
    cell::{Cell, RefCell},
    fs,
};
fn pixels(document: &paintcraft_document::Document, expected: &[u8]) {
    assert_eq!(
        expected.len(),
        document.canvas.0 as usize * document.canvas.1 as usize * 4
    );
    let layer = document.layers_in_order().next().unwrap();
    for (index, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer
                .raster
                .get(
                    (index % document.canvas.0 as usize) as i64,
                    (index / document.canvas.0 as usize) as i64
                )
                .0,
            *pixel
        );
    }
}
pub(super) fn qualify(root: &Path) {
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let mut cases = Vec::new();
    for (width, height) in [(1, 1), (4, 3), (17, 19), (33, 9)] {
        let input = root.join(format!("parent-{width}x{height}.jxr"));
        let rgb = fs::read(root.join(format!("parent-{width}x{height}.rgb"))).unwrap();
        cases.push((input, rgb));
    }
    for (name, bytes) in [
        (
            "long-wide",
            include_bytes!("../../../../tests/jxr-fixtures/white-rgb8-4096x1.jxr").as_slice(),
        ),
        (
            "long-tall",
            include_bytes!("../../../../tests/jxr-fixtures/white-rgb8-1x4096.jxr").as_slice(),
        ),
    ] {
        let input = root.join(format!("{name}.jxr"));
        fs::write(&input, bytes).unwrap();
        cases.push((input, vec![255; 4096 * 3]));
    }
    // Explicit immutable corrected-encoder/C artifacts, never ambient fixtures.
    let matrix = PathBuf::from(
        std::env::var_os("PAINTCRAFT_JXR_WRITER_MATRIX")
            .expect("explicit retained corrected encoder/C matrix"),
    )
    .join("matrix-large");
    for (name, encoded_sha, rgb_sha) in [
        (
            "edge-65536x1",
            "ecd8f7d26137937c9f46860b893faa28b7711609987469484a3a922e8cc5c89e",
            "fc09a8b3f419cd6371e1a202650c2adbcd0b455190bfcb20554e274c5e7927a7",
        ),
        (
            "edge-1x65536",
            "d45b792d5d3a53e74376b10de6fb3c1ca9296bcf522b7ee4c4066bfab74c2302",
            "fc09a8b3f419cd6371e1a202650c2adbcd0b455190bfcb20554e274c5e7927a7",
        ),
        (
            "gradient-1024x1024",
            "e33b01163893f2befc02c0192ee1328a7b4b862b632aa10fa3e329ce9f57c386",
            "27195fc17c2f64c20ca529715545bc012f9cc9f8235b36ff89b2e46461a41f9c",
        ),
    ] {
        let encoded = fs::read(matrix.join(format!("{name}.jxr"))).unwrap();
        let rgb = fs::read(matrix.join(format!("{name}.c.rgb"))).unwrap();
        assert_eq!(hex(&Sha256::digest(&encoded)), encoded_sha);
        assert_eq!(hex(&Sha256::digest(&rgb)), rgb_sha);
        let input = root.join(format!("{name}.jxr"));
        fs::write(&input, encoded).unwrap();
        cases.push((input, rgb));
    }
    for (input, rgb) in cases {
        let raw = fs::read(&input).unwrap();
        let expected = rgb
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect::<Vec<_>>();
        let source = crate::jxr::assess_writer_import(&raw, 1_048_576, &|| Ok(()))
            .unwrap()
            .source();
        let tiles_x = u64::from(source.width).checked_add(255).unwrap() / 256;
        let tiles_y = u64::from(source.height).checked_add(255).unwrap() / 256;
        let resident_quote = tiles_x
            .checked_mul(tiles_y)
            .unwrap()
            .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
            .unwrap();
        let resident = SharedBudget::new(resident_quote);
        let quote = reservation_bytes_for_input(raw.len() as u64).unwrap();
        let budget = SharedBudget::new(quote);
        let current = Cell::new(owner);
        let backing = RefCell::new(None::<PathBuf>);
        let job = || Job {
            input: input.clone(),
            identity: SourceIdentity::existing_destination(&input)
                .unwrap()
                .unwrap(),
            length: raw.len() as u64,
            codec: Codec::JxrRgb8,
            owner,
            pixels: 1_048_576,
            deadline: Duration::from_secs(20),
            application_version: env!("CARGO_PKG_VERSION").into(),
            expected_sha256: Some(hex(&Sha256::digest(&raw))),
        };
        let prepare = |stale, residency: SharedBudget| {
            job().prepare_jxr_writer(
                residency,
                ResourceLease::new(budget.reserve(quote).unwrap()),
                &|| Ok(()),
                &|| Ok(current.get()),
                |context, check| {
                    assert_eq!(context.arguments[0], "--jxr-writer-source");
                    let private = PathBuf::from(&context.arguments[1]);
                    assert_eq!(fs::read(&private).unwrap(), raw);
                    *backing.borrow_mut() = Some(private.parent().unwrap().to_owned());
                    let output = process::run_parent_lease(
                        context.executable,
                        context.arguments,
                        context.request.segments(),
                        process::Limits {
                            input_bytes: INPUT_BYTES_MAX + REQUEST_HEADER,
                            output_bytes: OUTPUT_BYTES_MAX,
                            deadline: context.remaining,
                            resource_exit_code: Some(RESOURCE_EXIT),
                        },
                        check,
                    )?;
                    assert!(output.status.success());
                    if stale {
                        let mut changed = owner;
                        changed.epoch += 1;
                        current.set(changed);
                    }
                    Ok(output.bytes)
                },
            )
        };
        let tiny = SharedBudget::new(1);
        assert_eq!(
            prepare(false, tiny.clone()).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(tiny.usage().reserved, 0);
        assert_eq!(budget.usage().reserved, 0);
        assert!(!input.with_extension("pcnd").exists());
        let prepared = prepare(false, resident.clone()).unwrap();
        assert!(!backing.borrow().as_ref().unwrap().exists());
        assert_eq!(prepared.decoder_profile(), candidate::WRITER_IMPORT_VARIANT);
        assert_eq!(prepared.losses(), &["jxr-resolution-not-retained"]);
        assert_eq!(budget.usage().reserved, quote);
        pixels(prepared.document(), &expected);
        let allowed = vec!["jxr-resolution-not-retained".into()];
        let adopted = prepared
            .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
            .unwrap();
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(resident.usage().reserved, resident_quote);
        eprintln!(
            "writer-import {} source={} rgba={} resident={resident_quote} transient={quote}",
            input.file_name().unwrap().to_string_lossy(),
            hex(&Sha256::digest(&raw)),
            hex(&Sha256::digest(&expected))
        );
        let output = input.with_extension("pcnd");
        let bytes = crate::pcnd::write(&adopted.document).unwrap();
        let result =
            paintcraft_platform::save::save_with(&output, false, bytes.len() as u64, |sink| {
                sink.write_all(&bytes)
                    .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))
            });
        assert!(result.is_success());
        pixels(
            &crate::pcnd::read(&fs::read(&output).unwrap()).unwrap(),
            &expected,
        );
        drop(adopted);
        assert_eq!(resident.usage().reserved, 0);
        assert_eq!(
            prepare(true, resident.clone()).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert_eq!(resident.usage().reserved, 0);
        assert!(!backing.borrow().as_ref().unwrap().exists());
        assert_eq!(budget.usage().reserved, 0);
        current.set(owner);
        // An exact schema3 installation is never a fifth legacy decode choice.
        assert_eq!(
            job()
                .consume(
                    &|| Ok(()),
                    &|| Ok(owner),
                    |_, _| panic!("no default schema3 launch"),
                    |_, _, _, _| Ok(())
                )
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(fs::read(&input).unwrap(), raw);
    }
}
