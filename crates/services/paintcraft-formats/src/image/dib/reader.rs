//! A stack-sized BMP file header over borrowed packed-DIB bytes.
use std::io::{self, BufRead, Read, Seek, SeekFrom};

pub(super) struct Prefixed<'a> {
    header: [u8; 14],
    body: &'a [u8],
    position: u64,
}
impl<'a> Prefixed<'a> {
    pub fn new(body: &'a [u8], offset: u32) -> Self {
        let mut header = [0; 14];
        header[..2].copy_from_slice(b"BM");
        header[2..6].copy_from_slice(&((body.len() + 14) as u32).to_le_bytes());
        header[10..14].copy_from_slice(&(offset + 14).to_le_bytes());
        Self {
            header,
            body,
            position: 0,
        }
    }
}
impl BufRead for Prefixed<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.position < 14 {
            return Ok(&self.header[self.position as usize..]);
        }
        Ok(usize::try_from(self.position - 14)
            .ok()
            .and_then(|at| self.body.get(at..))
            .unwrap_or_default())
    }
    fn consume(&mut self, amount: usize) {
        self.position = self.position.saturating_add(amount as u64);
    }
}
impl Read for Prefixed<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let source = self.fill_buf()?;
        let count = source.len().min(out.len());
        out[..count].copy_from_slice(&source[..count]);
        self.consume(count);
        Ok(count)
    }
}
impl Seek for Prefixed<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let position = match position {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => (self.body.len() as u64 + 14).checked_add_signed(delta),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid DIB seek"))?;
        self.position = position;
        Ok(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_and_seeks_across_borrowed_header_boundary_and_checked_edges() {
        let body = b"borrowed bytes";
        let mut reader = Prefixed::new(body, 4);
        let mut expected = vec![];
        reader.read_to_end(&mut expected).unwrap();
        assert_eq!(&expected[14..], body);
        assert_eq!(&expected[..2], b"BM");
        assert_eq!(u32::from_le_bytes(expected[10..14].try_into().unwrap()), 18);
        for at in 0..expected.len() + 2 {
            reader.seek(SeekFrom::Start(at as u64)).unwrap();
            let mut out = vec![];
            reader.read_to_end(&mut out).unwrap();
            assert_eq!(out, expected.get(at..).unwrap_or_default());
        }
        reader.seek(SeekFrom::Start(14)).unwrap();
        assert!(std::ptr::eq(
            reader.fill_buf().unwrap().as_ptr(),
            body.as_ptr()
        ));
        reader.seek(SeekFrom::End(-3)).unwrap();
        let mut end = [0; 3];
        reader.read_exact(&mut end).unwrap();
        assert_eq!(&end, b"tes");
        assert!(reader.seek(SeekFrom::End(i64::MIN)).is_err());
        reader.seek(SeekFrom::Start(u64::MAX)).unwrap();
        assert!(reader.seek(SeekFrom::Current(1)).is_err());
        assert_eq!(reader.read(&mut end).unwrap(), 0);
    }
}
