# Independent ordered Hue/Saturation reference

Source-only proposal until the complete integration gate. Run `python3 generate.py --check` from this directory after promotion. The generator verifies both oracle.json and the numeric Rust records in src/hue_saturation_tests/ordered_data.rs. The ignored staging location can pass --repo-root/--rust-data explicitly; normal maintained execution needs neither.

27 finite cases use exact Fraction HSL/V references and independently solved Curves polynomials; hashes and paths are bound in oracle.json. No product evaluator supplies expectations. Three HSL triples, coverage0/127/255 and V100/free Curves/Box1 in both orders yield16 causal order differences. Every point candidate rounds once then mixes A8 once. Box samples an opaque3x1 row with finite CLAMP; its vertical repetitions cancel exactly. Do not generalize that simplification to transparent Box. This does not claim hardware/runtime success.
