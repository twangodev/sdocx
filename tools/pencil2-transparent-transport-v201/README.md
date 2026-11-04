# Transparent Pencil2 direct PDF transport control

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

The retired prototype omitted only the experimental paper rectangle, using v200 native transparent frames, source objects, prefixes and 192×192 viewport. It checked frame hashes, dimensions, a transparent corner and premultiplied RGBA differences. Reader versions and other limits are recorded in v200.

This is a bounded experiment, not a public transparent-page export. The v200 SVG control omits paper explicitly, while its direct PDF uses the public document renderer: with this control’s default document metadata and render options, the absent page color resolves to light paper (#fcfcfc), and the renderer paints a page rectangle. The original v200 result therefore measures a paper mismatch, not an opaque PDF viewer or a failure of PDF transparency. Public SVG uses the same document scene and paper semantics.

For the first and second native prefixes respectively, resvg SVG total channel error was 269 and 1,918; direct PDF was 1,137 and 15,519 with MuPDF, and 185 and 2,045 with PDFium. Full maxima and differing channel counts are in the receipts. This confirms transparency survives both PDF readers for this isolated prototype, with reader-dependent transport differences. It does not establish general public transparent-page support, device parity, or a general export profile.
