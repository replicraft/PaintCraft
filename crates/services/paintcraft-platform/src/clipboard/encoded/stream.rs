//! Unknown-length providers receive a private capacity grant before opening.
use super::*;
const CHUNK: usize = 32 * 1024;
impl EncodedImage {
    /// Freeze a stream only after EOF; never ask a provider for text fallback.
    /// The private grant cannot be borrowed by callbacks or competing captures.
    pub fn capture_stream<R>(
        maximum: u64,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
        open: impl FnOnce() -> Result<(Encoding, R), String>,
        mut read: impl FnMut(&mut R, &mut [u8]) -> Result<usize, String>,
    ) -> Result<Self, String> {
        check()?;
        if maximum == 0 || maximum > MAX_ENCODED_BYTES {
            return Err("Clipboard encoded image exceeds the limit".into());
        }
        let grant = budget
            .reserve_capacity(maximum)
            .map_err(|e| e.to_string())?;
        check()?;
        let (encoding, mut source) = open()?;
        check()?;
        let mut bytes = Vec::new();
        let mut scratch = [0; CHUNK];
        loop {
            check()?;
            let remaining = maximum as usize - bytes.len();
            // Probe a single excess byte at the boundary without growing storage.
            let wanted = remaining.clamp(1, CHUNK);
            let count = read(&mut source, &mut scratch[..wanted])?;
            check()?;
            if count > wanted {
                return Err("Clipboard stream returned an invalid byte count".into());
            }
            if count == 0 {
                break;
            }
            if count > remaining {
                return Err("Clipboard encoded image exceeds the limit".into());
            }
            let needed = bytes.len() + count;
            if needed > bytes.capacity() {
                // Geometric growth also handles providers returning tiny fragments.
                let capacity = (bytes.capacity().max(1) * 2)
                    .max(needed)
                    .min(maximum as usize);
                bytes
                    .try_reserve_exact(capacity - bytes.len())
                    .map_err(|_| "Cannot allocate clipboard snapshot".to_owned())?;
            }
            if bytes.capacity() as u64 > maximum {
                return Err("Clipboard allocation exceeds the limit".into());
            }
            bytes.extend_from_slice(&scratch[..count]);
        }
        if bytes.is_empty() || !encoding.matches(&bytes) {
            return Err("Clipboard image signature does not match its advertised type".into());
        }
        let lease = grant
            .reserve(bytes.capacity() as u64)
            .map_err(|e| e.to_string())?;
        let image = Self {
            encoding,
            payload: Arc::new(Payload {
                bytes,
                _lease: lease,
                _extra: None,
            }),
        };
        check()?;
        // Dropping the private grant releases all capacity the payload did not use.
        Ok(image)
    }
}

#[cfg(test)]
mod tests;
