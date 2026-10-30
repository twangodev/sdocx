# Destination-scale source diagnostic

This is a synthetic v173 control, not a saved-document renderer. `v247_scaled_source_mask` generates the curve stream and mask at an explicit 1× or 2× viewport; the latter scales stamp coordinates and extents before sampling. Its 2× screen-size and antialias assumptions are candidates, not established native inputs. `v247-scale-diagnostic.py` rasterizes the caller-supplied APK SVG source using the restricted Rust pattern rasterizer, tries both 64 and 128 texture sizes, composites the generated mask and pattern into destination-pixel SVG rectangles, exports that SVG to PDF, and compares resvg, MuPDF and PDFium against the retained v173 native frames. Captured pixels never generate a candidate. PDF is SVG-derived, not direct document PDF.

On this host the 64-texture 1× SVG/MuPDF/PDFium L1 errors are 27/499/458; 2× are 60203/60967/48773. For the 128 texture they are 7054/7439/7384 and 67725/68452/55454 respectively. SVG and MuPDF compare premultiplied RGBA; PDFium compares RGB on white. Resampling the higher resolution texture is a hypothesis, not evidence that the native capture chose 128. These discrepancies, especially at 2×, prevent document dispatch. The retained receipt contains full metrics and mask and source-generated pattern hashes; native frame hashes and capture details are in `v173-receipt.json`.

Run with the pinned local source and readers, resvg 0.45.1, Rust, and the PDF dependencies already in Cargo's offline cache:

```sh
/tmp/sdocx-v190-readers/bin/python tools/tapepen-public-drawing/v247-scale-diagnostic.py
```

`v247_scaled_source_mask` is an explicitly gated diagnostic: an ordinary Cargo test without `SDOCX_TAPE_V247_MASK_OUT` returns without generating or comparing a mask. The script sets that path and `SDOCX_TAPE_V247_SCALE` for each supported scale and asserts the resulting metrics. The comparator enforces the source hashes and reader versions in `v247-provenance.json`; the local Python path above is an example environment, not a portable prerequisite.

To verify independently supplied native capture logs in the same invocation, append `--native-logs /path/to/1x.log /path/to/2x.log`. The verifier checks the logs against the retained input, native library hashes and actual attached framebuffer bytes before the comparison. Without this option, the comparator uses retained native frames only.

The script builds a temporary PDF harness and invokes the test under the shared agent target settings. It asserts the pinned source and retained native frame hashes and all recorded metrics. Its output comparisons depend on the local reader and host versions, and do not establish Android/device parity. Native capture replay requires host-local wrappers and pinned prerequisites that are not included in this diagnostic. The retained verifier accepts independently supplied logs; this package reproduces source/export comparisons, not native capture generation.

Fresh v173 native replays on this host passed the verifier for both scales: 1× frame SHA-256 `3ac51001c441c23f82c4f78b193254b0dad7b383d275f89c37309445ce22fe99`, and 2× frame SHA-256 `dae0bf1f3552ef5184c3ba6a6de71e2888f170066c363f9ada1ea7f1e6ef60b9`. The verifier checks the explicit input and attached framebuffer against the retained capture; the generated SVG/PDF candidates were then compared with those matching retained bytes.
