# AirBrush V1 tool 1 diagnostic

This is an experimental candidate, not a production profile. The receipt preserves two synthetic Model/Drawing host-Mesa captures, including original lower-left RGBA frames, source arrays, vector bytes, hashes, and capture prerequisites. It does not establish saved-note or device parity. The capture script requires the original hash-pinned local native runner and dependencies; they are not supplied by a clean checkout. The original capture script hash remains in the receipt; the separate replay script hash records its later exclusive-output change. Do not regenerate the native receipt from reader output.

From a clean checkout of `5cc39d4cc3e84d99a88da398c1404ec23deb8331`, apply `airbrush_v1_tool1_candidate.patch` in an isolated copy, then run:

```sh
airbrush_pdf_dir=$(mktemp -d /tmp/airbrush-tool1-pdfs.XXXXXX)
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 SDOCX_AIRBRUSH_MATRIX_DIR="$airbrush_pdf_dir" cargo test -p sdocx --lib --features pdf tool1_candidate_test -- --nocapture
python3 tools/pen-capture/airbrush_v1_tool1_supersample.py "$airbrush_pdf_dir"
```

The reader diagnostic requires PyMuPDF 1.28.2 and Pillow 12.1.1. It verifies the pinned PDF hashes before comparing 1x, 2x, and 4x MuPDF renders, box-reduced per channel, with the original premultiplied native frame. This is a reader sampling experiment; it does not change the PDF or prove parity at other scales. `airbrush_v1_tool1_verify.py` also checks the original capture or pinned MuPDF and PDFium 150 controls when their dependencies are available.

At 1x MuPDF, the reverse candidate increases RGBA error from 1,558 to 2,216. At 2x and 4x its total error falls below generic, but its alpha error remains higher. The candidate therefore remains diagnostic only; the exact host SVG matches do not resolve the reader regression.
