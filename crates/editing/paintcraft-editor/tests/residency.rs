use paintcraft_commands::Batch;
use paintcraft_core::types::{ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::execute::{Selection, execute_with_admission};
use paintcraft_raster::{Admission, RasterLayer};
#[test]
fn residency_exhaustion_rolls_back_the_whole_private_batch_and_reclaims_earlier_tiles() {
    let doc = new_document(257, 1, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let batch = Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"0"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#010203FF","mode":"replace"}},{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":256,"y":0,"width":1,"height":1}},"color":"#040506FF","mode":"replace"}}]}}"##,doc.state_id).as_bytes()).unwrap();
    let bytes = RasterLayer::TILE_BYTES as u64;
    let budget = SharedBudget::new(bytes);
    let error = execute_with_admission(
        &doc,
        Selection::Inactive,
        &batch,
        Admission::with_residency(2 * 65536, budget.clone()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
    assert_eq!(doc.layers[&id].raster.get(0, 0), Rgba([255; 4]));
    let enough = SharedBudget::new(2 * bytes);
    let staged = execute_with_admission(
        &doc,
        Selection::Inactive,
        &batch,
        Admission::with_residency(2 * 65536, enough.clone()),
    )
    .unwrap();
    assert_eq!(
        staged.doc.layers[&id].raster.get(0, 0),
        Rgba([1, 2, 3, 255])
    );
    assert_eq!(
        staged.doc.layers[&id].raster.get(256, 0),
        Rgba([4, 5, 6, 255])
    );
    let reader = staged.doc.clone();
    drop(staged);
    assert_eq!(enough.usage().reserved, 2 * bytes);
    drop(reader);
    assert_eq!(enough.usage().reserved, 0);
}

#[test]
fn history_compaction_releases_residency_only_after_readers_and_restore_owners_release_it() {
    use paintcraft_editor::session::Session;
    let observer = Admission::new(0);
    assert_eq!(observer.residency_usage().reserved, 0);
    let bytes = RasterLayer::TILE_BYTES as u64;
    let mut session = Session::new(new_document(8, 8, Some(Rgba([255; 4])), 0, POLICY).unwrap());
    let mut held = None;
    for x in 0..3 {
        let ticket = session.ticket();
        let id = session.document().roots[0];
        let batch = format!(
            r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":{x},"y":0,"width":1,"height":1}},"color":"#010203FF","mode":"replace"}}]}}"##,
            ticket.state_id, ticket.publication_epoch
        );
        session.apply_json(batch.as_bytes()).unwrap();
        if x == 0 {
            held = Some(session.snapshot());
        }
    }
    assert_eq!(observer.residency_usage().reserved, 3 * bytes);
    session.compact_history(1, 4 << 20, || Ok(())).unwrap();
    assert_eq!(observer.residency_usage().reserved, 2 * bytes);
    drop(held);
    assert_eq!(observer.residency_usage().reserved, bytes);
    session.undo().unwrap().unwrap();
    assert_eq!(observer.residency_usage().reserved, 2 * bytes);
    let raster = &session.document().layers[&session.document().roots[0]].raster;
    assert_eq!(raster.get(1, 0), Rgba([1, 2, 3, 255]));
    assert_eq!(raster.get(2, 0), Rgba([255; 4]));
    session.redo().unwrap().unwrap();
    assert_eq!(observer.residency_usage().reserved, bytes);
    assert_eq!(
        session.document().layers[&session.document().roots[0]]
            .raster
            .get(2, 0),
        Rgba([1, 2, 3, 255])
    );
    drop(session);
    assert_eq!(observer.residency_usage().reserved, 0);
}
