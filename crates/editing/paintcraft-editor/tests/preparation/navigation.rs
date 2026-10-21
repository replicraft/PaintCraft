use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn worker_undo_redo_and_seek_publish_only_at_the_owner() {
    let mut s = session();
    s.apply(&edit(&s, "#000000FF")).unwrap();
    let black = s.ticket().state_id;
    let black_history = s.history_view().entries[1].history_id;
    s.apply(&edit(&s, "#FF0000FF")).unwrap();
    let red = s.ticket().state_id;
    let work = s.preparation().unwrap();
    let prepared = std::thread::spawn(move || work.undo(|| Ok(())).unwrap())
        .join()
        .unwrap();
    assert_eq!(s.ticket().state_id, red);
    assert_eq!(
        s.publish_navigation(prepared, || Ok(()))
            .unwrap()
            .unwrap()
            .state_id,
        black
    );
    let prepared = s.preparation().unwrap().redo(|| Ok(())).unwrap();
    assert_eq!(s.ticket().state_id, black);
    s.mark_saved();
    assert_eq!(
        s.publish_navigation(prepared, || Ok(()))
            .unwrap()
            .unwrap()
            .state_id,
        red
    );
    assert!(s.is_dirty());
    let ticket = s.history_view().ticket;
    let prepared = s
        .preparation()
        .unwrap()
        .navigate_history(ticket, black_history, || Ok(()))
        .unwrap();
    s.publish_navigation(prepared, || Ok(())).unwrap().unwrap();
    assert!(!s.is_dirty());
}

#[test]
fn stale_cancelled_and_noop_navigation_do_not_move_the_cursor() {
    let mut s = session();
    let prepared = s.preparation().unwrap().undo(|| Ok(())).unwrap();
    assert_eq!(s.publish_navigation(prepared, || Ok(())).unwrap(), None);
    s.apply(&edit(&s, "#000000FF")).unwrap();
    let cancelled_result = s.preparation().unwrap().undo(|| Ok(())).unwrap();
    let view = s.history_view();
    assert_eq!(
        s.publish_navigation(cancelled_result, cancelled)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(s.history_view(), view);
    let stale = s.preparation().unwrap().undo(|| Ok(())).unwrap();
    s.apply(&edit(&s, "#FF0000FF")).unwrap();
    let view = s.history_view();
    assert_eq!(
        s.publish_navigation(stale, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.history_view(), view);
}

#[test]
fn cancellation_at_each_cold_restore_boundary_releases_decodes_and_allows_exact_retry() {
    let mut s = Session::new(
        paintcraft_document::import_image(
            512,
            256,
            &vec![255; 512 * 256 * 4],
            "cancelled-cold-navigation",
        )
        .unwrap(),
    );
    let id = s.document().roots[0];
    let t = s.ticket();
    let batch = Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":512,"height":256}},"color":"#000000FF","mode":"replace"}}]}}"##,t.state_id,t.publication_epoch).as_bytes()).unwrap();
    s.apply(&batch).unwrap();
    s.compact_history(1, 64 << 20, || Ok(())).unwrap();
    let view = s.history_view();
    let before = s.history_memory().decoded_bytes;
    let mut completed = false;
    let mut cancellations = 0;
    // Try every check boundary, stopping when the whole operation completes.
    // This includes cancellation after a private tile was decoded, without
    // hard-coding the implementation's number or order of checks.
    for cancel_at in 0..128 {
        let calls = AtomicUsize::new(0);
        match s.preparation().unwrap().undo(|| {
            if calls.fetch_add(1, Ordering::SeqCst) == cancel_at {
                cancelled()
            } else {
                Ok(())
            }
        }) {
            Err(error) => {
                assert_eq!(error.code, ErrorCode::OperationCancelled);
                cancellations += 1;
            }
            Ok(prepared) => {
                drop(prepared);
                completed = true;
            }
        }
        assert_eq!(s.history_view(), view);
        assert_eq!(s.history_memory().decoded_bytes, before);
        if completed {
            break;
        }
    }
    assert!(completed && cancellations > 0);
    let prepared = s.preparation().unwrap().undo(|| Ok(())).unwrap();
    s.publish_navigation(prepared, || Ok(())).unwrap().unwrap();
    for y in 0..256 {
        for x in 0..512 {
            assert_eq!(s.document().layers[&id].raster.get(x, y), Rgba([255; 4]));
        }
    }
}
