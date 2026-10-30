//! A bounded selected-save-profile snapshot, taken only after Worker admission.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
pub(super) const SOURCE_MAX: usize = 64 << 20;
struct Bounded {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl std::io::Write for Bounded {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        if input.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other("Filter source limit"));
        }
        self.bytes
            .try_reserve_exact(input.len())
            .map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn encode(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    encode_with_limit(doc, SOURCE_MAX, check)
}
fn encode_with_limit(
    doc: &Document,
    limit: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let mut output = Bounded {
        bytes: Vec::new(),
        limit,
        exceeded: false,
    };
    let result = paintcraft_formats::pcnd::write_compact_to_checked(doc, &mut output, check);
    if output.exceeded {
        return Err(Error::new(ErrorCode::ResourceLimit, "Filter source limit"));
    }
    result?;
    Ok(output.bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::{Rgba, resources::SharedBudget};
    use paintcraft_document::{
        Layer,
        source_metadata::{BYTES_MAX, SourceMetadata},
    };
    use paintcraft_raster::{Admission, RasterLayer};
    use uuid::Uuid;
    #[test]
    fn retained_compact_boundary_rejected_by_raw_still_opens_a_private_snapshot() {
        let mut doc=paintcraft_formats::pcnd::read(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"))).unwrap();
        let original = doc.source_metadata.take().unwrap();
        let descriptor = *original.descriptor();
        let size = (8 << 20) - 273;
        let mut profile = original.profile().unwrap().to_vec();
        profile.resize(size, 0);
        profile[..4].copy_from_slice(&(size as u32).to_be_bytes());
        let budget = SharedBudget::new(BYTES_MAX + 4096);
        let source =
            SourceMetadata::capture(descriptor, Some(&profile), BYTES_MAX, &budget, &|| Ok(()))
                .unwrap();
        doc.title = None;
        doc.canvas = (128, 128);
        doc.roots.clear();
        doc.layers.clear();
        let id = Uuid::from_u128(2);
        let mut raster = RasterLayer::new(128, 128, Rgba::TRANSPARENT);
        raster
            .set_block(
                0,
                0,
                &[255, 0, 0, 255].repeat(128 * 128),
                &mut Admission::new(65536),
            )
            .unwrap();
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "x".into(),
                origin: (0, 0),
                visible: true,
                opacity: 255,
                filters: Default::default(),
                raster,
            },
        );
        let doc = doc
            .with_source_metadata_checked(Some(source), &|| Ok(()))
            .unwrap();
        assert_eq!(
            paintcraft_formats::pcnd::write(&doc).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        let bytes = encode(&doc, &|| Ok(())).unwrap();
        assert_eq!(bytes.len(), (8 << 20) + 38);
        assert!(
            paintcraft_formats::pcnd::read(&bytes)
                .unwrap()
                .same_semantics(&doc)
        );
        assert_eq!(
            doc.source_metadata.as_ref().unwrap().profile().unwrap(),
            profile
        );
    }
    #[test]
    fn snapshot_input_cap_and_cancellation_never_return_a_partial_source() {
        let doc =
            paintcraft_document::new_document(8, 4, None, 0, paintcraft_document::POLICY).unwrap();
        assert_eq!(
            encode_with_limit(&doc, 1, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(
            encode(&doc, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancelled"
            )))
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
}
