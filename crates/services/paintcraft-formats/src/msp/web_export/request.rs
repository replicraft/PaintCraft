use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub source_len: u32,
    pub size: (u32, u32),
    pub owner: Owner,
    pub nonce: Uuid,
    pub source_sha256: [u8; 32],
    pub source_flags: u8,
}
impl Header {
    pub fn capture(plan: &SourcePlan, source: &[u8], owner: Owner, nonce: Uuid) -> Result<Self> {
        if owner.state != plan.source.state() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Browser MSP2 source state changed",
            ));
        }
        let header = Self {
            source_len: u32::try_from(source.len())
                .map_err(|_| resource("Browser MSP2 source extent"))?,
            size: plan.size(),
            owner,
            nonce,
            source_sha256: Sha256::digest(source).into(),
            source_flags: plan.source_flags(),
        };
        header.validate()?;
        Ok(header)
    }
    pub(super) fn validate(&self) -> Result<()> {
        if self.owner.frontend != 3 {
            return Err(invalid("Browser MSP2 is Studio-only"));
        }
        if self.source_len == 0
            || self.source_len as usize > SOURCE_BYTES_MAX
            || self.size.0 == 0
            || self.size.1 == 0
            || self.size.0 > 65535
            || self.size.1 > 65535
            || u64::from(self.size.0) * u64::from(self.size.1) > PIXELS_MAX
        {
            return Err(resource("Browser MSP2 request profile"));
        }
        if self.source_flags > 3 {
            return Err(invalid("Browser MSP2 source flags"));
        }
        RgbaLayout::new(Version::RowRleV2, self.size.0, self.size.1)?;
        Ok(())
    }
    pub fn encode(&self) -> [u8; REQUEST_HEADER] {
        let mut bytes = [0; REQUEST_HEADER];
        bytes[..8].copy_from_slice(b"PCMSPQ01");
        bytes[8..10].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        bytes[12..16].copy_from_slice(&self.source_len.to_be_bytes());
        bytes[16..20].copy_from_slice(&self.size.0.to_be_bytes());
        bytes[20..24].copy_from_slice(&self.size.1.to_be_bytes());
        bytes[24..65].copy_from_slice(&owner_bytes(self.owner));
        bytes[65..81].copy_from_slice(self.nonce.as_bytes());
        bytes[81..113].copy_from_slice(&self.source_sha256);
        bytes[113] = 2;
        bytes[114] = self.source_flags;
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REQUEST_HEADER
            || &bytes[..8] != b"PCMSPQ01"
            || bytes[8..10] != PROTOCOL_VERSION.to_be_bytes()
            || bytes[10..12] != [0, 0]
            || bytes[113] != 2
            || bytes[115..].iter().any(|b| *b != 0)
        {
            return Err(invalid("Browser MSP2 request framing/reserved fields"));
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
            source_flags: bytes[114],
        };
        header.validate()?;
        Ok(header)
    }
    pub fn check_source(&self, source: &[u8]) -> Result<()> {
        if source.len() != self.source_len as usize
            || <[u8; 32]>::from(Sha256::digest(source)) != self.source_sha256
        {
            return Err(failed("Browser MSP2 source length/hash mismatch"));
        }
        Ok(())
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
