# Historical destination-scale source diagnostic

The retired synthetic v173 diagnostic generated masks at explicit 1× and 2× viewports, rasterized caller-supplied pattern SVG at 64 and 128 pixels, and compared destination-pixel SVG and SVG-derived PDF with native frames. Captured pixels did not generate candidates. Its 2× display and antialias assumptions were hypotheses, not established native inputs.

On this host the 64-texture 1× SVG/MuPDF/PDFium L1 errors are 27/499/458; 2× are 60203/60967/48773. For the 128 texture they are 7054/7439/7384 and 67725/68452/55454 respectively. SVG and MuPDF compare premultiplied RGBA; PDFium compares RGB on white. Resampling the higher resolution texture is a hypothesis, not evidence that the native capture chose 128. These discrepancies, especially at 2×, prevent document dispatch. Native frame hashes and capture details remain in `v173-receipt.json`; historical source and reader pins remain in `v247-provenance.json`.

Export scaffolding and derived reports have been retired. The [v173 native receipt](v173-receipt.json) retains capture provenance and frame hashes. These measurements do not establish saved-document or device parity.
