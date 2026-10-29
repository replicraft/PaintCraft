//! Native integer JXL family. Header decoding stays inside the isolated worker.
pub mod contracts;
mod report;
use super::*;
pub use report::{POLICY, REPORT_BYTES, Report};

pub fn execute(header: Header, source: &[u8], peak: &dyn Fn() -> u64) -> Result<Vec<u8>> {
    if header.codec != Codec::JxlIntegerRgba8 {
        return Err(unsupported("Integer JXL requires its exact codec tag"));
    }
    header.verify_source(source)?;
    let decoded =
        crate::jxl::decode_worker_integer_checked(source, u64::from(header.pixels), &|| Ok(()))?;
    if !decoded.losses.is_empty() {
        return Err(invalid("Integer JXL profile cannot discard metadata"));
    }
    let (rgba, depth, changed_rgb_components, changed_alpha) = match decoded.samples {
        crate::jxl::Samples::Rgba8(rgba) => (rgba, 8, 0, 0),
        crate::jxl::Samples::Rgba16(samples) => {
            let converted = crate::jxl::convert_rgba16_checked(
                decoded.width,
                decoded.height,
                &samples,
                header.sha256,
                PIXELS_MAX as usize * 4,
                &|| Ok(()),
            )?;
            (
                converted.rgba,
                16,
                converted.assessment.changed_rgb_components,
                converted.assessment.changed_alpha,
            )
        }
    };
    let report = Report {
        source_depth: depth,
        width: decoded.width,
        height: decoded.height,
        source_sha256: header.sha256,
        output_sha256: Sha256::digest(&rgba).into(),
        changed_rgb_components,
        changed_alpha,
    };
    report.validate_payload(header, decoded.width, decoded.height, &rgba)?;
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    let mut output = response_header(
        header,
        0,
        decoded.width,
        decoded.height,
        NativeEncoding {
            primaries: 1,
            transfer: 13,
            matrix: 0,
            full_range: true,
        },
        rgba.len(),
        0,
        REPORT_BYTES,
        version.len(),
        peak(),
    )?;
    output
        .try_reserve_exact(version.len() + REPORT_BYTES + rgba.len())
        .map_err(|_| resource("Integer JXL response allocation"))?;
    output.extend_from_slice(version);
    output.extend_from_slice(&report.encode()?);
    output.extend_from_slice(&rgba);
    if output.len() > OUTPUT_BYTES_MAX {
        return Err(resource("Integer JXL response allowance"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
