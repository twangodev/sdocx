# Pencil2 V14 synthetic native controls

These controls use synthetic inputs, ARM64 emulation and host Mesa. They do not establish Android device, saved-note or general renderer parity.

## Maintained edge and projection controls

`run_control.py` shares record mixing, alpha isolation and capture handling for
the `edge`, `horizontal` and `projection` variants. `streams.py` exposes those
record operations for import. `control_adapter.py` contains the finite checked
source hooks required by the historical harness, whose draw state is local to
`capture()`. It reads the compressed base archive directly and verifies its
historical hash. The three original producer scripts and their receipts retain
their original bytes and identities.

```sh
python3 tools/pencil2_v14/run_control.py edge --check
python3 tools/pencil2_v14/run_control.py horizontal --check
python3 tools/pencil2_v14/run_control.py projection --check
PYTHONPATH=scratch/apk-analysis-runtime/python python3 tools/pencil2_v14/run_control.py horizontal --output /tmp/pencil2-horizontal-new.json
```

`--check` validates retained stream hashes, historical producer hashes and exact
generated-program hashes without running native drawing. The program pins were
established by comparing all three maintained programs byte for byte with the
historical producers' source transformations. Fresh captures require a new
output path and record the maintained schema, variant and helper hashes; they
do not replace or claim to have produced historical receipts. Native execution
of the maintained entrypoint has not been validated during this refactor.

## Mixed RGB capture

The capture uses the tracked compressed base source and pinned local native inputs. From the repository root, restore the base source to the path expected by the unchanged capture script, then run it:

```sh
mkdir -p tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38
gzip -dc tools/pencil2_v14/context-base-capture.py.gz > tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 tools/pencil2_v14/capture.py
```

The script checks the base source and native library pins. It also requires the pinned local native inputs and Python runtime used by the base capture. The generated receipt records synthetic host-Mesa results. The capture substitutes synthetic records at GPU upload and retains sparse premultiplied RGBA8 frames over several backgrounds; `mixed_rgb_tests` exercises blending, full frames, backdrops and submission ordering.

The tracked page-context, prefix and viewport controls exercise scale, projection and interpolation. Disabling Mesa's optional linear rasterizer leaves the recorded page prefixes unchanged; instrumented vertex feedback changes the result and is not an oracle for the original interpolation. The expanded-viewport frames and nominal-period translated frames match Rust; original clipped frames retain discrepancies. Changes in projection, interpolation and grain prevent attributing those differences solely to clipping.

The [Drawing test assets](../../crates/sdocx/src/ink/pencil2/test_assets/drawing/README.md) retain frames and receipts for repeated and independent strokes, origin offsets and translated public replay. Historical capture scripts may write fresh copies outside those canonical assets; compare outputs before removing them, and preserve hash-pinned scripts.

## Viewport shader diagnostics

`expanded_viewport_shader_control.py` runs all nine shader modes: `grain`, `mask`,
`mask_coord`, `mask_coord_fraction`, `mask_sample`, `mask_lod`,
`mask_texcoord_fraction`, `mask_texcoord_bits_x`, and `mask_texcoord_bits_y`.
It replaces the six separate `mask_*_control.py` scripts. All nine original
receipts are retained unchanged under `cases[mode]` in
[`pencil2-v14-viewport-shaders-native.json`](../../conformance/pencil2-v14-viewport-shaders-native.json).
Their script paths and hashes describe the historical producers. The bundle's
`capture_program_sha256` values pin the exact injected programs produced by those
scripts; the shared runner produces the same programs and input streams.

Verify all modes without native libraries, or write a fresh capture separately:

```sh
python3 tools/pencil2_v14/expanded_viewport_shader_control.py --check
PYTHONPATH=scratch/apk-analysis-runtime/python python3 tools/pencil2_v14/expanded_viewport_shader_control.py --mode mask_coord --output /tmp/pencil2-mask-coord.json
```

The runner reads the tracked compressed base directly. Native capture still
requires its pinned local libraries and emulator. These modes deliberately
instrument shader output; they are diagnostic evidence, not unmodified paint
or device parity. `--check` verifies inputs, shader bytes, and generated programs;
it does not execute native drawing.
