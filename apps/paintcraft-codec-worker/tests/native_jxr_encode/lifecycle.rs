use super::*;
use paintcraft_core::types::Error;
use std::{
    io::{BufRead, Write},
    process::{Child, Stdio},
    sync::mpsc,
    time::Instant,
};
fn cancel() -> Error {
    Error::new(ErrorCode::OperationCancelled, "private encode canceled")
}
#[test]
fn bounded_request_read_cancel_deadline_and_output_pipe_refusal_reap_child() {
    let request = make_request(65_536);
    let only_header = || vec![Arc::new(request.header().encode().to_vec())];
    let start = Instant::now();
    let error = process::run_parent_lease(
        helper(),
        &["--jxr-encode".into()],
        only_header(),
        Limits {
            deadline: Duration::from_secs(2),
            ..limits(&request)
        },
        &|| {
            if start.elapsed() >= Duration::from_millis(50) {
                Err(cancel())
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let error = process::run_parent_lease(
        helper(),
        &["--jxr-encode".into()],
        only_header(),
        Limits {
            deadline: Duration::from_millis(50),
            ..limits(&request)
        },
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    let error = process::run_parent_lease(
        helper(),
        &["--jxr-encode".into()],
        request.segments(),
        Limits {
            output_bytes: wire::RESPONSE_HEADER,
            ..limits(&request)
        },
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
}
struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn fixture(name: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--ignored", "--nocapture"]);
    command
}
#[test]
fn actual_parent_death_closes_sole_writer_while_encode_body_read_is_blocked() {
    let mut parent = Guard(
        fixture("lifecycle::fixture_parent_blocked_input")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = parent.0.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            if line.unwrap().contains("JXR_ENCODE_PARENT_READY") {
                send.send(false).unwrap();
            }
        }
        send.send(true).unwrap();
    });
    assert!(!receive.recv_timeout(Duration::from_secs(5)).unwrap());
    assert!(
        matches!(
            receive.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "the sole writer must keep the real child blocked before parent death"
    );
    parent.0.kill().unwrap();
    parent.0.wait().unwrap();
    // The real worker inherited this output handle. EOF follows only after it
    // also stops. No Child::wait-induced premature stdin close is used as proof.
    assert!(receive.recv_timeout(Duration::from_secs(5)).unwrap());
    reader.join().unwrap();
}
#[test]
#[ignore = "subprocess parent fixture invoked by exact lifecycle test"]
fn fixture_parent_blocked_input() {
    let request = make_request(65_536);
    let mut child = Command::new(helper())
        .arg("--jxr-encode")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut writer = child.stdin.take().unwrap();
    writer.write_all(&request.header().encode()).unwrap();
    println!("JXR_ENCODE_PARENT_READY");
    std::io::stdout().flush().unwrap();
    let _ = child.wait(); // Child.stdin is empty: the real sole writer stays here.
    drop(writer);
}
#[test]
fn complete_request_eof_invalidates_existing_parent_lease() {
    let source: Vec<u8> = (0..wire::RGB_BYTES_MAX).map(|i| (i % 251) as u8).collect();
    let request = wire::Request::capture(
        wire::Layout::new(1024, 1024, wire::ENCODED_BYTES_MAX).unwrap(),
        Arc::new(source),
        owner(),
        &|| Ok(()),
    )
    .unwrap();
    let mut child = Guard(
        Command::new(helper())
            .arg("--jxr-encode")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut writer = child.0.stdin.take().unwrap();
    writer.write_all(&request.header().encode()).unwrap();
    writer.write_all(request.rgb()).unwrap();
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "private encoder did not stop after full-request lease loss"
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(status.code(), Some(76));
}
