# DefaultPen subthreshold loop control

This synthetic tool-0 size-3 path starts with a subthreshold movement, then returns to its fractional starting point: (8.125, 8.375), (9, 9.125), (10.125, 8.375), (8.125, 8.375). Pressures are 0.25, 0.5, 0.75, and 1. Native Drawing callback and bridged stream hashes match, and Mesa reported no GL errors. Public SVG and direct PDF exports from pinned revision 8ef491537f96154099e7c0c4066c25bba0f4aa0a are compared on #fcfcfc paper against native host frames:

| Scale | resvg RGB L1 | MuPDF RGB L1 | PDFium RGB L1 |
| --- | --- | --- | --- |
| 1× | 2735 → 1459 | 6922 → 5552 | 2866 → 1547 |
| 2× | 5229 → 2761 | 21985 → 19460 | 5672 → 3157 |
| 4× | 12447 → 7254 | 78159 → 72725 | 12358 → 7102 |

Each pair is baseline → existing scale-aware profile. These synthetic host captures use an explicit vector bridge and do not establish serialized-note or device parity; lower error does not imply pixel identity. The receipt records hashes, source revision and reader versions. The host's full dependency closure is not pinned. This adds a control, not a change to profile admission.

On the prepared native capture host, use a fresh output directory:

```sh
.venv/bin/python tools/defaultpen-subthreshold-loop/capture.py --tool 0 /tmp/defaultpen-subthreshold-loop-fresh --case tool0
python3 tools/defaultpen-scale/retain_tool0_crossing.py /tmp/defaultpen-subthreshold-loop-fresh | sed 's/defaultpen-tool0-crossing-native-control-v1/defaultpen-tool0-subthreshold-loop-native-control-v1/' > /tmp/defaultpen-subthreshold-loop-native.json
cmp /tmp/defaultpen-subthreshold-loop-native.json tools/defaultpen-subthreshold-loop/native.json
PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/defaultpen-subthreshold-loop/compare.py --verify
```

Native recapture needs the pinned Samsung DSOs, Unicorn, AArch64 binutils, Mesa/EGL and the local ignored helpers identified in `tools/defaultpen-scale/hashes.json`. Generated exports and executable are not retained.
