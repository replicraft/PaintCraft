use super::*;
use paintcraft_core::types::{Rgba, ids::deterministic_id};
use paintcraft_document::{Document, Layer};
use paintcraft_raster::{Admission, RasterLayer};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

impl Raster {
    /// Distinct source, dimensions, DPI, fitting or rendered font/image pixels
    /// produce distinct native identities. Consent/publication remain external.
    pub fn document(
        &self,
        source_sha256: &str,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Document> {
        self.options.admit(PIXELS_MAX)?;
        if (self.width, self.height) != (self.options.width, self.options.height) {
            return Err(malformed("SVG raster geometry changed after preparation"));
        }
        check()?;
        let mut hash = Sha256::new();
        hash.update(b"paintcraft.svg-raster/v1\0");
        hash.update(source_sha256.as_bytes());
        hash.update(self.width.to_le_bytes());
        hash.update(self.height.to_le_bytes());
        hash.update(self.options.dpi.to_bits().to_le_bytes());
        hash.update([u8::from(self.options.fit == Fit::Stretch)]);
        for chunk in self.rgba.chunks(65536) {
            check()?;
            hash.update(chunk);
        }
        let sha = paintcraft_document::hex(&hash.finalize());
        let identity = |kind| deterministic_id(&["paintcraft.svg-conversion/v1", kind, &sha]);
        let mut admission = Admission::new(PIXELS_MAX * 16);
        let raster = RasterLayer::from_rgba_checked(
            self.width,
            self.height,
            Rgba::TRANSPARENT,
            &self.rgba,
            &mut admission,
            check,
        )?;
        let id = identity("layer");
        let layer = Layer {
            id,
            name: "Image".into(),
            origin: (0, 0),
            visible: true,
            opacity: 255,
            raster,
        };
        check()?;
        Ok(Document {
            source_metadata: None,
            document_id: identity("document"),
            state_id: identity("state"),
            canvas: (self.width, self.height),
            roots: vec![id],
            layers: HashMap::from([(id, layer)]),
            title: None,
        })
    }
}
