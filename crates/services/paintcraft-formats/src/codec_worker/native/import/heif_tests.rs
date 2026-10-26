use super::*;
use paintcraft_editor::session::Session;
const SOURCE: &[u8] =
    include_bytes!("../../../../tests/heif-color-fixtures/yuv420-full-alpha.heic");
fn prepared(
    resident: SharedBudget,
    preparation: SharedBudget,
    path: &Path,
    owner: Owner,
) -> PreparedImport {
    let reference =
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-full.rational-rgb");
    let rgba: Vec<_> = reference
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .flat_map(|(i, pixel)| {
            [
                pixel[0],
                pixel[1],
                pixel[2],
                [0, 1, 127, 128, 254, 255][((i % 32) + (i / 32)) % 6],
            ]
        })
        .collect();
    let mut admission = Admission::with_residency(PIXELS_MAX, resident);
    PreparedImport {
        document: paintcraft_document::import_image_with_admission_checked(
            32,
            24,
            &rgba,
            "HEIF source",
            &mut admission,
            &|| Ok(()),
        )
        .unwrap(),
        input: path.to_owned(),
        identity: SourceIdentity::existing_destination(path).unwrap().unwrap(),
        length: SOURCE.len() as u64,
        sha256: Sha256::digest(SOURCE).into(),
        owner,
        profile: PreparedProfile::Heif(
            crate::heif::assess_import(SOURCE, PIXELS_MAX, &|| Ok(())).unwrap(),
        ),
        _lease: ResourceLease::new(preparation.reserve(HEAP_BYTES).unwrap()),
    }
}
#[test]
fn heif_consent_source_owner_and_resident_handoff_preserve_existing_session() {
    let root = std::env::temp_dir().join(format!("paintcraft-heif-adoption-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("source.heic");
    std::fs::write(&input, SOURCE).unwrap();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let resident = SharedBudget::new(1 << 20);
    let preparation = SharedBudget::new(HEAP_BYTES);
    let current = Session::new(
        paintcraft_document::new_document(1, 1, None, 0, paintcraft_document::POLICY).unwrap(),
    );
    let ticket = current.ticket();
    let make = || prepared(resident.clone(), preparation.clone(), &input, owner);
    let pending = make();
    assert_eq!(pending.decoder_profile(), NATIVE_HEIF_IMPORT_VARIANT);
    assert!(pending.heif_assessment().unwrap().alpha());
    assert_eq!(
        pending.adopt(&|| Ok(()), &|| Ok(owner)).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(preparation.usage().reserved, 0);
    let allowed = vec!["heif-color-reconstruction".into()];
    assert_eq!(
        make()
            .adopt_with_losses(&["unknown".into()], &|| Ok(()), &|| Ok(owner))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let mut stale = owner;
    stale.epoch += 1;
    assert_eq!(
        make()
            .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(stale))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(
        make()
            .adopt_with_losses(
                &allowed,
                &|| Err(Error::new(ErrorCode::OperationCancelled, "cancel")),
                &|| Ok(owner)
            )
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(current.ticket(), ticket);
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(preparation.usage().reserved, 0);
    let adopted = make()
        .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
        .unwrap();
    let assessment = adopted.heif_assessment.unwrap();
    assert_eq!(assessment.sha256(), adopted.sha256);
    assert_eq!(assessment.color().matrix, 1);
    assert_eq!(preparation.usage().reserved, 0);
    assert!(resident.usage().reserved > 0);
    let snapshot = adopted.document.clone();
    drop(adopted.document);
    assert!(resident.usage().reserved > 0);
    drop(snapshot);
    assert_eq!(resident.usage().reserved, 0);
    let pending = make();
    std::fs::write(&input, b"changed").unwrap();
    assert!(
        pending
            .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
            .is_err()
    );
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(preparation.usage().reserved, 0);
    assert_eq!(current.ticket(), ticket);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn captured_heif_frame_geometry_hash_extent_and_opacity_are_checked_before_tiles() {
    let source = include_bytes!("../../../../tests/heif-color-fixtures/identity444.heic");
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let request = Request::capture_for(
        Codec::HeifSrgb8,
        std::sync::Arc::new(source.to_vec()),
        owner,
        PIXELS_MAX,
    )
    .unwrap();
    let mut rgba = vec![255; 32 * 24 * 4];
    fn frame(pixels: &[u8], width: u32) -> Frame<'_> {
        Frame {
            width,
            height: 24,
            pixels: Pixels::SrgbRgba8(pixels),
            heap_peak: 0,
        }
    }
    heif_profile(&request, &frame(&rgba, 32), &|| Ok(())).unwrap();
    assert!(heif_profile(&request, &frame(&rgba, 31), &|| Ok(())).is_err());
    assert!(heif_profile(&request, &frame(&rgba[..rgba.len() - 1], 32), &|| Ok(())).is_err());
    let mut changed = request.clone();
    changed.header.sha256[0] ^= 1;
    assert!(heif_profile(&changed, &frame(&rgba, 32), &|| Ok(())).is_err());
    rgba[3] = 0;
    assert!(heif_profile(&request, &frame(&rgba, 32), &|| Ok(())).is_err());
}
