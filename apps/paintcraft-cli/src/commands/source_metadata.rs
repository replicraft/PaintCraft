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
        pcnd::read_with_metadata_admission_checked(
            bytes,
            u64::MAX,
            paintcraft_core::types::policy::STAGED_PIXELS_MAX,
            paintcraft_document::LAYER_COUNT_MAX,
            self.admission(),
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
