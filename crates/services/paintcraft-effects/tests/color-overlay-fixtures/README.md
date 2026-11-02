# Independent Color Overlay fixtures

Fraction-only portable generator; no product/upstream evaluator imports. Run
`python3 generate.py --check` here for byte equality. 6912 RGBA cases cover three
effect modes, six opacities and six alphas;216 future mask/opacity stage cases
and155 intermediate-rounding counterexamples are separately labelled.

One final rational RGB round, incoming alpha exact including hidden RGB. These
are PaintCraft byte-policy expectations, not upstream f32 parity or execution
observations. Generator seed/order and packed RGB/RGBA hashes are in oracle.json.
Future-stage columns alone do not qualify document/render integration.
