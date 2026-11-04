# V7 finger holdout

A fresh synthetic Marker4 V7 tool-0 native ARM64/host Mesa capture uses the
retained `turn-35` input, 4× projection, phase (0.37, 0.63), and ARGB
`0x60c022a1`. The explicit scale-4 profile was compared
with its default fallback on white, using the public SVG and direct PDF exports.
RGB absolute-error sums against native were:

| Reader | Fallback | Opt-in |
| --- | ---: | ---: |
| resvg 0.45.1 | 4,018,544 | 66,705 |
| MuPDF 1.28.2 | 4,014,841 | 92,285 |
| PDFium 150.0.7869.0 | 4,016,752 | 128,994 |

This is a new phase and color for an existing shape, not an independent shape,
a real saved note, or device rendering. It does not justify broadening the
profile. The retained `../wide-extra/capture.py` pins the native libraries, adapters
and source fixture. Fresh capture requires the external ARM64 native libraries,
libc++, helper modules and fixture under the adapter's `ROOT` path, plus the
project's native execution runtime; those external artifacts are not included.
The capture is recorded in `capture.json.gz`
(SHA-256 after decompression `6acfc9dcae6f3fbd66d6e50eaff4e1cf822db2b312017046fb1e0d9562f035c9`)
and its GL bottom-up RGBA frame in `frame.rgba.gz` (decompressed SHA-256
`19eee088a631a1a3f3fcaf8e5d42ac457e9601e7de240e7b14f939a960e8a891`).

The exporter was built with rustc 1.98.0 and the shared Cargo target, against the
working checkout. Marker4 and render source SHA-256 were respectively
`a3aa15093295df2dc14414942ccffbb2dcc5510a0db6ef02559adc4a6ee6cf2a` and
`701b675ae40c7597a83ecd7419531fb260fc3a8961e88a66101c18f391f371cb`;
these two hashes do not pin the full dependency tree. A clean archive of commit
`782146d7` independently reproduced all four export hashes and all six reader
error sums using the retained harness lockfile. The exporter source SHA-256
is `22540d86dfbbfdacba3d494ae1e8f6297b24e58cc9516015f5d1b49d6e6e77fc`.
The experimental exporter, comparator, and harness have been retired; the receipt preserves their historical results and identities.

For a fresh native recapture, use `../wide-extra/capture.py` with the stated
arguments and a new output path. Its JSON contains the chosen image path, so its
digest differs from the retained capture and has a different digest from the retained capture. A fresh recapture requires its own input digest and receipt.
