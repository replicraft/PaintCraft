//! Strict single-raster P1..P7 admission before the image PNM decoder allocates.
use super::*;
const HEADER_MAX: usize = 64 << 10;

struct Tokens<'a> {
    bytes: &'a [u8],
    at: usize,
    comments: usize,
}
impl<'a> Tokens<'a> {
    fn skip(&mut self) -> Result<()> {
        loop {
            while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
                self.at += 1;
            }
            if self.bytes.get(self.at) != Some(&b'#') {
                return Ok(());
            }
            let start = self.at;
            while self.bytes.get(self.at).is_some_and(|v| *v != b'\n') {
                self.at += 1;
            }
            self.comments += self.at - start;
            if self.comments > HEADER_MAX {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "PNM comments exceed metadata budget",
                ));
            }
        }
    }
    fn number(&mut self) -> Result<u32> {
        self.skip()?;
        let start = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        let value = self
            .bytes
            .get(start..self.at)
            .ok_or_else(|| codec("invalid PNM token"))?;
        if value.is_empty() || value.len() > 10 {
            return Err(codec("invalid PNM integer"));
        }
        if self
            .bytes
            .get(self.at)
            .is_some_and(|v| !v.is_ascii_whitespace() && *v != b'#')
        {
            return Err(codec("invalid PNM integer delimiter"));
        }
        value.iter().try_fold(0u32, |n, d| {
            n.checked_mul(10)
                .and_then(|n| n.checked_add(u32::from(d - b'0')))
                .ok_or_else(|| codec("PNM integer overflows"))
        })
    }
    fn binary_start(&mut self) -> Result<usize> {
        let separator = *self
            .bytes
            .get(self.at)
            .ok_or_else(|| codec("missing PNM raster separator"))?;
        if !separator.is_ascii_whitespace() {
            return Err(codec("invalid PNM raster separator"));
        }
        self.at += 1;
        if separator == b'\r' && self.bytes.get(self.at) == Some(&b'\n') {
            self.at += 1;
        }
        Ok(self.at)
    }
}
fn losses(comments: bool) -> Vec<String> {
    if comments {
        vec!["pnm-metadata-ignored".into()]
    } else {
        Vec::new()
    }
}
fn maxval(value: u32) -> Result<()> {
    if !matches!(value, 1 | 255 | 65535) {
        return Err(unsupported(
            "PNM maxval profile supports 1, 255 and 65535 only",
        ));
    }
    Ok(())
}
fn binary(bytes: &[u8], offset: usize, samples: u64, maximum: u32) -> Result<()> {
    let stride = if maximum > 255 { 2 } else { 1 };
    if offset as u64 + samples * stride != bytes.len() as u64 {
        return Err(codec("PNM requires exactly one complete raster"));
    }
    if maximum == 1 && bytes[offset..].iter().any(|sample| *sample > 1) {
        return Err(codec("PNM sample exceeds maxval"));
    }
    Ok(())
}
pub(crate) fn admit(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    if bytes.len() < 3
        || bytes[0] != b'P'
        || !matches!(bytes[1], b'1'..=b'7')
        || !bytes[2].is_ascii_whitespace()
    {
        return Err(codec("invalid PNM signature"));
    }
    if bytes[1] == b'7' {
        return pam(bytes, pixels);
    }
    let mut tokens = Tokens {
        bytes,
        at: 2,
        comments: 0,
    };
    let width = tokens.number()?;
    let height = tokens.number()?;
    let count = admit_dimensions(width, height, pixels)?;
    let bitmap = matches!(bytes[1], b'1' | b'4');
    let maximum = if bitmap { 1 } else { tokens.number()? };
    maxval(maximum)?;
    if tokens.at > HEADER_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PNM header exceeds metadata budget",
        ));
    }
    let samples = count
        * if matches!(bytes[1], b'3' | b'6') {
            3
        } else {
            1
        };
    if bytes[1] <= b'3' {
        for _ in 0..samples {
            if tokens.number()? > maximum {
                return Err(codec("PNM sample exceeds maxval"));
            }
        }
        tokens.skip()?;
        if tokens.at != bytes.len() {
            return Err(codec(
                "multiple PNM rasters or trailing data are not qualified",
            ));
        }
    } else {
        let at = tokens.binary_start()?;
        if bytes[1] == b'4' {
            if at as u64 + u64::from(width.div_ceil(8)) * u64::from(height) != bytes.len() as u64 {
                return Err(codec("PBM requires exactly one complete raster"));
            }
        } else {
            binary(bytes, at, samples, maximum)?;
        }
    }
    Ok(losses(tokens.comments > 0))
}
fn pam(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    if bytes.get(..3) != Some(b"P7\n") {
        return Err(unsupported("PAM requires newline after signature"));
    }
    let mut at = 3;
    let mut values = [None; 4];
    let mut tuple = None;
    let mut comments = false;
    loop {
        let rest = bytes
            .get(at..)
            .ok_or_else(|| codec("truncated PAM header"))?;
        let length = rest
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| codec("truncated PAM header line"))?;
        at = at
            .checked_add(length + 1)
            .ok_or_else(|| codec("PAM header overflow"))?;
        if at > HEADER_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "PAM header exceeds metadata budget",
            ));
        }
        let line = std::str::from_utf8(&rest[..length])
            .map_err(|_| codec("PAM header must be ASCII"))?
            .trim();
        if !line.is_ascii() {
            return Err(codec("PAM header must be ASCII"));
        }
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            comments = true;
            continue;
        }
        if line == "ENDHDR" {
            break;
        }
        let mut parts = line.split_ascii_whitespace();
        let key = parts.next().ok_or_else(|| codec("invalid PAM field"))?;
        let value = parts.next().ok_or_else(|| codec("missing PAM value"))?;
        if parts.next().is_some() {
            return Err(unsupported("PAM field extensions are not qualified"));
        }
        if key == "TUPLTYPE" {
            if tuple.replace(value).is_some() {
                return Err(codec("duplicate PAM tuple type"));
            }
        } else {
            let field = match key {
                "WIDTH" => 0,
                "HEIGHT" => 1,
                "DEPTH" => 2,
                "MAXVAL" => 3,
                _ => return Err(unsupported("unknown PAM header field")),
            };
            if value.len() > 10 || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(codec("invalid PAM number"));
            }
            let parsed = value
                .parse::<u32>()
                .map_err(|_| codec("PAM integer overflows"))?;
            if values[field].replace(parsed).is_some() {
                return Err(codec("duplicate PAM field"));
            }
        }
    }
    let [Some(width), Some(height), Some(depth), Some(maximum)] = values else {
        return Err(codec("missing required PAM field"));
    };
    let count = admit_dimensions(width, height, pixels)?;
    maxval(maximum)?;
    match (tuple, depth, maximum) {
        (Some("BLACKANDWHITE"), 1, 1)
        | (Some("BLACKANDWHITE_ALPHA"), 2, 1)
        | (Some("GRAYSCALE"), 1, _)
        | (Some("GRAYSCALE_ALPHA"), 2, _)
        | (Some("RGB"), 3, _)
        | (Some("RGB_ALPHA"), 4, _) => {}
        _ => return Err(unsupported("PAM tuple/depth profile is not qualified")),
    }
    binary(bytes, at, count * u64::from(depth), maximum)?;
    Ok(losses(comments))
}
