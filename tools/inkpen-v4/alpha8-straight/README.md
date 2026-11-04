# InkPen V4 8× translucent host control

This literal `4;` synthetic straight-stroke capture uses ARGB `80402010` and host Mesa. The 8× contour reduces public SVG/resvg alpha error from 246,332 to 4,862 and direct PDF/MuPDF RGB error over #fcfcfc from 1,684,519 to 1,090,090 versus the 1× contour. MuPDF color conversion contributes to the RGB total. It extends the existing translucent 5× straight control; it does not establish other trajectories, device or saved-note parity, or native PDF parity. The capture script does not honor `INKPEN_CAPTURE_BENT`.

Reproduce with the local native libraries and dependencies in the parent README:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias INKPEN_CAPTURE_SCALE=8 python3 tools/inkpen-v4/alpha5/capture-literal.py --capture-one --argb 0x80402010 > /tmp/8-straight.json
```

The fixture contains the native source input, stream and compressed frame. The capture checks replacement anchors but does not verify the external runner or caller hashes; native libraries and helper dependencies are not fully pinned. Fixture hashes identify retained bytes, not the complete capture runtime. The historical paired metrics used the highscale export probe and shared alpha8 comparator; reproduction requires matching instrumented source and readers. See the crossing README for the comparator invocation. The measured build used a Git archive of commit `a5398c251230db3890661ec3f685d2ee472e316b`, with the workspace `inkpen.rs` overlaid and the temporary export probe inserted. The comparison receipt records hashes of the renderer and relevant sources, but does not attest the full dependency closure. Exports were temporary. The capture uses a literal setting, and no empty-setting equivalence is assumed for other inputs.

Copy the fixture into a disposable directory before running the alpha8 comparator: it writes `probe.svg`, `probe.png`, and `comparison.json` there. The receipt identifies the resvg executable by hash; it does not independently attest its version.

The shared [alpha8 comparator](../alpha8-crossing/compare.py) takes the disposable fixture directory, resvg executable, instrumented build source tree, and caller-supplied build provenance description. Its source hashes include the temporary probe; the description is supplied context, not independently verified. See the crossing README for the exact invocation and comparison procedure. The inherited capture case label `advanced_setting_state` says `empty`; the native getter reports `4;` and is authoritative for this control.
