# DefaultPen tight-turn control

This fresh synthetic tool-0 size-3 path visits (8.25, 8.5), (18.5, 22.25), (20.25, 9.5), and (22, 22.5), with alternating pressures 0.25/0.5/0.75/1. The native Drawing callback and bridged stream hashes match, and Mesa reported no GL errors. The source-derived existing output-scale profile is compared with baseline public SVG and direct PDF on #fcfcfc paper against the corresponding native frame.

| Scale | resvg RGB L1 | MuPDF RGB L1 | PDFium RGB L1 |
| --- | --- | --- | --- |
| 1× | 11978 → 6603 | 15872 → 10507 | 12180 → 6820 |
| 2× | 24057 → 13368 | 41503 → 30903 | 25874 → 15256 |
| 4× | 50321 → 29482 | 114317 → 93502 | 49499 → 28464 |

Each pair is baseline → existing opt-in profile; lower error does not imply identity. This synthetic host vector bridge is not serialized-note or device evidence. The comparison builds pinned revision 8ef491537f96154099e7c0c4066c25bba0f4aa0a; the receipt records the source, capture, fixture, output and dependency hashes and reader versions. The host's full dependency closure is not pinned. This extends the existing profile evidence rather than changing its admission.

On the prepared native capture host, use a fresh output directory, then compact the capture. The retention helper's schema label reflects the reused crossing harness; normalize it to the tight-turn label before comparing:

```sh
.venv/bin/python tools/defaultpen-tight-turn/capture.py --tool 0 /tmp/defaultpen-tight-turn-fresh --case tool0
python3 tools/defaultpen-scale/retain_tool0_crossing.py /tmp/defaultpen-tight-turn-fresh | sed 's/defaultpen-tool0-crossing-native-control-v1/defaultpen-tool0-tight-turn-native-control-v1/' > /tmp/defaultpen-tight-turn-native.json
cmp /tmp/defaultpen-tight-turn-native.json tools/defaultpen-tight-turn/native.json
PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/defaultpen-tight-turn/compare.py --verify
```

Native recapture requires the pinned Samsung DSOs, Unicorn, AArch64 binutils, Mesa/EGL and the local ignored helpers listed in `tools/defaultpen-scale/hashes.json`. Generated exports and the executable are not retained; the receipt does not attest the full build dependency closure.
