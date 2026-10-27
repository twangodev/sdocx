# InkPen2 null-setting V1 host paint

The [receipt](../../conformance/inkpen2-null-v1-paint-native.json) records two
synthetic Drawing-selected RTV1 captures for the baseline geometry in the
[controls](../../conformance/inkpen2-null-v1-controls-native.json). Both use a
1920 × 1080 screen profile, opaque `#336699`, a 128 × 96 host RGBA8 target and
unit draw scale. The first uses the identity draw matrix and clips against the
target edge; the second translates by (32, 32). Native RTV1 uploaded the same
300-byte, 25-record geometry in both cases and drew 25 instances. Its observed
fragment shader uses derivative-based circular coverage and source-over
blending. A fresh recapture reproduced both full RGBA SHA-256 values on this
host. These are host Mesa outputs, not Android/device pixels, a complete app
canvas, serialized-note admission, or SVG/PDF parity.

The [capture adapter](../../conformance/inkpen2_v1_paint_native.py) checks the
SHA-256 of the joined native harness and seven support scripts before running;
the parent harness checks pinned native libraries. It requires the existing
`scratch/apk-analysis-native`, `scratch/apk-analysis-runtime` and
`tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance` research
tree, plus host EGL/GLES. The support tree is a local prerequisite, not a
portable checked-in capture package; a different Mesa implementation can yield
different pixels. From the project root, recapture with:

```sh
export PYTHONPATH=scratch/straightinkpen2-alias:scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin
python3 conformance/inkpen2_v1_paint_native.py --print-capture > /tmp/inkpen2-v1-original.json
python3 conformance/inkpen2_v1_paint_native.py --print-capture --matrix-translate-x 32 --matrix-translate-y 32 > /tmp/inkpen2-v1-translated.json
```

Compare `geometry_sha256`, `rgba_sha256`, `draw_matrix`, and the complete
compressed RGBA readbacks with the corresponding receipt fields. The current
production vector renderer remains V9-specific; V1 vector paint and paired PDF
comparison remain open.
