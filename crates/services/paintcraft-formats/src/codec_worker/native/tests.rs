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

#[test]
fn sdr_job_does_not_dispatch_without_an_exact_installed_v6_contract() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-missing-sdr-install-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let current = std::env::current_exe().unwrap();
    let child = root.join(current.file_name().unwrap());
    std::fs::copy(&current, &child).unwrap();
    // Normalize only the isolated Linux debug fixture, retaining real limits.
    #[cfg(all(target_os = "linux", debug_assertions))]
    {
        let output = std::process::Command::new("strip")
            .arg("--strip-debug")
            .arg(&child)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let entries: Vec<_> = std::fs::read_dir(&root).unwrap().collect();
    assert_eq!(
        entries.len(),
        1,
        "missing-installation fixture contains only its executable"
    );
    let output = std::process::Command::new(&child)
        .args([
            "--exact",
            "codec_worker::native::tests::fixture_missing_sdr_installation",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "copied executable bytes={}: {} {}",
        std::fs::metadata(&child).unwrap().len(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "isolated missing-installation fixture invoked by parent"]
fn fixture_missing_sdr_installation() {
    let root = std::env::temp_dir().join(format!("paintcraft-private-sdr-job-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("source.avif");
    std::fs::write(&path, b"source").unwrap();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 1,
        frontend: 3,
    };
    for stale in [false, true] {
        let job = Job {
            input: path.clone(),
            identity: SourceIdentity::existing_destination(&path)
                .unwrap()
                .unwrap(),
            length: 6,
            codec: Codec::AvifSdr8,
            owner,
            pixels: 1,
            deadline: Duration::from_secs(1),
            application_version: env!("CARGO_PKG_VERSION").into(),
            expected_sha256: None,
        };
        let mut current = owner;
        current.epoch += u64::from(stale);
        let error = job
            .consume::<()>(
                &|| Ok(()),
                &|| Ok(current),
                |_, _| panic!("SDR job without a V6 installation must not launch"),
                |_, _, _, _| panic!("SDR job without a V6 installation must not consume"),
            )
            .unwrap_err();
        assert_eq!(
            error.code,
            if stale {
                ErrorCode::ConflictStaleBase
            } else {
                ErrorCode::CapabilityUnsupported
            },
            "{error:?}; executable bytes={}",
            std::fs::metadata(std::env::current_exe().unwrap())
                .unwrap()
                .len()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"source");
    }
    std::fs::remove_dir_all(root).unwrap();
}
