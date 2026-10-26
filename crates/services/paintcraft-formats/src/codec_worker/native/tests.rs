use super::*;
#[test]
fn stale_owner_and_cancel_are_rejected_before_source_or_helper_access() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-native-dispatch-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("source");
    std::fs::write(&path, b"source").unwrap();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    for cancelled in [false, true] {
        let job = Job {
            input: path.clone(),
            identity: SourceIdentity::existing_destination(&path)
                .unwrap()
                .unwrap(),
            length: 6,
            codec: Codec::JxlRgba8,
            owner,
            pixels: 1,
            deadline: Duration::from_secs(1),
            application_version: env!("CARGO_PKG_VERSION").into(),
            expected_sha256: None,
        };
        let mut foreign = owner;
        foreign.epoch += 1;
        let error = job
            .consume::<()>(
                &|| {
                    if cancelled {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                    } else {
                        Ok(())
                    }
                },
                &|| Ok(foreign),
                |_, _| panic!("stale job must not launch"),
                |_, _, _, _| panic!("stale job must not consume"),
            )
            .unwrap_err();
        assert_eq!(
            error.code,
            if cancelled {
                ErrorCode::OperationCancelled
            } else {
                ErrorCode::ConflictStaleBase
            }
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_avif_host_build_contract_cannot_launch_or_adopt() {
    let path = std::env::temp_dir().join(format!("paintcraft-avif-v4-{}", Uuid::new_v4()));
    std::fs::write(&path, b"source").unwrap();
    let identity = SourceIdentity::existing_destination(&path)
        .unwrap()
        .unwrap();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let job = Job {
        input: path.clone(),
        identity,
        length: 6,
        codec: Codec::AvifSrgb8,
        owner,
        pixels: 1,
        deadline: Duration::from_secs(1),
        application_version: String::new(),
        expected_sha256: None,
    };
    let error = job
        .consume::<()>(
            &|| Ok(()),
            &|| Ok(owner),
            |_, _| panic!("invalid host contract cannot launch AVIF"),
            |_, _, _, _| panic!("invalid host contract cannot publish AVIF"),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    std::fs::remove_file(path).unwrap();
}
