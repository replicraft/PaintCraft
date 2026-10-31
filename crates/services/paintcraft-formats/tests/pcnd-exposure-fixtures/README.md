# PCND6 Exposure codec fixtures

`python3 generate.py --check` verifies 21 independent deterministic specimens:
12 valid signed parameters (including disabled/default intentions) and 9 invalid
parameter lengths/ranges/codecs. The generator writes literal PCND6 bytes and
independently computes outer and semantic SHA256. It imports no product code and
uses no existing binary fixture as a parent. The raw two-pixel source includes
sRGB knee-adjacent values, hidden RGB and fractional alpha; these are codec tests,
not an Exposure rendering oracle. Existing eight-profile fixtures are unchanged.

Profile `exposure-srgb-v1`, scoped transport `params-v1`, exactly signed i16BE
`stops_hundredths` in [-800,800]. Disabled and zero-stop instances remain stored
catalog intentions. Generic reader support does not extend frozen Worker6 or
preview Source4 execution capabilities.
