# Independent CPU-region blend benchmark expectations

Stdlib Fraction literals frozen before benchmark Rust source, separate from the
production Core/Render implementations. No product import or expected-pixel
calculation occurs in the benchmark helper. The portable generator reproduces
`oracle.json`; hashes are independent expected output hashes, not actual measured
artifact/output hashes. See the candidate protocol for scope and unexecuted status.
