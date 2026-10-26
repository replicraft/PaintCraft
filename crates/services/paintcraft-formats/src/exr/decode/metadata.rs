//! Audit of every AttributeValue::read branch in pinned exr 1.74.2. These
//! checks apply to types at arbitrary names, not just conventional attributes.
use super::*;
pub(super) fn validate(header: &Header<'_>) -> Result<()> {
    for attribute in &header.attributes {
        let data = attribute.value;
        let fixed = match attribute.kind {
            "box2i" | "box2f" => Some(16),
            "int" | "float" => Some(4),
            "double" | "rational" | "timecode" | "v2i" | "v2f" => Some(8),
            "v3i" | "v3f" => Some(12),
            "chromaticities" => Some(32),
            "compression" | "envmap" | "lineOrder" => Some(1),
            "keycode" => Some(28),
            "m33f" => Some(36),
            "m44f" => Some(64),
            "tiledesc" => Some(9),
            _ => None,
        };
        if fixed.is_some_and(|len| data.len() != len) {
            return Err(invalid("EXR typed metadata fixed extent mismatch"));
        }
        match attribute.kind {
            "box2i" => {
                let values: [i32; 4] = std::array::from_fn(|i| {
                    i32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap())
                });
                for axis in 0..2 {
                    let min = values[axis].min(values[axis + 2]);
                    let max = values[axis].max(values[axis + 2]);
                    // Backend uses max+1-min in i32 after its own validation.
                    if max == i32::MAX || i64::from(max) - i64::from(min) + 1 > i64::from(i32::MAX)
                    {
                        return Err(unsupported(
                            "EXR integer-box metadata exceeds selected decoder arithmetic profile",
                        ));
                    }
                }
            }
            "preview" => {
                if data.len() < 8 {
                    return Err(invalid("EXR preview extent truncated"));
                }
                let width = u64::from(u32::from_le_bytes(data[..4].try_into().unwrap()));
                let height = u64::from(u32::from_le_bytes(data[4..8].try_into().unwrap()));
                if width.checked_mul(height).and_then(|n| n.checked_mul(4))
                    != Some((data.len() - 8) as u64)
                {
                    return Err(invalid(
                        "EXR preview nested geometry disagrees with bounded attribute",
                    ));
                }
            }
            "stringvector" => {
                let mut bytes = Bytes { bytes: data, at: 0 };
                while bytes.at < data.len() {
                    let count = bytes.i32()?;
                    if count < 0 {
                        return Err(invalid("EXR stringvector negative nested extent"));
                    }
                    bytes.take(count as usize)?;
                }
            }
            "bytes" => {
                let mut bytes = Bytes { bytes: data, at: 0 };
                let count = bytes.u32()?;
                bytes.take(count as usize)?;
            }
            "chlist" if attribute.name != "channels" => {
                return Err(unsupported(
                    "EXR additional typed channel metadata unqualified",
                ));
            }
            // String/custom values only copy the already bounded outer bytes;
            // all other recognized branches have fixed-size reads above.
            _ => {}
        }
    }
    Ok(())
}
