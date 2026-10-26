# Private dav1d build input

Upstream: https://github.com/videolan/dav1d/tree/1.5.4
Archive: https://codeload.github.com/videolan/dav1d/tar.gz/54706fc6bc0cdecab7e9593974a4039cc038fca7
SHA-256: `9edb11a2108b375cc58370354e705feebc93430bb780130363815c1e1ac0c250`
License: BSD-2-Clause, original notice in COPYING and inside the archive.

The optional native worker qualification feature extracts this verified source
only under Cargo OUT_DIR. It has no network or system-library fallback. The build
replaces the default allocation branch of src/mem.h with executable-local charged
allocation functions; upstream tracking allocation mode stays disabled. C-only
8-bit decoding is selected, assembly/tools/tests disabled. Decoder functionality,
resource containment and release-target packaging remain separately qualified.

Immutable upstream release commit: `54706fc6bc0cdecab7e9593974a4039cc038fca7`.
Pool allocation tags remain stored with tracking disabled so charged requests
can be classified without enabling upstream allocation-statistics code.
