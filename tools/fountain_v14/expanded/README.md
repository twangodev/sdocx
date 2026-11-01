# Expanded Fountain V14 output-scale controls

`receipt.json` compares commit `9c73bed1` with its parent against retained native host frames. Eight original saved trajectories are isolated at 1×. On each, resvg and both SVG-derived and direct PDF routes in MuPDF and PDFium have unchanged absolute RGBA error totals; equal totals do not establish pixel-identical outputs. The native captures share a Drawing instance and reset the white destination for each stroke. The separate single-source color control covers black, half-alpha blue and zero-alpha orange at 1× and 2×. At 2× the blue error decreases from 29,441 to 28,146 in resvg, 30,960 to 29,309 in MuPDF, and 30,919 to 29,744 in PDFium. Black reproduces the earlier output-scale control. At 1× all tested totals are unchanged. The native host ignores uniform color alpha for both the half-alpha blue and zero-alpha orange controls, while public vector export uses that alpha. The orange output is transparent, and its existing difference is unchanged.

Run from the repository root, using the reader versions recorded in the receipt:

```sh
PYTHONPATH=/tmp/sdocx-marker3-v133-readers .venv/bin/python tools/fountain_v14/expanded/replay.py
```

The `PYTHONPATH` above names the local PDFium installation used for this run; an equivalent Python environment with Pillow, PyMuPDF and pypdfium2, plus `resvg` on PATH, also works. The runner verifies input and probe hashes, builds both clean Git archives sequentially using the shared Cargo target, and checks every recorded metric. It uses temporary exports and removes them afterward. Reader versions are recorded, not pinned or enforced; differing versions may alter exact results. Compiler and transitive dependencies are not pinned.

These compare source-driven public vector output to white-prior native host Mesa captures. The eight independent strokes cover only 1×; the 2× color control is a single trajectory. The frame comparison does not establish native PDF, device, full-page, or arbitrary-scale fidelity. Both PDFs remain vector in the independent controls; the color control checks direct PDFs for image objects.
