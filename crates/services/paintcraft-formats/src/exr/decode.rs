//! Optional private decoder qualification. This API must run in bounded isolated
//! execution before any future frontend activation. No hard deadline/RSS claim.
mod metadata;
use super::*;
use ::exr::prelude::traits::{ReadChannels, ReadLayers, read};
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, SharedBudget};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
#[derive(Debug)]
pub struct Decoded {
    samples: Vec<f32>,
    source_sha256: [u8; 32],
    origin: [i32; 2],
    alpha: bool,
    _account: BudgetAccount,
}
impl Decoded {
    /// Exact interleaved RGB or RGBA float32 in named R,G,B,A order. No source
    /// color assignment, alpha unassociation or precision conversion occurs.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    pub fn origin(&self) -> [i32; 2] {
        self.origin
    }
    pub fn alpha(&self) -> bool {
        self.alpha
    }
}
struct Checked<'a> {
    source: Cursor<&'a [u8]>,
    check: &'a dyn Fn() -> Result<()>,
    error: &'a RefCell<Option<Error>>,
}
impl Checked<'_> {
    fn check(&self) -> io::Result<()> {
        if self.error.borrow().is_some() {
            return Err(io::Error::other("EXR qualification cancelled"));
        }
        if let Err(error) = (self.check)() {
            *self.error.borrow_mut() = Some(error);
            return Err(io::Error::other("EXR qualification cancelled"));
        }
        Ok(())
    }
}
impl Read for Checked<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        self.source.read(out)
    }
}
impl Seek for Checked<'_> {
    fn seek(&mut self, at: SeekFrom) -> io::Result<u64> {
        self.check()?;
        self.source.seek(at)
    }
}
/// Accepts only the original source sealed by successful bounded preflight.
/// Pre-admits output and conservative logical metadata/block scratch; this is
/// not a replacement for a future worker's hard requested-heap ceiling.
pub fn decode(
    admitted: &Inspection<'_>,
    budget: SharedBudget,
    check: &(dyn Fn() -> Result<()> + Sync),
) -> Result<Decoded> {
    check()?;
    metadata::validate(&admitted.header)?;
    let header = &admitted.header;
    let channels = header.channels.len();
    let count = header.width as usize * header.height as usize * channels;
    let pixel_bytes: usize = header
        .channels
        .iter()
        .map(|c| c.sample_type.bytes() as usize)
        .sum();
    let block = header.width as usize
        * (header.height as usize).min(if header.compression == 3 { 16 } else { 1 })
        * pixel_bytes;
    let scratch = 4 * (1 << 20) + block * 8;
    let mut account = budget.account(AllocationClass::DecodedTiles);
    account.resize((count * 4 + scratch) as u64)?;
    // The selected codec retains a thread-local interleave buffer. Keep the
    // caller-owned charge across join, including decoder error and unwind.
    let samples = std::thread::scope(|scope| {
        let task = std::thread::Builder::new()
            .spawn_scoped(scope, || {
                decode_on_thread(admitted, &mut account, scratch, check)
            })
            .map_err(|_| resource("EXR qualification thread creation failed"))?;
        task.join()
            .map_err(|_| invalid("EXR qualification decoder thread panicked"))?
    })?;
    // Join precedes release of the private scratch declaration. This is not
    // killable cancellation, a hard deadline, stack admission or an RSS bound.
    check()?;
    account.resize((samples.capacity() * 4) as u64)?;
    Ok(Decoded {
        samples,
        source_sha256: admitted.sha256,
        origin: header.origin,
        alpha: channels == 4,
        _account: account,
    })
}
fn decode_on_thread(
    admitted: &Inspection<'_>,
    account: &mut BudgetAccount,
    scratch: usize,
    check: &(dyn Fn() -> Result<()> + Sync),
) -> Result<Vec<f32>> {
    let header = &admitted.header;
    let width = header.width as usize;
    let height = header.height as usize;
    let alpha = header.channels.len() == 4;
    let channels = header.channels.len();
    let count = width * height * channels;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(count)
        .map_err(|_| resource("EXR float output allocation"))?;
    samples.resize(count, 0.0);
    account.resize((samples.capacity() * 4 + scratch) as u64)?;
    let storage = Rc::new(RefCell::new(Some(samples)));
    let callbacks = Rc::new(Cell::new(0usize));
    let mismatch = Rc::new(Cell::new(false));
    let error = RefCell::new(None);
    let create_storage = storage.clone();
    let create_mismatch = mismatch.clone();
    let set_callbacks = callbacks.clone();
    let set_mismatch = mismatch.clone();
    let reader = Checked {
        source: Cursor::new(admitted.source),
        check,
        error: &error,
    };
    let image = read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            move |size, _| {
                if size.width() != width || size.height() != height {
                    create_mismatch.set(true);
                }
                create_storage.borrow_mut().take().unwrap_or_default()
            },
            move |pixels: &mut Vec<f32>, position, (r, g, b, a): (f32, f32, f32, f32)| {
                set_callbacks.set(set_callbacks.get() + 1);
                if position.x() >= width || position.y() >= height {
                    set_mismatch.set(true);
                    return;
                }
                let index = (position.y() * width + position.x()) * channels;
                if let Some(target) = pixels.get_mut(index..index + channels) {
                    target[..3].copy_from_slice(&[r, g, b]);
                    if alpha {
                        target[3] = a;
                    }
                } else {
                    set_mismatch.set(true);
                }
            },
        )
        .first_valid_layer()
        .all_attributes()
        .non_parallel()
        .pedantic()
        .from_buffered(reader);
    if let Some(error) = error.into_inner() {
        return Err(error);
    }
    let image = image.map_err(|_| invalid("EXR qualified sample decoding failed"))?;
    if mismatch.get()
        || callbacks.get() != width * height
        || image.layer_data.size.width() != width
        || image.layer_data.size.height() != height
        || image.layer_data.attributes.layer_position.0 != header.origin[0]
        || image.layer_data.attributes.layer_position.1 != header.origin[1]
    {
        return Err(invalid("EXR decoder disagrees with sealed sample geometry"));
    }
    let samples = image.layer_data.channel_data.pixels;
    drop(image.layer_data.channel_data.channels);
    drop(image.layer_data.attributes);
    drop(image.attributes);
    check()?;
    Ok(samples)
}
#[cfg(test)]
mod tests;
