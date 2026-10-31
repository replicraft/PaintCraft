use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub source_len: u32,
    pub size: (u32, u32),
    pub owner: Owner,
    pub nonce: Uuid,
    pub source_sha256: [u8; 32],
    pub settings: [u8; 8],
    pub source_flags: u8,
    pub bake_intent: u8,
    pub filter_digest: [u8; 32],
    pub metadata: export::SourceMetadataContext,
    pub masks: export::SourceMaskContext,
    pub spatial: export::SourceSpatialContext,
    pub catalog: export::SourceCatalogContext,
}
impl Header {
    pub fn capture(plan: &SourcePlan, source: &[u8], owner: Owner, nonce: Uuid) -> Result<Self> {
        if plan.protocol_version() != 6 {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Export Worker6 requires stored catalog filters",
            ));
        }
        if owner.state != plan.source.state() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Browser DDS source state changed",
            ));
        }
        let header = Self {
            source_len: u32::try_from(source.len())
                .map_err(|_| resource("Browser DDS source extent"))?,
            size: plan.size(),
            owner,
            nonce,
            source_sha256: Sha256::digest(source).into(),
            settings: plan.settings,
            source_flags: plan.source_flags(),
            bake_intent: plan.bake_intent(),
            filter_digest: plan.filter_digest(),
            metadata: plan.metadata_context(),
            masks: plan.mask_context(),
            spatial: export::SourceSpatialContext::decode_catalog(
                &plan.spatial_context(source.len())?,
                WORKER_PAYLOAD_BYTES_MAX,
            )?,
            catalog: export::SourceCatalogContext::decode(&plan.catalog_context(source.len())?)?,
        };
        header.validate()?;
        Ok(header)
    }
    pub(super) fn validate(&self) -> Result<()> {
        if self.owner.frontend != 3 {
            return Err(invalid("Browser DDS is Studio-only"));
        }
        if self.source_len == 0
            || self.source_len as usize > SOURCE_BYTES_MAX
            || self.size.0 == 0
            || self.size.1 == 0
            || self.size.0 > 65536
            || self.size.1 > 65536
            || u64::from(self.size.0) * u64::from(self.size.1) > PIXELS_MAX
        {
            return Err(resource("Browser DDS request profile"));
        }
        if self.metadata.target()
            != export::metadata_policy::Target::new(
                crate::registry::Family::Dds,
                crate::registry::Operation::ExportFlat,
            )
        {
            return Err(invalid("Browser DDS metadata target mismatch"));
        }
        self.spatial.validate_catalog(WORKER_PAYLOAD_BYTES_MAX)?;
        self.catalog.validate_region(self.spatial)?;
        self.masks.check_flags(self.source_flags)?;
        if self.source_flags > 15 || self.bake_intent > 1 {
            return Err(invalid("Browser DDS source flags"));
        }
        decode_settings(self.settings, self.size)?;
        Ok(())
    }
    pub fn encode(&self) -> [u8; REQUEST_HEADER] {
        let mut bytes = [0; REQUEST_HEADER];
        bytes[..8].copy_from_slice(b"PCDDSQ06");
        bytes[8..10].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        bytes[12..16].copy_from_slice(&self.source_len.to_be_bytes());
        bytes[16..20].copy_from_slice(&self.size.0.to_be_bytes());
        bytes[20..24].copy_from_slice(&self.size.1.to_be_bytes());
        bytes[24..65].copy_from_slice(&owner_bytes(self.owner));
        bytes[65..81].copy_from_slice(self.nonce.as_bytes());
        bytes[81..113].copy_from_slice(&self.source_sha256);
        bytes[113..121].copy_from_slice(&self.settings);
        bytes[121] = self.source_flags;
        bytes[122] = self.bake_intent;
        bytes[128..160].copy_from_slice(&self.filter_digest);
        bytes[160..224].copy_from_slice(&self.metadata.encode());
        bytes[224..256].copy_from_slice(&self.masks.encode());
        bytes[256..320].copy_from_slice(&self.spatial.encode_catalog());
        bytes[320..384].copy_from_slice(&self.catalog.encode());
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REQUEST_HEADER
            || &bytes[..8] != b"PCDDSQ06"
            || bytes[8..10] != PROTOCOL_VERSION.to_be_bytes()
            || bytes[10..12] != [0, 0]
            || bytes[123..128].iter().any(|b| *b != 0)
        {
            return Err(invalid("Browser DDS request framing/reserved fields"));
        }
        let header = Self {
            source_len: u32::from_be_bytes(bytes[12..16].try_into().unwrap()),
            size: (
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
            ),
            owner: read_owner(&bytes[24..65]),
            nonce: Uuid::from_bytes(bytes[65..81].try_into().unwrap()),
            source_sha256: bytes[81..113].try_into().unwrap(),
            settings: bytes[113..121].try_into().unwrap(),
            source_flags: bytes[121],
            bake_intent: bytes[122],
            filter_digest: bytes[128..160].try_into().unwrap(),
            metadata: export::SourceMetadataContext::decode(&bytes[160..224])?,
            masks: export::SourceMaskContext::decode(&bytes[224..256])?,
            spatial: export::SourceSpatialContext::decode_catalog(
                &bytes[256..320],
                WORKER_PAYLOAD_BYTES_MAX,
            )?,
            catalog: export::SourceCatalogContext::decode(&bytes[320..384])?,
        };
        header.validate()?;
        Ok(header)
    }
    pub fn check_source(&self, source: &[u8]) -> Result<()> {
        self.check_source_checked(source, &|| Ok(()))
    }
    pub fn check_source_checked(
        &self,
        source: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if source.len() != self.source_len as usize {
            return Err(failed("Catalog source length mismatch"));
        }
        let mut digest = Sha256::new();
        for chunk in source.chunks(65_536) {
            check()?;
            digest.update(chunk);
        }
        if <[u8; 32]>::from(digest.finalize()) != self.source_sha256 {
            return Err(failed("Catalog source hash mismatch"));
        }
        if !source.starts_with(b"PCND\x06\r\n\x1a") {
            return Err(invalid("Export Worker6 requires PCND6 source"));
        }
        check()
    }
}
pub(super) fn owner_bytes(owner: Owner) -> [u8; 41] {
    let mut bytes = [0; 41];
    bytes[..16].copy_from_slice(owner.session.as_bytes());
    bytes[16..32].copy_from_slice(owner.state.as_bytes());
    bytes[32..40].copy_from_slice(&owner.epoch.to_be_bytes());
    bytes[40] = owner.frontend;
    bytes
}
pub(super) fn read_owner(bytes: &[u8]) -> Owner {
    Owner {
        session: Uuid::from_bytes(bytes[..16].try_into().unwrap()),
        state: Uuid::from_bytes(bytes[16..32].try_into().unwrap()),
        epoch: u64::from_be_bytes(bytes[32..40].try_into().unwrap()),
        frontend: bytes[40],
    }
}
