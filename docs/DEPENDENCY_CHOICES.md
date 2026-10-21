# PaintCraft dependency choices

The selected directions come from [TECH_STACK](temp/paintcraft-implementation-spec-v5-hyperfine/TECH_STACK.md)
and its dependency registry. This file records application adoption; the reference pack
remains a design source rather than evidence that a library is already used.

The user-approved crate naming supersedes the pack: shared foundations are
`paintcraft_core::types`; sessions and transactions belong to `paintcraft-editor`.
Dependencies are declared at the workspace root and enabled only by their actual consumers.
A correct integrated baseline is not a measured performance or release qualification.

## Selected library directions

| Library | Direction | Actual or intended consumer | Benchmark | Application state |
|---|---|---|---|---|
| `glam` | selected-on-use | Typed viewport and affine transforms in core/UI | BF01 | Integrated baseline, 0.34.1 with std/f64; inverse, composition, rebase and actual-pixel tests; benchmark pending |
| `wide` | preferred-on-use | Exact pixel kernels in raster/render | BF02 | Not directly adopted; activate with its consumer |
| `zerocopy` | selective-on-use | Bounded fixed-layout BMP header admission | BF03 | Integrated baseline, 0.8.63 with derive; header size/offset and malformed-input tests; benchmark pending |
| `bytemuck` | selected-on-use | Explicit GPU staging layouts in render | BF04 | Integrated baseline, 1.25.2 with derive; WGSL offsets/endian checks and Metal parity pass; benchmark pending |
| `tess2-rust` | evaluate | Complex path fills; compare Lyon | BF05 | Not directly adopted; activate with its consumer |
| `rayon` | selected-on-use | Shared native editing preparation and explicit CLI CPU export pools | BF06 | 1.12.0; disjoint bands/tile views and 1/2/4-worker bulk comparisons exist; queue/byte admission, targeted completion retention, cancellation, abandoned consumers, nested execution and panic tests pass. Native save/export barriers and process-shared declared job-byte admission have application tests; running/ready guards survive executor drop. Interactive fairness and physical process-wide budget qualification remain |
| `tokio` | preferred-on-use | Native save/recovery lifecycle | BF07 | Integrated baseline, 1.53.3 with rt/sync behind native-io; bounded FIFO, completion retention, panic and shutdown tests; recovery publication fences and native race tests pass; startup, restore and claimed-source handoff integrated; benchmark pending |
| `moka` | selective-on-use | Discardable derived CPU cache with independent memory accounting | BF08 | Not adopted; the current bounded compositor cache uses a small Vec/LRU table. A Moka comparison and accounting/lease evaluation remain pending |
| `fst` | deferred | Large immutable asset catalog, when one exists | BF09 | Not directly adopted; activate with its consumer |
| `smallvec` | selective-on-use | Short dirty-region and dependency lists | BF10 | Not directly adopted; activate with its consumer |
| `bitvec` | selective-on-use | Finite binary selection/dirty coverage | BF11 | Not directly adopted; activate with its consumer |
| `arrayvec` | selective-on-use | Scratch collections with a proven bound | BF12 | Not directly adopted; activate with its consumer |
| `dashmap` | evaluate | Narrow concurrent immutable-resource registry, if justified | BF13 | Not directly adopted; activate with its consumer |
| `slotmap` | selected-on-use | Runtime edit/render job handles with a separate owner UUID | BF14 | Integrated baseline, 1.1.1; cross-owner/stale-handle and slot-reuse tests; benchmark pending |
| `indexmap` | selective-on-use | Ordered presets, effect parameters and command metadata | BF15 | Not directly adopted; activate with its consumer |

## Other product and runtime selections

| Area | Choice | Application state |
|---|---|---|
| UI | egui | Integrated through eframe; Classic ribbon/vector widgets and separate Studio toolbar/floating windows exist; reference fidelity and further Studio features remain pending |
| UI font | Selawik | Unmodified Microsoft 1.01 Regular and Bold embedded with OFL 1.1 notice; document text uses these plus installed fonts |
| Retro UI font | [W95FA by MadeByArne](https://www.dafont.com/w95fa.font) | User-selected for RetroPaint; author listing identifies SIL Open Font License and commercial use. Not bundled yet; preserve the font archive's license and provenance when adopted |
| SVG UI assets | resvg 0.48.1, default features disabled | Shared UI renders the user-supplied generated icon sheet as 68 bundled SVG adaptations; per-context density cache and all-icon render tests; SVG document import remains deferred |
| Native shell | Compare eframe with direct egui-winit/egui-wgpu | eframe remains provisional pending T02 hardware/IME traces |
| GPU | wgpu + WGSL | Integer compositor and device epochs integrated; Metal parity, explicit destruction and fresh-device host-history reconstruction tested; async transfer, residency and automatic native-window device recovery pending |
| CLI | Clap | Integrated, command modules and end-to-end tests |
| File dialogs | rfd | Integrated for Open, Save As and Export |
| macOS menus | muda 0.21.2, default features disabled | Optional platform menus feature; desktop action routing and guarded Quit, no CLI dependency |
| Installed fonts | fontdb 0.23.0 | Shared with Cosmic Text to avoid a second database/version; standard installed-family enumeration; native std/fs plus Cosmic memmap/std features |
| Standard image codecs | image | Integrated baseline, 0.25.10 with only PNG/BMP/JPEG; strict admission and loss reporting remain in PaintCraft |
| Snapshot publication | Per-document coordinator; ArcSwap candidate | Synchronous atomic session baseline; concurrent publication study pending |
| Native container | ZIP64 reader/writer; raw and Zstd blocks | Current canonical ZIP subset is bounded and stored-only; extension must preserve v1 fixtures |
| Storage compression | Raw/LZ4/Zstd comparison | Shared raw, solid and independent patch records plus explicit lz4_flex 0.14.0 (safe checked block APIs, no alloc/frame) and zstd 0.14.3 (default features off, fixed levels 1/3) policies; CRC-protected independent restore tests pass. Default remains raw/patch; local BF19 comparisons retain CPU/RSS/ownership tradeoffs. Native session append segments and immutable byte leases are integrated; natural/mask/depth, relocation/GC and interference qualification remain pending (see HISTORY_STORAGE.md) |
| Color management | Evaluate moxcms/LittleCMS | Profiles currently reported as discarded; color-managed pipeline pending |
| Document text | Cosmic Text 0.19.0 + Swash 0.2.10 | std/swash/shape-run-cache, default features and fontconfig disabled; HarfRust shaping, fontdb fallback, hinted grayscale masks; atomic preview/publication; legacy bitmap commands remain stable. [Comparison](TEXT_RENDERING.md) |
| Wasm effects | wasmi conformance baseline; Wasmtime throughput candidate | Not implemented; core wasm32 ABI is the first SDK target |
| Statistical benchmarks | Divan 0.1.21 and Criterion 0.8.2 (development only) | Divan retains allocation summaries; Criterion adds raw per-sample JSON/CSV for bounded CPU partitions, recorded history and raster paint-plan replay with optional parallel analysis disabled. Earlier Divan-only raw-sample gates remain pending. See [parallel renderer](PARALLEL_COMPOSITOR.md) |
| Process benchmarks | Hyperfine 2.0.0 | Three native sessions per CPU span, cache and partition workload, with raw exports and RSS evidence; [span decision](../benchmarks/results/cpu-spans-2026-10-19/DECISION.md), [cache decision](../benchmarks/results/cpu-cache-2026-10-20/DECISION.md), [partition decision](../benchmarks/results/cpu-parallel-2026-10-20/DECISION.md). Other cases remain pending |

## Admission and evidence

Each adoption records the requirement/task, real consumer, version/features from the
compiled lockfile, correctness checks, and benchmark disposition. The fifteen BF01–BF15
cases remain pending until their actual workloads are measured. No optimized or
release-qualified claim follows from adding a dependency or passing a model test.
The tracked [roadmap ledger](ROADMAP_STATUS.md) identifies remaining acceptance work.
