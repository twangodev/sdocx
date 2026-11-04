# InkPen V4 translucent 5× host control

A synthetic straight stroke with ARGB `80402010` extends the high-scale controls to one translucent color. Native host Mesa readback is retained in `packed.bin`; `packed.json` includes its input, geometry stream and hashes. The public probe selects `4;` while the capture uses the empty V4 setting. The scale-5 contour reduces SVG/resvg alpha error from 87,924 to 3,032 and direct PDF/MuPDF RGB error over #fcfcfc from 636,475 to 427,708 compared with the scale-1 contour. MuPDF's background/color conversion contributes to the RGB totals. This single synthetic host control does not establish device or saved-note parity, other colors or trajectories, or native PDF parity.

Reproduce the capture with the local native libraries and inputs described in the parent README:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias INKPEN_CAPTURE_SCALE=5 python3 tools/inkpen-capture/inkpen_v4_color_scale_capture.py --capture-one --argb 0x80402010 > /tmp/v4-alpha5-capture.json
python3 tools/inkpen-v4/alpha5/pack.py /tmp/v4-alpha5-capture.json /tmp/v4-alpha5
cmp /tmp/v4-alpha5/packed.json tools/inkpen-v4/alpha5/packed.json
cmp /tmp/v4-alpha5/packed.bin tools/inkpen-v4/alpha5/packed.bin
```

The export figures above are historical measurements from a temporary source copy; they do not validate the current checkout.

A separate capture sets the native ObjectStroke advanced setting to literal `4;`. Its native getter returned `4;`, selected V4, and produced the identical complete geometry stream and framebuffer SHA-256 as the empty-setting capture. This verifies the setting pair for this input. The capture script modifies an external caller source in memory and checks replacement anchors, but does not verify the runner or caller source hashes. Its native libraries and helper dependencies are not fully pinned. The inherited case label still says `empty`, so the native getter is the authoritative setting observation. `verify-literal.py` compares the recorded getter, selected renderer, stream values and stream hash, plus the capture-reported framebuffer digest; it does not decompress or independently hash that capture’s framebuffer. Retained fixture hashes identify bytes, not the complete capture runtime.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias INKPEN_CAPTURE_SCALE=5 python3 tools/inkpen-v4/alpha5/capture-literal.py --capture-one --argb 0x80402010 > /tmp/v4-5-literal.json
python3 tools/inkpen-v4/alpha5/verify-literal.py /tmp/v4-5-literal.json
```
