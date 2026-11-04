# Crayon2 document grid holdout

This fresh synthetic native Model/Drawing and Mesa capture uses a third path
`(33,58), (57,39), (82,72)`, pressure `0.3, 0.6, 1`, ARGB `c23c92d7`,
and a 160×160 opaque target with background RGB `(64,128,192)`. The view
translation is zero. The native texture-scale uniform is
`1.4222222566604614` on both axes. The 160-pixel projection changes the
scale from the previous 192- and 256-pixel controls; this is not a nonidentity
view-transform control. The capture and its pinned native dependencies are
host-only, not evidence of Android or saved-note parity.

A historical export experiment independently sampled this source,
checked that its vector SHA-256 matched the native vector, and rendered the
actual public document SVG and direct document PDF with the opt-in profile.
The retained native frame is not passed to the renderer. Against that frame,
resvg differs in 342 pixels with maximum RGB channel error 3 and total absolute
RGB error 478. Direct PDF differs in 443/4/1115 for MuPDF and 442/4/1111 for
PDFium. These residuals provide no justification for a paint correction. The
receipt retains hashes and metrics; `frame.rgba.zlib` is the native bottom-up
RGBA8 readback and `vector.bin` is the native uploaded record stream.

To reproduce, use the local native prerequisites described in
[the independent control](../crayon2-independent-control/README.md), then:

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-holdout.XXXXXX)
export PYTHONPATH="$PWD/tmp/conformance-cleanup/native-capture-a241793/conformance:$PWD/conformance:$PWD/scratch/apk-analysis-runtime/python"
export LD_LIBRARY_PATH="$PWD/scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
.venv/bin/python tools/crayon2-document-holdout/capture.py > "$work_dir/native.json"
.venv/bin/python tools/crayon2-document-holdout/verify_native.py "$work_dir/native.json"
```

The native verifier checks source, uniforms, texture uploads, blend and draw evidence, and the retained frame and vector bytes. Texture and helper inputs are local, hash-gated dependencies, not supplied by a clean checkout. The export generator and comparator have been retired.
