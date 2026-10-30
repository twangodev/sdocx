# DefaultPen tool-0 crossing evidence

A fresh synthetic size-3 tool-0 path crosses itself at (10.5, 8.25),
(42.25, 23.5), (10.5, 23.5), (42.25, 8.25), with the harness's alternating
pressures 0.25/0.5/0.75/1. Native Drawing callback streams match the Rust
fixture; the host Mesa captures have zero reported GL errors. The existing
output-scale profile reduces RGB L1 error against the native frame on #fcfcfc:

| Scale | SVG | MuPDF | PDFium |
| --- | --- | --- | --- |
| 1× | 26036 → 15035 | 30046 → 18821 | 26776 → 15503 |
| 2× | 50101 → 27779 | 68804 → 46406 | 54187 → 31817 |
| 4× | 106881 → 62764 | 169931 → 125196 | 106023 → 62102 |

The [native fixture](tool0-crossing-native.json) and [export receipt](tool0-crossing-export.json)
retain hashes and reader versions. The fixture retains both the Drawing-phase callback hash and bridged stream hash;
the comparison checks their equality and pins its capture and comparator sources.
The export comparison uses clean source from pinned revision
`8ef491537f96154099e7c0c4066c25bba0f4aa0a`, its locked Cargo dependencies,
and exact tool inputs referenced by its Rust tests. The receipt records that
revision and tool dependency hashes. Native recapture requires the prepared
host: pinned Samsung DSOs, Unicorn, AArch64 binutils, Mesa/EGL, and the
local ignored helpers listed in `hashes.json`; the complete host dependency
closure is not pinned. The capture rejects an existing output directory.

The comparison uses an explicit host vector bridge and a synthetic input; it does not
establish saved-note or device parity. Lower error does not imply pixel identity.
No production change follows.

```sh
.venv/bin/python tools/defaultpen-scale/tool0_crossing_capture.py --tool 0 /tmp/defaultpen-crossing-fresh --case tool0
python3 tools/defaultpen-scale/retain_tool0_crossing.py /tmp/defaultpen-crossing-fresh > /tmp/defaultpen-crossing-native.json
cmp /tmp/defaultpen-crossing-native.json tools/defaultpen-scale/tool0-crossing-native.json
PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/defaultpen-scale/compare_tool0_crossing.py --verify
```
