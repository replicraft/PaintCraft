//! One lazy asynchronous encoded read; opaque platform calls are not preemptible.
use super::{EncodedImage, Job, Lease, State, Wake};
use paintcraft_core::types::resources::SharedBudget;
use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, atomic::Ordering, mpsc},
};
#[cfg(target_os = "macos")]
mod macos;
trait Reader {
    fn read(
        &mut self,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<EncodedImage, String>;
}
type Factory = Box<dyn FnMut() -> Result<Box<dyn Reader>, String> + Send>;
struct Request {
    budget: SharedBudget,
    lease: Arc<Lease>,
    reply: mpsc::Sender<Result<EncodedImage, String>>,
    wake: Wake,
}
pub struct ReadService {
    sender: Option<mpsc::SyncSender<Request>>,
    state: Arc<State>,
}
impl fmt::Debug for ReadService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardReadService")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}
impl ReadService {
    pub fn supported() -> bool {
        cfg!(target_os = "macos")
    }
    pub fn new() -> Result<Self, String> {
        Self::with_factory(Box::new(|| {
            #[cfg(target_os = "macos")]
            {
                Ok(Box::new(macos::NativeReader))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("Bounded image clipboard reads are unavailable on this platform".into())
            }
        }))
    }
    fn with_factory(factory: Factory) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let state = Arc::new(State::default());
        let worker = state.clone();
        std::thread::Builder::new()
            .name("paintcraft-clipboard-read".into())
            .spawn(move || run(receiver, worker, factory))
            .map_err(|e| format!("Cannot start clipboard reader: {e}"))?;
        Ok(Self {
            sender: Some(sender),
            state,
        })
    }
    /// A read starts only on explicit submission. At most one request includes
    /// its unconsumed result; result clones retain the caller's encoded-byte lease.
    pub fn submit(
        &self,
        budget: SharedBudget,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<Job<EncodedImage>, String> {
        let lease = Lease::acquire(&self.state)?;
        let (reply, receiver) = mpsc::channel();
        let request = Request {
            budget,
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
            return Err("Clipboard read worker stopped".into());
        }
        Ok(Job {
            receiver,
            lease,
            consumed: false.into(),
        })
    }
}
impl Drop for ReadService {
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Release);
        self.sender.take();
    }
}
fn run(receiver: mpsc::Receiver<Request>, state: Arc<State>, mut factory: Factory) {
    let mut reader: Option<Box<dyn Reader>> = None;
    while let Ok(request) = receiver.recv() {
        let check = || {
            if request.lease.cancelled() {
                Err("Clipboard read cancelled".to_owned())
            } else {
                Ok(())
            }
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            check()?;
            if reader.is_none() {
                reader = Some(factory()?);
            }
            check()?;
            let image = reader
                .as_mut()
                .expect("initialized reader")
                .read(&request.budget, &check)?;
            check()?;
            Ok(image)
        }));
        let result = match result {
            Ok(result) => result,
            Err(_) => {
                state.poisoned.store(true, Ordering::Release);
                Err("Clipboard read backend panicked; service disabled".into())
            }
        };
        let result = if request.lease.cancelled() {
            Err("Clipboard read cancelled".into())
        } else {
            result
        };
        request.lease.finish();
        let _ = request.reply.send(result);
        let _ = catch_unwind(AssertUnwindSafe(request.wake));
        if state.closed.load(Ordering::Acquire) || state.poisoned.load(Ordering::Acquire) {
            break;
        }
    }
}
#[cfg(test)]
mod tests;
