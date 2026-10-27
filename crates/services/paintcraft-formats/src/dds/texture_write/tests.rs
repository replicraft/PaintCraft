use super::*;
use std::cell::Cell;
fn options(encoding: Encoding) -> TextureOptions {
    TextureOptions {
        encoding,
        transfer: Transfer::Linear,
        alpha: if encoding == Encoding::Bc1 {
            SourceAlpha::Opaque
        } else {
            SourceAlpha::Straight
        },
        mips: MipCount::Full,
        filter: MipFilter::BoxStored,
        quality: TextureQuality::Normal,
    }
}
#[test]
fn exact_area_alpha_and_nearest_hidden_rgb_are_distinct_policies() {
    let source = [255, 0, 0, 255, 19, 217, 31, 0, 0, 0, 255, 255];
    let target = MipLevel {
        width: 1,
        height: 1,
        byte_offset: 0,
        byte_len: 4,
    };
    assert_eq!(
        filter::generate(&source, 3, 1, &target, MipFilter::BoxStored, &|| Ok(())).unwrap(),
        [128, 0, 128, 170]
    );
    assert_eq!(
        filter::generate(&source, 3, 1, &target, MipFilter::Nearest, &|| Ok(())).unwrap(),
        [19, 217, 31, 0]
    );
    let invisible = [21, 57, 113, 0, 244, 72, 63, 0];
    assert_eq!(
        filter::generate(&invisible, 2, 1, &target, MipFilter::BoxStored, &|| Ok(())).unwrap(),
        [0, 0, 0, 0]
    );
}
#[test]
fn complete_skinny_chain_prices_all_blocks_and_not_four_thirds() {
    let plan = TexturePlan::new(1, 65536, options(Encoding::Bc3)).unwrap();
    assert_eq!(plan.levels().len(), 17);
    assert_eq!(plan.levels().last().unwrap().height, 1);
    assert_eq!(plan.levels().last().unwrap().byte_len, 16);
    assert_eq!(plan.encoded_bytes(), 148 + 524_304);
    assert!(plan.reservation_bytes().unwrap() > plan.source_bytes() + plan.encoded_bytes());
    for pair in plan.levels().windows(2) {
        assert_eq!(pair[0].byte_offset + pair[0].byte_len, pair[1].byte_offset);
    }
}
#[test]
fn refusal_and_admission_precede_encoder_or_source_scanning() {
    let calls = Cell::new(0);
    let rejected = Error::new(ErrorCode::ResourceLimit, "caller denied reservation");
    let source = [17, 29, 53, 0];
    let error = encode_texture_checked(
        1,
        1,
        &source,
        options(Encoding::Bc1),
        |_| {
            calls.set(calls.get() + 1);
            Err(rejected.clone())
        },
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, rejected.code);
    assert_eq!(error.message, rejected.message);
    assert_eq!(calls.get(), 1);
    let error =
        encode_texture_checked(
            1,
            1,
            &source,
            options(Encoding::Bc1),
            |_| Ok(()),
            &|| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    let mut opts = options(Encoding::Bc1);
    opts.alpha = SourceAlpha::Straight;
    assert!(TexturePlan::new(1, 1, opts).is_err());
    for (w, h) in [(0, 1), (1, 0), (65537, 1), (4096, 4096)] {
        assert!(TexturePlan::new(w, h, options(Encoding::Bc3)).is_err());
    }
    let mut opts = options(Encoding::Rgba8);
    opts.mips = MipCount::Levels(4);
    assert!(TexturePlan::new(7, 5, opts).is_err());
    opts.mips = MipCount::Levels(0);
    assert!(TexturePlan::new(7, 5, opts).is_err());
}
#[test]
fn single_level_rgba_matches_existing_writer_and_base_bytes_remain_immutable() {
    let pixels = [19, 57, 117, 0, 1, 31, 217, 127, 255, 19, 83, 255];
    let opts = TextureOptions {
        mips: MipCount::Levels(1),
        transfer: Transfer::Srgb,
        ..options(Encoding::Rgba8)
    };
    let result = encode_texture_checked(3, 1, &pixels, opts, |_| Ok(()), &|| Ok(())).unwrap();
    assert_eq!(
        result.bytes(),
        encode_rgba8_checked(3, 1, &pixels, Transfer::Srgb, &|| Ok(())).unwrap()
    );
    assert!(result.plan().losses().is_empty());
    assert_eq!(&result.bytes()[148..], pixels);
}
#[test]
fn encoding_declares_actual_dxgi_alpha_levels_and_compression_loss() {
    for encoding in [Encoding::Rgba8, Encoding::Bc1, Encoding::Bc2, Encoding::Bc3] {
        let pixels = [23, 57, 91, 255].repeat(35);
        let opts = TextureOptions {
            transfer: Transfer::Srgb,
            ..options(encoding)
        };
        let encoded = encode_texture_checked(7, 5, &pixels, opts, |_| Ok(()), &|| Ok(())).unwrap();
        let info = inspect_top_mip_checked(encoded.bytes(), 35, &|| Ok(())).unwrap();
        assert_eq!(info.image.encoding, encoding);
        assert_eq!(info.image.transfer, Transfer::Srgb);
        assert_eq!(info.image.alpha, opts.alpha);
        assert_eq!(info.levels, encoded.plan().levels());
        assert_eq!(
            info.levels
                .iter()
                .map(|l| (l.width, l.height))
                .collect::<Vec<_>>(),
            [(7, 5), (3, 2), (1, 1)]
        );
        assert_eq!(
            !encoded.plan().losses().is_empty(),
            encoding != Encoding::Rgba8
        );
        assert_eq!(
            encoded.compression_policy(),
            (encoding != Encoding::Rgba8).then_some(UNIFORM_LOSSY_POLICY)
        );
    }
}
#[test]
fn cancellation_in_area_work_and_encoder_drops_private_output() {
    let source = [29, 59, 89, 127].repeat(65 * 17);
    let original = source.clone();
    for until in [0, 8, 25] {
        let calls = Cell::new(0);
        let check = || {
            let n = calls.get();
            calls.set(n + 1);
            if n >= until {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "bounded cancellation",
                ))
            } else {
                Ok(())
            }
        };
        let error =
            encode_texture_checked(65, 17, &source, options(Encoding::Bc3), |_| Ok(()), &check)
                .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(source, original);
    }
}

#[test]
fn actual_hidden_rgb_loss_depends_on_source_and_selected_filter() {
    let source = [17, 43, 91, 0, 255, 0, 0, 255];
    for filter in [MipFilter::Nearest, MipFilter::BoxStored] {
        let opts = TextureOptions {
            filter,
            ..options(Encoding::Rgba8)
        };
        let output = encode_texture_checked(2, 1, &source, opts, |_| Ok(()), &|| Ok(())).unwrap();
        assert_eq!(&output.bytes()[148..156], source);
        assert_eq!(
            output.losses().contains(&MIP_HIDDEN_RGB_LOSS),
            filter == MipFilter::BoxStored
        );
    }
}
