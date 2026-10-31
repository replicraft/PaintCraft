# Independent Hue/Saturation foundation data

Frozen source-only Fraction oracle promoted byte-identically from the reviewed
planning data. This directory qualifies a standalone numerical foundation; it
does not register a twelfth Descriptor, PCND decoder, registry or UI capability.
Original oracle schema/status text is historical and deliberately unchanged.

Policy: exact encoded RGB->HSL; H degrees[-180,180], S/L hundredths[-100,100],
default0/0/0. Hue wraps Euclidean modulo1; saturation multiplies by1+S/100 and
clamps, lightness addsL/100 and clamps. Reconstruct RGB, round nearest/ties-up
ONCE to bytes; alpha exact, hidden RGB transformed. Coverage mixes that rounded
candidate once using existing alpha-preserving point policy.

`generate.py` imports ONLY the independently authored Fraction to_hsl/from_hsl
helpers from the tracked vibrance-fixtures/generate.py, hash
c77dd70bbea25ea7eacad1db4bceaedb07478ac9272af0993c29c6046c09f30d.
It imports no product, closed-form sector kernel or capacity checker. The sole
portability edit changes workspace-root depth for this tracked directory; output
oracle bytes remain exact SHA256
490cc0da56a18139abbed89b3866e449824a857ef816135e3d70c38244f25e72.
Original planning provenance remains in docs/validation/hsl-oracle/source-ready.json
and docs/validation/hsl-foundation/source/fixture-promotion.json.

Finite corpus:68colors x40parameter tuples=2720wholeRGB records;266causal exact
half-channel ties;7hand-pinned literals;3240RGBA/coverage records with6sampled
alphas, not all256alphas. Packed ordered RGB hash
91626bb73ff4c776b2278e5a10ca0939bbd9c9b3046ba606cb735c63e4ded10d;
RGBA hash9259aa809df6c9a1c6f63979a5054442359f1727daa0f0d4430ccd446c68eab0.
These samples do not exhaust361*201*201 parameter tuples or2^24colors. Fifteen
lexical/tuple/domain invalid controls remain data; a future parser needs its
own registration. Typed constructor bounds are separately tested in Core.

From repository root, regenerate/verify WITHOUT bytecode side effects:

    PYTHONDONTWRITEBYTECODE=1 python3 crates/services/paintcraft-effects/tests/hue-saturation-fixtures/generate.py --check

Expected math uses normalized Fraction hue_channel reconstruction; product math
uses independently derived checked-i64 sector rational arithmetic. No upstream
f32/Photoshop parity, performance, prepared/backend or host feature claim follows.
