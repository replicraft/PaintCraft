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

#[test]
fn callback_unwind_joins_blocked_pipe_workers_before_input_charge_can_drop() {
    use std::{
        panic::AssertUnwindSafe,
        sync::atomic::{AtomicUsize, Ordering},
    };
    let source = Arc::new(vec![1; 1 << 20]);
    let checks = AtomicUsize::new(0);
    let started = Instant::now();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = run(
            &std::env::current_exe().unwrap(),
            &args("process::tests::child_stall"),
            vec![source.clone()],
            limits(),
            &|| {
                if checks.fetch_add(1, Ordering::Relaxed) > 0 {
                    panic!("owner callback panic after pipe workers started");
                }
                Ok(())
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(
        Arc::strong_count(&source),
        1,
        "input writer outlived supervisor unwind"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn running_drop_reaps_child_before_joining_both_workers_on_unwind() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let completed = Arc::new(AtomicUsize::new(0));
    let result = std::panic::catch_unwind({
        let completed = completed.clone();
        move || {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(args("process::tests::child_stall"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut stdout = child.stdout.take().unwrap();
            let mut stdin = child.stdin.take().unwrap();
            let mut running = Running {
                child,
                reaped: false,
                writer: None,
                reader: None,
                stdin_lease: None,
            };
            let done = completed.clone();
            running.writer = Some(thread::spawn(move || {
                let _ = stdin.write_all(&vec![1; 1 << 20]);
                done.fetch_add(1, Ordering::Release);
            }));
            running.reader = Some(thread::spawn(move || {
                let _ = std::io::copy(&mut stdout, &mut std::io::sink());
                completed.fetch_add(1, Ordering::Release);
            }));
            panic!("unwind while both pipe workers are live");
        }
    });
    assert!(result.is_err());
    assert_eq!(completed.load(Ordering::Acquire), 2);
}

#[test]
fn retained_parent_lease_keeps_stdin_open_until_child_completes() {
    let output = run_parent_lease(
        &std::env::current_exe().unwrap(),
        &args("process::tests::child_exact_input"),
        vec![Arc::new(vec![42])],
        limits(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(
        output
            .bytes
            .windows(13)
            .any(|bytes| bytes == b"LEASE_IS_OPEN")
    );
}
#[test]
#[ignore = "bounded parent-lease transport fixture"]
fn child_exact_input() {
    let mut byte = [0];
    std::io::stdin().read_exact(&mut byte).unwrap();
    assert_eq!(byte, [42]);
    // A separate read remains blocked while the supervisor holds the lease.
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let result = std::io::stdin().read(&mut byte);
        let _ = send.send(result);
    });
    assert!(matches!(
        receive.recv_timeout(Duration::from_millis(30)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    std::io::stdout().write_all(b"LEASE_IS_OPEN").unwrap();
    // Returning from a test would make libtest wait for no threads but retaining
    // stdin in this fixture is deliberate. Exit models the real helper main.
    std::process::exit(0);
}

#[test]
fn executor_panic_cleanup_retains_charge_until_consumed_without_pipe_source_leak() {
    use crate::{Executor, Limits as PoolLimits};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let source = Arc::new(vec![1; 1 << 20]);
    let input = source.clone();
    let (send, receive) = mpsc::channel();
    let mut executor = Executor::new(PoolLimits {
        workers: 1,
        pending_jobs: 1,
        transient_bytes: 2 << 20,
    })
    .unwrap();
    let handle = executor
        .submit(
            1 << 20,
            move |_| {
                let checks = AtomicUsize::new(0);
                run_parent_lease(
                    &std::env::current_exe().unwrap(),
                    &args("process::tests::child_stall"),
                    vec![input],
                    limits(),
                    &|| {
                        assert_eq!(
                            checks.fetch_add(1, Ordering::Relaxed),
                            0,
                            "owner check panic"
                        );
                        Ok(())
                    },
                )
            },
            move || {
                send.send(()).unwrap();
            },
        )
        .unwrap();
    receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(executor.reserved_bytes(), 1 << 20);
    assert_eq!(Arc::strong_count(&source), 1);
    assert!(executor.poll_for(handle).unwrap().is_err());
    assert_eq!(executor.reserved_bytes(), 0);
}
