# Marker3 terminal controls

`capture.py OUTPUT` captures size-8 tool-1 Model/Drawing vectors and host-Mesa RTV1 frames using the pinned `marker3-zero` host inputs. `inside` reproduces the skipped terminal segment; `two` and `near` test two source points above and below the 2-unit rejection threshold. The native Drawing and saved-redraw vectors are retained in the compressed caller receipts; the reduced vectors are also checked bit-for-bit by the Rust test. The RT vector bridge and Mesa host limitations match `marker3-size9-expanded` and do not establish device parity.

Historically at 1×, SVG / MuPDF / PDFium aggregate absolute error against native (with the PDF composed on #fcfcfc) was respectively 16580 / 45524 / 17202 for `inside`, 4544 / 36309 / 4042 for `near`, and 9797 / 40249 / 10001 for `two`. The opt-in profile yielded 5974 / 37237 / 7177, 1467 / 33905 / 1508, and 3186 / 35243 / 4417. In each case neither output misses native SVG support pixels. These are reader-specific controls, not pixel parity.

Historical readers were resvg 0.45.1, PyMuPDF 1.28.2 and pypdfium2 5.14.0 / PDFium 156.0.8076.0; the replay used revision `5b0f0c9b1b91e8f13353cbfe939f1b86563f8385` plus the retired renderer patch. Native `.rgba.zlib` frames remain zlib-decodable.

The maintained `terminal_segment_matches_expanded_native_vectors` Rust test
checks Drawing/saved-redraw geometry for the terminal and expanded controls,
including rejection of the 9.0001 page case. The current tool-1 page size range
is 4–9 subject to its other guards. Historical reader metrics above are not
current external-reader assertions.

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test -p sdocx --lib --features render,serde terminal_segment_matches_expanded_native_vectors
```

The experimental exporter, comparator and replay workflow were retired; native
inputs, frames, receipts and capture scripts remain. `SHA256SUMS` checks retained
files only. Retired source hashes remain in Git history.

From the repository root on the prepared host, use a fresh output directory:

```sh
.venv/bin/python tools/marker3-terminal/capture.py /tmp/marker3-terminal-fresh
```

Compare decompressed frames and native vectors with the retained receipt hashes.

Native recapture needs local Samsung DSOs, Unicorn, AArch64 binutils, Mesa/EGL and the ignored helpers described in `tools/marker3-zero/hashes.json`. The full runtime closure is not provided.
