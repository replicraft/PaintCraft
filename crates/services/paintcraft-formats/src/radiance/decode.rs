//! Private qualification API: run only inside a bounded isolated worker before
//! any future frontend activation. Logical byte admission is not heap/RSS or
//! a deadline guarantee; caller retains source and process-wide confinement.
use super::*;
use image::ImageDecoder;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, SharedBudget};
use std::{
    cell::RefCell,
    io::{self, Cursor, Read},
};
#[derive(Debug)]
pub struct Decoded {
    samples: Vec<f32>,
    source_sha256: [u8; 32],
    // Storage drops before its output declaration.
    _account: BudgetAccount,
}
impl Decoded {
    /// As-stored scene-linear RGB, lower-edge RGBE representative. Header
    /// exposure/correction is neither applied again nor undone. No alpha.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
}
struct Checked<'a, R> {
    reader: R,
    check: &'a dyn Fn() -> Result<()>,
    error: &'a RefCell<Option<Error>>,
}
impl<R: Read> Read for Checked<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if let Err(error) = (self.check)() {
            *self.error.borrow_mut() = Some(error);
            return Err(io::Error::other("HDR qualification cancelled"));
        }
        self.reader.read(output)
    }
}
/// Decode the body sealed by inspection; no unrelated source/body argument is
/// accepted. Pixel buffers plus selected decoder's width*4 scratch are admitted
/// before invoking it. The canonical header excludes original untrusted records
/// from backend parsing; all original records remain available on Inspection.
pub fn decode(
    admitted: &Inspection<'_>,
    budget: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    check()?;
    let width = admitted.header.width;
    let height = admitted.header.height;
    let count = width as usize * height as usize * 3;
    let output = count * 4;
    let mut account = budget.account(AllocationClass::DecodedTiles);
    // Two full RGB32F buffers during safe byte→float conversion; selected
    // backend allocates one RGBE scanline. Small canonical header is fixed-bound.
    account.resize((output * 2 + width as usize * 4 + 4096) as u64)?;
    let canonical = format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y {height} +X {width}\n");
    let error = RefCell::new(None);
    let reader = Checked {
        reader: Cursor::new(canonical.as_bytes()).chain(Cursor::new(admitted.body)),
        check,
        error: &error,
    };
    let decoder = image::codecs::hdr::HdrDecoder::new(reader).map_err(|_| {
        error
            .borrow_mut()
            .take()
            .unwrap_or_else(|| invalid("HDR qualified header decode failed"))
    })?;
    if decoder.dimensions() != (width, height) || decoder.total_bytes() != output as u64 {
        return Err(invalid("HDR decoder disagrees with sealed dimensions"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(output)
        .map_err(|_| resource("HDR float output allocation"))?;
    bytes.resize(output, 0);
    account.resize((bytes.capacity() + output + width as usize * 4 + 4096) as u64)?;
    let result = decoder.read_image(&mut bytes);
    if let Some(error) = error.into_inner() {
        return Err(error);
    }
    result.map_err(|_| invalid("HDR qualified packet decode failed"))?;
    check()?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(count)
        .map_err(|_| resource("HDR sample allocation"))?;
    account
        .resize((bytes.capacity() + samples.capacity() * 4 + width as usize * 4 + 4096) as u64)?;
    for (index, word) in bytes.as_chunks::<4>().0.iter().enumerate() {
        if index % 3072 == 0 {
            check()?;
        }
        samples.push(f32::from_ne_bytes(*word));
    }
    drop(bytes);
    account.resize((samples.capacity() * 4) as u64)?;
    check()?;
    Ok(Decoded {
        samples,
        source_sha256: admitted.sha256,
        _account: account,
    })
}
#[cfg(test)]
mod tests;
