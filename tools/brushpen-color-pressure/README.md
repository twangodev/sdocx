# Colored pressure BrushPen host control

The synthetic serialized GLV10 object is loaded and attached by native Model, then changed to size 16, point 2 equal to point 0, 204 pressures repeating 0.15, 0.35, 0.65, 0.9, and opaque color #2379d1. Native Drawing and RTV1 run under Unicorn and host Mesa. This is not an original BrushPen note, full archive loading, device paint, or a native SVG/PDF export. The capture pins the principal native libraries and adapters; their transitive dependencies and toolchain are not pinned.

The complete 37-record callback stream matches the public sampler. Canvas PNG, resvg SVG, and direct and SVG-derived PDF in PDFium match the native premultiplied frame exactly. MuPDF's two PDF routes have alpha L1 83, maximum 1, and premultiplied RGB L1 675. The [receipt](receipt.json) records hashes and reader versions. This control does not justify a production change.

On the prepared host, reproduce the native capture (it checks the frame and stream against the retained compressed files and uses a private temporary directory):

```sh
BRUSHPEN_CAPTURE_ROOT="$PWD" PYTHONPATH="$PWD/scratch/apk-analysis-runtime/python" /tmp/sdocx-unicorn-venv/bin/python tools/brushpen-color-pressure/capture.py
```

The experimental exporter, replay driver and reader comparator have been retired.
`serialized_mutations_match_native_geometry` checks all 37 native records in
memory using the retained pressure cycle and stream. Export hashes and reader
metrics above are historical observations; the Rust geometry check does not
revalidate external PDF reader pixels. Native acquisition and receipts remain.
