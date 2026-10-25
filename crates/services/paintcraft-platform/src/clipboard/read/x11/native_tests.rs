//! Opt-in isolated selections on an explicit server; never use CLIPBOARD/PRIMARY.
use super::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use x11rb::{
    connection::Connection,
    protocol::{Event, xproto::*},
    wrapper::ConnectionExt as _,
};
mod owner;
#[derive(Default)]
struct Offer {
    png: Option<Vec<u8>>,
    tiff: Option<Vec<u8>>,
    incremental: bool,
    incr_targets: bool,
    changed_stamp: bool,
    lose_owner: bool,
    refuse_image: bool,
    stall: bool,
}
struct Fixture {
    selection: Vec<u8>,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    image_requested: mpsc::Receiver<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn display() -> String {
    std::env::var("PAINTCRAFT_TEST_X11_DISPLAY")
        .expect("Use an isolated server and explicitly set PAINTCRAFT_TEST_X11_DISPLAY")
}
fn fixture(display: &str, offer: Offer) -> Fixture {
    let selection = format!("_PAINTCRAFT_TEST_{}", uuid::Uuid::new_v4()).into_bytes();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (ready, wait) = mpsc::channel();
    let (image, image_requested) = mpsc::channel();
    let (name, log, stopped, address) = (
        selection.clone(),
        requests.clone(),
        stop.clone(),
        display.to_owned(),
    );
    let worker =
        thread::spawn(move || owner::serve(&address, &name, offer, log, stopped, ready, image));
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    Fixture {
        selection,
        requests,
        stop,
        worker: Some(worker),
        image_requested,
    }
}
fn capture(display: &str, f: &Fixture, budget: &SharedBudget) -> Result<EncodedImage, String> {
    read_selection(Some(display), &f.selection, budget, &|| Ok(()))
}
#[test]
fn quota_denial_happens_before_attempting_an_x11_connection() {
    let error = read_selection(
        Some("invalid-display-never-connect"),
        b"CLIPBOARD",
        &SharedBudget::new(0),
        &|| Ok(()),
    )
    .unwrap_err();
    // An invalid display would otherwise be parsed by x11rb's connection setup.
    assert!(!error.to_lowercase().contains("display"));
}
#[test]
#[ignore = "Requires an explicitly isolated X11 server; CI uses Xvfb"]
fn native_x11_direct_incr_targets_priority_and_ownership_checks() {
    let display = display();
    for incremental in [false, true] {
        let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
        data.extend((0..100_003).map(|n| (n % 251) as u8));
        let f = fixture(
            &display,
            Offer {
                png: Some(data.clone()),
                tiff: Some(b"II*\0tiff".to_vec()),
                incremental,
                incr_targets: incremental,
                ..Default::default()
            },
        );
        let budget = SharedBudget::new(MAX_ENCODED_BYTES);
        let image = capture(&display, &f, &budget).unwrap();
        assert_eq!(image.encoding(), Encoding::Png);
        assert_eq!(image.bytes(), data);
        assert_eq!(
            &*f.requests.lock().unwrap(),
            &["TARGETS", "TIMESTAMP", "image/png", "TIMESTAMP"]
        );
        let clone = image.clone();
        drop(image);
        assert!(budget.usage().reserved > 0);
        drop(clone);
        assert_eq!(budget.usage().reserved, 0);
    }
    let f = fixture(
        &display,
        Offer {
            tiff: Some(b"II*\0tiff".to_vec()),
            ..Default::default()
        },
    );
    assert_eq!(
        capture(&display, &f, &SharedBudget::new(MAX_ENCODED_BYTES))
            .unwrap()
            .encoding(),
        Encoding::Tiff
    );
    for offer in [
        Offer {
            png: Some(b"spoofed".to_vec()),
            tiff: Some(b"II*\0tiff".to_vec()),
            ..Default::default()
        },
        Offer {
            png: Some(b"\x89PNG\r\n\x1a\n".to_vec()),
            changed_stamp: true,
            ..Default::default()
        },
        Offer {
            png: Some(b"\x89PNG\r\n\x1a\n".to_vec()),
            lose_owner: true,
            ..Default::default()
        },
        Offer {
            png: Some(b"\x89PNG\r\n\x1a\n".to_vec()),
            refuse_image: true,
            ..Default::default()
        },
    ] {
        let f = fixture(&display, offer);
        let budget = SharedBudget::new(MAX_ENCODED_BYTES);
        assert!(capture(&display, &f, &budget).is_err());
        assert_eq!(budget.usage().reserved, 0);
        assert!(
            !f.requests
                .lock()
                .unwrap()
                .iter()
                .any(|name| name == "image/tiff")
        );
    }
}
#[test]
#[ignore = "Requires an explicitly isolated X11 server; CI uses Xvfb"]
fn native_x11_stalled_incr_transfer_cancels_without_publishing_bytes() {
    let display = display();
    let f = fixture(
        &display,
        Offer {
            png: Some(b"\x89PNG\r\n\x1a\n".to_vec()),
            incremental: true,
            stall: true,
            ..Default::default()
        },
    );
    let selection = f.selection.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let budget = SharedBudget::new(MAX_ENCODED_BYTES);
    let account = budget.clone();
    let worker = thread::spawn(move || {
        read_selection(Some(&display), &selection, &account, &|| {
            if signal.load(Ordering::Acquire) {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        })
    });
    f.image_requested
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    cancel.store(true, Ordering::Release);
    assert!(worker.join().unwrap().unwrap_err().contains("cancelled"));
    assert_eq!(budget.usage().reserved, 0);
}
