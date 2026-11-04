# SimpleBrush 2× point-count controls

The experimental public-export generator, comparators and replay callers have been retired. Export hashes and external SVG/PDF reader measurements below are historical observations, not current reproduction instructions or full source/build attestations. Native capture commands, inputs and receipts remain retained.

The synthetic Drawing captures use the project-local native and Mesa inputs described in `../simplebrush-scale/README.md`. They do not establish device or saved-note parity. Run from the repository root, supplying a fresh directory:

```sh
mkdir -p /tmp/simplebrush-points-run
POINT_COUNT=16 python3 tools/simplebrush-points/capture.py fraction12 /tmp/simplebrush-points-run/native.json
```

Counts 2, 4, 7, 16 and 32 are admitted controls at size 12 and tool 2. The current opt-in gate admits 2–256 points at sizes 8–20 for tools 0, 2 and 3, with separate three-point sizes 3–32; this capture set supports only the explicitly listed cases. Other combinations extrapolate from those controls. Two additional 32-point controls use `VARIANT=reverse` or `VARIANT=alternate` with `tools/simplebrush-points/capture.py`. The size-12, tool-2 controls do not establish general PDF improvement: for the original 32-point control, matched PDF RGB error is 38,344 for 2× versus 38,161 for 1×; the reversed and alternate controls yield 38,745 versus 38,747, and 39,368 versus 44,867. Generic errors are 676,814, 787,174 and 889,751 respectively. The other point counts, sizes and tools extrapolate from these controls. The native wrapper checks the source, paint script and DSO pins inherited from the existing 2× receipt and checks each resulting full-frame and geometry hash. Other dynamically loaded inputs are not independently pinned. The historical comparator required resvg 0.45.1 and MuPDF 1.28.2; it checked retained native crop hashes and compared against matched blank-page PDF rasterization. Rasterizer differences can affect metrics.

To reproduce either added 32-point variant, use a fresh directory and set the
desired variant on capture:

```sh
mkdir -p /tmp/simplebrush-32-variant
VARIANT=alternate POINT_COUNT=32 python3 tools/simplebrush-points/capture.py fraction12 /tmp/simplebrush-32-variant/native.json
```

## Longer controls

Synthetic 33-point size-8 and 64-point size-20 controls, including reversed 64-point input, use 0.75-unit horizontal spacing and the fractional 2× matrix. Their [tool-2 receipt](../../conformance/simplebrush-long-controls.json) and [tool-0](../../conformance/simplebrush-tool0-long-identity.json) and [tool-3](../../conformance/simplebrush-tool3-long-identity.json) receipts record matching native geometry and full-frame hashes. Other inputs extrapolate; these host controls do not establish saved-note or device parity.

```sh
mkdir -p /tmp/simplebrush-long
POINT_COUNT=33 PROBE_SIZE=8 python3 tools/simplebrush-points/capture.py fraction12 /tmp/simplebrush-long/native.json --tool 2
```

For the 64-point control use `POINT_COUNT=64 PROBE_SIZE=20`; add `VARIANT=reverse` for the reverse case. Select `--tool 0` or `--tool 3` for their matching controls. The export generators and comparators have been retired; their reader measurements are historical.

The tool-0 alternate 32-point size-20 capture uses the [retained receipt](tool0-alternate-size20-receipt.json): set `VARIANT=alternate POINT_COUNT=32 PROBE_SIZE=20` and pass `--tool 0`. Its measurements are synthetic host observations.
