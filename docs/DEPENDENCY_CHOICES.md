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
| `rayon` | selected-on-use | Bounded native render preparation pool | BF06 | Integrated baseline, 1.12.0; queue/byte admission, cancellation and panic tests; performance/fairness qualification pending |
| `tokio` | preferred-on-use | Native saves, checkpoints, startup recovery and cleanup | BF07 | Integrated baseline, 1.53.3 with rt/sync behind native-io; bounded FIFO, completion retention, panic and shutdown tests; recovery publication fences and native race tests pass; startup recovery integrated; restore integration and benchmark pending |
| `moka` | selective-on-use | Discardable derived CPU cache with independent memory accounting | BF08 | Not directly adopted; activate with its consumer |
| `fst` | deferred | Large immutable asset catalog, when one exists | BF09 | Not directly adopted; activate with its consumer |
| `smallvec` | selective-on-use | Short dirty-region and dependency lists | BF10 | Not directly adopted; activate with its consumer |
| `bitvec` | selective-on-use | Finite binary selection/dirty coverage | BF11 | Not directly adopted; activate with its consumer |
| `arrayvec` | selective-on-use | Scratch collections with a proven bound | BF12 | Not directly adopted; activate with its consumer |
| `dashmap` | evaluate | Narrow concurrent immutable-resource registry, if justified | BF13 | Not directly adopted; activate with its consumer |
| `slotmap` | selected-on-use | Runtime render-job handles with a separate owner UUID | BF14 | Integrated baseline, 1.1.1; cross-owner/stale-handle and slot-reuse tests; benchmark pending |
| `indexmap` | selective-on-use | Ordered presets, effect parameters and command metadata | BF15 | Not directly adopted; activate with its consumer |

## Other product and runtime selections

| Area | Choice | Application state |
|---|---|---|
| UI | egui | Integrated through eframe; Classic ribbon/vector widgets exist; reference fidelity and Studio remain pending |
| Native shell | Compare eframe with direct egui-winit/egui-wgpu | eframe remains provisional pending T02 hardware/IME traces |
| GPU | wgpu + WGSL | Integer compositor integrated; Metal parity tested; async transfer/device-loss work pending |
| CLI | Clap | Integrated, command modules and end-to-end tests |
| File dialogs | rfd | Integrated for Open, Save As and Export |
| Standard image codecs | image | Integrated baseline, 0.25.10 with only PNG/BMP/JPEG; strict admission and loss reporting remain in PaintCraft |
| Snapshot publication | Per-document coordinator; ArcSwap candidate | Synchronous atomic session baseline; concurrent publication study pending |
| Native container | ZIP64 reader/writer; raw and Zstd blocks | Current canonical ZIP subset is bounded and stored-only; extension must preserve v1 fixtures |
| Storage compression | Raw/LZ4/Zstd comparison | Raw COW tile baseline; measured compression/compaction pending |
| Color management | Evaluate moxcms/LittleCMS | Profiles currently reported as discarded; color-managed pipeline pending |
| Text | Evaluate parley or cosmic-text plus rasterizer | Bitmap text baseline only; system fonts, shaping and IME pending |
| Wasm effects | wasmi conformance baseline; Wasmtime throughput candidate | Not implemented; core wasm32 ABI is the first SDK target |
| Statistical benchmarks | Criterion or Divan, selected at first comparison | Not yet selected |
| Process benchmarks | Hyperfine | Installed developer tool; application decision runs not yet recorded |

## Admission and evidence

Each adoption records the requirement/task, real consumer, version/features from the
compiled lockfile, correctness checks, and benchmark disposition. The fifteen BF01–BF15
cases remain pending until their actual workloads are measured. No optimized or
release-qualified claim follows from adding a dependency or passing a model test.
The tracked [roadmap ledger](ROADMAP_STATUS.md) identifies remaining acceptance work.
