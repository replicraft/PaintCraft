//! Strict, non-extracting ZIP32 subset for an active OpenRaster consumer.
//! Central and local records must agree before any payload inflation.
use super::*;
use flate2::{Decompress, FlushDecompress, Status};
use std::ops::Range;

#[derive(Debug)]
pub(super) struct Entry<'a> {
    pub name: &'a str,
    pub method: u16,
    pub directory: bool,
    payload: &'a [u8],
    pub uncompressed: usize,
    crc: u32,
    range: Range<usize>,
}
pub(super) struct Archive<'a> {
    pub entries: Vec<Entry<'a>>,
}
fn bytes(input: &[u8], offset: usize, n: usize) -> Result<&[u8]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(n)
                    .ok_or_else(|| malformed("ORA ZIP offset overflow"))?,
        )
        .ok_or_else(|| malformed("Truncated ORA ZIP record"))
}
fn u16le(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes(input, offset, 2)?.try_into().unwrap(),
    ))
}
fn u32le(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes(input, offset, 4)?.try_into().unwrap(),
    ))
}
pub(super) fn safe_path(path: &str, limits: &Limits, directory: bool) -> Result<()> {
    if path.is_empty()
        || path.len() > limits.path_bytes
        || path.starts_with('/')
        || path.contains(['\\', ':'])
        || path.chars().any(|c| c.is_control())
    {
        return Err(unsupported(
            "ORA archive paths must be safe relative UTF-8 names",
        ));
    }
    let name = if directory {
        path.strip_suffix('/')
            .ok_or_else(|| malformed("ORA directory lacks its slash"))?
    } else {
        path
    };
    if name.is_empty()
        || name
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || name.split('/').count() > 32
    {
        return Err(unsupported(
            "ORA archive path traversal or ambiguous components",
        ));
    }
    Ok(())
}
fn extra(input: &[u8]) -> Result<()> {
    let mut p = 0;
    while p < input.len() {
        let id = u16le(input, p)?;
        let size = usize::from(u16le(input, p + 2)?);
        p += 4;
        bytes(input, p, size)?;
        p += size;
        if ![0x5455, 0x000a, 0x7875].contains(&id) {
            return Err(unsupported(
                "ORA ZIP extension, alternate path or ZIP64 fields are not qualified",
            ));
        }
    }
    Ok(())
}
impl<'a> Archive<'a> {
    pub fn parse(
        input: &'a [u8],
        limits: &Limits,
        budget: &Budget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if input.len() > limits.archive_bytes {
            return Err(resource("ORA archive exceeds source byte limit"));
        }
        let start = input.len().saturating_sub(65_557);
        let end = (start..input.len().saturating_sub(21))
            .rev()
            .find(|&p| {
                input.get(p..p + 4) == Some(b"PK\x05\x06")
                    && u16le(input, p + 20)
                        .ok()
                        .and_then(|n| p.checked_add(22 + usize::from(n)))
                        == Some(input.len())
            })
            .ok_or_else(|| malformed("ORA lacks a complete ZIP directory terminator"))?;
        if u16le(input, end + 4)? != 0
            || u16le(input, end + 6)? != 0
            || u16le(input, end + 8)? != u16le(input, end + 10)?
        {
            return Err(unsupported("Multipart ORA ZIP archives are unsupported"));
        }
        let count = usize::from(u16le(input, end + 10)?);
        let size = u32le(input, end + 12)? as usize;
        let central = u32le(input, end + 16)? as usize;
        if count == 0xffff || size == u32::MAX as usize || central == u32::MAX as usize {
            return Err(unsupported("ZIP64 ORA archives are not qualified"));
        }
        if count == 0 || count > limits.entries || size > limits.metadata_bytes {
            return Err(resource("ORA ZIP directory exceeds metadata limits"));
        }
        if central.checked_add(size) != Some(end) {
            return Err(malformed("ORA ZIP central directory has an invalid range"));
        }
        let mut entries = budget.vector(count)?;
        let mut p = central;
        let mut total = 0usize;
        for _ in 0..count {
            check()?;
            let header = bytes(input, p, 46)?;
            if &header[..4] != b"PK\x01\x02" {
                return Err(malformed("Invalid ORA central directory record"));
            }
            let needed = u16le(header, 6)?;
            let flags = u16le(header, 8)?;
            let method = u16le(header, 10)?;
            if needed > 20
                || flags & !0x0806 != 0
                || !matches!(method, 0 | 8)
                || method == 0 && flags & 6 != 0
                || u16le(header, 34)? != 0
            {
                return Err(unsupported(
                    "ORA ZIP compression, encryption, descriptors or disk references are unsupported",
                ));
            }
            let crc = u32le(header, 16)?;
            let compressed = u32le(header, 20)? as usize;
            let uncompressed = u32le(header, 24)? as usize;
            if compressed == u32::MAX as usize || uncompressed == u32::MAX as usize {
                return Err(unsupported("ZIP64 ORA entry sizes are unsupported"));
            }
            if uncompressed > limits.entry_bytes {
                return Err(resource("ORA ZIP entry exceeds inflated byte limit"));
            }
            total = total
                .checked_add(uncompressed)
                .ok_or_else(|| resource("ORA inflated byte count overflow"))?;
            if total > limits.inflated_bytes {
                return Err(resource("ORA aggregate ZIP inflation exceeds byte limit"));
            }
            if method == 0 && compressed != uncompressed {
                return Err(malformed("ORA stored ZIP entry lengths disagree"));
            }
            let name_len = usize::from(u16le(header, 28)?);
            let extra_len = usize::from(u16le(header, 30)?);
            let comment_len = usize::from(u16le(header, 32)?);
            if name_len > limits.path_bytes {
                return Err(resource("ORA ZIP name exceeds limit"));
            }
            let name_bytes = bytes(input, p + 46, name_len)?;
            if flags & 0x800 == 0 && !name_bytes.is_ascii() {
                return Err(unsupported("Non-ASCII ORA ZIP names require UTF-8 flag"));
            }
            let name = std::str::from_utf8(name_bytes)
                .map_err(|_| malformed("ORA ZIP name is not UTF-8"))?;
            let attrs = u32le(header, 38)?;
            let directory = name.ends_with('/');
            safe_path(name, limits, directory)?;
            let mode = (attrs >> 16) & 0xf000;
            if ![0, 0x4000, 0x8000].contains(&mode)
                || (mode == 0x4000 || attrs & 0x10 != 0) && !directory
            {
                return Err(unsupported(
                    "ORA symlinks and special filesystem entries are unsupported",
                ));
            }
            if directory && (uncompressed != 0 || compressed != 0 || crc != 0 || method != 0) {
                return Err(malformed("ORA directory entry contains data"));
            }
            if entries.iter().any(|entry: &Entry<'_>| entry.name == name) {
                return Err(malformed("Duplicate ORA ZIP path"));
            }
            extra(bytes(input, p + 46 + name_len, extra_len)?)?;
            let next = p
                .checked_add(46 + name_len + extra_len + comment_len)
                .filter(|next| *next <= end)
                .ok_or_else(|| malformed("ORA directory entry exceeds its scope"))?;
            let offset = u32le(header, 42)? as usize;
            if offset == u32::MAX as usize {
                return Err(unsupported("ORA ZIP64 local offsets are unsupported"));
            }
            let local = bytes(input, offset, 30)?;
            if &local[..4] != b"PK\x03\x04"
                || u16le(local, 4)? != needed
                || u16le(local, 6)? != flags
                || u16le(local, 8)? != method
                || u16le(local, 10)? != u16le(header, 12)?
                || u16le(local, 12)? != u16le(header, 14)?
                || u32le(local, 14)? != crc
                || u32le(local, 18)? as usize != compressed
                || u32le(local, 22)? as usize != uncompressed
            {
                return Err(malformed("ORA ZIP local/central headers disagree"));
            }
            let local_name = usize::from(u16le(local, 26)?);
            let local_extra = usize::from(u16le(local, 28)?);
            if bytes(input, offset + 30, local_name)? != name_bytes {
                return Err(malformed("ORA ZIP local/central names disagree"));
            }
            extra(bytes(input, offset + 30 + local_name, local_extra)?)?;
            let payload_start = offset
                .checked_add(30 + local_name + local_extra)
                .ok_or_else(|| malformed("ORA local offset overflow"))?;
            let payload_end = payload_start
                .checked_add(compressed)
                .filter(|end| *end <= central)
                .ok_or_else(|| malformed("ORA ZIP payload exceeds data region"))?;
            let payload = bytes(input, payload_start, compressed)?;
            entries.push(Entry {
                name,
                method,
                directory,
                payload,
                uncompressed,
                crc,
                range: offset..payload_end,
            });
            p = next;
        }
        if p != end {
            return Err(malformed(
                "ORA ZIP entry count disagrees with central directory",
            ));
        }
        entries.sort_unstable_by_key(|entry| entry.range.start);
        check()?;
        if entries.first().is_none_or(|entry| entry.range.start != 0)
            || entries
                .last()
                .is_none_or(|entry| entry.range.end != central)
            || entries
                .windows(2)
                .any(|pair| pair[0].range.end != pair[1].range.start)
        {
            return Err(malformed(
                "ORA ZIP local payloads overlap or leave undeclared data",
            ));
        }
        if entries[0].name != "mimetype" {
            return Err(malformed("ORA mimetype must be the first local entry"));
        }
        Ok(Self { entries })
    }
    pub fn index(&self, name: &str) -> Result<usize> {
        self.entries
            .iter()
            .position(|entry| entry.name == name)
            .ok_or_else(|| malformed("Required ORA archive entry is missing"))
    }
    pub fn read(
        &self,
        index: usize,
        limits: &Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Cow<'a, [u8]>> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| malformed("ORA entry index exceeds directory"))?;
        if entry.uncompressed > limits.entry_bytes {
            return Err(resource("ORA ZIP inflation exceeds entry limit"));
        }
        let value = if entry.method == 0 {
            Cow::Borrowed(entry.payload)
        } else {
            let mut output = Vec::new();
            output
                .try_reserve_exact(entry.uncompressed)
                .map_err(|_| resource("Cannot allocate inflated ORA ZIP payload"))?;
            output.resize(entry.uncompressed, 0);
            let mut decoder = Decompress::new(false);
            let mut consumed: usize = 0;
            let mut written: usize = 0;
            let mut excess = [0u8; 1];
            loop {
                check()?;
                let old_in = decoder.total_in();
                let old_out = decoder.total_out();
                let input = &entry.payload
                    [consumed..entry.payload.len().min(consumed.saturating_add(65536))];
                let target = if written < output.len() {
                    let end = output.len().min(written.saturating_add(65536));
                    &mut output[written..end]
                } else {
                    &mut excess
                };
                let status = decoder
                    .decompress(input, target, FlushDecompress::None)
                    .map_err(|_| malformed("Invalid ORA ZIP DEFLATE stream"))?;
                let read = (decoder.total_in() - old_in) as usize;
                let produced = (decoder.total_out() - old_out) as usize;
                consumed += read;
                written += produced;
                if written > output.len() {
                    return Err(malformed("ORA ZIP inflated output exceeds declared size"));
                }
                if status == Status::StreamEnd {
                    if written != output.len() || consumed != entry.payload.len() {
                        return Err(malformed("ORA ZIP DEFLATE size/EOF mismatch"));
                    }
                    break;
                }
                if read == 0 && produced == 0 {
                    return Err(malformed("Truncated ORA ZIP DEFLATE stream"));
                }
            }
            Cow::Owned(output)
        };
        let mut crc = crc32fast::Hasher::new();
        for chunk in value.chunks(65536) {
            check()?;
            crc.update(chunk);
        }
        if crc.finalize() != entry.crc {
            return Err(malformed("ORA ZIP payload CRC mismatch"));
        }
        check()?;
        Ok(value)
    }
}
