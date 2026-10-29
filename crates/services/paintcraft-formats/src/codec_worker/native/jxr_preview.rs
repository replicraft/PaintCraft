//! Actual decode of a sealed writer result; no path-selected or host codec.
use super::super::jxr_encode::Layout;
use super::*;
const RGB_SCRATCH: usize = 49152;
pub const JXR_PREVIEW_SIDE: u32 = 192;
/// Decode stage: retained encoded result, one source copy, child/response,
/// RGB seal scratch and three bounded thumbnail/upload copies. Logical only.
pub fn jxr_preview_reservation_bytes(layout: Layout) -> Result<u64> {
    layout
        .encoded_cap()
        .checked_mul(2)
        .and_then(|n| n.checked_add(HEAP_BYTES + OUTPUT_BYTES_MAX as u64))
        .and_then(|n| {
            n.checked_add(u64::from(JXR_PREVIEW_SIDE).pow(2) * 4 * 3 + RGB_SCRATCH as u64 + 65536)
        })
        .ok_or_else(|| resource("JXR preview reservation overflow"))
}
impl PreparedJxrEncode {
    /// Caller admits the full preview quote before invoking. The launch callback
    /// must supervise/reap/join exactly as native Job::consume; borrowed pixels
    /// cannot outlive the validated response. No document is created or adopted.
    pub fn consume_decoded_preview<T>(
        &self,
        deadline: Duration,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
        consume: impl FnOnce(u32, u32, &[u8], &dyn Fn() -> Result<()>) -> Result<T>,
    ) -> Result<T> {
        if deadline.is_zero() || deadline > Duration::from_secs(120) {
            return Err(invalid("Invalid JXR preview deadline"));
        }
        let start = Instant::now();
        let owner = self.header().owner();
        let check = || {
            host_check()?;
            if current_owner()? != owner {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "JXR preview owner changed",
                ));
            }
            if start.elapsed() >= deadline {
                return Err(resource("JXR preview deadline exceeded"));
            }
            Ok(())
        };
        self.check_current(&check, current_owner)?;
        let layout = self.header().layout();
        jxr_preview_reservation_bytes(layout)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.bytes().len())
            .map_err(|_| resource("JXR preview source allocation"))?;
        if bytes.capacity() as u64 > layout.encoded_cap() {
            return Err(resource("JXR preview source capacity"));
        }
        for chunk in self.bytes().chunks(65536) {
            check()?;
            bytes.extend_from_slice(chunk);
        }
        let request = Request::capture_for(
            Codec::JxrRgb8,
            Arc::new(bytes),
            owner,
            u64::from(layout.width()) * u64::from(layout.height()),
        )?;
        if request.header.sha256 != self.sha256() {
            return Err(invalid("JXR preview encoded source identity"));
        }
        let helper = writer_import::Helper::writer(env!("CARGO_PKG_VERSION"), &check)?;
        if helper.sha256() != self.helper_sha256() {
            return Err(invalid("JXR preview decoder differs from encoder helper"));
        }
        let executable = helper.checked_executable(&check)?;
        let backing = launch::Backing::selected(&request, true, &check)?;
        let remaining = deadline
            .checked_sub(start.elapsed())
            .ok_or_else(|| resource("JXR preview deadline exceeded"))?;
        let output = backing.invoke(executable, &request, remaining, &check, launch)?;
        helper.checked_executable(&check)?;
        self.check_current(&check, current_owner)?;
        let frame = decode(&output, &request, current_owner()?)?;
        let rgba = validated_pixels(&frame, layout, self.header().rgb_sha256(), &check)?;
        let value = consume(frame.width, frame.height, rgba, &check)?;
        helper.checked_executable(&check)?;
        self.check_current(&check, current_owner)?;
        check()?;
        Ok(value)
    }
}
fn validated_pixels<'a>(
    frame: &Frame<'a>,
    layout: Layout,
    expected: [u8; 32],
    check: &dyn Fn() -> Result<()>,
) -> Result<&'a [u8]> {
    check()?;
    if (frame.width, frame.height) != (layout.width(), layout.height()) {
        return Err(invalid(
            "JXR preview geometry/alpha differs from sealed encode",
        ));
    }
    let Pixels::SrgbRgba8(rgba) = frame.pixels else {
        return Err(invalid("JXR preview requires actual sRGB RGBA8"));
    };
    if rgba.len() as u64 != u64::from(frame.width) * u64::from(frame.height) * 4 {
        return Err(invalid("JXR preview RGBA extent"));
    }
    let mut scratch = [0u8; RGB_SCRATCH];
    let mut hash = Sha256::new();
    for chunk in rgba.chunks(65536) {
        check()?;
        let mut at = 0;
        for pixel in chunk.as_chunks::<4>().0 {
            if pixel[3] != 255 {
                return Err(invalid("JXR preview contains nonopaque samples"));
            }
            scratch[at..at + 3].copy_from_slice(&pixel[..3]);
            at += 3;
        }
        hash.update(&scratch[..at]);
    }
    if <[u8; 32]>::from(hash.finalize()) != expected {
        return Err(invalid(
            "JXR decoded RGB differs from sealed lossless input",
        ));
    }
    check()?;
    Ok(rgba)
}

#[cfg(test)]
#[path = "jxr_preview/tests.rs"]
mod tests;
