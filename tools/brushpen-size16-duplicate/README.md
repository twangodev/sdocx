# Serialized-derived BrushPen mutation control

The existing synthetic GLV10 serialized object was loaded by native Model, then
mutated after attachment: pen size 16 and point 2 copied from point 0. Drawing
and RTV1 executed under the existing Unicorn and host Mesa adapters. This is not
an independently serialized note or Android device capture. The captured 33
callback records match the public sampler byte for byte. Against the retained
321×228 native frame, alpha L1/max are 1/1 for canvas PNG, resvg SVG, and
PDFium direct PDF; MuPDF direct PDF is 73/1. Premultiplied RGB L1 is zero in all
four comparisons. SVG-derived PDF matches the corresponding direct PDF metrics.
The receipt identifies input, script, stream, frame and export hashes and reader
versions. No production change follows from this control.

On the prepared host, run from the repository root:

```sh
BRUSHPEN_CAPTURE_ROOT="$PWD" PYTHONPATH="$PWD/scratch/apk-analysis-runtime/python" /tmp/sdocx-unicorn-venv/bin/python tools/brushpen-size16-duplicate/capture.py
```

The capture requires the pinned DSOs and ignored helpers listed in the base
capture and writes `/tmp/brush-variant-{frame,stream}.bin`. Compare their hashes
to the receipt. To replay the public exporter from the pinned clean Git commit,
run with resvg, NumPy, Pillow, PyMuPDF and pypdfium2 available:

```sh
PYTHON="$PWD/.venv/bin/python" .venv/bin/python tools/brushpen-size16-duplicate/replay.py
```

The replay checks the source payload, table, frame, paint source, scripts and
native fixture hashes, adds the retained focused test to a temporary Git archive,
checks its complete geometry against the native stream, exports SVG and direct
PDF, and asserts reader metrics. It uses the shared agent target and removes the
temporary source checkout. The lockfile is from the pinned commit; installed
compiler and reader dependency closures are not pinned. The comparator reads
the compressed frame directly and checks its digest.
