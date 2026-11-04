# InkPen V4 8× translucent crossing host control

This literal `4;` synthetic crossing-stroke capture uses ARGB `80402010` and host Mesa. The 8× contour reduces public SVG/resvg alpha error from 469,948 to 9,524 and direct PDF/MuPDF RGB error over #fcfcfc from 2,232,062 to 1,100,108 versus the 1× contour. MuPDF color conversion contributes to the RGB total. It extends the existing translucent straight controls; it does not establish other trajectories, device or saved-note parity, or native PDF parity.

Run `PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias INKPEN_CAPTURE_SCALE=8 python3 tools/inkpen-v4/alpha8-crossing/capture.py --capture-one --argb 0x80402010 > /tmp/8-crossing.json` with the local native inputs described in the parent README. The script modifies the caller source in memory and asserts the trajectory and all replacement anchors. The fixture retains its source input, native stream, and compressed framebuffer. Replacement-anchor checks do not verify the external runner or caller hashes; native libraries and helper dependencies are not fully pinned. The fixture hashes identify retained bytes, not the complete capture runtime.

The paired export metrics are historical measurements from an instrumented source tree. The inherited case label says `empty`; the native getter reports `4;` and is authoritative for this control.
