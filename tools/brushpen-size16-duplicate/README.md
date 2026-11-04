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
to the receipt.

The experimental exporter, replay driver and reader comparator have been retired.
`serialized_mutations_match_native_geometry` checks all 33 native records in
memory using the retained stream. Export hashes and reader metrics above are
historical observations; the Rust geometry check does not revalidate external
PDF reader pixels. Native inputs, capture scripts and receipts remain retained.
