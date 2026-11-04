# Historical 25-point Montblanc control

One synthetic size-1 path used x coordinates `0,4,…,96`, y coordinates `0,2,4,6,8,6,4,2,0,-2,-4,-6,-8,-6,-4,-2,0,2,4,6,8,6,4,2,0`, constant pressure 0.5 and ARGB `ff336699`. The adapter supplied points and pressures without independent native getter readback. The retained capture and stderr contain host-Mesa paint and 100 native callbacks with six binary32 lanes each. This is not saved-note, device, lifecycle or native-PDF parity.

Run `python3 tools/montblanc/twentyfive-investigation/clean-fixture.py` to check both compressed evidence hashes, all callback records and the complete 256×256 native frame. Despite its historical name, the script now performs native-only verification and writes no experimental source. The exact adapter recipe remains in that script.

Native acquisition remains available through `tools/montblanc/narrow-probe/capture.py` and its preserved native adapters. Supply the recipe above through `MB_CASE` and use a fresh output path, following [the native capture prerequisites](../narrow-probe/README.md). Recapture requires local native binaries, Unicorn and host Mesa; retained capture metadata records the original environment. It is separate from verifying retained evidence.

The experimental sampler equality test and exporter/reader replay have been retired. The historical patched sampler matched the native callbacks, but maintained production still rejects 25 points; `longer_profiles_preserve_point_count_boundaries` checks that boundary. There is no claim that a maintained test reproduces the retired positive 25-point sampler experiment.

Historical public unit-scale results used native transparency mathematically composed over #fcfcfc. Values below are `[native support, export support, intersection, union, union RGB L1]`, with support defined by RGB difference >2 from 252.

| Export | Candidate | Fallback |
| --- | --- | --- |
| resvg SVG | [556, 636, 556, 636, 24716] | [556, 196, 170, 582, 159662] |
| MuPDF direct PDF | [556, 635, 556, 635, 18318] | [556, 192, 166, 582, 160304] |
| PDFium direct PDF | [556, 621, 556, 621, 17755] | [556, 196, 170, 582, 144345] |

Candidate SVG/PDF sizes were 966260/248025 bytes versus 2946/1965 for fallback. Overcoverage and this single path did not justify widening admission. The clean archived revision `a03df32d0eea4ea119622c1f02fd2e0ffe8ad81b` plus the historical patch reproduced the dirty-tree run's stream equality, four export hashes and nine metrics. `experimental-crate-sha256.txt` and original capture metadata are historical provenance, not active verification manifests or a complete build closure. The original README at `ad86dc9fb32270eef6254c814cb4a9d44c2eb270` retains the historical script, patch and export hashes. Readers were resvg 0.45.1, PyMuPDF 1.28.2, PDFium 150.0.7869.0 and Pillow 12.1.1.

Historical subdivision controls sampled alpha at patch centroids; each source triangle produced up to n² paths. Lower subdivision reduced size but worsened error; higher subdivision reduced excess support but increased RGB error and size:

| n | SVG/PDF bytes | resvg | MuPDF | PDFium |
| --- | --- | --- | --- | --- |
| 4 | 250288 / 75523 | [556, 653, 556, 653, 33430] | [556, 666, 556, 666, 29557] | [556, 655, 556, 655, 29272] |
| 8 | 966260 / 248025 | [556, 636, 556, 636, 24716] | [556, 635, 556, 635, 18318] | [556, 621, 556, 621, 17755] |
| 16 | 3895592 / 833213 | [556, 601, 556, 601, 25086] | [556, 590, 552, 594, 22110] | [556, 579, 551, 584, 22275] |

Coordinate quantization rounded emitted patch vertices while leaving sampler and centroid alpha unchanged:

| Decimal places | SVG/PDF bytes | resvg | MuPDF | PDFium |
| --- | --- | --- | --- | --- |
| 3 | 848714 / 231671 | [556, 636, 556, 636, 24726] | [556, 634, 556, 634, 18294] | [556, 621, 556, 621, 17765] |
| 2 | 790993 / 220730 | [556, 636, 556, 636, 24728] | [556, 634, 556, 634, 18267] | [556, 621, 556, 621, 17780] |

The two rounding controls saved 12.2%/18.1% of SVG bytes and 6.6%/11.0% of PDF bytes with small unit-scale metric changes. These lossy controls did not establish arbitrary-transform, zoom or input error bounds; no production rounding was introduced.
