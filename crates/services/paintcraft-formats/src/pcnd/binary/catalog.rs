//! Bounded PCND6 requirements, borrowed before retaining document payloads.
use super::*;
mod params;
mod stack;
pub(super) use stack::{read_stack, stack_bytes, write_stack};
const ENTRIES_MAX: usize = 256;
const PRELUDE_MAX: u64 = 65_536;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub profile_id: String,
    pub codec_id: String,
}
#[derive(Clone, Copy)]
struct Entry<'a> {
    profile: &'a str,
    codec: &'a str,
}
pub(super) struct Manifest<'a> {
    entries: [Option<Entry<'a>>; ENTRIES_MAX],
    used: [bool; ENTRIES_MAX],
    len: usize,
}
fn invalid() -> Error {
    codec("invalid PCND catalog framing")
}
fn identifier(value: &[u8]) -> Result<&str> {
    let value = std::str::from_utf8(value).map_err(|_| invalid())?;
    let (name, version) = value.rsplit_once("-v").ok_or_else(invalid)?;
    if !name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || version.is_empty()
        || version.starts_with('0')
        || !version.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    Ok(value)
}
fn number_bytes(mut n: u64) -> u64 {
    let mut bytes = 1;
    while n >= 128 {
        n >>= 7;
        bytes += 1;
    }
    bytes
}
impl<'a> Manifest<'a> {
    fn empty() -> Self {
        Self {
            entries: [None; ENTRIES_MAX],
            used: [false; ENTRIES_MAX],
            len: 0,
        }
    }
    pub(super) fn read(
        input: &mut Input<'a>,
        supported: bool,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let length = input.number(PRELUDE_MAX)? as usize;
        let mut prelude = Input::new(input.take(length)?);
        let count = prelude.number(ENTRIES_MAX as u64)? as usize;
        if count == 0 {
            return Err(invalid());
        }
        let mut manifest = Self::empty();
        for index in 0..count {
            check()?;
            let length = prelude.number(96)? as usize;
            let profile = identifier(prelude.take(length)?)?;
            let length = prelude.number(64)? as usize;
            let codec = identifier(prelude.take(length)?)?;
            if index > 0 && manifest.entries[index - 1].unwrap().profile >= profile {
                return Err(invalid());
            }
            manifest.entries[index] = Some(Entry { profile, codec });
            manifest.len += 1;
        }
        if prelude.remaining() != 0 {
            return Err(invalid());
        }
        if supported {
            for entry in manifest.entries[..count].iter().flatten() {
                check()?;
                params::require(entry.profile, entry.codec)?;
            }
        }
        Ok(manifest)
    }
    pub(super) fn capture(
        doc: &Document,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Manifest<'static>> {
        let mut manifest = Manifest::empty();
        for layer in doc.layers.values() {
            check()?;
            for filter in layer.filters.iter() {
                check()?;
                let profile = filter.descriptor().id();
                if !manifest.entries[..manifest.len]
                    .iter()
                    .flatten()
                    .any(|e| e.profile == profile)
                {
                    if manifest.len == ENTRIES_MAX {
                        return Err(invalid());
                    }
                    manifest.entries[manifest.len] = Some(Entry {
                        profile,
                        codec: "params-v1",
                    });
                    manifest.len += 1;
                }
            }
        }
        if manifest.len == 0 {
            return Err(invalid());
        }
        manifest.entries[..manifest.len].sort_unstable_by_key(|e| e.unwrap().profile);
        Ok(manifest)
    }
    fn body_bytes(&self) -> u64 {
        number_bytes(self.len as u64)
            + self.entries[..self.len]
                .iter()
                .flatten()
                .map(|e| {
                    number_bytes(e.profile.len() as u64)
                        + e.profile.len() as u64
                        + number_bytes(e.codec.len() as u64)
                        + e.codec.len() as u64
                })
                .sum::<u64>()
    }
    pub(super) fn wire_bytes(&self) -> u64 {
        let body = self.body_bytes();
        number_bytes(body) + body
    }
    pub(super) fn write<W: Write>(
        &self,
        output: &mut Output<W>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let length = self.body_bytes();
        if length > PRELUDE_MAX {
            return Err(invalid());
        }
        output.number(length)?;
        output.number(self.len as u64)?;
        for entry in self.entries[..self.len].iter().flatten() {
            check()?;
            output.string(entry.profile)?;
            output.string(entry.codec)?;
        }
        Ok(())
    }
    pub(super) fn finish(&self) -> Result<()> {
        if self.used[..self.len].iter().any(|v| !v) {
            return Err(codec("unused PCND catalog requirement"));
        }
        Ok(())
    }
    pub(super) fn requirements(&self, check: &dyn Fn() -> Result<()>) -> Result<Vec<Requirement>> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.len).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "PCND catalog assessment allocation",
            )
        })?;
        for entry in self.entries[..self.len].iter().flatten() {
            check()?;
            let owned = |value: &str| -> Result<String> {
                let mut text = String::new();
                text.try_reserve_exact(value.len()).map_err(|_| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "PCND catalog identifier allocation",
                    )
                })?;
                text.push_str(value);
                Ok(text)
            };
            rows.push(Requirement {
                profile_id: owned(entry.profile)?,
                codec_id: owned(entry.codec)?,
            });
        }
        check()?;
        Ok(rows)
    }
}
