# Independent Multiply/Screen reference vectors

Frozen SHA256:96f0a8debfec8d571b333691bb7a6f9d1fe894b4248b3c32fe7493c52ac49ff7.
Run `python3 generate.py --check` from any directory. Standard library only;
no product or Palette kernel imports.18 input cases×3 modes,2 exact channel
half-ties, and2 outputs for a three-layer order reversal.

The generator evaluates normalized encoded RGB with Fraction, W3C-style alpha
composition, and final nearest/ties-up. It explicitly applies separate existing
mask/opacity alpha-byte rounds. Transparent source leaves the existing backdrop
unchanged, including its hidden RGB. Each layer writes straight RGBA8 before the
next layer. Display premultiplication values are future boundary references;
these Core tests qualify only the standalone straight RGBA8 API.

Mathematical source review: Palette commit aefc84fe9233e66344147078de866860326d8cfa,
[composite/blend.rs](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/composite/blend.rs)
Git blobef52c04bf6229db37d8386dda4639cf624ca1b15 and composite.rs. Palette uses
f32; this independent rational policy does not claim byte parity. No source code
from Palette is copied into this fixture generator or product kernel. No Palette
GPU source or proprietary-Paint execution was used.
