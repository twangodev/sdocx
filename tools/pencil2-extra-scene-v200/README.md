# Pencil2 additional mixed Drawing scene controls

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

Historical readers: resvg 0.45.1, Pillow 12.1.1, MuPDF 1.28.2 and PDFium 156.0.8076.0. Receipts retain full-frame error and byte counts. The fixed two-object prototype capped each object at 84 frames and rectangle output at 8 MB; 2× SVG reached 2,449,401 bytes. These were not document-wide budgets.

These isolated source-grid transport experiments extend v199 to the retained white 1× and 2×, transparent 1×, and blue 2× native Drawing controls in `../pencil2-mixed-scene-controls`. Both prefixes are checked against native RGBA8 frames; those frames are used only as comparison targets. Stamps derive from the two independently generated source inputs and their record SHA-256 values are asserted against the native geometry hashes. The draw transforms are scale 1 or 2 with translations (6,9) and (12,12); grain offsets are (6,9), (12,13.999998) at 1× and (6,9), (12,16.0000019) at 2×. 2× scales the 1× generated geometry, not source sampling. The native capture is ARM64 emulation with synthetic callback queue and host Mesa; it is not device or saved-page evidence.

The transparent PDF experiment is a negative control: with this control’s default document metadata and render options, the public document renderer resolves the absent page color to opaque light paper (#fcfcfc) and paints it in the generated page, so its approximately 37 million total error reflects the background mismatch. It does not validate transparent PDF transport. The isolated paper-omission experiment in `../pencil2-transparent-transport-v201` tests that transport separately. Other cases show reader-dependent and accumulated color errors; neither this experiment nor v199 justifies enabling a general export profile. Generated generic SVG is incidental and is not used as evidence.
