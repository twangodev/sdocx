# SimpleBrush 2× point-count controls

The experimental public-export generator, comparators and replay callers have been retired. Export hashes and external SVG/PDF reader measurements below are historical observations, not current reproduction instructions or full source/build attestations. Native capture commands, inputs and receipts remain retained.

The synthetic Drawing captures use the project-local native and Mesa inputs described in `../simplebrush-scale/README.md`. They do not establish device or saved-note parity. Run from the repository root, supplying a fresh directory:

```sh
mkdir -p /tmp/simplebrush-points-run
POINT_COUNT=16 python3 tools/simplebrush-points/capture.py fraction12 /tmp/simplebrush-points-run/native.json
```

Counts 2, 4, 7, 16 and 32 are admitted controls at size 12 and tool 2. The combined gate admits 2–32 points at sizes 8–20 for tools 2 and 3, and 2–16 points for tool 0, while the separate three-point tool-2 size controls admit sizes 3–32. Other combinations extrapolate from those controls. Two additional 32-point controls use `VARIANT=reverse` or `VARIANT=alternate` with `tools/simplebrush-points/capture.py`. The size-12, tool-2 controls do not establish general PDF improvement: for the original 32-point control, matched PDF RGB error is 38,344 for 2× versus 38,161 for 1×; the reversed and alternate controls yield 38,745 versus 38,747, and 39,368 versus 44,867. Generic errors are 676,814, 787,174 and 889,751 respectively. The other point counts, sizes and tools extrapolate from these controls. The native wrapper checks the source, paint script and DSO pins inherited from the existing 2× receipt and checks each resulting full-frame and geometry hash. Other dynamically loaded inputs are not independently pinned. The historical comparator required resvg 0.45.1 and MuPDF 1.28.2; it checks retained native crop hashes and compares against matched blank-page PDF rasterization. Rasterizer differences can affect metrics.

To reproduce either added 32-point variant, use a fresh directory and set the
desired variant on capture:

```sh
mkdir -p /tmp/simplebrush-32-variant
VARIANT=alternate POINT_COUNT=32 python3 tools/simplebrush-points/capture.py fraction12 /tmp/simplebrush-32-variant/native.json
```
