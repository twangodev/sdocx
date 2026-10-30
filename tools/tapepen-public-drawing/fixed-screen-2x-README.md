# TapePen fixed-display 2× control

This varies the v173 2× host-Mesa Drawing control by returning 256×256 from the native IDisplay getters while retaining the 512×512 target bitmap and injected 2× model matrix. The wrapper checks the injected matrix and the native screen resolution calls. The wrapper corrects inherited profile, context, DrawObject and scope metadata to reflect the injected values; the native resolution-call assertions and matrix readback check the intervention. No saved-document or device parity is implied.

The fixed-display frame SHA-256 is `51c98e15dec23b0d7ca62e9646f4535584c2ab062c3331f26f911431aa36f73a`. Its absolute RGBA difference against the original v173 2× frame is 67,714. Downsampling to 256×256 and comparing against v173's native 1× frame yields absolute RGBA errors of 22,387 using nearest and 19,717 using box; the original 2× control yields 27,195 and 26,065 respectively. Pillow's RGBA resizing semantics apply. This isolates a display-size effect in this capture and leaves a substantial difference with matrix scale still present. The experiments do not establish its cause or a vector solution.

Reproduce in the original investigation workspace with the v173 pinned prerequisites. This compact control is not a standalone replay kit: `capture_candidate.py`, `preflight_candidate.py`, `tapepen_canvas_native.py`, `preflight.json`, `candidate.json`, `phase-control.patch`, and `rotation-v113.patch` must be available beside the wrapper. The wrapper checks their exact SHA-256 hashes before execution. The preflight additionally checks local native libraries, the tape SVG, and helper sources under `scratch/` and `tmp/vector-parity-plan/`; these prerequisites are not included in this control. Its existing harness expects the checkout at `/home/jding/RustroverProjects/sdocx`.

Run:

```sh
UV_CACHE_DIR=/tmp/sdocx-uv-cache uv run --offline --with unicorn==2.1.4 python tools/tapepen-public-drawing/fixed-screen-2x.py --run --shape64 --size12 > /tmp/tape-fixed-screen.log
```

Verify the captured input, library hashes, profile, attached framebuffer, and retained frame bytes with `python3 tools/tapepen-public-drawing/verify-fixed-screen-2x.py /tmp/tape-fixed-screen.log`. The native resolution-call assertion is performed during capture. The retained compressed frame contains the unmodified 512×512 RGBA readback; the full log is transient.
