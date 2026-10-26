# JPEG XR selected source, PaintCraft revision 1

This is the exact published `jpegxr-pure-rs` 0.1.0 source plus a visible reviewed API patch. It remains an experimental third-party codec. The dependency is used by the existing isolated native worker and trusted developer tests; vendoring does not enable a JPEG XR writer or broaden existing format gates.

## Source and local delta

- Upstream repository: https://github.com/henriksson-lab/jpegxr-pure-rs
- Immutable commit: `cd626cd555e944c6cdcf273fac52434ca1fedd61`.
- Original crate: `https://static.crates.io/crates/jpegxr-pure-rs/jpegxr-pure-rs-0.1.0.crate`.
- Original archive SHA256: `be1bfea78bcd73aa67dfce7cdd22902e4e6ac06b387a5dea4f225f52525e8590` (193,741 bytes).
- Original inventory: 50 regular files, 1,485,413 expanded bytes; all files are retained, including examples and original Cargo metadata/version.
- Reviewed delta: `jpegxr-pure-rs-0.1.0-paintcraft1.patch`, SHA256 `274411f407f4afe64ab4af6ebc70f23540d9702115911e53e768087b38d49cb6`.
- Full before/after file inventory and license hashes: `jpegxr-pure-rs-0.1.0-paintcraft1.provenance.json`.

Only `src/api.rs` and `src/image/sys/strcodec.rs` differ from that archive. The patch adds a checked RGB8 PixelFormat factory and caller-owned capped seekable output, with sticky cancellation/IO errors and a documented internal synchronous lifetime. Width/height now reject values above i32::MAX before encoder casts or sink access; the existing stride-u32 guard remains. Decoder code and the existing path/convenience APIs are otherwise unchanged. Product code must continue avoiding predictable-temp convenience APIs.

The safe output API borrows an empty position-zero Write+Seek sink. It opens no path. Every stream seek/write checks maximum extent before forwarding, and errors remain sticky through encoder/stream destruction and final cancellation/flush. Cancellation occurs at stream boundaries, not arbitrary CPU-loop preemption. Failed output may contain private bounded partial bytes; callers own cleanup/publication/durability. Caller sink/check panics are caught only under an unwind build. No arbitrary codec memory-safety, process sandbox, hard physical-memory or peak CPU guarantee is implied.

## Source/license preservation

`LICENSE.md` is retained verbatim, including the Microsoft copyright, source/binary redistribution conditions and disclaimer. Its actual notice is also the existing `packaging/JPEGXR_LICENSE.txt`. Upstream Cargo metadata says BSD-3-Clause; this source copy does not substitute that label for the distributed notice or relicense upstream code. PaintCraft patch authorship remains separate from upstream provenance.

The root workspace explicitly excludes this versioned third-party package and points its shared dependency at it with original version and default features disabled. There is no Cargo-cache mutation, build-time patching, mutable git dependency, automatic network source fetch, system codec fallback or published fork. Future upstream changes require an explicit new inventory/diff review and qualification.

## Verification and source ownership

```sh
python3 scripts/verify_jpegxr_vendor.py
python3 -m unittest discover -s scripts/tests -p test_verify_jpegxr_vendor.py
python3 scripts/check_file_lengths.py
```

For an independently acquired original crate, additionally pass `--archive PATH` to the verifier. It verifies the pinned archive hash, every original file, every final file and the exact complete archive-to-vendor unified diff. No source extraction or modification occurs during verification.

Default verification rejects missing/extra/changed files, symlinks, unexpected patch/exception identities and a mismatched packaged notice. `scripts/check_file_lengths.py` invokes that verification before allowing exactly twelve named inherited source files above 800 lines. There is no blanket vendor-directory exception; unrecognized or changed source fails closed. `src/api.rs`, all first-party scripts and new adapters remain under 800 lines. The complete first-party API patch also has fewer than 800 added lines; inherited functions are not mechanically split or reformatted to obscure their source.

`vendor/.gitattributes` disables newline conversion only for this exact upstream version and exact patch/provenance artifacts. The exact unified-diff artifact disables whitespace checking because blank context records require their single-space prefix; the exact upstream README also retains its original blank line at EOF. No Rust source needs a whitespace exemption. Application whitespace/line rules remain unchanged. Byte verification is intended to fail if checkout filtering changes upstream content.

## Qualification boundary

Two preceding private experiments qualified 14 authored RGB8 vectors against untouched original C JXRLib, preserving full container/header and exact RGB, plus capped-sink/cancellation/IO/panic controls and unchanged decoder fixtures. The actual vendor-path qualification separately repeats those vectors and adds dimension/stride cast-boundary controls; raw receipts identify the exact source revision. Those local controlled results do not establish maximum-image workspace bounds, alpha/quality/depth modes, Windows consumer parity or six-target execution.

The original crate's ordinary unit-test command currently cannot resolve its benchmark-only `original_jpegxr` development dependency from the offline local index. That tooling failure is retained rather than counted as a pass. Existing product feature checks, trusted decoder regressions and the separate vendor-path qualification do not need that development dependency. A public fresh writer still requires its own typed helper operation, resource admission, real staged-copy consumer tests and exact registry qualification.
