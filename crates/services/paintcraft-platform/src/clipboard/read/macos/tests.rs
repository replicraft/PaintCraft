use super::*;
use objc2::rc::Retained;
use objc2_foundation::NSData;
use std::cell::Cell;
struct PrivateBoard(Retained<NSPasteboard>);
impl Drop for PrivateBoard {
    fn drop(&mut self) {
        self.0.clearContents();
    }
}
fn put(board: &NSPasteboard, kind: Encoding, bytes: &[u8]) {
    let mime = NSString::from_str(match kind {
        Encoding::Png => "public.png",
        Encoding::Tiff => "public.tiff",
        Encoding::Dib | Encoding::DibV5 => panic!("DIB is not a macOS fixture flavor"),
    });
    assert!(
        board.setData_forType(Some(&NSData::with_bytes(bytes)), &mime),
        "private board write rejected"
    );
}
#[test]
#[ignore = "explicit isolated native pasteboard test; never accesses the general clipboard"]
fn isolated_native_pasteboard_reads_frozen_bytes_and_rejects_changed_ownership() {
    autoreleasepool(|_| {
        let board = PrivateBoard(NSPasteboard::pasteboardWithUniqueName());
        let budget = SharedBudget::new(4096);
        let png = b"\x89PNG\r\n\x1a\nraw encoded transport";
        let tiff = b"II*\0raw encoded transport";
        board.0.clearContents();
        put(&board.0, Encoding::Tiff, tiff);
        put(&board.0, Encoding::Png, png);
        let image = capture_board(&board.0, &budget, &|| Ok(())).unwrap();
        assert_eq!(image.encoding(), Encoding::Png);
        assert_eq!(image.bytes(), png);
        drop(image);
        board.0.clearContents();
        put(&board.0, Encoding::Tiff, tiff);
        let image = capture_board(&board.0, &budget, &|| Ok(())).unwrap();
        assert_eq!(image.encoding(), Encoding::Tiff);
        assert_eq!(image.bytes(), tiff);
        drop(image);
        board.0.clearContents();
        put(&board.0, Encoding::Tiff, tiff);
        put(&board.0, Encoding::Png, b"not a PNG");
        assert!(
            capture_board(&board.0, &budget, &|| Ok(()))
                .unwrap_err()
                .contains("signature")
        );
        assert_eq!(budget.usage().reserved, 0);
        board.0.clearContents();
        put(&board.0, Encoding::Png, png);
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 4 {
                board.0.clearContents();
            }
            Ok(())
        };
        assert!(
            capture_board(&board.0, &budget, &check)
                .unwrap_err()
                .contains("changed")
        );
        assert_eq!(budget.usage().reserved, 0);
        board.0.clearContents();
        put(&board.0, Encoding::Png, png);
        let quota = SharedBudget::new(1);
        assert!(capture_board(&board.0, &quota, &|| Ok(())).is_err());
        assert_eq!(quota.usage().peak, 0);
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        };
        assert!(
            capture_board(&board.0, &budget, &check)
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(budget.usage().reserved, 0);
        board.0.clearContents();
        for index in 0..257 {
            let kind = NSString::from_str(&format!("com.paintcraft.private-test-{index}"));
            assert!(
                board
                    .0
                    .setData_forType(Some(&NSData::with_bytes(b"x")), &kind)
            );
        }
        assert!(
            capture_board(&board.0, &budget, &|| Ok(()))
                .unwrap_err()
                .contains("type count")
        );
        assert_eq!(budget.usage().reserved, 0);
        board.0.clearContents();
        assert!(capture_board(&board.0, &budget, &|| Ok(())).is_err());
    });
}
