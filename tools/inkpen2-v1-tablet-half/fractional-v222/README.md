# Fractional trajectory on an odd viewport

This independent synthetic four-point RTV1 host-Mesa capture uses a 63×47 transparent target, 0.5 scale and matrix translation (16.25, 17.375). The public source points add (32.5, 34.75) instead; the resulting page is 126×94. Both use opaque red, tool 3, null setting and the 1440×2560 host profile. The capture has 29 native stamps, a complete framebuffer and no reported GL errors. The frame, input, geometry, hashes and comparisons are retained in `receipt.json` and `frame.rgba.zlib`.

On the 39 native ink pixels, direct-row candidate versus ordinary RGB L1 is 16 versus 1957 in resvg, 85 versus 4110 in MuPDF and 67 versus 3952 in PDFium. Full-frame MuPDF error includes the paper-color conversion. Flipped rows give much larger errors. This single synthetic stroke does not establish saved-note or device parity or general orientation and composition behavior.

The Rust `fractional_odd_viewport_matches_native_frame` test checks the captured
geometry and compares the half-grid alpha against the native frame.

Historical comparisons used resvg 0.45.1, MuPDF 1.28.2 and PDFium
150.0.7869.0, compositing the native premultiplied frame over #fcfcfc. A clean
replay at `a4460e0a7f624d001e17b3b557adda1e61af2669` reproduced the original
four export hashes and metrics. The export hook, patch and comparator have
been retired. The hybrid receipt retains their historical source identities
alongside unique native geometry, matrix, framebuffer and GL evidence; it does
not validate current reader output.
