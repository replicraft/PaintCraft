use super::*;
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicBool, Ordering},
};
#[test]
fn retired_font_discovery_keeps_actual_thread_until_drain_join() {
    let release = Arc::new(Barrier::new(2));
    let wait = Arc::clone(&release);
    let retired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&retired);
    let mut fonts = FontState::default();
    fonts
        .threads
        .spawn("font-retirement-test", move || {
            wait.wait();
            flag.store(true, Ordering::Release);
        })
        .unwrap();
    assert!(!fonts.loading());
    assert!(fonts.retirement_busy());
    let threads = fonts.retire_for_update();
    assert!(fonts.retiring);
    let drain = std::thread::spawn(move || threads.shutdown());
    assert!(!retired.load(Ordering::Acquire));
    release.wait();
    drain.join().unwrap().unwrap();
    assert!(retired.load(Ordering::Acquire));
    fonts.retire_for_update().shutdown().unwrap();
}
#[test]
fn retired_font_state_does_not_start_a_new_discovery() {
    let mut app = App::new(None);
    app.fonts.retire_for_update().shutdown().unwrap();
    app.initialize_fonts(&egui::Context::default());
    assert!(app.fonts.pending.is_none());
    assert!(!app.fonts.threads.busy());
}
