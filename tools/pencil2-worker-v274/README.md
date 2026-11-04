# Manager RGBA framebuffer control

After the v219 preparation, run both native controls:

```sh
python3 tools/pencil2-worker-v274/probe.py
PENCIL_SKIP_DRAW=1 python3 tools/pencil2-worker-v274/probe.py
```

The wrapper pins v273 and changes manager-supplied texture layouts to 6 and 2,
including the ninth emulated stack argument. It retains native factory, manager
map, deferred-message and attachment assertions. The factory requests
RGBA8/RGBA/UNSIGNED_BYTE. On this host the initial readback is transparent;
Run changes 322 bytes, and all 192×192 RGBA bytes match v256, bottom row first
(SHA-256 `a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e`).
Skipping Draw changes no bytes. Null-data initialization and Mesa are host-dependent.

The retired v259 Rust harness historically matched the same native frame after
row flipping. Its SVG/resvg and direct-PDF/PDFium exports matched every alpha
pixel; MuPDF differed at 80 pixels by one alpha level. The external PDF-reader measurements remain historical. The maintained
`worker_v259_geometry_probe` checks native geometry, grain-offset bits, full
canvas RGBA equality, PNG/SVG alpha and in-memory PDF construction. It does
not validate this manager lifecycle or full-color export fidelity.

The synthetic deferred message substitutes the manager canvas and explicitly
initializes the drawable. Frame equality does not establish real-page layout,
saved-page traversal, automatic queue lifecycle, object destruction or device
fidelity. See the v273 and v259 READMEs for local dependencies, matrices and
capture boundaries.
