//! Captured-image native printing without modifying documents or writing formats.
use std::sync::{Arc, mpsc};
pub const ENCODED_BYTES_MAX: usize = 64 << 20;
pub const PIXELS_MAX: u64 = 16_777_216;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintOutcome {
    Printed,
    Cancelled,
}
type Result<T> = std::result::Result<T, String>;
type Runner = dyn Fn(Vec<u8>, String) -> Result<PrintOutcome> + Send + Sync;

pub struct PrintService {
    sender: mpsc::Sender<Result<PrintOutcome>>,
    receiver: mpsc::Receiver<Result<PrintOutcome>>,
    runner: Arc<Runner>,
    pending: bool,
}
impl std::fmt::Debug for PrintService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrintService")
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}
impl Default for PrintService {
    fn default() -> Self {
        Self::with_runner(Arc::new(native_print))
    }
}
impl PrintService {
    fn with_runner(runner: Arc<Runner>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            runner,
            pending: false,
        }
    }
    pub fn available() -> bool {
        cfg!(target_os = "macos")
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    /// Own one captured PNG until the native print operation finishes. An active
    /// request cannot be replaced, and launching a helper never bypasses its panel.
    pub fn request(
        &mut self,
        png: Vec<u8>,
        title: String,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<()> {
        if self.pending {
            return Err("A native print dialog is already active".into());
        }
        validate_png(&png)?;
        if title.len() > 4096 || title.contains('\0') {
            return Err("Print title exceeds its limit or contains a null byte".into());
        }
        let (sender, runner) = (self.sender.clone(), self.runner.clone());
        self.pending = true;
        if let Err(error) = std::thread::Builder::new()
            .name("paintcraft-print".into())
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner(png, title)))
                        .unwrap_or_else(|_| Err("Native printing worker failed".into()));
                let _ = sender.send(result);
                wake();
            })
        {
            self.pending = false;
            return Err(format!("Cannot start native printing: {error}"));
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Result<PrintOutcome>> {
        let result = self.receiver.try_recv().ok()?;
        self.pending = false;
        Some(result)
    }
}
fn validate_png(png: &[u8]) -> Result<()> {
    if png.len() > ENCODED_BYTES_MAX
        || png.len() < 33
        || png[..8] != [137, 80, 78, 71, 13, 10, 26, 10]
        || png[8..16] != [0, 0, 0, 13, 73, 72, 68, 82]
    {
        return Err("Print image has an invalid PNG header or exceeds its encoded limit".into());
    }
    let width = u32::from_be_bytes(png[16..20].try_into().expect("checked PNG header"));
    let height = u32::from_be_bytes(png[20..24].try_into().expect("checked PNG header"));
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > PIXELS_MAX {
        return Err("Print image exceeds its pixel limit".into());
    }
    Ok(())
}
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
fn native_print(png: Vec<u8>, title: String) -> Result<PrintOutcome> {
    macos::run(png, title)
}
#[cfg(not(target_os = "macos"))]
fn native_print(_: Vec<u8>, _: String) -> Result<PrintOutcome> {
    Err("Native printing is not implemented for this platform".into())
}
#[cfg(test)]
mod tests;
