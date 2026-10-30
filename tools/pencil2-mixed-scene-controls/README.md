# Pencil2 mixed Drawing scene controls

`capture.py` extends the archived translated Drawing adapter in `../pencil2-drawing-capture-closure`. It constructs two separate native ObjectStroke inputs, draws them in order onto the same 192 × 192 host Mesa framebuffer, and retains both frame prefixes. The second ObjectStroke causes new source geometry to be generated but the native Drawing reuses the same renderer. This is synthetic ARM64 emulation with the existing callback-queue intervention, explicit native Update and Draw, not a device or saved-note capture.

The first input is the public four-point pressure ramp, size 2, ARGB 80336699; the second has points (0,2), (10,2), (20,4), (30,12), pressures .9/.7/.5/.3, size 10, ARGB c0cc3344. Both have tilt .4, zero orientation, timestamps 10/20/30/40 and V14 settings. Source arrays and object getter values for the second input are in each receipt; the original mixed capture receipt and native buffers are retained in `../../crates/sdocx/src/ink/pencil2/test_assets/drawing/independent-drawing`. The archived first-input construction is in `../pencil2-drawing-capture-closure/sources/first_point_independent_capture.py`.

| Prior RGBA | Draw matrix scale | Grain offsets, in draw order | Pixels changed by second draw |
| --- | ---: | --- | ---: |
| transparent | 1 | (6,9), (12,13.999998) | 357 |
| white | 1 | (6,9), (12,13.999998) | 357 |
| blue (64,128,191,255) | 2 | (6,9), (12,16.0000019) | 1426 |
| white | 2 | (6,9), (12,16.0000019) | 1426 |

Translation is (6,9) for the first draw and (12,12) for the second. At 2x the draw matrix scales existing native geometry, not a new 2x sampling pass. All captured draw and update GL errors are zero; all four second geometry streams hash to `ab8c231108800d1a9437783f27d12c57e59c4d81a00538268343f662a1c64389`. Frame blobs contain bottom-up RGBA8; frame hashes and uniform traces are in the receipts. These controls do not themselves establish vector-export agreement. In particular, no source-derived vector output is compared here.

Run from the repository root with the pinned ARM64 libraries and runtime. The source closure is reconstructed in a disposable `/tmp` workspace; it never writes historical checkout paths. By default native inputs are read from `scratch/apk-analysis-native/arm64-v8a` and `scratch/apk-analysis-runtime`; set `PENCIL_SCENE_NATIVE_DIR` and `PENCIL_SCENE_RUNTIME_DIR` to use other locations:

```sh
PENCIL_SCENE_OUTPUT=/tmp/pencil-scene-new PENCIL_SCENE_BACKDROP=white PENCIL_SCENE_SCALE=1 python3 tools/pencil2-mixed-scene-controls/capture.py
```

Supported backdrops are `transparent`, `white`, and `blue`, and scales are `1` and `2`. Each run requires a new output directory and writes its receipt, frame files and identical second geometry buffer there. The retained receipts identify the pre-hardening capture script used for the original runs; subsequent runs record the current script hash. The two scale-2 captures apply the matrix scale in the capture script; the `matrices` metadata embedded in the original upstream probe is not used as evidence of the modified matrix. These controls retain compact native outputs, not disposable rendering intermediates.

A white 2x recapture using the local native inputs matched both retained frame hashes, the second geometry hash, and uniform traces; its GL errors were zero. The original receipt predates explicit backdrop, scale, and matrix metadata, which the current runner records from its selected inputs.

The runner rejects optimized Python and invalid arguments before creating output, pins the archived base, patch, transitive source archive and GLES helper hashes, checks capture completion and GL status, and verifies frame lengths and receipt hashes before creating a new output directory. These checks do not pin or independently validate all transitive native DSOs, emulator inputs, or host graphics drivers. The output hashes verify consistency with the capture receipt; they are not an assertion that every environment reproduces a pinned frame.
