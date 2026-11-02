# Independent CPU integration expectations

The stdlib Fraction generator computes exact Box-before-mask, separate mask/opacity rounds, ordered blend and complete-native alpha-weighted LOD literals. It never calls a product codec or numerical kernel. The raw two-pixel source and clamped Box averages are explicit literals. Run generate.py --check. General54 expectations remain the separately frozen portable Core layer-blend-fixtures corpus. No product expectation is recorded from renderer output.
