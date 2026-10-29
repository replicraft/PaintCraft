use super::*;

#[test]
fn independent_bmp_profiles_density_models_and_hidden_pixels_are_retained_exactly() {
    let oracle = json::parse(&own("oracle.json")).unwrap();
    let mut cases = 0;
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = own(name);
        let original = bytes.clone();
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let budget = SharedBudget::new(1 << 20);
        let captured =
            prepared(&bytes, &budget, &|| Ok(())).unwrap_or_else(|error| panic!("{name}: {error}"));
        let expected = numbers(row, "expected_rgba8");
        assert_eq!(captured.image.rgba, expected, "{name}");
        assert_eq!(captured.image.rgba, own(&format!("{name}.rgba")));
        assert_eq!(
            captured.image.rgba,
            super::super::super::decode_with_pixel_limit(&bytes, PIXELS)
                .unwrap()
                .rgba
        );
        let source = bmp_metadata::inspect(&bytes, PIXELS, &|| Ok(())).unwrap();
        let icc = row.get("profile").unwrap().as_bool().unwrap();
        let density: Vec<i64> = row
            .get("density")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        let retained_density = density != [0, 0];
        assert_eq!(
            captured.attachment.is_some(),
            icc || retained_density,
            "{name}"
        );
        assert_eq!(
            captured
                .image
                .losses
                .iter()
                .any(|s| s == ICC_RETAINED_UNAPPLIED),
            icc
        );
        assert_eq!(
            captured
                .image
                .losses
                .iter()
                .any(|s| s == RESOLUTION_RETAINED_INACTIVE),
            retained_density
        );
        assert!(
            !captured
                .image
                .losses
                .iter()
                .any(|s| s == "icc-profile-ignored" || s == "resolution-metadata-ignored")
        );
        assert_eq!(
            source.bits_per_pixel,
            row.get("bits_per_pixel").unwrap().as_i64().unwrap() as u16
        );
        assert_eq!(source.palette_reserved, numbers(row, "palette_reserved"));
        if source.bits_per_pixel <= 8 {
            assert!(source.palette.alpha.is_empty());
            assert!(
                captured
                    .image
                    .losses
                    .iter()
                    .any(|s| s == "palette-indices-expanded")
            );
        }
        assert_eq!(
            captured
                .image
                .losses
                .iter()
                .any(|s| s == "bmp-color-metadata-ignored"),
            source.dib_bytes >= 108
        );
        if let Some(meta) = &captured.attachment {
            let descriptor = meta.descriptor();
            assert_eq!(descriptor.format, SourceFormat::Bmp);
            assert_eq!(
                descriptor.source_sha256,
                <[u8; 32]>::from(Sha256::digest(&bytes))
            );
            assert_eq!(descriptor.source_bytes, bytes.len() as u64);
            assert_eq!(descriptor.source_dimensions, (3, 2));
            assert_eq!(descriptor.output_dimensions, (3, 2));
            assert_eq!(descriptor.orientation, None);
            assert_eq!(descriptor.selection, None);
            assert_eq!(meta.working_color(), WorkingColor::UnmanagedRgba8);
            assert_eq!(
                descriptor.source_model,
                if source.bits_per_pixel <= 8 {
                    SourceColorModel::Indexed
                } else {
                    SourceColorModel::Rgb
                }
            );
            if retained_density {
                let resolution = descriptor.resolution.unwrap();
                assert_eq!(
                    [
                        resolution.horizontal.numerator,
                        resolution.vertical.numerator
                    ],
                    density.as_slice()
                );
                assert_eq!(
                    (
                        resolution.horizontal.denominator,
                        resolution.vertical.denominator
                    ),
                    (1, 1)
                );
                assert_eq!(
                    resolution.unit,
                    paintcraft_document::source_metadata::ResolutionUnit::Metre
                );
                assert_eq!(resolution.raw_unit, None);
                assert_eq!(
                    resolution.physical().is_some(),
                    density.iter().all(|n| *n > 0)
                );
            } else {
                assert!(descriptor.resolution.is_none());
            }
            if icc {
                assert_eq!(meta.profile().unwrap(), original_profile(&bytes));
                assert_eq!(
                    hex(&Sha256::digest(meta.profile().unwrap())),
                    row.get("profile_sha256").unwrap().as_str().unwrap()
                );
            } else {
                assert!(meta.profile().is_none());
            }
            assert_eq!(budget.usage().reserved, meta.storage_usage().1);
        }
        let doc = document(&captured).unwrap();
        let back = crate::pcnd::read(&crate::pcnd::write_compact(&doc).unwrap()).unwrap();
        assert!(back.same_semantics(&doc));
        assert_eq!(back.source_metadata, doc.source_metadata);
        assert_eq!(
            prepared(&bytes, &SharedBudget::new(1 << 20), &|| Ok(()))
                .unwrap()
                .document_key,
            captured.document_key
        );
        assert_eq!(bytes, original);
        drop(doc);
        drop(back);
        drop(captured);
        assert_eq!(budget.usage().reserved, 0);
        cases += 1;
    }
    assert_eq!(cases, 13);
}

#[test]
fn existing_packed16_and_rle_profiles_keep_literal_samples_and_omission_truth() {
    let mut cases = 0;
    for folder in ["bmp16-fixtures", "bmp-rle-fixtures"] {
        let oracle = json::parse(&fixture(folder, "oracle.json")).unwrap();
        for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
            let name = row.get("file").unwrap().as_str().unwrap();
            let bytes = fixture(folder, name);
            let capture = prepared(&bytes, &SharedBudget::new(1 << 20), &|| Ok(()))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let expected = if folder == "bmp16-fixtures" {
                fixture(folder, &name.replace(".bmp", ".rgba"))
            } else {
                numbers(row, "expected_rgba")
            };
            assert_eq!(capture.image.rgba, expected, "{name}");
            assert!(
                !capture
                    .image
                    .losses
                    .iter()
                    .any(|s| s == "depth-16-reduced-to-8"),
                "packed16bpp is not16bits per channel"
            );
            if folder == "bmp-rle-fixtures" {
                assert!(
                    capture
                        .image
                        .losses
                        .iter()
                        .any(|s| s == "palette-indices-expanded")
                );
                let source = bmp_metadata::inspect(&bytes, PIXELS, &|| Ok(())).unwrap();
                assert_eq!(
                    capture.attachment.as_ref().and_then(|m| m.profile()),
                    source.icc.as_ref().map(|_| original_profile(&bytes))
                );
            }
            cases += 1;
        }
    }
    assert!(cases >= 30);
}
