# HDR/EXR dependency notice inventory

`HDR_EXR_LICENSES_V1.txt` contains unmodified upstream license/copyright texts. Its companion `HDR_EXR_LICENSES_V1.provenance.json` records the exact locked versions, crate archive checksums, published source commits, source URLs, selected license options, raw text hashes and byte offsets within the aggregate.

The inventory compares normal dependency graphs for explicit `--no-default-features --features avif-sdr-codec` and `--no-default-features --features float-codecs`, using locked offline Cargo trees for all six native release targets. Build/dev dependencies are excluded. The ARM64 delta contains nine packages; x86_64 additionally introduces `raw-cpuid` 11.6.0. Reused `image` 0.25.10 (HDR decoder) and `half` 2.7.1 attributions are included separately. The presence of a dependency in the graph does not prove its code survives native linking; `pulp-wasm-simd-flag` is included conservatively although its implementation is Wasm-only.

Every cached license and published `.cargo_vcs_info.json` was compared with its crate archive after verifying that archive's SHA-256 against `Cargo.lock`. For MIT-or-Apache components, the aggregate selects the exact upstream MIT text. The complete `libm` license file is retained, including its additional third-party notices. The complete `exr` license file retains both OpenEXR and exrs notices.

Two published crates omit standalone license files. `pulp-wasm-simd-flag` uses the repository-root MIT license fetched at its published commit `5eb07fd7b68edf0a5e19f71737d315f72a510295`; its bytes match the license in the `pulp` archive from that same commit. `zune-inflate` explicitly offers `MIT OR Apache-2.0 OR Zlib`; this inventory selects the exact `LICENSE-ZLIB` fetched from its published commit `69502ce83fdfecdd0beefd677e2abb3781b29d98`, and retains the root copyright notice and the crate's own source copyright/license header. No generic license text was substituted for either missing file.

This inventory does not change runtime qualification or package payloads. It is intended for the final qualified V7 contract only. Existing JPEGXR/dav1d notices are unchanged. Future wiring must derive required notices from the exact verified helper profile: legacy V4/V5/V6 archives retain their historical payload sets, while final V7 requires the complete aggregate. Old private V7 manifests remain unsupported in production. Archive extraction must remain bounded and accept only the exact legacy or final file set; upgrade/rollback tests must cover transitions in both directions without permitting arbitrary extra files.

Run this lightweight offline integrity check from the repository root:

```sh
python3 - <<'PY'
import hashlib, json, pathlib, tomllib
base = pathlib.Path('packaging')
record = json.loads((base / 'HDR_EXR_LICENSES_V1.provenance.json').read_text())
data = (base / record['aggregate']['file']).read_bytes()
sha = lambda value: hashlib.sha256(value).hexdigest()
assert sha(data) == record['aggregate']['sha256']
assert len(data) == record['aggregate']['byte_length']
lock_bytes = pathlib.Path('Cargo.lock').read_bytes()
assert sha(lock_bytes) == record['cargo_lock_sha256']
locked = {(p['name'], p['version']): p.get('checksum')
          for p in tomllib.loads(lock_bytes.decode())['package']}
for package in record['packages']:
    assert locked[package['name'], package['version']] == package['cargo_lock_checksum']
    for text in package['texts']:
        start = text['aggregate_byte_offset']
        assert sha(data[start:start + text['byte_length']]) == text['sha256']
print('Verified aggregate, lock snapshot, package checksums and all 14 exact texts')
PY
```

Refresh this versioned inventory deliberately when its selected dependencies or license sources change. Preserve exact source bytes and update provenance together; the aggregate hash binds their current layout. The provenance describes source/notice verification, not actual execution on all six platforms.
