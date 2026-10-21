# Excluded harness preflight

No timings were collected. The benchmark attempted to upload an egui font-atlas
placeholder before the first UI pass. The native application already performs that
pass through eframe. Both benchmark variants now initialize and upload the same UI
state before the canvas sequence. The manifest and prototype preserve the failed
source/log; its other source hashes match the recorded reproduction base commit.
