use super::*;
use std::cell::Cell;
const PREFERRED: &[(u32, Encoding)] = &[
    (100, Encoding::Png),
    (101, Encoding::Png),
    (17, Encoding::DibV5),
    (8, Encoding::Dib),
    (6, Encoding::Tiff),
];
struct Board {
    formats: Vec<u32>,
    data: Vec<u8>,
    length: usize,
    generation: Cell<u32>,
    copied: usize,
    changed: bool,
    resized: bool,
    delayed: bool,
    error: bool,
    budget: SharedBudget,
}
impl Board {
    fn new() -> Self {
        let data = b"\x89PNG\r\n\x1a\ntest".to_vec();
        Self {
            formats: vec![100, 6],
            length: data.len(),
            data,
            generation: Cell::new(10),
            copied: 0,
            changed: false,
            resized: false,
            delayed: false,
            error: false,
            budget: SharedBudget::new(1024),
        }
    }
    fn capture(&mut self) -> Result<EncodedImage, String> {
        let budget = self.budget.clone();
        capture(self, PREFERRED, &budget, &|| Ok(()))
    }
}
impl Snapshot for Board {
    fn formats(&mut self) -> Result<Vec<u32>, String> {
        Ok(self.formats.clone())
    }
    fn generation(&self) -> Result<u32, String> {
        Ok(self.generation.get())
    }
    fn length(&mut self, _: u32) -> Result<usize, String> {
        if self.delayed {
            self.generation.set(11);
            self.delayed = false;
        }
        Ok(self.length)
    }
    fn copy(&mut self, format: u32, bytes: &mut [u8]) -> Result<usize, String> {
        assert_eq!(self.budget.usage().reserved, self.length as u64);
        self.copied += 1;
        assert!(self.formats.contains(&format));
        if self.error {
            return Err("native copy failed".into());
        }
        let count = bytes.len().min(self.data.len());
        bytes[..count].copy_from_slice(&self.data[..count]);
        if self.changed {
            self.generation.set(12);
        }
        if self.resized {
            self.length += 1;
        }
        Ok(count)
    }
}
#[test]
fn preferred_png_and_tiff_remain_frozen_and_share_one_capacity_lease() {
    for formats in [vec![100, 6], vec![101], vec![6]] {
        let mut board = Board::new();
        board.formats = formats;
        if board.formats == [6] {
            board.data = b"II*\0tiff".to_vec();
            board.length = board.data.len();
        }
        let expected = board.data.clone();
        let image = board.capture().unwrap();
        assert_eq!(board.copied, 1);
        assert_eq!(image.bytes(), expected);
        assert_eq!(
            image.encoding(),
            if board.formats == [6] {
                Encoding::Tiff
            } else {
                Encoding::Png
            }
        );
        board.data.fill(0);
        let clone = image.clone();
        drop(image);
        assert_eq!(clone.bytes(), expected);
        assert!(board.budget.usage().reserved > 0);
        drop(clone);
        assert_eq!(board.budget.usage().reserved, 0);
    }
}
#[test]
fn changed_or_short_sources_native_errors_and_invalid_preferred_signatures_fail_without_fallback() {
    for mode in 0..5 {
        let mut board = Board::new();
        match mode {
            0 => board.changed = true,
            1 => board.resized = true,
            2 => board.data.truncate(8),
            3 => board.error = true,
            _ => board.data[0] = 0,
        }
        assert!(board.capture().is_err());
        assert_eq!(board.copied, 1);
        assert_eq!(board.budget.usage().reserved, 0);
    }
}
#[test]
fn admission_type_count_and_encoded_size_fail_before_copying() {
    for mode in 0..5 {
        let mut board = Board::new();
        match mode {
            0 => board.budget = SharedBudget::new(1),
            1 => board.length = crate::clipboard::MAX_ENCODED_BYTES as usize + 1,
            2 => board.length = 0,
            3 => board.formats = (0..257).collect(),
            _ => board.formats = vec![99],
        }
        assert!(board.capture().is_err());
        assert_eq!(board.copied, 0);
        assert_eq!(board.budget.usage().reserved, 0);
    }
}
#[test]
fn delayed_realization_is_pinned_after_size_query_and_cancellation_never_exposes_bytes() {
    let mut board = Board::new();
    board.delayed = true;
    assert!(board.capture().is_ok());
    for at in 0..6 {
        let mut board = Board::new();
        let budget = board.budget.clone();
        let calls = Cell::new(0);
        let result = capture(&mut board, PREFERRED, &budget, &|| {
            let count = calls.get();
            calls.set(count + 1);
            if count == at {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn declared_dibv5_precedes_dib_and_tiff_without_decoding_or_reframing() {
    for (formats, kind, size) in [
        (vec![17, 8, 6], Encoding::DibV5, 124u32),
        (vec![8, 6], Encoding::Dib, 40u32),
    ] {
        let mut board = Board::new();
        board.formats = formats;
        board.data = vec![0; size as usize];
        board.data[..4].copy_from_slice(&size.to_le_bytes());
        board.length = board.data.len();
        let expected = board.data.clone();
        let image = board.capture().unwrap();
        assert_eq!(image.encoding(), kind);
        assert_eq!(image.bytes(), expected);
        assert_eq!(board.copied, 1);
    }
    let mut board = Board::new();
    board.formats = vec![17, 8];
    board.data = vec![0; 40];
    board.data[..4].copy_from_slice(&40u32.to_le_bytes());
    board.length = 40;
    assert!(board.capture().is_err());
    assert_eq!(board.copied, 1);
    assert_eq!(board.budget.usage().reserved, 0);
}
