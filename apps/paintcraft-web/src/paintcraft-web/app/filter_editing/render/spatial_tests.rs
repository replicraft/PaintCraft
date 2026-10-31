//! Source-profile identity is distinct from parameter-generation identity.
use super::*;
use paintcraft_commands::{FilterOp, LayerRef};
use paintcraft_core::types::{
    Rect,
    filters::{BoxBlurV1, Descriptor},
};
fn source() -> Capture {
    let document = Arc::new(
        paintcraft_document::import_image(1, 1, &[10, 20, 30, 255], "profile-upgrade").unwrap(),
    );
    Capture {
        owner: Uuid::from_u128(1),
        ticket: Ticket {
            state_id: document.state_id,
            publication_epoch: 0,
        },
        draft: Uuid::from_u128(2),
        document,
        spatial: false,
    }
}
fn request(capture: &Capture, descriptor: Descriptor) -> Request {
    Request {
        revision: Revision {
            draft: capture.draft,
            generation: 1,
        },
        edit: Some(FilterEdit {
            owner: capture.owner,
            state_id: capture.ticket.state_id,
            publication_epoch: 0,
            operation: FilterOp::Add {
                layer: LayerRef::Id(capture.document.roots[0]),
                alias: "preview".into(),
                descriptor,
                enabled: true,
                to_index: None,
            },
        }),
        render: true,
        view: View::new(&capture.document, Rect::new(0, 0, 1, 1).unwrap(), 0).unwrap(),
    }
}
#[test]
fn adding_box_upgrades_capture_identity_and_base_requests_retain_the_chosen_profile() {
    let original = source();
    let mut renderer = Renderer::default();
    renderer.want(
        original.clone(),
        request(&original, Descriptor::InvertRgbV1),
    );
    assert!(!renderer.wanted.as_ref().unwrap().capture.spatial);
    renderer.want(
        original.clone(),
        request(&original, Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap())),
    );
    let upgraded = renderer.wanted.as_ref().unwrap().capture.clone();
    assert!(upgraded.spatial);
    assert!(!original.same(&upgraded)); // existing poll closes+acks before source replacement
    assert!(Arc::ptr_eq(&original.document, &upgraded.document));
    let mut base = request(&upgraded, Descriptor::InvertRgbV1);
    base.edit = None;
    renderer.want(upgraded.clone(), base);
    assert!(renderer.wanted.as_ref().unwrap().capture.same(&upgraded));
    assert!(renderer.wanted.as_ref().unwrap().capture.spatial);
}
