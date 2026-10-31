//! Closed Source2/3 must not inherit the automatically selected catalog batch.
use super::*;
use paintcraft_core::types::filters::{CurvesRgbV1, ExposureSrgbV1, VibranceRgbV1};
use paintcraft_document::{FilterInstance, FilterStack};
#[test]
fn sources2_and3_refuse_default_disabled_catalog_add_update_before_private_candidate() {
    let mut d = fixture();
    let layer = d.roots[0];
    let id = Uuid::from_u128(923);
    d.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        id,
        false,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    let bytes = pcnd::write(&d).unwrap();
    let mut attempts = 0;
    for descriptor in [
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::default()),
        Descriptor::CurvesRgbV1(CurvesRgbV1::default()),
        Descriptor::VibranceRgbV1(VibranceRgbV1::default()),
    ] {
        for protocol in [2, 3] {
            let mut source = plan(&d, &bytes).as_object().unwrap().clone();
            if protocol == 3 {
                for (key, n) in [
                    ("protocol", 3),
                    ("spatialCount", 0),
                    ("workspaceBytes", 64 << 20),
                    ("workLimit", 268435456),
                ] {
                    source.insert(key.into(), Value::Int(n));
                }
            }
            let source = Value::Object(source);
            for op in [
                FilterOp::Add {
                    layer: LayerRef::Id(layer),
                    alias: "closed".into(),
                    descriptor,
                    enabled: false,
                    to_index: None,
                },
                FilterOp::Update {
                    layer: LayerRef::Id(layer),
                    filter: FilterRef::Id(id),
                    descriptor,
                },
            ] {
                let mut s = Service::open(&bytes, &text(&source)).unwrap();
                let error = s
                    .prepare(&text(&request(
                        &source,
                        1,
                        Some(layer),
                        op.to_value().unwrap(),
                        false,
                        [0, 0, 3, 2],
                        0,
                    )))
                    .unwrap_err();
                assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
                assert!(s.candidate.is_none());
                assert!(s.frame.is_none());
                assert_eq!(pcnd::write(&s.base).unwrap(), bytes);
                attempts += 1;
            }
        }
    }
    assert_eq!(attempts, 12);
}
