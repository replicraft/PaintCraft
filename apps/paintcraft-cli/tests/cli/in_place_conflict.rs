use super::*;
use rustix::fs::{CWD, Mode, OFlags, openat};
use std::{
    io::Write,
    process::{Child, Stdio},
    time::{Duration, Instant},
};
struct Process(Option<Child>);
impl Drop for Process {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn in_place_commands_reject_replaced_or_deleted_input_after_capture() {
    for (paste, deleted) in [(false, false), (false, true), (true, false), (true, true)] {
        let s = Scratch::new(&format!("in-place-{paste}-{deleted}"));
        let input = s.path("input.pcnd");
        assert_eq!(
            run(&["new", "--width", "320", "--height", "320", "-o", &input]).code,
            0
        );
        let fifo = s.path(if paste { "image.png" } else { "ops.fifo" });
        let payload = if paste {
            paintcraft_formats::image::encode(
                paintcraft_formats::image::ImageFormat::Png,
                1,
                1,
                &[23, 45, 67, 255],
                90,
            )
            .unwrap()
        } else {
            m1_batch().into_bytes()
        };
        let arguments = if paste {
            vec![
                "paste-image",
                &input,
                &fifo,
                "-o",
                &input,
                "--overwrite",
                "--json",
            ]
        } else {
            vec![
                "apply",
                &input,
                "--operations",
                &fifo,
                "-o",
                &input,
                "--overwrite",
                "--json",
            ]
        };
        assert!(
            Command::new("mkfifo")
                .args(["-m", "600", &fifo])
                .status()
                .unwrap()
                .success()
        );
        let mut process = Process(Some(
            Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
                .args(arguments)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ));
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut writer = loop {
            match openat(
                CWD,
                fifo.as_str(),
                OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(fd) => break fs::File::from(fd),
                Err(rustix::io::Errno::NXIO) => {
                    assert!(
                        process.0.as_mut().unwrap().try_wait().unwrap().is_none(),
                        "command exited before acquiring operations"
                    );
                    assert!(Instant::now() < deadline, "operations handshake timed out");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("cannot open operations pipe: {error}"),
            }
        };
        // A FIFO writer opens only after the command opens its reader. The
        // input was already read and hashed, without a scheduling-based sleep.
        if deleted {
            fs::remove_file(&input).unwrap();
        } else {
            fs::write(&input, b"newer external document bytes").unwrap();
        }
        writer.write_all(&payload).unwrap();
        drop(writer);
        let deadline = Instant::now() + Duration::from_secs(10);
        while process.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "command did not finish after its input became available"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        let result = process.0.take().unwrap().wait_with_output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(6),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let response = json::parse(&result.stdout).unwrap();
        assert_eq!(text(&response, &["outcome"]), "file-incomplete");
        assert_eq!(
            response.get("engine").unwrap().get("committed"),
            Some(&Value::Bool(true))
        );
        assert_eq!(text(&response, &["file", "operation"]), "save");
        assert_eq!(text(&response, &["file", "replacement"]), "not-replaced");
        if deleted {
            assert!(!Path::new(&input).exists());
        } else {
            assert_eq!(fs::read(&input).unwrap(), b"newer external document bytes");
        }
        assert_eq!(
            fs::read_dir(&s.0).unwrap().count(),
            if deleted { 1 } else { 2 }
        );
    }
}
