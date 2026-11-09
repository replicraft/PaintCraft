//! Native internal assertions over actual owned domains, never a product hook.
use super::*;
use paintcraft_core::types::json::Value;
use paintcraft_raster::coverage_mask::A8Mask;
use sha2::{Digest, Sha256};
fn n(v: &Value, k: &str) -> i64 {
    v.get(k).unwrap().as_i64().unwrap()
}
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).unwrap().as_str().unwrap()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn raw(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn mask(actual: Option<(bool, &A8Mask)>, expected: &Value) {
    if expected == &Value::Null {
        assert!(actual.is_none());
        return;
    }
    let (enabled, actual) = actual.unwrap();
    assert_eq!(enabled, expected.get("enabled").unwrap().as_bool().unwrap());
    assert_eq!(hex(&actual.digest()), text(expected, "digest"));
    let d = expected.get("domain").unwrap().as_array().unwrap();
    let domain = actual.domain();
    assert_eq!(
        [domain.x0, domain.y0, domain.x1, domain.y1],
        [
            d[0].as_i64().unwrap(),
            d[1].as_i64().unwrap(),
            d[2].as_i64().unwrap(),
            d[3].as_i64().unwrap()
        ]
    );
    assert_eq!(
        actual.stats().bands,
        expected.get("bands").unwrap().as_array().unwrap().len()
    );
}
fn assert_payload(payload: &SourcePayload8, expected: &Value) {
    let doc = &payload._document;
    assert_eq!(
        hex(doc.document_id.as_bytes()),
        text(expected, "document_id")
    );
    assert_eq!(hex(doc.state_id.as_bytes()), text(expected, "state_id"));
    assert_eq!(
        doc.title.as_deref(),
        expected.get("title").unwrap().as_str()
    );
    let canvas = expected.get("canvas").unwrap().as_array().unwrap();
    assert_eq!(
        doc.canvas,
        (
            canvas[0].as_i64().unwrap() as u32,
            canvas[1].as_i64().unwrap() as u32
        )
    );
    let layers = expected.get("layers").unwrap().as_array().unwrap();
    assert_eq!(doc.roots.len(), layers.len());
    assert_eq!(doc.layers.len(), layers.len());
    for (id, e) in doc.roots.iter().zip(layers) {
        assert_eq!(hex(id.as_bytes()), text(e, "id"));
        let l = &doc.layers[id];
        assert_eq!(l.id, *id);
        assert_eq!(l.name, text(e, "name"));
        assert_eq!(l.visible, e.get("visible").unwrap().as_bool().unwrap());
        assert_eq!(i64::from(l.opacity), n(e, "opacity"));
        assert_eq!(l.blend_mode as u8, n(e, "blend") as u8);
        let origin = e.get("origin").unwrap().as_array().unwrap();
        assert_eq!(
            l.origin,
            (origin[0].as_i64().unwrap(), origin[1].as_i64().unwrap())
        );
        assert_eq!(l.raster.width(), n(e, "width") as u32);
        assert_eq!(l.raster.height(), n(e, "height") as u32);
        let color = e
            .get("default")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u8)
            .collect::<Vec<_>>();
        assert_eq!(l.raster.default_pixel().0.as_slice(), color);
        let filters = e.get("filters").unwrap().as_array().unwrap();
        assert_eq!(l.filters.len(), filters.len());
        for (f, x) in l.filters.iter().zip(filters) {
            assert_eq!(hex(f.id().as_bytes()), text(x, "id"));
            assert_eq!(f.enabled(), x.get("enabled").unwrap().as_bool().unwrap());
            assert_eq!(f.descriptor().id(), text(x, "profile"));
            let (p, len) = f.descriptor().semantic_parameter_bytes();
            assert_eq!(hex(&p[..len]), text(x, "parameters"));
            mask(
                f.mask().map(|m| (m.enabled(), m.coverage())),
                x.get("mask").unwrap(),
            );
        }
        mask(
            l.mask.as_ref().map(|m| (m.enabled(), m.coverage())),
            e.get("mask").unwrap(),
        );
        let tiles = e.get("tiles").unwrap().as_array().unwrap();
        assert_eq!(l.raster.tile_leases().len(), tiles.len());
        for (coord, lease) in l.raster.tile_leases() {
            let tile = tiles
                .iter()
                .find(|t| n(t, "x") == coord.tx && n(t, "y") == coord.ty)
                .unwrap();
            assert_eq!(
                hex(&Sha256::digest(lease.bytes())),
                text(tile, "physical_sha256")
            );
            assert_eq!(lease.bytes().len(), 262144);
        }
    }
    match (
        doc.source_metadata.as_ref(),
        expected.get("source_metadata_hex").unwrap().as_str(),
    ) {
        (None, None) => {}
        (Some(actual), Some(encoded)) => {
            let b = raw(encoded);
            assert_eq!(actual.wire_header().as_slice(), &b[..192]);
            assert_eq!(actual.profile().unwrap_or(&[]), &b[192..]);
        }
        _ => panic!("ICC presence changed"),
    }
}

pub(in crate::source8) fn assert_closed(source: &ClosedSource8, expected: &Value) {
    assert_payload(source.payload.0.as_deref().unwrap(), expected);
}
