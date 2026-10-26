//! Native transport lifetime: after one bounded request, stdin stays open and
//! silent for the entire job. The sole parent writer is the liveness lease.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::io::Read;

/// Distinct from requested-heap refusal (75); no partial frame is trustworthy.
const PARENT_LEASE_LOST: i32 = 76;

pub fn start() -> Result<()> {
    std::thread::Builder::new()
        .name("paintcraft-parent-lease".into())
        .spawn(|| {
            let mut input = std::io::stdin().lock();
            // EOF, an IO failure, or an unexpected byte invalidates the parent
            // lease. Never return to decoder threads, log or acquire output locks.
            let _ = input.read(&mut [0]);
            crate::allocator::nonallocating_exit(PARENT_LEASE_LOST);
        })
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "parent lease watcher startup"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, Write},
        process::{Child, Command, Stdio},
        sync::mpsc,
        time::Duration,
    };
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
    fn actual_parent_death_closes_lease_and_stops_sleeping_child() {
        let mut parent = Guard(
            fixture("parent_lease::tests::fixture_parent")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let stdout = parent.0.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                if line.unwrap().contains("PARENT_LEASE_READY") {
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
            "child must remain alive while sole parent writer is held"
        );
        parent.0.kill().unwrap();
        parent.0.wait().unwrap();
        // The sleeping child inherited this output pipe. EOF is observed only
        // after it stops as well; this is not parent-alive kill/wait reaping.
        assert!(receive.recv_timeout(Duration::from_secs(5)).unwrap());
        reader.join().unwrap();
    }
    #[test]
    fn unexpected_byte_refuses_whole_job_with_dedicated_status() {
        let mut child = fixture("parent_lease::tests::fixture_child")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.as_mut().unwrap().write_all(&[1]).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(PARENT_LEASE_LOST));
    }
    #[test]
    #[ignore = "subprocess parent fixture: invoked only by lifecycle test"]
    fn fixture_parent() {
        let mut child = fixture("parent_lease::tests::fixture_child")
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // ChildStdin remains owned here while waiting. Killing this real parent
        // closes the sole writer on Unix and Windows without cooperative cleanup.
        // Child::wait closes Child.stdin, so move the sole writer out first.
        // Retain it until wait returns; only actual parent death closes it here.
        let writer = child.stdin.take().unwrap();
        let _ = child.wait();
        drop(writer);
    }
    #[test]
    #[ignore = "subprocess lease fixture: invoked only by lifecycle tests"]
    fn fixture_child() {
        start().unwrap();
        println!("PARENT_LEASE_READY");
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(60));
    }
}
