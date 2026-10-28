# InkPen V4 3× host comparison

`comparison.json` records absolute errors for five synthetic 3× host-Mesa
readbacks against the public SVG and direct PDF exporters. The 1 and 3 keys
select the export contour; all outputs are rasterized at 3×. SVG uses resvg
and alpha-channel error; MuPDF and PDFium use RGB error over #fcfcfc. The
fixture includes native stream and raw-frame hashes. These are host controls,
not device or saved-note parity.

Reproduce native frames from the candidate checkout, with its capture inputs
available under `tmp/` and `scratch/`:

```sh
export PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias
INKPEN_CAPTURE_SCALE=3 python3 conformance/inkpen_v4_scale_capture.py --print-capture > /tmp/v4-3-straight.json
for name in 1 loop phase; do
  INKPEN_CAPTURE_SCALE=3 INKPEN_CAPTURE_BENT="$name" python3 conformance/inkpen_v4_scale_capture.py --print-capture > "/tmp/v4-3-$name.json"
done
INKPEN_CAPTURE_SCALE=3 python3 conformance/inkpen_v4_color_scale_capture.py --capture-one --argb 0x80402010 > /tmp/v4-3-color.json
python3 tools/inkpen-v4/pack.py /tmp
```

The packer writes the fixture in the checkout; run it only on a disposable copy
when preserving existing fixtures. The capture scripts use native inputs described by the existing scale receipt;
the loop and phase routes additionally enforce helper hashes.
The `phase` capture was independently rerun with matching stream and RGBA hashes.

To reproduce public comparisons, install the Python imports used by `compare.py`
and provide the existing resvg helper binary. Coordinate the shared Cargo target
before running. The script archives `HEAD`, overlays only candidate render files
and fixtures into a temporary directory, instruments that copy, and cleans it up.
It never edits the source checkout. For example:

```sh
python3 tools/inkpen-v4/compare.py --repo /path/to/git-checkout \
  --source /path/to/candidate-tree \
  --svg-renderer /tmp/sdocx-agent-target/debug/sdocx-render-svg-047 \
  --output /tmp/inkpen-v4-3x-comparison.json
```

Compare the resulting per-case hashes and errors with `comparison.json`; raster
library versions can change numeric results. The temporary export instrumentation
uses the captured saved inputs and calls the public SVG and PDF APIs.
