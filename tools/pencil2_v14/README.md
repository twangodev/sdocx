# Pencil2 V14 synthetic native controls

These controls use synthetic inputs, ARM64 emulation and host Mesa. They do not establish Android device, saved-note or general renderer parity.

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

Some historical shader, edge and export controls are not packaged for reproduction. Their retained measurements are bounded diagnostics, not evidence of a general correction. Historical synthetic tolerance receipts for 0.0, 0.2, 0.47816234827041626, 1.0 and 5.0 are preserved byte-for-byte in [`tolerance-receipts.zip`](../pencil2-serialized-bridge/tolerance-receipts.zip). Use `unzip -l` to list the original names or `unzip` to extract them; the capture script writes separate JSON files for fresh runs.
