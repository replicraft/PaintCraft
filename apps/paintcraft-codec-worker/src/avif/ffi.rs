//! Narrow trusted ABI to the selected static decoder; never used in the UI process.
#![allow(unsafe_code)]
use paintcraft_core::types::{Error, ErrorCode, Result};
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub(super) struct Info {
    pub width: u32,
    pub height: u32,
    pub layout: u32,
    pub depth: u32,
    pub primaries: u32,
    pub transfer: u32,
    pub matrix: u32,
    pub full_range: u32,
    pub chroma_position: u32,
    pub still_picture: u32,
    pub reduced_header: u32,
}
unsafe extern "C" {
    fn paintcraft_av1_decode(
        input: *const u8,
        length: usize,
        output: *mut u8,
        output_length: usize,
        info: *mut Info,
    ) -> i32;
}
#[derive(Debug)]
pub(super) struct Decoded {
    pub info: Info,
    pub planes: Vec<u8>,
}
/// Three full-area slots contain tightly packed native planes. No color,
/// transfer, alpha, item graph or public-format policy is inferred here.
pub(super) fn decode(bytes: &[u8], width: u32, height: u32) -> Result<Decoded> {
    let pixels = u64::from(width) * u64::from(height);
    if bytes.is_empty()
        || bytes.len() > 64 << 20
        || width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || pixels > 4_194_304
    {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Native AV1 input or output extent exceeds allowance",
        ));
    }
    let mut planes = vec![0; pixels as usize * 3];
    let mut info = Info::default();
    // SAFETY: both slices remain live and exclusive across this synchronous call;
    // Info exactly matches the private C struct. C checks output size and stride
    // before copying and retires every decoder/data/picture handle before return.
    let result = unsafe {
        paintcraft_av1_decode(
            bytes.as_ptr(),
            bytes.len(),
            planes.as_mut_ptr(),
            planes.len(),
            &mut info,
        )
    };
    if result != 0 {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            format!("Native AV1 decode failed ({result})"),
        ));
    }
    if (info.width, info.height) != (width, height) || info.depth != 8 || info.layout > 3 {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "Native AV1 dimensions/depth/layout disagree with inspected source",
        ));
    }
    Ok(Decoded { info, planes })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    fn sample() -> Vec<u8> {
        let image = paintcraft_formats::avif::encode_rgba8(
            16,
            16,
            &[80, 90, 100, 255].repeat(256),
            100,
            &|| Ok(()),
        )
        .unwrap()
        .bytes;
        // Controlled current-writer fixture extraction only. Production item
        // offsets/associations must use the format service's shared item graph.
        let mut at = 0;
        while at + 8 <= image.len() {
            let n = u32::from_be_bytes(image[at..at + 4].try_into().unwrap()) as usize;
            assert!(n >= 8 && n <= image.len() - at);
            if &image[at + 4..at + 8] == b"mdat" {
                return image[at + 8..at + n].to_vec();
            }
            at += n;
        }
        panic!("controlled writer fixture has no media data");
    }
    #[test]
    fn real_static_decoder_reports_native_planes_and_rejects_wrong_geometry() {
        let raw = sample();
        let got = decode(&raw, 16, 16).unwrap();
        assert_eq!((got.info.layout, got.info.depth), (3, 8));
        assert_eq!(
            (
                got.info.primaries,
                got.info.transfer,
                got.info.matrix,
                got.info.full_range
            ),
            (1, 13, 0, 1)
        );
        assert_eq!(got.planes.len(), 16 * 16 * 3);
        for (plane, expected) in got.planes.as_chunks::<256>().0.iter().zip([90, 100, 80]) {
            assert!(plane.iter().all(|v| v.abs_diff(expected) <= 3));
        }
        assert!(decode(&raw, 15, 16).is_err());
        assert!(decode(&raw[..raw.len() / 2], 16, 16).is_err());
    }
    #[test]
    fn real_c_context_scratch_and_picture_denial_exit_without_unwinding() {
        for name in [
            "fixture_deny_context",
            "fixture_deny_scratch",
            "fixture_deny_picture",
        ] {
            let exact = format!("avif::ffi::tests::{name}");
            let child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &exact, "--ignored"])
                .output()
                .unwrap();
            assert_eq!(
                child.status.code(),
                Some(75),
                "{} {}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr)
            );
        }
    }
    #[test]
    fn actual_decoder_retires_native_allocations_before_returning_planes() {
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "avif::ffi::tests::fixture_allocation_balance",
                "--ignored",
            ])
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{} {}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }
    #[test]
    #[ignore = "private native allocator child"]
    fn fixture_allocation_balance() {
        let raw = sample();
        let before = crate::HEAP.live();
        let decoded = decode(&raw, 16, 16).unwrap();
        assert_eq!(crate::HEAP.live(), before + decoded.planes.capacity());
        drop(decoded);
        assert_eq!(crate::HEAP.live(), before);
    }
    fn denied(kind: u32) {
        let raw = sample();
        crate::avif::heap::FAIL_KIND.store(kind, std::sync::atomic::Ordering::Relaxed);
        let _ = decode(&raw, 16, 16);
        panic!("targeted native allocation was not reached");
    }
    #[test]
    #[ignore = "private native allocator child"]
    fn fixture_deny_context() {
        denied(4);
    }
    #[test]
    #[ignore = "private native allocator child"]
    fn fixture_deny_scratch() {
        denied(17);
    }
    #[test]
    #[ignore = "private native allocator child"]
    fn fixture_deny_picture() {
        denied(12);
    }
}
