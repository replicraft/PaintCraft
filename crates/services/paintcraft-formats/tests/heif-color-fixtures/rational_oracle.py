"""Independent exact rational reference for the explicit PaintCraft RGB policy.

No selected-decoder implementation is imported. H.273 matrix1 constants use
exact decimal fractions; H.265 type0 420 siting is x=0,y=1/2 luma pixels.
Bilinear interpolation clamps source coordinates to outermost chroma samples.
Quantization rounds positive values at half upward and clips to unsigned8.
"""
from fractions import Fraction as F


def rgb(raw, width, height, chroma, matrix, full_range):
    count = width * height
    cw, ch = (width // 2, height // 2) if chroma == 1 else (width, height)
    ccount = cw * ch
    if len(raw) != count + 2 * ccount:
        raise ValueError("unexpected authored sample extent")
    luma, cb, cr = raw[:count], raw[count:count + ccount], raw[count + ccount:]

    def sample(plane, x, y):
        xx = F(x, 2) if chroma == 1 else F(x)
        yy = F(2 * y - 1, 4) if chroma == 1 else F(y)
        xx, yy = max(F(0), min(F(cw - 1), xx)), max(F(0), min(F(ch - 1), yy))
        x0, y0 = int(xx), int(yy)
        x1, y1 = min(cw - 1, x0 + 1), min(ch - 1, y0 + 1)
        tx, ty = xx - x0, yy - y0
        return (
            plane[y0 * cw + x0] * (1 - tx) * (1 - ty)
            + plane[y0 * cw + x1] * tx * (1 - ty)
            + plane[y1 * cw + x0] * (1 - tx) * ty
            + plane[y1 * cw + x1] * tx * ty
        )

    result = bytearray()
    for y in range(height):
        for x in range(width):
            if matrix == 0:
                result.extend((cr[y * width + x], luma[y * width + x], cb[y * width + x]))
                continue
            yy = F(luma[y * width + x], 255) if full_range else F(luma[y * width + x] - 16, 219)
            blue = (sample(cb, x, y) - 128) / (255 if full_range else 224)
            red = (sample(cr, x, y) - 128) / (255 if full_range else 224)
            values = (
                yy + F(15748, 10000) * red,
                yy - F(2126 * 15748, 7152 * 10000) * red - F(722 * 18556, 7152 * 10000) * blue,
                yy + F(18556, 10000) * blue,
            )
            result.extend(min(255, max(0, int(v * 255 + F(1, 2)))) for v in values)
    return bytes(result)
