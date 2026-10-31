# Curves PCND6 planned specimens

Status: source-only planning, not accepted codec support or reader qualification.
`generate.py` is an independent standard-library literal framing writer. It does
not invoke PaintCraft serializers, readers, evaluators or a prepared LUT. Run
`python3 docs/validation/curves-format-planning/generate.py --check` to reproduce.

The 14 specimens contain nine planned positive cases: identity, free endpoints,
nonmonotone, both extreme constants, disabled identity and nonmonotone intentions,
a fractional A8 mask with negative domain, and an ordered stack mixing Invert,
Curves, Levels and disabled Exposure. Five negatives cover exact-length 4/6,
unknown codec/profile even when disabled, and a corrupt inner SHA256 trailer.
PCND uses SHA256, not CRC. Literal raw pixels include hidden RGB and partial alpha.
Positive stack hashes independently encode semantic v3, including saved mask
intent. Rendered appearance is deliberately outside this byte-codec oracle.

The proposed `curves-rgb-v1/params-v1` payload is exactly five output bytes in
fixed input-knot order [0,64,128,192,255]; no endpoint lock or monotonicity rule.
Existing PCND1–5 framing and all old fixture files remain untouched. Adding this
pair to a future generic PCND6 reader must not grant worker execution authority.
Source4 and Export6 must continue to refuse every stored Curves requirement,
including identity, disabled instances and empty/disabled masks, before adoption
or candidate work. Their frozen capability profiles are independent of whether
the generic file reader recognizes these bytes. Current legacy writer refusal
remains in place; these specimens do not activate a codec or registry.
