//! Native child lifetimes for admitted work. Invoke inside the existing Executor;
//! cancellation/deadlines terminate and reap the child before releasing the job.
//! This bounds Rust-owned pipe buffers, not the child's RSS or filesystem access.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub deadline: Duration,
    /// The dedicated helper's nonallocating requested-heap/System-OOM exit.
    pub resource_exit_code: Option<i32>,
}
#[derive(Debug)]
pub struct Output {
    pub bytes: Vec<u8>,
    pub status: ExitStatus,
}
struct Running {
    child: Child,
    reaped: bool,
    writer: Option<thread::JoinHandle<()>>,
    reader: Option<thread::JoinHandle<()>>,
    stdin_lease: Option<ChildStdin>,
}
impl Running {
    fn join_pipes(&mut self) -> bool {
        let writer = self
            .writer
            .take()
            .is_none_or(|thread| thread.join().is_ok());
        let reader = self
            .reader
            .take()
            .is_none_or(|thread| thread.join().is_ok());
        writer && reader
    }
    fn stop(&mut self) -> std::io::Result<ExitStatus> {
        let _ = self.child.kill();
        let status = self.child.wait()?;
        self.reaped = true;
        Ok(status)
    }
    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let status = self.child.wait()?;
        self.reaped = true;
        Ok(status)
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        // Always stop/reap before joining: either pipe may be blocked on the
        // child. This also runs when the caller's check callback unwinds.
        self.join_pipes();
    }
}
fn failure(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}

/// Run an explicit absolute helper, with no PATH lookup or inherited environment.
/// Input segments are immutable captures; successful bytes remain private until
/// the format/owner protocol validates them. Stderr is discarded, not buffered.
///
/// The trusted helper must not spawn descendants or delegate inherited pipe
/// handles: only this exact child is terminated. OS process-tree sandboxing and
/// installation/signature checks are separate release gates.
pub fn run(
    executable: &Path,
    args: &[OsString],
    input: Vec<Arc<Vec<u8>>>,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Output> {
    run_inner(executable, args, input, limits, check, false)
}
/// Codec transport only: keep stdin open and silent after writing the exact
/// request. A compatible helper watches this sole writer for parent death.
/// Installation/build-contract verification must happen before selecting this
/// mode; an EOF-framed ordinary tool would wait until its deadline instead.
pub fn run_parent_lease(
    executable: &Path,
    args: &[OsString],
    input: Vec<Arc<Vec<u8>>>,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Output> {
    run_inner(executable, args, input, limits, check, true)
}
fn run_inner(
    executable: &Path,
    args: &[OsString],
    input: Vec<Arc<Vec<u8>>>,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
    parent_lease: bool,
) -> Result<Output> {
    check()?;
    if !executable.is_absolute()
        || limits.input_bytes == 0
        || limits.output_bytes == 0
        || limits.deadline.is_zero()
        || limits.deadline > Duration::from_secs(120)
        || input.len() > 8
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "invalid native worker path or bounds",
        ));
    }
    let bytes = input
        .iter()
        .try_fold(0usize, |sum, part| sum.checked_add(part.len()))
        .ok_or_else(|| resource("worker input length overflow"))?;
    if bytes > limits.input_bytes {
        return Err(resource("worker input exceeds allowance"));
    }
    let started = Instant::now();
    let mut command = Command::new(executable);
    command
        .args(args)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command
        .spawn()
        .map_err(|e| failure(format!("cannot start native worker: {e}")))?;
    let mut running = Running {
        child,
        reaped: false,
        writer: None,
        reader: None,
        stdin_lease: None,
    };
    let mut stdin = running
        .child
        .stdin
        .take()
        .ok_or_else(|| failure("worker stdin missing"))?;
    let mut stdout = running
        .child
        .stdout
        .take()
        .ok_or_else(|| failure("worker stdout missing"))?;
    let (tx, rx) = mpsc::sync_channel(2);
    let writer_tx = tx.clone();
    running.writer = Some(
        thread::Builder::new()
            .name("paintcraft-worker-input".into())
            .spawn(move || {
                let result = (|| {
                    for bytes in input {
                        stdin.write_all(&bytes)?;
                    }
                    stdin.flush()
                })();
                let lease = (parent_lease && result.is_ok()).then_some(stdin);
                let _ = writer_tx.send(Event::Input(result, lease));
            })
            .map_err(|_| resource("cannot start worker input pipe"))?,
    );
    running.reader = Some(
        match thread::Builder::new()
            .name("paintcraft-worker-output".into())
            .spawn(move || {
                let result = (|| {
                    let mut bytes = Vec::new();
                    let mut block = [0; 8192];
                    loop {
                        let count = stdout
                            .read(&mut block)
                            .map_err(|e| failure(format!("cannot read worker output: {e}")))?;
                        if count == 0 {
                            break;
                        }
                        if bytes
                            .len()
                            .checked_add(count)
                            .is_none_or(|size| size > limits.output_bytes)
                        {
                            return Err(resource("worker output exceeds allowance"));
                        }
                        bytes
                            .try_reserve_exact(count)
                            .map_err(|_| resource("worker output allocation failed"))?;
                        bytes.extend_from_slice(&block[..count]);
                    }
                    Ok(bytes)
                })();
                drop(stdout);
                let _ = tx.send(Event::Output(result));
            }) {
            Ok(reader) => reader,
            Err(_) => {
                let _ = running.stop();
                running.join_pipes();
                return Err(resource("cannot start worker output pipe"));
            }
        },
    );
    let result = (|| {
        let mut output = None;
        let mut input_finished = false;
        let mut input_error = None;
        loop {
            check()?;
            if started.elapsed() >= limits.deadline {
                return Err(resource("native worker deadline exceeded"));
            }
            match rx.recv_timeout(Duration::from_millis(5)) {
                Ok(Event::Input(result, lease)) => {
                    running.stdin_lease = lease;
                    // A capped helper can exit before consuming its input. Wait
                    // for its exit status before classifying this broken pipe.
                    input_error = result.err();
                    input_finished = true;
                }
                Ok(Event::Output(result)) => {
                    output = Some(result?);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected)
                    if output.is_none() || !input_finished =>
                {
                    return Err(failure("native worker pipe stopped"));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // recv_timeout returns immediately after both pipe threads
                    // finish. A child may still be alive with its pipes closed.
                    thread::sleep(Duration::from_millis(5));
                }
            }
            if let Some(status) = running
                .child
                .try_wait()
                .map_err(|e| failure(format!("cannot inspect native worker: {e}")))?
            {
                // wait reaps even on platforms where try_wait already did so.
                let _ = running
                    .wait()
                    .map_err(|e| failure(format!("cannot reap native worker: {e}")))?;
                if limits
                    .resource_exit_code
                    .is_some_and(|code| status.code() == Some(code))
                {
                    return Err(resource(
                        "native worker requested-memory allowance or system allocation failed",
                    ));
                }
                if !status.success() {
                    return Err(failure("native worker exited without a successful result"));
                }
                if let Some(error) = input_error.take() {
                    return Err(failure(format!(
                        "cannot write native worker input: {error}"
                    )));
                }
                if input_finished && let Some(bytes) = output.take() {
                    check()?;
                    return Ok(Output { bytes, status });
                }
            }
        }
    })();
    // Closing the trusted helper closes its pipe endpoints, so blocked IO
    // threads complete before their buffers and the caller's reservation leave.
    let stopped = if !running.reaped {
        Some(
            running
                .stop()
                .map_err(|e| failure(format!("cannot terminate/reap native worker: {e}"))),
        )
    } else {
        None
    };
    if !running.join_pipes() {
        return Err(failure("native worker pipe thread failed"));
    }
    if let Some(status) = stopped {
        let status = status?;
        // A worker can exhaust its cap before reading all input. Its dedicated
        // resource exit takes precedence over the resulting broken stdin pipe.
        if limits
            .resource_exit_code
            .is_some_and(|code| status.code() == Some(code))
            && !matches!(&result, Err(error) if error.code == ErrorCode::OperationCancelled)
        {
            return Err(resource(
                "native worker requested-memory allowance or system allocation failed",
            ));
        }
    }
    result
}
enum Event {
    Input(std::io::Result<()>, Option<ChildStdin>),
    Output(Result<Vec<u8>>),
}
#[cfg(test)]
mod tests;
