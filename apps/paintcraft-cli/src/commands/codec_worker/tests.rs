use super::*;
use std::fs;
#[test]
fn stale_owner_and_cancel_keep_existing_runtime_admission_until_consumption() {
    let root = std::env::temp_dir().join(format!("paintcraft-codec-owner-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let input = root.join("source");
    fs::write(&input, b"source bytes").unwrap();
    for cancelled in [false, true] {
        let identity = SourceIdentity::existing_destination(&input)
            .unwrap()
            .unwrap();
        let captured = owner();
        let mut changed = captured;
        changed.epoch += 1;
        let options = Options {
            input: input.clone(),
            codec: Codec::Jxl,
            pixels: 1,
            deadline_ms: 10000,
            memory_limit: 512 << 20,
            expect_input_sha256: None,
        };
        let charge = wire::reservation_bytes_for_input(12).unwrap() + VERIFICATION_SLACK;
        let mut runtime = Executor::new(Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: charge,
        })
        .unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let handle = runtime
            .submit(
                charge,
                move |cancel| {
                    receive.recv().unwrap();
                    inspect(options, identity, 12, captured, cancel, || Ok(changed))
                },
                || {},
            )
            .unwrap();
        if cancelled {
            assert!(runtime.cancel(handle));
        }
        assert_eq!(runtime.reserved_bytes(), charge);
        send.send(()).unwrap();
        let result = loop {
            if let Some(result) = runtime.poll_for(handle) {
                break result;
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(
            result.unwrap_err().code,
            if cancelled {
                ErrorCode::OperationCancelled
            } else {
                ErrorCode::ConflictStaleBase
            }
        );
        assert_eq!(runtime.reserved_bytes(), 0);
    }
    fs::remove_dir_all(root).unwrap();
}
