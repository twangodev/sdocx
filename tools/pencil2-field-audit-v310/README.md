# Pencil2 source-field interpolation control

Run `.venv/bin/python tools/pencil2-field-audit-v310/audit.py` and `PENCIL_SCALE=2 .venv/bin/python tools/pencil2-field-audit-v310/audit.py`. Both assert the reported values. The script verifies both texture hashes, the 84-record first native descriptor, and each retained white native first-frame hash. Its 1× and 2× source transforms and phase match the v308 first-prefix controls. It compares the exact floating-point fragment-center field with source-cell triangle interpolation, both unmerged and merged into horizontal runs like v309. Each holds the center's nearest texture selection and rounded source-over sequence fixed; this isolates field interpolation from texture-boundary selection. PDF coordinate and alpha quantization, subpixel coverage, cell clipping, the mesh's negligible-alpha cutoff, and reader behavior are excluded. The equations are simulated, not executed by the GPU.

RGB absolute error sum / max / differing channels:

| Scale | Exact vs native | Unmerged vs native | Merged vs native | Unmerged vs exact | Merged vs exact |
|---|---:|---:|---:|---:|---:|
| 1× | 344 / 29 / 139 | 344 / 29 / 139 | 349 / 29 / 140 | 0 / 0 / 0 | 7 / 3 / 3 |
| 2× | 767 / 4 / 607 | 767 / 4 / 606 | 1451 / 9 / 800 | 10 / 1 / 10 | 1074 / 8 / 655 |

There are 459 and 2032 nonzero point-and-grain fragment samples at 1× and 2×. At 1× pixel centers lie on grain-cell corners, so the unmerged field agrees there. Coalescing is also small here, though it has a material effect at 2×. The 1× v309 first-prefix PDF's MuPDF error of 4505 / 90 / 301 is therefore not explained by field interpolation or coalescing alone under these controlled assumptions. Texture selection at boundaries, quantization, rasterization and reader transport remain entangled; these numbers do not isolate a particular reader defect or provide an error bound. Native host and capture limitations from v309 apply. No production change follows from this control.

## Full PDF reader control

The retired full-PDF wrapper disabled horizontal grain-run merging in the v308 two-stroke approximation. It checked triangle counts, absence of image objects and reader error triples; these described the experimental PDF, not the public exporter. Historical readers were MuPDF 1.28.2 and PDFium 156.0.8076.0. The native capture and host limitations documented by v308 apply.

| Backdrop, scale | Merge | Triangles | PDF bytes | MuPDF RGB error sum / max / channels | PDFium RGB error sum / max / channels |
|---|---|---:|---:|---:|---:|
| White, 1× | yes | 360,670 | 1,688,899 | 12,947 / 72 / 1,182 | 14,264 / 71 / 1,239 |
| White, 1× | no | 2,596,148 | 10,441,921 | 13,299 / 77 / 1,196 | 14,868 / 78 / 1,234 |
| White, 2× | yes | 360,672 | 1,715,588 | 43,676 / 92 / 4,230 | 45,620 / 93 / 4,361 |
| White, 2× | no | 2,596,136 | 10,431,637 | 44,931 / 94 / 4,236 | 47,652 / 95 / 4,352 |
| Blue, 2× | yes | 360,672 | 1,715,609 | 43,579 / 66 / 4,341 | 45,357 / 66 / 4,399 |
| Blue, 2× | no | 2,596,136 | 10,431,658 | 43,963 / 66 / 4,351 | 46,448 / 66 / 4,409 |

Disabling merging increases the PDF size about sixfold and the triangles sevenfold while increasing total error in these reader comparisons. Both complete successfully within the explicit 16 MiB and 3 million triangle output caps; this is not a measurement of peak memory or a guarantee that larger scenes are safe. The reader results include quantization and coverage and do not isolate their individual contributions. The control does not establish a scalable production vector approach or justify changing the public exporter.

The retained `fixtures.py` checks compressed native frame and receipt hashes, all six decompressed 192×192 frame hashes, both texture assets and the 82/84-record geometry streams. Run it directly with Python, or through the retained field audit commands above. It does not load a PDF reader.

The historical field and PDF replay used Python with Pillow 12.1.1, PyMuPDF 1.28.2 and pypdfium2 5.14.0 / PDFium 156.0.8076.0. A clean-source replay from `9c4e475c743ff609e26fef7f81f3ebf0ca287b6e` plus the diagnostic directory matched the assertions and reported PDF sizes. Size figures describe serialized outputs, not geometry correctness or memory bounds; no generated PDFs were retained.
