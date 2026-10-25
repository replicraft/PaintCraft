use super::*;
use crate::clipboard::{EncodedImage, Encoding};
use paintcraft_core::types::resources::SharedBudget;
use std::{
    fs::File,
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
fn pair() -> (File, File) {
    let (read, write) = rustix::pipe::pipe().unwrap();
    (File::from(read), File::from(write))
}
#[test]
fn real_pipe_hangup_drains_remaining_bytes_and_eof_freezes_exact_payload() {
    let (read, mut write) = pair();
    let data = b"\x89PNG\r\n\x1a\npipe";
    write.write_all(data).unwrap();
    drop(write);
    let budget = SharedBudget::new(1024);
    let image = EncodedImage::capture_stream(
        1024,
        &budget,
        &|| Ok(()),
        || {
            Ok((
                Encoding::Png,
                Nonblocking::new(read, Duration::from_secs(1))?,
            ))
        },
        |source, out| source.read(out, &|| Ok(())),
    )
    .unwrap();
    assert_eq!(image.bytes(), data);
    assert!(budget.usage().reserved < 1024);
    drop(image);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn an_idle_live_writer_times_out_and_can_be_cancelled_before_eof() {
    let (read, _write) = pair();
    let mut source = Nonblocking::new(read, Duration::from_millis(10)).unwrap();
    assert!(
        source
            .read(&mut [0; 8], &|| Ok(()))
            .unwrap_err()
            .contains("timed out")
    );
    let (read, _write) = pair();
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let (entered, waiting) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let mut source = Nonblocking::new(read, Duration::from_secs(5)).unwrap();
        source.read(&mut [0; 8], &|| {
            let _ = entered.try_send(());
            if signal.load(Ordering::Acquire) {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        })
    });
    waiting.recv_timeout(Duration::from_secs(2)).unwrap();
    cancel.store(true, Ordering::Release);
    assert!(worker.join().unwrap().unwrap_err().contains("cancelled"));
}

#[test]
fn service_cancellation_releases_stream_capacity_and_allows_a_fresh_snapshot() {
    use crate::clipboard::read::{ReadService, Reader};
    use std::collections::VecDeque;
    struct Pipes {
        sources: VecDeque<File>,
        entered: mpsc::SyncSender<()>,
    }
    impl Reader for Pipes {
        fn read(
            &mut self,
            budget: &SharedBudget,
            check: &dyn Fn() -> Result<(), String>,
        ) -> Result<EncodedImage, String> {
            EncodedImage::capture_stream(
                1024,
                budget,
                check,
                || {
                    Ok((
                        Encoding::Png,
                        Nonblocking::new(
                            self.sources.pop_front().unwrap(),
                            Duration::from_secs(5),
                        )?,
                    ))
                },
                |source, out| {
                    let _ = self.entered.try_send(());
                    source.read(out, check)
                },
            )
        }
    }
    let (idle, _live_writer) = pair();
    let (ready, mut writer) = pair();
    let data = b"\x89PNG\r\n\x1a\nfresh";
    writer.write_all(data).unwrap();
    drop(writer);
    let (entered, waiting) = mpsc::sync_channel(1);
    let mut reader = Some(Pipes {
        sources: [idle, ready].into(),
        entered,
    });
    let service =
        ReadService::with_factory(Box::new(move || Ok(Box::new(reader.take().unwrap())))).unwrap();
    let budget = SharedBudget::new(1024);
    let (wake, woken) = mpsc::channel();
    let signal = wake.clone();
    let cancelled = service
        .submit(budget.clone(), move || signal.send(()).unwrap())
        .unwrap();
    waiting.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(budget.usage().reserved, 1024);
    assert!(service.submit(budget.clone(), || {}).is_err());
    cancelled.cancel();
    woken.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(cancelled.try_result().unwrap().is_err());
    assert_eq!(budget.usage().reserved, 0);
    let fresh = service
        .submit(budget.clone(), move || wake.send(()).unwrap())
        .unwrap();
    woken.recv_timeout(Duration::from_secs(2)).unwrap();
    let image = fresh.try_result().unwrap().unwrap();
    assert_eq!(image.bytes(), data);
    drop(image);
    assert_eq!(budget.usage().reserved, 0);
}
