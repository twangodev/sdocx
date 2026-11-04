# InkPen2 V1 native Model tolerance controls

The retained host-Unicorn controls convert Fountain trajectories to setter-built
InkPen2 V1 objects; they are not saved InkPen2, complete application or Android
device captures. For Fountain source 29 (57 points, translated by −96, −268),
tool 2, size 9 and a 1920×1080 screen, omitted tolerance and explicit 0, 0.5, 2
and 100 produce identical nine-record Drawing and saved-redraw streams. The
native getter checks each supplied value. `receipt.json` retains complete
callback bytes and hashes.

A second saved Fountain trajectory, scene stroke 7, preserves 231 points and
channels after translation by (−148.24111938476562, −116.8331298828125).
Explicit tolerance 0.5 produces 46 matching Drawing/saved-redraw records in
`scene7-redraw.json`. Current Rust tests compare every record for both controls:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test -p sdocx --features serde,pdf v1_model_tolerance_ --lib
```

Eight native Mesa paint captures retain geometry and frame hashes in
`paint-receipt.json`: source 29 omitted/0.5, scene 7 at 0/0.5/100, and a
synthetic four-point tool-3 turn at 0/0.5/100. Each group has identical geometry
and RGBA bytes. The tool-3 control uses size 4, 1440×2560 and a 2× transform.
The three compressed native frames remain retained.

The explicit V1 route admits finite tolerance 0–100 and passes 2 to its sampler.
Direct-call disassembly of `RedrawPen` at 0x3ecac and `redraw` at 0x3f00c found
no `GetInitialToloerance` call; the latter retrieves points, time, pressure, tilt,
orientation and tool directly. Admission between captured values extrapolates
from this bounded dataflow. Other versions retain the zero-only guard; this does
not establish every indirect caller or general tolerance behavior.

Historical public SVG and direct-PDF comparisons had lower RGB L1 error than
fallback on all three controls in resvg, MuPDF and PDFium. Native transparent
frames were composited over #fcfcfc; the first two controls used 1× SVG and
4/3 PDF pixels per point, and tool 3 used 2× SVG and 8/3 PDF pixels per point.
`paint-receipt.json` retains the measured error pairs and provenance. The optional
exporter and reader comparator were retired. These results are not current
reader validation or pixel parity; the reader/toolchain closure was not fully
pinned.

Native acquisition requires the locally held saved-caller harness, pinned
proprietary libraries, Unicorn/Mesa and additional helper dependencies. These
are not bundled as a clean-checkout runtime. From the repository root on the prepared capture host:

```sh
.venv/bin/python tools/inkpen2-v1-model-tolerance/capture.py > /tmp/inkpen2-tolerance.json
.venv/bin/python tools/inkpen2-v1-model-tolerance/capture_scene7.py
.venv/bin/python tools/inkpen2-v1-model-tolerance/capture_paint.py
```

Compare the first command's JSON with `receipt.json`. The other commands check
the retained scene-7 streams and eight paint captures respectively. Capture
scripts and native inputs remain independent of the retired export workflow.
