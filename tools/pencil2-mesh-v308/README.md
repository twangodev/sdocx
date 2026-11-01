# Pencil2 source-cell run coalescing control

This diagnostic changes v307's Type 4 soft-mask construction: for each row of source-cell intersections, it merges consecutive cells with the same binary point gate and grain value. It still evaluates the nonlinear stamp field at the resulting rectangle corners, uses two triangles per rectangle, and composites stamps in order. The merge is source-space dependent, not an output-pixel tiling; it changes the linear approximation, so equality of source texture values does not establish a bound on the field error. The default uses the retained first native prefix as an image backdrop to isolate the second stroke. `--full` instead renders both native geometry streams in order over a solid background; it checks that the PDF has no image XObjects. Source asset, captured geometry and native prefix hashes are verified by the script. See v307 and the mixed-scene controls for capture and model limitations.

With PyMuPDF 1.28.2 and PDFium 156.0.8076.0, RGB error sum / maximum / differing channels against the captured complete native frame:

| Case | MuPDF | PDFium | Triangles | PDF bytes |
|---|---:|---:|---:|---:|
| White 1× | 10,616 / 72 / 1,079 | 11,522 / 71 / 1,137 | 273,206 | 1,196,012 |
| White 2× | 39,126 / 92 / 4,153 | 40,383 / 93 / 4,303 | 273,208 | 1,217,140 |
| Blue 2× | 41,744 / 66 / 4,295 | 43,120 / 66 / 4,365 | 273,208 | 1,216,906 |

The v307 mesh used approximately 1.414 million triangles and 5.0 MB; both its readers had larger error sums for these cases. The maximum error remains large. This is not a public exporter and does not establish a safe approximation tolerance or budget for arbitrary documents, transforms or scales.

`pixel_probe.py` independently evaluates the same source-derived point gate, grain and approximate floating-point stamp field at framebuffer pixel centers with ordered compositing over the native first prefix. It uses native pixels only for the backdrop and comparison, and is a diagnostic, not an export strategy. Its error sum / maximum / differing RGB channels is 2,705 / 26 / 764, 10,361 / 24 / 2,992, and 17,299 / 24 / 3,866 for the respective cases. This is consistent with a substantial coverage or sampling contribution to mesh residuals; it does not isolate that from interpolation, reader composition, rounding or the model's precision differences.

`public_compare.py` measures current generic public output for complete scenes generated from the corresponding two source inputs. At 2× the generic scene uses the original points and widths on a 96×96 page with half translations, then renders that page at 2×; this matches the output scale and translations applied to the native geometry. The generic renderer and native generate different geometry, so this is a whole-route comparison, not a pure transport-only comparison. SVG rendered with resvg and direct PDF rendered with MuPDF and PDFium had these RGB error sums / maxima against the same complete native targets:

| Case | Generic SVG resvg | Generic PDF MuPDF | Generic PDF PDFium | SVG / PDF bytes |
|---|---:|---:|---:|---:|
| White 1× | 50,427 / 204 | 50,304 / 204 | 54,482 / 204 | 845 / 1,850 |
| White 2× | 212,057 / 204 | 209,505 / 204 | 211,927 / 204 | 826 / 1,850 |
| Blue 2× | 159,126 / 140 | 157,492 / 140 | 158,472 / 140 | 826 / 1,863 |

The generic outputs represent both strokes and are much smaller. The prefix mesh numbers above use the native first stroke as input; the full-scene captured-geometry results below use both captured geometry streams and no native pixels as output inputs. Native captures are synthetic ARM64 emulation with host Mesa and a synthetic queue, not device or saved-note evidence. Generic results reflect the mutable checkout at the time of the run.

Reproduce from the repository root (the scripts use the installed reader libraries and do not retain PDFs):

```sh
PYTHONPATH=/tmp/sdocx-marker3-v133-readers .venv/bin/python tools/pencil2-mesh-v308/scene.py --full
PYTHONPATH=/tmp/sdocx-marker3-v133-readers .venv/bin/python tools/pencil2-mesh-v308/pixel_probe.py
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 cargo build --offline --locked --manifest-path tools/pencil2-mesh-v308/Cargo.toml --features export-pdf
PYTHONPATH=/tmp/sdocx-marker3-v133-readers .venv/bin/python tools/pencil2-mesh-v308/public_compare.py
```

Next work needs a bounded field approximation and explicit resource accounting, source-derived complete-scene output, transformed and repeated-boundary tests, and both reader comparisons. SVG has no equivalent portable mesh shading. No Rust exporter change is justified by these controls alone.

## Full-scene captured-geometry mesh and size experiment

Both geometry buffers are verified against their retained native capture hashes; the first has 84 records, the second 82. With stride 1, every point-gate texel boundary is represented before run merging. PDF rendering uses the same native-frame targets and reader settings as the table above. The two complete-scene routes use the same source strokes, transforms, and backdrop, but generic reconstructs geometry while this prototype takes captured native descriptors.

| Case | MuPDF sum / max / channels | PDFium sum / max / channels | Triangles | PDF bytes |
|---|---:|---:|---:|---:|
| White 1× | 12,947 / 72 / 1,182 | 14,264 / 71 / 1,239 | 360,670 | 1,688,899 |
| White 2× | 43,676 / 92 / 4,230 | 45,620 / 93 / 4,361 | 360,672 | 1,715,588 |
| Blue 2× | 43,579 / 66 / 4,341 | 45,357 / 66 / 4,399 | 360,672 | 1,715,609 |

`PENCIL_MESH_POINT_STRIDE=4` or `20` omits intermediate point-gate texel boundaries and samples the gate at the midpoint of each resulting cell. Grain boundaries remain. This is an intentionally uncontrolled lossy approximation and may discard fine features. For stride 4, sizes are 665,321 / 685,430 / 685,451 bytes and triangle counts 127,582 / 127,598 / 127,598; MuPDF sums/maxima are 12,778/71, 43,330/91, 43,254/66 and PDFium 14,148/75, 45,209/91, 44,861/65. For stride 20, sizes are 390,774 / 405,623 / 405,644 bytes and triangle counts 65,866 / 65,864 / 65,864; MuPDF sums/maxima are 12,160/72, 41,797/85, 42,386/66 and PDFium 13,567/77, 43,296/86, 43,605/67. Lower summed error for these particular scenes is not an error guarantee or evidence that coarse gating generalizes.

The full-scene result still uses native-captured descriptors. A separate note-derived input and Rust-generated descriptors have not been evaluated by this mesh, so the experiment does not yet justify a reusable implementation.

The compact [receipt](receipt.json) records source script and geometry hashes, native target hashes, full-scene measurements and limitations. `/tmp/sdocx-marker3-v133-readers` is a pre-existing local Python reader dependency directory, not a retained project artifact; the command requires equivalent installed PyMuPDF, pypdfium2 and Pillow if that directory is unavailable. The public comparator invokes the shared target binary `/tmp/sdocx-agent-target/debug/pencil2-mesh-public-control`, created by the preceding build command. It also needs `resvg` on PATH. The Rust build depends on the current checkout and its offline Cargo cache, not a pinned source snapshot. Generated PDFs and SVGs live only in temporary directories or memory and are removed when each run finishes.

Independent validation replayed `scene.py --full`, `pixel_probe.py`, and the locked offline public build/comparison against a clean archive of `b15cdb7871bfbccb89e0046fd11a86dfc462981c` plus this diagnostic directory. All listed full-scene, pixel-probe and generic comparison metrics matched. The scripts read tracked geometry, source texture assets, native frames and receipts; no v307 helper is required. Native recapture is separate: [mixed-scene capture](../pencil2-mixed-scene-controls/README.md) uses the tracked [capture source closure](../pencil2-drawing-capture-closure/README.md) and requires local ARM64 libraries, Unicorn/binutils runtime and host graphics dependencies described there. These external native inputs are unnecessary for replaying the retained fixtures.
