use super::*;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    layout: Layout,
    owner: Owner,
    nonce: Uuid,
    rgb_sha256: [u8; 32],
    request_sha256: [u8; 32],
}
impl Header {
    pub fn layout(self) -> Layout {
        self.layout
    }
    pub fn owner(self) -> Owner {
        self.owner
    }
    pub fn nonce(self) -> Uuid {
        self.nonce
    }
    pub fn rgb_sha256(self) -> [u8; 32] {
        self.rgb_sha256
    }
    pub fn request_sha256(self) -> [u8; 32] {
        self.request_sha256
    }
    pub fn encode(self) -> [u8; REQUEST_HEADER] {
        let mut out = [0; REQUEST_HEADER];
        out[..8].copy_from_slice(REQUEST_MAGIC);
        out[8..10].copy_from_slice(&VERSION.to_be_bytes());
        out[10] = PROFILE_TAG;
        out[12..16].copy_from_slice(&self.layout.width.to_be_bytes());
        out[16..20].copy_from_slice(&self.layout.height.to_be_bytes());
        out[20..24].copy_from_slice(&self.layout.stride.to_be_bytes());
        out[24..32].copy_from_slice(&self.layout.rgb_bytes.to_be_bytes());
        out[32..40].copy_from_slice(&self.layout.encoded_cap.to_be_bytes());
        put_owner(&mut out[40..81], self.owner);
        out[81..97].copy_from_slice(self.nonce.as_bytes());
        out[97..129].copy_from_slice(&self.rgb_sha256);
        out[129..161].copy_from_slice(&self.request_sha256);
        out
    }
    fn canonical_digest(self) -> [u8; 32] {
        let mut header = self.encode();
        header[129..161].fill(0);
        let mut hash = Sha256::new();
        hash.update(HASH_DOMAIN);
        hash.update(header);
        hash.finalize().into()
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if cfg!(target_arch = "wasm32") {
            return Err(unsupported("JXR encode frames are native-only"));
        }
        if bytes.len() != REQUEST_HEADER {
            return Err(invalid("JXR encode header extent"));
        }
        if bytes[..8] != *REQUEST_MAGIC
            || bytes[8..10] != VERSION.to_be_bytes()
            || bytes[10] != PROFILE_TAG
        {
            return Err(unsupported("JXR encode protocol/fixed profile"));
        }
        if bytes[11] != 0 || bytes[161..].iter().any(|n| *n != 0) {
            return Err(invalid("JXR encode nonzero reserved fields"));
        }
        let u32_at = |n| u32::from_be_bytes(bytes[n..n + 4].try_into().unwrap());
        let u64_at = |n| u64::from_be_bytes(bytes[n..n + 8].try_into().unwrap());
        let layout = Layout::new(u32_at(12), u32_at(16), u64_at(32))?;
        if u32_at(20) != layout.stride || u64_at(24) != layout.rgb_bytes {
            return Err(invalid("JXR encode declared stride/RGB extent mismatch"));
        }
        let header = Self {
            layout,
            owner: get_owner(&bytes[40..81]),
            nonce: Uuid::from_bytes(bytes[81..97].try_into().unwrap()),
            rgb_sha256: bytes[97..129].try_into().unwrap(),
            request_sha256: bytes[129..161].try_into().unwrap(),
        };
        if header.owner.frontend != 3 {
            return Err(unsupported("JXR encode is Studio-only"));
        }
        if header.request_sha256 != header.canonical_digest() {
            return Err(stale());
        }
        Ok(header)
    }
    pub fn verify_rgb(self, rgb: &[u8], check: &dyn Fn() -> Result<()>) -> Result<()> {
        if rgb.len() as u64 != self.layout.rgb_bytes {
            return Err(invalid("JXR encode RGB extent"));
        }
        if hash(rgb, check)? != self.rgb_sha256 {
            return Err(stale());
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Request {
    header: Header,
    rgb: Arc<Vec<u8>>,
}
impl Request {
    /// Packed opaque RGB8 input; no RGBA interpretation or silent alpha discard.
    pub fn capture(
        layout: Layout,
        rgb: Arc<Vec<u8>>,
        owner: Owner,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        if cfg!(target_arch = "wasm32") || owner.frontend != 3 {
            return Err(unsupported("JXR encode is native Studio-only"));
        }
        if rgb.len() as u64 != layout.rgb_bytes {
            return Err(invalid("JXR encode RGB extent"));
        }
        let mut header = Header {
            layout,
            owner,
            nonce: Uuid::new_v4(),
            rgb_sha256: hash(rgb.as_slice(), check)?,
            request_sha256: [0; 32],
        };
        header.request_sha256 = header.canonical_digest();
        Ok(Self { header, rgb })
    }
    pub fn header(&self) -> Header {
        self.header
    }
    pub fn rgb(&self) -> &[u8] {
        self.rgb.as_slice()
    }
    /// Existing supervised transport segments: only the fixed header is copied.
    /// Caller admits that small header; packed RGB ownership is shared unchanged.
    pub fn segments(&self) -> Vec<Arc<Vec<u8>>> {
        vec![Arc::new(self.header.encode().to_vec()), self.rgb.clone()]
    }
    pub fn from_parts(
        header: Header,
        rgb: Arc<Vec<u8>>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        header.verify_rgb(rgb.as_slice(), check)?;
        Ok(Self { header, rgb })
    }
    /// Consume exactly one declared frame. EOF is not read: a supervised parent
    /// may retain stdin as its lifetime lease. Transport owns trailing-byte policy.
    /// Caller admits declared RGB storage before invoking this allocating helper.
    pub fn read_from(reader: &mut impl Read, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let mut encoded = [0; REQUEST_HEADER];
        read_checked(reader, &mut encoded, check)?;
        let header = Header::decode(&encoded)?;
        let length = usize::try_from(header.layout.rgb_bytes)
            .map_err(|_| resource("JXR encode addressable RGB extent"))?;
        let mut rgb = Vec::new();
        rgb.try_reserve_exact(length)
            .map_err(|_| resource("JXR encode RGB allocation"))?;
        rgb.resize(length, 0);
        read_checked(reader, &mut rgb, check)?;
        Self::from_parts(header, Arc::new(rgb), check)
    }
    pub fn write_to(&self, writer: &mut impl Write, check: &dyn Fn() -> Result<()>) -> Result<()> {
        self.header.verify_rgb(self.rgb(), check)?;
        write_checked(writer, &self.header.encode(), check)?;
        write_checked(writer, self.rgb(), check)
    }
}
