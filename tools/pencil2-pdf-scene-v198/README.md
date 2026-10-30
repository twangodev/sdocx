# Pencil2 direct PDF marker experiment

This isolated diagnostic patches the pinned `61fdc795adb2e2b075b6417ec9533f41cf884861` source archive after the v195 probe patch. It adds no production option. Run from the repository root:

```sh
PYTHONPATH=/tmp/sdocx-marker3-v133-readers tools/pencil2-pdf-scene-v198/run.sh
```

The script needs offline Cargo dependencies and resvg 0.45.1; the Python environment needs Pillow 12.1.1, PyMuPDF 1.28.2 and PDFium 156.0.8076.0. The observed build used Rust/Cargo 1.98.0. The runner checks the pinned probe, example, lockfile and crop hashes and uses a fresh temporary snapshot and output directory, removed on exit. The sampler checks the source-generated 84-record stream SHA-256 `770651fc1b27bfa8a73731f1a3e24cf1a37847e920581aa36855c5107835c2fe`. Native pixels are comparison targets, not marker inputs.

The normal direct document PDF route renders a typed ink marker at the stroke's actual position, then replaces that marker image with a transparent Form containing the v197 ordered luminosity mask. Existing content and resources are retained. Admission requires the exact black source stream, a 192×192 page and the test environment switch. A page admits at most one such marker; other strokes fall back. Source frame staging is bounded to 84 frames and 16 MiB, with the v197 SVG staging estimate and a 128 MiB per-mask command-stream cap. These are diagnostic per-page/per-field limits, not an aggregate document budget. The SVG estimate is not an enforced emitted-byte limit.

The native reference is the retained 62×22 host-Mesa transparent alpha crop; zero outside the crop is assumed from its capture support check. The 2× case changes the draw matrix on the same stream in the same 192×192 viewport, not viewer zoom or a new Drawing sample. Reader results below are absolute alpha error sums, maximum error in parentheses:

| Scale | Source SVG / resvg | Direct marker PDF / MuPDF | Direct marker PDF / PDFium |
| --- | ---: | ---: | ---: |
| 1× | 27 (2) | 133 (5) | 60 (3) |
| 2× | 73 (2) | 466 (5) | 242 (3) |

The v197 whole-page replacement measured MuPDF 94 (4) and 341 (4), respectively. The scene-preserving marker regresses in MuPDF; the cause is not established. PDFium matches v197's totals. This is no evidence for a production profile.

A separate direct-PDF scene places a red generic stroke, the admitted black stroke, and a blue generic stroke in that order. Both PDF readers assert white paper, a red-only pixel and a blue overlap pixel at 1× and 2×; reversing the red and blue strokes changes the overlap to red. At 2×, an additional pixel changes from red to dark red when the black stroke follows red, and returns to red when red is last. These are selected-pixel order checks, not a native mixed-color scene comparison; colors of the surrounding strokes use the generic renderer. The checks do not establish clipping, arbitrary transforms, many-stroke resource behavior, saved-note or device parity. No SVG scene comparison is asserted.
