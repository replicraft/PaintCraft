//! Canonical sealed conversion assessment, independent of browser/native wires.
use super::*;
const MAGIC: &[u8; 8] = b"PCTIFF01";
const DEPTH: &str = "depth-16-reduced-to-8";
const META: &str = "tiff-metadata-ignored";
const ICC: &str = "icc-profile-ignored";
const LOSSES: [&[&str]; 16] = [
    &[],
    &[DEPTH],
    &[META],
    &[DEPTH, META],
    &[ICC],
    &[DEPTH, ICC],
    &[META, ICC],
    &[DEPTH, META, ICC],
    &[LOSS_OTHER_PAGES],
    &[DEPTH, LOSS_OTHER_PAGES],
    &[META, LOSS_OTHER_PAGES],
    &[DEPTH, META, LOSS_OTHER_PAGES],
    &[ICC, LOSS_OTHER_PAGES],
    &[DEPTH, ICC, LOSS_OTHER_PAGES],
    &[META, ICC, LOSS_OTHER_PAGES],
    &[DEPTH, META, ICC, LOSS_OTHER_PAGES],
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assessment {
    source_sha256: [u8; 32],
    source_bytes: u64,
    page_index: u16,
    page_count: u16,
    width: u32,
    height: u32,
    descriptor: Descriptor,
    output_sha256: [u8; 32],
    losses: u16,
}
pub(super) fn qualified(value: &Descriptor) -> bool {
    let n = value.samples_per_pixel;
    let components = |c: Components, expected: Option<u32>| {
        (c.count == 1 || c.count == n)
            && c.count <= 4
            && c.values[..c.count as usize]
                .iter()
                .all(|v| expected.is_none_or(|e| *v == e))
            && c.values[c.count as usize..].iter().all(|v| *v == 0)
    };
    let depth = value.bits_per_sample.values[0];
    let alpha = match value.extra_samples.count {
        0 if value.extra_samples.values == [0; 4] => 0,
        1 if value.extra_samples.values == [2, 0, 0, 0] => 1,
        _ => return false,
    };
    (1..=4).contains(&n)
        && matches!(depth, 1 | 8 | 16)
        && components(value.bits_per_sample, Some(depth))
        && components(value.sample_format, Some(1))
        && match value.photometric {
            0 | 1 => n == 1 + alpha && (depth != 1 || n == 1),
            2 => n == 3 + alpha && depth != 1,
            _ => false,
        }
        && value.orientation == 1
        && value.planar_configuration == 1
        && matches!(value.storage, 1 | 2)
        && match value.compression {
            4 => depth == 1 && n == 1,
            7 => depth == 8 && alpha == 0 && matches!(n, 1 | 3),
            1 | 5 | 8 | 32946 | 32773 => true,
            _ => false,
        }
        && value.icc_bytes <= value.metadata_bytes
        && value.metadata_bytes <= METADATA_PER_PAGE_MAX as u32
        && (value.icc_bytes != 0 || value.icc_sha256 == [0; 32])
}
impl Assessment {
    pub(super) fn new(
        inventory: &Inventory,
        page: &Page,
        rgba: &[u8],
        losses: &[String],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut mask = 0;
        for loss in losses {
            let bit = match loss.as_str() {
                DEPTH => 1,
                META => 2,
                ICC => 4,
                LOSS_OTHER_PAGES => 8,
                _ => return Err(unsupported("TIFF conversion returned an unknown loss")),
            };
            if mask & bit != 0 {
                return Err(malformed("duplicate TIFF conversion loss"));
            }
            mask |= bit;
        }
        let value = Self {
            source_sha256: inventory.source_sha256,
            source_bytes: inventory.source_bytes,
            page_index: page.index,
            page_count: inventory.pages.len() as u16,
            width: page.width,
            height: page.height,
            descriptor: page.descriptor,
            output_sha256: hash_checked(rgba, check)?,
            losses: mask,
        };
        value.validate()?;
        value.validate_result(
            inventory.source_sha256,
            inventory.source_bytes,
            page.index,
            page.width,
            page.height,
            rgba,
        )?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        let expected = u16::from(self.source_depth() == 16)
            | (u16::from(self.descriptor.omitted_metadata) << 1)
            | (u16::from(self.descriptor.icc_bytes != 0) << 2)
            | (u16::from(self.page_count > 1) << 3);
        if self.source_bytes < 8
            || self.source_bytes > BYTES_MAX as u64
            || self.page_count == 0
            || usize::from(self.page_count) > PAGES_MAX
            || self.page_index >= self.page_count
            || self.width == 0
            || self.height == 0
            || self.width > 1_048_576
            || self.height > 1_048_576
            || u64::from(self.width) * u64::from(self.height) > PIXELS_MAX
            || self.descriptor.ifd_offset < 8
            || self.descriptor.ifd_offset >= self.source_bytes
            || !qualified(&self.descriptor)
            || self.losses != expected
        {
            return Err(malformed(
                "TIFF assessment policy, descriptor or bounds differ",
            ));
        }
        Ok(())
    }
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    pub fn page_index(&self) -> u16 {
        self.page_index
    }
    pub fn page_count(&self) -> u16 {
        self.page_count
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    pub fn output_sha256(&self) -> [u8; 32] {
        self.output_sha256
    }
    pub fn source_depth(&self) -> u32 {
        self.descriptor.bits_per_sample.values[0]
    }
    pub fn policy(&self) -> &'static str {
        CONVERSION_POLICY
    }
    pub fn losses(&self) -> &'static [&'static str] {
        LOSSES[usize::from(self.losses)]
    }
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.encode()).into()
    }
    /// Deterministic import identity binds the selected page and its pixels.
    /// Use this key with document::import_image*; the raw file digest alone
    /// would assign the same document/state/layer IDs to different pages.
    pub fn document_identity(&self) -> [u8; 32] {
        self.digest()
    }
    pub fn validate_result(
        &self,
        source_sha256: [u8; 32],
        source_bytes: u64,
        index: u16,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<()> {
        self.validate()?;
        if self.source_sha256 != source_sha256
            || self.source_bytes != source_bytes
            || self.page_index != index
            || (self.width, self.height) != (width, height)
            || rgba.len() as u64 != u64::from(width) * u64::from(height) * 4
            || self.output_sha256 != <[u8; 32]>::from(Sha256::digest(rgba))
        {
            return Err(malformed(
                "TIFF assessment is not bound to the captured source/page/output",
            ));
        }
        Ok(())
    }
    pub fn encode(&self) -> [u8; ASSESSMENT_BYTES] {
        let mut out = [0; ASSESSMENT_BYTES];
        out[..8].copy_from_slice(MAGIC);
        out[8..10].copy_from_slice(&1u16.to_be_bytes());
        out[10..12].copy_from_slice(&self.losses.to_be_bytes());
        out[12..20].copy_from_slice(&self.source_bytes.to_be_bytes());
        out[20..22].copy_from_slice(&self.page_index.to_be_bytes());
        out[22..24].copy_from_slice(&self.page_count.to_be_bytes());
        out[24..28].copy_from_slice(&self.width.to_be_bytes());
        out[28..32].copy_from_slice(&self.height.to_be_bytes());
        out[32..64].copy_from_slice(&self.source_sha256);
        out[64..96].copy_from_slice(&self.output_sha256);
        let d = &self.descriptor;
        out[96..104].copy_from_slice(&d.ifd_offset.to_be_bytes());
        out[104] = u8::from(d.little_endian);
        out[105] = d.storage;
        for (at, c) in [
            (108, d.bits_per_sample),
            (128, d.sample_format),
            (148, d.extra_samples),
        ] {
            out[at..at + 4].copy_from_slice(&c.count.to_be_bytes());
            for (index, value) in c.values.iter().enumerate() {
                out[at + 4 + index * 4..at + 8 + index * 4].copy_from_slice(&value.to_be_bytes());
            }
        }
        for (at, value) in [
            (168, d.photometric),
            (172, d.samples_per_pixel),
            (176, d.compression),
            (180, d.orientation),
            (184, d.planar_configuration),
            (188, d.icc_bytes),
            (224, d.metadata_bytes),
        ] {
            out[at..at + 4].copy_from_slice(&value.to_be_bytes());
        }
        out[192..224].copy_from_slice(&d.icc_sha256);
        out[228] = u8::from(d.omitted_metadata);
        out[229] = u8::from(d.resolution_x.is_some())
            | (u8::from(d.resolution_y.is_some()) << 1)
            | (u8::from(d.resolution_unit.is_some()) << 2);
        let x = d.resolution_x.unwrap_or_default();
        let y = d.resolution_y.unwrap_or_default();
        for (at, value) in [
            (230, x.0),
            (234, x.1),
            (238, y.0),
            (242, y.1),
            (246, d.resolution_unit.unwrap_or_default()),
        ] {
            out[at..at + 4].copy_from_slice(&value.to_be_bytes());
        }
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ASSESSMENT_BYTES
            || &bytes[..8] != MAGIC
            || bytes[8..10] != 1u16.to_be_bytes()
            || bytes[104] > 1
            || bytes[106..108] != [0; 2]
            || bytes[228] > 1
            || bytes[229] > 7
            || bytes[250..] != [0; 6]
        {
            return Err(malformed("TIFF assessment framing or version differs"));
        }
        let word = |at| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
        let components = |at| Components {
            count: word(at),
            values: [word(at + 4), word(at + 8), word(at + 12), word(at + 16)],
        };
        let flags = bytes[229];
        if (flags & 1 == 0 && bytes[230..238] != [0; 8])
            || (flags & 2 == 0 && bytes[238..246] != [0; 8])
            || (flags & 4 == 0 && word(246) != 0)
        {
            return Err(malformed("TIFF absent resolution fields carry values"));
        }
        let value = Self {
            source_sha256: bytes[32..64].try_into().unwrap(),
            output_sha256: bytes[64..96].try_into().unwrap(),
            source_bytes: u64::from_be_bytes(bytes[12..20].try_into().unwrap()),
            page_index: u16::from_be_bytes(bytes[20..22].try_into().unwrap()),
            page_count: u16::from_be_bytes(bytes[22..24].try_into().unwrap()),
            width: word(24),
            height: word(28),
            losses: u16::from_be_bytes(bytes[10..12].try_into().unwrap()),
            descriptor: Descriptor {
                ifd_offset: u64::from_be_bytes(bytes[96..104].try_into().unwrap()),
                little_endian: bytes[104] == 1,
                storage: bytes[105],
                bits_per_sample: components(108),
                sample_format: components(128),
                extra_samples: components(148),
                photometric: word(168),
                samples_per_pixel: word(172),
                compression: word(176),
                orientation: word(180),
                planar_configuration: word(184),
                icc_bytes: word(188),
                icc_sha256: bytes[192..224].try_into().unwrap(),
                metadata_bytes: word(224),
                omitted_metadata: bytes[228] == 1,
                resolution_x: (flags & 1 != 0).then(|| (word(230), word(234))),
                resolution_y: (flags & 2 != 0).then(|| (word(238), word(242))),
                resolution_unit: (flags & 4 != 0).then(|| word(246)),
            },
        };
        value.validate()?;
        Ok(value)
    }
}
