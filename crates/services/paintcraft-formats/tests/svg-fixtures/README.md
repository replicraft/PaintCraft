# Independent SVG rendering corpus

These are authored SVG scenes and an arithmetic PNG, with no third-party
artwork. `generate.py` uses only Python's standard library. `oracle.mjs` renders
the scenes with independent Google Chrome, blocks network requests, and loads
the repository's Selawik bytes explicitly. It never calls PaintCraft or resvg.
Source, oracle PNG and font hashes are recorded in `oracle.json`.
`aligned.svgz` is a deterministic standard-library gzip sidecar for the real
CLI/MCP import tests; its compressed-byte hash is in `sources.json`.

Regenerate from the repository root using an isolated Playwright installation:

```sh
python3 crates/services/paintcraft-formats/tests/svg-fixtures/generate.py
pnpm --dir /tmp/paintcraft-svg-oracle add playwright@1.64.1
PLAYWRIGHT_PACKAGE_JSON=/tmp/paintcraft-svg-oracle/package.json \
    node crates/services/paintcraft-formats/tests/svg-fixtures/oracle.mjs
cargo test -p paintcraft-formats --test svg_oracle -- --nocapture
```

Google Chrome must be installed. Only headless windows are used. The checked-in
oracles were captured with Chrome 155.0.8059.85 and Playwright 1.64.1 on macOS on
2026-10-25, at 96 DPI and one device pixel per CSS pixel.

The aligned rectangles and nearest-sampled embedded PNG match exact RGBA. The
remaining comparisons use premultiplied RGBA to avoid undefined transparent RGB
and amplification when unpremultiplying faint antialiased edges. Values below
are measured 8-bit channel differences, not performance results.

| Scene | Maximum difference | Mean absolute difference | Pixels with a channel difference above 8 |
| --- | ---: | ---: | ---: |
| Aligned rectangles | 0 | 0 | 0% |
| Gradient and local use | 1 | 0.165365 | 0% |
| Mask and clip | 28 | 0.519043 | 4.2969% |
| Gaussian blur | 12 | 1.070964 | 6.3802% |
| Curves and group opacity | 68 | 0.683350 | 6.6406% |
| Selawik text | 108 | 1.783875 | 11.8408% |
| Pixelated embedded image | 0 | 0 | 0% |
| CSS auto overriding the pixelated attribute | 7 | 1.562500 | 0% |
| CSS smooth discrepancy, excluded from agreement | 112 | 16.000000 | 50% |

The last scene is deliberately retained. Chrome reports that CSS
`image-rendering: smooth` is unsupported and keeps the presentation attribute's
pixelated mode. resvg implements smooth sampling. Its mismatch is checked and
reported separately, never counted as agreement. Pixelated and crisp-edges
presentation attributes need a private adapter hint conversion for usvg 0.48.1;
the CSS-auto case checks that stylesheet priority survives this conversion.

The test's agreement gates require exact samples for the two exact cases and
mean error at most 2 with at most 15% significant pixels for the other six.
This small corpus does not prove pixel equivalence, arbitrary SVG support,
font equivalence across platforms, process confinement, or bounded cancellation
within the upstream renderer.
