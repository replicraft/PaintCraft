//! Acknowledged native image writes. The worker retains clipboard ownership.
use paintcraft_core::types::clipboard::Image;
mod encoded;
pub use encoded::{EncodedImage, Encoding, MAX_ENCODED_BYTES};
use std::borrow::Cow;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

trait Writer {
    fn write(&mut self, image: &Image) -> Result<(), String>;
}

struct NativeWriter(arboard::Clipboard);
impl Writer for NativeWriter {
    fn write(&mut self, image: &Image) -> Result<(), String> {
        self.0
            .set_image(arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: Cow::Borrowed(image.rgba()),
            })
            .map_err(|error| error.to_string())
    }
}

type Factory = Box<dyn FnMut() -> Result<Box<dyn Writer>, String> + Send>;
type Wake = Box<dyn FnOnce() + Send>;

#[derive(Default, Debug)]
struct State {
    busy: AtomicBool,
    closed: AtomicBool,
    poisoned: AtomicBool,
}

#[derive(Debug)]
struct Lease {
    state: Arc<State>,
    cancelled: AtomicBool,
    done: AtomicBool,
    retired: AtomicBool,
    released: AtomicBool,
}
impl Lease {
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) || self.state.closed.load(Ordering::Acquire)
    }
    fn release(&self) {
        // Completion and retirement can race on different threads. A shared
        // total order prevents both sides from missing the other's store.
        if self.done.load(Ordering::SeqCst)
            && self.retired.load(Ordering::SeqCst)
            && !self.released.swap(true, Ordering::AcqRel)
        {
            self.state.busy.store(false, Ordering::Release);
        }
    }
    fn finish(&self) {
        self.done.store(true, Ordering::SeqCst);
        self.release();
    }
    fn retire(&self) {
        self.retired.store(true, Ordering::SeqCst);
        self.release();
    }
}

struct Request {
    image: Image,
    lease: Arc<Lease>,
    reply: mpsc::Sender<Result<(), String>>,
    wake: Wake,
}

/// One outstanding write at a time, including acknowledgements not yet consumed.
/// Backend initialization is lazy, so unavailable desktop services report errors
/// through the job instead of preventing application startup.
pub struct ClipboardService {
    sender: Option<mpsc::SyncSender<Request>>,
    state: Arc<State>,
}
impl fmt::Debug for ClipboardService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardService")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}
impl ClipboardService {
    pub fn new() -> Result<Self, String> {
        Self::with_factory(Box::new(|| {
            arboard::Clipboard::new()
                .map(|clipboard| Box::new(NativeWriter(clipboard)) as Box<dyn Writer>)
                .map_err(|error| error.to_string())
        }))
    }

    fn with_factory(factory: Factory) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let state = Arc::new(State::default());
        let worker_state = state.clone();
        std::thread::Builder::new()
            .name("paintcraft-clipboard".into())
            .spawn(move || run(receiver, worker_state, factory))
            .map_err(|error| format!("Cannot start clipboard worker: {error}"))?;
        Ok(Self {
            sender: Some(sender),
            state,
        })
    }

    pub fn submit(
        &self,
        image: Image,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<Job, String> {
        if self.state.closed.load(Ordering::Acquire) || self.state.poisoned.load(Ordering::Acquire)
        {
            return Err("Clipboard service is unavailable".into());
        }
        self.state
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "A clipboard write is already pending".to_owned())?;
        // A previous job may have poisoned the worker while this submission
        // waited to acquire the slot. Recheck after acquiring ownership.
        if self.state.poisoned.load(Ordering::Acquire) {
            self.state.busy.store(false, Ordering::Release);
            return Err("Clipboard service is unavailable".into());
        }
        let lease = Arc::new(Lease {
            state: self.state.clone(),
            cancelled: AtomicBool::new(false),
            done: AtomicBool::new(false),
            retired: AtomicBool::new(false),
            released: AtomicBool::new(false),
        });
        let (reply, receiver) = mpsc::channel();
        let request = Request {
            image,
            lease: lease.clone(),
            reply,
            wake: Box::new(wake),
        };
        if self
            .sender
            .as_ref()
            .expect("live sender")
            .try_send(request)
            .is_err()
        {
            lease.finish();
            lease.retire();
            return Err("Clipboard worker stopped".into());
        }
        Ok(Job {
            receiver,
            lease,
            consumed: AtomicBool::new(false),
        })
    }
}
impl Drop for ClipboardService {
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Release);
        self.sender.take();
    }
}

/// A successful result acknowledges the native write, never just its submission.
/// Cancellation cannot undo an opaque OS call already in progress, but suppresses
/// its acknowledgement, so callers must not delete source pixels after cancelling.
#[derive(Debug)]
pub struct Job {
    receiver: mpsc::Receiver<Result<(), String>>,
    lease: Arc<Lease>,
    consumed: AtomicBool,
}
impl Job {
    pub fn cancel(&self) {
        self.lease.cancelled.store(true, Ordering::Release);
    }

    pub fn try_result(&self) -> Option<Result<(), String>> {
        if self.consumed.load(Ordering::Acquire) {
            return None;
        }
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.lease.finish();
                Err("Clipboard worker stopped".into())
            }
        };
        self.consumed.store(true, Ordering::Release);
        let result = if self.lease.cancelled() {
            Err("Clipboard write cancelled".into())
        } else {
            result
        };
        self.lease.retire();
        Some(result)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
        self.lease.retire();
    }
}

fn run(receiver: mpsc::Receiver<Request>, state: Arc<State>, mut factory: Factory) {
    let mut writer: Option<Box<dyn Writer>> = None;
    while let Ok(request) = receiver.recv() {
        let result = if request.lease.cancelled() {
            Ok(Err("Clipboard write cancelled".into()))
        } else {
            catch_unwind(AssertUnwindSafe(|| {
                if writer.is_none() {
                    writer = Some(factory()?);
                }
                if request.lease.cancelled() {
                    return Err("Clipboard write cancelled".into());
                }
                writer
                    .as_mut()
                    .expect("initialized writer")
                    .write(&request.image)
            }))
        };
        let result = match result {
            Ok(result) => result,
            Err(_) => {
                state.poisoned.store(true, Ordering::Release);
                Err("Clipboard backend panicked; service disabled".into())
            }
        };
        let Request {
            image,
            lease,
            reply,
            wake,
        } = request;
        drop(image);
        let result = if lease.cancelled() {
            Err("Clipboard write cancelled".into())
        } else {
            result
        };
        lease.finish();
        let _ = reply.send(result);
        let _ = catch_unwind(AssertUnwindSafe(wake));
        if state.poisoned.load(Ordering::Acquire) || state.closed.load(Ordering::Acquire) {
            break;
        }
    }
}

#[cfg(test)]
mod tests;
