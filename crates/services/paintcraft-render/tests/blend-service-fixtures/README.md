# Independent mixed CPU-service pixels

`generate.py` uses only Python's standard library and exact Fraction arithmetic.
It imports no Rust/product code. It freezes eight ordered Levels -> Exposure ->
Curves -> own mask -> opacity -> Multiply/Screen cases before the corresponding
Rust tests were authored. Run `python3 generate.py --check` to reproduce them.

Levels has constant128 output; its optional coverage is mixed once. Exposure+100
compares exact100th powers of sRGB-decoded half-byte thresholds, avoiding floating
point. Inverse Curves knots and centered/secant derivatives equal255-x exactly.
Own coverage and opacity each round alpha separately. Composition against an
opaque colored backdrop rounds its complete exact rational expression once.

These literals qualify only the declared finite cases after actual execution.
They establish no timing, instruction, RSS, platform or browser result by themselves.
