The scripts in this directory retain the pinned native capture harness and its
required Python helpers. `capture.py` adapts the retained harness in memory for
the two synthetic controls. The original scripts retain historical paths in
their source text; the wrapper substitutes the local path before execution.
The native libraries under `scratch/apk-analysis-native/arm64-v8a`, the runtime
Python adapters under `scratch/apk-analysis-runtime/python`, Unicorn, and the
pinned host EGL/GLES libraries remain prerequisites. The temporary runtime
directory contains only symlinks to those libraries; all library hashes are
checked by the harness.

From the repository root, use a new output filename for each capture:

```sh
MB_CASE='{"points":[[1,3],[10,10],[20,2],[30,0]],"pressure":[0.3,0.6,1,0.7],"pen_size":2}' PYTHONPATH=scratch/apk-analysis-runtime/python:tools/montblanc/extended-paint/conformance .venv/bin/python tools/montblanc/extended-paint/capture.py --case baseline --output /tmp/montblanc-four.json
MB_CASE='{"points":[[0,0],[5,8],[11,-1],[15,6],[22,-3]],"pressure":[0.3,0.6,1,0.7,0.5],"pen_size":2}' PYTHONPATH=scratch/apk-analysis-runtime/python:tools/montblanc/extended-paint/conformance .venv/bin/python tools/montblanc/extended-paint/capture.py --case baseline --output /tmp/montblanc-five.json
.venv/bin/python tools/montblanc/extended-paint/scene_compare.py
```

Compare the capture frame and native-vector hashes with
`conformance/montblanc-extended-paint-native.json`. The Scene comparison
uses the retained captures, the pinned source revision and tracked synthetic
legacy-marker fixture. It requires offline Cargo dependencies, resvg, Pillow,
and PyMuPDF and writes its compact comparison receipt in `conformance`.
