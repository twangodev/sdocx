# InkPen V4 8× translucent host control

This literal `4;` synthetic straight-stroke capture uses ARGB `80402010` and host Mesa. The 8× contour reduces public SVG/resvg alpha error from 246,332 to 4,862 and direct PDF/MuPDF RGB error over #fcfcfc from 1,684,519 to 1,090,090 versus the 1× contour. MuPDF color conversion contributes to the RGB total. It extends the existing translucent 5× straight control; it does not establish other trajectories, device or saved-note parity, or native PDF parity. The capture script does not honor `INKPEN_CAPTURE_BENT`.

Reproduce with the local native libraries and dependencies in the parent README:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias INKPEN_CAPTURE_SCALE=8 python3 tools/inkpen-v4/alpha5/capture-literal.py --capture-one --argb 0x80402010 > /tmp/8-straight.json
```

The fixture contains the native source input, stream and compressed frame. The capture checks replacement anchors but does not verify the external runner or caller hashes; native libraries and helper dependencies are not fully pinned. The export metrics are historical measurements. The inherited case label says `empty`; the native getter reports `4;` and is authoritative for this control.
