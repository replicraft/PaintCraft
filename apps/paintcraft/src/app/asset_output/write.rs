//! Background staging stops before replacement until UI revalidates frozen intent.
use super::*;
struct Bytes {
    bytes: Vec<u8>,
    _owner: Owner,
}
pub(crate) enum WriteEvent {
    Authorize(Option<PathBuf>),
    Done(save::Outcome),
    Failed(Error),
}
pub(crate) struct WriteJob {
    receiver: mpsc::Receiver<WriteEvent>,
    authorize: mpsc::SyncSender<bool>,
    cancel: Arc<AtomicBool>,
}
impl Drop for WriteJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl WriteJob {
    pub(crate) fn start(
        bytes: Vec<u8>,
        owner: Owner,
        path: PathBuf,
        expected: Option<save::Expectation>,
        max: u64,
        ctx: &egui::Context,
    ) -> Result<Self> {
        if bytes.len() as u64 > max {
            return Err(limit());
        }
        let data = Bytes {
            bytes,
            _owner: owner,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        let (authorize, wait) = mpsc::sync_channel(1);
        let wake = ctx.clone();
        std::thread::Builder::new()
            .name("paintcraft-asset-write".into())
            .spawn(move || {
                let result = save::save_with_expectation_and_preparation(
                    &path,
                    expected.is_some(),
                    max,
                    expected.as_ref(),
                    |sink| {
                        for block in data.bytes.chunks(64 * 1024) {
                            check(&flag)?;
                            std::io::Write::write_all(sink, block)
                                .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))?;
                        }
                        Ok(())
                    },
                    |outcome| {
                        check(&flag)?;
                        sender
                            .send(WriteEvent::Authorize(
                                outcome.resolved_destination().map(Path::to_path_buf),
                            ))
                            .map_err(|_| limit())?;
                        wake.request_repaint();
                        loop {
                            check(&flag)?;
                            match wait.recv_timeout(Duration::from_millis(25)) {
                                Ok(true) => break Ok(()),
                                Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    break Err(limit());
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                            }
                        }
                    },
                );
                // The worker owns bytes before Owner, including cancellation/error.
                drop(data);
                let _ = sender.send(WriteEvent::Done(result));
                wake.request_repaint();
            })
            .map_err(|e| Error::new(ErrorCode::ResourceLimit, e.to_string()))?;
        Ok(Self {
            receiver,
            authorize,
            cancel,
        })
    }
    pub(crate) fn poll(&self) -> Option<WriteEvent> {
        match self.receiver.try_recv() {
            Ok(value) => Some(value),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(WriteEvent::Failed(limit())),
        }
    }
    pub(crate) fn authorize(&self, value: bool) {
        let _ = self.authorize.try_send(value);
    }
}
