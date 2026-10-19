//! A deliberately small ZIP subset: stored members only, no ZIP64, no encryption,
//! no data descriptors. The writer is canonical (fixed header fields and
//! timestamp); the reader is strict about everything it accepts and bounds every
//! size before allocating.

use std::collections::HashSet;

use paintcraft_types::{Error, ErrorCode, Result};

/// DOS timestamp 2026-10-15 00:00:00, shared with the published specimen.
const DOS_DATE: u16 = ((2026 - 1980) << 9) | (10 << 5) | 15;
const DOS_TIME: u16 = 0;
const UNIX_FILE_0644: u32 = 0o100644 << 16;

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Canonical archive. `members` must already be in byte-wise name order.
pub fn write_stored(members: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in members {
        let offset = out.len() as u32;
        let crc = crc32fast::hash(data);
        let (size, nlen) = (data.len() as u32, name.len() as u16);
        // Local file header.
        out.extend_from_slice(&LOCAL_SIG.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // stored
        out.extend_from_slice(&DOS_TIME.to_le_bytes());
        out.extend_from_slice(&DOS_DATE.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra length
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        // Central directory entry.
        central.extend_from_slice(&CENTRAL_SIG.to_le_bytes());
        central.extend_from_slice(&0x0314u16.to_le_bytes()); // made by: Unix, 2.0
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&DOS_TIME.to_le_bytes());
        central.extend_from_slice(&DOS_DATE.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&nlen.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        central.extend_from_slice(&UNIX_FILE_0644.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let dir_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&EOCD_SIG.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]); // disk numbers
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&dir_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    out
}

fn codec(m: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, m.to_owned())
}

fn unsupported(m: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, m.to_owned())
}

/// Read every member. `cap` gives the maximum size for a member name, or `None`
/// to refuse the name. All structural checks happen before any member is copied.
pub fn read(
    bytes: &[u8],
    max_entries: usize,
    cap: impl Fn(&str) -> Option<u64>,
) -> Result<Vec<(String, Vec<u8>)>> {
    // End of central directory: scan back over at most a 64 KiB comment.
    let lowest = bytes.len().saturating_sub(22 + 65_535);
    let eocd = (lowest..=bytes.len().saturating_sub(22))
        .rev()
        .find(|&i| {
            u32le(bytes, i) == Some(EOCD_SIG)
                && u16le(bytes, i + 20).is_some_and(|c| i + 22 + usize::from(c) == bytes.len())
        })
        .ok_or_else(|| codec("not a ZIP archive"))?;
    let (disk, dir_disk, here, total) = (
        u16le(bytes, eocd + 4).unwrap_or(1),
        u16le(bytes, eocd + 6).unwrap_or(1),
        u16le(bytes, eocd + 8).unwrap_or(0),
        u16le(bytes, eocd + 10).unwrap_or(0),
    );
    if disk != 0 || dir_disk != 0 || here != total {
        return Err(unsupported("multi-disk archives are not supported"));
    }
    let (dir_size, dir_offset) = (
        u32le(bytes, eocd + 12).unwrap_or(0),
        u32le(bytes, eocd + 16).unwrap_or(0),
    );
    if total == 0xFFFF || dir_size == 0xFFFF_FFFF || dir_offset == 0xFFFF_FFFF {
        return Err(unsupported("ZIP64 archives are not supported"));
    }
    if usize::from(total) > max_entries {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "too many archive entries",
        ));
    }
    let (dir_offset, dir_size) = (dir_offset as usize, dir_size as usize);
    if dir_offset.checked_add(dir_size) != Some(eocd) {
        return Err(codec(
            "central directory does not end at the end-of-directory record",
        ));
    }

    struct Central {
        name: String,
        crc: u32,
        size: u32,
        offset: usize,
    }
    let mut entries = Vec::with_capacity(usize::from(total));
    let mut at = dir_offset;
    let mut seen = HashSet::new();
    for _ in 0..total {
        let bad = || codec("malformed central directory");
        if u32le(bytes, at) != Some(CENTRAL_SIG) {
            return Err(bad());
        }
        let flags = u16le(bytes, at + 8).ok_or_else(bad)?;
        let method = u16le(bytes, at + 10).ok_or_else(bad)?;
        let crc = u32le(bytes, at + 16).ok_or_else(bad)?;
        let (csize, size) = (
            u32le(bytes, at + 20).ok_or_else(bad)?,
            u32le(bytes, at + 24).ok_or_else(bad)?,
        );
        let (nlen, elen, clen) = (
            usize::from(u16le(bytes, at + 28).ok_or_else(bad)?),
            usize::from(u16le(bytes, at + 30).ok_or_else(bad)?),
            usize::from(u16le(bytes, at + 32).ok_or_else(bad)?),
        );
        let attrs = u32le(bytes, at + 38).ok_or_else(bad)?;
        let offset = u32le(bytes, at + 42).ok_or_else(bad)? as usize;
        let name = bytes.get(at + 46..at + 46 + nlen).ok_or_else(bad)?;
        let name = std::str::from_utf8(name)
            .map_err(|_| codec("member name is not UTF-8"))?
            .to_owned();
        if flags & 1 != 0 {
            return Err(unsupported("encrypted member"));
        }
        if flags != 0 {
            return Err(unsupported(
                "member uses unsupported flags (data descriptor or extension)",
            ));
        }
        if method != 0 || csize != size {
            return Err(unsupported("only stored members are supported"));
        }
        if (attrs >> 16) & 0o170_000 == 0o120_000 {
            return Err(codec("symbolic-link member"));
        }
        if name.is_empty()
            || name.starts_with('/')
            || name.ends_with('/')
            || name.contains('\\')
            || name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
        {
            return Err(codec("unsafe member path"));
        }
        if !seen.insert(name.clone()) {
            return Err(codec("duplicate member name"));
        }
        let Some(limit) = cap(&name) else {
            return Err(codec("unexpected member"));
        };
        if u64::from(size) > limit {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "member exceeds its size limit",
            ));
        }
        entries.push(Central {
            name,
            crc,
            size,
            offset,
        });
        at += 46 + nlen + elen + clen;
    }
    if at != eocd {
        return Err(codec("trailing bytes in central directory"));
    }

    // Local headers must agree with the central directory; members may not overlap.
    let mut spans = Vec::with_capacity(entries.len());
    let mut out = Vec::with_capacity(entries.len());
    for e in &entries {
        let bad = || codec("local header disagrees with central directory");
        if u32le(bytes, e.offset) != Some(LOCAL_SIG) {
            return Err(bad());
        }
        let (flags, method) = (
            u16le(bytes, e.offset + 6).ok_or_else(bad)?,
            u16le(bytes, e.offset + 8).ok_or_else(bad)?,
        );
        let (crc, size) = (
            u32le(bytes, e.offset + 14).ok_or_else(bad)?,
            u32le(bytes, e.offset + 22).ok_or_else(bad)?,
        );
        let (nlen, elen) = (
            usize::from(u16le(bytes, e.offset + 26).ok_or_else(bad)?),
            usize::from(u16le(bytes, e.offset + 28).ok_or_else(bad)?),
        );
        let name = bytes
            .get(e.offset + 30..e.offset + 30 + nlen)
            .ok_or_else(bad)?;
        if flags != 0 || method != 0 || crc != e.crc || size != e.size || name != e.name.as_bytes()
        {
            return Err(bad());
        }
        let start = e.offset + 30 + nlen + elen;
        let end = start
            .checked_add(e.size as usize)
            .filter(|&end| end <= dir_offset)
            .ok_or_else(|| codec("member data outside the archive"))?;
        let data = &bytes[start..end];
        if crc32fast::hash(data) != e.crc {
            return Err(codec("member CRC-32 mismatch"));
        }
        spans.push((e.offset, end));
        out.push((e.name.clone(), data.to_vec()));
    }
    spans.sort_unstable();
    if spans.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err(codec("overlapping members"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<(String, Vec<u8>)> {
        vec![
            ("a.bin".into(), vec![1, 2, 3]),
            ("b.json".into(), b"{}".to_vec()),
            ("empty".into(), vec![]),
        ]
    }

    fn any(_: &str) -> Option<u64> {
        Some(1 << 20)
    }

    #[test]
    fn round_trips_members_in_order() {
        let z = write_stored(&sample());
        assert_eq!(read(&z, 10, any).unwrap(), sample());
    }

    #[test]
    fn matches_the_published_specimen_header_layout() {
        let z = write_stored(&sample());
        assert_eq!(&z[..4], b"PK\x03\x04");
        assert_eq!(u16le(&z, 4), Some(20));
        assert_eq!(u16le(&z, 10), Some(0));
        assert_eq!(u16le(&z, 12), Some(DOS_DATE));
    }

    #[test]
    fn rejects_damage() {
        let z = write_stored(&sample());
        let code = |b: &[u8]| read(b, 10, any).unwrap_err().code;
        assert_eq!(code(b"not a zip"), ErrorCode::CodecFailed);
        let mut flipped = z.clone();
        flipped[30 + 5] ^= 1; // first member's data
        assert_eq!(code(&flipped), ErrorCode::CodecFailed);
        assert_eq!(code(&z[..z.len() - 1]), ErrorCode::CodecFailed);
        let mut trailing = z.clone();
        trailing.extend_from_slice(b"junk");
        assert_eq!(code(&trailing), ErrorCode::CodecFailed);
        assert_eq!(read(&z, 2, any).unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(
            read(&z, 10, |n| (n != "b.json").then_some(100))
                .unwrap_err()
                .code,
            ErrorCode::CodecFailed
        );
        assert_eq!(
            read(&z, 10, |_| Some(2)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }

    #[test]
    fn rejects_unsafe_and_duplicate_names() {
        for name in ["../x", "/x", "a//b", "dir/", "a\\b", "./x"] {
            let z = write_stored(&[(name.to_owned(), vec![])]);
            assert_eq!(
                read(&z, 10, any).unwrap_err().code,
                ErrorCode::CodecFailed,
                "{name}"
            );
        }
        let z = write_stored(&[("x".into(), vec![1]), ("x".into(), vec![2])]);
        assert_eq!(read(&z, 10, any).unwrap_err().code, ErrorCode::CodecFailed);
    }

    #[test]
    fn rejects_compression_and_encryption_flags() {
        let mut z = write_stored(&sample());
        let central = u32le(&z, z.len() - 6).unwrap() as usize;
        z[central + 10] = 8; // deflate
        assert_eq!(
            read(&z, 10, any).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let mut z = write_stored(&sample());
        z[central + 8] = 1; // encrypted
        assert_eq!(
            read(&z, 10, any).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
