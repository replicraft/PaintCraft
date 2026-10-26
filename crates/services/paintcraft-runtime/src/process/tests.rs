use super::*;
fn args(name: &str) -> Vec<OsString> {
    ["--exact", name, "--ignored", "--nocapture"]
        .into_iter()
        .map(OsString::from)
        .collect()
}
fn limits() -> Limits {
    Limits {
        input_bytes: 1 << 20,
        output_bytes: 1 << 20,
        deadline: Duration::from_secs(2),
        resource_exit_code: Some(75),
    }
}
#[test]
fn successful_output_is_private_and_input_is_frozen() {
    let data = Arc::new(vec![9; 100]);
    let out = run(
        &std::env::current_exe().unwrap(),
        &args("process::tests::child_echo"),
        vec![data.clone()],
        limits(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(out.status.success());
    assert!(out.bytes.windows(100).any(|v| v == data.as_slice()));
    assert_eq!(*data, vec![9; 100]);
}
#[test]
fn deadline_cancel_oversize_and_child_panic_never_return_partial_success() {
    let exe = std::env::current_exe().unwrap();
    let start = Instant::now();
    let short = Limits {
        deadline: Duration::from_millis(40),
        ..limits()
    };
    assert_eq!(
        run(
            &exe,
            &args("process::tests::child_stall"),
            Vec::new(),
            short,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    let start = Instant::now();
    assert_eq!(
        run(
            &exe,
            &args("process::tests::child_stall"),
            Vec::new(),
            limits(),
            &|| if start.elapsed() > Duration::from_millis(40) {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
            } else {
                Ok(())
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(
        run(
            &exe,
            &args("process::tests::child_flood"),
            Vec::new(),
            Limits {
                output_bytes: 64,
                ..limits()
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        run(
            &exe,
            &args("process::tests::child_panic"),
            Vec::new(),
            limits(),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::CodecFailed
    );
}
#[test]
fn blocked_input_writer_is_released_when_cancel_kills_and_reaps() {
    let exe = std::env::current_exe().unwrap();
    let start = Instant::now();
    assert!(
        run(
            &exe,
            &args("process::tests::child_stall"),
            vec![Arc::new(vec![1; 1 << 20])],
            Limits {
                deadline: Duration::from_millis(40),
                ..limits()
            },
            &|| Ok(())
        )
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}
#[test]
#[ignore = "child protocol fixture invoked only by process tests"]
fn child_echo() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    std::io::stdout().write_all(&input).unwrap();
}
#[test]
#[ignore = "child protocol fixture invoked only by process tests"]
fn child_stall() {
    thread::sleep(Duration::from_secs(60));
}
#[test]
#[ignore = "child protocol fixture invoked only by process tests"]
fn child_flood() {
    std::io::stdout().write_all(&[1; 65536]).unwrap();
}
#[test]
#[ignore = "child protocol fixture invoked only by process tests"]
fn child_panic() {
    panic!("worker panic fixture");
}

#[test]
fn resource_exit_discards_partial_output_even_when_input_pipe_breaks() {
    let error = run(
        &std::env::current_exe().unwrap(),
        &args("process::tests::child_resource_exit"),
        vec![Arc::new(vec![0; 1 << 20])],
        limits(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
}
#[test]
#[ignore = "fixture invoked by subprocess supervisor tests"]
fn child_resource_exit() {
    std::io::stdout()
        .write_all(b"partial invalid output")
        .unwrap();
    std::process::exit(75);
}

#[test]
#[cfg(unix)]
fn closed_pipes_do_not_busy_spin_while_the_child_is_still_alive() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let polls = AtomicUsize::new(0);
    // Replace the shell with the same process's sleep image after closing its
    // pipes. No descendant retains a pipe or survives supervisor termination.
    let args = [
        OsString::from("-c"),
        OsString::from("exec 0<&-; exec 1>&-; exec /bin/sleep 5"),
    ];
    let error = run(
        Path::new("/bin/sh"),
        &args,
        Vec::new(),
        Limits {
            deadline: Duration::from_millis(60),
            ..limits()
        },
        &|| {
            polls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(
        polls.load(Ordering::Relaxed) < 100,
        "disconnected pipe busy loop"
    );
}
