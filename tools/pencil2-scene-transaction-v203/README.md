# Isolated Pencil2 scene transaction

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

The retired prototype used revision `504b0654`. Its probe checked source-point, frame, work, stream and PDF-object limits, later-stroke rejection and injected failures after each staged stroke. Prior SVG registry, PDF objects and object ID remained unchanged; success produced two ordered fields/forms.

The prototype plans every stroke before raster allocation. It accepts caller-supplied 192×192 grid, 1× or 2× scale, translation and grain offset; it checks finite offsets. The scene bounds aggregate source points, record counts, pixel work, conservative rectangle stream bytes and PDF resource objects/IDs. SVG fields and newly created PDF objects are staged separately and appended only after every stroke succeeds. These are stream bounds, not a bound on the entire serialized PDF, allocator overhead, or other scene content. The PDF forms use unit bounding boxes and require their caller to place them appropriately. The probe does not test serialized PDF or SVG through readers, and it does not establish saved-document or device fidelity. The known v199 paired-reader results and their accumulated errors remain the evidence boundary. This is not production dispatch or a general export profile.

This is a synthetic detached scene harness: the registry is a `Vec<String>` and the destination is a minimal `lopdf::Document`, not the renderer's real scene registry or document export path. Each aggregate limit is checked as strokes are visited so a rejected prefix does not require planning the remaining strokes. No native paired-reader comparison was performed for this new prototype.
