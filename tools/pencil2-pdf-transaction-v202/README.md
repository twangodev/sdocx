# Isolated Pencil2 PDF object transaction

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

The retired standalone lopdf prototype accepted already-created 192×192 RGBA frames. It checked frame, stream-byte, object-count and object-ID bounds before allocation, staged forms and alpha states in a separate document with IDs above the destination maximum, then appended only after every form succeeded. It did not clone the destination or integrate into the renderer.

Three retired prototype tests exercised two fields and a dense full-frame stream. They checked exact preflight limits, one-below limits, malformed frames, ID overflow, and injected failures during command encoding, alpha-resource creation, and after each staged field against a pre-existing marker stream and trailer. Successful insertion checks form order, prior marker and trailer preservation, two pixel commands, and every one of the 255 alpha-resource references per form. Each emitted command is checked against the 256-byte bound, and the dense-frame test checks the complete stream against its preflight bound. These are real `lopdf::Document` objects but not the renderer's scene registry or a complete renderable page. The byte bound covers form commands only; resource and allocation overhead are bounded by object count, not measured in bytes. The supplied frame buffers and sampler work are outside admission. The transaction does not cover scene marker registration, SVG, serialization, reader output, device fidelity, or process failure/OOM during the final map append. This is not evidence for production admission.
