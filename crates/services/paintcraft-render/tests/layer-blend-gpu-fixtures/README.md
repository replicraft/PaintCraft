# Independent public GPU stages

Fraction equations and the frozen Core corpus are input only; no renderer readback or product numerical kernel is consulted. Every backdrop first composes over transparent black, so hidden RGB in a raw alpha-zero backdrop is not falsely seeded into a public GPU surface. Expected straight native and final premultiplied display bytes are distinct. Explicit mask127/opacity128, complete-native-before-LOD and partial-edge literals are included. generate.py --check is stdlib-only. Tests are authored after the oracle freeze; actual GPU qualification is pending.
