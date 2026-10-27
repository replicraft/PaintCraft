# Owned portable JPEG XR export goldens

These four opaque images are authored for PaintCraft tests. For pixel `(x,y)`,
RGBA is `[(47*x+13*y)%256, (47*x+13*y+71)%256,
(47*x+13*y+142)%256, 255]`. The `.rgba` files retain those exact bytes.
Geometries are 4×3, 17×19, 4096×1 and 1×4096; the latter two exercise long
headers. These are fixed lossless RGB8 fresh copies, not general JPEG XR vectors.

This is a **new curation**, performed on 2026-10-26, rather than a relabeling of
previous consumer results. The real installed CLI exported each newly built PCND
project through the immutable public V8 helper. The pinned original C decoder
then reopened each complete JXR, exited successfully, and matched every expected
RGB byte. `provenance.json` records raw RGBA, encoded file and C output hashes,
actual CLI receipts and executable hashes:

- CLI (debug): `237961d02809293dda1d8c723811f21ac718b31fd7f6af917d34d962ad33abf1`
- Helper (optimized): `529ed471936258cba3ce1e72444c98bc5f9f0c2848b808552ca213dc3f2c716c`
- C oracle: `8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce`

The recorded `source_head` is the curation harness checkout, not a claim that the
previously built CLI/helper came from that HEAD. The helper's independent producer
receipt identifies its archived source at `35e1480798cf3bbd6713e37e5bf5b8aa67f01ae6`.
The executable hashes above identify the actual curation tools. On Apple Silicon,
fresh curation passed in 3.89s; the initial portable positive passed in 10.12s.
After an equivalent iterator change required by Clippy, the final portable test
passed in 9.84s; scoped formatting and strict Clippy passed.
Raw logs and C output files remain in `/private/tmp/paintcraft-jxr-portable-curation*`.
Those timings describe test runs and are not benchmarks.

The ignored installed test
`jxr_export::portable::installed_cli_mcp_match_c_qualified_goldens` requires only
`PAINTCRAFT_JXR_INSTALLATION`, pointing to a complete public V8 CLI/helper bin
installation. It validates fixture hashes and authored input, compares all four
CLI and MCP exports to the frozen files, imports through each actual consumer,
and independently compares reopened PCND pixels to `.rgba`. It also checks
missing-loss consent, source-hash refusal, dirty-session preservation, renamed
project hardlink refusal and imported-source alias refusal after PCND Save.
It **does not execute C on that host**: it proves exact byte identity to files
that C checked during curation. CI must additionally bind the installed CLI to
the release test artifact; the release qualification selector owns that check.
No six-platform or Windows WIC execution is inferred from this local result.

For a deliberate new curation, execute the separately ignored
`jxr_export::portable::curate_c_qualified_goldens` with the installation variable,
`PAINTCRAFT_JXR_REFERENCE` pointing to the exact C oracle, and
`PAINTCRAFT_JXR_GOLDEN_OUTPUT` naming a fresh nonexistent directory. It refuses to
overwrite an existing directory. Review the new C results and executable/source
identities before replacing any checked-in files. The curation helper is not a
portable CI case and must not be included in the no-C release selector.
