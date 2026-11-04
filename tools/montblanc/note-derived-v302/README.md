# Montblanc 2.5 note-derived controls

These host Unicorn Drawing and Mesa captures use source trajectories 3 and 6 from `crates/sdocx/tests/assets/fountain-v14/scene-sources.json`, not saved Montblanc objects. Stroke 3 uses 24 evenly distributed indices 0, 14, 28, 41, 55, 69, 83, 96, 110, 124, 138, 152, 165, 179, 193, 207, 221, 234, 248, 262, 276, 289, 303, 317; stroke 6 uses every fourth index through 84 and the final index 86. Positions were translated by (-132,-135) and (-165,-132), respectively. Pressures at those indices are retained; native setter timestamps and tilt/orientation are synthetic. The metadata is replaced with Montblanc size 2.5 and opaque ARGB `ffa34127` or `ff336699`, respectively. The native capture bridges its callback stream into host paint; it is not device, serialized Montblanc, or native PDF evidence.

Further downsampling of stroke 3 uses indices 0, 40, 79, 119, 158, 198, 238, 277 and 317 for nine points, and indices 0, 29, 58, 86, 115, 144, 173, 202, 231, 259, 288 and 317. Each native Drawing stream is identical at 1× and 2×; the regression compares every method and six float words of all 31, 38, 46 and 36 respective records. Each `receipt.json` records public SVG/resvg and direct PDF/MuPDF/PDFium RGB union errors against the native frame composited over #fcfcfc. Baseline and candidate use identical inputs. Errors baseline → candidate:

| Control | resvg | MuPDF | PDFium |
| --- | --- | --- | --- |
| Stroke 3, 1× | 65,716 → 8,558 | 65,340 → 6,843 | 74,369 → 6,552 |
| Stroke 3, 2× | 296,227 → 34,879 | 296,901 → 29,759 | 301,550 → 29,052 |
| Stroke 3, 9 points, 1× | 62,457 → 8,896 | 62,757 → 7,348 | 68,154 → 7,200 |
| Stroke 3, 9 points, 2× | 283,459 → 34,842 | 283,694 → 30,019 | 285,153 → 29,422 |
| Stroke 3, 12 points, 1× | 68,933 → 9,648 | 68,970 → 7,681 | 71,923 → 7,574 |
| Stroke 3, 12 points, 2× | 311,137 → 36,776 | 311,617 → 30,149 | 314,971 → 29,324 |
| Stroke 6, 1× | 46,587 → 7,625 | 48,071 → 5,614 | 39,642 → 5,500 |
| Stroke 6, 2× | 226,985 → 30,296 | 224,959 → 24,746 | 223,700 → 24,128 |

Before prefix sharing, candidate SVGs were 372–576 KB and PDFs 119–184 KB per stroke; the existing 32,768-patch page budget remains in force. Candidate rendering is opt-in at the asserted unit or 2× output scale. Widths between 2 and 2.5 and other trajectories are extrapolations; a separate stroke-6 size-2.2 1× host check also matched all 38 native records and reduced all three errors. Lower aggregate error does not imply pixel parity. Host reader versions and native pins are recorded in the receipts and captures; the full dependency closure is not pinned.

For reproduction, each `run.py` uses `MB_CLEAN_ROOT` to locate the current checkout and requires the local native helper closure described in `../wide-v276/README.md`. `capture` takes a fresh `MB_EXPORT_WORK` and writes both native frames, input and complete streams. `build` takes a fresh `MB_EXPORT_WORK`, `MB_CAPTURE_WORK` pointing to retained `captures`, and `MB_RECEIPT_OUTPUT` naming a new file. It builds public exporters from isolated crate copies, reverting only the 2.5 width gates in the baseline. Run with the project Cargo target settings and with resvg, PyMuPDF and pypdfium2 installed. Generated SVG/PDF files are disposable. The historical receipts were produced against the isolated candidate before production admission, so source hashes differ after admission; compare frame hashes, artifact metrics and reader versions instead of byte-comparing entire receipts.

A six-point downsample of stroke 3 uses indices 0, 63, 127, 190, 254 and 317. Native Drawing emitted 25 records, retained in `note3-6/native.json`. The model returns `InvalidInput`, preserving fallback; the host paint bridge rejected an empty native vector, so no paint comparison was produced for this input.

The outer option gate still allows 3–24 points, but the three-, four- and five-point source samplers reject size >2 and therefore keep generic fallback. Before prefix sharing, the nine-point case's SVG/PDF sizes were 449,441/146,435 bytes at 1× and 431,046/122,042 bytes at 2×. For a fresh nine-point export replay from this checkout:

```sh
mkdir /tmp/mb-note9-export
MB_CLEAN_ROOT="$PWD" MB_CAPTURE_WORK="$PWD/tools/montblanc/note-derived-v302/note3-9/captures" \
MB_EXPORT_WORK=/tmp/mb-note9-export MB_RECEIPT_OUTPUT=/tmp/mb-note9-replay.json \
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 PYTHONPATH=/path/to/readers \
.venv/bin/python tools/montblanc/note-derived-v302/note3-9/run.py build
```

Nine-point SHA-256 pins: source `scene-sources.json` `bf9d7c975c710ed98612bed909eca339bbd237c60934980879eb5ac696850b41`; native input/stream `57b8d799eff752bbe5337341f4d361e4b4332b106b35d76b4cc876469ec84a30`; comparison receipt `b76a137c1706e981a6853a8932212e4e80d68c8053e0c343dfa5910279ca08db`; reproduction script `e2be63d12ffa9df3d96578e66318964e9fd5723599dbdc895ebb6628312d012c`; 1× and 2× capture files `e0f3124aebc5770297407bdaf952aae0922349a20db7a4a3eb73c2865f9f4c66` and `038a2d3b0f82b17baabbefaed1c54c56873ee32aa144994d46a1b77f47de4d96`. The captures contain the native frame bytes and input pins; the stream JSON and input sidecars support exact replay.

The vector serializer can share an exactly repeated, ordered prefix of patches through SVG `<defs>` and `<use>`, retaining both draws and the original source-over order. Consumers must support standard SVG references; exporters that flatten them retain the original patch stream. On the retained 1×/2× note-derived controls, stroke 6, stroke 3 with 9 points, and stroke 3 with 12 points shrink from 431–576 KB to 276–338 KB; the full stroke 3 control has no reusable prefix. The direct PDF bytes and resvg, MuPDF, and PDFium pixel hashes matched the prior exports for all eight cases. This is a size improvement; an eight-run local resvg timing for the nine-point 1× case showed medians of 7.52 ms before and 7.87 ms after, not a demonstrated speedup. That SVG-only change left PDF size unchanged because the PDF exporter expanded SVG references. The direct PDF exporter now reuses repeated marked solid-fill blocks as Form XObjects after checking their resolved geometry and paint. The eight retained controls keep the same resvg, MuPDF, and PDFium RGB hashes: repeated cases save 24,867–40,715 PDF bytes (17–25%), while the full stroke 3 control remains unchanged. See `pdf-reuse-receipt.json`; this is a size result, with no speed claim. The exporter falls back for internal effects or transforms, native ink hooks, unmatched content, and bounded-scan limits. Historical comparisons found matching flat/shared pixels for mixed, offscreen-first, and masked/multiply fixtures in both PDF readers at 1×/2×. The receipt’s export commands and source hashes record that historical run; the optional exporter and reader comparator have been retired. Current `cargo test -p sdocx --features pdf,serde reuse_tests --lib` checks form reuse and constructs those PDFs in memory; it does not repeat the external reader pixel comparisons.
