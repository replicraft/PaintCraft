//! Readiness-driven transfer. Initial compositor acquisition is separately opaque.
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{OFlags, fcntl_getfl, fcntl_setfl},
};
use std::{
    io::{ErrorKind, Read},
    os::fd::AsFd,
    time::{Duration, Instant},
};
pub(super) struct Nonblocking<R> {
    source: R,
    idle: Duration,
}
impl<R: Read + AsFd> Nonblocking<R> {
    pub fn new(source: R, idle: Duration) -> Result<Self, String> {
        let flags =
            fcntl_getfl(&source).map_err(|e| format!("Cannot inspect clipboard pipe: {e}"))?;
        fcntl_setfl(&source, flags | OFlags::NONBLOCK)
            .map_err(|e| format!("Cannot configure clipboard pipe: {e}"))?;
        Ok(Self { source, idle })
    }
    pub fn read(
        &mut self,
        out: &mut [u8],
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<usize, String> {
        let start = Instant::now();
        loop {
            check()?;
            let remaining = self
                .idle
                .checked_sub(start.elapsed())
                .ok_or("Clipboard image transfer timed out")?;
            let duration = remaining.min(Duration::from_millis(50));
            let timeout = Timespec {
                tv_sec: 0,
                tv_nsec: duration.subsec_nanos().into(),
            };
            let ready = {
                let mut fds = [PollFd::new(&self.source, PollFlags::IN | PollFlags::HUP)];
                match poll(&mut fds, Some(&timeout)) {
                    Ok(0) => continue,
                    Ok(_) => fds[0].revents(),
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(error) => return Err(format!("Cannot wait for clipboard image: {error}")),
                }
            };
            check()?;
            if ready.intersects(PollFlags::NVAL | PollFlags::ERR) {
                return Err("Clipboard image pipe failed".into());
            }
            match self.source.read(out) {
                Ok(count) => return Ok(count),
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
                {
                    continue;
                }
                Err(error) => return Err(format!("Cannot read clipboard image: {error}")),
            }
        }
    }
}

#[cfg(test)]
mod tests;
