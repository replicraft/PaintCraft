//! Encoded CLIPBOARD reads; never query PRIMARY or request text conversions.
use super::{EncodedImage, SharedBudget};
use crate::clipboard::{Encoding, MAX_ENCODED_BYTES};
use client::{Client, Reading};
use x11rb::protocol::xproto::AtomEnum;
mod client;
mod transfer;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) fn session_available() -> bool {
    std::env::var_os("DISPLAY").is_some_and(|name| !name.is_empty())
}
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) struct NativeReader;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
impl super::Reader for NativeReader {
    fn read(
        &mut self,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<EncodedImage, String> {
        read_selection(None, b"CLIPBOARD", budget, check)
    }
}
struct Source {
    client: Client,
    reading: Reading,
    timestamp: Option<u32>,
    pending: Vec<u8>,
    position: usize,
    finished: bool,
}
fn stamp(client: &Client, check: &dyn Fn() -> Result<(), String>) -> Result<u32, String> {
    let values = client.words(client.atoms.timestamp, AtomEnum::INTEGER.into(), 1, check)?;
    match values.as_slice() {
        [value] if *value != 0 => Ok(*value),
        _ => Err("Invalid X11 clipboard ownership timestamp".into()),
    }
}
impl Source {
    fn open(
        display: Option<&str>,
        selection: &[u8],
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<(Encoding, Self), String> {
        check()?;
        let client = Client::connect(display, selection)?;
        let targets = client.words(client.atoms.targets, AtomEnum::ATOM.into(), 256, check)?;
        let timestamp = if targets.contains(&client.atoms.timestamp) {
            Some(stamp(&client, check)?)
        } else {
            None
        };
        let (encoding, target) = if targets.contains(&client.atoms.png) {
            (Encoding::Png, client.atoms.png)
        } else if targets.contains(&client.atoms.tiff) {
            (Encoding::Tiff, client.atoms.tiff)
        } else {
            return Err("X11 clipboard does not offer a supported PNG or TIFF image".into());
        };
        let reading = client.start(target, target, 8, MAX_ENCODED_BYTES as usize, check)?;
        Ok((
            encoding,
            Self {
                client,
                reading,
                timestamp,
                pending: Vec::new(),
                position: 0,
                finished: false,
            },
        ))
    }
    fn read(
        &mut self,
        out: &mut [u8],
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<usize, String> {
        check()?;
        if self.finished {
            return Ok(0);
        }
        if self.position == self.pending.len() {
            self.client.same_owner(check)?;
            if let Some(bytes) = self.client.next(&mut self.reading, check)? {
                self.pending = bytes;
                self.position = 0;
            } else {
                if let Some(timestamp) = self.timestamp
                    && stamp(&self.client, check)? != timestamp
                {
                    return Err("X11 clipboard timestamp changed during reading".into());
                }
                self.client.same_owner(check)?;
                self.finished = true;
                return Ok(0);
            }
        }
        let count = out.len().min(self.pending.len() - self.position);
        out[..count].copy_from_slice(&self.pending[self.position..self.position + count]);
        self.position += count;
        check()?;
        Ok(count)
    }
}
fn read_selection(
    display: Option<&str>,
    selection: &[u8],
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<EncodedImage, String> {
    EncodedImage::capture_stream(
        MAX_ENCODED_BYTES,
        budget,
        check,
        || Source::open(display, selection, check),
        |source, out| source.read(out, check),
    )
}

#[cfg(test)]
mod native_tests;
