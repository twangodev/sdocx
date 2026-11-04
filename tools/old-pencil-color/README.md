# Legacy Pencil color controls

These four synthetic host-Mesa observations use the pinned original V17 capture source in `capture-v17.py.gz` and `make_controls.py` to create the four exact captured variants. `queue_contract.py` is the exact helper checked by the capture. `hashes.json` pins packaged sources and fixtures; `receipt.json` records draw and frame identities. The source generator's hashes match the four actually executed files at the original repository root `/home/jding/RustroverProjects/sdocx`.

The capture uses local files which are **not provided in this package**: APK and extracted native DSOs, Unicorn 2.1.4, the project-local Drawing runtime and its transitive helpers, scratch BrushPen evidence, GNU binutils, host libc, and EGL/GLES Mesa. `local-inputs.json` inventories and hashes available files but is not proven a complete dependency closure. The capture's own preflight checks its pinned direct files. The native host was glibc 2.41, libc SHA-256 `9792e3cbb541c8f44c7acf5f14f4022ea62998ecc787d326bed4d8b6547dfd92`, Mesa 25.0.7-2+deb13u1, llvmpipe LLVM 19.1.7, EGL 1.5 and OpenGL ES 3.2. Exact host-library binary identities and a clean-checkout reproduction are not established. In particular, the ignored runtime helper tree remains a local prerequisite, not a committed or packaged dependency; do not represent these controls as independently reproducible until it is packaged and its transitive closure verified.

From the original checkout, with all local prerequisites present, run:

```sh
python3 tools/old-pencil-color/make_controls.py "$PWD" /tmp/sdocx-pencil-repro
PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance python3 /tmp/sdocx-pencil-repro/red.py --capture --initial-seed 1 --output /tmp/sdocx-pencil-repro/red.json > /tmp/sdocx-pencil-repro/red.stdout
PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance python3 /tmp/sdocx-pencil-repro/alpha.py --capture --initial-seed 1 --output /tmp/sdocx-pencil-repro/alpha.json > /tmp/sdocx-pencil-repro/alpha.stdout
PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance python3 /tmp/sdocx-pencil-repro/backdrop.py --capture --initial-seed 1 --output /tmp/sdocx-pencil-repro/backdrop.json > /tmp/sdocx-pencil-repro/backdrop.stdout
PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance python3 /tmp/sdocx-pencil-repro/varied.py --capture --initial-seed 1 --output /tmp/sdocx-pencil-repro/varied.json > /tmp/sdocx-pencil-repro/varied.stdout
python3 tools/old-pencil-color/extract_verify.py /tmp/sdocx-pencil-repro conformance/old-pencil-color
```

`extract_verify.py` compares all raw pre/post pixels and VBO bytes against the packaged fixtures, after changing OpenGL bottom-origin rows to top-origin; compression representation is not used as comparison evidence. The red and backdrop VBOs were byte-identical to the retained alpha VBO, which the extractor uses for those cases. The half-alpha backdrop cases inject host clears immediately before the draw and are not device or saved-note captures. Live clock values in full JSON receipts need not match. The isolated source patch tests the 4 full frames against all 12 model profiles without the untracked shared compositing helper.

The alpha control shares its byte-identical pre-draw frame with `red-pre.rgba.zlib`.
The extractor retains separate per-case native hash checks; `hashes.json` pins the
canonical file and updated extractor. Capture receipts remain unchanged.
