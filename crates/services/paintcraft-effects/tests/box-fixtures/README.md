Independent proposed BoxBlurV1 numerical fixtures
=================================================

These fixtures describe the proposed PaintCraft BoxBlurV1 foundation, not a
shipped blur feature or Palette byte-parity result. The generator uses only
Python stdlib `Fraction`, integer counters, SHA-256, and JSON. It never imports
PaintCraft, Palette, image libraries, product outputs, native binaries, or FFI.
The retained pinned Palette source is a separate reference: its separable
premultiplied-u16 rounding is deliberately not this contract.

Run `python3 generate.py` to verify the complete `oracle.json` byte for byte.
Use `--write` only after intentionally reviewing a proposed contract change.
No binary output streams need committing: each stream has explicit framing,
record counts, byte counts, and a frozen SHA-256 expectation in `oracle.json`.
Rust tests should hash actual kernel outputs against these independently
calculated constants, alongside the literal controls.

Contract
--------

Radius is an integer 0–32, default2. The finite source extent stays fixed; sample
coordinates clamp to that source, including when a requested output ROI is an
interior rectangle. Source origins may be negative. Radius0 returns the exact
incoming RGBA sample, including hidden RGB. Positive radii use one full square
with N=(2r+1)^2 repeated/clamped samples, Sa=sum(A), and Sc=sum(C*A). Output alpha
is nearest(Sa/N); each output color is nearest(Sc/Sa) if Sa>0, otherwise0.
Nearest means exact nonnegative rational rounding, half ties upward. Channels
are encoded RGB8; there is no linear-light conversion or CMS application.

This excludes input hidden RGB when alpha0, changes alpha, and rounds only the
final candidate. If Sa>0, rounded byte alpha can be0 while RGB remains nonzero.
Do not zero those colors merely because the quantized alpha became0.

An A8 coverage value is applied once after rounding that candidate. Endpoints
are exact incoming/candidate copies. For an interior coverage m, define
W=(255-m)*Ap+m*Aq. Output alpha is nearest(W/255). If W>0, color is
nearest(((255-m)*Cp*Ap+m*Cq*Aq)/W). If W=0, color uses straight hidden-RGB
interpolation nearest(((255-m)*Cp+m*Cq)/255), preserving the specified zero-alpha
behavior. These are alpha-aware mask semantics, not the older point-filter
alpha-preserving mask rule.

Coverage and framing
--------------------

The15 plane literals include transparent-neighbor red, half ties, all-hidden
colors, alpha rounding to0, nonsquare negative-origin sources, and interior ROI
partitions at radii0/1/2/32. Each partition stores global region coordinates and
its exact expected row-major plane. They must be stitched by position, not
concatenated blindly. Positive-radius interior controls also prove that clamping
a cropped ROI produces a different result from clamping the true source.

The45 mask literals cover five input/candidate pairs at nine coverages. Twelve
additional planes apply one mask to an already-rounded actual Box candidate,
including both endpoints and fractional coverage128.

The three stream recipes are fully disclosed in generator order:

- 8,448 1×1 records: every radius0–32 and alpha0–255, fixed RGB[17,93,201].
- 198 two-pixel edge records: every radius, three declared RGBA pairs, both x.
- 46,080 mask records: four declared RGB pairs, every input alpha, five candidate
  alphas, and nine coverages. The complete source recipes live in JSON.

The54,726 records establish numerical expectations only. They do not qualify
product cancellation, bounded allocation, halo caching, persistence, renderer,
GPU, browser Worker, UI, exports, performance, or hardware integration.
