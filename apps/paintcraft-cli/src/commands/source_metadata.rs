//! Process-owned metadata admission shared by commands and persistent MCP state.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
#[derive(Clone, Debug)]
pub struct Realm(pub SharedBudget);
impl Default for Realm {
    fn default() -> Self {
        // The raster realm is process-global; cloning it never creates a new
        // allowance for a request, document, parked session or history snapshot.
        Self(paintcraft_raster::Admission::new(0).paint_metadata_budget())
    }
}
impl Realm {
    pub fn admission(&self) -> pcnd::MetadataAdmission<'_> {
        pcnd::MetadataAdmission {
            bytes: paintcraft_document::source_metadata::BYTES_MAX,
            budget: &self.0,
        }
    }
    pub fn read(&self, bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Document> {
        pcnd::read_with_admission_checked(
            bytes,
            u64::MAX,
            paintcraft_core::types::policy::STAGED_PIXELS_MAX,
            paintcraft_document::LAYER_COUNT_MAX,
            pcnd::ReadAdmission {
                metadata: self.admission(),
                coverage: pcnd::CoverageAdmission {
                    bytes: 256 << 20,
                    budget: &paintcraft_core::types::resources::default_coverage_budget(),
                    limits: paintcraft_raster::coverage_mask::Limits::default(),
                },
            },
            check,
        )
    }
    pub fn bmp(
        &self,
        bytes: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<image::bmp_retention::Prepared> {
        image::bmp_retention::decode(bytes, image::DECODE_PIXELS_MAX, self.admission(), check)
    }
    pub fn png(
        &self,
        bytes: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<image::png_retention::Prepared> {
        image::png_retention::decode(bytes, image::DECODE_PIXELS_MAX, self.admission(), check)
    }
}
pub fn read(bytes: &[u8]) -> Result<Document> {
    Realm::default().read(bytes, &|| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_png_and_project_reads_compete_in_one_persistent_host_realm() {
        let bytes = include_bytes!(
            "../../../../crates/services/paintcraft-formats/tests/png-retention-fixtures/rgba8.png"
        );
        let probe = Realm(SharedBudget::new(1 << 20));
        let prepared = probe.png(bytes, &|| Ok(())).unwrap();
        let charge = prepared.attachment.as_ref().unwrap().storage_usage().1;
        let document = prepared
            .document_with_admission(&mut paintcraft_raster::Admission::new(65536), &|| Ok(()))
            .unwrap();
        let project = pcnd::write_compact(&document).unwrap();
        let realm = Realm(SharedBudget::new(charge));
        let captured = realm.png(bytes, &|| Ok(())).unwrap();
        assert_eq!(
            realm.clone().read(&project, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(realm.0.usage().reserved, charge);
        drop(captured);
        assert_eq!(realm.0.usage().reserved, 0);
        let opened = realm.clone().read(&project, &|| Ok(())).unwrap();
        assert_eq!(
            realm.png(bytes, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(realm.0.usage().reserved, charge);
        drop(opened);
        assert_eq!(realm.0.usage().reserved, 0);
    }
    #[test]
    fn bmp_png_and_project_reads_share_the_same_host_metadata_allowance() {
        let png_bytes = include_bytes!(
            "../../../../crates/services/paintcraft-formats/tests/png-retention-fixtures/rgba8.png"
        );
        let bmp_bytes = include_bytes!(
            "../../../../crates/services/paintcraft-formats/tests/bmp-retention-fixtures/rgb24-v124-profile-positive.bmp"
        );
        let probe = Realm(SharedBudget::new(1 << 20));
        let png = probe.png(png_bytes, &|| Ok(())).unwrap();
        let bmp = probe.bmp(bmp_bytes, &|| Ok(())).unwrap();
        let png_charge = png.attachment.as_ref().unwrap().storage_usage().1;
        let bmp_charge = bmp.attachment.as_ref().unwrap().storage_usage().1;
        drop((png, bmp));
        let realm = Realm(SharedBudget::new(png_charge + bmp_charge - 1));
        let png = realm.png(png_bytes, &|| Ok(())).unwrap();
        assert_eq!(
            realm.bmp(bmp_bytes, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(realm.0.usage().reserved, png_charge);
        drop(png);
        let bmp = realm.bmp(bmp_bytes, &|| Ok(())).unwrap();
        let document = bmp
            .document_with_admission(&mut paintcraft_raster::Admission::new(65536), &|| Ok(()))
            .unwrap();
        let project = pcnd::write_compact(&document).unwrap();
        drop((bmp, document));
        assert_eq!(realm.0.usage().reserved, 0);
        let opened = realm.read(&project, &|| Ok(())).unwrap();
        assert_eq!(
            realm.png(png_bytes, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(realm.0.usage().reserved, bmp_charge);
        drop(opened);
        assert_eq!(realm.0.usage().reserved, 0);
    }
}

#[cfg(test)]
mod mask_tests {
    use super::*;
    #[test]
    fn cli_and_mcp_shared_project_reader_accepts_pcnd4_without_relabeling_raw_pixels() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/services/paintcraft-formats/tests/pcnd-mask-fixtures/opaque-half.pcnd"
        ));
        let realm = Realm(SharedBudget::new(0));
        for _ in 0..2 {
            let doc = realm.read(bytes, &|| Ok(())).unwrap();
            let layer = &doc.layers[&doc.roots[0]];
            assert_eq!(
                layer.raster.get(0, 0),
                paintcraft_core::types::Rgba([255, 0, 0, 255])
            );
            assert_eq!(
                layer.filters.as_slice()[0]
                    .mask()
                    .unwrap()
                    .coverage()
                    .coverage(0, 0),
                128
            );
            assert_eq!(pcnd::write_compact(&doc).unwrap(), bytes);
        }
    }
}
